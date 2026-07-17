"use strict";

const schema = "vine.win32-trace.v2";
const agentOptions = globalThis.__vineTraceOptions ?? {
  vtables: [],
  observations: [],
};
const selectedVtables = new Set(agentOptions.vtables ?? []);
const selectedObservations = new Set(agentOptions.observations ?? []);
const stopOnEvent = agentOptions.stopOnEvent ?? null;
let sequence = 0;
let nextCallId = 1;
const attached = new Set();
const attachedVtableMethods = new Set();
const observedVtables = new Set();
const activeCallsByThread = new Map();
let criticalSectionOracleScheduled = false;
let criticalSectionOracleSentinel = null;
let criticalSectionOracleLastError = null;
let initializeCriticalSectionConsumersRecorded = 0;
let commandLineOracleScheduled = false;
const commandLineFirstPointers = new Map();
const commandLineLatestPointers = new Map();
let environmentStringsOracleScheduled = false;
let environmentStringsOracleSentinel = null;
let environmentStringsOracleLastError = null;
let wideCharOracleScheduled = false;
let wideCharOracleSentinel = null;
let wideCharOracleLastError = null;
let multiByteOracleScheduled = false;
let multiByteOracleSentinel = null;
let multiByteOracleLastError = null;
const multiByteConsumerKeys = new Set();
let setCurrentDirectoryOracleScheduled = false;
let setCurrentDirectoryOracleSentinel = null;
let setCurrentDirectoryOracleResult = null;
let setCurrentDirectoryOracleLastError = null;
let setCurrentDirectoryConsumersRecorded = 0;
let createMutexOracleScheduled = false;
let createMutexOracleSentinel = null;
let createMutexOracleResult = null;
let createMutexOracleLastError = null;
let createMutexConsumersRecorded = 0;
let mutexLifecycleConsumersRecorded = 0;
const trackedMutexHandles = new Map();
let getStringTypeOracleScheduled = false;
let getStringTypeOracleSentinel = null;
let getStringTypeOracleResult = null;
let getStringTypeOracleLastError = null;
let getStringTypeConsumerRecorded = false;
let lcMapStringOracleScheduled = false;
let lcMapStringOracleSentinel = null;
let lcMapStringOracleResult = null;
let lcMapStringOracleLastError = null;
let lcMapStringConsumersRecorded = 0;
let interlockedDecrementOracleScheduled = false;
let interlockedDecrementOracleSentinel = null;
let interlockedDecrementOracleResult = null;
let interlockedDecrementOracleLastError = null;
let interlockedDecrementConsumersRecorded = 0;
let getModuleFileNameAOracleScheduled = false;
let getModuleFileNameAOracleSentinel = null;
let getModuleFileNameAOracleResult = null;
let getModuleFileNameAOracleLastError = null;
let getModuleFileNameAConsumersRecorded = 0;
let tlsLifecycleOracleScheduled = false;
let tlsGetValueConsumerRecorded = false;
let flsLifecycleOracleScheduled = false;
let flsGetValueConsumerRecorded = false;
let getLastErrorOracleScheduled = false;
let getLastErrorOracleSentinel = null;
let getLastErrorOracleResult = null;
let getLastErrorOracleLastError = null;
let setLastErrorOracleScheduled = false;
let setLastErrorOracleActive = false;
let setLastErrorOracleArgument = null;
let setLastErrorOracleEaxBefore = null;
let setLastErrorOracleEaxAfter = null;
let setLastErrorOracleLastErrorAfter = null;
let setLastErrorConsumerRecorded = false;
let getAcpOracleScheduled = false;
let getAcpOracleSentinel = null;
let getAcpOracleResult = null;
let getAcpOracleLastError = null;
let getAcpConsumerRecorded = false;
let systemDefaultLangIdOracleScheduled = false;
let systemDefaultLangIdOracleSentinel = null;
let systemDefaultLangIdOracleResult = null;
let systemDefaultLangIdOracleLastError = null;
let systemDefaultLangIdConsumerRecorded = false;
let driveTypeOracleScheduled = false;
let driveTypeOracleSentinel = null;
let driveTypeOracleResult = null;
let driveTypeOracleLastError = null;
let driveTypeConsumerRecorded = false;
let driveTypeScanCompletionRecorded = false;
let driveTypeMainModuleCallCount = 0;
let swapMouseButtonStateScheduled = false;
let swapMouseButtonInitialState = null;
let swapMouseButtonGetSystemMetrics = null;
let swapMouseButtonRestore = null;
let swapMouseButtonRestoreActive = false;
let swapMouseButtonConsumerRecorded = false;
let swapMouseButtonCompletionRecorded = false;
const swapMouseButtonMainModuleCalls = [];
let timeGetDevCapsOracleScheduled = false;
let timeGetDevCapsOracleCall = null;
let timePeriodOracleCall = null;
let timeGetDevCapsOracleComplete = false;
let timeGetDevCapsOracleEvidence = null;
let timePeriodOracleEvidence = null;
let timeGetDevCapsConsumerRecorded = false;
let timeGetDevCapsCompletionRecorded = false;
let timeGetDevCapsDrainCheckScheduled = false;
const timeGetDevCapsMainModuleCalls = [];
const timePeriodMainModuleCalls = [];
let getVersionExAOracleScheduled = false;
let getVersionExAOracleCall = null;
let getVersionExAOracleComplete = false;
let getVersionExAOracleEvidence = null;
let getVersionExACompletionRecorded = false;
let getVersionExADrainCheckScheduled = false;
const getVersionExAMainModuleCalls = [];
let createWaitableTimerAOracleScheduled = false;
let createWaitableTimerAOracleCall = null;
let createWaitableTimerAOracleComplete = false;
let createWaitableTimerAOracleEvidence = null;
let createWaitableTimerACompletionRecorded = false;
let createWaitableTimerADrainCheckScheduled = false;
const createWaitableTimerAMainModuleCalls = [];
let loadIconAOracleScheduled = false;
let loadIconAOracleCall = null;
let loadIconAOracleComplete = false;
let loadIconAOracleEvidence = null;
let loadIconACompletionRecorded = false;
let loadIconADrainCheckScheduled = false;
const loadIconAMainModuleCalls = [];
let loadCursorAOracleScheduled = false;
let loadCursorAOracleCall = null;
let loadCursorAOracleComplete = false;
let loadCursorAOracleEvidence = null;
let loadCursorACompletionRecorded = false;
let loadCursorADrainCheckScheduled = false;
const loadCursorAMainModuleCalls = [];
let getCpInfoOracleScheduled = false;
let getCpInfoOracleSentinel = null;
let getCpInfoOracleResult = null;
let getCpInfoOracleLastError = null;
let getCpInfoConsumerRecorded = false;
let processorFeatureOracleScheduled = false;
let processorFeatureOracleSentinel = null;
let processorFeatureOracleResult = null;
let processorFeatureOracleLastError = null;
let processorFeatureConsumersRecorded = 0;
let unhandledExceptionFilterOracleScheduled = false;
let unhandledExceptionFilterOracleSentinel = null;
let unhandledExceptionFilterOracleResult = null;
let unhandledExceptionFilterOracleLastError = null;
let unhandledExceptionFilterConsumersRecorded = 0;
const unhandledExceptionFilterCallbacks = [];
let heapSizeOracleScheduled = false;
let heapSizeOracleSentinel = null;
let heapSizeOracleResult = null;
let heapSizeOracleLastError = null;
let heapSizeConsumersRecorded = 0;
const observedMainModuleHeapAllocations = new Map();
let getCurrentThreadOracleScheduled = false;
let setThreadAffinityMaskOracleScheduled = false;
let setThreadAffinityMaskOracleSentinel = null;
let setThreadAffinityMaskOracleResult = null;
let setThreadAffinityMaskOracleLastError = null;
let setThreadAffinityMaskConsumersRecorded = 0;
let setThreadAffinityMaskPrimaryCalls = 0;
let setThreadAffinityCalibration = null;
let getSystemInfoOracleScheduled = false;
let getSystemInfoOracleSentinel = null;
let getSystemInfoOracleLastError = null;
let getSystemInfoConsumerRecorded = false;
let enumDisplayMonitorsOracleScheduled = false;
let enumDisplayMonitorsOracleSentinel = null;
let enumDisplayMonitorsOracleResult = null;
let enumDisplayMonitorsOracleLastError = null;
let enumDisplayMonitorsConsumerRecorded = false;
const enumDisplayMonitorsOracleCallbacks = [];
const activeEnumDisplayMonitorsByThread = new Map();
const attachedEnumDisplayMonitorCallbacks = new Set();
let comInitializationOracleScheduled = false;
let comInitializationOracleCall = null;
let comInitializationConsumerRecorded = false;

function snapshotNulTerminatedString(pointer, encoding) {
  const maximumBytes = 4096;
  const unitBytes = encoding === "utf16" ? 2 : 1;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      encoding,
      maximumBytes,
      error: "null pointer",
    };
  }
  try {
    let byteLengthWithNul = null;
    for (let offset = 0; offset + unitBytes <= maximumBytes; offset += unitBytes) {
      const terminator =
        unitBytes === 1
          ? pointer.add(offset).readU8() === 0
          : pointer.add(offset).readU16() === 0;
      if (terminator) {
        byteLengthWithNul = offset + unitBytes;
        break;
      }
    }
    if (byteLengthWithNul === null) {
      return {
        pointer: pointer.toString(),
        encoding,
        maximumBytes,
        error: "terminator not found within bounded observation",
      };
    }
    const buffer = pointer.readByteArray(byteLengthWithNul);
    const bytes = [...new Uint8Array(buffer)];
    const api = encoding === "utf16" ? "GetCommandLineW" : "GetCommandLineA";
    const firstPointer = commandLineFirstPointers.get(api) ?? pointer.toString();
    commandLineFirstPointers.set(api, firstPointer);
    commandLineLatestPointers.set(api, pointer.toString());
    const otherApi = api === "GetCommandLineA" ? "GetCommandLineW" : "GetCommandLineA";
    const otherPointer = commandLineLatestPointers.get(otherApi) ?? null;
    return {
      pointer: pointer.toString(),
      encoding,
      maximumBytes,
      byteLengthWithNul,
      bytes,
      string: readString(pointer, encoding),
      equalsFirstPointer: pointer.toString() === firstPointer,
      otherApi,
      otherPointer,
      equalsOtherEncodingPointer:
        otherPointer === null ? null : pointer.toString() === otherPointer,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      encoding,
      maximumBytes,
      error: error.message,
    };
  }
}

function fnv1aByte(hash, byte) {
  return Math.imul((hash ^ byte) >>> 0, 0x01000193) >>> 0;
}

function signed32(value) {
  const unsigned = value.toUInt32();
  return unsigned > 0x7fffffff ? unsigned - 0x100000000 : unsigned;
}

function snapshotWideCharSource(pointer, requestedUnits) {
  const maximumUnits = 512 * 1024;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      error: "null pointer",
    };
  }
  if (requestedUnits === 0 || requestedUnits < -1) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      error: "invalid UTF-16 unit count",
    };
  }
  try {
    let observedUnits = requestedUnits;
    if (requestedUnits === -1) {
      observedUnits = null;
      for (let index = 0; index < maximumUnits; index += 1) {
        if (pointer.add(index * 2).readU16() === 0) {
          observedUnits = index + 1;
          break;
        }
      }
      if (observedUnits === null) {
        return {
          pointer: pointer.toString(),
          requestedUnits,
          maximumUnits,
          error: "UTF-16 NUL not found within bounded observation",
        };
      }
    }
    if (observedUnits > maximumUnits) {
      return {
        pointer: pointer.toString(),
        requestedUnits,
        observedUnits,
        maximumUnits,
        error: "UTF-16 input exceeds bounded observation",
      };
    }
    let hash = 0x811c9dc5;
    let nulUnits = 0;
    let asciiUnits = 0;
    let nonAsciiBmpUnits = 0;
    let highSurrogates = 0;
    let lowSurrogates = 0;
    let lastUnit = null;
    for (let index = 0; index < observedUnits; index += 1) {
      const unit = pointer.add(index * 2).readU16();
      hash = fnv1aByte(hash, unit & 0xff);
      hash = fnv1aByte(hash, unit >>> 8);
      lastUnit = unit;
      if (unit === 0) {
        nulUnits += 1;
      } else if (unit <= 0x7f) {
        asciiUnits += 1;
      } else if (unit >= 0xd800 && unit <= 0xdbff) {
        highSurrogates += 1;
      } else if (unit >= 0xdc00 && unit <= 0xdfff) {
        lowSurrogates += 1;
      } else {
        nonAsciiBmpUnits += 1;
      }
    }
    return {
      pointer: pointer.toString(),
      requestedUnits,
      observedUnits,
      maximumUnits,
      byteLength: observedUnits * 2,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      nulUnits,
      asciiUnits,
      nonAsciiBmpUnits,
      highSurrogates,
      lowSurrogates,
      endsWithNul: lastUnit === 0,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      error: error.message,
    };
  }
}

function snapshotMultiByteSource(pointer, requestedBytes) {
  const maximumBytes = 1024 * 1024;
  const maximumRecordedBytes = 4096;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "null pointer",
    };
  }
  if (requestedBytes === 0 || requestedBytes < -1) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "invalid multibyte byte count",
    };
  }
  try {
    let observedBytes = requestedBytes;
    if (requestedBytes === -1) {
      observedBytes = null;
      for (let index = 0; index < maximumBytes; index += 1) {
        if (pointer.add(index).readU8() === 0) {
          observedBytes = index + 1;
          break;
        }
      }
      if (observedBytes === null) {
        return {
          pointer: pointer.toString(),
          requestedBytes,
          maximumBytes,
          error: "multibyte NUL not found within bounded observation",
        };
      }
    }
    if (observedBytes > maximumBytes) {
      return {
        pointer: pointer.toString(),
        requestedBytes,
        observedBytes,
        maximumBytes,
        error: "multibyte input exceeds bounded observation",
      };
    }
    const buffer = pointer.readByteArray(observedBytes);
    const bytes = [...new Uint8Array(buffer)];
    let hash = 0x811c9dc5;
    let nulBytes = 0;
    let asciiBytes = 0;
    for (const byte of bytes) {
      hash = fnv1aByte(hash, byte);
      if (byte === 0) {
        nulBytes += 1;
      }
      if (byte <= 0x7f) {
        asciiBytes += 1;
      }
    }
    return {
      pointer: pointer.toString(),
      requestedBytes,
      observedBytes,
      maximumBytes,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      nulBytes,
      asciiBytes,
      bytes:
        observedBytes <= maximumRecordedBytes
          ? bytes
          : bytes.slice(0, maximumRecordedBytes),
      bytesTruncated: observedBytes > maximumRecordedBytes,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: error.message,
    };
  }
}

function snapshotCurrentDirectoryA(module) {
  const capacity = 32 * 1024;
  try {
    const address = findExport(module, "GetCurrentDirectoryA");
    if (address === null) {
      return { capacity, error: "GetCurrentDirectoryA export is unavailable" };
    }
    const getCurrentDirectoryA = new SystemFunction(
      address,
      "uint32",
      ["uint32", "pointer"],
      "stdcall",
    );
    const buffer = Memory.alloc(capacity);
    buffer.writeByteArray(Array(capacity).fill(0xa5));
    const outcome = getCurrentDirectoryA(capacity, buffer);
    const result = Number(outcome.value) >>> 0;
    const systemLastError = Number(outcome.lastError) >>> 0;
    return {
      capacity,
      result,
      systemLastError,
      path:
        result > 0 && result < capacity
          ? snapshotMultiByteSource(buffer, -1)
          : null,
    };
  } catch (error) {
    return { capacity, error: error.message };
  }
}

function snapshotByteBuffer(pointer, requestedBytes) {
  const maximumBytes = 1024 * 1024;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      isNull: true,
    };
  }
  if (requestedBytes < 0 || requestedBytes > maximumBytes) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "byte buffer length is outside the bounded observation",
    };
  }
  try {
    const buffer = pointer.readByteArray(requestedBytes);
    const bytes = new Uint8Array(buffer);
    let hash = 0x811c9dc5;
    let nulBytes = 0;
    for (const byte of bytes) {
      hash = fnv1aByte(hash, byte);
      if (byte === 0) {
        nulBytes += 1;
      }
    }
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      isNull: false,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      nulBytes,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: error.message,
    };
  }
}

function snapshotWideCharBuffer(pointer, requestedUnits) {
  const maximumUnits = 512 * 1024;
  const maximumRecordedUnits = 4096;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      isNull: true,
    };
  }
  if (requestedUnits < 0 || requestedUnits > maximumUnits) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      error: "UTF-16 buffer length is outside the bounded observation",
    };
  }
  try {
    const units = Array.from({ length: requestedUnits }, (_, index) =>
      pointer.add(index * 2).readU16(),
    );
    let hash = 0x811c9dc5;
    let nulUnits = 0;
    for (const unit of units) {
      hash = fnv1aByte(hash, unit & 0xff);
      hash = fnv1aByte(hash, unit >>> 8);
      if (unit === 0) {
        nulUnits += 1;
      }
    }
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      isNull: false,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      nulUnits,
      units:
        requestedUnits <= maximumRecordedUnits
          ? units
          : units.slice(0, maximumRecordedUnits),
      unitsTruncated: requestedUnits > maximumRecordedUnits,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      error: error.message,
    };
  }
}

function snapshotWordBuffer(pointer, requestedWords) {
  const maximumWords = 512 * 1024;
  const maximumRecordedWords = 4096;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedWords,
      maximumWords,
      isNull: true,
    };
  }
  if (requestedWords < 0 || requestedWords > maximumWords) {
    return {
      pointer: pointer.toString(),
      requestedWords,
      maximumWords,
      error: "WORD buffer length is outside the bounded observation",
    };
  }
  try {
    const words = Array.from({ length: requestedWords }, (_, index) =>
      pointer.add(index * 2).readU16(),
    );
    let hash = 0x811c9dc5;
    let zeroWords = 0;
    let bitwiseOr = 0;
    let bitwiseAnd = 0xffff;
    for (const word of words) {
      hash = fnv1aByte(hash, word & 0xff);
      hash = fnv1aByte(hash, word >>> 8);
      if (word === 0) {
        zeroWords += 1;
      }
      bitwiseOr |= word;
      bitwiseAnd &= word;
    }
    return {
      pointer: pointer.toString(),
      requestedWords,
      maximumWords,
      isNull: false,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      zeroWords,
      bitwiseOr,
      bitwiseAnd,
      words:
        requestedWords <= maximumRecordedWords
          ? words
          : words.slice(0, maximumRecordedWords),
      wordsTruncated: requestedWords > maximumRecordedWords,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedWords,
      maximumWords,
      error: error.message,
    };
  }
}

function snapshotBoundedRawBytes(pointer, requestedBytes, maximumBytes = 64) {
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "null pointer",
    };
  }
  if (
    !Number.isInteger(requestedBytes) ||
    requestedBytes < 0 ||
    requestedBytes > maximumBytes
  ) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "raw byte length is outside the bounded observation",
    };
  }
  try {
    const buffer = pointer.readByteArray(requestedBytes);
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      bytes: [...new Uint8Array(buffer)],
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: error.message,
    };
  }
}

function snapshotMeasuredCpInfo(pointer) {
  const measuredBytes = 18;
  const raw = snapshotBoundedRawBytes(pointer, measuredBytes);
  if (raw.error !== undefined) {
    return {
      profile: "measured-x86-cpinfo-fields-v1",
      measuredBytes,
      raw,
    };
  }
  try {
    const leadByte = Array.from({ length: 12 }, (_, index) =>
      pointer.add(6 + index).readU8(),
    );
    const leadBytePairs = Array.from({ length: 6 }, (_, index) => [
      leadByte[index * 2],
      leadByte[index * 2 + 1],
    ]);
    return {
      profile: "measured-x86-cpinfo-fields-v1",
      measuredBytes,
      raw,
      fields: {
        maxCharSize: pointer.readU32(),
        defaultChar: [pointer.add(4).readU8(), pointer.add(5).readU8()],
        leadByte,
        leadBytePairs,
      },
    };
  } catch (error) {
    return {
      profile: "measured-x86-cpinfo-fields-v1",
      measuredBytes,
      raw,
      error: error.message,
    };
  }
}

