"use strict";

const schema = "vine.win32-trace.v2";
const agentOptions = globalThis.__vineTraceOptions ?? {
  vtables: [],
  observations: [],
};
const selectedVtables = new Set(agentOptions.vtables ?? []);
const selectedObservations = new Set(agentOptions.observations ?? []);
let sequence = 0;
let nextCallId = 1;
const attached = new Set();
const attachedVtableMethods = new Set();
const observedVtables = new Set();
const activeCallsByThread = new Map();
let criticalSectionOracleScheduled = false;
let criticalSectionOracleSentinel = null;
let criticalSectionOracleLastError = null;
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
let multiByteConsumerRecorded = false;
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
let getCpInfoOracleScheduled = false;
let getCpInfoOracleSentinel = null;
let getCpInfoOracleResult = null;
let getCpInfoOracleLastError = null;
let getCpInfoConsumerRecorded = false;
const observedMainModuleHeapAllocations = new Map();

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

function snapshotBoundedRawBytes(pointer, requestedBytes) {
  const maximumBytes = 64;
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
  const recordsConsumer =
    !multiByteConsumerRecorded &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    multiByteConsumerRecorded = true;
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
    ["CoInitialize", []],
    ["CoInitializeEx", []],
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
if (selectedObservations.has("get-cp-info")) {
  hooks["kernel32.dll"].push(["GetCPInfo", [null, null]]);
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
      const focusedHeapObservation =
        selectedObservations.has("heap-allocation") &&
        isFocusedHeapApi &&
        isMainModuleCaller;
      const focusedGetCpInfoObservation =
        selectedObservations.has("get-cp-info") &&
        name === "GetCPInfo" &&
        caller.module !== null &&
        (isMainModuleCaller || caller.module.toLowerCase() === "frida-agent.dll");
      const focusedMultiByteObservation =
        selectedObservations.has("multi-byte-to-wide-char") &&
        name === "MultiByteToWideChar" &&
        caller.module !== null &&
        (isMainModuleCaller || caller.module.toLowerCase() === "frida-agent.dll");
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
        selectedObservations.has("multi-byte-to-wide-char") &&
        name === "MultiByteToWideChar" &&
        !focusedMultiByteObservation
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
      this.traceGetCpInfoOracle =
        getCpInfoOracleSentinel !== null &&
        selectedObservations.has("get-cp-info") &&
        name === "GetCPInfo" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceGetCpInfoOracle) {
        this.lastError = getCpInfoOracleSentinel;
      }
      const arguments_ = decoders.map((decoder, index) =>
        decoder === null ? args[index].toString() : decodeArgument(args[index], decoder),
      );
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
      this.traceGetCpInfo =
        focusedGetCpInfoObservation
          ? { codePage: args[0], output: args[1] }
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
        : this.traceGetCpInfo !== null
          ? snapshotGetCpInfoCall(
              caller,
              this.returnAddress,
              this.lastError,
              this.context,
              this.traceGetCpInfo.codePage,
              this.traceGetCpInfo.output,
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
        : name === "GetStartupInfoW"
          ? { startupInfoBefore: snapshotStartupInfoW(args[0]) }
          : selectedObservations.has("critical-section") &&
              name.includes("CriticalSection")
            ? {
                lastErrorBefore: Number(this.lastError) >>> 0,
                criticalSectionBefore: snapshotCriticalSection(args[0]),
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
    },
    onLeave(result) {
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
      if (this.traceGetStringTypeOracle) {
        getStringTypeOracleResult = result.toUInt32();
        getStringTypeOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceLCMapStringOracle) {
        lcMapStringOracleResult = signed32(result);
        lcMapStringOracleLastError = Number(lastError) >>> 0;
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
      if (this.traceGetCpInfoOracle) {
        getCpInfoOracleResult = result.toUInt32();
        getCpInfoOracleLastError = Number(lastError) >>> 0;
      }
      const wideCharResult =
        this.traceWideCharConversion === null ? null : signed32(result);
      const multiByteResult =
        this.traceMultiByteConversion === null ? null : signed32(result);
      const lcMapStringResult =
        this.traceLCMapStringW === null ? null : signed32(result);
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
      };
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
      if (this.traceHeapAllocation !== null && !result.isNull()) {
        observedMainModuleHeapAllocations.set(result.toString().toLowerCase(), {
          callId: this.traceFrame.callId,
          requestedBytes: this.traceHeapAllocation.requestedBytes,
        });
      }
      if (this.traceHeapFree !== null && result.toUInt32() !== 0) {
        observedMainModuleHeapAllocations.delete(this.traceHeapFree.pointer);
      }
      if (name === "Direct3DCreate9" && selectedVtables.has("idirect3d9")) {
        attachIdirect3d9(result);
      }
    },
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
            932,
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
            codePage: 932,
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
      emit("multi-byte-to-wide-char-oracle", {
        profile: "x86-cp932-fixed-query-convert-v1",
        calls: [
          query,
          conversion,
          nulTerminatedQuery,
          insufficientBuffer,
          zeroSourceCount,
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
        profile: "x86-uncontended-recursive-v1",
        structureSize: 24,
        spinCount: 4000,
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
      selectedObservations.has("get-cp-info")
    ) {
      scheduleGetCpInfoOracle(module);
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
  },
});

emit("agent-ready", {
  architecture: Process.arch,
  pointerSize: Process.pointerSize,
  processId: Process.id,
  selectedVtables: [...selectedVtables],
  selectedObservations: [...selectedObservations],
});