function snapshotSystemInfo(pointer) {
  const measuredBytes = 36;
  const raw = snapshotBoundedRawBytes(pointer, measuredBytes);
  if (raw.error !== undefined) {
    return {
      profile: "x86-system-info-layout-v1",
      measuredBytes,
      raw,
    };
  }
  if (Process.pointerSize !== 4) {
    return {
      profile: "x86-system-info-layout-v1",
      measuredBytes,
      raw,
      error: `x86 SYSTEM_INFO observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  try {
    return {
      profile: "x86-system-info-layout-v1",
      measuredBytes,
      raw,
      fields: {
        processorArchitecture: pointer.readU16(),
        reserved: pointer.add(2).readU16(),
        pageSize: pointer.add(4).readU32(),
        minimumApplicationAddress: pointer.add(8).readU32(),
        maximumApplicationAddress: pointer.add(12).readU32(),
        activeProcessorMask: pointer.add(16).readU32(),
        numberOfProcessors: pointer.add(20).readU32(),
        processorType: pointer.add(24).readU32(),
        allocationGranularity: pointer.add(28).readU32(),
        processorLevel: pointer.add(32).readU16(),
        processorRevision: pointer.add(34).readU16(),
      },
    };
  } catch (error) {
    return {
      profile: "x86-system-info-layout-v1",
      measuredBytes,
      raw,
      error: error.message,
    };
  }
}

function snapshotTimeCaps(pointer) {
  const measuredBytes = 8;
  const raw = snapshotBoundedRawBytes(pointer, measuredBytes);
  if (raw.error !== undefined) {
    return {
      profile: "x86-timecaps-layout-v1",
      measuredBytes,
      raw,
    };
  }
  try {
    return {
      profile: "x86-timecaps-layout-v1",
      measuredBytes,
      raw,
      fields: {
        periodMinimumMilliseconds: pointer.readU32(),
        periodMaximumMilliseconds: pointer.add(4).readU32(),
      },
    };
  } catch (error) {
    return {
      profile: "x86-timecaps-layout-v1",
      measuredBytes,
      raw,
      error: error.message,
    };
  }
}

function snapshotOsVersionInfoA(pointer) {
  const profile = "x86-os-version-info-a-layout-v1";
  const baseBytes = 148;
  const extendedBytes = 156;
  if (pointer.isNull()) {
    return {
      profile,
      baseBytes,
      extendedBytes,
      error: "null pointer",
    };
  }
  try {
    const declaredBytes = pointer.readU32();
    const measuredBytes =
      declaredBytes >= extendedBytes
        ? extendedBytes
        : declaredBytes >= baseBytes
          ? baseBytes
          : 4;
    const raw = snapshotBoundedRawBytes(pointer, measuredBytes, extendedBytes);
    if (declaredBytes < baseBytes || raw.error !== undefined) {
      return {
        profile,
        baseBytes,
        extendedBytes,
        declaredBytes,
        measuredBytes,
        raw,
      };
    }
    const servicePackBytes = raw.bytes.slice(20, baseBytes);
    const terminator = servicePackBytes.indexOf(0);
    const servicePackBytesWithoutNul =
      terminator < 0 ? servicePackBytes : servicePackBytes.slice(0, terminator);
    return {
      profile,
      baseBytes,
      extendedBytes,
      declaredBytes,
      measuredBytes,
      raw,
      fields: {
        majorVersion: pointer.add(4).readU32(),
        minorVersion: pointer.add(8).readU32(),
        buildNumber: pointer.add(12).readU32(),
        platformId: pointer.add(16).readU32(),
        servicePackBytes: servicePackBytesWithoutNul,
        servicePackTerminated: terminator >= 0,
        ...(declaredBytes < extendedBytes
          ? {}
          : {
              servicePackMajor: pointer.add(148).readU16(),
              servicePackMinor: pointer.add(150).readU16(),
              suiteMask: pointer.add(152).readU16(),
              productType: pointer.add(154).readU8(),
              reserved: pointer.add(155).readU8(),
            }),
      },
    };
  } catch (error) {
    return {
      profile,
      baseBytes,
      extendedBytes,
      error: error.message,
    };
  }
}

function snapshotRect(pointer) {
  const raw = snapshotBoundedRawBytes(pointer, 16);
  if (raw.error !== undefined) {
    return { profile: "x86-rect-v1", raw };
  }
  try {
    return {
      profile: "x86-rect-v1",
      raw,
      fields: {
        left: pointer.readS32(),
        top: pointer.add(4).readS32(),
        right: pointer.add(8).readS32(),
        bottom: pointer.add(12).readS32(),
      },
    };
  } catch (error) {
    return { profile: "x86-rect-v1", raw, error: error.message };
  }
}

function attachEnumDisplayMonitorCallback(callback) {
  if (callback.isNull()) {
    return;
  }
  const key = callback.toString().toLowerCase();
  if (attachedEnumDisplayMonitorCallbacks.has(key)) {
    return;
  }
  attachedEnumDisplayMonitorCallbacks.add(key);
  Interceptor.attach(callback, {
    onEnter(args) {
      const threadId = Process.getCurrentThreadId();
      const active = activeEnumDisplayMonitorsByThread.get(threadId) ?? null;
      this.enumDisplayMonitorsActive = active;
      this.enumDisplayMonitorsRect = args[2];
      if (active === null) {
        this.enumDisplayMonitorsCallback = null;
        return;
      }
      const record = {
        callback: callback.toString(),
        caller: describeCaller(this.returnAddress),
        monitor: args[0].toString(),
        monitorDc: args[1].toString(),
        rectBefore: snapshotRect(args[2]),
        data: args[3].toString(),
        lastErrorBefore: Number(this.lastError) >>> 0,
      };
      this.enumDisplayMonitorsCallback = record;
      active.callbacks.push(record);
    },
    onLeave(result) {
      const record = this.enumDisplayMonitorsCallback;
      if (record === null) {
        return;
      }
      record.result = result.toUInt32();
      record.rectAfter = snapshotRect(this.enumDisplayMonitorsRect);
      record.lastErrorAfter = Number(this.lastError) >>> 0;
    },
  });
}

function snapshotOptionalBool(pointer) {
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), isNull: true };
  }
  try {
    return {
      pointer: pointer.toString(),
      isNull: false,
      value: pointer.readU32(),
    };
  } catch (error) {
    return { pointer: pointer.toString(), isNull: false, error: error.message };
  }
}

function snapshotInstructions(address, maximumInstructions) {
  try {
    const instructions = [];
    let cursor = address;
    for (let index = 0; index < maximumInstructions; index += 1) {
      const instruction = Instruction.parse(cursor);
      instructions.push({
        address: instruction.address.toString(),
        mnemonic: instruction.mnemonic,
        opStr: instruction.opStr,
        size: instruction.size,
      });
      cursor = instruction.next;
    }
    return {
      address: address.toString(),
      maximumInstructions,
      instructions,
    };
  } catch (error) {
    return {
      address: address.toString(),
      maximumInstructions,
      error: error.message,
    };
  }
}

function snapshotInstructionsFollowingDirectJump(address, maximumInstructions) {
  const entry = snapshotInstructions(address, maximumInstructions);
  try {
    const instruction = Instruction.parse(address);
    const operand = instruction.operands?.[0] ?? null;
    if (instruction.mnemonic !== "jmp" || operand?.type !== "imm") {
      return { entry, directJumpTarget: null };
    }
    const target = ptr(operand.value);
    return {
      entry,
      directJumpTarget: snapshotInstructions(target, maximumInstructions),
    };
  } catch (error) {
    return {
      entry,
      directJumpTarget: { error: error.message },
    };
  }
}

function snapshotX86FrameCallerContinuation(context, maximumInstructions) {
  if (Process.arch !== "ia32" || context.ebp === undefined) {
    return { error: "x86 EBP frame is unavailable" };
  }
  try {
    const framePointer = ptr(context.ebp);
    const address = framePointer.add(Process.pointerSize).readPointer();
    return {
      framePointer: framePointer.toString(),
      ...snapshotInstructions(address, maximumInstructions),
    };
  } catch (error) {
    return { error: error.message };
  }
}

function snapshotX86FrameCallerContinuations(
  context,
  maximumFrames,
  maximumInstructions,
) {
  if (Process.arch !== "ia32" || context.ebp === undefined) {
    return { error: "x86 EBP frame is unavailable" };
  }
  const frames = [];
  let framePointer = ptr(context.ebp);
  for (let depth = 0; depth < maximumFrames; depth += 1) {
    try {
      const address = framePointer.add(Process.pointerSize).readPointer();
      frames.push({
        depth,
        framePointer: framePointer.toString(),
        caller: describeCaller(address),
        ...snapshotInstructions(address, maximumInstructions),
      });
      const nextFramePointer = framePointer.readPointer();
      if (nextFramePointer.isNull() || nextFramePointer.equals(framePointer)) {
        break;
      }
      framePointer = nextFramePointer;
    } catch (error) {
      return { maximumFrames, maximumInstructions, frames, error: error.message };
    }
  }
  return { maximumFrames, maximumInstructions, frames };
}

function snapshotX86FrameCallerContinuationsFollowingDirectJump(
  context,
  maximumFrames,
  maximumInstructions,
) {
  if (Process.arch !== "ia32" || context.ebp === undefined) {
    return { error: "x86 EBP frame is unavailable" };
  }
  const frames = [];
  let framePointer = ptr(context.ebp);
  for (let depth = 0; depth < maximumFrames; depth += 1) {
    try {
      const address = framePointer.add(Process.pointerSize).readPointer();
      frames.push({
        depth,
        framePointer: framePointer.toString(),
        caller: describeCaller(address),
        continuation: snapshotInstructionsFollowingDirectJump(
          address,
          maximumInstructions,
        ),
      });
      const nextFramePointer = framePointer.readPointer();
      if (nextFramePointer.isNull() || nextFramePointer.equals(framePointer)) {
        break;
      }
      framePointer = nextFramePointer;
    } catch (error) {
      return { maximumFrames, maximumInstructions, frames, error: error.message };
    }
  }
  return { maximumFrames, maximumInstructions, frames };
}

function snapshotTlsGetValueCall(caller, returnAddress, lastErrorBefore) {
  const recordsConsumer =
    !tlsGetValueConsumerRecorded && caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    tlsGetValueConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    immediateConsumer: recordsConsumer
      ? snapshotInstructions(returnAddress, 6)
      : null,
  };
}

function snapshotFlsGetValueCall(caller, returnAddress, lastErrorBefore) {
  const recordsConsumer =
    !flsGetValueConsumerRecorded && caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    flsGetValueConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    immediateConsumer: recordsConsumer
      ? snapshotInstructions(returnAddress, 6)
      : null,
  };
}

function snapshotSetLastErrorCall(caller, returnAddress, lastErrorBefore, eaxBefore) {
  const recordsConsumer =
    !setLastErrorConsumerRecorded && caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    setLastErrorConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    eaxBefore: eaxBefore.toUInt32(),
    immediateConsumer: recordsConsumer
      ? snapshotInstructions(returnAddress, 6)
      : null,
  };
}

function snapshotGetAcpCall(caller, returnAddress, lastErrorBefore, context) {
  const recordsConsumer =
    !getAcpConsumerRecorded && caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    getAcpConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    immediateConsumer: recordsConsumer
      ? {
          apiContinuation: snapshotInstructionsFollowingDirectJump(
            returnAddress,
            32,
          ),
          frameCallerContinuation: snapshotX86FrameCallerContinuation(context, 8),
        }
      : null,
  };
}

function snapshotGetCpInfoCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  codePage,
  output,
) {
  const recordsConsumer =
    !getCpInfoConsumerRecorded &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    getCpInfoConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    getCpInfo: {
      codePage: codePage.toUInt32(),
      outputBefore: snapshotBoundedRawBytes(output, 32),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              4,
              12,
            ),
          }
        : null,
    },
  };
}

function snapshotGetSystemInfoCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  output,
) {
  const recordsConsumer =
    !getSystemInfoConsumerRecorded &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    getSystemInfoConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    getSystemInfo: {
      outputBefore: snapshotSystemInfo(output),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              64,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              6,
              16,
            ),
          }
        : null,
    },
  };
}

function snapshotProcessorFeatureCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  feature,
) {
  const recordsConsumer =
    processorFeatureConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    processorFeatureConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    processorFeature: {
      index: feature.toUInt32(),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              4,
              12,
            ),
          }
        : null,
    },
  };
}

function snapshotSetThreadAffinityMaskCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  threadHandle,
  affinityMask,
) {
  const recordsConsumer =
    setThreadAffinityMaskConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    setThreadAffinityMaskConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    setThreadAffinityMask: {
      threadHandle: threadHandle.toString(),
      affinityMask: affinityMask.toUInt32(),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              8,
              12,
            ),
          }
        : null,
    },
  };
}

function snapshotUnhandledExceptionFilterCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  filter,
) {
  const recordsConsumer =
    unhandledExceptionFilterConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    unhandledExceptionFilterConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    unhandledExceptionFilter: {
      filter: filter.toString(),
      filterIdentity: filter.isNull() ? null : describeCaller(filter),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              4,
              12,
            ),
          }
        : null,
    },
  };
}

function snapshotMultiByteToWideCharCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  codePage,
  flags,
  source,
  sourceBytes,
  destination,
  destinationUnits,
) {
  const consumerKey =
    caller.module !== null && caller.module.toLowerCase() === "bgi.exe"
      ? `${caller.module.toLowerCase()}:${caller.rva ?? caller.address}`
      : null;
  const recordsConsumer =
    consumerKey !== null && !multiByteConsumerKeys.has(consumerKey);
  if (recordsConsumer) {
    multiByteConsumerKeys.add(consumerKey);
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    multiByteToWideChar: {
      codePage: codePage.toUInt32(),
      flags: flags.toUInt32(),
      source: snapshotMultiByteSource(source, sourceBytes),
      destinationBefore: snapshotWideCharBuffer(
        destination,
        Math.max(0, destinationUnits),
      ),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              4,
              12,
            ),
          }
        : null,
    },
  };
}

function snapshotSetCurrentDirectoryACall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  path,
  currentDirectoryBefore,
) {
  const recordsConsumer =
    setCurrentDirectoryConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    setCurrentDirectoryConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    setCurrentDirectoryA: {
      pathBefore: snapshotMultiByteSource(path, -1),
      currentDirectoryBefore,
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function snapshotCreateMutexACall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  securityAttributes,
  initialOwner,
  name,
) {
  const recordsConsumer =
    createMutexConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    createMutexConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    createMutexA: {
      securityAttributes: securityAttributes.toString(),
      initialOwner: initialOwner.toUInt32(),
      nameBefore: snapshotMultiByteSource(name, -1),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function snapshotGetStringTypeWCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  infoType,
  source,
  sourceUnits,
  output,
) {
  const recordsConsumer =
    !getStringTypeConsumerRecorded &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    getStringTypeConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    getStringTypeW: {
      infoType: infoType.toUInt32(),
      sourceCount: sourceUnits,
      source: snapshotWideCharSource(source, sourceUnits < 0 ? -1 : sourceUnits),
      sourceContent: snapshotWordBuffer(source, Math.max(0, sourceUnits)),
      outputBefore: snapshotWordBuffer(output, Math.max(0, sourceUnits)),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function snapshotLCMapStringWCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  locale,
  flags,
  source,
  sourceUnits,
  destination,
  destinationUnits,
) {
  const recordsConsumer =
    lcMapStringConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    lcMapStringConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    lcMapStringW: {
      locale: locale.toUInt32(),
      flags: flags.toUInt32(),
      sourceCount: sourceUnits,
      destinationCount: destinationUnits,
      source: snapshotWideCharSource(source, sourceUnits < 0 ? -1 : sourceUnits),
      sourceContent: snapshotWordBuffer(source, Math.max(0, sourceUnits)),
      destinationBefore: snapshotWideCharBuffer(
        destination,
        Math.max(0, destinationUnits),
      ),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function snapshotInterlockedDecrementCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  target,
) {
  const recordsConsumer =
    interlockedDecrementConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    interlockedDecrementConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    interlockedDecrement: {
      targetBefore: snapshotLong(target),
      targetAlignment: target.toUInt32() % 4,
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function snapshotAnsiOutputBuffer(pointer, requestedBytes) {
  const maximumBytes = 32 * 1024;
  const maximumRecordedBytes = 4096;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      isNull: true,
    };
  }
  if (
    !Number.isInteger(requestedBytes) ||
    requestedBytes < 0 ||
    requestedBytes > maximumBytes
  ) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "ANSI buffer length is outside the bounded observation",
    };
  }
  try {
    const buffer = pointer.readByteArray(requestedBytes);
    const bytes = [...new Uint8Array(buffer)];
    let hash = 0x811c9dc5;
    let nulBytes = 0;
    let firstNulOffset = null;
    for (const [index, byte] of bytes.entries()) {
      hash = fnv1aByte(hash, byte);
      if (byte === 0) {
        nulBytes += 1;
        firstNulOffset ??= index;
      }
    }
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      isNull: false,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      nulBytes,
      firstNulOffset,
      bytes:
        requestedBytes <= maximumRecordedBytes
          ? bytes
          : bytes.slice(0, maximumRecordedBytes),
      bytesTruncated: requestedBytes > maximumRecordedBytes,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: error.message,
    };
  }
}

function snapshotGetModuleFileNameACall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  moduleHandle,
  output,
  capacity,
) {
  const recordsConsumer =
    getModuleFileNameAConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    getModuleFileNameAConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    getModuleFileNameA: {
      moduleHandle: moduleHandle.toString(),
      capacity,
      outputBefore: snapshotAnsiOutputBuffer(output, capacity),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function readProcessParametersEnvironmentPointer() {
  try {
    const ntdll = Process.getModuleByName("ntdll.dll");
    const address = findExport(ntdll, "RtlGetCurrentPeb");
    if (address === null) {
      return { error: "RtlGetCurrentPeb export is unavailable" };
    }
    const peb = new NativeFunction(address, "pointer", [], "stdcall")();
    const processParameters = peb.add(0x10).readPointer();
    const environment = processParameters.add(0x48).readPointer();
    return {
      peb: peb.toString(),
      processParameters: processParameters.toString(),
      environment: environment.toString(),
    };
  } catch (error) {
    return { error: error.message };
  }
}

function snapshotUtf16EnvironmentBlock(pointer, processEnvironmentPointer) {
  const maximumBytes = 1024 * 1024;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      encoding: "utf16-environment-block",
      maximumBytes,
      error: "null pointer",
    };
  }
  try {
    const entryUtf16UnitLengths = [];
    const foldedNames = [];
    let pseudoDriveEntryCount = 0;
    let entriesMissingAssignmentSeparator = 0;
    let current = [];
    let previousWasNul = false;
    let byteLengthWithDoubleNul = null;
    let hash = 0x811c9dc5;
    for (let offset = 0; offset + 2 <= maximumBytes; offset += 2) {
      const unit = pointer.add(offset).readU16();
      hash = fnv1aByte(hash, unit & 0xff);
      hash = fnv1aByte(hash, unit >>> 8);
      if (unit === 0) {
        if (previousWasNul) {
          byteLengthWithDoubleNul = offset + 2;
          break;
        }
        if (current.length > 0) {
          entryUtf16UnitLengths.push(current.length);
          const assignmentStart = current[0] === 0x3d ? 1 : 0;
          const assignmentIndex = current.indexOf(0x3d, assignmentStart);
          if (assignmentIndex < 0) {
            entriesMissingAssignmentSeparator += 1;
          } else {
            if (current[0] === 0x3d) {
              pseudoDriveEntryCount += 1;
            }
            foldedNames.push(
              String.fromCharCode(...current.slice(0, assignmentIndex)).toUpperCase(),
            );
          }
        }
        current = [];
        previousWasNul = true;
      } else {
        current.push(unit);
        previousWasNul = false;
      }
    }
    if (byteLengthWithDoubleNul === null) {
      return {
        pointer: pointer.toString(),
        encoding: "utf16-environment-block",
        maximumBytes,
        error: "double-NUL terminator not found within bounded observation",
      };
    }
    const names = new Set();
    let duplicateCaseInsensitiveNameCount = 0;
    for (const name of foldedNames) {
      if (names.has(name)) {
        duplicateCaseInsensitiveNameCount += 1;
      }
      names.add(name);
    }
    const range = Process.findRangeByAddress(pointer);
    return {
      pointer: pointer.toString(),
      encoding: "utf16-environment-block",
      maximumBytes,
      byteLengthWithDoubleNul,
      entryCount: entryUtf16UnitLengths.length,
      entryUtf16UnitLengths,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      pseudoDriveEntryCount,
      entriesMissingAssignmentSeparator,
      duplicateCaseInsensitiveNameCount,
      caseFoldedNameOrderNondecreasing: foldedNames.every(
        (name, index) => index === 0 || foldedNames[index - 1] <= name,
      ),
      processParametersEnvironmentPointer:
        processEnvironmentPointer?.environment ?? null,
      equalsProcessParametersEnvironmentPointer:
        processEnvironmentPointer?.environment === pointer.toString(),
      range:
        range === null
          ? null
          : {
              base: range.base.toString(),
              size: range.size,
              protection: range.protection,
              pointerOffset: pointer.sub(range.base).toUInt32(),
            },
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      encoding: "utf16-environment-block",
      maximumBytes,
      error: error.message,
    };
  }
}

function emit(kind, fields = {}) {
  send({
    schema,
    sequence: sequence++,
    timestampMs: Date.now(),
    threadId: Process.getCurrentThreadId(),
    kind,
    ...fields,
  });
}

function readString(pointer, encoding) {
  if (pointer.isNull()) {
    return null;
  }
  try {
    return encoding === "utf16"
      ? pointer.readUtf16String()
      : pointer.readAnsiString();
  } catch (error) {
    return `<unreadable:${error.message}>`;
  }
}

function decodeArgument(argument, decoder) {
  if (decoder === "ansi") {
    return readString(argument, "ansi");
  }
  if (decoder === "utf16") {
    return readString(argument, "utf16");
  }
  if (decoder === "proc") {
    const value = argument.toUInt32();
    return value <= 0xffff ? `#${value}` : readString(argument, "ansi");
  }
  return argument.toString();
}

function readOptionalUtf16Address(address) {
  if (address === 0) {
    return null;
  }
  try {
    return ptr(address).readUtf16String();
  } catch (error) {
    return `<unreadable:${error.message}>`;
  }
}

function snapshotStartupInfoW(pointer, decodeStrings = false) {
  const size = 68;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 STARTUPINFOW observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    const bytes = [...new Uint8Array(buffer)];
    const view = new DataView(buffer);
    const fields = {
      cb: view.getUint32(0, true),
      lpReserved: view.getUint32(4, true),
      lpDesktop: view.getUint32(8, true),
      lpTitle: view.getUint32(12, true),
      dwX: view.getUint32(16, true),
      dwY: view.getUint32(20, true),
      dwXSize: view.getUint32(24, true),
      dwYSize: view.getUint32(28, true),
      dwXCountChars: view.getUint32(32, true),
      dwYCountChars: view.getUint32(36, true),
      dwFillAttribute: view.getUint32(40, true),
      dwFlags: view.getUint32(44, true),
      wShowWindow: view.getUint16(48, true),
      cbReserved2: view.getUint16(50, true),
      lpReserved2: view.getUint32(52, true),
      hStdInput: view.getUint32(56, true),
      hStdOutput: view.getUint32(60, true),
      hStdError: view.getUint32(64, true),
    };
    return {
      pointer: pointer.toString(),
      size,
      bytes,
      fields,
      ...(decodeStrings
        ? {
            strings: {
              reserved: readOptionalUtf16Address(fields.lpReserved),
              desktop: readOptionalUtf16Address(fields.lpDesktop),
              title: readOptionalUtf16Address(fields.lpTitle),
            },
          }
        : {}),
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotCriticalSection(pointer) {
  const size = 24;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 CRITICAL_SECTION observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    const bytes = [...new Uint8Array(buffer)];
    const view = new DataView(buffer);
    return {
      pointer: pointer.toString(),
      size,
      bytes,
      fields: {
        debugInfo: view.getUint32(0, true),
        lockCount: view.getInt32(4, true),
        recursionCount: view.getInt32(8, true),
        owningThread: view.getUint32(12, true),
        lockSemaphore: view.getUint32(16, true),
        spinCount: view.getUint32(20, true),
      },
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotLong(pointer) {
  const size = 4;
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    return {
      pointer: pointer.toString(),
      size,
      bytes: [...new Uint8Array(buffer)],
      value: new DataView(buffer).getInt32(0, true),
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function describeCaller(address) {
  const module = Process.findModuleByAddress(address);
  return {
    address: address.toString(),
    module: module === null ? null : module.name,
    rva: module === null ? null : address.sub(module.base).toUInt32(),
  };
}

function beginCall(fields, returnAddress) {
  const threadId = Process.getCurrentThreadId();
  const stack = activeCallsByThread.get(threadId) ?? [];
  activeCallsByThread.set(threadId, stack);
  const frame = {
    callId: nextCallId++,
    parentCallId: stack.at(-1)?.callId ?? null,
    caller: describeCaller(returnAddress),
    startedAtMs: Date.now(),
    threadId,
  };
  stack.push(frame);
  emit("api-enter", {
    threadId,
    callId: frame.callId,
    parentCallId: frame.parentCallId,
    caller: frame.caller,
    ...fields,
  });
  return frame;
}

function finishCall(frame, fields, lastError) {
  const finishedAtMs = Date.now();
  const stack = activeCallsByThread.get(frame.threadId) ?? [];
  if (stack.at(-1) === frame) {
    stack.pop();
  } else {
    emit("call-stack-error", {
      threadId: frame.threadId,
      callId: frame.callId,
      expectedCallId: stack.at(-1)?.callId ?? null,
    });
    const index = stack.lastIndexOf(frame);
    if (index !== -1) {
      stack.splice(index, 1);
    }
  }
  if (stack.length === 0) {
    activeCallsByThread.delete(frame.threadId);
  }
  emit("api-leave", {
    threadId: frame.threadId,
    callId: frame.callId,
    parentCallId: frame.parentCallId,
    caller: frame.caller,
    lastError: Number(lastError) >>> 0,
    durationMs: Math.max(0, finishedAtMs - frame.startedAtMs),
    ...fields,
  });
}

const hooks = {
  "kernel32.dll": [
    ["LoadLibraryA", ["ansi"]],
    ["LoadLibraryW", ["utf16"]],
    ["GetProcAddress", [null, "proc"]],
    ["CreateFileA", ["ansi"]],
    ["CreateFileW", ["utf16"]],
    ["CreateThread", []],
  ],
  "user32.dll": [
    ["CreateWindowExA", [null, "ansi", "ansi"]],
    ["CreateWindowExW", [null, "utf16", "utf16"]],
  ],
  "gdi32.dll": [
    ["BitBlt", []],
    ["StretchDIBits", []],
  ],
  "gdiplus.dll": [["GdiplusStartup", []]],
  "d3d9.dll": [["Direct3DCreate9", [null]]],
  "dinput8.dll": [["DirectInput8Create", []]],
  "dsound.dll": [
    ["DirectSoundCreate", []],
    ["DirectSoundCreate8", []],
  ],
  "winmm.dll": [
    ["mciSendCommandA", [null, null, null, null]],
    ["mciSendCommandW", [null, null, null, null]],
    ["waveOutOpen", []],
  ],
  "ole32.dll": [
    ["CoInitialize", [null]],
    ["CoInitializeEx", [null, null]],
    ["CoCreateInstance", []],
  ],
};

if (selectedObservations.has("startup-info")) {
  hooks["kernel32.dll"].push(["GetStartupInfoW", [null]]);
}
if (selectedObservations.has("heap-set-information")) {
  hooks["kernel32.dll"].push([
    "HeapSetInformation",
    [null, null, null, null],
  ]);
}
if (selectedObservations.has("heap-lifecycle")) {
  hooks["kernel32.dll"].push(
    ["HeapCreate", [null, null, null]],
    ["HeapAlloc", [null, null, null]],
    ["HeapReAlloc", [null, null, null, null]],
    ["HeapFree", [null, null, null]],
    ["HeapSize", [null, null, null]],
    ["HeapDestroy", [null]],
  );
}
if (selectedObservations.has("heap-allocation")) {
  hooks["kernel32.dll"].push(
    ["HeapAlloc", [null, null, null]],
    ["HeapFree", [null, null, null]],
  );
}
if (selectedObservations.has("heap-size")) {
  hooks["kernel32.dll"].push(
    ["HeapAlloc", [null, null, null]],
    ["HeapFree", [null, null, null]],
    ["HeapSize", [null, null, null]],
  );
}
if (selectedObservations.has("module-handle")) {
  hooks["kernel32.dll"].push(
    ["GetModuleHandleA", ["ansi"]],
    ["GetModuleHandleW", ["utf16"]],
  );
}
if (selectedObservations.has("tls-lifecycle")) {
  hooks["kernel32.dll"].push(
    ["TlsAlloc", []],
    ["TlsFree", [null]],
    ["TlsGetValue", [null]],
    ["TlsSetValue", [null, null]],
  );
}
if (selectedObservations.has("fls-lifecycle")) {
  hooks["kernel32.dll"].push(
    ["FlsAlloc", [null]],
    ["FlsFree", [null]],
    ["FlsGetValue", [null]],
    ["FlsSetValue", [null, null]],
  );
}
if (selectedObservations.has("pointer-encoding")) {
  hooks["kernel32.dll"].push(
    ["EncodePointer", [null]],
    ["DecodePointer", [null]],
  );
}
if (selectedObservations.has("critical-section")) {
  hooks["kernel32.dll"].push(
    ["InitializeCriticalSection", [null]],
    ["InitializeCriticalSectionAndSpinCount", [null, null]],
    ["InitializeCriticalSectionEx", [null, null, null]],
    ["EnterCriticalSection", [null]],
    ["TryEnterCriticalSection", [null]],
    ["LeaveCriticalSection", [null]],
    ["SetCriticalSectionSpinCount", [null, null]],
    ["DeleteCriticalSection", [null]],
  );
}
if (selectedObservations.has("interlocked-increment")) {
  hooks["kernel32.dll"].push(["InterlockedIncrement", [null]]);
}
if (selectedObservations.has("interlocked-decrement")) {
  hooks["kernel32.dll"].push(["InterlockedDecrement", [null]]);
}
if (selectedObservations.has("get-module-file-name-a")) {
  hooks["kernel32.dll"].push(["GetModuleFileNameA", [null, null, null]]);
}
if (selectedObservations.has("standard-handle")) {
  hooks["kernel32.dll"].push(["GetStdHandle", [null]]);
}
if (selectedObservations.has("file-type")) {
  hooks["kernel32.dll"].push(["GetFileType", [null]]);
}
if (selectedObservations.has("set-handle-count")) {
  hooks["kernel32.dll"].push(["SetHandleCount", [null]]);
}
if (selectedObservations.has("command-line-a")) {
  hooks["kernel32.dll"].push(
    ["GetCommandLineA", []],
    ["GetCommandLineW", []],
  );
}
if (selectedObservations.has("environment-strings-w")) {
  hooks["kernel32.dll"].push(
    ["GetEnvironmentStringsW", []],
    ["FreeEnvironmentStringsW", [null]],
  );
}
if (selectedObservations.has("wide-char-to-multi-byte")) {
  hooks["kernel32.dll"].push([
    "WideCharToMultiByte",
    [null, null, null, null, null, null, null, null],
  ]);
}
if (selectedObservations.has("multi-byte-to-wide-char")) {
  hooks["kernel32.dll"].push([
    "MultiByteToWideChar",
    [null, null, null, null, null, null],
  ]);
}
if (selectedObservations.has("set-current-directory-a")) {
  hooks["kernel32.dll"].push(["SetCurrentDirectoryA", ["ansi"]]);
}
if (selectedObservations.has("create-mutex-a")) {
  hooks["kernel32.dll"].push(
    ["CreateMutexA", [null, null, "ansi"]],
    ["WaitForSingleObject", [null, null]],
    ["ReleaseMutex", [null]],
    ["CloseHandle", [null]],
  );
}
if (selectedObservations.has("get-string-type-w")) {
  hooks["kernel32.dll"].push([
    "GetStringTypeW",
    [null, null, null, null],
  ]);
}
if (selectedObservations.has("lc-map-string-w")) {
  hooks["kernel32.dll"].push([
    "LCMapStringW",
    [null, null, null, null, null, null],
  ]);
}
if (selectedObservations.has("get-last-error")) {
  hooks["kernel32.dll"].push(["GetLastError", []]);
}
if (selectedObservations.has("set-last-error")) {
  hooks["kernel32.dll"].push(["SetLastError", [null]]);
}
if (selectedObservations.has("get-acp")) {
  hooks["kernel32.dll"].push(["GetACP", []]);
}
if (selectedObservations.has("system-default-lang-id")) {
  hooks["kernel32.dll"].push(["GetSystemDefaultLangID", []]);
}
if (selectedObservations.has("drive-type")) {
  hooks["kernel32.dll"].push(["GetDriveTypeA", ["ansi"]]);
}
if (selectedObservations.has("get-cp-info")) {
  hooks["kernel32.dll"].push(["GetCPInfo", [null, null]]);
}
if (selectedObservations.has("processor-feature")) {
  hooks["kernel32.dll"].push(["IsProcessorFeaturePresent", [null]]);
}
if (selectedObservations.has("unhandled-exception-filter")) {
  hooks["kernel32.dll"].push(["SetUnhandledExceptionFilter", [null]]);
}
if (selectedObservations.has("set-thread-affinity-mask")) {
  hooks["kernel32.dll"].push(["SetThreadAffinityMask", [null, null]]);
}
if (selectedObservations.has("get-system-info")) {
  hooks["kernel32.dll"].push(["GetSystemInfo", [null]]);
}
if (selectedObservations.has("enum-display-monitors")) {
  hooks["user32.dll"].push([
    "EnumDisplayMonitors",
    [null, null, null, null],
  ]);
}
if (selectedObservations.has("swap-mouse-button")) {
  hooks["user32.dll"].push(["SwapMouseButton", [null]]);
}
if (selectedObservations.has("time-get-dev-caps")) {
  hooks["winmm.dll"].push(
    ["timeGetDevCaps", [null, null]],
    ["timeBeginPeriod", [null]],
    ["timeEndPeriod", [null]],
  );
}
if (selectedObservations.has("com-initialization")) {
  hooks["ole32.dll"].push(["CoUninitialize", []]);
}
const idirect3d9Methods = [
  [0, "QueryInterface", 2],
  [1, "AddRef", 0],
  [2, "Release", 0],
  [3, "RegisterSoftwareDevice", 1],
  [4, "GetAdapterCount", 0],
  [5, "GetAdapterIdentifier", 3],
  [6, "GetAdapterModeCount", 2],
  [7, "EnumAdapterModes", 4],
  [8, "GetAdapterDisplayMode", 2],
  [9, "CheckDeviceType", 5],
  [10, "CheckDeviceFormat", 6],
  [11, "CheckDeviceMultiSampleType", 6],
  [12, "CheckDepthStencilMatch", 5],
  [13, "CheckDeviceFormatConversion", 4],
  [14, "GetDeviceCaps", 3],
  [15, "GetAdapterMonitor", 1],
  [16, "CreateDevice", 6],
];

function findExport(module, name) {
  if (typeof module.findExportByName === "function") {
    return module.findExportByName(name);
  }
  if (typeof Module.findExportByName === "function") {
    return Module.findExportByName(module.name, name);
  }
  return null;
}

function attachGetCurrentThreadCallSites(module) {
  const target = findExport(module, "GetCurrentThread");
  if (target === null) {
    throw new Error("GetCurrentThread export is unavailable");
  }
  const mainModule = Process.mainModule;
  const callSites = [];
  for (const range of mainModule.enumerateRanges("r-x")) {
    for (const match of Memory.scanSync(range.base, range.size, "ff 15")) {
      try {
        const slot = ptr(match.address.add(2).readU32());
        const resolved = slot.readPointer();
        if (resolved.equals(target)) {
          callSites.push({ address: match.address, slot });
        }
      } catch (_) {
        // Ignore unreadable false-positive instruction patterns.
      }
    }
  }
  if (callSites.length === 0) {
    throw new Error("BGI GetCurrentThread IAT call site was not found");
  }
  emit("get-current-thread-callsite-observed", {
    module: mainModule.name,
    target: target.toString(),
    callSites: callSites.map(({ address, slot }) => ({
      callSite: describeCaller(address),
      returnSite: describeCaller(address.add(6)),
      iatAddress: slot.toString(),
      callInstruction: snapshotInstructions(address, 1),
      immediateConsumer: snapshotInstructionsFollowingDirectJump(address.add(6), 16),
    })),
  });
}

function attachThreadAffinityTickCountObserver(module) {
  const address = findExport(module, "GetTickCount");
  if (address === null) {
    throw new Error("GetTickCount export is unavailable");
  }
  const key = `thread-affinity-tick-count:${address}`;
  if (attached.has(key)) {
    return;
  }
  attached.add(key);
  Interceptor.attach(address, {
    onEnter() {
      const caller = describeCaller(this.returnAddress);
      this.traceThreadAffinityTick =
        setThreadAffinityCalibration?.active === true &&
        caller.module !== null &&
        caller.module.toLowerCase() === "bgi.exe";
      this.traceThreadAffinityTickCaller = caller;
      this.traceThreadAffinityTickReturnAddress = this.returnAddress;
    },
    onLeave(result) {
      if (!this.traceThreadAffinityTick) {
        return;
      }
      const calibration = setThreadAffinityCalibration;
      if (calibration === null || !calibration.active) {
        return;
      }
      const value = result.toUInt32();
      const sample = {
        index: calibration.tickCountCalls + 1,
        value,
        lastError: Number(this.lastError) >>> 0,
      };
      if (calibration.tickCountCalls === 0) {
        sample.caller = this.traceThreadAffinityTickCaller;
        sample.immediateConsumer = snapshotInstructionsFollowingDirectJump(
          this.traceThreadAffinityTickReturnAddress,
          16,
        );
        calibration.firstValue = value;
      } else {
        const delta = (value - calibration.lastValue) >>> 0;
        if (delta !== 0) {
          calibration.valueChanges += 1;
        }
        if (delta > 0x7fff_ffff) {
          calibration.regressions += 1;
        }
      }
      calibration.tickCountCalls += 1;
      calibration.lastValue = value;
      if (calibration.firstSamples.length < 8) {
        calibration.firstSamples.push(sample);
      }
      calibration.lastSamples.push(sample);
      if (calibration.lastSamples.length > 8) {
        calibration.lastSamples.shift();
      }
    },
  });
}

function attachIdirect3d9(object) {
  if (object.isNull()) {
    return;
  }
  try {
    const vtable = object.readPointer();
    const vtableKey = vtable.toString();
    if (observedVtables.has(vtableKey)) {
      return;
    }
    observedVtables.add(vtableKey);
    const slots = [];
    for (const [slot, name, argumentCount] of idirect3d9Methods) {
      const address = vtable.add(slot * Process.pointerSize).readPointer();
      slots.push({ slot, name, address: address.toString() });
      const key = `IDirect3D9:${slot}:${address}`;
      if (attachedVtableMethods.has(key)) {
        continue;
      }
      attachedVtableMethods.add(key);
      const implementationModule = Process.findModuleByAddress(address);
      Interceptor.attach(address, {
        onEnter(args) {
          const objectAddress = args[0].toString();
          const arguments_ = Array.from({ length: argumentCount }, (_, index) =>
            args[index + 1].toString(),
          );
          this.traceObject = objectAddress;
          this.traceFrame = beginCall(
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3D9",
              vtableSlot: slot,
              object: objectAddress,
              arguments: arguments_,
            },
            this.returnAddress,
          );
        },
        onLeave(result) {
          const lastError = this.lastError;
          finishCall(
            this.traceFrame,
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3D9",
              vtableSlot: slot,
              object: this.traceObject,
              result: result.toString(),
            },
            lastError,
          );
        },
      });
    }
    emit("vtable-observed", {
      interface: "IDirect3D9",
      object: object.toString(),
      vtable: vtable.toString(),
      slots,
    });
  } catch (error) {
    emit("vtable-error", {
      interface: "IDirect3D9",
      object: object.toString(),
      error: error.message,
    });
  }
}

function attachGetVersionExAHook(module) {
  const name = "GetVersionExA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" && getVersionExAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.output = args[0];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = getVersionExAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.outputBefore = snapshotOsVersionInfoA(this.output);
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              64,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              4,
              16,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.output.toString()],
          lastErrorBefore: this.lastErrorBefore,
          getVersionExA: {
            outputBefore: this.outputBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const rawResult = result.toUInt32();
      const outputAfter = snapshotOsVersionInfoA(this.output);
      if (this.oracleCaller) {
        getVersionExAOracleCall.hookResult = rawResult;
        getVersionExAOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          getVersionExA: {
            result: rawResult,
            outputAfter,
          },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && getVersionExAMainModuleCalls.length === 0) {
        getVersionExAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          outputAddress: this.output.toString(),
          lastErrorBefore: this.lastErrorBefore,
          outputBefore: this.outputBefore,
          outputAfter,
          result: rawResult,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        setImmediate(maybeCompleteGetVersionExAObservation);
      }
    },
  });
}

function attachCreateWaitableTimerAHook(module) {
  const name = "CreateWaitableTimerA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        createWaitableTimerAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.securityAttributes = args[0];
      this.manualReset = args[1].toUInt32();
      this.nameAddress = args[2];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = createWaitableTimerAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              12,
              16,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [
            this.securityAttributes.toString(),
            this.manualReset,
            this.nameAddress.isNull()
              ? null
              : snapshotNulTerminatedString(this.nameAddress, "ansi"),
          ],
          lastErrorBefore: this.lastErrorBefore,
          createWaitableTimerA: {
            securityAttributes: this.securityAttributes.toString(),
            manualReset: this.manualReset,
            nameAddress: this.nameAddress.toString(),
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      if (this.oracleCaller) {
        createWaitableTimerAOracleCall.hookResult = resultValue;
        createWaitableTimerAOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          createWaitableTimerA: {
            result: resultValue,
            resultNonNull: !result.isNull(),
          },
        },
        this.lastError,
      );
      if (
        this.mainModuleCaller &&
        createWaitableTimerAMainModuleCalls.length === 0
      ) {
        createWaitableTimerAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          securityAttributes: this.securityAttributes.toString(),
          manualReset: this.manualReset,
          nameAddress: this.nameAddress.toString(),
          result: resultValue,
          resultNonNull: !result.isNull(),
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        setImmediate(maybeCompleteCreateWaitableTimerAObservation);
      }
    },
  });
}

function snapshotResourceNameA(address) {
  const value = address.toUInt32();
  if (value <= 0xffff) {
    return {
      address: address.toString(),
      kind: "integer",
      id: value,
    };
  }
  return {
    address: address.toString(),
    kind: "string",
    value: snapshotNulTerminatedString(address, "ansi"),
  };
}

function attachLoadIconAHook(module) {
  const name = "LoadIconA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" && loadIconAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.instance = args[0];
      this.resourceName = args[1];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = loadIconAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.resource = snapshotResourceNameA(this.resourceName);
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              48,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              6,
              20,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.instance.toString(), this.resource],
          lastErrorBefore: this.lastErrorBefore,
          loadIconA: {
            instance: this.instance.toString(),
            instanceIsMainModule:
              this.instance.toString() === Process.mainModule.base.toString(),
            resource: this.resource,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      if (this.oracleCaller) {
        loadIconAOracleCall.hookResult = resultValue;
        loadIconAOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          loadIconA: {
            result: resultValue,
            resultNonNull: !result.isNull(),
          },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && !loadIconACompletionRecorded) {
        loadIconAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          instance: this.instance.toString(),
          instanceIsMainModule:
            this.instance.toString() === Process.mainModule.base.toString(),
          resource: this.resource,
          result: resultValue,
          resultNonNull: !result.isNull(),
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleLoadIconAOracle(module);
        setImmediate(maybeCompleteLoadIconAObservation);
      }
    },
  });
}

function attachLoadCursorAHook(module) {
  const name = "LoadCursorA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" && loadCursorAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.instance = args[0];
      this.resourceName = args[1];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = loadCursorAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.resource = snapshotResourceNameA(this.resourceName);
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              48,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              6,
              20,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.instance.toString(), this.resource],
          lastErrorBefore: this.lastErrorBefore,
          loadCursorA: {
            instance: this.instance.toString(),
            instanceIsNull: this.instance.isNull(),
            resource: this.resource,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      if (this.oracleCaller) {
        loadCursorAOracleCall.hookResult = resultValue;
        loadCursorAOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          loadCursorA: {
            result: resultValue,
            resultNonNull: !result.isNull(),
          },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && !loadCursorACompletionRecorded) {
        loadCursorAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          instance: this.instance.toString(),
          instanceIsNull: this.instance.isNull(),
          resource: this.resource,
          result: resultValue,
          resultNonNull: !result.isNull(),
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleLoadCursorAOracle(module);
        setImmediate(maybeCompleteLoadCursorAObservation);
      }
    },
  });
}

function attachHook(module, name, decoders) {
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const isFocusedHeapApi = name === "HeapAlloc" || name === "HeapFree";
      const isMainModuleCaller =
        caller.module !== null &&
        caller.module.toLowerCase() === Process.mainModule.name.toLowerCase();
      const recordsInitializeCriticalSectionConsumer =
        selectedObservations.has("critical-section") &&
        name === "InitializeCriticalSection" &&
        isMainModuleCaller &&
        initializeCriticalSectionConsumersRecorded < 2;
      if (recordsInitializeCriticalSectionConsumer) {
        initializeCriticalSectionConsumersRecorded += 1;
      }
      const focusedHeapObservation =
        selectedObservations.has("heap-allocation") &&
        isFocusedHeapApi &&
        isMainModuleCaller;
      const heapSizeAllocationTracking =
        selectedObservations.has("heap-size") &&
        !selectedObservations.has("heap-allocation") &&
        isFocusedHeapApi &&
        isMainModuleCaller;
      const focusedGetCpInfoObservation =
        selectedObservations.has("get-cp-info") &&
        name === "GetCPInfo" &&
        caller.module !== null &&
        (isMainModuleCaller || caller.module.toLowerCase() === "frida-agent.dll");
      const focusedProcessorFeatureObservation =
        selectedObservations.has("processor-feature") &&
        name === "IsProcessorFeaturePresent" &&
        caller.module !== null &&
        (isMainModuleCaller || caller.module.toLowerCase() === "frida-agent.dll");
      const focusedSetThreadAffinityMaskObservation =
        selectedObservations.has("set-thread-affinity-mask") &&
        name === "SetThreadAffinityMask" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            setThreadAffinityMaskOracleSentinel !== null));
      const focusedGetSystemInfoObservation =
        selectedObservations.has("get-system-info") &&
        name === "GetSystemInfo" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            getSystemInfoOracleSentinel !== null));
      const focusedEnumDisplayMonitorsObservation =
        selectedObservations.has("enum-display-monitors") &&
        name === "EnumDisplayMonitors" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            enumDisplayMonitorsOracleSentinel !== null));
      const focusedSwapMouseButtonObservation =
        selectedObservations.has("swap-mouse-button") &&
        name === "SwapMouseButton" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            swapMouseButtonRestoreActive));
      const focusedTimeGetDevCapsObservation =
        selectedObservations.has("time-get-dev-caps") &&
        name === "timeGetDevCaps" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            timeGetDevCapsOracleCall !== null));
      const focusedTimePeriodObservation =
        selectedObservations.has("time-get-dev-caps") &&
        (name === "timeBeginPeriod" || name === "timeEndPeriod") &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            timePeriodOracleCall !== null));
      const focusedUnhandledExceptionFilterObservation =
        selectedObservations.has("unhandled-exception-filter") &&
        name === "SetUnhandledExceptionFilter" &&
        caller.module !== null &&
        (isMainModuleCaller || caller.module.toLowerCase() === "frida-agent.dll");
      const focusedHeapSizeObservation =
        selectedObservations.has("heap-size") &&
        name === "HeapSize" &&
        caller.module !== null &&
        (isMainModuleCaller || caller.module.toLowerCase() === "frida-agent.dll");
      const focusedMultiByteObservation =
        selectedObservations.has("multi-byte-to-wide-char") &&
        name === "MultiByteToWideChar" &&
        caller.module !== null &&
        (isMainModuleCaller || caller.module.toLowerCase() === "frida-agent.dll");
      const focusedSetCurrentDirectoryAObservation =
        selectedObservations.has("set-current-directory-a") &&
        name === "SetCurrentDirectoryA" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            setCurrentDirectoryOracleSentinel !== null));
      const isCreateMutexA = name === "CreateMutexA";
      const isMutexLifecycleApi =
        name === "WaitForSingleObject" ||
        name === "ReleaseMutex" ||
        name === "CloseHandle";
      const mutexHandleKey = isMutexLifecycleApi
        ? args[0].toString().toLowerCase()
        : null;
      const focusedCreateMutexAObservation =
        selectedObservations.has("create-mutex-a") &&
        isCreateMutexA &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            createMutexOracleSentinel !== null));
      const focusedMutexLifecycleObservation =
        selectedObservations.has("create-mutex-a") &&
        isMutexLifecycleApi &&
        mutexHandleKey !== null &&
        trackedMutexHandles.has(mutexHandleKey);
      const focusedGetStringTypeObservation =
        selectedObservations.has("get-string-type-w") &&
        name === "GetStringTypeW" &&
        caller.module !== null &&
        (isMainModuleCaller || caller.module.toLowerCase() === "frida-agent.dll");
      const focusedLCMapStringObservation =
        selectedObservations.has("lc-map-string-w") &&
        name === "LCMapStringW" &&
        caller.module !== null &&
        (isMainModuleCaller || caller.module.toLowerCase() === "frida-agent.dll");
      const focusedInterlockedDecrementObservation =
        selectedObservations.has("interlocked-decrement") &&
        name === "InterlockedDecrement" &&
        caller.module !== null &&
        (isMainModuleCaller || caller.module.toLowerCase() === "frida-agent.dll");
      const focusedGetModuleFileNameAObservation =
        selectedObservations.has("get-module-file-name-a") &&
        name === "GetModuleFileNameA" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            getModuleFileNameAOracleSentinel !== null));
      if (heapSizeAllocationTracking) {
        this.traceFrame = null;
        this.trackHeapSizeAllocation =
          name === "HeapAlloc"
            ? {
                requestedBytes: args[2].toUInt32(),
              }
            : null;
        this.trackHeapSizeFree =
          name === "HeapFree" ? args[2].toString().toLowerCase() : null;
        return;
      }
      if (
        selectedObservations.has("heap-size") &&
        !selectedObservations.has("heap-allocation") &&
        !selectedObservations.has("heap-lifecycle") &&
        isFocusedHeapApi
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("heap-allocation") &&
        !selectedObservations.has("heap-lifecycle") &&
        isFocusedHeapApi &&
        !focusedHeapObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("get-cp-info") &&
        name === "GetCPInfo" &&
        !focusedGetCpInfoObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("processor-feature") &&
        name === "IsProcessorFeaturePresent" &&
        !focusedProcessorFeatureObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("set-thread-affinity-mask") &&
        name === "SetThreadAffinityMask" &&
        !focusedSetThreadAffinityMaskObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("get-system-info") &&
        name === "GetSystemInfo" &&
        !focusedGetSystemInfoObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("enum-display-monitors") &&
        name === "EnumDisplayMonitors" &&
        !focusedEnumDisplayMonitorsObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("swap-mouse-button") &&
        name === "SwapMouseButton" &&
        !focusedSwapMouseButtonObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("time-get-dev-caps") &&
        name === "timeGetDevCaps" &&
        !focusedTimeGetDevCapsObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("time-get-dev-caps") &&
        (name === "timeBeginPeriod" || name === "timeEndPeriod") &&
        !focusedTimePeriodObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("unhandled-exception-filter") &&
        name === "SetUnhandledExceptionFilter" &&
        !focusedUnhandledExceptionFilterObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("heap-size") &&
        name === "HeapSize" &&
        !focusedHeapSizeObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("multi-byte-to-wide-char") &&
        name === "MultiByteToWideChar" &&
        !focusedMultiByteObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("set-current-directory-a") &&
        name === "SetCurrentDirectoryA" &&
        !focusedSetCurrentDirectoryAObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("create-mutex-a") &&
        (isCreateMutexA || isMutexLifecycleApi) &&
        !focusedCreateMutexAObservation &&
        !focusedMutexLifecycleObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("get-string-type-w") &&
        name === "GetStringTypeW" &&
        !focusedGetStringTypeObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("lc-map-string-w") &&
        name === "LCMapStringW" &&
        !focusedLCMapStringObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("interlocked-decrement") &&
        name === "InterlockedDecrement" &&
        !focusedInterlockedDecrementObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("get-module-file-name-a") &&
        name === "GetModuleFileNameA" &&
        !focusedGetModuleFileNameAObservation
      ) {
        this.traceFrame = null;
        return;
      }
      this.traceCriticalSectionOracle =
        criticalSectionOracleSentinel !== null &&
        selectedObservations.has("critical-section") &&
        name.includes("CriticalSection") &&
        caller.module === "frida-agent.dll";
      if (this.traceCriticalSectionOracle) {
        this.lastError = criticalSectionOracleSentinel;
      }
      this.traceEnvironmentStringsOracle =
        environmentStringsOracleSentinel !== null &&
        selectedObservations.has("environment-strings-w") &&
        (name === "GetEnvironmentStringsW" ||
          name === "FreeEnvironmentStringsW") &&
        caller.module === "frida-agent.dll";
      if (this.traceEnvironmentStringsOracle) {
        this.lastError = environmentStringsOracleSentinel;
      }
      this.traceWideCharOracle =
        wideCharOracleSentinel !== null &&
        selectedObservations.has("wide-char-to-multi-byte") &&
        name === "WideCharToMultiByte" &&
        caller.module === "frida-agent.dll";
      if (this.traceWideCharOracle) {
        this.lastError = wideCharOracleSentinel;
      }
      this.traceMultiByteOracle =
        multiByteOracleSentinel !== null &&
        selectedObservations.has("multi-byte-to-wide-char") &&
        name === "MultiByteToWideChar" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceMultiByteOracle) {
        this.lastError = multiByteOracleSentinel;
      }
      this.traceSetCurrentDirectoryOracle =
        setCurrentDirectoryOracleSentinel !== null &&
        selectedObservations.has("set-current-directory-a") &&
        name === "SetCurrentDirectoryA" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceSetCurrentDirectoryOracle) {
        this.lastError = setCurrentDirectoryOracleSentinel;
      }
      this.traceCreateMutexOracle =
        createMutexOracleSentinel !== null &&
        selectedObservations.has("create-mutex-a") &&
        (isCreateMutexA || isMutexLifecycleApi) &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceCreateMutexOracle) {
        this.lastError = createMutexOracleSentinel;
      }
      this.traceGetStringTypeOracle =
        getStringTypeOracleSentinel !== null &&
        selectedObservations.has("get-string-type-w") &&
        name === "GetStringTypeW" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceGetStringTypeOracle) {
        this.lastError = getStringTypeOracleSentinel;
      }
      this.traceLCMapStringOracle =
        lcMapStringOracleSentinel !== null &&
        selectedObservations.has("lc-map-string-w") &&
        name === "LCMapStringW" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceLCMapStringOracle) {
        this.lastError = lcMapStringOracleSentinel;
      }
      this.traceInterlockedDecrementOracle =
        interlockedDecrementOracleSentinel !== null &&
        selectedObservations.has("interlocked-decrement") &&
        name === "InterlockedDecrement" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceInterlockedDecrementOracle) {
        this.lastError = interlockedDecrementOracleSentinel;
      }
      this.traceGetModuleFileNameAOracle =
        getModuleFileNameAOracleSentinel !== null &&
        selectedObservations.has("get-module-file-name-a") &&
        name === "GetModuleFileNameA" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceGetModuleFileNameAOracle) {
        this.lastError = getModuleFileNameAOracleSentinel;
      }
      this.traceGetLastErrorOracle =
        getLastErrorOracleSentinel !== null &&
        selectedObservations.has("get-last-error") &&
        name === "GetLastError" &&
        caller.module === "frida-agent.dll";
      if (this.traceGetLastErrorOracle) {
        this.lastError = getLastErrorOracleSentinel;
      }
      this.traceSetLastErrorOracle =
        setLastErrorOracleActive &&
        selectedObservations.has("set-last-error") &&
        name === "SetLastError" &&
        caller.module === "frida-agent.dll";
      if (this.traceSetLastErrorOracle) {
        setLastErrorOracleArgument = args[0].toUInt32();
        setLastErrorOracleEaxBefore = this.context.eax.toUInt32();
      }
      this.traceGetAcpOracle =
        getAcpOracleSentinel !== null &&
        selectedObservations.has("get-acp") &&
        name === "GetACP" &&
        caller.module === "frida-agent.dll";
      if (this.traceGetAcpOracle) {
        this.lastError = getAcpOracleSentinel;
      }
      this.traceSystemDefaultLangIdOracle =
        systemDefaultLangIdOracleSentinel !== null &&
        selectedObservations.has("system-default-lang-id") &&
        name === "GetSystemDefaultLangID" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceSystemDefaultLangIdOracle) {
        this.lastError = systemDefaultLangIdOracleSentinel;
      }
      this.traceDriveTypeOracle =
        driveTypeOracleSentinel !== null &&
        selectedObservations.has("drive-type") &&
        name === "GetDriveTypeA" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceDriveTypeOracle) {
        this.lastError = driveTypeOracleSentinel;
      }
      this.traceGetCpInfoOracle =
        getCpInfoOracleSentinel !== null &&
        selectedObservations.has("get-cp-info") &&
        name === "GetCPInfo" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceGetCpInfoOracle) {
        this.lastError = getCpInfoOracleSentinel;
      }
      this.traceProcessorFeatureOracle =
        processorFeatureOracleSentinel !== null &&
        selectedObservations.has("processor-feature") &&
        name === "IsProcessorFeaturePresent" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceProcessorFeatureOracle) {
        this.lastError = processorFeatureOracleSentinel;
      }
      this.traceSetThreadAffinityMaskOracle =
        setThreadAffinityMaskOracleSentinel !== null &&
        selectedObservations.has("set-thread-affinity-mask") &&
        name === "SetThreadAffinityMask" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceSetThreadAffinityMaskOracle) {
        this.lastError = setThreadAffinityMaskOracleSentinel;
      }
      this.traceGetSystemInfoOracle =
        getSystemInfoOracleSentinel !== null &&
        selectedObservations.has("get-system-info") &&
        name === "GetSystemInfo" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceGetSystemInfoOracle) {
        this.lastError = getSystemInfoOracleSentinel;
      }
      this.traceEnumDisplayMonitorsOracle =
        enumDisplayMonitorsOracleSentinel !== null &&
        selectedObservations.has("enum-display-monitors") &&
        name === "EnumDisplayMonitors" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceEnumDisplayMonitorsOracle) {
        this.lastError = enumDisplayMonitorsOracleSentinel;
      }
      this.traceTimeGetDevCapsOracle =
        timeGetDevCapsOracleCall !== null &&
        selectedObservations.has("time-get-dev-caps") &&
        name === "timeGetDevCaps" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceTimeGetDevCapsOracle) {
        this.lastError = timeGetDevCapsOracleCall.sentinel;
      }
      this.traceTimePeriodOracle =
        timePeriodOracleCall !== null &&
        selectedObservations.has("time-get-dev-caps") &&
        name === timePeriodOracleCall.api &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceTimePeriodOracle) {
        this.lastError = timePeriodOracleCall.sentinel;
      }
      this.traceComInitializationOracle =
        comInitializationOracleCall !== null &&
        selectedObservations.has("com-initialization") &&
        name === comInitializationOracleCall.api &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceComInitializationOracle) {
        this.lastError = comInitializationOracleCall.sentinel;
      }
      this.traceUnhandledExceptionFilterOracle =
        unhandledExceptionFilterOracleSentinel !== null &&
        selectedObservations.has("unhandled-exception-filter") &&
        name === "SetUnhandledExceptionFilter" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceUnhandledExceptionFilterOracle) {
        this.lastError = unhandledExceptionFilterOracleSentinel;
      }
      this.traceHeapSizeOracle =
        heapSizeOracleSentinel !== null &&
        selectedObservations.has("heap-size") &&
        name === "HeapSize" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceHeapSizeOracle) {
        this.lastError = heapSizeOracleSentinel;
      }
      const arguments_ = decoders.map((decoder, index) =>
        decoder === null ? args[index].toString() : decodeArgument(args[index], decoder),
      );
      this.traceThreadAffinityPrimaryCall =
        focusedSetThreadAffinityMaskObservation && isMainModuleCaller
          ? ++setThreadAffinityMaskPrimaryCalls
          : null;
      this.traceThreadAffinityRequestedMask =
        this.traceThreadAffinityPrimaryCall === null ? null : args[1].toUInt32();
      this.traceHeapAllocation =
        focusedHeapObservation && name === "HeapAlloc"
          ? {
              heap: args[0].toString(),
              flags: args[1].toUInt32(),
              requestedBytes: args[2].toUInt32(),
              immediateConsumer: {
                apiContinuation: snapshotInstructionsFollowingDirectJump(
                  this.returnAddress,
                  16,
                ),
                frameCallerContinuations: snapshotX86FrameCallerContinuations(
                  this.context,
                  4,
                  12,
                ),
              },
            }
          : null;
      const heapFreePointer =
        focusedHeapObservation && name === "HeapFree"
          ? args[2].toString().toLowerCase()
          : null;
      const relatedHeapAllocation =
        heapFreePointer === null
          ? null
          : (observedMainModuleHeapAllocations.get(heapFreePointer) ?? null);
      this.traceHeapFree =
        heapFreePointer === null
          ? null
          : {
              pointer: heapFreePointer,
              relatedAllocationCallId: relatedHeapAllocation?.callId ?? null,
              requestedBytes: relatedHeapAllocation?.requestedBytes ?? null,
              bufferBeforeFree:
                relatedHeapAllocation === null
                  ? null
                  : snapshotByteBuffer(
                      args[2],
                      relatedHeapAllocation.requestedBytes,
                    ),
            };
      const heapSizePointer = focusedHeapSizeObservation
        ? args[2].toString().toLowerCase()
        : null;
      const relatedHeapSizeAllocation =
        heapSizePointer === null
          ? null
          : (observedMainModuleHeapAllocations.get(heapSizePointer) ?? null);
      const recordsHeapSizeConsumer =
        focusedHeapSizeObservation && isMainModuleCaller && heapSizeConsumersRecorded < 2;
      if (recordsHeapSizeConsumer) {
        heapSizeConsumersRecorded += 1;
      }
      this.traceHeapSize =
        heapSizePointer === null
          ? null
          : {
              heap: args[0].toString().toLowerCase(),
              flags: args[1].toUInt32(),
              pointer: heapSizePointer,
              relatedAllocationCallId: relatedHeapSizeAllocation?.callId ?? null,
              requestedBytes: relatedHeapSizeAllocation?.requestedBytes ?? null,
              immediateConsumer: recordsHeapSizeConsumer
                ? {
                    apiContinuation: snapshotInstructionsFollowingDirectJump(
                      this.returnAddress,
                      24,
                    ),
                    frameCallerContinuations: snapshotX86FrameCallerContinuations(
                      this.context,
                      4,
                      12,
                    ),
                  }
                : null,
            };
      this.traceWideCharConversion =
        name === "WideCharToMultiByte" &&
        selectedObservations.has("wide-char-to-multi-byte")
          ? {
              source: args[2],
              sourceUnits: signed32(args[3]),
              destination: args[4],
              destinationBytes: signed32(args[5]),
              usedDefaultChar: args[7],
            }
          : null;
      this.traceMultiByteConversion =
        focusedMultiByteObservation
          ? {
              codePage: args[0],
              flags: args[1],
              source: args[2],
              sourceBytes: signed32(args[3]),
              destination: args[4],
              destinationUnits: signed32(args[5]),
            }
          : null;
      this.traceSetCurrentDirectoryA =
        focusedSetCurrentDirectoryAObservation
          ? {
              path: args[0],
              currentDirectoryBefore: (() => {
                const lastErrorBefore = Number(this.lastError) >>> 0;
                const currentDirectory = snapshotCurrentDirectoryA(module);
                this.lastError = lastErrorBefore;
                return currentDirectory;
              })(),
            }
          : null;
      this.traceCreateMutexA =
        focusedCreateMutexAObservation
          ? {
              securityAttributes: args[0],
              initialOwner: args[1],
              name: args[2],
              nameBefore: snapshotMultiByteSource(args[2], -1),
            }
          : null;
      const recordsMutexLifecycleConsumer =
        focusedMutexLifecycleObservation &&
        isMainModuleCaller &&
        mutexLifecycleConsumersRecorded < 6;
      if (recordsMutexLifecycleConsumer) {
        mutexLifecycleConsumersRecorded += 1;
      }
      this.traceMutexLifecycle =
        focusedMutexLifecycleObservation
          ? {
              operation: name,
              handle: mutexHandleKey,
              trackedObject: trackedMutexHandles.get(mutexHandleKey),
              timeoutMilliseconds:
                name === "WaitForSingleObject" ? args[1].toUInt32() : null,
              immediateConsumer: recordsMutexLifecycleConsumer
                ? snapshotInstructionsFollowingDirectJump(this.returnAddress, 24)
                : null,
            }
          : null;
      this.traceGetStringTypeW =
        focusedGetStringTypeObservation
          ? {
              infoType: args[0],
              source: args[1],
              sourceUnits: signed32(args[2]),
              output: args[3],
            }
          : null;
      this.traceLCMapStringW =
        focusedLCMapStringObservation
          ? {
              locale: args[0],
              flags: args[1],
              source: args[2],
              sourceUnits: signed32(args[3]),
              destination: args[4],
              destinationUnits: signed32(args[5]),
            }
          : null;
      this.traceInterlockedDecrement =
        focusedInterlockedDecrementObservation ? { target: args[0] } : null;
      this.traceGetModuleFileNameA =
        focusedGetModuleFileNameAObservation
          ? {
              moduleHandle: args[0],
              output: args[1],
              capacity: args[2].toUInt32(),
            }
          : null;
      this.traceGetCpInfo =
        focusedGetCpInfoObservation
          ? { codePage: args[0], output: args[1] }
          : null;
      this.traceGetSystemInfo =
        focusedGetSystemInfoObservation ? { output: args[0] } : null;
      const recordsSystemDefaultLangIdConsumer =
        selectedObservations.has("system-default-lang-id") &&
        name === "GetSystemDefaultLangID" &&
        isMainModuleCaller &&
        !systemDefaultLangIdConsumerRecorded;
      if (recordsSystemDefaultLangIdConsumer) {
        systemDefaultLangIdConsumerRecorded = true;
      }
      this.traceSystemDefaultLangId =
        selectedObservations.has("system-default-lang-id") &&
        name === "GetSystemDefaultLangID"
          ? {
              immediateConsumer: recordsSystemDefaultLangIdConsumer
                ? {
                    apiContinuation: snapshotInstructionsFollowingDirectJump(
                      this.returnAddress,
                      24,
                    ),
                    frameCallerContinuations: snapshotX86FrameCallerContinuations(
                      this.context,
                      0,
                      12,
                    ),
                  }
                : null,
            }
          : null;
      const recordsDriveTypeConsumer =
        selectedObservations.has("drive-type") &&
        name === "GetDriveTypeA" &&
        isMainModuleCaller &&
        !driveTypeConsumerRecorded;
      if (recordsDriveTypeConsumer) {
        driveTypeConsumerRecorded = true;
      }
      this.traceDriveType =
        selectedObservations.has("drive-type") && name === "GetDriveTypeA"
          ? {
              rootPathBefore: snapshotNulTerminatedString(args[0], "ansi"),
              immediateConsumer: recordsDriveTypeConsumer
                ? {
                    apiContinuation: snapshotInstructionsFollowingDirectJump(
                      this.returnAddress,
                      32,
                    ),
                    frameCallerContinuations: snapshotX86FrameCallerContinuations(
                      this.context,
                      4,
                      16,
                    ),
                  }
                : null,
            }
          : null;
      this.traceDriveTypeMainModuleCaller =
        this.traceDriveType !== null && isMainModuleCaller;
      const recordsEnumDisplayMonitorsConsumer =
        focusedEnumDisplayMonitorsObservation &&
        isMainModuleCaller &&
        !enumDisplayMonitorsConsumerRecorded;
      if (recordsEnumDisplayMonitorsConsumer) {
        enumDisplayMonitorsConsumerRecorded = true;
      }
      this.traceEnumDisplayMonitors = focusedEnumDisplayMonitorsObservation
        ? {
            hdc: args[0].toString(),
            clipRect: args[1].toString(),
            clipRectBefore: args[1].isNull() ? null : snapshotRect(args[1]),
            callback: args[2].toString(),
            callbackInstructions: args[2].isNull()
              ? null
              : snapshotInstructions(args[2], 24),
            data: args[3].toString(),
            immediateConsumer: recordsEnumDisplayMonitorsConsumer
              ? {
                  apiContinuation: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    24,
                  ),
                  frameCallerContinuations: snapshotX86FrameCallerContinuations(
                    this.context,
                    4,
                    12,
                  ),
                }
              : null,
            callbacks: [],
          }
        : null;
      if (this.traceEnumDisplayMonitors !== null) {
        attachEnumDisplayMonitorCallback(args[2]);
      }
      const recordsSwapMouseButtonConsumer =
        focusedSwapMouseButtonObservation &&
        isMainModuleCaller &&
        !swapMouseButtonConsumerRecorded;
      if (recordsSwapMouseButtonConsumer) {
        swapMouseButtonConsumerRecorded = true;
      }
      this.traceSwapMouseButton = focusedSwapMouseButtonObservation
        ? {
            swap: args[0].toUInt32(),
            stateBefore:
              swapMouseButtonGetSystemMetrics === null
                ? null
                : swapMouseButtonGetSystemMetrics(23) !== 0,
            immediateConsumer: recordsSwapMouseButtonConsumer
              ? {
                  apiContinuation: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    24,
                  ),
                  frameCallerContinuations: snapshotX86FrameCallerContinuations(
                    this.context,
                    4,
                    16,
                  ),
                }
              : null,
          }
        : null;
      this.traceSwapMouseButtonMainModuleCaller =
        this.traceSwapMouseButton !== null && isMainModuleCaller;
      const recordsTimeGetDevCapsConsumer =
        focusedTimeGetDevCapsObservation &&
        isMainModuleCaller &&
        !timeGetDevCapsConsumerRecorded;
      if (recordsTimeGetDevCapsConsumer) {
        timeGetDevCapsConsumerRecorded = true;
      }
      this.traceTimeGetDevCaps = focusedTimeGetDevCapsObservation
        ? {
            output: args[0],
            outputAddress: args[0].toString(),
            size: args[1].toUInt32(),
            outputBefore: snapshotTimeCaps(args[0]),
            immediateConsumer: recordsTimeGetDevCapsConsumer
              ? {
                  apiContinuation: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    48,
                  ),
                  frameCallerContinuations: snapshotX86FrameCallerContinuations(
                    this.context,
                    8,
                    16,
                  ),
                }
              : null,
          }
        : null;
      this.traceTimeGetDevCapsMainModuleCaller =
        this.traceTimeGetDevCaps !== null && isMainModuleCaller;
      this.traceTimePeriod = focusedTimePeriodObservation
        ? {
            api: name,
            periodMilliseconds: args[0].toUInt32(),
            immediateConsumer: isMainModuleCaller
              ? {
                  apiContinuation: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    24,
                  ),
                  frameCallerContinuations: snapshotX86FrameCallerContinuations(
                    this.context,
                    4,
                    12,
                  ),
                }
              : null,
          }
        : null;
      this.traceTimePeriodMainModuleCaller =
        this.traceTimePeriod !== null && isMainModuleCaller;
      const tracesComInitialization =
        selectedObservations.has("com-initialization") &&
        (name === "CoInitialize" ||
          name === "CoInitializeEx" ||
          name === "CoUninitialize");
      const recordsComInitializationConsumer =
        tracesComInitialization &&
        !comInitializationConsumerRecorded &&
        name === "CoInitialize" &&
        isMainModuleCaller;
      if (recordsComInitializationConsumer) {
        comInitializationConsumerRecorded = true;
      }
      this.traceComInitialization = tracesComInitialization
        ? {
            api: name,
            reserved:
              name === "CoInitialize" || name === "CoInitializeEx"
                ? args[0].toString()
                : null,
            concurrency:
              name === "CoInitializeEx" ? args[1].toUInt32() : null,
            immediateConsumer: recordsComInitializationConsumer
              ? {
                  apiContinuation: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    24,
                  ),
                  frameCallerContinuations: snapshotX86FrameCallerContinuations(
                    this.context,
                    name === "CoInitializeEx" ? 8 : name === "CoInitialize" ? 4 : 0,
                    12,
                  ),
                }
              : null,
          }
        : null;
      const observation =
        this.traceHeapAllocation !== null
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              heapAllocation: this.traceHeapAllocation,
            }
        : this.traceHeapFree !== null
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              heapFree: this.traceHeapFree,
            }
        : this.traceHeapSize !== null
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              heapSize: this.traceHeapSize,
            }
        : name === "SetLastError" && selectedObservations.has("set-last-error")
          ? snapshotSetLastErrorCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context.eax,
            )
        : name === "FlsGetValue" && selectedObservations.has("fls-lifecycle")
          ? snapshotFlsGetValueCall(caller, this.returnAddress, this.lastError)
        : name === "TlsGetValue" && selectedObservations.has("tls-lifecycle")
          ? snapshotTlsGetValueCall(caller, this.returnAddress, this.lastError)
        : name === "GetLastError" && selectedObservations.has("get-last-error")
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              immediateConsumer:
                caller.module.toLowerCase() === "bgi.exe"
                  ? snapshotInstructions(this.returnAddress, 4)
                  : null,
            }
        : name === "GetACP" && selectedObservations.has("get-acp")
          ? snapshotGetAcpCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
            )
        : this.traceSystemDefaultLangId !== null
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              systemDefaultLangId: this.traceSystemDefaultLangId,
            }
        : this.traceDriveType !== null
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              driveType: this.traceDriveType,
            }
        : this.traceGetCpInfo !== null
          ? snapshotGetCpInfoCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              this.traceGetCpInfo.codePage,
              this.traceGetCpInfo.output,
            )
        : this.traceGetSystemInfo !== null
          ? snapshotGetSystemInfoCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              this.traceGetSystemInfo.output,
            )
        : this.traceEnumDisplayMonitors !== null
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              enumDisplayMonitors: {
                hdc: this.traceEnumDisplayMonitors.hdc,
                clipRect: this.traceEnumDisplayMonitors.clipRect,
                clipRectBefore: this.traceEnumDisplayMonitors.clipRectBefore,
                callback: this.traceEnumDisplayMonitors.callback,
                callbackInstructions:
                  this.traceEnumDisplayMonitors.callbackInstructions,
                data: this.traceEnumDisplayMonitors.data,
                immediateConsumer:
                  this.traceEnumDisplayMonitors.immediateConsumer,
              },
            }
        : this.traceSwapMouseButton !== null
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              swapMouseButton: this.traceSwapMouseButton,
            }
        : this.traceTimeGetDevCaps !== null
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              timeGetDevCaps: {
                outputAddress: this.traceTimeGetDevCaps.outputAddress,
                size: this.traceTimeGetDevCaps.size,
                outputBefore: this.traceTimeGetDevCaps.outputBefore,
                immediateConsumer: this.traceTimeGetDevCaps.immediateConsumer,
              },
            }
        : this.traceTimePeriod !== null
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              timePeriod: this.traceTimePeriod,
            }
        : this.traceComInitialization !== null
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              comInitialization: this.traceComInitialization,
            }
        : focusedProcessorFeatureObservation
          ? snapshotProcessorFeatureCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              args[0],
            )
        : focusedSetThreadAffinityMaskObservation
          ? snapshotSetThreadAffinityMaskCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              args[0],
              args[1],
            )
        : focusedUnhandledExceptionFilterObservation
          ? snapshotUnhandledExceptionFilterCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              args[0],
            )
        : this.traceMultiByteConversion !== null
          ? snapshotMultiByteToWideCharCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              this.traceMultiByteConversion.codePage,
              this.traceMultiByteConversion.flags,
              this.traceMultiByteConversion.source,
              this.traceMultiByteConversion.sourceBytes,
              this.traceMultiByteConversion.destination,
              this.traceMultiByteConversion.destinationUnits,
            )
        : this.traceSetCurrentDirectoryA !== null
          ? snapshotSetCurrentDirectoryACall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              this.traceSetCurrentDirectoryA.path,
              this.traceSetCurrentDirectoryA.currentDirectoryBefore,
            )
        : this.traceCreateMutexA !== null
          ? snapshotCreateMutexACall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              this.traceCreateMutexA.securityAttributes,
              this.traceCreateMutexA.initialOwner,
              this.traceCreateMutexA.name,
            )
        : this.traceMutexLifecycle !== null
          ? {
              lastErrorBefore: Number(this.lastError) >>> 0,
              mutexLifecycle: this.traceMutexLifecycle,
            }
        : this.traceGetStringTypeW !== null
          ? snapshotGetStringTypeWCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              this.traceGetStringTypeW.infoType,
              this.traceGetStringTypeW.source,
              this.traceGetStringTypeW.sourceUnits,
              this.traceGetStringTypeW.output,
            )
        : this.traceLCMapStringW !== null
          ? snapshotLCMapStringWCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              this.traceLCMapStringW.locale,
              this.traceLCMapStringW.flags,
              this.traceLCMapStringW.source,
              this.traceLCMapStringW.sourceUnits,
              this.traceLCMapStringW.destination,
              this.traceLCMapStringW.destinationUnits,
            )
        : this.traceInterlockedDecrement !== null
          ? snapshotInterlockedDecrementCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              this.traceInterlockedDecrement.target,
            )
        : this.traceGetModuleFileNameA !== null
          ? snapshotGetModuleFileNameACall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              this.traceGetModuleFileNameA.moduleHandle,
              this.traceGetModuleFileNameA.output,
              this.traceGetModuleFileNameA.capacity,
            )
        : name === "GetStartupInfoW"
          ? { startupInfoBefore: snapshotStartupInfoW(args[0]) }
          : selectedObservations.has("critical-section") &&
              name.includes("CriticalSection")
            ? {
                lastErrorBefore: Number(this.lastError) >>> 0,
                criticalSectionBefore: snapshotCriticalSection(args[0]),
                initializeCriticalSectionConsumer:
                  recordsInitializeCriticalSectionConsumer
                    ? {
                        apiContinuation: snapshotInstructions(
                          this.returnAddress,
                          16,
                        ),
                        frameCallerContinuations:
                          snapshotX86FrameCallerContinuations(
                            this.context,
                            4,
                            12,
                          ),
                      }
                    : null,
              }
          : name === "InterlockedIncrement" &&
              selectedObservations.has("interlocked-increment")
            ? {
                lastErrorBefore: Number(this.lastError) >>> 0,
                interlockedTargetBefore: snapshotLong(args[0]),
              }
          : name === "GetStdHandle" &&
              selectedObservations.has("standard-handle")
            ? { lastErrorBefore: Number(this.lastError) >>> 0 }
          : name === "GetFileType" && selectedObservations.has("file-type")
            ? { lastErrorBefore: Number(this.lastError) >>> 0 }
          : name === "SetHandleCount" &&
              selectedObservations.has("set-handle-count")
            ? { lastErrorBefore: Number(this.lastError) >>> 0 }
          : (name === "GetCommandLineA" || name === "GetCommandLineW") &&
              selectedObservations.has("command-line-a")
            ? { lastErrorBefore: Number(this.lastError) >>> 0 }
          : (name === "GetEnvironmentStringsW" ||
                name === "FreeEnvironmentStringsW") &&
              selectedObservations.has("environment-strings-w")
            ? {
                lastErrorBefore: Number(this.lastError) >>> 0,
                ...(name === "FreeEnvironmentStringsW"
                  ? {
                      environmentBlockBeforeFree: snapshotUtf16EnvironmentBlock(
                        args[0],
                        readProcessParametersEnvironmentPointer(),
                      ),
                    }
                  : {}),
              }
          : this.traceWideCharConversion !== null
            ? {
                lastErrorBefore: Number(this.lastError) >>> 0,
                codePage: args[0].toUInt32(),
                flags: args[1].toUInt32(),
                source: snapshotWideCharSource(
                  this.traceWideCharConversion.source,
                  this.traceWideCharConversion.sourceUnits,
                ),
                destinationBefore: snapshotByteBuffer(
                  this.traceWideCharConversion.destination,
                  Math.max(0, this.traceWideCharConversion.destinationBytes),
                ),
                defaultCharPointer: args[6].toString(),
                usedDefaultCharBefore: snapshotOptionalBool(
                  this.traceWideCharConversion.usedDefaultChar,
                ),
              }
          : name.startsWith("Heap") ||
              name.startsWith("GetModuleHandle") ||
              (name.startsWith("Tls") && selectedObservations.has("tls-lifecycle")) ||
              (name.startsWith("Fls") && selectedObservations.has("fls-lifecycle")) ||
              ((name === "EncodePointer" || name === "DecodePointer") &&
                selectedObservations.has("pointer-encoding")) ||
              (name === "GetProcAddress" && selectedObservations.has("dynamic-export"))
            ? { lastErrorBefore: Number(this.lastError) >>> 0 }
            : {};
      this.traceStartupInfoPointer =
        name === "GetStartupInfoW" ? args[0] : null;
      this.traceCriticalSectionPointer =
        selectedObservations.has("critical-section") &&
        name.includes("CriticalSection")
          ? args[0]
          : null;
      this.traceInterlockedPointer =
        name === "InterlockedIncrement" &&
        selectedObservations.has("interlocked-increment")
          ? args[0]
          : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: arguments_,
          ...observation,
        },
        this.returnAddress,
      );
      if (this.traceEnumDisplayMonitors !== null) {
        activeEnumDisplayMonitorsByThread.set(
          this.traceFrame.threadId,
          this.traceEnumDisplayMonitors,
        );
      }
    },
    onLeave(result) {
      if (this.trackHeapSizeAllocation != null && !result.isNull()) {
        observedMainModuleHeapAllocations.set(result.toString().toLowerCase(), {
          callId: null,
          requestedBytes: this.trackHeapSizeAllocation.requestedBytes,
        });
      }
      if (this.trackHeapSizeFree != null && result.toUInt32() !== 0) {
        observedMainModuleHeapAllocations.delete(this.trackHeapSizeFree);
      }
      if (this.traceFrame === null) {
        return;
      }
      const lastError = this.lastError;
      if (this.traceCriticalSectionOracle) {
        criticalSectionOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceEnvironmentStringsOracle) {
        environmentStringsOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceWideCharOracle) {
        wideCharOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceMultiByteOracle) {
        multiByteOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceSetCurrentDirectoryOracle) {
        setCurrentDirectoryOracleResult = result.toUInt32();
        setCurrentDirectoryOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceCreateMutexOracle && name === "CreateMutexA") {
        createMutexOracleResult = result.toString();
        createMutexOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceGetStringTypeOracle) {
        getStringTypeOracleResult = result.toUInt32();
        getStringTypeOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceLCMapStringOracle) {
        lcMapStringOracleResult = signed32(result);
        lcMapStringOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceInterlockedDecrementOracle) {
        interlockedDecrementOracleResult = signed32(result);
        interlockedDecrementOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceGetModuleFileNameAOracle) {
        getModuleFileNameAOracleResult = result.toUInt32();
        getModuleFileNameAOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceGetLastErrorOracle) {
        getLastErrorOracleResult = result.toUInt32();
        getLastErrorOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceSetLastErrorOracle) {
        setLastErrorOracleEaxAfter = result.toUInt32();
        setLastErrorOracleLastErrorAfter = Number(lastError) >>> 0;
      }
      if (this.traceGetAcpOracle) {
        getAcpOracleResult = result.toUInt32();
        getAcpOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceSystemDefaultLangIdOracle) {
        systemDefaultLangIdOracleResult = result.toUInt32();
        systemDefaultLangIdOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceDriveTypeOracle) {
        driveTypeOracleResult = result.toUInt32();
        driveTypeOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceGetCpInfoOracle) {
        getCpInfoOracleResult = result.toUInt32();
        getCpInfoOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceProcessorFeatureOracle) {
        processorFeatureOracleResult = result.toUInt32();
        processorFeatureOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceSetThreadAffinityMaskOracle) {
        setThreadAffinityMaskOracleResult = result.toUInt32();
        setThreadAffinityMaskOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceGetSystemInfoOracle) {
        getSystemInfoOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceEnumDisplayMonitorsOracle) {
        enumDisplayMonitorsOracleResult = result.toUInt32();
        enumDisplayMonitorsOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceTimeGetDevCapsOracle) {
        timeGetDevCapsOracleCall.hookResult = result.toUInt32();
        timeGetDevCapsOracleCall.hookLastErrorAfter =
          Number(lastError) >>> 0;
      }
      if (this.traceTimePeriodOracle) {
        timePeriodOracleCall.hookResult = result.toUInt32();
        timePeriodOracleCall.hookLastErrorAfter = Number(lastError) >>> 0;
      }
      if (this.traceComInitializationOracle) {
        comInitializationOracleCall.hookResult = result.toUInt32();
        comInitializationOracleCall.hookLastErrorAfter = Number(lastError) >>> 0;
      }
      if (this.traceThreadAffinityPrimaryCall === 1 && result.toUInt32() !== 0) {
        setThreadAffinityCalibration = {
          active: true,
          startedAtMs: Date.now(),
          initialRequestedMask: this.traceThreadAffinityRequestedMask,
          initialPreviousMask: result.toUInt32(),
          tickCountCalls: 0,
          valueChanges: 0,
          regressions: 0,
          firstValue: null,
          lastValue: null,
          firstSamples: [],
          lastSamples: [],
        };
      } else if (this.traceThreadAffinityPrimaryCall === 2) {
        const calibration = setThreadAffinityCalibration;
        if (calibration !== null) {
          calibration.active = false;
          emit("set-thread-affinity-mask-calibration", {
            profile: "bgi-get-tick-count-affinity-calibration-v1",
            initialRequestedMask: calibration.initialRequestedMask,
            initialPreviousMask: calibration.initialPreviousMask,
            restoreRequestedMask: this.traceThreadAffinityRequestedMask,
            restorePreviousMask: result.toUInt32(),
            tickCountCalls: calibration.tickCountCalls,
            valueChanges: calibration.valueChanges,
            regressions: calibration.regressions,
            firstValue: calibration.firstValue,
            lastValue: calibration.lastValue,
            elapsedModulo32:
              calibration.firstValue === null || calibration.lastValue === null
                ? null
                : (calibration.lastValue - calibration.firstValue) >>> 0,
            observedDurationMs: Math.max(0, Date.now() - calibration.startedAtMs),
            firstSamples: calibration.firstSamples,
            lastSamples: calibration.lastSamples,
          });
          setThreadAffinityCalibration = null;
        }
      }
      if (this.traceUnhandledExceptionFilterOracle) {
        unhandledExceptionFilterOracleResult = result.toString();
        unhandledExceptionFilterOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceHeapSizeOracle) {
        heapSizeOracleResult = result.toUInt32();
        heapSizeOracleLastError = Number(lastError) >>> 0;
      }
      const wideCharResult =
        this.traceWideCharConversion === null ? null : signed32(result);
      const multiByteResult =
        this.traceMultiByteConversion === null ? null : signed32(result);
      const lcMapStringResult =
        this.traceLCMapStringW === null ? null : signed32(result);
      const setCurrentDirectoryAfter =
        this.traceSetCurrentDirectoryA === null
          ? null
          : (() => {
              const currentDirectory = snapshotCurrentDirectoryA(module);
              this.lastError = lastError;
              return currentDirectory;
            })();
      const observation = {
        ...(this.traceHeapAllocation === null
          ? {}
          : {
              heapAllocation: {
                heap: this.traceHeapAllocation.heap,
                flags: this.traceHeapAllocation.flags,
                requestedBytes: this.traceHeapAllocation.requestedBytes,
                bufferAfterReturn: snapshotByteBuffer(
                  result,
                  this.traceHeapAllocation.requestedBytes,
                ),
              },
            }),
        ...(this.traceHeapFree === null
          ? {}
          : {
              heapFree: {
                pointer: this.traceHeapFree.pointer,
                relatedAllocationCallId:
                  this.traceHeapFree.relatedAllocationCallId,
                requestedBytes: this.traceHeapFree.requestedBytes,
              },
            }),
        ...(this.traceHeapSize === null
          ? {}
          : {
              heapSize: {
                heap: this.traceHeapSize.heap,
                flags: this.traceHeapSize.flags,
                pointer: this.traceHeapSize.pointer,
                relatedAllocationCallId:
                  this.traceHeapSize.relatedAllocationCallId,
                requestedBytes: this.traceHeapSize.requestedBytes,
                result: result.toUInt32(),
              },
            }),
        ...(this.traceStartupInfoPointer === null
          ? {}
          : {
              startupInfoAfter: snapshotStartupInfoW(
                this.traceStartupInfoPointer,
                true,
              ),
            }),
        ...(this.traceCriticalSectionPointer === null
          ? {}
          : {
              criticalSectionAfter: snapshotCriticalSection(
                this.traceCriticalSectionPointer,
              ),
            }),
        ...(this.traceInterlockedPointer === null
          ? {}
          : {
              interlockedTargetAfter: snapshotLong(this.traceInterlockedPointer),
            }),
        ...(this.traceInterlockedDecrement === null
          ? {}
          : {
              interlockedDecrement: {
                result: signed32(result),
                targetAfter: snapshotLong(this.traceInterlockedDecrement.target),
              },
            }),
        ...(this.traceGetModuleFileNameA === null
          ? {}
          : {
              getModuleFileNameA: {
                result: result.toUInt32(),
                outputAfter: snapshotAnsiOutputBuffer(
                  this.traceGetModuleFileNameA.output,
                  this.traceGetModuleFileNameA.capacity,
                ),
              },
            }),
        ...(name === "SetLastError" && selectedObservations.has("set-last-error")
          ? { eaxAfter: result.toUInt32() }
          : {}),
        ...((name === "GetCommandLineA" || name === "GetCommandLineW") &&
        selectedObservations.has("command-line-a")
          ? {
              commandLine: snapshotNulTerminatedString(
                result,
                name === "GetCommandLineW" ? "utf16" : "ansi",
              ),
            }
          : {}),
        ...(name === "GetEnvironmentStringsW" &&
        selectedObservations.has("environment-strings-w")
          ? {
              environmentBlock: snapshotUtf16EnvironmentBlock(
                result,
                readProcessParametersEnvironmentPointer(),
              ),
            }
          : {}),
        ...(this.traceWideCharConversion === null
          ? {}
          : {
              wideCharConversion: {
                result: wideCharResult,
                destinationAfter: snapshotByteBuffer(
                  this.traceWideCharConversion.destination,
                  wideCharResult > 0
                    ? wideCharResult
                    : Math.max(0, this.traceWideCharConversion.destinationBytes),
                ),
                usedDefaultCharAfter: snapshotOptionalBool(
                  this.traceWideCharConversion.usedDefaultChar,
                ),
              },
            }),
        ...(this.traceMultiByteConversion === null
          ? {}
          : {
              multiByteToWideChar: {
                result: multiByteResult,
                destinationAfter: snapshotWideCharBuffer(
                  this.traceMultiByteConversion.destination,
                  multiByteResult > 0
                    ? multiByteResult
                    : Math.max(0, this.traceMultiByteConversion.destinationUnits),
                ),
              },
            }),
        ...(this.traceSetCurrentDirectoryA === null
          ? {}
          : {
              setCurrentDirectoryA: {
                result: result.toUInt32(),
                pathAfter: snapshotMultiByteSource(
                  this.traceSetCurrentDirectoryA.path,
                  -1,
                ),
                currentDirectoryAfter: setCurrentDirectoryAfter,
              },
            }),
        ...(this.traceCreateMutexA === null
          ? {}
          : {
              createMutexA: {
                result: result.toString(),
                nameAfter: snapshotMultiByteSource(
                  this.traceCreateMutexA.name,
                  -1,
                ),
              },
            }),
        ...(this.traceMutexLifecycle === null
          ? {}
          : {
              mutexLifecycle: {
                ...this.traceMutexLifecycle,
                result: result.toString(),
              },
            }),
        ...(this.traceGetStringTypeW === null
          ? {}
          : {
              getStringTypeW: {
                result: result.toUInt32(),
                outputAfter: snapshotWordBuffer(
                  this.traceGetStringTypeW.output,
                  Math.max(0, this.traceGetStringTypeW.sourceUnits),
                ),
              },
            }),
        ...(this.traceLCMapStringW === null
          ? {}
          : {
              lcMapStringW: {
                result: lcMapStringResult,
                destinationAfter: snapshotWideCharBuffer(
                  this.traceLCMapStringW.destination,
                  lcMapStringResult > 0
                    ? lcMapStringResult
                    : Math.max(0, this.traceLCMapStringW.destinationUnits),
                ),
              },
            }),
        ...(this.traceGetCpInfo === null
          ? {}
          : {
              getCpInfo: {
                codePage: this.traceGetCpInfo.codePage.toUInt32(),
                outputAfter: snapshotBoundedRawBytes(
                  this.traceGetCpInfo.output,
                  32,
                ),
                measuredOutput: snapshotMeasuredCpInfo(
                  this.traceGetCpInfo.output,
                ),
              },
            }),
        ...(this.traceGetSystemInfo === null
          ? {}
          : {
              getSystemInfo: {
                outputAfter: snapshotSystemInfo(this.traceGetSystemInfo.output),
              },
            }),
        ...(this.traceSystemDefaultLangId === null
          ? {}
          : {
              systemDefaultLangId: {
                ...this.traceSystemDefaultLangId,
                langId: result.toUInt32() & 0xffff,
                rawEax: result.toUInt32(),
              },
            }),
        ...(this.traceDriveType === null
          ? {}
          : {
              driveType: {
                ...this.traceDriveType,
                result: result.toUInt32(),
              },
            }),
        ...(this.traceEnumDisplayMonitors === null
          ? {}
          : {
              enumDisplayMonitors: {
                result: result.toUInt32(),
                hdc: this.traceEnumDisplayMonitors.hdc,
                clipRect: this.traceEnumDisplayMonitors.clipRect,
                clipRectAfter:
                  this.traceEnumDisplayMonitors.clipRect === ptr(0).toString()
                    ? null
                    : snapshotRect(ptr(this.traceEnumDisplayMonitors.clipRect)),
                callback: this.traceEnumDisplayMonitors.callback,
                data: this.traceEnumDisplayMonitors.data,
                callbacks: this.traceEnumDisplayMonitors.callbacks,
              },
            }),
        ...(this.traceSwapMouseButton === null
          ? {}
          : {
              swapMouseButton: {
                ...this.traceSwapMouseButton,
                result: result.toUInt32(),
                stateAfter:
                  swapMouseButtonGetSystemMetrics === null
                    ? null
                    : swapMouseButtonGetSystemMetrics(23) !== 0,
              },
            }),
        ...(this.traceTimeGetDevCaps === null
          ? {}
          : {
              timeGetDevCaps: {
                result: result.toUInt32(),
                outputAddress: this.traceTimeGetDevCaps.outputAddress,
                size: this.traceTimeGetDevCaps.size,
                outputAfter: snapshotTimeCaps(this.traceTimeGetDevCaps.output),
              },
            }),
        ...(this.traceTimePeriod === null
          ? {}
          : {
              timePeriod: {
                ...this.traceTimePeriod,
                result: result.toUInt32(),
              },
            }),
        ...(this.traceComInitialization === null
          ? {}
          : {
              comInitialization: {
                ...this.traceComInitialization,
                result:
                  name === "CoUninitialize" ? null : result.toUInt32(),
                signedResult:
                  name === "CoUninitialize" ? null : signed32(result),
              },
            }),
      };
      if (this.traceEnumDisplayMonitors !== null) {
        const active = activeEnumDisplayMonitorsByThread.get(
          this.traceFrame.threadId,
        );
        if (active === this.traceEnumDisplayMonitors) {
          activeEnumDisplayMonitorsByThread.delete(this.traceFrame.threadId);
        }
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          ...observation,
        },
        lastError,
      );
      if (
        this.traceTimeGetDevCaps !== null &&
        this.traceTimeGetDevCapsMainModuleCaller &&
        timeGetDevCapsMainModuleCalls.length === 0
      ) {
        timeGetDevCapsMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          outputAddress: this.traceTimeGetDevCaps.outputAddress,
          size: this.traceTimeGetDevCaps.size,
          outputBefore: this.traceTimeGetDevCaps.outputBefore,
          outputAfter: snapshotTimeCaps(this.traceTimeGetDevCaps.output),
          result: result.toUInt32(),
          lastError: Number(lastError) >>> 0,
          immediateConsumer: this.traceTimeGetDevCaps.immediateConsumer,
        });
        setImmediate(maybeCompleteTimeGetDevCapsObservation);
      }
      if (
        this.traceTimePeriod !== null &&
        this.traceTimePeriodMainModuleCaller
      ) {
        timePeriodMainModuleCalls.push({
          callId: this.traceFrame.callId,
          api: this.traceTimePeriod.api,
          returnAddress: this.returnAddress.toString(),
          periodMilliseconds: this.traceTimePeriod.periodMilliseconds,
          result: result.toUInt32(),
          lastError: Number(lastError) >>> 0,
          immediateConsumer: this.traceTimePeriod.immediateConsumer,
        });
        setImmediate(maybeCompleteTimeGetDevCapsObservation);
      }
      if (
        this.traceSwapMouseButton !== null &&
        this.traceSwapMouseButtonMainModuleCaller &&
        !swapMouseButtonCompletionRecorded
      ) {
        swapMouseButtonMainModuleCalls.push({
          callId: this.traceFrame.callId,
          argument: this.traceSwapMouseButton.swap,
          stateBefore: this.traceSwapMouseButton.stateBefore,
          result: result.toUInt32(),
          lastError: Number(lastError) >>> 0,
          stateAfter:
            swapMouseButtonGetSystemMetrics === null
              ? null
              : swapMouseButtonGetSystemMetrics(23) !== 0,
        });
        if (swapMouseButtonMainModuleCalls.length === 2) {
          swapMouseButtonCompletionRecorded = true;
          const calls = swapMouseButtonMainModuleCalls.map((call) => ({
            ...call,
          }));
          setImmediate(() => {
            completeSwapMouseButtonObservation(calls);
          });
        }
      }
      if (
        this.traceDriveType !== null &&
        this.traceDriveTypeMainModuleCaller
      ) {
        driveTypeMainModuleCallCount += 1;
        if (
          !driveTypeScanCompletionRecorded &&
          driveTypeMainModuleCallCount === 26
        ) {
          driveTypeScanCompletionRecorded = true;
          emit("drive-type-scan-complete", {
            profile: "x86-drive-letter-scan-v1",
            callId: this.traceFrame.callId,
            callCount: driveTypeMainModuleCallCount,
            rootPath: this.traceDriveType.rootPathBefore,
            result: result.toUInt32(),
            lastError: Number(lastError) >>> 0,
          });
          if (stopOnEvent === "drive-type-scan-complete") {
            recv("vine-trace-stop", () => {}).wait();
          }
        }
      }
      if (this.traceHeapAllocation !== null && !result.isNull()) {
        observedMainModuleHeapAllocations.set(result.toString().toLowerCase(), {
          callId: this.traceFrame.callId,
          requestedBytes: this.traceHeapAllocation.requestedBytes,
        });
      }
      if (this.traceHeapFree !== null && result.toUInt32() !== 0) {
        observedMainModuleHeapAllocations.delete(this.traceHeapFree.pointer);
      }
      if (this.traceCreateMutexA !== null && !result.isNull()) {
        trackedMutexHandles.set(result.toString().toLowerCase(), {
          createdByCallId: this.traceFrame.callId,
          initialOwner: this.traceCreateMutexA.initialOwner.toUInt32(),
          name: this.traceCreateMutexA.nameBefore,
        });
      }
      if (
        this.traceMutexLifecycle !== null &&
        name === "CloseHandle" &&
        result.toUInt32() !== 0
      ) {
        trackedMutexHandles.delete(this.traceMutexLifecycle.handle);
      }
      if (name === "Direct3DCreate9" && selectedVtables.has("idirect3d9")) {
        attachIdirect3d9(result);
      }
    },
  });
}

function scheduleSetCurrentDirectoryAOracle(module) {
  if (setCurrentDirectoryOracleScheduled) {
    return;
  }
  setCurrentDirectoryOracleScheduled = true;
  setImmediate(() => {
    let setCurrentDirectoryA = null;
    let originalPointer = null;
    let restored = false;
    try {
      const address = findExport(module, "SetCurrentDirectoryA");
      if (address === null) {
        throw new Error("SetCurrentDirectoryA export is unavailable");
      }
      setCurrentDirectoryA = new SystemFunction(
        address,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const original = snapshotCurrentDirectoryA(module);
      const originalBytes = original.path?.bytes ?? null;
      if (
        original.error !== undefined ||
        originalBytes === null ||
        originalBytes.length < 2 ||
        originalBytes.at(-1) !== 0
      ) {
        throw new Error("current-directory oracle could not snapshot its original path");
      }

      const allocatePath = (bytes) => {
        const pointer = Memory.alloc(Math.max(1, bytes.length));
        pointer.writeByteArray(bytes.length === 0 ? [0] : bytes);
        return pointer;
      };
      originalPointer = allocatePath(originalBytes);
      const invoke = (id, sentinel, pathBytes) => {
        const pointer = allocatePath(pathBytes);
        setCurrentDirectoryOracleSentinel = sentinel;
        setCurrentDirectoryOracleResult = null;
        setCurrentDirectoryOracleLastError = null;
        try {
          const outcome = setCurrentDirectoryA(pointer);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            setCurrentDirectoryOracleResult === null ||
            setCurrentDirectoryOracleLastError === null
          ) {
            throw new Error(`${id} did not pass through the current-directory hook`);
          }
          return {
            record: {
              id,
              sentinel,
              pathBytes,
              result,
              hookResult: setCurrentDirectoryOracleResult,
              hookLastErrorAfter: setCurrentDirectoryOracleLastError,
              systemLastError,
              currentDirectoryAfter: snapshotCurrentDirectoryA(module),
            },
            pointer,
          };
        } finally {
          setCurrentDirectoryOracleSentinel = null;
          setCurrentDirectoryOracleResult = null;
          setCurrentDirectoryOracleLastError = null;
        }
      };

      const parent = invoke("relative-parent", 87, [0x2e, 0x2e, 0]);
      parent.pointer.writeByteArray([0x58, 0x58, 0]);
      const currentDirectoryAfterInputMutation = snapshotCurrentDirectoryA(module);
      const self = invoke("relative-self", 88, [0x2e, 0]);

      const originalWithoutNul = originalBytes.slice(0, -1);
      const separator = originalWithoutNul.at(-1) === 0x5c ? [] : [0x5c];
      const moduleNameBytes = [...Process.mainModule.name].map((character) =>
        character.charCodeAt(0),
      );
      const filePathBytes = [
        ...originalWithoutNul,
        ...separator,
        ...moduleNameBytes,
        0,
      ];
      const missingPathBytes = [
        ...originalWithoutNul,
        ...separator,
        ..."__vine_missing_current_directory_oracle_31__"].map((value) =>
          typeof value === "number" ? value : value.charCodeAt(0),
        );
      missingPathBytes.push(0);
      const file = invoke("existing-file-not-directory", 89, filePathBytes);
      const missing = invoke("missing-directory", 90, missingPathBytes);
      const empty = invoke("empty-path", 91, [0]);
      const restore = invoke("restore-original", 92, originalBytes);
      restored = restore.record.result !== 0;

      emit("set-current-directory-a-oracle", {
        profile: "x86-process-directory-copy-relative-failure-v1",
        original,
        inputCopiedBeforeMutation:
          parent.record.result !== 0 &&
          currentDirectoryAfterInputMutation.path?.contentFnv1a32 ===
            parent.record.currentDirectoryAfter.path?.contentFnv1a32,
        currentDirectoryAfterInputMutation,
        calls: [
          parent.record,
          self.record,
          file.record,
          missing.record,
          empty.record,
          restore.record,
        ],
        restored,
      });
    } catch (error) {
      emit("set-current-directory-a-oracle-error", { error: error.message });
    } finally {
      setCurrentDirectoryOracleSentinel = null;
      setCurrentDirectoryOracleResult = null;
      setCurrentDirectoryOracleLastError = null;
      if (!restored && setCurrentDirectoryA !== null && originalPointer !== null) {
        try {
          setCurrentDirectoryA(originalPointer);
        } catch (error) {
          emit("set-current-directory-a-oracle-restore-error", {
            error: error.message,
          });
        }
      }
    }
  });
}

function scheduleCreateMutexAOracle(module) {
  if (createMutexOracleScheduled) {
    return;
  }
  createMutexOracleScheduled = true;
  setImmediate(() => {
    const handles = [];
    let closeHandle = null;
    try {
      const address = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const createMutexA = new SystemFunction(
        address("CreateMutexA"),
        "pointer",
        ["pointer", "uint32", "pointer"],
        "stdcall",
      );
      const waitForSingleObject = new SystemFunction(
        address("WaitForSingleObject"),
        "uint32",
        ["pointer", "uint32"],
        "stdcall",
      );
      const releaseMutex = new SystemFunction(
        address("ReleaseMutex"),
        "uint32",
        ["pointer"],
        "stdcall",
      );
      closeHandle = new SystemFunction(
        address("CloseHandle"),
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const nameBytes = [
        ...`Local\\VineCreateMutexAOracle-${Process.id}`,
      ].map((character) => character.charCodeAt(0));
      nameBytes.push(0);
      const allocateName = () => {
        const pointer = Memory.alloc(nameBytes.length);
        pointer.writeByteArray(nameBytes);
        return pointer;
      };
      const invokeCreate = (id, sentinel, initialOwner, name) => {
        createMutexOracleSentinel = sentinel;
        createMutexOracleResult = null;
        createMutexOracleLastError = null;
        try {
          const outcome = createMutexA(ptr(0), initialOwner, name);
          const result = outcome.value.toString();
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            createMutexOracleResult === null ||
            createMutexOracleLastError === null
          ) {
            throw new Error(`${id} did not pass through the CreateMutexA hook`);
          }
          if (result !== "0x0") {
            handles.push(outcome.value);
          }
          return {
            id,
            sentinel,
            initialOwner,
            name: snapshotMultiByteSource(name, -1),
            result,
            hookResult: createMutexOracleResult,
            hookLastErrorAfter: createMutexOracleLastError,
            systemLastError,
          };
        } finally {
          createMutexOracleSentinel = null;
          createMutexOracleResult = null;
          createMutexOracleLastError = null;
        }
      };
      const invokeUint32 = (api, id, sentinel, operation) => {
        createMutexOracleSentinel = sentinel;
        try {
          const outcome = operation();
          return {
            api,
            id,
            sentinel,
            result: Number(outcome.value) >>> 0,
            systemLastError: Number(outcome.lastError) >>> 0,
          };
        } finally {
          createMutexOracleSentinel = null;
        }
      };

      const firstName = allocateName();
      const first = invokeCreate("create-initial-owner", 87, 1, firstName);
      firstName.writeU8(0x58);
      const firstInputAfterMutation = snapshotMultiByteSource(firstName, -1);
      const duplicateName = allocateName();
      const duplicate = invokeCreate(
        "create-existing-initial-owner-ignored",
        88,
        1,
        duplicateName,
      );
      const firstHandle = ptr(first.result);
      const duplicateHandle = ptr(duplicate.result);
      const recursiveWait = invokeUint32(
        "WaitForSingleObject",
        "wait-recursive-through-duplicate-handle",
        89,
        () => waitForSingleObject(duplicateHandle, 0),
      );
      const releaseRecursive = invokeUint32(
        "ReleaseMutex",
        "release-recursive-acquisition",
        90,
        () => releaseMutex(duplicateHandle),
      );
      const releaseInitial = invokeUint32(
        "ReleaseMutex",
        "release-initial-ownership",
        91,
        () => releaseMutex(firstHandle),
      );
      const releaseUnowned = invokeUint32(
        "ReleaseMutex",
        "reject-release-while-unowned",
        92,
        () => releaseMutex(firstHandle),
      );
      const waitAfterRelease = invokeUint32(
        "WaitForSingleObject",
        "wait-after-full-release",
        93,
        () => waitForSingleObject(duplicateHandle, 0),
      );
      const releaseAfterWait = invokeUint32(
        "ReleaseMutex",
        "release-after-wait",
        94,
        () => releaseMutex(duplicateHandle),
      );
      const closeFirst = invokeUint32(
        "CloseHandle",
        "close-first-handle",
        95,
        () => closeHandle(firstHandle),
      );
      const waitAfterFirstClose = invokeUint32(
        "WaitForSingleObject",
        "wait-through-remaining-handle",
        96,
        () => waitForSingleObject(duplicateHandle, 0),
      );
      const releaseAfterFirstClose = invokeUint32(
        "ReleaseMutex",
        "release-through-remaining-handle",
        97,
        () => releaseMutex(duplicateHandle),
      );
      const closeDuplicate = invokeUint32(
        "CloseHandle",
        "close-last-handle",
        98,
        () => closeHandle(duplicateHandle),
      );
      const recreatedName = allocateName();
      const recreated = invokeCreate(
        "recreate-after-last-close",
        99,
        0,
        recreatedName,
      );
      const recreatedHandle = ptr(recreated.result);
      const waitRecreated = invokeUint32(
        "WaitForSingleObject",
        "wait-recreated-unowned-mutex",
        100,
        () => waitForSingleObject(recreatedHandle, 0),
      );
      const releaseRecreated = invokeUint32(
        "ReleaseMutex",
        "release-recreated-mutex",
        101,
        () => releaseMutex(recreatedHandle),
      );
      const closeRecreated = invokeUint32(
        "CloseHandle",
        "close-recreated-handle",
        102,
        () => closeHandle(recreatedHandle),
      );

      emit("create-mutex-a-oracle", {
        profile: "x86-named-copy-owner-handle-lifetime-v1",
        constants: {
          errorAlreadyExists: 183,
          errorNotOwner: 288,
          waitObject0: 0,
          zeroTimeoutMilliseconds: 0,
        },
        nameBytes,
        firstInputAfterMutation,
        calls: [
          first,
          duplicate,
          recursiveWait,
          releaseRecursive,
          releaseInitial,
          releaseUnowned,
          waitAfterRelease,
          releaseAfterWait,
          closeFirst,
          waitAfterFirstClose,
          releaseAfterFirstClose,
          closeDuplicate,
          recreated,
          waitRecreated,
          releaseRecreated,
          closeRecreated,
        ],
        relations: {
          nameCopiedBeforeCallerMutation: duplicate.systemLastError === 183,
          sameObjectReturnedThroughDistinctHandles:
            duplicate.systemLastError === 183 && first.result !== duplicate.result,
          newObjectClearsLastError:
            first.systemLastError === 0 && recreated.systemLastError === 0,
          initialOwnerGrantedAndExistingInitialOwnerIgnored:
            recursiveWait.result === 0 &&
            releaseRecursive.result !== 0 &&
            releaseInitial.result !== 0 &&
            releaseUnowned.result === 0 &&
            releaseUnowned.systemLastError === 288,
          remainingHandleKeepsObjectAlive:
            closeFirst.result !== 0 &&
            waitAfterFirstClose.result === 0 &&
            releaseAfterFirstClose.result !== 0,
          lastCloseRetiresNamedObject:
            closeDuplicate.result !== 0 &&
            recreated.result !== "0x0" &&
            recreated.systemLastError === 0,
        },
      });
    } catch (error) {
      emit("create-mutex-a-oracle-error", { error: error.message });
    } finally {
      createMutexOracleSentinel = null;
      createMutexOracleResult = null;
      createMutexOracleLastError = null;
      if (closeHandle !== null) {
        for (const handle of handles) {
          try {
            closeHandle(handle);
          } catch {
            // Best-effort cleanup after a partial oracle sequence.
          }
        }
      }
    }
  });
}

function scheduleCommandLineOracle(module) {
  if (commandLineOracleScheduled) {
    return;
  }
  commandLineOracleScheduled = true;
  setImmediate(() => {
    try {
      const getExport = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const getCommandLineA = new NativeFunction(
        getExport("GetCommandLineA"),
        "pointer",
        [],
        "stdcall",
      );
      const getCommandLineW = new NativeFunction(
        getExport("GetCommandLineW"),
        "pointer",
        [],
        "stdcall",
      );
      const observations = [];
      for (const [api, encoding, operation] of [
        ["GetCommandLineA", "ansi", getCommandLineA],
        ["GetCommandLineA", "ansi", getCommandLineA],
        ["GetCommandLineW", "utf16", getCommandLineW],
        ["GetCommandLineW", "utf16", getCommandLineW],
      ]) {
        const pointer = operation();
        observations.push({
          api,
          commandLine: snapshotNulTerminatedString(pointer, encoding),
        });
      }
      emit("command-line-oracle", {
        profile: "x86-process-lifetime-stability-v1",
        observations,
        ansiPointerStable:
          observations[0].commandLine.pointer === observations[1].commandLine.pointer,
        utf16PointerStable:
          observations[2].commandLine.pointer === observations[3].commandLine.pointer,
        ansiAndUtf16PointersDistinct:
          observations[0].commandLine.pointer !== observations[2].commandLine.pointer,
        stringsEqual:
          observations[0].commandLine.string === observations[2].commandLine.string,
      });
    } catch (error) {
      emit("command-line-oracle-error", { error: error.message });
    }
  });
}

function scheduleEnvironmentStringsOracle(module) {
  if (environmentStringsOracleScheduled) {
    return;
  }
  environmentStringsOracleScheduled = true;
  setImmediate(() => {
    try {
      const getExport = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const getEnvironmentStringsW = new NativeFunction(
        getExport("GetEnvironmentStringsW"),
        "pointer",
        [],
        "stdcall",
      );
      const freeEnvironmentStringsW = new NativeFunction(
        getExport("FreeEnvironmentStringsW"),
        "int",
        ["pointer"],
        "stdcall",
      );
      const processEnvironmentPointer = readProcessParametersEnvironmentPointer();
      const invoke = (step, sentinel, operation) => {
        environmentStringsOracleSentinel = sentinel;
        environmentStringsOracleLastError = null;
        try {
          const result = operation();
          const lastErrorAfter = environmentStringsOracleLastError;
          if (lastErrorAfter === null) {
            throw new Error(`${step} did not pass through the environment hook`);
          }
          return { result, lastErrorAfter };
        } finally {
          environmentStringsOracleSentinel = null;
          environmentStringsOracleLastError = null;
        }
      };
      const acquire = (sentinel) => {
        const outcome = invoke("acquire", sentinel, () => getEnvironmentStringsW());
        return {
          sentinel,
          lastErrorAfter: outcome.lastErrorAfter,
          block: snapshotUtf16EnvironmentBlock(
            outcome.result,
            processEnvironmentPointer,
          ),
        };
      };
      const release = (acquisition, sentinel) => {
        const outcome = invoke("release", sentinel, () =>
          freeEnvironmentStringsW(ptr(acquisition.block.pointer)),
        );
        return {
          sentinel,
          lastErrorAfter: outcome.lastErrorAfter,
          result: Number(outcome.result),
          pointer: acquisition.block.pointer,
        };
      };
      const acquisitions = [acquire(0), acquire(87)];
      const releases = [
        release(acquisitions[0], 88),
        release(acquisitions[1], 89),
      ];
      emit("environment-strings-oracle", {
        profile: "x86-wide-copy-lifetime-v1",
        processEnvironmentPointer,
        acquisitions,
        releases,
        pointersEqual:
          acquisitions[0].block.pointer === acquisitions[1].block.pointer,
        contentShapesEqual:
          acquisitions[0].block.byteLengthWithDoubleNul ===
            acquisitions[1].block.byteLengthWithDoubleNul &&
          acquisitions[0].block.entryCount === acquisitions[1].block.entryCount &&
          acquisitions[0].block.contentFnv1a32 ===
            acquisitions[1].block.contentFnv1a32,
      });
    } catch (error) {
      emit("environment-strings-oracle-error", { error: error.message });
    }
  });
}

function scheduleWideCharOracle(module) {
  if (wideCharOracleScheduled) {
    return;
  }
  wideCharOracleScheduled = true;
  setImmediate(() => {
    try {
      const getExport = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const wideCharToMultiByte = new NativeFunction(
        getExport("WideCharToMultiByte"),
        "int",
        [
          "uint32",
          "uint32",
          "pointer",
          "int",
          "pointer",
          "int",
          "pointer",
          "pointer",
        ],
        "stdcall",
      );
      const getAcp = new NativeFunction(
        getExport("GetACP"),
        "uint32",
        [],
        "stdcall",
      );
      const allocateUtf16 = (units) => {
        const pointer = Memory.alloc(units.length * 2);
        units.forEach((unit, index) => pointer.add(index * 2).writeU16(unit));
        return pointer;
      };
      const invoke = ({
        id,
        sentinel,
        sourceUnits,
        sourceCount,
        destinationBytes,
        defaultCharBytes = null,
        observeUsedDefault = false,
      }) => {
        const source = allocateUtf16(sourceUnits);
        const destination =
          destinationBytes === 0 ? ptr(0) : Memory.alloc(destinationBytes);
        if (!destination.isNull()) {
          destination.writeByteArray(Array(destinationBytes).fill(0xa5));
        }
        const defaultChar =
          defaultCharBytes === null ? ptr(0) : Memory.alloc(defaultCharBytes.length);
        if (!defaultChar.isNull()) {
          defaultChar.writeByteArray(defaultCharBytes);
        }
        const usedDefault = observeUsedDefault ? Memory.alloc(4) : ptr(0);
        if (!usedDefault.isNull()) {
          usedDefault.writeU32(0xa5a5a5a5);
        }
        wideCharOracleSentinel = sentinel;
        wideCharOracleLastError = null;
        try {
          const result = Number(
            wideCharToMultiByte(
              0,
              0,
              source,
              sourceCount,
              destination,
              destinationBytes,
              defaultChar,
              usedDefault,
            ),
          );
          if (wideCharOracleLastError === null) {
            throw new Error(`${id} did not pass through the conversion hook`);
          }
          return {
            id,
            sentinel,
            sourceUnits,
            sourceCount,
            destinationCapacity: destinationBytes,
            defaultCharBytes,
            observeUsedDefault,
            result,
            lastErrorAfter: wideCharOracleLastError,
            destinationBytes: destination.isNull()
              ? null
              : [...new Uint8Array(destination.readByteArray(destinationBytes))],
            usedDefaultChar: usedDefault.isNull() ? null : usedDefault.readU32(),
          };
        } finally {
          wideCharOracleSentinel = null;
          wideCharOracleLastError = null;
        }
      };

      const emptyUnits = [0, 0];
      const japaneseUnits = [0x41, 0x3042, 0];
      const unmappableUnits = [0xd83d, 0xde00, 0];
      const emptyQuery = invoke({
        id: "empty-query",
        sentinel: 87,
        sourceUnits: emptyUnits,
        sourceCount: 1,
        destinationBytes: 0,
      });
      const emptyConversion = invoke({
        id: "empty-conversion",
        sentinel: 88,
        sourceUnits: emptyUnits,
        sourceCount: 1,
        destinationBytes: emptyQuery.result,
      });
      const japaneseQuery = invoke({
        id: "ascii-japanese-query",
        sentinel: 89,
        sourceUnits: japaneseUnits,
        sourceCount: japaneseUnits.length,
        destinationBytes: 0,
        observeUsedDefault: true,
      });
      const japaneseConversion = invoke({
        id: "ascii-japanese-conversion",
        sentinel: 90,
        sourceUnits: japaneseUnits,
        sourceCount: japaneseUnits.length,
        destinationBytes: japaneseQuery.result,
        observeUsedDefault: true,
      });
      const unmappableConversion = invoke({
        id: "unmappable-conversion",
        sentinel: 91,
        sourceUnits: unmappableUnits,
        sourceCount: unmappableUnits.length,
        destinationBytes: 8,
        defaultCharBytes: [0x3f, 0],
        observeUsedDefault: true,
      });
      const insufficientBuffer = invoke({
        id: "insufficient-buffer",
        sentinel: 92,
        sourceUnits: japaneseUnits,
        sourceCount: japaneseUnits.length,
        destinationBytes: 1,
        observeUsedDefault: true,
      });
      const zeroSourceCount = invoke({
        id: "zero-source-count",
        sentinel: 93,
        sourceUnits: emptyUnits,
        sourceCount: 0,
        destinationBytes: 0,
      });
      emit("wide-char-to-multi-byte-oracle", {
        profile: "x86-cp-acp-query-convert-v1",
        activeCodePage: Number(getAcp()),
        calls: [
          emptyQuery,
          emptyConversion,
          japaneseQuery,
          japaneseConversion,
          unmappableConversion,
          insufficientBuffer,
          zeroSourceCount,
        ],
      });
    } catch (error) {
      emit("wide-char-to-multi-byte-oracle-error", { error: error.message });
    }
  });
}

function scheduleMultiByteOracle(module) {
  if (multiByteOracleScheduled) {
    return;
  }
  multiByteOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "MultiByteToWideChar");
      if (address === null) {
        throw new Error("MultiByteToWideChar export is unavailable");
      }
      const multiByteToWideChar = new SystemFunction(
        address,
        "int",
        ["uint32", "uint32", "pointer", "int", "pointer", "int"],
        "stdcall",
      );
      const invoke = ({
        id,
        sentinel,
        sourceBytes,
        sourceCount,
        destinationUnits,
        codePage = 932,
        flags = 1,
      }) => {
        const source = Memory.alloc(Math.max(1, sourceBytes.length));
        source.writeByteArray(sourceBytes.length === 0 ? [0] : sourceBytes);
        const storageBytes = Math.max(16, destinationUnits * 2 + 16);
        const storage = Memory.alloc(storageBytes);
        storage.writeByteArray(Array(storageBytes).fill(0xa5));
        const destination = destinationUnits === 0 ? ptr(0) : storage.add(8);
        const storageBefore = snapshotBoundedRawBytes(
          storage,
          Math.min(64, storageBytes),
        );
        multiByteOracleSentinel = sentinel;
        multiByteOracleLastError = null;
        try {
          const outcome = multiByteToWideChar(
            codePage,
            flags,
            source,
            sourceCount,
            destination,
            destinationUnits,
          );
          const result = Number(outcome.value);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (multiByteOracleLastError === null) {
            throw new Error(`${id} did not pass through the conversion hook`);
          }
          return {
            id,
            sentinel,
            codePage,
            flags,
            sourceBytes,
            sourceCount,
            destinationCapacityUnits: destinationUnits,
            result,
            hookLastErrorAfter: multiByteOracleLastError,
            systemLastError,
            storageOffsetBytes: destinationUnits === 0 ? null : 8,
            storageBefore,
            storageAfter: snapshotBoundedRawBytes(
              storage,
              Math.min(64, storageBytes),
            ),
            destinationAfter: snapshotWideCharBuffer(
              destination,
              result > 0 ? result : destinationUnits,
            ),
          };
        } finally {
          multiByteOracleSentinel = null;
          multiByteOracleLastError = null;
        }
      };

      const asciiJapaneseBytes = [0x41, 0x82, 0xa0, 0];
      const query = invoke({
        id: "ascii-japanese-fixed-query",
        sentinel: 87,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: asciiJapaneseBytes.length,
        destinationUnits: 0,
      });
      const conversion = invoke({
        id: "ascii-japanese-fixed-conversion",
        sentinel: 88,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: asciiJapaneseBytes.length,
        destinationUnits: query.result,
      });
      const nulTerminatedQuery = invoke({
        id: "ascii-japanese-nul-query",
        sentinel: 89,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: -1,
        destinationUnits: 0,
      });
      const insufficientBuffer = invoke({
        id: "insufficient-buffer",
        sentinel: 90,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: asciiJapaneseBytes.length,
        destinationUnits: 1,
      });
      const zeroSourceCount = invoke({
        id: "zero-source-count",
        sentinel: 91,
        sourceBytes: [0],
        sourceCount: 0,
        destinationUnits: 0,
      });
      const activeCodePageQuery = invoke({
        id: "active-code-page-nul-query",
        sentinel: 92,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: -1,
        destinationUnits: 0,
        codePage: 0,
        flags: 0,
      });
      const activeCodePageConversion = invoke({
        id: "active-code-page-nul-conversion",
        sentinel: 93,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: -1,
        destinationUnits: activeCodePageQuery.result,
        codePage: 0,
        flags: 0,
      });
      emit("multi-byte-to-wide-char-oracle", {
        profile: "x86-windows-932-fixed-and-acp-nul-v2",
        calls: [
          query,
          conversion,
          nulTerminatedQuery,
          insufficientBuffer,
          zeroSourceCount,
          activeCodePageQuery,
          activeCodePageConversion,
        ],
      });
    } catch (error) {
      emit("multi-byte-to-wide-char-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetStringTypeOracle(module) {
  if (getStringTypeOracleScheduled) {
    return;
  }
  getStringTypeOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetStringTypeW");
      if (address === null) {
        throw new Error("GetStringTypeW export is unavailable");
      }
      const getStringTypeW = new SystemFunction(
        address,
        "uint32",
        ["uint32", "pointer", "int", "pointer"],
        "stdcall",
      );
      const invoke = ({
        id,
        sentinel,
        sourceWords,
        sourceCount = sourceWords.length,
        infoType = 1,
        outputCapacityWords = Math.max(1, sourceWords.length),
      }) => {
        const source = Memory.alloc(Math.max(2, sourceWords.length * 2));
        for (let index = 0; index < sourceWords.length; index += 1) {
          source.add(index * 2).writeU16(sourceWords[index]);
        }
        const leadingGuardBytes = 8;
        const trailingGuardBytes = 8;
        const storageBytes =
          leadingGuardBytes + outputCapacityWords * 2 + trailingGuardBytes;
        const storage = Memory.alloc(storageBytes);
        storage.writeByteArray(Array(storageBytes).fill(0xa5));
        const output = storage.add(leadingGuardBytes);
        const leadingGuardBefore = snapshotBoundedRawBytes(
          storage,
          leadingGuardBytes,
        );
        const outputBefore = snapshotWordBuffer(output, outputCapacityWords);
        const trailingGuard = output.add(outputCapacityWords * 2);
        const trailingGuardBefore = snapshotBoundedRawBytes(
          trailingGuard,
          trailingGuardBytes,
        );
        getStringTypeOracleSentinel = sentinel;
        getStringTypeOracleResult = null;
        getStringTypeOracleLastError = null;
        try {
          const outcome = getStringTypeW(
            infoType,
            source,
            sourceCount,
            output,
          );
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getStringTypeOracleResult === null ||
            getStringTypeOracleLastError === null
          ) {
            throw new Error(`${id} did not pass through the classification hook`);
          }
          return {
            id,
            sentinel,
            infoType,
            sourceWords,
            sourceCount,
            outputCapacityWords,
            result,
            hookResult: getStringTypeOracleResult,
            hookLastErrorAfter: getStringTypeOracleLastError,
            systemLastError,
            leadingGuardBefore,
            leadingGuardAfter: snapshotBoundedRawBytes(
              storage,
              leadingGuardBytes,
            ),
            outputBefore,
            outputAfter: snapshotWordBuffer(output, outputCapacityWords),
            trailingGuardBefore,
            trailingGuardAfter: snapshotBoundedRawBytes(
              trailingGuard,
              trailingGuardBytes,
            ),
          };
        } finally {
          getStringTypeOracleSentinel = null;
          getStringTypeOracleResult = null;
          getStringTypeOracleLastError = null;
        }
      };

      const representativeWords = [
        0x0000,
        0x0009,
        0x000a,
        0x000d,
        0x0020,
        0x0021,
        0x002b,
        0x002c,
        0x002e,
        0x0030,
        0x0039,
        0x0041,
        0x005a,
        0x005f,
        0x0061,
        0x007a,
        0x007f,
        0x0080,
        0x00a0,
        0x00a5,
        0x3000,
        0x3042,
        0x30a2,
        0x30fb,
        0x4e00,
        0xff01,
        0xff10,
        0xff21,
        0xff41,
        0xff61,
        0xff66,
        0xff71,
        0xff9e,
        0xf8f0,
      ];
      const calls = [0, 87, 0x12345678].map((sentinel, index) =>
        invoke({
          id: `representative-ctype1-${index + 1}`,
          sentinel,
          sourceWords: representativeWords,
        }),
      );
      calls.push(
        invoke({
          id: "nul-terminated-ctype1",
          sentinel: 88,
          sourceWords: [0x41, 0],
          sourceCount: -1,
          outputCapacityWords: 2,
        }),
        invoke({
          id: "zero-source-count",
          sentinel: 89,
          sourceWords: [0x41],
          sourceCount: 0,
          outputCapacityWords: 1,
        }),
        invoke({
          id: "negative-nul-terminated-ctype1",
          sentinel: 90,
          sourceWords: [0x41, 0],
          sourceCount: -2,
          outputCapacityWords: 2,
        }),
        invoke({
          id: "invalid-info-type",
          sentinel: 91,
          sourceWords: [0x41],
          infoType: 0,
          outputCapacityWords: 1,
        }),
      );
      emit("get-string-type-w-oracle", {
        profile: "x86-ctype1-representative-v1",
        calls,
      });
    } catch (error) {
      emit("get-string-type-w-oracle-error", { error: error.message });
    }
  });
}

function scheduleLCMapStringOracle(module) {
  if (lcMapStringOracleScheduled) {
    return;
  }
  lcMapStringOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "LCMapStringW");
      if (address === null) {
        throw new Error("LCMapStringW export is unavailable");
      }
      const lcMapStringW = new SystemFunction(
        address,
        "int",
        ["uint32", "uint32", "pointer", "int", "pointer", "int"],
        "stdcall",
      );
      const allocateUtf16 = (words) => {
        const pointer = Memory.alloc(words.length * 2);
        words.forEach((word, index) => pointer.add(index * 2).writeU16(word));
        return pointer;
      };
      const invoke = ({
        id,
        sentinel,
        sourceWords,
        sourceCount = sourceWords.length,
        destinationCapacity = 0,
        locale = 0x0411,
        flags = 0x0100,
        nullDestinationWithCapacity = false,
      }) => {
        const source = allocateUtf16(sourceWords);
        const guardBytes = 16;
        const storage =
          destinationCapacity === 0 || nullDestinationWithCapacity
            ? null
            : Memory.alloc(guardBytes + destinationCapacity * 2 + guardBytes);
        if (storage !== null) {
          storage.writeByteArray(
            Array(guardBytes + destinationCapacity * 2 + guardBytes).fill(0xa5),
          );
        }
        const destination =
          storage === null ? ptr(0) : storage.add(guardBytes);
        const trailingGuard =
          storage === null
            ? null
            : destination.add(destinationCapacity * 2);
        const sourceBefore = snapshotWideCharBuffer(source, sourceWords.length);
        const leadingGuardBefore =
          storage === null ? null : snapshotBoundedRawBytes(storage, guardBytes);
        const destinationBefore = snapshotWideCharBuffer(
          destination,
          destinationCapacity,
        );
        const trailingGuardBefore =
          trailingGuard === null
            ? null
            : snapshotBoundedRawBytes(trailingGuard, guardBytes);
        lcMapStringOracleSentinel = sentinel;
        lcMapStringOracleResult = null;
        lcMapStringOracleLastError = null;
        try {
          const outcome = lcMapStringW(
            locale,
            flags,
            source,
            sourceCount,
            destination,
            destinationCapacity,
          );
          const result = Number(outcome.value) | 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            lcMapStringOracleResult === null ||
            lcMapStringOracleLastError === null
          ) {
            throw new Error(`${id} did not pass through the mapping hook`);
          }
          return {
            id,
            sentinel,
            locale,
            flags,
            sourceWords,
            sourceCount,
            destinationCapacity,
            nullDestinationWithCapacity,
            result,
            hookResult: lcMapStringOracleResult,
            hookLastErrorAfter: lcMapStringOracleLastError,
            systemLastError,
            sourceBefore,
            sourceAfter: snapshotWideCharBuffer(source, sourceWords.length),
            leadingGuardBefore,
            leadingGuardAfter:
              storage === null
                ? null
                : snapshotBoundedRawBytes(storage, guardBytes),
            destinationBefore,
            destinationAfter: snapshotWideCharBuffer(
              destination,
              destinationCapacity,
            ),
            trailingGuardBefore,
            trailingGuardAfter:
              trailingGuard === null
                ? null
                : snapshotBoundedRawBytes(trailingGuard, guardBytes),
          };
        } finally {
          lcMapStringOracleSentinel = null;
          lcMapStringOracleResult = null;
          lcMapStringOracleLastError = null;
        }
      };

      const representativeWords = [
        0x0041,
        0x005a,
        0x0061,
        0x3042,
        0x30a2,
        0xff21,
        0xff41,
      ];
      const calls = [
        invoke({
          id: "explicit-lowercase-query",
          sentinel: 0,
          sourceWords: representativeWords,
        }),
        invoke({
          id: "explicit-lowercase-conversion",
          sentinel: 87,
          sourceWords: representativeWords,
          destinationCapacity: representativeWords.length,
        }),
        invoke({
          id: "nul-terminated-lowercase",
          sentinel: 88,
          sourceWords: [...representativeWords, 0],
          sourceCount: -1,
          destinationCapacity: representativeWords.length + 1,
        }),
        invoke({
          id: "insufficient-destination",
          sentinel: 89,
          sourceWords: representativeWords,
          destinationCapacity: 2,
        }),
        invoke({
          id: "zero-source-count",
          sentinel: 90,
          sourceWords: [0x0041],
          sourceCount: 0,
          destinationCapacity: 1,
        }),
        invoke({
          id: "negative-two-source-count",
          sentinel: 91,
          sourceWords: [0x0041, 0],
          sourceCount: -2,
          destinationCapacity: 2,
        }),
        invoke({
          id: "invalid-flags",
          sentinel: 92,
          sourceWords: [0x0041],
          flags: 0,
        }),
        invoke({
          id: "null-destination-with-capacity",
          sentinel: 93,
          sourceWords: [0x0041],
          destinationCapacity: 1,
          nullDestinationWithCapacity: true,
        }),
      ];
      emit("lc-map-string-w-oracle", {
        profile: "x86-ja-jp-lowercase-v1",
        calls,
      });
    } catch (error) {
      emit("lc-map-string-w-oracle-error", { error: error.message });
    }
  });
}

function scheduleInterlockedDecrementOracle(module) {
  if (interlockedDecrementOracleScheduled) {
    return;
  }
  interlockedDecrementOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "InterlockedDecrement");
      if (address === null) {
        throw new Error("InterlockedDecrement export is unavailable");
      }
      const interlockedDecrement = new SystemFunction(
        address,
        "int",
        ["pointer"],
        "stdcall",
      );
      const invoke = (id, initialValue, sentinel) => {
        const guardBytes = 16;
        const storage = Memory.alloc(guardBytes + 4 + guardBytes);
        storage.writeByteArray(Array(guardBytes + 4 + guardBytes).fill(0xa5));
        const target = storage.add(guardBytes);
        target.writeS32(initialValue);
        const targetBefore = snapshotLong(target);
        const leadingGuardBefore = snapshotBoundedRawBytes(storage, guardBytes);
        const trailingGuard = target.add(4);
        const trailingGuardBefore = snapshotBoundedRawBytes(
          trailingGuard,
          guardBytes,
        );
        interlockedDecrementOracleSentinel = sentinel;
        interlockedDecrementOracleResult = null;
        interlockedDecrementOracleLastError = null;
        try {
          const outcome = interlockedDecrement(target);
          const result = Number(outcome.value) | 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            interlockedDecrementOracleResult === null ||
            interlockedDecrementOracleLastError === null
          ) {
            throw new Error(`${id} did not pass through the decrement hook`);
          }
          return {
            id,
            sentinel,
            initialValue,
            targetAlignment: target.toUInt32() % 4,
            targetBefore,
            targetAfter: snapshotLong(target),
            result,
            hookResult: interlockedDecrementOracleResult,
            hookLastErrorAfter: interlockedDecrementOracleLastError,
            systemLastError,
            leadingGuardBefore,
            leadingGuardAfter: snapshotBoundedRawBytes(storage, guardBytes),
            trailingGuardBefore,
            trailingGuardAfter: snapshotBoundedRawBytes(
              trailingGuard,
              guardBytes,
            ),
          };
        } finally {
          interlockedDecrementOracleSentinel = null;
          interlockedDecrementOracleResult = null;
          interlockedDecrementOracleLastError = null;
        }
      };
      emit("interlocked-decrement-oracle", {
        profile: "x86-aligned-wrapping-v1",
        calls: [
          invoke("one-to-zero", 1, 0),
          invoke("zero-to-minus-one", 0, 87),
          invoke("minimum-to-maximum", -0x80000000, 0x12345678),
        ],
      });
    } catch (error) {
      emit("interlocked-decrement-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetModuleFileNameAOracle(module) {
  if (getModuleFileNameAOracleScheduled) {
    return;
  }
  getModuleFileNameAOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetModuleFileNameA");
      if (address === null) {
        throw new Error("GetModuleFileNameA export is unavailable");
      }
      const getModuleFileNameA = new SystemFunction(
        address,
        "uint32",
        ["pointer", "pointer", "uint32"],
        "stdcall",
      );
      const invoke = ({
        id,
        sentinel,
        moduleHandle = ptr(0),
        capacity,
        nullOutput = false,
      }) => {
        const guardBytes = 16;
        const storage = nullOutput
          ? null
          : Memory.alloc(guardBytes + capacity + guardBytes);
        if (storage !== null) {
          storage.writeByteArray(Array(guardBytes + capacity + guardBytes).fill(0xa5));
        }
        const output = storage === null ? ptr(0) : storage.add(guardBytes);
        const trailingGuard =
          storage === null ? null : output.add(capacity);
        const leadingGuardBefore =
          storage === null ? null : snapshotBoundedRawBytes(storage, guardBytes);
        const outputBefore = snapshotAnsiOutputBuffer(output, capacity);
        const trailingGuardBefore =
          trailingGuard === null
            ? null
            : snapshotBoundedRawBytes(trailingGuard, guardBytes);
        getModuleFileNameAOracleSentinel = sentinel;
        getModuleFileNameAOracleResult = null;
        getModuleFileNameAOracleLastError = null;
        try {
          const outcome = getModuleFileNameA(moduleHandle, output, capacity);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getModuleFileNameAOracleResult === null ||
            getModuleFileNameAOracleLastError === null
          ) {
            throw new Error(`${id} did not pass through the module-file-name hook`);
          }
          return {
            id,
            sentinel,
            moduleHandle: moduleHandle.toString(),
            capacity,
            nullOutput,
            result,
            hookResult: getModuleFileNameAOracleResult,
            hookLastErrorAfter: getModuleFileNameAOracleLastError,
            systemLastError,
            leadingGuardBefore,
            leadingGuardAfter:
              storage === null
                ? null
                : snapshotBoundedRawBytes(storage, guardBytes),
            outputBefore,
            outputAfter: snapshotAnsiOutputBuffer(output, capacity),
            trailingGuardBefore,
            trailingGuardAfter:
              trailingGuard === null
                ? null
                : snapshotBoundedRawBytes(trailingGuard, guardBytes),
          };
        } finally {
          getModuleFileNameAOracleSentinel = null;
          getModuleFileNameAOracleResult = null;
          getModuleFileNameAOracleLastError = null;
        }
      };

      const generous = invoke({
        id: "main-module-generous",
        sentinel: 0,
        capacity: 1024,
      });
      if (generous.result === 0 || generous.result >= 1024) {
        throw new Error("main-module generous query did not return a bounded path");
      }
      const calls = [
        generous,
        invoke({
          id: "main-module-exact",
          sentinel: 87,
          capacity: generous.result + 1,
        }),
        invoke({
          id: "main-module-insufficient",
          sentinel: 0x12345678,
          capacity: generous.result,
        }),
        invoke({
          id: "main-module-one-byte",
          sentinel: 88,
          capacity: 1,
        }),
        invoke({
          id: "main-module-zero-capacity",
          sentinel: 89,
          capacity: 0,
        }),
        invoke({
          id: "main-module-null-zero-capacity",
          sentinel: 90,
          capacity: 0,
          nullOutput: true,
        }),
        invoke({
          id: "explicit-kernel32-module",
          sentinel: 91,
          moduleHandle: module.base,
          capacity: 1024,
        }),
        invoke({
          id: "invalid-module-handle",
          sentinel: 92,
          moduleHandle: ptr(1),
          capacity: 1024,
        }),
      ];
      emit("get-module-file-name-a-oracle", {
        profile: "x86-main-and-explicit-module-v1",
        mainModule: {
          name: Process.mainModule.name,
          base: Process.mainModule.base.toString(),
          path: Process.mainModule.path,
        },
        calls,
      });
    } catch (error) {
      emit("get-module-file-name-a-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetLastErrorOracle(module) {
  if (getLastErrorOracleScheduled) {
    return;
  }
  getLastErrorOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetLastError");
      if (address === null) {
        throw new Error("GetLastError export is unavailable");
      }
      const getLastError = new NativeFunction(
        address,
        "uint32",
        [],
        "stdcall",
      );
      const invoke = (sentinel) => {
        getLastErrorOracleSentinel = sentinel;
        getLastErrorOracleResult = null;
        getLastErrorOracleLastError = null;
        try {
          const result = Number(getLastError()) >>> 0;
          if (
            getLastErrorOracleResult === null ||
            getLastErrorOracleLastError === null
          ) {
            throw new Error("GetLastError oracle did not pass through the hook");
          }
          return {
            sentinel,
            result,
            hookResult: getLastErrorOracleResult,
            lastErrorAfter: getLastErrorOracleLastError,
            resultMatchesSentinel: result === sentinel,
            hookResultMatchesSentinel: getLastErrorOracleResult === sentinel,
            preservesLastError: getLastErrorOracleLastError === sentinel,
          };
        } finally {
          getLastErrorOracleSentinel = null;
          getLastErrorOracleResult = null;
          getLastErrorOracleLastError = null;
        }
      };
      const calls = [0, 87, 122, 0x12345678].map(invoke);
      emit("get-last-error-oracle", {
        profile: "x86-thread-last-error-read-v1",
        calls,
        allResultsMatch: calls.every(
          (call) =>
            call.resultMatchesSentinel &&
            call.hookResultMatchesSentinel &&
            call.preservesLastError,
        ),
      });
    } catch (error) {
      emit("get-last-error-oracle-error", { error: error.message });
    }
  });
}

function scheduleSetLastErrorOracle(module) {
  if (setLastErrorOracleScheduled) {
    return;
  }
  setLastErrorOracleScheduled = true;
  setImmediate(() => {
    try {
      const setAddress = findExport(module, "SetLastError");
      if (setAddress === null) {
        throw new Error("SetLastError export is unavailable");
      }
      const setLastError = new SystemFunction(
        setAddress,
        "void",
        ["uint32"],
        "stdcall",
      );
      const invoke = (sentinel) => {
        setLastErrorOracleActive = true;
        setLastErrorOracleArgument = null;
        setLastErrorOracleEaxBefore = null;
        setLastErrorOracleEaxAfter = null;
        setLastErrorOracleLastErrorAfter = null;
        try {
          const outcome = setLastError(sentinel);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            setLastErrorOracleArgument === null ||
            setLastErrorOracleEaxBefore === null ||
            setLastErrorOracleEaxAfter === null ||
            setLastErrorOracleLastErrorAfter === null
          ) {
            throw new Error("SetLastError oracle did not pass through the hook");
          }
          return {
            sentinel,
            hookArgument: setLastErrorOracleArgument,
            eaxBefore: setLastErrorOracleEaxBefore,
            eaxAfter: setLastErrorOracleEaxAfter,
            preservesEax: setLastErrorOracleEaxAfter === setLastErrorOracleEaxBefore,
            hookLastErrorAfter: setLastErrorOracleLastErrorAfter,
            systemLastError,
            argumentMatches: setLastErrorOracleArgument === sentinel,
            hookLastErrorMatches: setLastErrorOracleLastErrorAfter === sentinel,
            systemLastErrorMatches: systemLastError === sentinel,
          };
        } finally {
          setLastErrorOracleActive = false;
          setLastErrorOracleArgument = null;
          setLastErrorOracleEaxBefore = null;
          setLastErrorOracleEaxAfter = null;
          setLastErrorOracleLastErrorAfter = null;
        }
      };
      const calls = [0, 87, 122, 0x12345678].map(invoke);
      emit("set-last-error-oracle", {
        profile: "x86-thread-last-error-write-v1",
        calls,
        allStateEffectsMatch: calls.every(
          (call) =>
            call.argumentMatches &&
            call.hookLastErrorMatches &&
            call.systemLastErrorMatches,
        ),
      });
    } catch (error) {
      emit("set-last-error-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetAcpOracle(module) {
  if (getAcpOracleScheduled) {
    return;
  }
  getAcpOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetACP");
      if (address === null) {
        throw new Error("GetACP export is unavailable");
      }
      const getAcp = new SystemFunction(address, "uint32", [], "stdcall");
      const invoke = (sentinel) => {
        getAcpOracleSentinel = sentinel;
        getAcpOracleResult = null;
        getAcpOracleLastError = null;
        try {
          const outcome = getAcp();
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (getAcpOracleResult === null || getAcpOracleLastError === null) {
            throw new Error("GetACP oracle did not pass through the hook");
          }
          return {
            sentinel,
            result,
            hookResult: getAcpOracleResult,
            hookLastErrorAfter: getAcpOracleLastError,
            systemLastError,
            hookResultMatches: getAcpOracleResult === result,
            hookPreservesLastError: getAcpOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          getAcpOracleSentinel = null;
          getAcpOracleResult = null;
          getAcpOracleLastError = null;
        }
      };
      const calls = [0, 87, 122, 0x12345678].map(invoke);
      const activeCodePage = calls[0].result;
      emit("get-acp-oracle", {
        profile: "x86-active-code-page-read-v1",
        activeCodePage,
        calls,
        allResultsStable: calls.every(
          (call) => call.result === activeCodePage && call.hookResultMatches,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("get-acp-oracle-error", { error: error.message });
    }
  });
}

function scheduleSystemDefaultLangIdOracle(module) {
  if (systemDefaultLangIdOracleScheduled) {
    return;
  }
  systemDefaultLangIdOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetSystemDefaultLangID");
      if (address === null) {
        throw new Error("GetSystemDefaultLangID export is unavailable");
      }
      const getSystemDefaultLangId = new SystemFunction(
        address,
        "uint16",
        [],
        "stdcall",
      );
      const invoke = (sentinel) => {
        systemDefaultLangIdOracleSentinel = sentinel;
        systemDefaultLangIdOracleResult = null;
        systemDefaultLangIdOracleLastError = null;
        try {
          const outcome = getSystemDefaultLangId();
          const result = Number(outcome.value) & 0xffff;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            systemDefaultLangIdOracleResult === null ||
            systemDefaultLangIdOracleLastError === null
          ) {
            throw new Error(
              "GetSystemDefaultLangID oracle did not pass through the hook",
            );
          }
          return {
            sentinel,
            result,
            hookRawEax: systemDefaultLangIdOracleResult,
            hookLangId: systemDefaultLangIdOracleResult & 0xffff,
            hookLastErrorAfter: systemDefaultLangIdOracleLastError,
            systemLastError,
            hookResultMatches:
              (systemDefaultLangIdOracleResult & 0xffff) === result,
            hookPreservesLastError:
              systemDefaultLangIdOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          systemDefaultLangIdOracleSentinel = null;
          systemDefaultLangIdOracleResult = null;
          systemDefaultLangIdOracleLastError = null;
        }
      };
      const calls = [0, 87, 122, 0x12345678].map(invoke);
      const langId = calls[0].result;
      emit("system-default-lang-id-oracle", {
        profile: "x86-system-default-lang-id-read-v1",
        langId,
        calls,
        rawEaxValues: calls.map((call) => call.hookRawEax),
        upperEaxBitsContract: "unspecified",
        allResultsStable: calls.every(
          (call) => call.result === langId && call.hookResultMatches,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("system-default-lang-id-oracle-error", { error: error.message });
    }
  });
}

function scheduleDriveTypeOracle(module) {
  if (driveTypeOracleScheduled) {
    return;
  }
  driveTypeOracleScheduled = true;
  setImmediate(() => {
    try {
      const driveTypeAddress = findExport(module, "GetDriveTypeA");
      const currentDirectoryAddress = findExport(module, "GetCurrentDirectoryA");
      if (driveTypeAddress === null || currentDirectoryAddress === null) {
        throw new Error("drive-type oracle exports are unavailable");
      }
      const getDriveType = new SystemFunction(
        driveTypeAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const getCurrentDirectory = new SystemFunction(
        currentDirectoryAddress,
        "uint32",
        ["uint32", "pointer"],
        "stdcall",
      );
      const currentDirectoryBuffer = Memory.alloc(32768);
      const currentDirectoryOutcome = getCurrentDirectory(
        32768,
        currentDirectoryBuffer,
      );
      const currentDirectoryLength =
        Number(currentDirectoryOutcome.value) >>> 0;
      if (currentDirectoryLength === 0 || currentDirectoryLength >= 32768) {
        throw new Error("GetCurrentDirectoryA did not return a bounded path");
      }
      const currentDirectory = currentDirectoryBuffer.readAnsiString();
      if (
        currentDirectory.length < 3 ||
        currentDirectory[1] !== ":" ||
        currentDirectory[2] !== "\\"
      ) {
        throw new Error(
          "drive-type oracle requires a drive-letter current directory",
        );
      }
      const currentRoot = currentDirectory.slice(0, 2) + "\\";
      const currentRootPointer = Memory.allocUtf8String(currentRoot);
      const currentDirectoryPointer = currentDirectoryBuffer;
      const relativePointer = Memory.allocUtf8String(".");
      const invoke = ({ id, sentinel, rootPath, pointer }) => {
        driveTypeOracleSentinel = sentinel;
        driveTypeOracleResult = null;
        driveTypeOracleLastError = null;
        try {
          const outcome = getDriveType(pointer);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            driveTypeOracleResult === null ||
            driveTypeOracleLastError === null
          ) {
            throw new Error("GetDriveTypeA oracle did not pass through the hook");
          }
          return {
            id,
            sentinel,
            rootPath,
            result,
            hookResult: driveTypeOracleResult,
            hookLastErrorAfter: driveTypeOracleLastError,
            systemLastError,
            hookResultMatches: driveTypeOracleResult === result,
            hookPreservesLastError: driveTypeOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          driveTypeOracleSentinel = null;
          driveTypeOracleResult = null;
          driveTypeOracleLastError = null;
        }
      };
      const calls = [
        invoke({
          id: "current-root",
          sentinel: 0,
          rootPath: currentRoot,
          pointer: currentRootPointer,
        }),
        invoke({
          id: "null-current-drive",
          sentinel: 87,
          rootPath: null,
          pointer: NULL,
        }),
        invoke({
          id: "current-directory-not-root",
          sentinel: 122,
          rootPath: currentDirectory,
          pointer: currentDirectoryPointer,
        }),
        invoke({
          id: "relative-dot",
          sentinel: 0x12345678,
          rootPath: ".",
          pointer: relativePointer,
        }),
      ];
      emit("drive-type-oracle", {
        profile: "x86-current-drive-type-read-v1",
        currentDirectory,
        currentRoot,
        calls,
      });
    } catch (error) {
      emit("drive-type-oracle-error", { error: error.message });
    }
  });
}

function scheduleSwapMouseButtonStateObserver(module) {
  if (swapMouseButtonStateScheduled) {
    return;
  }
  swapMouseButtonStateScheduled = true;
  setImmediate(() => {
    try {
      const swapAddress = findExport(module, "SwapMouseButton");
      const metricsAddress = findExport(module, "GetSystemMetrics");
      if (swapAddress === null || metricsAddress === null) {
        throw new Error("SwapMouseButton state-observer exports are unavailable");
      }
      swapMouseButtonGetSystemMetrics = new NativeFunction(
        metricsAddress,
        "int32",
        ["int32"],
        "stdcall",
      );
      swapMouseButtonRestore = new SystemFunction(
        swapAddress,
        "uint32",
        ["uint32"],
        "stdcall",
      );
      swapMouseButtonInitialState = swapMouseButtonGetSystemMetrics(23) !== 0;
      emit("swap-mouse-button-initial-state", {
        profile: "x86-process-observed-system-button-mapping-v1",
        swapped: swapMouseButtonInitialState,
      });
    } catch (error) {
      emit("swap-mouse-button-state-observer-error", { error: error.message });
    }
  });
}

function completeSwapMouseButtonObservation(calls) {
  try {
    if (
      swapMouseButtonInitialState === null ||
      swapMouseButtonGetSystemMetrics === null ||
      swapMouseButtonRestore === null
    ) {
      throw new Error("SwapMouseButton initial state was not captured before the BGI call");
    }
    if (calls.length !== 2) {
      throw new Error("SwapMouseButton completion requires the two BGI calls");
    }
    const stateAfter = calls[1].stateAfter;
    let restoration = null;
    if (stateAfter !== swapMouseButtonInitialState) {
      swapMouseButtonRestoreActive = true;
      try {
        const outcome = swapMouseButtonRestore(
          swapMouseButtonInitialState ? 1 : 0,
        );
        restoration = {
          requestedState: swapMouseButtonInitialState,
          result: Number(outcome.value) >>> 0,
          lastError: Number(outcome.lastError) >>> 0,
        };
      } finally {
        swapMouseButtonRestoreActive = false;
      }
    }
    const finalState = swapMouseButtonGetSystemMetrics(23) !== 0;
    emit("swap-mouse-button-complete", {
      profile: "x86-bgi-normal-button-mapping-v1",
      initialState: swapMouseButtonInitialState,
      calls,
      firstResultRestoredBySecondArgument:
        calls[1].argument === calls[0].result,
      restoration,
      finalState,
      restored: finalState === swapMouseButtonInitialState,
    });
    if (stopOnEvent === "swap-mouse-button-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  } catch (error) {
    emit("swap-mouse-button-completion-error", { error: error.message, calls });
  }
}

function maybeCompleteTimeGetDevCapsObservation() {
  if (
    timeGetDevCapsCompletionRecorded ||
    !timeGetDevCapsOracleComplete ||
    timeGetDevCapsMainModuleCalls.length === 0 ||
    !timePeriodMainModuleCalls.some(
      (call) => call.api === "timeBeginPeriod",
    )
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteTimeGetDevCapsObservation, 10);
    return;
  }
  if (timeGetDevCapsDrainCheckScheduled) {
    return;
  }
  timeGetDevCapsDrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    timeGetDevCapsDrainCheckScheduled = false;
    if (timeGetDevCapsCompletionRecorded) {
      return;
    }
    if (
      activeCallsByThread.size !== 0 ||
      nextCallId !== expectedNextCallId
    ) {
      maybeCompleteTimeGetDevCapsObservation();
      return;
    }
    timeGetDevCapsCompletionRecorded = true;
    emit("time-get-dev-caps-complete", {
      profile: "x86-bgi-time-get-dev-caps-v1",
      oracleProfile: timeGetDevCapsOracleEvidence.profile,
      periodOracleProfile: timePeriodOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: timeGetDevCapsMainModuleCalls.map((call) => ({
        ...call,
      })),
      timePeriodCalls: timePeriodMainModuleCalls.map((call) => ({
        ...call,
      })),
    });
    if (stopOnEvent === "time-get-dev-caps-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleTimeGetDevCapsOracle(module) {
  if (timeGetDevCapsOracleScheduled) {
    return;
  }
  timeGetDevCapsOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "timeGetDevCaps");
      if (address === null) {
        throw new Error("timeGetDevCaps export is unavailable");
      }
      const timeGetDevCaps = new SystemFunction(
        address,
        "uint32",
        ["pointer", "uint32"],
        "stdcall",
      );
      const invoke = ({ size, sentinel }) => {
        const guardBytes = 16;
        const outputCapacity = 16;
        const storageBytes = guardBytes + outputCapacity + guardBytes;
        const storage = Memory.alloc(storageBytes);
        storage.writeByteArray(Array(storageBytes).fill(0xa5));
        const output = storage.add(guardBytes);
        const storageBefore = snapshotBoundedRawBytes(storage, storageBytes);
        timeGetDevCapsOracleCall = {
          size,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = timeGetDevCaps(output, size);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            timeGetDevCapsOracleCall.hookResult === null ||
            timeGetDevCapsOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error("timeGetDevCaps oracle did not pass through the hook");
          }
          const storageAfter = snapshotBoundedRawBytes(storage, storageBytes);
          const beforeBytes = storageBefore.bytes ?? [];
          const afterBytes = storageAfter.bytes ?? [];
          const changedStorageOffsets =
            beforeBytes.length === storageBytes &&
            afterBytes.length === storageBytes
              ? beforeBytes.flatMap((value, index) =>
                  value === afterBytes[index] ? [] : [index],
                )
              : [];
          const changedOutputOffsets = changedStorageOffsets
            .filter(
              (offset) =>
                offset >= guardBytes &&
                offset < guardBytes + outputCapacity,
            )
            .map((offset) => offset - guardBytes);
          return {
            size,
            sentinel,
            hookResult: timeGetDevCapsOracleCall.hookResult,
            systemResult,
            hookLastErrorAfter:
              timeGetDevCapsOracleCall.hookLastErrorAfter,
            systemLastError,
            storageOffset: guardBytes,
            outputCapacity,
            storageBefore,
            storageAfter,
            outputAfter: snapshotTimeCaps(output),
            outputBytesAfter: snapshotBoundedRawBytes(output, outputCapacity),
            changedOutputOffsets,
            prefixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes
                .slice(0, guardBytes)
                .every((value) => value === 0xa5),
            suffixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes
                .slice(guardBytes + outputCapacity)
                .every((value) => value === 0xa5),
            bytesBeyondTimeCapsUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes
                .slice(guardBytes + 8, guardBytes + outputCapacity)
                .every((value) => value === 0xa5),
            hookResultMatches:
              timeGetDevCapsOracleCall.hookResult === systemResult,
            hookPreservesLastError:
              timeGetDevCapsOracleCall.hookLastErrorAfter === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          timeGetDevCapsOracleCall = null;
        }
      };
      const calls = [
        { size: 0, sentinel: 0x10203040 },
        { size: 4, sentinel: 0x11223344 },
        { size: 7, sentinel: 0x55667788 },
        { size: 8, sentinel: 0 },
        { size: 8, sentinel: 87 },
        { size: 8, sentinel: 0x12345678 },
        { size: 12, sentinel: 0x76543210 },
      ].map(invoke);
      const successfulCalls = calls.filter((call) => call.size >= 8);
      const referenceFields = successfulCalls[0]?.outputAfter.fields ?? null;
      if (referenceFields === null) {
        throw new Error("timeGetDevCaps oracle did not produce timer capabilities");
      }
      const beginAddress = findExport(module, "timeBeginPeriod");
      const endAddress = findExport(module, "timeEndPeriod");
      if (beginAddress === null || endAddress === null) {
        throw new Error("time period exports are unavailable");
      }
      const periodFunctions = {
        timeBeginPeriod: new SystemFunction(
          beginAddress,
          "uint32",
          ["uint32"],
          "stdcall",
        ),
        timeEndPeriod: new SystemFunction(
          endAddress,
          "uint32",
          ["uint32"],
          "stdcall",
        ),
      };
      const invokePeriod = ({ api, periodMilliseconds, sentinel }) => {
        timePeriodOracleCall = {
          api,
          periodMilliseconds,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = periodFunctions[api](periodMilliseconds);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            timePeriodOracleCall.hookResult === null ||
            timePeriodOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(`${api} oracle did not pass through the hook`);
          }
          return {
            api,
            periodMilliseconds,
            sentinel,
            hookResult: timePeriodOracleCall.hookResult,
            systemResult,
            hookLastErrorAfter: timePeriodOracleCall.hookLastErrorAfter,
            systemLastError,
            hookResultMatches: timePeriodOracleCall.hookResult === systemResult,
            hookPreservesLastError:
              timePeriodOracleCall.hookLastErrorAfter === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          timePeriodOracleCall = null;
        }
      };
      const minimum = referenceFields.periodMinimumMilliseconds;
      const maximum = referenceFields.periodMaximumMilliseconds;
      const differentValid = minimum < maximum ? minimum + 1 : minimum;
      const aboveMaximum = maximum === 0xffff_ffff ? maximum : maximum + 1;
      const periodCalls = [
        { api: "timeEndPeriod", periodMilliseconds: minimum, sentinel: 0x01020304 },
        { api: "timeBeginPeriod", periodMilliseconds: 0, sentinel: 0x11121314 },
        { api: "timeEndPeriod", periodMilliseconds: 0, sentinel: 0x21222324 },
        {
          api: "timeBeginPeriod",
          periodMilliseconds: aboveMaximum,
          sentinel: 0x31323334,
        },
        {
          api: "timeEndPeriod",
          periodMilliseconds: aboveMaximum,
          sentinel: 0x41424344,
        },
        { api: "timeBeginPeriod", periodMilliseconds: differentValid, sentinel: 0x45464748 },
        { api: "timeEndPeriod", periodMilliseconds: differentValid, sentinel: 0x494a4b4c },
        { api: "timeBeginPeriod", periodMilliseconds: 0xffff_ffff, sentinel: 0x4d4e4f50 },
        { api: "timeEndPeriod", periodMilliseconds: 0xffff_ffff, sentinel: 0x55565758 },
        { api: "timeBeginPeriod", periodMilliseconds: minimum, sentinel: 0x51525354 },
        { api: "timeBeginPeriod", periodMilliseconds: minimum, sentinel: 0x61626364 },
        {
          api: "timeEndPeriod",
          periodMilliseconds: differentValid,
          sentinel: 0x71727374,
        },
        { api: "timeEndPeriod", periodMilliseconds: minimum, sentinel: 0x81828384 },
        { api: "timeEndPeriod", periodMilliseconds: minimum, sentinel: 0x91929394 },
        { api: "timeEndPeriod", periodMilliseconds: minimum, sentinel: 0xa1a2a3a4 },
        { api: "timeBeginPeriod", periodMilliseconds: maximum, sentinel: 0xb1b2b3b4 },
        { api: "timeEndPeriod", periodMilliseconds: maximum, sentinel: 0xc1c2c3c4 },
      ].map(invokePeriod);
      timePeriodOracleEvidence = {
        profile: "x86-time-period-range-and-pairing-v1",
        periodMinimumMilliseconds: minimum,
        periodMaximumMilliseconds: maximum,
        differentValidPeriodMilliseconds: differentValid,
        aboveMaximumPeriodMilliseconds: aboveMaximum,
        calls: periodCalls,
        allResultsMatchHooks: periodCalls.every((call) => call.hookResultMatches),
        allPreserveLastError: periodCalls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      };
      timeGetDevCapsOracleEvidence = {
        profile: "x86-time-get-dev-caps-size-and-output-v1",
        calls,
        timeErrorNoError: 0,
        timeErrorNoCanDo: 97,
        shortSizesRejectedWithoutWrites: calls
          .filter((call) => call.size < 8)
          .every(
            (call) =>
              call.systemResult === 97 &&
              call.hookResultMatches &&
              call.changedOutputOffsets.length === 0,
          ),
        supportedSizesSucceeded: successfulCalls.every(
          (call) => call.systemResult === 0 && call.hookResultMatches,
        ),
        successfulOutputsStable: successfulCalls.every(
          (call) =>
            JSON.stringify(call.outputAfter.fields ?? null) ===
            JSON.stringify(referenceFields),
        ),
        allWritesBoundedToTimeCaps: successfulCalls.every(
          (call) =>
            call.prefixGuardUnchanged &&
            call.suffixGuardUnchanged &&
            call.bytesBeyondTimeCapsUnchanged &&
            call.changedOutputOffsets.every((offset) => offset < 8),
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
        periodLifecycle: timePeriodOracleEvidence,
      };
      timeGetDevCapsOracleComplete = true;
      emit("time-get-dev-caps-oracle", timeGetDevCapsOracleEvidence);
      maybeCompleteTimeGetDevCapsObservation();
    } catch (error) {
      emit("time-get-dev-caps-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteCreateWaitableTimerAObservation() {
  if (
    createWaitableTimerACompletionRecorded ||
    !createWaitableTimerAOracleComplete ||
    createWaitableTimerAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteCreateWaitableTimerAObservation, 10);
    return;
  }
  if (createWaitableTimerADrainCheckScheduled) {
    return;
  }
  createWaitableTimerADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    createWaitableTimerADrainCheckScheduled = false;
    if (createWaitableTimerACompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteCreateWaitableTimerAObservation();
      return;
    }
    createWaitableTimerACompletionRecorded = true;
    emit("create-waitable-timer-a-complete", {
      profile: "x86-bgi-create-waitable-timer-a-v1",
      oracleProfile: createWaitableTimerAOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: createWaitableTimerAMainModuleCalls.map((call) => ({
        ...call,
      })),
    });
    if (stopOnEvent === "create-waitable-timer-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleCreateWaitableTimerAOracle(module) {
  if (createWaitableTimerAOracleScheduled) {
    return;
  }
  createWaitableTimerAOracleScheduled = true;
  setImmediate(() => {
    try {
      const createAddress = findExport(module, "CreateWaitableTimerA");
      const waitAddress = findExport(module, "WaitForSingleObject");
      const closeAddress = findExport(module, "CloseHandle");
      if (createAddress === null || waitAddress === null || closeAddress === null) {
        throw new Error("waitable timer oracle exports are unavailable");
      }
      const createWaitableTimerA = new SystemFunction(
        createAddress,
        "pointer",
        ["pointer", "uint32", "pointer"],
        "stdcall",
      );
      const waitForSingleObject = new SystemFunction(
        waitAddress,
        "uint32",
        ["pointer", "uint32"],
        "stdcall",
      );
      const closeHandle = new SystemFunction(
        closeAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const invoke = ({ manualReset, sentinel }) => {
        createWaitableTimerAOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const createOutcome = createWaitableTimerA(
            ptr(0),
            manualReset ? 1 : 0,
            ptr(0),
          );
          const handle = createOutcome.value;
          const systemLastError = Number(createOutcome.lastError) >>> 0;
          if (
            createWaitableTimerAOracleCall.hookResult === null ||
            createWaitableTimerAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              "CreateWaitableTimerA oracle did not pass through the hook",
            );
          }
          if (handle.isNull()) {
            throw new Error("CreateWaitableTimerA oracle returned null");
          }
          const waitOutcome = waitForSingleObject(handle, 0);
          const closeOutcome = closeHandle(handle);
          return {
            manualReset,
            sentinel,
            handleNonNull: true,
            hookResult: createWaitableTimerAOracleCall.hookResult,
            systemResult: handle.toString(),
            hookLastErrorAfter:
              createWaitableTimerAOracleCall.hookLastErrorAfter,
            systemLastError,
            initialWaitResult: Number(waitOutcome.value) >>> 0,
            closeResult: Number(closeOutcome.value) >>> 0,
            hookResultMatches:
              createWaitableTimerAOracleCall.hookResult === handle.toString(),
            hookAndSystemLastErrorMatch:
              createWaitableTimerAOracleCall.hookLastErrorAfter ===
              systemLastError,
          };
        } finally {
          createWaitableTimerAOracleCall = null;
        }
      };
      const calls = [
        { manualReset: true, sentinel: 0x11223344 },
        { manualReset: false, sentinel: 0x55667788 },
      ].map(invoke);
      createWaitableTimerAOracleEvidence = {
        profile: "x86-create-waitable-timer-initial-state-v1",
        waitObject0: 0,
        waitTimeout: 258,
        calls,
        allCreated: calls.every((call) => call.handleNonNull),
        allInitiallyNonsignaled: calls.every(
          (call) => call.initialWaitResult === 258,
        ),
        allClosed: calls.every((call) => call.closeResult !== 0),
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      createWaitableTimerAOracleComplete = true;
      emit(
        "create-waitable-timer-a-oracle",
        createWaitableTimerAOracleEvidence,
      );
      maybeCompleteCreateWaitableTimerAObservation();
    } catch (error) {
      emit("create-waitable-timer-a-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteLoadIconAObservation() {
  if (
    loadIconACompletionRecorded ||
    !loadIconAOracleComplete ||
    loadIconAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteLoadIconAObservation, 10);
    return;
  }
  if (loadIconADrainCheckScheduled) {
    return;
  }
  loadIconADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    loadIconADrainCheckScheduled = false;
    if (loadIconACompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteLoadIconAObservation();
      return;
    }
    loadIconACompletionRecorded = true;
    emit("load-icon-a-complete", {
      profile: "x86-bgi-load-icon-a-v1",
      oracleProfile: loadIconAOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: loadIconAMainModuleCalls.map((call) => ({ ...call })),
    });
    if (stopOnEvent === "load-icon-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleLoadIconAOracle(module) {
  if (loadIconAOracleScheduled) {
    return;
  }
  loadIconAOracleScheduled = true;
  setImmediate(() => {
    try {
      const loadIconAddress = findExport(module, "LoadIconA");
      const getIconInfoAddress = findExport(module, "GetIconInfo");
      const getSystemMetricsAddress = findExport(module, "GetSystemMetrics");
      const gdi32 = Process.getModuleByName("gdi32.dll");
      const getObjectAddress = findExport(gdi32, "GetObjectW");
      const deleteObjectAddress = findExport(gdi32, "DeleteObject");
      if (
        loadIconAddress === null ||
        getIconInfoAddress === null ||
        getSystemMetricsAddress === null ||
        getObjectAddress === null ||
        deleteObjectAddress === null
      ) {
        throw new Error("LoadIconA oracle exports are unavailable");
      }
      const loadIconA = new SystemFunction(
        loadIconAddress,
        "pointer",
        ["pointer", "pointer"],
        "stdcall",
      );
      const getIconInfo = new SystemFunction(
        getIconInfoAddress,
        "uint32",
        ["pointer", "pointer"],
        "stdcall",
      );
      const getSystemMetrics = new NativeFunction(
        getSystemMetricsAddress,
        "int32",
        ["int32"],
        "stdcall",
      );
      const getObjectW = new SystemFunction(
        getObjectAddress,
        "int32",
        ["pointer", "int32", "pointer"],
        "stdcall",
      );
      const deleteObject = new SystemFunction(
        deleteObjectAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );

      const snapshotBitmap = (handle) => {
        if (handle.isNull()) {
          return { handle: handle.toString(), isNull: true };
        }
        const storage = Memory.alloc(24);
        storage.writeByteArray(Array(24).fill(0xa5));
        const outcome = getObjectW(handle, 24, storage);
        const result = Number(outcome.value);
        const bytes = [...new Uint8Array(storage.readByteArray(24))];
        const view = new DataView(Uint8Array.from(bytes).buffer);
        return {
          handle: handle.toString(),
          isNull: false,
          result,
          lastError: Number(outcome.lastError) >>> 0,
          fields:
            result === 0
              ? null
              : {
                  type: view.getInt32(0, true),
                  width: view.getInt32(4, true),
                  height: view.getInt32(8, true),
                  widthBytes: view.getInt32(12, true),
                  planes: view.getUint16(16, true),
                  bitsPixel: view.getUint16(18, true),
                  bits: `0x${view.getUint32(20, true).toString(16)}`,
                },
        };
      };

      const snapshotIcon = (handle) => {
        const storage = Memory.alloc(20);
        storage.writeByteArray(Array(20).fill(0xa5));
        const outcome = getIconInfo(handle, storage);
        const succeeded = Number(outcome.value) !== 0;
        if (!succeeded) {
          return {
            result: Number(outcome.value) >>> 0,
            lastError: Number(outcome.lastError) >>> 0,
          };
        }
        const mask = storage.add(12).readPointer();
        const color = storage.add(16).readPointer();
        let maskSnapshot = null;
        let colorSnapshot = null;
        const deletions = [];
        try {
          maskSnapshot = snapshotBitmap(mask);
          colorSnapshot = snapshotBitmap(color);
        } finally {
          for (const [kind, bitmap] of [
            ["mask", mask],
            ["color", color],
          ]) {
            if (!bitmap.isNull()) {
              const deletion = deleteObject(bitmap);
              deletions.push({
                kind,
                result: Number(deletion.value) >>> 0,
                lastError: Number(deletion.lastError) >>> 0,
              });
            }
          }
        }
        return {
          result: Number(outcome.value) >>> 0,
          lastError: Number(outcome.lastError) >>> 0,
          fIcon: storage.readU32(),
          xHotspot: storage.add(4).readU32(),
          yHotspot: storage.add(8).readU32(),
          mask: maskSnapshot,
          color: colorSnapshot,
          deletions,
        };
      };

      const invoke = ({ id, resourceId, sentinel, inspect }) => {
        loadIconAOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = loadIconA(
            Process.mainModule.base,
            ptr(resourceId),
          );
          const handle = outcome.value;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            loadIconAOracleCall.hookResult === null ||
            loadIconAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error("LoadIconA oracle did not pass through the hook");
          }
          return {
            id,
            instance: Process.mainModule.base.toString(),
            resourceId,
            sentinel,
            hookResult: loadIconAOracleCall.hookResult,
            systemResult: handle.toString(),
            resultNonNull: !handle.isNull(),
            hookLastErrorAfter: loadIconAOracleCall.hookLastErrorAfter,
            systemLastError,
            hookResultMatches:
              loadIconAOracleCall.hookResult === handle.toString(),
            hookAndSystemLastErrorMatch:
              loadIconAOracleCall.hookLastErrorAfter === systemLastError,
            icon: inspect && !handle.isNull() ? snapshotIcon(handle) : null,
          };
        } finally {
          loadIconAOracleCall = null;
        }
      };
      const calls = [
        {
          id: "valid-first",
          resourceId: 101,
          sentinel: 0x11223344,
          inspect: true,
        },
        {
          id: "valid-repeated",
          resourceId: 101,
          sentinel: 0x55667788,
          inspect: false,
        },
        {
          id: "missing-resource",
          resourceId: 0xffff,
          sentinel: 0x7a6b5c4d,
          inspect: false,
        },
      ].map(invoke);
      const validCalls = calls.slice(0, 2);
      const missingCall = calls[2];
      const mainResult = loadIconAMainModuleCalls[0].result;
      loadIconAOracleEvidence = {
        profile: "x86-module-group-icon-shared-identity-v1",
        mainModule: {
          name: Process.mainModule.name,
          base: Process.mainModule.base.toString(),
        },
        systemMetrics: {
          iconWidth: getSystemMetrics(11),
          iconHeight: getSystemMetrics(12),
          smallIconWidth: getSystemMetrics(49),
          smallIconHeight: getSystemMetrics(50),
        },
        calls,
        validCallsSucceeded: validCalls.every((call) => call.resultNonNull),
        repeatedValidIdentityShared:
          validCalls[0].systemResult === validCalls[1].systemResult,
        validIdentityMatchesMainCall: validCalls.every(
          (call) => call.systemResult === mainResult,
        ),
        missingResourceRejected: !missingCall.resultNonNull,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      loadIconAOracleComplete = true;
      emit("load-icon-a-oracle", loadIconAOracleEvidence);
      maybeCompleteLoadIconAObservation();
    } catch (error) {
      emit("load-icon-a-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteLoadCursorAObservation() {
  if (
    loadCursorACompletionRecorded ||
    !loadCursorAOracleComplete ||
    loadCursorAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteLoadCursorAObservation, 10);
    return;
  }
  if (loadCursorADrainCheckScheduled) {
    return;
  }
  loadCursorADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    loadCursorADrainCheckScheduled = false;
    if (loadCursorACompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteLoadCursorAObservation();
      return;
    }
    loadCursorACompletionRecorded = true;
    emit("load-cursor-a-complete", {
      profile: "x86-bgi-load-cursor-a-v1",
      oracleProfile: loadCursorAOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: loadCursorAMainModuleCalls.map((call) => ({ ...call })),
    });
    if (stopOnEvent === "load-cursor-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleLoadCursorAOracle(module) {
  if (loadCursorAOracleScheduled) {
    return;
  }
  loadCursorAOracleScheduled = true;
  setImmediate(() => {
    try {
      const loadCursorAddress = findExport(module, "LoadCursorA");
      const getSystemMetricsAddress = findExport(module, "GetSystemMetrics");
      if (loadCursorAddress === null || getSystemMetricsAddress === null) {
        throw new Error("LoadCursorA oracle exports are unavailable");
      }
      const loadCursorA = new SystemFunction(
        loadCursorAddress,
        "pointer",
        ["pointer", "pointer"],
        "stdcall",
      );
      const getSystemMetrics = new NativeFunction(
        getSystemMetricsAddress,
        "int32",
        ["int32"],
        "stdcall",
      );
      const invoke = ({ id, resourceId, sentinel }) => {
        loadCursorAOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = loadCursorA(ptr(0), ptr(resourceId));
          const handle = outcome.value;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            loadCursorAOracleCall.hookResult === null ||
            loadCursorAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error("LoadCursorA oracle did not pass through the hook");
          }
          return {
            id,
            instance: "0x0",
            resourceId,
            sentinel,
            hookResult: loadCursorAOracleCall.hookResult,
            systemResult: handle.toString(),
            resultNonNull: !handle.isNull(),
            hookLastErrorAfter: loadCursorAOracleCall.hookLastErrorAfter,
            systemLastError,
            hookResultMatches:
              loadCursorAOracleCall.hookResult === handle.toString(),
            hookAndSystemLastErrorMatch:
              loadCursorAOracleCall.hookLastErrorAfter === systemLastError,
          };
        } finally {
          loadCursorAOracleCall = null;
        }
      };
      const calls = [
        {
          id: "arrow-first",
          resourceId: 0x7f00,
          sentinel: 0x11223344,
        },
        {
          id: "arrow-repeated",
          resourceId: 0x7f00,
          sentinel: 0x55667788,
        },
        {
          id: "missing-predefined",
          resourceId: 0xffff,
          sentinel: 0x7a6b5c4d,
        },
      ].map(invoke);
      const validCalls = calls.slice(0, 2);
      const missingCall = calls[2];
      const mainResult = loadCursorAMainModuleCalls[0].result;
      loadCursorAOracleEvidence = {
        profile: "x86-system-arrow-cursor-shared-identity-v1",
        predefinedResourceId: 0x7f00,
        systemMetrics: {
          cursorWidth: getSystemMetrics(13),
          cursorHeight: getSystemMetrics(14),
        },
        calls,
        validCallsSucceeded: validCalls.every((call) => call.resultNonNull),
        repeatedValidIdentityShared:
          validCalls[0].systemResult === validCalls[1].systemResult,
        validIdentityMatchesMainCall: validCalls.every(
          (call) => call.systemResult === mainResult,
        ),
        missingPredefinedRejected: !missingCall.resultNonNull,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      loadCursorAOracleComplete = true;
      emit("load-cursor-a-oracle", loadCursorAOracleEvidence);
      maybeCompleteLoadCursorAObservation();
    } catch (error) {
      emit("load-cursor-a-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteGetVersionExAObservation() {
  if (
    getVersionExACompletionRecorded ||
    !getVersionExAOracleComplete ||
    getVersionExAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteGetVersionExAObservation, 10);
    return;
  }
  if (getVersionExADrainCheckScheduled) {
    return;
  }
  getVersionExADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    getVersionExADrainCheckScheduled = false;
    if (getVersionExACompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteGetVersionExAObservation();
      return;
    }
    getVersionExACompletionRecorded = true;
    emit("get-version-ex-a-complete", {
      profile: "x86-bgi-get-version-ex-a-v1",
      oracleProfile: getVersionExAOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: getVersionExAMainModuleCalls.map((call) => ({ ...call })),
    });
    if (stopOnEvent === "get-version-ex-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleGetVersionExAOracle(module) {
  if (getVersionExAOracleScheduled) {
    return;
  }
  getVersionExAOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetVersionExA");
      if (address === null) {
        throw new Error("GetVersionExA export is unavailable");
      }
      const getVersionExA = new SystemFunction(
        address,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const invoke = ({ declaredBytes, sentinel }) => {
        const guardBytes = 16;
        const outputCapacity = 156;
        const storageBytes = guardBytes + outputCapacity + guardBytes;
        const storage = Memory.alloc(storageBytes);
        storage.writeByteArray(Array(storageBytes).fill(0xa5));
        const output = storage.add(guardBytes);
        output.writeU32(declaredBytes);
        const storageBefore = snapshotBoundedRawBytes(
          storage,
          storageBytes,
          256,
        );
        getVersionExAOracleCall = {
          declaredBytes,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = getVersionExA(output);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getVersionExAOracleCall.hookResult === null ||
            getVersionExAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error("GetVersionExA oracle did not pass through the hook");
          }
          const storageAfter = snapshotBoundedRawBytes(
            storage,
            storageBytes,
            256,
          );
          const beforeBytes = storageBefore.bytes ?? [];
          const afterBytes = storageAfter.bytes ?? [];
          const changedStorageOffsets =
            beforeBytes.length === storageBytes &&
            afterBytes.length === storageBytes
              ? beforeBytes.flatMap((value, index) =>
                  value === afterBytes[index] ? [] : [index],
                )
              : [];
          const changedOutputOffsets = changedStorageOffsets
            .filter(
              (offset) =>
                offset >= guardBytes &&
                offset < guardBytes + outputCapacity,
            )
            .map((offset) => offset - guardBytes);
          return {
            declaredBytes,
            sentinel,
            hookResult: getVersionExAOracleCall.hookResult,
            systemResult,
            hookLastErrorAfter: getVersionExAOracleCall.hookLastErrorAfter,
            systemLastError,
            storageOffset: guardBytes,
            outputCapacity,
            storageBefore,
            storageAfter,
            outputAfter: snapshotOsVersionInfoA(output),
            changedOutputOffsets,
            prefixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes.slice(0, guardBytes).every((value) => value === 0xa5),
            suffixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes
                .slice(guardBytes + outputCapacity)
                .every((value) => value === 0xa5),
            bytesBeyondDeclaredSizeUnchanged:
              beforeBytes.length === storageBytes &&
              afterBytes.length === storageBytes &&
              beforeBytes
                .slice(guardBytes + Math.max(4, declaredBytes))
                .every(
                  (value, index) =>
                    value ===
                    afterBytes[guardBytes + Math.max(4, declaredBytes) + index],
                ),
            hookResultMatches:
              getVersionExAOracleCall.hookResult === systemResult,
            hookPreservesLastError:
              getVersionExAOracleCall.hookLastErrorAfter === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          getVersionExAOracleCall = null;
        }
      };
      const calls = [
        { declaredBytes: 0, sentinel: 0x10203040 },
        { declaredBytes: 147, sentinel: 0x11223344 },
        { declaredBytes: 148, sentinel: 0 },
        { declaredBytes: 148, sentinel: 87 },
        { declaredBytes: 148, sentinel: 0x12345678 },
        { declaredBytes: 156, sentinel: 0x76543210 },
      ].map(invoke);
      const baseCalls = calls.filter((call) => call.declaredBytes === 148);
      const referenceFields = baseCalls[0]?.outputAfter.fields ?? null;
      getVersionExAOracleEvidence = {
        profile: "x86-get-version-ex-a-size-and-output-v1",
        baseStructureBytes: 148,
        extendedStructureBytes: 156,
        calls,
        baseSizeSucceeded: baseCalls.every(
          (call) => call.systemResult !== 0 && call.hookResultMatches,
        ),
        baseOutputsStable:
          referenceFields !== null &&
          baseCalls.every(
            (call) =>
              JSON.stringify(call.outputAfter.fields ?? null) ===
              JSON.stringify(referenceFields),
          ),
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allGuardsUnchanged: calls.every(
          (call) => call.prefixGuardUnchanged && call.suffixGuardUnchanged,
        ),
      };
      getVersionExAOracleComplete = true;
      emit("get-version-ex-a-oracle", getVersionExAOracleEvidence);
      maybeCompleteGetVersionExAObservation();
    } catch (error) {
      emit("get-version-ex-a-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetCpInfoOracle(module) {
  if (getCpInfoOracleScheduled) {
    return;
  }
  getCpInfoOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetCPInfo");
      if (address === null) {
        throw new Error("GetCPInfo export is unavailable");
      }
      const getCpInfo = new SystemFunction(
        address,
        "uint32",
        ["uint32", "pointer"],
        "stdcall",
      );
      const invoke = (sentinel) => {
        const storage = Memory.alloc(64);
        storage.writeByteArray(Array(64).fill(0xa5));
        const output = storage.add(16);
        const storageBefore = snapshotBoundedRawBytes(storage, 64);
        getCpInfoOracleSentinel = sentinel;
        getCpInfoOracleResult = null;
        getCpInfoOracleLastError = null;
        try {
          const outcome = getCpInfo(932, output);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getCpInfoOracleResult === null ||
            getCpInfoOracleLastError === null
          ) {
            throw new Error("GetCPInfo oracle did not pass through the hook");
          }
          return {
            codePage: 932,
            sentinel,
            result,
            hookResult: getCpInfoOracleResult,
            hookLastErrorAfter: getCpInfoOracleLastError,
            systemLastError,
            storageOffset: 16,
            storageBefore,
            storageAfter: snapshotBoundedRawBytes(storage, 64),
            measuredOutput: snapshotMeasuredCpInfo(output),
            hookResultMatches: getCpInfoOracleResult === result,
            hookPreservesLastError: getCpInfoOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          getCpInfoOracleSentinel = null;
          getCpInfoOracleResult = null;
          getCpInfoOracleLastError = null;
        }
      };
      const calls = [0, 87, 0x12345678].map(invoke);
      emit("get-cp-info-oracle", {
        profile: "x86-code-page-932-output-v1",
        calls,
        allSucceeded: calls.every(
          (call) => call.result === 1 && call.hookResultMatches,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("get-cp-info-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetSystemInfoOracle(module) {
  if (getSystemInfoOracleScheduled) {
    return;
  }
  getSystemInfoOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetSystemInfo");
      if (address === null) {
        throw new Error("GetSystemInfo export is unavailable");
      }
      const getSystemInfo = new SystemFunction(
        address,
        "void",
        ["pointer"],
        "stdcall",
      );
      const invoke = (sentinel) => {
        const storage = Memory.alloc(64);
        storage.writeByteArray(Array(64).fill(0xa5));
        const output = storage.add(16);
        const storageBefore = snapshotBoundedRawBytes(storage, 64);
        getSystemInfoOracleSentinel = sentinel;
        getSystemInfoOracleLastError = null;
        try {
          const outcome = getSystemInfo(output);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (getSystemInfoOracleLastError === null) {
            throw new Error("GetSystemInfo oracle did not pass through the hook");
          }
          const storageAfter = snapshotBoundedRawBytes(storage, 64);
          const bytes = storageAfter.bytes ?? [];
          return {
            sentinel,
            hookLastErrorAfter: getSystemInfoOracleLastError,
            systemLastError,
            storageOffset: 16,
            storageBefore,
            storageAfter,
            measuredOutput: snapshotSystemInfo(output),
            prefixGuardUnchanged:
              bytes.length === 64 && bytes.slice(0, 16).every((value) => value === 0xa5),
            suffixGuardUnchanged:
              bytes.length === 64 && bytes.slice(52).every((value) => value === 0xa5),
            hookPreservesLastError: getSystemInfoOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          getSystemInfoOracleSentinel = null;
          getSystemInfoOracleLastError = null;
        }
      };
      const calls = [0, 87, 0x12345678].map(invoke);
      emit("get-system-info-oracle", {
        profile: "x86-system-info-output-v1",
        calls,
        allOutputsStable: calls.every(
          (call) =>
            JSON.stringify(call.measuredOutput.fields) ===
            JSON.stringify(calls[0].measuredOutput.fields),
        ),
        allGuardsUnchanged: calls.every(
          (call) => call.prefixGuardUnchanged && call.suffixGuardUnchanged,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("get-system-info-oracle-error", { error: error.message });
    }
  });
}

function scheduleEnumDisplayMonitorsOracle(module) {
  if (enumDisplayMonitorsOracleScheduled) {
    return;
  }
  enumDisplayMonitorsOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "EnumDisplayMonitors");
      if (address === null) {
        throw new Error("EnumDisplayMonitors export is unavailable");
      }
      const enumDisplayMonitors = new SystemFunction(
        address,
        "uint32",
        ["pointer", "pointer", "pointer", "pointer"],
        "stdcall",
      );
      const invoke = (id, callbackResult, sentinel, data) => {
        const callbacks = [];
        const callback = new NativeCallback(
          (monitor, monitorDc, rect, callbackData) => {
            callbacks.push({
              threadId: Process.getCurrentThreadId(),
              monitor: monitor.toString(),
              monitorDc: monitorDc.toString(),
              rect: snapshotRect(rect),
              data: callbackData.toString(),
              result: callbackResult,
            });
            return callbackResult;
          },
          "uint32",
          ["pointer", "pointer", "pointer", "pointer"],
          "stdcall",
        );
        enumDisplayMonitorsOracleCallbacks.push(callback);
        enumDisplayMonitorsOracleSentinel = sentinel;
        enumDisplayMonitorsOracleResult = null;
        enumDisplayMonitorsOracleLastError = null;
        try {
          const outcome = enumDisplayMonitors(ptr(0), ptr(0), callback, data);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            enumDisplayMonitorsOracleResult === null ||
            enumDisplayMonitorsOracleLastError === null
          ) {
            throw new Error(
              `EnumDisplayMonitors ${id} oracle did not pass through the hook`,
            );
          }
          return {
            id,
            callbackResult,
            sentinel,
            hdc: ptr(0).toString(),
            clipRect: ptr(0).toString(),
            data: data.toString(),
            hookResult: enumDisplayMonitorsOracleResult,
            hookLastErrorAfter: enumDisplayMonitorsOracleLastError,
            systemResult,
            systemLastError,
            callbacks,
            hookResultMatches:
              enumDisplayMonitorsOracleResult === systemResult,
            hookPreservesLastError:
              enumDisplayMonitorsOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
            allCallbackDataRoundTrips: callbacks.every(
              (invocation) => invocation.data === data.toString(),
            ),
            allCallbackDcsNull: callbacks.every(
              (invocation) => invocation.monitorDc === ptr(0).toString(),
            ),
          };
        } finally {
          enumDisplayMonitorsOracleSentinel = null;
          enumDisplayMonitorsOracleResult = null;
          enumDisplayMonitorsOracleLastError = null;
        }
      };
      const calls = [
        invoke("continue", 1, 87, ptr("0x12345678")),
        invoke("stop", 0, 0x12345678, ptr("0x87654321")),
      ];
      emit("enum-display-monitors-oracle", {
        profile: "x86-null-dc-null-clip-callback-enumeration-v1",
        calls,
        continueCallbackCount: calls[0].callbacks.length,
        stopCallbackCount: calls[1].callbacks.length,
        stopHaltsAfterFirstCallback: calls[1].callbacks.length === 1,
        expectedResultsObserved:
          calls[0].systemResult === 1 &&
          calls[1].systemResult === 0 &&
          calls.every((call) => call.hookResultMatches),
        allCallbackDataRoundTrips: calls.every(
          (call) => call.allCallbackDataRoundTrips,
        ),
        allCallbackDcsNull: calls.every((call) => call.allCallbackDcsNull),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
        firstMonitorStable:
          calls.every((call) => call.callbacks.length > 0) &&
          calls[0].callbacks[0].monitor === calls[1].callbacks[0].monitor,
        firstRectStable:
          calls.every((call) => call.callbacks.length > 0) &&
          JSON.stringify(calls[0].callbacks[0].rect.fields) ===
            JSON.stringify(calls[1].callbacks[0].rect.fields),
      });
    } catch (error) {
      emit("enum-display-monitors-oracle-error", { error: error.message });
    }
  });
}

function scheduleComInitializationOracle(module) {
  if (comInitializationOracleScheduled) {
    return;
  }
  comInitializationOracleScheduled = true;
  setImmediate(() => {
    let coUninitialize = null;
    let successfulCalls = 0;
    try {
      const coInitializeAddress = findExport(module, "CoInitialize");
      const coInitializeExAddress = findExport(module, "CoInitializeEx");
      const coUninitializeAddress = findExport(module, "CoUninitialize");
      if (
        coInitializeAddress === null ||
        coInitializeExAddress === null ||
        coUninitializeAddress === null
      ) {
        throw new Error("COM initialization exports are unavailable");
      }
      const coInitialize = new SystemFunction(
        coInitializeAddress,
        "int",
        ["pointer"],
        "stdcall",
      );
      const coInitializeEx = new SystemFunction(
        coInitializeExAddress,
        "int",
        ["pointer", "uint32"],
        "stdcall",
      );
      coUninitialize = new NativeFunction(
        coUninitializeAddress,
        "void",
        [],
        "stdcall",
      );
      const invoke = (id, api, call, args, sentinel) => {
        const observation = {
          id,
          api,
          threadId: Process.getCurrentThreadId(),
          reserved: ptr(0).toString(),
          concurrency: api === "CoInitializeEx" ? args[1] : null,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        comInitializationOracleCall = observation;
        try {
          const outcome = call(...args);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            observation.hookResult === null ||
            observation.hookLastErrorAfter === null
          ) {
            throw new Error(`${api} ${id} oracle did not pass through the hook`);
          }
          if ((systemResult & 0x80000000) === 0) {
            successfulCalls += 1;
          }
          return {
            ...observation,
            systemResult,
            signedSystemResult: systemResult | 0,
            systemLastError,
            hookResultMatches: observation.hookResult === systemResult,
            hookAndSystemLastErrorMatch:
              observation.hookLastErrorAfter === systemLastError,
            hookPreservesSentinel:
              observation.hookLastErrorAfter === sentinel,
            systemPreservesSentinel: systemLastError === sentinel,
          };
        } finally {
          comInitializationOracleCall = null;
        }
      };
      const balanceOne = () => {
        if (successfulCalls === 0) {
          throw new Error("COM oracle attempted an unbalanced CoUninitialize");
        }
        coUninitialize();
        successfulCalls -= 1;
      };

      const calls = [];
      calls.push(
        invoke("sta-first", "CoInitialize", coInitialize, [ptr(0)], 0x11111111),
      );
      calls.push(
        invoke("sta-repeat", "CoInitialize", coInitialize, [ptr(0)], 0x22222222),
      );
      calls.push(
        invoke(
          "sta-to-mta-conflict",
          "CoInitializeEx",
          coInitializeEx,
          [ptr(0), 0],
          0x33333333,
        ),
      );
      balanceOne();
      calls.push(
        invoke(
          "sta-count-one-conflict",
          "CoInitializeEx",
          coInitializeEx,
          [ptr(0), 0],
          0x44444444,
        ),
      );
      balanceOne();
      calls.push(
        invoke(
          "mta-first",
          "CoInitializeEx",
          coInitializeEx,
          [ptr(0), 0],
          0x55555555,
        ),
      );
      calls.push(
        invoke(
          "mta-repeat",
          "CoInitializeEx",
          coInitializeEx,
          [ptr(0), 0],
          0x66666666,
        ),
      );
      calls.push(
        invoke(
          "mta-to-sta-conflict",
          "CoInitialize",
          coInitialize,
          [ptr(0)],
          0x77777777,
        ),
      );
      balanceOne();
      balanceOne();
      calls.push(
        invoke(
          "balanced-sta-first",
          "CoInitialize",
          coInitialize,
          [ptr(0)],
          0x88888888,
        ),
      );
      balanceOne();

      const expectedResults = [
        0x00000000,
        0x00000001,
        0x80010106,
        0x80010106,
        0x00000000,
        0x00000001,
        0x80010106,
        0x00000000,
      ];
      emit("com-initialization-oracle", {
        profile: "x86-balanced-agent-thread-com-initialization-v1",
        calls,
        expectedResults,
        expectedResultsObserved: calls.every(
          (call, index) =>
            call.systemResult === expectedResults[index] && call.hookResultMatches,
        ),
        allHookAndSystemLastErrorMatch: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
        repeatAndConflictCallsPreserveSentinels: calls
          .filter(
            (call) =>
              call.id.includes("repeat") || call.id.includes("conflict"),
          )
          .every(
            (call) => call.hookPreservesSentinel && call.systemPreservesSentinel,
          ),
        firstSuccessLastErrors: calls
          .filter((call) => call.id.includes("first"))
          .map((call) => ({
            id: call.id,
            hookLastErrorAfter: call.hookLastErrorAfter,
            systemLastError: call.systemLastError,
            sentinel: call.sentinel,
          })),
        successfulCallBalanceZero: successfulCalls === 0,
      });
    } catch (error) {
      emit("com-initialization-oracle-error", { error: error.message });
    } finally {
      comInitializationOracleCall = null;
      if (coUninitialize !== null) {
        while (successfulCalls > 0) {
          coUninitialize();
          successfulCalls -= 1;
        }
      }
    }
  });
}

function scheduleProcessorFeatureOracle(module) {
  if (processorFeatureOracleScheduled) {
    return;
  }
  processorFeatureOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "IsProcessorFeaturePresent");
      if (address === null) {
        throw new Error("IsProcessorFeaturePresent export is unavailable");
      }
      const query = new SystemFunction(
        address,
        "uint32",
        ["uint32"],
        "stdcall",
      );
      const invoke = (feature, sentinel) => {
        processorFeatureOracleSentinel = sentinel;
        processorFeatureOracleResult = null;
        processorFeatureOracleLastError = null;
        try {
          const outcome = query(feature);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            processorFeatureOracleResult === null ||
            processorFeatureOracleLastError === null
          ) {
            throw new Error("processor-feature oracle did not pass through the hook");
          }
          return {
            feature,
            sentinel,
            result,
            hookResult: processorFeatureOracleResult,
            hookLastErrorAfter: processorFeatureOracleLastError,
            systemLastError,
            hookResultMatches: processorFeatureOracleResult === result,
            hookPreservesLastError: processorFeatureOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          processorFeatureOracleSentinel = null;
          processorFeatureOracleResult = null;
          processorFeatureOracleLastError = null;
        }
      };
      const calls = [
        invoke(10, 0),
        invoke(10, 87),
        invoke(10, 0x12345678),
        invoke(0xffffffff, 122),
      ];
      emit("processor-feature-oracle", {
        profile: "x86-processor-feature-query-v1",
        calls,
        feature10Stable: calls
          .slice(0, 3)
          .every(
            (call) =>
              call.result === calls[0].result && call.hookResultMatches,
          ),
        invalidFeatureRejected: calls[3].result === 0,
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("processor-feature-oracle-error", { error: error.message });
    }
  });
}

function scheduleUnhandledExceptionFilterOracle(module) {
  if (unhandledExceptionFilterOracleScheduled) {
    return;
  }
  unhandledExceptionFilterOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "SetUnhandledExceptionFilter");
      if (address === null) {
        throw new Error("SetUnhandledExceptionFilter export is unavailable");
      }
      const setFilter = new SystemFunction(
        address,
        "pointer",
        ["pointer"],
        "stdcall",
      );
      const callbackA = new NativeCallback(
        () => 0,
        "int",
        ["pointer"],
        "stdcall",
      );
      const callbackB = new NativeCallback(
        () => 0,
        "int",
        ["pointer"],
        "stdcall",
      );
      unhandledExceptionFilterCallbacks.push(callbackA, callbackB);

      const invoke = (step, filter, sentinel) => {
        unhandledExceptionFilterOracleSentinel = sentinel;
        unhandledExceptionFilterOracleResult = null;
        unhandledExceptionFilterOracleLastError = null;
        try {
          const outcome = setFilter(filter);
          const result = outcome.value.toString().toLowerCase();
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            unhandledExceptionFilterOracleResult === null ||
            unhandledExceptionFilterOracleLastError === null
          ) {
            throw new Error(
              "unhandled-exception-filter oracle did not pass through the hook",
            );
          }
          return {
            step,
            filter: filter.toString().toLowerCase(),
            sentinel,
            result,
            hookResult: unhandledExceptionFilterOracleResult.toLowerCase(),
            hookLastErrorAfter: unhandledExceptionFilterOracleLastError,
            systemLastError,
            hookResultMatches:
              unhandledExceptionFilterOracleResult.toLowerCase() === result,
            hookPreservesLastError:
              unhandledExceptionFilterOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          unhandledExceptionFilterOracleSentinel = null;
          unhandledExceptionFilterOracleResult = null;
          unhandledExceptionFilterOracleLastError = null;
        }
      };

      const installA = invoke("install-a", callbackA, 0);
      const installB = invoke("replace-with-b", callbackB, 87);
      const remove = invoke("remove", NULL, 0x12345678);
      const restore = invoke("restore-original", ptr(installA.result), 122);
      const calls = [installA, installB, remove, restore];
      emit("unhandled-exception-filter-oracle", {
        profile: "x86-process-filter-replacement-v1",
        callbacks: [
          callbackA.toString().toLowerCase(),
          callbackB.toString().toLowerCase(),
        ],
        originalFilter: installA.result,
        calls,
        replacementChainExact:
          installB.result === callbackA.toString().toLowerCase() &&
          remove.result === callbackB.toString().toLowerCase() &&
          restore.result === NULL.toString().toLowerCase(),
        originalFilterRestored: restore.filter === installA.result,
        allHookResultsMatch: calls.every((call) => call.hookResultMatches),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("unhandled-exception-filter-oracle-error", {
        error: error.message,
      });
    }
  });
}

function scheduleHeapSizeOracle(module) {
  if (heapSizeOracleScheduled) {
    return;
  }
  heapSizeOracleScheduled = true;
  setImmediate(() => {
    let heap = NULL;
    let allocation = NULL;
    let allocationLive = false;
    try {
      const requireExport = (name) => {
        const address = findExport(module, name);
        if (address === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return address;
      };
      const heapCreate = new SystemFunction(
        requireExport("HeapCreate"),
        "pointer",
        ["uint", "uint", "uint"],
        "stdcall",
      );
      const heapAlloc = new SystemFunction(
        requireExport("HeapAlloc"),
        "pointer",
        ["pointer", "uint", "uint"],
        "stdcall",
      );
      const heapSize = new SystemFunction(
        requireExport("HeapSize"),
        "uint",
        ["pointer", "uint", "pointer"],
        "stdcall",
      );
      const heapFree = new SystemFunction(
        requireExport("HeapFree"),
        "bool",
        ["pointer", "uint", "pointer"],
        "stdcall",
      );
      const heapDestroy = new SystemFunction(
        requireExport("HeapDestroy"),
        "bool",
        ["pointer"],
        "stdcall",
      );

      heap = heapCreate(0, 0, 0).value;
      if (heap.isNull()) {
        throw new Error("HeapCreate returned NULL");
      }
      allocation = heapAlloc(heap, 0, 128).value;
      if (allocation.isNull()) {
        throw new Error("HeapAlloc returned NULL");
      }
      allocationLive = true;

      const invoke = (step, flags, pointer, sentinel) => {
        heapSizeOracleSentinel = sentinel;
        heapSizeOracleResult = null;
        heapSizeOracleLastError = null;
        try {
          const outcome = heapSize(heap, flags, pointer);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (heapSizeOracleResult === null || heapSizeOracleLastError === null) {
            throw new Error("heap-size oracle did not pass through the hook");
          }
          return {
            step,
            heap: heap.toString().toLowerCase(),
            flags,
            pointer: pointer.toString().toLowerCase(),
            sentinel,
            result,
            hookResult: heapSizeOracleResult,
            hookLastErrorAfter: heapSizeOracleLastError,
            systemLastError,
            hookResultMatches: heapSizeOracleResult === result,
            hookPreservesLastError: heapSizeOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          heapSizeOracleSentinel = null;
          heapSizeOracleResult = null;
          heapSizeOracleLastError = null;
        }
      };

      const live = invoke("live-flags-zero", 0, allocation, 0);
      const liveRepeat = invoke("live-repeat", 0, allocation, 87);
      const noSerialize = invoke(
        "live-no-serialize",
        1,
        allocation,
        0x12345678,
      );
      const interior = invoke("interior-pointer", 0, allocation.add(1), 122);
      const freeOutcome = heapFree(heap, 0, allocation);
      if (!freeOutcome.value) {
        throw new Error("HeapFree failed in heap-size oracle");
      }
      allocationLive = false;
      const freed = invoke("freed-pointer", 0, allocation, 5);
      const calls = [live, liveRepeat, noSerialize, interior, freed];
      emit("heap-size-oracle", {
        profile: "x86-private-heap-size-query-v1",
        requestedBytes: 128,
        allocation: allocation.toString().toLowerCase(),
        failureSentinel: 0xffffffff,
        calls,
        liveResultStable:
          live.result === liveRepeat.result && live.result === noSerialize.result,
        invalidPointersReturnFailure:
          interior.result === 0xffffffff && freed.result === 0xffffffff,
        allHookResultsMatch: calls.every((call) => call.hookResultMatches),
        successfulCallsPreserveLastError: [live, liveRepeat, noSerialize].every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
        invalidCallsSetErrorInvalidParameter: [interior, freed].every(
          (call) =>
            call.hookLastErrorAfter === 87 && call.systemLastError === 87,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("heap-size-oracle-error", { error: error.message });
    } finally {
      heapSizeOracleSentinel = null;
      heapSizeOracleResult = null;
      heapSizeOracleLastError = null;
      try {
        if (allocationLive && !heap.isNull() && !allocation.isNull()) {
          const address = findExport(module, "HeapFree");
          if (address !== null) {
            new NativeFunction(
              address,
              "bool",
              ["pointer", "uint", "pointer"],
              "stdcall",
            )(heap, 0, allocation);
          }
        }
        if (!heap.isNull()) {
          const address = findExport(module, "HeapDestroy");
          if (address !== null) {
            new NativeFunction(
              address,
              "bool",
              ["pointer"],
              "stdcall",
            )(heap);
          }
        }
      } catch (cleanupError) {
        emit("heap-size-oracle-cleanup-error", {
          error: cleanupError.message,
        });
      }
    }
  });
}

function scheduleGetCurrentThreadOracle(module) {
  if (getCurrentThreadOracleScheduled) {
    return;
  }
  getCurrentThreadOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetCurrentThread");
      if (address === null) {
        throw new Error("GetCurrentThread export is unavailable");
      }
      const oracleCode = Memory.alloc(Process.pageSize);
      const targetBytes = address.toUInt32().toString(16).padStart(8, "0");
      const targetLittleEndian = [
        Number.parseInt(targetBytes.slice(6, 8), 16),
        Number.parseInt(targetBytes.slice(4, 6), 16),
        Number.parseInt(targetBytes.slice(2, 4), 16),
        Number.parseInt(targetBytes.slice(0, 2), 16),
      ];
      const machineCode = [
        0x8b, 0x4c, 0x24, 0x04, // mov ecx, [esp + 4]
        0x64, 0x89, 0x0d, 0x34, 0x00, 0x00, 0x00, // mov fs:[0x34], ecx
        0xb8,
        ...targetLittleEndian, // mov eax, GetCurrentThread
        0xff, 0xd0, // call eax
        0x8b, 0x54, 0x24, 0x08, // mov edx, [esp + 8]
        0x89, 0x02, // mov [edx], eax
        0x64, 0x8b, 0x0d, 0x34, 0x00, 0x00, 0x00, // mov ecx, fs:[0x34]
        0x89, 0x4a, 0x04, // mov [edx + 4], ecx
        0xc2, 0x08, 0x00, // ret 8
      ];
      Memory.patchCode(oracleCode, machineCode.length, (writable) => {
        writable.writeByteArray(machineCode);
      });
      const invokeNativeOracle = new NativeFunction(
        oracleCode,
        "void",
        ["uint32", "pointer"],
        "stdcall",
      );
      const invoke = (sentinel) => {
        const threadIdBefore = Process.getCurrentThreadId();
        const output = Memory.alloc(8);
        output.writeByteArray(Array(8).fill(0xa5));
        invokeNativeOracle(sentinel, output);
        const threadIdAfter = Process.getCurrentThreadId();
        const result = output.readU32();
        const nativeLastErrorAfter = output.add(4).readU32();
        return {
          sentinel,
          threadIdBefore,
          threadIdAfter,
          result,
          nativeLastErrorAfter,
          sameThread: threadIdBefore === threadIdAfter,
          nativePreservesLastError: nativeLastErrorAfter === sentinel,
        };
      };
      const calls = [0, 87, 122, 0x12345678].map(invoke);
      const pseudoHandle = calls[0].result;
      emit("get-current-thread-oracle", {
        profile: "x86-current-thread-pseudo-handle-v1",
        threadId: Process.getCurrentThreadId(),
        pseudoHandle,
        calls,
        allResultsStable: calls.every(
          (call) => call.result === pseudoHandle && call.sameThread,
        ),
        allPreserveLastError: calls.every(
          (call) => call.nativePreservesLastError,
        ),
      });
    } catch (error) {
      emit("get-current-thread-oracle-error", { error: error.message });
    }
  });
}

function scheduleSetThreadAffinityMaskOracle(module) {
  if (setThreadAffinityMaskOracleScheduled) {
    return;
  }
  setThreadAffinityMaskOracleScheduled = true;
  setImmediate(() => {
    let originalMask = null;
    let currentMask = null;
    let currentThread = null;
    let setAffinity = null;
    const calls = [];
    try {
      const address = (name) => {
        const found = findExport(module, name);
        if (found === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return found;
      };
      const getCurrentThread = new NativeFunction(
        address("GetCurrentThread"),
        "pointer",
        [],
        "stdcall",
      );
      const getCurrentProcess = new NativeFunction(
        address("GetCurrentProcess"),
        "pointer",
        [],
        "stdcall",
      );
      const getProcessAffinityMask = new SystemFunction(
        address("GetProcessAffinityMask"),
        "uint32",
        ["pointer", "pointer", "pointer"],
        "stdcall",
      );
      setAffinity = new SystemFunction(
        address("SetThreadAffinityMask"),
        "uint32",
        ["pointer", "uint32"],
        "stdcall",
      );
      currentThread = getCurrentThread();
      const processMaskStorage = Memory.alloc(4);
      const systemMaskStorage = Memory.alloc(4);
      processMaskStorage.writeU32(0);
      systemMaskStorage.writeU32(0);
      const affinityQuery = getProcessAffinityMask(
        getCurrentProcess(),
        processMaskStorage,
        systemMaskStorage,
      );
      if ((Number(affinityQuery.value) >>> 0) === 0) {
        throw new Error(
          `GetProcessAffinityMask failed with ${Number(affinityQuery.lastError) >>> 0}`,
        );
      }
      const processMask = processMaskStorage.readU32();
      const systemMask = systemMaskStorage.readU32();
      const selectedMask = (processMask & -processMask) >>> 0;
      if (selectedMask === 0) {
        throw new Error("process affinity mask has no usable x86 processor bit");
      }

      const invoke = (label, threadHandle, requestedMask, sentinel) => {
        setThreadAffinityMaskOracleSentinel = sentinel;
        setThreadAffinityMaskOracleResult = null;
        setThreadAffinityMaskOracleLastError = null;
        try {
          const outcome = setAffinity(threadHandle, requestedMask);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            setThreadAffinityMaskOracleResult === null ||
            setThreadAffinityMaskOracleLastError === null
          ) {
            throw new Error(
              "set-thread-affinity-mask oracle did not pass through the hook",
            );
          }
          const call = {
            label,
            threadHandle: threadHandle.toString(),
            requestedMask,
            sentinel,
            result,
            hookResult: setThreadAffinityMaskOracleResult,
            hookLastErrorAfter: setThreadAffinityMaskOracleLastError,
            systemLastError,
            hookResultMatches: setThreadAffinityMaskOracleResult === result,
          };
          calls.push(call);
          return call;
        } finally {
          setThreadAffinityMaskOracleSentinel = null;
          setThreadAffinityMaskOracleResult = null;
          setThreadAffinityMaskOracleLastError = null;
        }
      };

      const firstSet = invoke("set-selected", currentThread, selectedMask, 0x12345678);
      if (firstSet.result === 0) {
        throw new Error(
          `valid affinity set failed with ${firstSet.systemLastError}`,
        );
      }
      originalMask = firstSet.result;
      currentMask = selectedMask;
      const restore = invoke("restore-original", currentThread, originalMask, 87);
      if (restore.result !== selectedMask) {
        throw new Error(
          `affinity restore returned ${restore.result} instead of ${selectedMask}`,
        );
      }
      currentMask = originalMask;
      const zeroMask = invoke("reject-zero-mask", currentThread, 0, 122);
      const invalidHandle = invoke("reject-null-handle", ptr(0), selectedMask, 0);

      emit("set-thread-affinity-mask-oracle", {
        profile: "x86-current-thread-logical-affinity-v1",
        currentThread: currentThread.toString(),
        processMask,
        systemMask,
        selectedMask,
        originalMask,
        calls,
        successReturnsPreviousMask:
          firstSet.result === originalMask && restore.result === selectedMask,
        successPreservesLastError:
          firstSet.hookLastErrorAfter === firstSet.sentinel &&
          firstSet.systemLastError === firstSet.sentinel &&
          restore.hookLastErrorAfter === restore.sentinel &&
          restore.systemLastError === restore.sentinel,
        zeroMaskRejected:
          zeroMask.result === 0 &&
          zeroMask.hookLastErrorAfter === 87 &&
          zeroMask.systemLastError === 87,
        nullHandleRejected:
          invalidHandle.result === 0 &&
          invalidHandle.hookLastErrorAfter === 6 &&
          invalidHandle.systemLastError === 6,
        restored: currentMask === originalMask,
      });
    } catch (error) {
      emit("set-thread-affinity-mask-oracle-error", { error: error.message });
    } finally {
      if (
        setAffinity !== null &&
        currentThread !== null &&
        originalMask !== null &&
        currentMask !== originalMask
      ) {
        try {
          setThreadAffinityMaskOracleSentinel = 0;
          setAffinity(currentThread, originalMask);
        } catch (cleanupError) {
          emit("set-thread-affinity-mask-oracle-cleanup-error", {
            error: cleanupError.message,
          });
        } finally {
          setThreadAffinityMaskOracleSentinel = null;
          setThreadAffinityMaskOracleResult = null;
          setThreadAffinityMaskOracleLastError = null;
        }
      }
    }
  });
}

function scheduleTlsLifecycleOracle(module) {
  if (tlsLifecycleOracleScheduled) {
    return;
  }
  tlsLifecycleOracleScheduled = true;
  setImmediate(() => {
    const allocatedIndices = [];
    let tlsFree = null;
    let finalized = false;
    const finish = (error, observation = null) => {
      if (finalized) {
        return;
      }
      finalized = true;
      if (error === null) {
        emit("tls-get-value-oracle", observation);
      } else {
        emit("tls-get-value-oracle-error", { error: error.message });
      }
      try {
        if (tlsFree !== null) {
          for (const index of allocatedIndices.reverse()) {
            tlsFree(index);
          }
        }
      } catch (cleanupError) {
        emit("tls-get-value-oracle-cleanup-error", {
          error: cleanupError.message,
        });
      }
    };
    try {
      const address = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const tlsAlloc = new NativeFunction(
        address("TlsAlloc"),
        "uint32",
        [],
        "stdcall",
      );
      tlsFree = new NativeFunction(
        address("TlsFree"),
        "int",
        ["uint32"],
        "stdcall",
      );
      const tlsSetValue = new NativeFunction(
        address("TlsSetValue"),
        "int",
        ["uint32", "pointer"],
        "stdcall",
      );
      const tlsGetValue = new SystemFunction(
        address("TlsGetValue"),
        "pointer",
        ["uint32"],
        "stdcall",
      );
      const setLastError = new NativeFunction(
        address("SetLastError"),
        "void",
        ["uint32"],
        "stdcall",
      );
      const allocate = () => {
        const index = Number(tlsAlloc()) >>> 0;
        if (index === 0xffffffff) {
          throw new Error("TlsAlloc returned TLS_OUT_OF_INDEXES");
        }
        allocatedIndices.push(index);
        return index;
      };
      const read = (id, index, sentinel) => {
        setLastError(sentinel);
        const outcome = tlsGetValue(index);
        return {
          id,
          index,
          sentinel,
          result: outcome.value.toString(),
          lastError: Number(outcome.lastError) >>> 0,
        };
      };

      const mainIndex = allocate();
      const mainValue = ptr("0x12345678");
      const mainThreadId = Process.getCurrentThreadId();
      if (Number(tlsSetValue(mainIndex, mainValue)) === 0) {
        throw new Error("TlsSetValue rejected the main-thread oracle value");
      }
      const mainSetRead = read("main-thread-set-value", mainIndex, 87);

      const unsetIndex = allocate();
      const unsetRead = read("allocated-unset-value", unsetIndex, 122);
      const invalidRead = read("invalid-index", 0xffffffff, 122);
      finish(null, {
        profile: "x86-success-null-and-invalid-v1",
        threadId: mainThreadId,
        setValue: mainValue.toString(),
        calls: [mainSetRead, unsetRead, invalidRead],
      });
    } catch (error) {
      finish(error);
    }
  });
}

function scheduleFlsLifecycleOracle(module) {
  if (flsLifecycleOracleScheduled) {
    return;
  }
  flsLifecycleOracleScheduled = true;
  setImmediate(() => {
    const allocatedIndices = [];
    let flsFree = null;
    let finalized = false;
    const finish = (error, observation = null) => {
      if (finalized) {
        return;
      }
      finalized = true;
      if (error === null) {
        emit("fls-get-value-oracle", observation);
      } else {
        emit("fls-get-value-oracle-error", { error: error.message });
      }
      try {
        if (flsFree !== null) {
          for (const index of allocatedIndices.reverse()) {
            flsFree(index);
          }
        }
      } catch (cleanupError) {
        emit("fls-get-value-oracle-cleanup-error", {
          error: cleanupError.message,
        });
      }
    };
    try {
      const address = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const flsAlloc = new NativeFunction(
        address("FlsAlloc"),
        "uint32",
        ["pointer"],
        "stdcall",
      );
      flsFree = new NativeFunction(
        address("FlsFree"),
        "int",
        ["uint32"],
        "stdcall",
      );
      const flsSetValue = new NativeFunction(
        address("FlsSetValue"),
        "int",
        ["uint32", "pointer"],
        "stdcall",
      );
      const flsGetValue = new SystemFunction(
        address("FlsGetValue"),
        "pointer",
        ["uint32"],
        "stdcall",
      );
      const setLastError = new NativeFunction(
        address("SetLastError"),
        "void",
        ["uint32"],
        "stdcall",
      );
      const allocate = () => {
        const index = Number(flsAlloc(ptr(0))) >>> 0;
        if (index === 0xffffffff) {
          throw new Error("FlsAlloc returned FLS_OUT_OF_INDEXES");
        }
        allocatedIndices.push(index);
        return index;
      };
      const read = (id, index, sentinel) => {
        setLastError(sentinel);
        const outcome = flsGetValue(index);
        return {
          id,
          index,
          sentinel,
          result: outcome.value.toString(),
          lastError: Number(outcome.lastError) >>> 0,
        };
      };

      const mainIndex = allocate();
      const mainValue = ptr("0x12345678");
      const mainThreadId = Process.getCurrentThreadId();
      if (Number(flsSetValue(mainIndex, mainValue)) === 0) {
        throw new Error("FlsSetValue rejected the main-thread oracle value");
      }
      const mainSetRead = read("main-thread-set-value", mainIndex, 87);

      const unsetIndex = allocate();
      const unsetRead = read("allocated-unset-value", unsetIndex, 122);
      const invalidRead = read("invalid-index", 0xffffffff, 122);
      finish(null, {
        profile: "x86-success-null-and-invalid-v1",
        threadId: mainThreadId,
        setValue: mainValue.toString(),
        calls: [mainSetRead, unsetRead, invalidRead],
      });
    } catch (error) {
      finish(error);
    }
  });
}

function scheduleCriticalSectionOracle(module) {
  if (criticalSectionOracleScheduled) {
    return;
  }
  criticalSectionOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const initialize = new NativeFunction(
        address("InitializeCriticalSectionAndSpinCount"),
        "int",
        ["pointer", "uint32"],
        "stdcall",
      );
      const initializeDefault = new NativeFunction(
        address("InitializeCriticalSection"),
        "void",
        ["pointer"],
        "stdcall",
      );
      const enter = new NativeFunction(
        address("EnterCriticalSection"),
        "void",
        ["pointer"],
        "stdcall",
      );
      const leave = new NativeFunction(
        address("LeaveCriticalSection"),
        "void",
        ["pointer"],
        "stdcall",
      );
      const delete_ = new NativeFunction(
        address("DeleteCriticalSection"),
        "void",
        ["pointer"],
        "stdcall",
      );
      const storage = Memory.alloc(24);
      storage.writeByteArray(Array(24).fill(0xa5));
      const states = [{ step: "before", snapshot: snapshotCriticalSection(storage) }];
      const defaultStorage = Memory.alloc(24);
      defaultStorage.writeByteArray(Array(24).fill(0xa5));
      const defaultBefore = snapshotCriticalSection(defaultStorage);
      criticalSectionOracleSentinel = 86;
      criticalSectionOracleLastError = null;
      let defaultLastError = null;
      try {
        initializeDefault(defaultStorage);
        defaultLastError = criticalSectionOracleLastError;
        if (defaultLastError === null) {
          throw new Error(
            "initialize-default did not pass through the critical-section hook",
          );
        }
      } finally {
        criticalSectionOracleSentinel = null;
        criticalSectionOracleLastError = null;
      }
      const defaultAfter = snapshotCriticalSection(defaultStorage);
      const invoke = (step, sentinel, operation) => {
        criticalSectionOracleSentinel = sentinel;
        criticalSectionOracleLastError = null;
        try {
          const result = operation();
          const lastError = criticalSectionOracleLastError;
          if (lastError === null) {
            throw new Error(`${step} did not pass through the critical-section hook`);
          }
          states.push({
            step,
            sentinel,
            result: result === undefined ? null : Number(result),
            lastError,
            snapshot: snapshotCriticalSection(storage),
          });
        } finally {
          criticalSectionOracleSentinel = null;
          criticalSectionOracleLastError = null;
        }
      };
      invoke("initialize", 87, () => initialize(storage, 4000));
      invoke("enter-first", 88, () => enter(storage));
      invoke("enter-recursive", 89, () => enter(storage));
      invoke("leave-recursive", 90, () => leave(storage));
      invoke("leave-final", 91, () => leave(storage));
      invoke("delete", 92, () => delete_(storage));
      emit("critical-section-oracle", {
        profile: "x86-default-and-uncontended-recursive-v2",
        structureSize: 24,
        spinCount: 4000,
        defaultInitialization: {
          sentinel: 86,
          lastError: defaultLastError,
          exception: null,
          before: defaultBefore,
          after: defaultAfter,
        },
        states,
      });
    } catch (error) {
      emit("critical-section-oracle-error", { error: error.message });
    }
  });
}

Process.attachModuleObserver({
  onAdded(module) {
    emit("module-load", {
      module: module.name,
      base: module.base.toString(),
      size: module.size,
      path: module.path,
    });

    const moduleHooks = hooks[module.name.toLowerCase()];
    if (moduleHooks === undefined) {
      return;
    }
    for (const [name, decoders] of moduleHooks) {
      try {
        attachHook(module, name, decoders);
      } catch (error) {
        emit("hook-error", { module: module.name, api: name, error: error.message });
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-version-ex-a")
    ) {
      try {
        attachGetVersionExAHook(module);
        scheduleGetVersionExAOracle(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "GetVersionExA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("create-waitable-timer-a")
    ) {
      try {
        attachCreateWaitableTimerAHook(module);
        scheduleCreateWaitableTimerAOracle(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "CreateWaitableTimerA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("critical-section")
    ) {
      scheduleCriticalSectionOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("command-line-a")
    ) {
      scheduleCommandLineOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("environment-strings-w")
    ) {
      scheduleEnvironmentStringsOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("wide-char-to-multi-byte")
    ) {
      scheduleWideCharOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("multi-byte-to-wide-char")
    ) {
      scheduleMultiByteOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("set-current-directory-a")
    ) {
      scheduleSetCurrentDirectoryAOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("create-mutex-a")
    ) {
      scheduleCreateMutexAOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-string-type-w")
    ) {
      scheduleGetStringTypeOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("lc-map-string-w")
    ) {
      scheduleLCMapStringOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("interlocked-decrement")
    ) {
      scheduleInterlockedDecrementOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-module-file-name-a")
    ) {
      scheduleGetModuleFileNameAOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-last-error")
    ) {
      scheduleGetLastErrorOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("set-last-error")
    ) {
      scheduleSetLastErrorOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-acp")
    ) {
      scheduleGetAcpOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("system-default-lang-id")
    ) {
      scheduleSystemDefaultLangIdOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("drive-type")
    ) {
      scheduleDriveTypeOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-cp-info")
    ) {
      scheduleGetCpInfoOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-system-info")
    ) {
      scheduleGetSystemInfoOracle(module);
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("load-icon-a")
    ) {
      try {
        attachLoadIconAHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "LoadIconA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("load-cursor-a")
    ) {
      try {
        attachLoadCursorAHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "LoadCursorA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("enum-display-monitors")
    ) {
      scheduleEnumDisplayMonitorsOracle(module);
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("swap-mouse-button")
    ) {
      scheduleSwapMouseButtonStateObserver(module);
    }
    if (
      module.name.toLowerCase() === "winmm.dll" &&
      selectedObservations.has("time-get-dev-caps")
    ) {
      scheduleTimeGetDevCapsOracle(module);
    }
    if (
      module.name.toLowerCase() === "ole32.dll" &&
      selectedObservations.has("com-initialization")
    ) {
      scheduleComInitializationOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("processor-feature")
    ) {
      scheduleProcessorFeatureOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("unhandled-exception-filter")
    ) {
      scheduleUnhandledExceptionFilterOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("heap-size")
    ) {
      scheduleHeapSizeOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("tls-lifecycle")
    ) {
      scheduleTlsLifecycleOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("fls-lifecycle")
    ) {
      scheduleFlsLifecycleOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-current-thread")
    ) {
      try {
        attachGetCurrentThreadCallSites(module);
      } catch (error) {
        emit("get-current-thread-callsite-error", { error: error.message });
      }
      scheduleGetCurrentThreadOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("set-thread-affinity-mask")
    ) {
      try {
        attachThreadAffinityTickCountObserver(module);
      } catch (error) {
        emit("set-thread-affinity-mask-calibration-error", {
          error: error.message,
        });
      }
      scheduleSetThreadAffinityMaskOracle(module);
    }
  },
});

emit("agent-ready", {
  architecture: Process.arch,
  pointerSize: Process.pointerSize,
  processId: Process.id,
  selectedVtables: [...selectedVtables],
  selectedObservations: [...selectedObservations],
});
