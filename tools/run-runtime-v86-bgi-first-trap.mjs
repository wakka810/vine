import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

import {
  distinctDispatchesThroughCheckpoint,
  loadRuntimeCheckpointManifest,
  selectRuntimeCheckpoint,
} from "./runtime-checkpoint-manifest.mjs";
import { observeArtifactIdentity } from "./evidence-classification.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const commandArguments = process.argv.slice(2);
const defaultManifestPath = path.join(
  root,
  "fixtures",
  "checkpoints",
  "bgi-startup-win32.json",
);
let checkpointId = "bgi-first-trap";
let manifestPath = defaultManifestPath;
let checkpointOptionSeen = false;
let manifestOptionSeen = false;
let genericRuntimeReentry = false;
let genericRuntimeCapability = null;
let runtimePolicyPath = null;
let runtimePolicyOptionSeen = false;
const positionalArguments = [];
for (let index = 0; index < commandArguments.length; index += 1) {
  const argument = commandArguments[index];
  if (argument === "--checkpoint") {
    assert(!checkpointOptionSeen, "--checkpoint may be specified only once");
    checkpointOptionSeen = true;
    checkpointId = commandArguments[index + 1];
    assert(checkpointId !== undefined, "--checkpoint requires an ID");
    index += 1;
  } else if (argument === "--manifest") {
    assert(!manifestOptionSeen, "--manifest may be specified only once");
    manifestOptionSeen = true;
    manifestPath = path.resolve(commandArguments[index + 1] ?? "");
    assert(commandArguments[index + 1] !== undefined, "--manifest requires a file");
    index += 1;
  } else if (argument === "--generic-runtime-reentry") {
    assert(!genericRuntimeReentry, "--generic-runtime-reentry may be specified only once");
    genericRuntimeReentry = true;
  } else if (argument === "--runtime-capability") {
    assert(genericRuntimeCapability === null, "--runtime-capability may be specified only once");
    genericRuntimeCapability = commandArguments[index + 1] ?? null;
    assert(
      [
        "kernel32-critical-section-synchronization",
        "kernel32-pointer-decoding",
        "kernel32-fiber-local-storage-allocation",
        "kernel32-private-heap-allocation",
        "kernel32-fiber-local-storage-value",
        "kernel32-interlocked-increment",
        "kernel32-private-heap-second-allocation",
        "kernel32-standard-input-handle",
        "kernel32-file-type",
        "kernel32-standard-output-handle",
        "kernel32-standard-output-file-type",
        "kernel32-standard-error-handle",
        "kernel32-standard-error-file-type",
        "kernel32-set-handle-count",
        "kernel32-command-line-a",
        "kernel32-environment-strings-w",
        "kernel32-wide-char-to-multi-byte",
        "kernel32-private-heap-size-1",
        "kernel32-get-last-error",
        "kernel32-tls-get-value",
        "kernel32-fls-get-value",
        "kernel32-set-last-error",
        "kernel32-get-acp",
        "kernel32-private-heap-size-544",
        "kernel32-get-cp-info",
        "kernel32-multi-byte-to-wide-char",
        "kernel32-get-string-type-w",
        "kernel32-lc-map-string-w",
        "kernel32-interlocked-decrement",
        "kernel32-get-module-file-name-a",
        "kernel32-private-heap-size-16",
        "kernel32-private-heap-size-4",
        "kernel32-private-heap-free-size-1",
        "kernel32-private-heap-size-128",
        "kernel32-is-processor-feature-present",
        "kernel32-set-unhandled-exception-filter",
        "kernel32-heap-size",
        "kernel32-initialize-critical-section",
      ].includes(genericRuntimeCapability),
      "unsupported --runtime-capability value",
    );
    index += 1;
  } else if (argument === "--runtime-policy") {
    assert(!runtimePolicyOptionSeen, "--runtime-policy may be specified only once");
    runtimePolicyOptionSeen = true;
    runtimePolicyPath = path.resolve(commandArguments[index + 1] ?? "");
    assert(commandArguments[index + 1] !== undefined, "--runtime-policy requires a file");
    index += 1;
  } else {
    assert(!argument.startsWith("--"), `unknown option ${argument}`);
    positionalArguments.push(argument);
  }
}
assert(
  positionalArguments.length <= 2 && positionalArguments.every((argument) => !argument.startsWith("--")),
  "usage: run-runtime-v86-bgi-first-trap.mjs [--manifest FILE] [--checkpoint ID] [--generic-runtime-reentry [--runtime-capability ID] [--runtime-policy FILE]] [WASM] [BGI.exe]",
);
assert(
  genericRuntimeCapability === null || genericRuntimeReentry,
  "--runtime-capability requires --generic-runtime-reentry",
);
assert(runtimePolicyPath === null || genericRuntimeReentry, "--runtime-policy requires --generic-runtime-reentry");
if (genericRuntimeReentry && runtimePolicyPath === null) {
  const policyName =
    genericRuntimeCapability === "kernel32-initialize-critical-section"
      ? "kernel32-initialize-critical-section-v1.json"
      : genericRuntimeCapability === "kernel32-heap-size"
      ? "kernel32-heap-size-v1.json"
      : genericRuntimeCapability === "kernel32-set-unhandled-exception-filter" ||
    genericRuntimeCapability === "kernel32-is-processor-feature-present"
      ? "kernel32-is-processor-feature-present-v1.json"
      : genericRuntimeCapability === "kernel32-private-heap-size-128"
      ? "kernel32-private-heap-size-128-v1.json"
      : genericRuntimeCapability === "kernel32-private-heap-free-size-1"
      ? "kernel32-private-heap-free-size-1-v1.json"
      : genericRuntimeCapability === "kernel32-private-heap-size-4"
      ? "kernel32-private-heap-size-4-v1.json"
      : genericRuntimeCapability === "kernel32-private-heap-size-16"
      ? "kernel32-private-heap-size-16-v1.json"
      : genericRuntimeCapability === "kernel32-get-module-file-name-a"
        ? "kernel32-get-module-file-name-a-v1.json"
      : genericRuntimeCapability === "kernel32-lc-map-string-w" ||
          genericRuntimeCapability === "kernel32-interlocked-decrement"
      ? "kernel32-lc-map-string-w-v1.json"
      : genericRuntimeCapability === "kernel32-get-string-type-w"
      ? "kernel32-get-string-type-w-v1.json"
      : genericRuntimeCapability === "kernel32-multi-byte-to-wide-char"
      ? "kernel32-multi-byte-to-wide-char-v1.json"
      : genericRuntimeCapability === "kernel32-get-cp-info"
      ? "kernel32-get-cp-info-v1.json"
      : genericRuntimeCapability === "kernel32-private-heap-size-544"
      ? "kernel32-private-heap-size-544-v1.json"
      : genericRuntimeCapability === "kernel32-get-acp" ||
    genericRuntimeCapability === "kernel32-set-last-error" ||
    genericRuntimeCapability === "kernel32-fls-get-value" ||
    genericRuntimeCapability === "kernel32-tls-get-value" ||
    genericRuntimeCapability === "kernel32-get-last-error" ||
    genericRuntimeCapability === "kernel32-private-heap-size-1"
      ? "kernel32-private-heap-size-1-v1.json"
      : genericRuntimeCapability === "kernel32-wide-char-to-multi-byte"
      ? "kernel32-wide-char-to-multi-byte-v1.json"
      : genericRuntimeCapability === "kernel32-environment-strings-w"
      ? "kernel32-environment-strings-w-v1.json"
      : genericRuntimeCapability === "kernel32-command-line-a"
      ? "kernel32-command-line-a-v1.json"
      : genericRuntimeCapability === "kernel32-set-handle-count"
      ? "kernel32-set-handle-count-v1.json"
      : genericRuntimeCapability === "kernel32-standard-error-file-type"
      ? "kernel32-standard-error-file-type-v1.json"
      : genericRuntimeCapability === "kernel32-standard-error-handle"
      ? "kernel32-standard-error-v1.json"
      : genericRuntimeCapability === "kernel32-standard-output-file-type"
      ? "kernel32-standard-output-file-type-v1.json"
      : genericRuntimeCapability === "kernel32-standard-output-handle"
      ? "kernel32-standard-output-v1.json"
      : [
            "kernel32-private-heap-second-allocation",
            "kernel32-standard-input-handle",
            "kernel32-file-type",
          ].includes(genericRuntimeCapability)
        ? "kernel32-heap-two-v1.json"
        : "kernel32-controlled-v1.json";
  runtimePolicyPath = path.join(root, "fixtures", "runtime-policies", policyName);
}
const runtimePolicy =
  runtimePolicyPath === null
    ? null
    : JSON.parse(fs.readFileSync(runtimePolicyPath, "utf8"));
if (runtimePolicy !== null) {
  assert.equal(runtimePolicy.schema, "vine.kernel32-compatibility-policy");
  assert.equal(runtimePolicy.schemaVersion, 1);
  assert.equal(typeof runtimePolicy.id, "string");
}
const checkpointManifest = loadRuntimeCheckpointManifest(manifestPath);
const selectedCheckpoint = selectRuntimeCheckpoint(checkpointManifest, checkpointId);
const includesCall = (callNumber) => selectedCheckpoint.supportedCallCount >= callNumber;
const dispatchSystemTime = includesCall(1);
const dispatchProcessId = includesCall(2);
const dispatchThreadId = includesCall(3);
const dispatchTickCount = includesCall(4);
const dispatchPerformanceCounter = includesCall(5);
const dispatchStartupInfo = includesCall(6);
const dispatchHeapSetInformation = includesCall(7);
const dispatchHeapCreate = includesCall(8);
const dispatchModuleHandle = includesCall(9);
const dispatchGetProcAddress = includesCall(10);
const dispatchGetProcAddressSecond = includesCall(11);
const dispatchGetProcAddressThird = includesCall(12);
const dispatchGetProcAddressFourth = includesCall(13);
const dispatchTlsAlloc = includesCall(14);
const dispatchTlsSetValue = includesCall(15);
const dispatchEncodePointer = includesCall(16);
const dispatchEncodePointerSecond = includesCall(17);
const initializeCriticalSectionEnabled =
  genericRuntimeCapability === "kernel32-initialize-critical-section";
const heapSizeEnabled =
  initializeCriticalSectionEnabled ||
  genericRuntimeCapability === "kernel32-heap-size";
const setUnhandledExceptionFilterEnabled =
  heapSizeEnabled ||
  genericRuntimeCapability === "kernel32-set-unhandled-exception-filter";
const isProcessorFeaturePresentEnabled =
  setUnhandledExceptionFilterEnabled ||
  genericRuntimeCapability === "kernel32-is-processor-feature-present";
const privateHeapSize128Enabled =
  isProcessorFeaturePresentEnabled ||
  genericRuntimeCapability === "kernel32-private-heap-size-128";
const privateHeapFreeSize1Enabled =
  privateHeapSize128Enabled ||
  genericRuntimeCapability === "kernel32-private-heap-free-size-1";
const privateHeapSize4Enabled =
  privateHeapFreeSize1Enabled ||
  genericRuntimeCapability === "kernel32-private-heap-size-4";
const privateHeapSize16Enabled =
  privateHeapSize4Enabled ||
  genericRuntimeCapability === "kernel32-private-heap-size-16";
const getModuleFileNameAEnabled =
  privateHeapSize16Enabled ||
  genericRuntimeCapability === "kernel32-get-module-file-name-a";
const interlockedDecrementEnabled =
  getModuleFileNameAEnabled ||
  genericRuntimeCapability === "kernel32-interlocked-decrement";
const lcMapStringWEnabled =
  interlockedDecrementEnabled ||
  genericRuntimeCapability === "kernel32-lc-map-string-w";
const getStringTypeWEnabled =
  lcMapStringWEnabled || genericRuntimeCapability === "kernel32-get-string-type-w";
const multiByteToWideCharEnabled =
  getStringTypeWEnabled ||
  genericRuntimeCapability === "kernel32-multi-byte-to-wide-char";
const getCpInfoEnabled =
  multiByteToWideCharEnabled || genericRuntimeCapability === "kernel32-get-cp-info";
const privateHeapSize544Enabled =
  getCpInfoEnabled || genericRuntimeCapability === "kernel32-private-heap-size-544";
const getAcpEnabled =
  privateHeapSize544Enabled || genericRuntimeCapability === "kernel32-get-acp";
const setLastErrorEnabled =
  getAcpEnabled || genericRuntimeCapability === "kernel32-set-last-error";
const flsGetValueEnabled =
  setLastErrorEnabled || genericRuntimeCapability === "kernel32-fls-get-value";
const tlsGetValueEnabled =
  flsGetValueEnabled || genericRuntimeCapability === "kernel32-tls-get-value";
const getLastErrorEnabled =
  tlsGetValueEnabled || genericRuntimeCapability === "kernel32-get-last-error";
const privateHeapSize1Enabled =
  getLastErrorEnabled || genericRuntimeCapability === "kernel32-private-heap-size-1";
const wideCharToMultiByteEnabled =
  privateHeapSize1Enabled ||
  genericRuntimeCapability === "kernel32-wide-char-to-multi-byte";
const environmentStringsWEnabled =
  wideCharToMultiByteEnabled ||
  genericRuntimeCapability === "kernel32-environment-strings-w";
const commandLineAEnabled =
  environmentStringsWEnabled ||
  genericRuntimeCapability === "kernel32-command-line-a";
const setHandleCountEnabled =
  commandLineAEnabled || genericRuntimeCapability === "kernel32-set-handle-count";
const standardErrorFileTypeEnabled =
  setHandleCountEnabled ||
  genericRuntimeCapability === "kernel32-standard-error-file-type";
const standardErrorHandleEnabled =
  standardErrorFileTypeEnabled ||
  genericRuntimeCapability === "kernel32-standard-error-handle";
const standardOutputFileTypeEnabled =
  standardErrorHandleEnabled ||
  genericRuntimeCapability === "kernel32-standard-output-file-type";
const standardOutputHandleEnabled =
  standardOutputFileTypeEnabled ||
  genericRuntimeCapability === "kernel32-standard-output-handle";
const fileTypeEnabled =
  standardOutputHandleEnabled ||
  genericRuntimeCapability === "kernel32-file-type";
const standardInputHandleEnabled =
  fileTypeEnabled ||
  genericRuntimeCapability === "kernel32-standard-input-handle";
const privateHeapSecondAllocationEnabled =
  standardInputHandleEnabled ||
  genericRuntimeCapability === "kernel32-private-heap-second-allocation";
const interlockedIncrementEnabled =
  privateHeapSecondAllocationEnabled ||
  genericRuntimeCapability === "kernel32-interlocked-increment";
const fiberLocalStorageValueEnabled =
  interlockedIncrementEnabled ||
  genericRuntimeCapability === "kernel32-fiber-local-storage-value";
const privateHeapAllocationEnabled =
  fiberLocalStorageValueEnabled ||
  genericRuntimeCapability === "kernel32-private-heap-allocation";
const fiberLocalStorageAllocationEnabled =
  privateHeapAllocationEnabled ||
  genericRuntimeCapability === "kernel32-fiber-local-storage-allocation";
const runtimeHeapPageAllowance = isProcessorFeaturePresentEnabled
  ? 8
  : Number(privateHeapAllocationEnabled);
const wasmPath = path.resolve(
  positionalArguments[0] ??
    path.join(root, ".work", "v86-adapter-source", "build", "v86-fallback.wasm"),
);
const bgiPath = path.resolve(
  positionalArguments[1] ??
    process.env.VINE_BGI_EXE ??
    "V:\\softs\\\u30b5\u30af\u30e9\u30ce\u8a69\\BGI.exe",
);
const outputDirectory = path.join(
  root,
  ".work",
  genericRuntimeCapability !== null
      ? initializeCriticalSectionEnabled
        ? "v86-bgi-initialize-critical-section"
        : heapSizeEnabled
        ? "v86-bgi-heap-size"
        : setUnhandledExceptionFilterEnabled
        ? "v86-bgi-set-unhandled-exception-filter"
        : isProcessorFeaturePresentEnabled
        ? "v86-bgi-is-processor-feature-present"
        : privateHeapSize128Enabled
        ? "v86-bgi-private-heap-size-128"
        : privateHeapFreeSize1Enabled
        ? "v86-bgi-private-heap-free-size-1"
        : privateHeapSize4Enabled
        ? "v86-bgi-private-heap-size-4"
        : privateHeapSize16Enabled
        ? "v86-bgi-private-heap-size-16"
        : getModuleFileNameAEnabled
        ? "v86-bgi-get-module-file-name-a"
        : interlockedDecrementEnabled
        ? "v86-bgi-interlocked-decrement"
        : lcMapStringWEnabled
      ? "v86-bgi-lc-map-string-w"
      : getStringTypeWEnabled
      ? "v86-bgi-get-string-type-w"
      : multiByteToWideCharEnabled
      ? "v86-bgi-multi-byte-to-wide-char"
      : getCpInfoEnabled
      ? "v86-bgi-get-cp-info"
      : privateHeapSize544Enabled
      ? "v86-bgi-private-heap-size-544"
      : getAcpEnabled
      ? "v86-bgi-get-acp"
      : setLastErrorEnabled
      ? "v86-bgi-set-last-error"
      : flsGetValueEnabled
      ? "v86-bgi-fls-get-value"
      : tlsGetValueEnabled
      ? "v86-bgi-tls-get-value"
      : getLastErrorEnabled
      ? "v86-bgi-get-last-error"
      : privateHeapSize1Enabled
      ? "v86-bgi-private-heap-size-1"
      : wideCharToMultiByteEnabled
      ? "v86-bgi-wide-char-to-multi-byte"
      : environmentStringsWEnabled
      ? "v86-bgi-environment-strings-w"
      : commandLineAEnabled
      ? "v86-bgi-command-line-a"
      : setHandleCountEnabled
      ? "v86-bgi-set-handle-count"
      : standardErrorFileTypeEnabled
      ? "v86-bgi-standard-error-file-type"
      : standardErrorHandleEnabled
      ? "v86-bgi-standard-error-handle"
      : standardOutputFileTypeEnabled
      ? "v86-bgi-standard-output-file-type"
      : standardOutputHandleEnabled
      ? "v86-bgi-standard-output-handle"
      : fileTypeEnabled
      ? "v86-bgi-file-type"
      : standardInputHandleEnabled
      ? "v86-bgi-standard-input-handle"
      : privateHeapSecondAllocationEnabled
      ? "v86-bgi-private-heap-second-allocation"
      : interlockedIncrementEnabled
      ? "v86-bgi-interlocked-increment"
      : fiberLocalStorageValueEnabled
      ? "v86-bgi-fiber-local-storage-value"
      : privateHeapAllocationEnabled
      ? "v86-bgi-private-heap-allocation"
      : fiberLocalStorageAllocationEnabled
      ? "v86-bgi-fiber-local-storage-allocation"
      : genericRuntimeCapability === "kernel32-pointer-decoding"
        ? "v86-bgi-pointer-decoding"
        : "v86-bgi-critical-section-synchronization"
    : genericRuntimeReentry
      ? "v86-bgi-generic-runtime-reentry"
    : selectedCheckpoint.artifactDirectory,
);
const reportPath = path.join(outputDirectory, "report.json");

const controlledClock = {
  owner: "checkpoint-host-clock",
  sampleId: "utc-2026-07-11T00:00:00.000Z",
  utcIso: "2026-07-11T00:00:00.000Z",
  fileTimeTicks: 134_282_016_000_000_000n,
  fileTimeLow: 929_513_472,
  fileTimeHigh: 31_264_968,
};
const controlledProcess = {
  owner: "checkpoint-process-identity",
  processId: 4_096,
};
const controlledThread = {
  owner: "checkpoint-thread-identity",
  threadId: 8_192,
};
const controlledMonotonicClock = {
  owner: "checkpoint-monotonic-clock",
  sampleId: "uptime-ms-3600000",
  pairedWallClockSampleId: controlledClock.sampleId,
  tickCount: 3_600_000,
};
const controlledPerformanceCounter = {
  owner: controlledMonotonicClock.owner,
  sampleId: "qpc-ticks-12345678901",
  pairedTickSampleId: controlledMonotonicClock.sampleId,
  ticks: 12_345_678_901n,
  low: 3_755_744_309,
  high: 2,
};
const controlledProcessStartup = {
  owner: "checkpoint-process-startup",
  profileId: "startupinfow-v68-zero-optional-fields",
  structureSize: 68,
  fields: {
    cb: 68,
    lpReserved: 0,
    lpDesktop: 0,
    lpTitle: 0,
    dwX: 0,
    dwY: 0,
    dwXSize: 0,
    dwYSize: 0,
    dwXCountChars: 0,
    dwYCountChars: 0,
    dwFillAttribute: 0,
    dwFlags: 0,
    wShowWindow: 0,
    cbReserved2: 0,
    lpReserved2: 0,
    hStdInput: 0,
    hStdOutput: 0,
    hStdError: 0,
  },
  nativeObservation: {
    source: "focused-frida-getstartupinfow-2026-07-12",
    callerRva: 0x000a36e7,
    structureSize: 68,
    inputBytesInitialized: false,
    reserved: "",
    desktop: "",
    titleRole: "absolute-image-path",
    flags: 0x00000100,
    standardHandles: "host-inherited-nonzero",
  },
};
const controlledHeapPolicy = {
  owner: "checkpoint-process-heap-policy",
  policyId: "heap-enable-termination-on-corruption",
  heapHandle: 0,
  informationClass: 1,
  informationAddress: 0,
  informationLength: 0,
  terminationOnCorruptionEnabled: true,
  preserveLastError: true,
  result: 1,
  nativeObservation: {
    source: "focused-frida-heapsetinformation-2026-07-12",
    callerRva: 0x000a36fc,
    arguments: [0, 1, 0, 0],
    result: 1,
    lastErrorBefore: 87,
    lastErrorAfter: 87,
  },
};
const controlledPrivateHeap = {
  owner: "checkpoint-process-private-heaps",
  variantId: "heapcreate-growable-initial-4096",
  handle: {
    namespace: "vine.logical-private-heap.v1",
    value: 0x68010001,
    typeTag: 0x68,
    generation: 1,
    slot: 1,
  },
  kind: "private-heap",
  lifecycle: "active",
  creationFlags: 0,
  initialSize: 0x1000,
  maximumSize: 0,
  growable: true,
  preserveLastError: true,
  backingMaterialization: "deferred-until-first-reached-heap-operation",
  liveAllocations: 0,
  nativeObservation: {
    source: "focused-frida-heap-lifecycle-2026-07-12",
    traceSha256: "9bdf000eedb8844a6e8db4732d025e7dd3a33854bea47175e26c5d5a3e678682",
    callerRva: 0x000a8886,
    arguments: [0, 0x1000, 0],
    resultRole: "host-process-heap-address-observed-and-discarded",
    lastErrorBefore: 0,
    lastErrorAfter: 0,
    firstConsumer: {
      api: "HeapAlloc",
      callerRva: 0x000ac3cc,
      flags: 0x8,
      bytes: 0x214,
    },
  },
};
const controlledLoadedModule = {
  owner: "checkpoint-loaded-modules",
  variantId: "getmodulehandlew-kernel32-loaded-runtime-module",
  handle: {
    namespace: "vine.logical-loaded-module.v1",
    value: 0x4d010001,
    typeTag: 0x4d,
    generation: 1,
    slot: 1,
  },
  kind: "loaded-module",
  lifecycle: "loaded",
  requestedName: "KERNEL32.DLL",
  canonicalName: "kernel32.dll",
  nameMatch: "unicode-case-insensitive-basename",
  moduleRole: "runtime-system-module",
  referenceCountChanged: false,
  guestImageMaterialization: "not-present",
  exportResolution: "deferred-until-first-reached-getprocaddress",
  preserveLastError: true,
  nativeObservation: {
    source: "focused-frida-module-handle-2026-07-12",
    traceBytes: 347_850,
    traceSha256: "bac37acf086ff691ba436b2959143c76894f20f5e271667813cefebeb5ee3557",
    callerRva: 0x000a5811,
    argument: "KERNEL32.DLL",
    resultRole: "host-system-module-base-observed-and-discarded",
    lastErrorBefore: 87,
    lastErrorAfter: 87,
    firstConsumer: {
      api: "GetProcAddress",
      callerRva: 0x000a582f,
      symbol: "FlsAlloc",
    },
  },
};
const controlledDynamicExport = {
  owner: "checkpoint-dynamic-exports",
  variantId: "getprocaddress-kernel32-flsalloc-runtime-endpoint",
  moduleHandle: controlledLoadedModule.handle,
  canonicalModule: "kernel32.dll",
  lookupKind: "name",
  callVariant: "mov-esi-iat-push-name-push-edi-call-esi",
  requestedName: "FlsAlloc",
  resolvedName: "FlsAlloc",
  endpoint: {
    namespace: "vine.dynamic-export-endpoint.v1",
    id: 1,
    address: 0x7f020000,
    owner: 9,
    arenaRange: { start: 0x7f020000, byteLength: 0x10000 },
    committedPages: 1,
    slotSize: 0x10,
    module: "kernel32.dll",
    name: "FlsAlloc",
    lifetime: "process-runtime",
    contentSha256: dispatchGetProcAddressFourth
      ? "82ff4f595efb66795e9086158d940bdfb0f8e30a73d6d16879d599eb5afab615"
      : dispatchGetProcAddressThird
      ? "3b6345284f0929d025fc45cb9ae5f1d720c7575ec110cb41e7fbcaaa2fad2439"
      : dispatchGetProcAddressSecond
      ? "bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc"
      : "4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059",
    ownerSealed: true,
    invalidation: {
      generation: 27,
      dataTlb: true,
      instructionFetch: true,
      generatedCode: true,
    },
  },
  preserveLastError: true,
  hostAddressImported: false,
  nativeObservation: {
    source: "focused-frida-dynamic-export-2026-07-12",
    traceBytes: 320_917,
    traceSha256: "5b94c823da26761172ff7cc2e8b867b7fc8661046bbc6612ddbaff613161cdfe",
    callerRva: 0x000a582f,
    module: "KERNEL32.DLL",
    symbol: "FlsAlloc",
    resultRole: "host-function-address-observed-and-discarded",
    lastErrorBefore: 87,
    lastErrorAfter: 87,
    followingLookups: [
      { callerRva: 0x000a583c, symbol: "FlsGetValue" },
      { callerRva: 0x000a5849, symbol: "FlsSetValue" },
      { callerRva: 0x000a5856, symbol: "FlsFree" },
    ],
  },
};
const controlledSecondDynamicExport = {
  owner: controlledDynamicExport.owner,
  variantId: "getprocaddress-kernel32-flsgetvalue-runtime-endpoint",
  moduleHandle: controlledLoadedModule.handle,
  canonicalModule: "kernel32.dll",
  lookupKind: "name",
  callVariant: "push-name-push-edi-store-prior-result-call-esi",
  indirectCallRegister: "esi",
  requestedName: "FlsGetValue",
  resolvedName: "FlsGetValue",
  firstEndpointStoreAddress: 0x100feebc,
  endpointStoreAddress: 0x100feec0,
  endpoint: {
    ...controlledDynamicExport.endpoint,
    id: 2,
    address: 0x7f020010,
    name: "FlsGetValue",
  },
  preserveLastError: true,
  hostAddressImported: false,
  nativeObservation: {
    ...controlledDynamicExport.nativeObservation,
    callerRva: 0x000a583c,
    symbol: "FlsGetValue",
    priorLookup: { callerRva: 0x000a582f, symbol: "FlsAlloc" },
    followingLookups: [
      { callerRva: 0x000a5849, symbol: "FlsSetValue" },
      { callerRva: 0x000a5856, symbol: "FlsFree" },
    ],
  },
};
const controlledThirdDynamicExport = {
  owner: controlledDynamicExport.owner,
  variantId: "getprocaddress-kernel32-flssetvalue-runtime-endpoint",
  moduleHandle: controlledLoadedModule.handle,
  canonicalModule: "kernel32.dll",
  lookupKind: "name",
  callVariant: "push-name-push-edi-store-prior-result-call-esi",
  indirectCallRegister: "esi",
  requestedName: "FlsSetValue",
  resolvedName: "FlsSetValue",
  firstEndpointStoreAddress: controlledSecondDynamicExport.firstEndpointStoreAddress,
  secondEndpointStoreAddress: controlledSecondDynamicExport.endpointStoreAddress,
  endpointStoreAddress: 0x100feec4,
  endpoint: {
    ...controlledDynamicExport.endpoint,
    id: 3,
    address: 0x7f020020,
    name: "FlsSetValue",
  },
  preserveLastError: true,
  hostAddressImported: false,
  nativeObservation: {
    ...controlledDynamicExport.nativeObservation,
    callerRva: 0x000a5849,
    symbol: "FlsSetValue",
    priorLookups: [
      { callerRva: 0x000a582f, symbol: "FlsAlloc" },
      { callerRva: 0x000a583c, symbol: "FlsGetValue" },
    ],
    followingLookups: [{ callerRva: 0x000a5856, symbol: "FlsFree" }],
  },
};
const controlledFourthDynamicExport = {
  owner: controlledDynamicExport.owner,
  variantId: "getprocaddress-kernel32-flsfree-runtime-endpoint",
  moduleHandle: controlledLoadedModule.handle,
  canonicalModule: "kernel32.dll",
  lookupKind: "name",
  callVariant: "push-name-push-edi-store-prior-result-call-esi",
  indirectCallRegister: "esi",
  requestedName: "FlsFree",
  resolvedName: "FlsFree",
  firstEndpointStoreAddress: controlledThirdDynamicExport.firstEndpointStoreAddress,
  secondEndpointStoreAddress: controlledThirdDynamicExport.secondEndpointStoreAddress,
  thirdEndpointStoreAddress: controlledThirdDynamicExport.endpointStoreAddress,
  endpointStoreAddress: 0x100feec8,
  endpoint: {
    ...controlledDynamicExport.endpoint,
    id: 4,
    address: 0x7f020030,
    name: "FlsFree",
  },
  preserveLastError: true,
  hostAddressImported: false,
  nativeObservation: {
    ...controlledDynamicExport.nativeObservation,
    callerRva: 0x000a5856,
    symbol: "FlsFree",
    priorLookups: [
      { callerRva: 0x000a582f, symbol: "FlsAlloc" },
      { callerRva: 0x000a583c, symbol: "FlsGetValue" },
      { callerRva: 0x000a5849, symbol: "FlsSetValue" },
    ],
    followingLookups: [],
  },
};
const controlledTlsAllocation = {
  owner: "checkpoint-thread-local-storage",
  variantId: "tlsalloc-first-controlled-index",
  index: {
    namespace: "vine.logical-tls-index.v1",
    value: 0,
    failureSentinel: 0xffff_ffff,
  },
  lifecycle: "allocated",
  storageScope: "thread",
  initializedThreadValues: 0,
  backingMaterialization: "metadata-only-until-first-reached-tls-operation",
  preserveLastError: true,
  nativeObservation: {
    source: "focused-frida-tls-lifecycle-2026-07-12",
    traceBytes: 6_138_442,
    traceLines: 19_816,
    traceSha256: "214ca138648d6674860ba3fc43e18c017470f0b31e023f1e7e9e4afa88dae08b",
    confirmationTraceBytes: 62_273,
    confirmationTraceLines: 223,
    confirmationTraceSha256:
      "fd23eb23fca34953216b0a1cf11c5ce4d88959618c5bc1173b58eab5199ad30e",
    callerRva: 0x000a58aa,
    arguments: [],
    nativeResult: 0x1c,
    nativeResultImported: false,
    lastErrorBefore: 87,
    lastErrorAfter: 87,
    firstConsumer: {
      api: "TlsSetValue",
      callerRva: 0x000a58c1,
      indexRole: "native-index-observed-and-discarded",
      valueRole: "native-FlsGetValue-address-observed-and-discarded",
      result: 1,
      lastErrorBefore: 87,
      lastErrorAfter: 87,
    },
  },
};
const controlledTlsValue = {
  owner: controlledTlsAllocation.owner,
  variantId: "tlssetvalue-initial-thread-flsgetvalue-endpoint",
  allocationVariantId: controlledTlsAllocation.variantId,
  threadIdentity: controlledThread,
  index: controlledTlsAllocation.index,
  valueKind: "dynamic-export-endpoint",
  endpoint: controlledSecondDynamicExport.endpoint,
  preserveLastError: true,
  result: 1,
  nativeObservation: {
    source: controlledTlsAllocation.nativeObservation.source,
    traceSha256: controlledTlsAllocation.nativeObservation.traceSha256,
    confirmationTraceSha256:
      controlledTlsAllocation.nativeObservation.confirmationTraceSha256,
    callerRva: 0x000a58c1,
    nativeArguments: [0x1c, 0x758d33f0],
    controlledArguments: [
      controlledTlsAllocation.index.value,
      controlledSecondDynamicExport.endpoint.address,
    ],
    nativeArgumentsImported: false,
    result: 1,
    lastErrorBefore: 87,
    lastErrorAfter: 87,
  },
};
const controlledEncodedNull = {
  owner: "checkpoint-pointer-encoding",
  variantId: "encodepointer-null-process-token",
  namespace: "vine.logical-encoded-pointer.v1",
  value: 0xec010001,
  sourceKind: "null",
  sourcePointer: 0,
  lifetime: "process-runtime",
  processCookieMaterialization: "not-imported",
  nativeResultImported: false,
  preserveLastError: true,
  nativeObservation: {
    source: "focused-frida-pointer-encoding-2026-07-12",
    traceBytes: 2_213_858,
    traceLines: 6_979,
    traceSha256: "71a79c89aa9511cf0eca37c952d24130765212705ab34be08de56b1529045fa3",
    confirmationTraceBytes: 312_314,
    confirmationTraceLines: 1_040,
    confirmationTraceSha256:
      "4309d18d16ad3dc6b5390df421d7edc4137c712221cea65378b91774e31ef04c",
    callerRva: 0x000a5467,
    arguments: [0],
    nativeResults: [0x6f713c58, 0x765b8b70],
    nativeResultRole: "process-cookie-dependent-values-observed-and-discarded",
    lastErrorSamples: [
      { before: 0, after: 0 },
      { before: 87, after: 87 },
    ],
    firstConsumer: {
      wrapperReturnRva: 0x000a4fd6,
      operation: "mov-esi-eax-and-pass-as-initialization-sentinel",
    },
  },
};
const controlledEncodedProcessInitializer = {
  owner: "checkpoint-pointer-encoding",
  variantId: "encodepointer-process-initializer-function-token",
  namespace: "vine.logical-encoded-pointer.v1",
  value: 0xec010002,
  sourceKind: "guest-executable-function",
  sourcePointer: 0x100a9608,
  sourcePageAddress: 0x100a9000,
  sourceFunctionPrefix: [
    0x6a, 0x08, 0x68, 0xf8, 0xb2, 0x0e, 0x10, 0xe8, 0x5c, 0xbc, 0xff, 0xff, 0xe8, 0x33,
    0xc0, 0xff, 0xff,
  ],
  sourceRole: "process-initializer-guest-function",
  encodedStoreAddress: 0x100ff62c,
  lifetime: "process-runtime",
  processCookieMaterialization: "not-imported",
  nativeResultImported: false,
  preserveLastError: true,
  nativeObservation: {
    source: "focused-frida-pointer-encoding-2026-07-12",
    traceBytes: 2_213_858,
    traceLines: 6_979,
    traceSha256: "71a79c89aa9511cf0eca37c952d24130765212705ab34be08de56b1529045fa3",
    confirmationTraceBytes: 312_314,
    confirmationTraceLines: 1_040,
    confirmationTraceSha256:
      "4309d18d16ad3dc6b5390df421d7edc4137c712221cea65378b91774e31ef04c",
    callerRva: 0x000a9697,
    sourceRva: 0x000a9608,
    nativeArguments: [0x003a9608, 0x003a9608],
    nativeResults: [0x6f766e99, 0x71094a70],
    nativeResultRole: "process-cookie-dependent-values-observed-and-discarded",
    lastErrorSamples: [
      { before: 0, after: 0 },
      { before: 87, after: 87 },
    ],
    firstConsumer: {
      instructionRva: 0x000a9697,
      operation: "store-encoded-result-to-process-global",
      destinationRva: 0x000ff62c,
    },
    downstreamConsumer: {
      decodeCallRva: 0x000a966c,
      indirectCallRva: 0x000a9674,
      operation: "decode-global-and-call-process-initializer",
    },
  },
};

const declaredDispatches = distinctDispatchesThroughCheckpoint(
  checkpointManifest,
  selectedCheckpoint,
);
const getProcAddressCallCount = checkpointManifest.checkpoints
  .slice(1, selectedCheckpoint.supportedCallCount + 1)
  .filter(
    ({ dispatch }) =>
      dispatch.module.toLowerCase() === "kernel32.dll" &&
      dispatch.symbol === "GetProcAddress",
  ).length;

const expected = {
  source: checkpointManifest.source,
  wasm: checkpointManifest.adapter,
  image: {
    owner: 1,
    loadBase: 0x10000000,
    entry: 0x100a383e,
    pages: 380,
    readablePages: 380,
    writablePages: 100,
    executablePages: 208,
  },
  imports: {
    owner: 2,
    pages: 2,
    modules: 14,
    total: 330,
    declared: 1 + declaredDispatches.length,
    unsupported: 329 - declaredDispatches.length,
    first: {
      id: 1,
      address: 0x7f000000,
      iatAddress: 0x100d14f4,
      module: "d3d9.dll",
      name: "Direct3DCreate9",
    },
    second: {
      id: 2,
      address: 0x7f000010,
      iatAddress: 0x100d1050,
      module: "DINPUT8.dll",
      name: "DirectInput8Create",
    },
    last: {
      id: 330,
      address: 0x7f001490,
      iatAddress: 0x100d1058,
      module: "DSOUND.dll",
      ordinal: 11,
    },
  },
  control: {
    owner: 3,
    page: 0x7f010000,
    callbackId: 1,
    callbackAddress: 0x7f010000,
    processEntryId: 2,
    processEntryAddress: 0x7f010010,
  },
  stack: {
    owner: 4,
    base: 0x006ff000,
    top: 0x00700000,
    initialEsp: 0x006ffffc,
  },
  processEnvironment: {
    tebOwner: 5,
    pebOwner: 6,
    processParametersOwner: 7,
    commandLineOwner: 8,
    tebBase: 0x7ffde000,
    pebBase: 0x7ffdf000,
    processParametersBase: 0x7ffdc000,
    commandLineBase: 0x7ffdd000,
    commandLine: "BGI.exe",
    commandLineLengthBytes: 14,
    commandLineMaximumLengthBytes: 16,
    tebContentSha256: "5d4ad643f294d6976304f87d2912ec4a0b1f774fae1fd9a94de59749bfdcd67e",
    pebContentSha256: "780207feb049a4c658816bf0c986bb386ec04214722a21c47936cdec01ee4d39",
    processParametersContentSha256:
      environmentStringsWEnabled
        ? "ffbaed76ceb380c4240a04d838f0ce068c93fca8b0951a895ade87e863981625"
        : "ac17565d250da1d27dc2429b2c81bd242fe26d67373495b2238eadbb762b6d33",
    commandLineContentSha256:
      environmentStringsWEnabled
        ? "99c221425a6c0f4a91e97aaecebe7f078007be033c9a6d81f4444286579ce121"
        : commandLineAEnabled
        ? "99c221425a6c0f4a91e97aaecebe7f078007be033c9a6d81f4444286579ce121"
        : "c9b83dbe193708a3418517c43a4dadd26ca50a4006b00bd11701f3bb6ef0e6cc",
    ansiBuffer: 0x7ffdd010,
    ansiByteLengthWithNul: 8,
    ansiEncoding: "ascii-v1",
    environmentAuthorityBuffer: 0x7ffdd020,
    environmentCopyBuffer: 0x7ffdd030,
    environmentByteLengthWithDoubleNul: 4,
    environmentEncoding: "utf16le-double-nul-v1",
    generation: 24,
  },
  dynamicExport: controlledDynamicExport.endpoint,
  dynamicExports: [
    controlledDynamicExport.endpoint,
    controlledSecondDynamicExport.endpoint,
    controlledThirdDynamicExport.endpoint,
    controlledFourthDynamicExport.endpoint,
  ].slice(0, Math.max(2, getProcAddressCallCount)),
  memory: {
    owners: dispatchGetProcAddress ? 9 : 8,
    reservedPages: dispatchGetProcAddress ? 433 : 417,
    reflectedPages: dispatchGetProcAddress ? 389 : 388,
    generation: dispatchGetProcAddress ? 27 : 24,
  },
  reflectionContentSha256: commandLineAEnabled
    ? environmentStringsWEnabled
      ? "cdba586268a16e9f45dbfc562dc00fec117cf7c5307629ba3237ca4f293a4a55"
      : "79b095d59577cb70afcd654328c5a014a3c7f99b5832fc4f7819aef43eef94be"
    : selectedCheckpoint.evidence.reflectionSha256,
  offerLineBytes: commandLineAEnabled
    ? environmentStringsWEnabled
      ? 4_923_170
      : 4_923_010
    : selectedCheckpoint.evidence.offerLineBytes,
  receiptLineBytes: selectedCheckpoint.evidence.reflectionReceiptLineBytes,
  terminal: {
    thunk: {
      id: 104,
      address: 0x7f000670,
      iatAddress: 0x100d1248,
      module: "KERNEL32.dll",
      name: "GetSystemTimeAsFileTime",
    },
    instructionCount: 19,
    executeStops: 19,
    budgetStops: 18,
    returnAddress: 0x100a88d2,
    stackWords: [0x100a88d2, 0x006fffec, 0, 0, 0, 0, 0, 0],
  },
  firstCall: {
    requestLineBytes: 8_670,
    deltaLineBytes: 9_584,
    deltaReceiptLineBytes: 361,
    deltaGeneration: dispatchGetProcAddress ? 28 : 25,
    stackPageContentSha256: "46d99e572b1fb4f18aeb340362f31a5264cfc745cb01d22e32bc00bbb19dc785",
    reconciledChangedBytes: 11,
    deltaContentSha256: "dfad2cdbcc57bb6c05b66962dc185dbb6139a458f4da3e725536168474653840",
    nextThunk: {
      identity: {
        id: 103,
        address: 0x7f000660,
        iatAddress: 0x100d1244,
        module: "KERNEL32.dll",
        name: "GetCurrentProcessId",
      },
      unsupported: true,
      returnAddress: 0x100a88de,
      stackWords: [0x100a88de, 0, 0, 0, 0, 0, 929_513_472, 31_264_968],
    },
    instructionCount: 23,
    executeStops: 4,
    budgetStops: 3,
  },
  secondCall: {
    requestLineBytes: 8_552,
    receiptLineBytes: 845,
    stackPageContentSha256: "5aa539018b6830e5fb6ebdd82880174b37e6edd05f6933008b68d6d994c8223f",
    reconciledChangedBytes: 4,
    nextThunk: {
      identity: {
        id: 149,
        address: 0x7f000940,
        iatAddress: 0x100d12fc,
        module: "KERNEL32.dll",
        name: "GetCurrentThreadId",
      },
      unsupported: true,
      returnAddress: 0x100a88e6,
      stackWords: [0x100a88e6, 0, 0, 0, 0, 0, 929_513_472, 31_264_968],
    },
    instructionCount: 26,
    executeStops: 3,
    budgetStops: 2,
  },
  thirdCall: {
    requestLineBytes: 8_548,
    receiptLineBytes: 840,
    stackPageContentSha256: "b22f529af561901c5156e388732e0e79954527b3650fc733ae5b780319791617",
    reconciledChangedBytes: 1,
    nextThunk: {
      identity: {
        id: 13,
        address: 0x7f0000c0,
        iatAddress: 0x100d10dc,
        module: "KERNEL32.dll",
        name: "GetTickCount",
      },
      unsupported: true,
      returnAddress: 0x100a88ee,
      stackWords: [0x100a88ee, 0, 0, 0, 0, 0, 929_513_472, 31_264_968],
    },
    instructionCount: 29,
    executeStops: 3,
    budgetStops: 2,
  },
  fourthCall: {
    requestLineBytes: 8_640,
    receiptLineBytes: 926,
    stackPageContentSha256: "f70a02d880af738aa49ba1233b61151929abf67f5e370e3479858c20f312453b",
    reconciledChangedBytes: 1,
    nextThunk: {
      identity: {
        id: 157,
        address: 0x7f0009c0,
        iatAddress: 0x100d131c,
        module: "KERNEL32.dll",
        name: "QueryPerformanceCounter",
      },
      unsupported: true,
      returnAddress: 0x100a88fa,
      stackWords: [0x100a88fa, 0x006fffe4, 0, 0, 0, 0, 0, 929_513_472],
    },
    instructionCount: 34,
    executeStops: 5,
    budgetStops: 4,
  },
  fifthCall: {
    requestLineBytes: 8_681,
    deltaLineBytes: 9_583,
    deltaReceiptLineBytes: 366,
    stackPageContentSha256: "a581c7ce28aa0c683d1b0489e93fbe29d24924e35cd2abe3e966d2fc7c65c4bc",
    reconciledChangedBytes: 5,
    deltaGeneration: dispatchGetProcAddress ? 29 : 26,
    deltaContentSha256: "e466741f272179ad06d4777d23b91fa11b4fa526a47063c0adfa62c842c3d951",
    nextThunk: {
      identity: {
        id: 83,
        address: 0x7f000520,
        iatAddress: 0x100d11f4,
        module: "KERNEL32.dll",
        name: "GetStartupInfoW",
      },
      unsupported: !dispatchStartupInfo,
      returnAddress: 0x100a36e7,
      stackWords: [0x100a36e7, 0x006fff90, 0xe93f6d87, 0, 0, 0, 0, 0],
    },
    instructionCount: 78,
    executeStops: 44,
    budgetStops: 43,
  },
  sixthCall: {
    requestLineBytes: 8_663,
    deltaLineBytes: 9_936,
    deltaReceiptLineBytes: 474,
    outputBeforeSha256: "5fa35fdf26ef33a7361c44c3dabdb80cee7c2a468bf39e1ff0d2188e53b48aed",
    outputBytesSha256: "9a577f8dadf13d9197f2f83feebcf47567472ab68625099b4e7bcde0fef18653",
    stackPageContentSha256: "75e2211e3e9a79f92313319d9897bb1e868d74bb15f0dc513cc9cb393c9bb843",
    reconciledChangedBytes: 31,
    deltaGeneration: dispatchGetProcAddress ? 30 : 27,
    deltaContentSha256: "a2c82c133e41916b0b9af10314376ece9a281a8503db4b811031e5ce52919de8",
    nextThunk: {
      identity: {
        id: 84,
        address: 0x7f000530,
        iatAddress: 0x100d11f8,
        module: "KERNEL32.dll",
        name: "HeapSetInformation",
      },
      unsupported: !dispatchHeapSetInformation,
      returnAddress: 0x100a36fc,
      stackWords: [0x100a36fc, 0, 1, 0, 0, 0xe93f6d87, 0, 0],
    },
    instructionCount: 87,
    executeStops: 9,
    budgetStops: 8,
  },
  seventhCall: {
    requestLineBytes: 8_724,
    receiptLineBytes: 1_042,
    stackPageContentSha256: "d121a898c549de8cf2eea5d76912375a993cac9a7dfb41d101ef2eb2b68fa3da",
    reconciledChangedBytes: 12,
    nextThunk: {
      identity: {
        id: 102,
        address: 0x7f000650,
        iatAddress: 0x100d1240,
        module: "KERNEL32.dll",
        name: "HeapCreate",
      },
      unsupported: !dispatchHeapCreate,
      returnAddress: 0x100a8886,
      stackWords: [0x100a8886, 0, 0x1000, 0, 0x100a374a, 0xe93f6d87, 0, 0],
    },
    instructionCount: 108,
    executeStops: 21,
    budgetStops: 20,
  },
  eighthCall: {
    requestLineBytes: 8_691,
    receiptLineBytes: 1_233,
    stackPageContentSha256: "48f30a675492096d7110c249c8f8bc35da1b58ab3db07df9553e168e56e05d93",
    reconciledChangedBytes: 8,
    nextThunk: {
      identity: {
        id: 72,
        address: 0x7f000470,
        iatAddress: 0x100d11c8,
        module: "KERNEL32.dll",
        name: "GetModuleHandleW",
      },
      unsupported: !dispatchModuleHandle,
      returnAddress: 0x100a5811,
      stackWords: [0x100a5811, 0x100d24d0, 0, 0x100a375b, 0xe93f6d87, 0, 0, 0],
    },
    instructionCount: 122,
    executeStops: 14,
    budgetStops: 13,
  },
  ninthCall: {
    requestLineBytes: 8_721,
    receiptLineBytes: 1_499,
    moduleNameBytesSha256: "4d40ac1582e46f291dedb30130e36b167bbe19b401a248b1972b1bfa7391d924",
    stackPageContentSha256: "2d08da9d16a6782982e4b2b9e4617909c4ec2a65fbc1702479972ad4618f4b2e",
    reconciledChangedBytes: 9,
    nextThunk: {
      identity: {
        id: 150,
        address: 0x7f000950,
        iatAddress: 0x100d1300,
        module: "KERNEL32.dll",
        name: "GetProcAddress",
      },
      unsupported: !dispatchGetProcAddress,
      returnAddress: 0x100a582f,
      stackWords: [0x100a582f, 0x4d010001, 0x100d250c, 0, 0, 0x100a375b, 0xe93f6d87, 0],
    },
    instructionCount: 131,
    executeStops: 9,
    budgetStops: 8,
  },
  tenthCall: {
    requestLineBytes: 8_821,
    receiptLineBytes: 1_872,
    procedureNameBytesSha256: "574f9b2cb1c509be66aa90427c3250a3249a19c7a243b86f4ab13b4040abbd1e",
    stackPageContentSha256: "0ec9e7bed9bb310bdab5ae447581656314c744cf4d8fe2104a36d9fe297215c7",
    reconciledChangedBytes: 15,
    nextThunk: {
      identity: {
        id: 150,
        address: 0x7f000950,
        iatAddress: 0x100d1300,
        module: "KERNEL32.dll",
        name: "GetProcAddress",
      },
      unsupported: false,
      returnAddress: 0x100a583c,
      stackWords: [0x100a583c, 0x4d010001, 0x100d2500, 0, 0, 0x100a375b, 0xe93f6d87, 0],
    },
    instructionCount: 136,
    executeStops: 5,
    budgetStops: 4,
  },
  eleventhCall: {
    requestLineBytes: 17_273,
    receiptLineBytes: 2_704,
    procedureNameBytesSha256: "51598ab3ab558b58b22deb49515346e61d997d79ab4215fd040ce7125ea0818f",
    stackPageContentSha256: "5a4cdc83d919e8a3dfc1f0617bde34c9fc0f7eb3cee2a6597aa9f22e8e7e67ef",
    stackReconciledChangedBytes: 2,
    firstEndpointStorePageContentSha256: "098e6d79168e51989225bbab5e1c59d889363412c53b29d7478a0ad4174101f6",
    firstEndpointStoreReconciledChangedBytes: 2,
    nextThunk: {
      identity: {
        id: 150,
        address: 0x7f000950,
        iatAddress: 0x100d1300,
        module: "KERNEL32.dll",
        name: "GetProcAddress",
      },
      unsupported: false,
      returnAddress: 0x100a5849,
      stackWords: [0x100a5849, 0x4d010001, 0x100d24f4, 0, 0, 0x100a375b, 0xe93f6d87, 0],
    },
    instructionCount: 141,
    executeStops: 5,
    budgetStops: 4,
  },
  twelfthCall: {
    requestLineBytes: 17_336,
    receiptLineBytes: 3_233,
    procedureNameBytesSha256: "24e391b36bf5f29d67cdb94094f5662081c98677e63407f52f1dc1e924034c7d",
    stackPageContentSha256: "c4f6754b2889228cfd3f7382adff164a4c9bb8c9722ac1f5f0a38e03471a9286",
    stackReconciledChangedBytes: 3,
    endpointStorePageContentSha256: "1cb318f989df8cbf93f59a84f1f5ab9b62d57f3237f7e7162c8df79456ca9d68",
    endpointStoreReconciledChangedBytes: 3,
    nextThunk: {
      identity: {
        id: 150,
        address: 0x7f000950,
        iatAddress: 0x100d1300,
        module: "KERNEL32.dll",
        name: "GetProcAddress",
      },
      unsupported: false,
      returnAddress: 0x100a5856,
      stackWords: [0x100a5856, 0x4d010001, 0x100d24ec, 0, 0, 0x100a375b, 0xe93f6d87, 0],
    },
    instructionCount: 146,
    executeStops: 5,
    budgetStops: 4,
  },
  thirteenthCall: {
    requestLineBytes: 17_404,
    receiptLineBytes: 3_736,
    procedureNameBytesSha256: "b25da59ca0b82d9b4b8ddb0a22566cbce73af231e364ed982046c841b26c8dd8",
    stackPageContentSha256: "ecbad1e057c0e424b1bdc8c863bd9a4ae85d77f6bee312ce7ea02704160dedf8",
    stackReconciledChangedBytes: 2,
    endpointStorePageContentSha256: "c166a29b61c167001046de381b7dd5ba238ef5e5694021a152d626c9024ca7d0",
    endpointStoreReconciledChangedBytes: 3,
    nextTerminal: {
      kind: "owned-import-thunk",
      thunk: {
        id: 70,
        address: 0x7f000450,
        requesterOwner: 1,
        module: "KERNEL32.dll",
        iatRva: 0x000d11c0,
        iatAddress: 0x100d11c0,
        originalIatValue: 0x000efb12,
        disposition: "unsupported-trap",
        kind: "name",
        name: "TlsAlloc",
        hint: 1221,
        nameRva: 0x000efb12,
      },
      returnAddress: 0x100a58aa,
      stackWords: [0x100a58aa, 0, 0, 0x100a375b, 0xe93f6d87, 0, 0, 0],
    },
    instructionCount: 158,
    executeStops: 12,
    budgetStops: 11,
  },
  fourteenthCall: {
    requestLineBytes: 17_318,
    receiptLineBytes: 1_651,
    stackPageContentSha256: "3595f775ff05d8b86348f1ddc3f931cc11b2bd9e769af5f012919daa888f91da",
    stackReconciledChangedBytes: 3,
    endpointStorePageContentSha256:
      "705c675572bfe882af9addb23e20270278161fb8a8c7f4360afca09b0fbf96f8",
    endpointStoreReconciledChangedBytes: 3,
    nextTerminal: {
      kind: "owned-import-thunk",
      thunk: {
        id: 68,
        address: 0x7f000430,
        requesterOwner: 1,
        module: "KERNEL32.dll",
        iatRva: 0x000d11b8,
        iatAddress: 0x100d11b8,
        originalIatValue: 0x000efb2c,
        disposition: "unsupported-trap",
        kind: "name",
        name: "TlsSetValue",
        hint: 1224,
        nameRva: 0x000efb2c,
      },
      returnAddress: 0x100a58c1,
      stackWords: [0x100a58c1, 0, 0x7f020010, 0, 0, 0x100a375b, 0xe93f6d87, 0],
    },
    instructionCount: 165,
    executeStops: 7,
    budgetStops: 6,
  },
  fifteenthCall: {
    requestLineBytes: 28_349,
    receiptLineBytes: 2_733,
    stackPageContentSha256: "0f62e02ebf62129f93e1bbad8e4b2f2739783536fd70af2df31a7cd5ef3accec",
    stackReconciledChangedBytes: 8,
    endpointStorePageContentSha256:
      "705c675572bfe882af9addb23e20270278161fb8a8c7f4360afca09b0fbf96f8",
    endpointStoreReconciledChangedBytes: 0,
    tlsIndexStorePageContentSha256:
      "b25cb15bb316bf0e716d70f9503a982c60ed5189eeb903b29d602141896d3c2c",
    tlsIndexStoreReconciledChangedBytes: 12,
    nextTerminal: {
      kind: "owned-import-thunk",
      thunk: {
        id: 89,
        address: 0x7f000580,
        requesterOwner: 1,
        module: "KERNEL32.dll",
        iatRva: 0x000d120c,
        iatAddress: 0x100d120c,
        originalIatValue: 0x000ef9b0,
        disposition: "unsupported-trap",
        kind: "name",
        name: "EncodePointer",
        hint: 234,
        nameRva: 0x000ef9b0,
      },
      returnAddress: 0x100a5467,
      stackWords: [0x100a5467, 0, 0x100a4fd6, 0x7f000430, 0x100a58ce, 0, 0, 0x100a375b],
    },
    instructionCount: 174,
    executeStops: 9,
    budgetStops: 8,
  },
  sixteenthCall: {
    requestLineBytes: 8_848,
    receiptLineBytes: 2_118,
    stackPageContentSha256: "5f103e07657780d9f8a17053498cf300c1edee9a17272a20455ffd453fdff42a",
    stackReconciledChangedBytes: 13,
    nextTerminal: {
      kind: "owned-import-thunk",
      thunk: {
        id: 89,
        address: 0x7f000580,
        requesterOwner: 1,
        module: "KERNEL32.dll",
        iatRva: 0x000d120c,
        iatAddress: 0x100d120c,
        originalIatValue: 0x000ef9b0,
        kind: "name",
        name: "EncodePointer",
        hint: 234,
        nameRva: 0x000ef9b0,
      },
      returnAddress: 0x100a9697,
      stackWords: [
        0x100a9697,
        0x100a9608,
        0x100a4ffc,
        0xec010001,
        0xec010001,
        0xec010001,
        0xec010001,
        0xec010001,
      ],
    },
    instructionCount: 229,
    executeStops: 55,
    budgetStops: 54,
  },
  seventeenthCall: {
    requestLineBytes: 9_136,
    receiptLineBytes: 2_574,
    sourceFunctionPrefixSha256:
      "ca74fa61a06f6555a4ec69ea76cdb33d4e1a461a970975d416644ecbeaf43b68",
    stackPageContentSha256: "3091514821863563f1c42861b1835da0639a0aecf15286732f71dec102615893",
    stackReconciledChangedBytes: 32,
    encodedStoreValueBefore: 0,
    encodedStorePageContentSha256:
      "d8e3945ee6c5f229abe4f6e66c9ff547432323a81c23457829ec236d83674b10",
    nextTerminal: {
      kind: "owned-import-thunk",
      thunk: {
        id: 89,
        address: 0x7f000580,
        requesterOwner: 1,
        module: "KERNEL32.dll",
        iatRva: 0x000d120c,
        iatAddress: 0x100d120c,
        originalIatValue: 0x000ef9b0,
        kind: "name",
        name: "EncodePointer",
        hint: 234,
        nameRva: 0x000ef9b0,
      },
      returnAddress: 0x100a58dc,
      stackWords: [
        0x100a58dc,
        0x7f020000,
        0,
        0,
        0x100a375b,
        0xe93f6d87,
        0,
        0,
      ],
    },
    instructionCount: 238,
    executeStops: 9,
    budgetStops: 8,
  },
};

const page = {
  size: 0x1000,
  present: 1,
  writable: 2,
  user: 4,
  dirty: 0x40,
};
const protocol = {
  maximumLineBytes: 8 * 1024 * 1024,
  maximumReflectionPages: 400,
};
const runtimeFrame = {
  magic: "VINEFR01",
  version: 1,
  headerBytes: 24,
  acknowledgement: 2,
  cpuWriteback: 3,
  writebackReceipt: 4,
  hostDelta: 5,
  sessionReady: 6,
  trap: 7,
  apiCompletion: 8,
  apiFault: 9,
  sessionStop: 10,
  sessionStopped: 11,
};
const writebackFormat = {
  magic: "VINEWB01",
  version: 1,
  headerBytes: 32,
  descriptorBytes: 8,
};
const deltaFormat = {
  magic: "VINEDL01",
  version: 1,
  headerBytes: 56,
  mappingDescriptorBytes: 32,
  pageDescriptorBytes: 16,
};
const genericRuntimeExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 21,
  writebackBatches: 22,
  dirtyPages: 38,
  changedBytes: 268,
  hostDeltaBatches: 3,
  finalGeneration: 52,
  firstUnsupported: {
    capabilityCluster: "kernel32-critical-section-synchronization",
    endpointId: 100,
    endpointAddress: 0x7f000630,
    module: "kernel32.dll",
    symbol: "InitializeCriticalSectionAndSpinCount",
    iatAddress: 0x100d1238,
    arguments: [0x100ff630, 4000],
    returnAddress: 0x100ab5e7,
    instructionCount: 266,
    faultCode: "endpoint-contract-missing",
  },
  reportBytes: 96_244,
  reportSha256: "a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa",
};
const criticalSectionSynchronizationExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 35,
  initializationCalls: 14,
  firstStorageAddress: 0x100ff630,
  storageStride: 24,
  spinCount: 4000,
  writebackBatches: 36,
  dirtyPages: 65,
  changedBytes: 344,
  hostDeltaBatches: 17,
  finalGeneration: 80,
  firstUnsupported: {
    capabilityCluster: "kernel32-pointer-encoding",
    endpointId: 88,
    endpointAddress: 0x7f000570,
    module: "kernel32.dll",
    symbol: "DecodePointer",
    iatAddress: 0x100d1208,
    arguments: [0xec010003],
    returnAddress: 0x100a5924,
    instructionCount: 575,
    faultCode: "endpoint-contract-missing",
  },
  reportBytes: 167_719,
  reportSha256: "cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69",
};
const pointerDecodingExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 36,
  initializationCalls: 14,
  firstStorageAddress: 0x100ff630,
  storageStride: 24,
  spinCount: 4000,
  decodeCalls: 1,
  decodedToken: 0xec010003,
  decodedSource: 0x7f020000,
  writebackBatches: 37,
  dirtyPages: 66,
  changedBytes: 348,
  hostDeltaBatches: 17,
  finalGeneration: 81,
  firstUnsupported: {
    capabilityCluster: "kernel32-fiber-local-storage",
    endpointId: 1,
    endpointAddress: 0x7f020000,
    module: "kernel32.dll",
    name: "FlsAlloc",
    arguments: [0x100a5666],
    returnAddress: 0x100a5926,
    instructionCount: 577,
    faultCode: "dynamic-export-contract-missing",
  },
  reportBytes: 171_356,
  reportSha256: "5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e",
};
const fiberLocalStorageAllocationExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 37,
  initializationCalls: 14,
  decodeCalls: 1,
  allocationCalls: 1,
  callback: 0x100a5666,
  logicalIndex: 0,
  allocationInstructionCount: 577,
  allocationReturnAddress: 0x100a5926,
  writebackBatches: 38,
  dirtyPages: 68,
  changedBytes: 399,
  hostDeltaBatches: 17,
  finalGeneration: 82,
  firstUnsupported: {
    capabilityCluster: "kernel32-private-heap-allocation",
    endpointId: 75,
    endpointAddress: 0x7f0004a0,
    module: "kernel32.dll",
    symbol: "HeapAlloc",
    iatAddress: 0x100d11d4,
    arguments: [0x68010001, 8, 0x214],
    returnAddress: 0x100ac3cc,
    instructionCount: 618,
    faultCode: "endpoint-contract-missing",
  },
  reportBytes: 175_123,
  reportSha256: "26ed41c2a7e8266a47431da9fd737d26752e5812f057400b9715047990e6695c",
};
const privateHeapAllocationExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 39,
  decodeCalls: 2,
  heapAllocationCalls: 1,
  heapHandle: 0x68010001,
  flags: 0x8,
  byteLength: 0x214,
  guestAddress: 0x20000000,
  allocationInstructionCount: 618,
  allocationReturnAddress: 0x100ac3cc,
  writebackBatches: 40,
  dirtyPages: 70,
  changedBytes: 415,
  hostDeltaBatches: 18,
  finalGeneration: 87,
  heapDelta: {
    sequence: 38,
    fromGeneration: 82,
    generation: 85,
    mappingOperations: 2,
    address: 0x20000000,
    owner: 10,
    payloadSha256: "ad7facb2586fc6e966c004d7d1d16b024f5805ff7cb47c7a85dabd8b48892ca7",
  },
  firstUnsupported: {
    capabilityCluster: "kernel32-fiber-local-storage",
    endpointId: 3,
    endpointAddress: 0x7f020020,
    module: "kernel32.dll",
    name: "FlsSetValue",
    arguments: [0, 0x20000000],
    returnAddress: 0x100a5955,
    instructionCount: 644,
    faultCode: "dynamic-export-contract-missing",
  },
  reportBytes: 183_102,
  reportSha256: "d4081cfeca7648435b8a4f49c56b7abaec483dd8cd181343c2c543d2586df2b4",
};
const fiberLocalStorageValueExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 42,
  flsSetValueCalls: 1,
  logicalIndex: 0,
  value: 0x20000000,
  threadId: 8192,
  callInstructionCount: 644,
  callReturnAddress: 0x100a5955,
  continuationSymbols: ["FlsSetValue", "GetModuleHandleW", "EnterCriticalSection"],
  writebackBatches: 43,
  dirtyPages: 75,
  changedBytes: 497,
  hostDeltaBatches: 19,
  finalGeneration: 91,
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 124,
    endpointAddress: 0x7f0007b0,
    module: "kernel32.dll",
    symbol: "InterlockedIncrement",
    iatAddress: 0x100d1298,
    arguments: [0x100f08e0],
    returnAddress: 0x100a5577,
    instructionCount: 707,
    faultCode: "endpoint-contract-missing",
  },
  reportBytes: 195_821,
  reportSha256: "f6698bdfb51d0420bd3a85b2b2525b3cd09e155fde529a4997283548382e742f",
};
const interlockedIncrementExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 50,
  callCount: 3,
  calls: [
    {
      targetAddress: 0x100f08e0,
      before: 0,
      after: 1,
      instructionCount: 707,
      returnAddress: 0x100a5577,
      writeGeneration: 92,
    },
    {
      targetAddress: 0x100f0f70,
      before: 1,
      after: 2,
      instructionCount: 759,
      returnAddress: 0x100a7351,
      writeGeneration: 98,
    },
    {
      targetAddress: 0x100f0ebc,
      before: 0,
      after: 1,
      instructionCount: 823,
      returnAddress: 0x100a73c7,
      writeGeneration: 100,
    },
  ],
  writebackBatches: 51,
  dirtyPages: 86,
  changedBytes: 667,
  hostDeltaBatches: 26,
  finalGeneration: 106,
  firstUnsupported: {
    capabilityCluster: "kernel32-private-heap-allocation",
    endpointId: 75,
    endpointAddress: 0x7f0004a0,
    module: "kernel32.dll",
    symbol: "HeapAlloc",
    iatAddress: 0x100d11d4,
    arguments: [0x68010001, 8, 0x800],
    returnAddress: 0x100ac3cc,
    instructionCount: 934,
    faultCode: "heap-alloc-variant-unsupported",
  },
  reportBytes: 234_775,
  reportSha256: "4c6cfe6d57acd34763d69b1f00f9e20a882360b971f2bc1dd70084a72e67f643",
};
const privateHeapSecondAllocationExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 51,
  heapHandle: 0x68010001,
  firstGuestAddress: 0x20000000,
  secondGuestAddress: 0x20000218,
  secondByteLength: 0x800,
  secondInstructionCount: 934,
  secondReturnAddress: 0x100ac3cc,
  writebackBatches: 52,
  dirtyPages: 89,
  changedBytes: 906,
  hostDeltaBatches: 27,
  finalGeneration: 108,
  secondDelta: {
    sequence: 51,
    fromGeneration: 106,
    generation: 107,
    address: 0x20000000,
    owner: 10,
    payloadSha256: "bd49b343e4864d3d12162b7f0d9a7ce77782b4aa1c931f72121ddcb0f2018532",
  },
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 94,
    endpointAddress: 0x7f0005d0,
    module: "kernel32.dll",
    symbol: "GetStdHandle",
    iatAddress: 0x100d1220,
    arguments: [0xfffffff6],
    returnAddress: 0x100a87b6,
    instructionCount: 1395,
    faultCode: "endpoint-contract-missing",
  },
  reportBytes: 239_789,
  reportSha256: "fe23cec93befb00a73e58ddd9ce94b9fa1d5c78cf193d4751a08f1fb68708ae8",
};
const standardInputHandleExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 52,
  selector: 0xfffffff6,
  handle: 0x53010001,
  instructionCount: 1395,
  returnAddress: 0x100a87b6,
  writebackBatches: 53,
  dirtyPages: 90,
  changedBytes: 911,
  hostDeltaBatches: 27,
  finalGeneration: 109,
  firstUnsupported: {
    endpointId: 101,
    endpointAddress: 0x7f000640,
    module: "kernel32.dll",
    symbol: "GetFileType",
    iatAddress: 0x100d123c,
    arguments: [0x53010001],
    returnAddress: 0x100a87c8,
    instructionCount: 1403,
    faultCode: "endpoint-contract-missing",
  },
  reportBytes: 243_749,
  reportSha256: "5c1210d5313c7008c1628a7afeda4df3ef2ef3caa3fa29ba8bc577030c80a80e",
};
const fileTypeExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 54,
  handle: 0x53010001,
  fileType: 2,
  instructionCount: 1403,
  returnAddress: 0x100a87c8,
  writebackBatches: 55,
  dirtyPages: 94,
  changedBytes: 938,
  hostDeltaBatches: 28,
  finalGeneration: 112,
  criticalSection: {
    occurrence: 15,
    address: 0x20000224,
    spinCount: 4000,
    instructionCount: 1416,
    returnAddress: 0x100a87f6,
  },
  firstUnsupported: {
    endpointId: 94,
    endpointAddress: 0x7f0005d0,
    module: "kernel32.dll",
    symbol: "GetStdHandle",
    iatAddress: 0x100d1220,
    arguments: [0xfffffff5],
    returnAddress: 0x100a87b6,
    instructionCount: 1439,
    faultCode: "standard-handle-selector-unsupported",
  },
  reportBytes: 252_994,
  reportSha256: "dff76ce6eb289bdc4e57e6013c113906d9b0cee80eaa8cbf23948dcccd7efbcf",
};
const standardOutputHandleExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 55,
  inputHandle: 0x53010001,
  outputHandle: 0x53010002,
  outputSelector: 0xfffffff5,
  instructionCount: 1439,
  returnAddress: 0x100a87b6,
  writebackBatches: 56,
  dirtyPages: 95,
  changedBytes: 943,
  hostDeltaBatches: 28,
  finalGeneration: 113,
  firstUnsupported: {
    endpointId: 101,
    endpointAddress: 0x7f000640,
    module: "kernel32.dll",
    symbol: "GetFileType",
    iatAddress: 0x100d123c,
    arguments: [0x53010002],
    returnAddress: 0x100a87c8,
    instructionCount: 1447,
    faultCode: "file-type-standard-handle-unsupported",
  },
  reportBytes: 257_008,
  reportSha256: "0c6047e5c5007f244edaa5bd7ea963dd410283801d3bb572fa8a3aa82b10795d",
};
const standardOutputFileTypeExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 57,
  outputHandle: 0x53010002,
  outputSelector: 0xfffffff5,
  outputFileType: 3,
  instructionCount: 1447,
  returnAddress: 0x100a87c8,
  writebackBatches: 58,
  dirtyPages: 99,
  changedBytes: 966,
  hostDeltaBatches: 29,
  finalGeneration: 116,
  criticalSection: {
    occurrence: 16,
    address: 0x20000264,
    spinCount: 4000,
    instructionCount: 1461,
    returnAddress: 0x100a87f6,
  },
  firstUnsupported: {
    endpointId: 94,
    endpointAddress: 0x7f0005d0,
    module: "kernel32.dll",
    symbol: "GetStdHandle",
    iatAddress: 0x100d1220,
    arguments: [0xfffffff4],
    returnAddress: 0x100a87b6,
    instructionCount: 1484,
    faultCode: "standard-handle-selector-unsupported",
  },
  memoryContentSha256: "909cca3e66d023f7bc2a6b9c8ab9f2435b17338fa850a14823983f513b1c1b68",
  reportBytes: 267_223,
  reportSha256: "0bc3770b1da0af519e4065021c11cc423427b9855d9ceedac6d9b00053837301",
};
const standardErrorHandleExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 58,
  selector: 0xfffffff4,
  handle: 0x53010003,
  instructionCount: 1484,
  returnAddress: 0x100a87b6,
  writebackBatches: 59,
  dirtyPages: 100,
  changedBytes: 971,
  hostDeltaBatches: 29,
  finalGeneration: 117,
  memoryContentSha256: "a97f019fb34b6ab90d9cbf76f35b194674296de83088cb1736cbdbad090b5b12",
  firstUnsupported: {
    endpointId: 101,
    endpointAddress: 0x7f000640,
    module: "kernel32.dll",
    symbol: "GetFileType",
    iatAddress: 0x100d123c,
    arguments: [0x53010003],
    returnAddress: 0x100a87c8,
    instructionCount: 1492,
    faultCode: "file-type-standard-handle-unsupported",
  },
  reportBytes: 271_184,
  reportSha256: "bfa1da3badbbd96ed5c282bd21c3bb0041fa06834dbad38fe0af32e116432c2f",
};
const standardErrorFileTypeExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 60,
  handle: 0x53010003,
  selector: 0xfffffff4,
  fileType: 3,
  instructionCount: 1492,
  returnAddress: 0x100a87c8,
  writebackBatches: 61,
  dirtyPages: 104,
  changedBytes: 991,
  hostDeltaBatches: 30,
  finalGeneration: 120,
  criticalSection: {
    occurrence: 17,
    address: 0x200002a4,
    spinCount: 4000,
    instructionCount: 1506,
    returnAddress: 0x100a87f6,
  },
  firstUnsupported: {
    endpointId: 99,
    endpointAddress: 0x7f000620,
    module: "kernel32.dll",
    symbol: "SetHandleCount",
    iatAddress: 0x100d1234,
    arguments: [32],
    returnAddress: 0x100a881f,
    instructionCount: 1516,
    faultCode: "endpoint-contract-missing",
  },
  memoryContentSha256: "5bb58fba2f503a84550a71e0ba35f11049e77ebb9038461db91f04d0dfb3bb61",
  reportBytes: 280_475,
  reportSha256: "ce9d4271db746b1b7d377bb3e42a861f9c611367e5fcfefba0f19b35fca5aee1",
};
const setHandleCountExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 61,
  count: 32,
  instructionCount: 1516,
  returnAddress: 0x100a881f,
  writebackBatches: 62,
  dirtyPages: 105,
  changedBytes: 992,
  hostDeltaBatches: 30,
  finalGeneration: 121,
  firstUnsupported: {
    endpointId: 85,
    endpointAddress: 0x7f000540,
    module: "kernel32.dll",
    symbol: "GetCommandLineA",
    iatAddress: 0x100d11fc,
    arguments: [],
    returnAddress: 0x100a3786,
    instructionCount: 1526,
    faultCode: "endpoint-contract-missing",
  },
  memoryContentSha256: "3e0e3dc6fc0420af4cf1bc4233d8fc267311f3926abc18b91ce839ce2ff083b3",
  reportBytes: 284_215,
  reportSha256: "91be62c168dd5296fa549a8bf553975853dfef506bcb3ce7670a7b8843f82f3b",
};
const commandLineAExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 62,
  pointer: 0x7ffdd010,
  instructionCount: 1526,
  returnAddress: 0x100a3786,
  writebackBatches: 63,
  dirtyPages: 107,
  changedBytes: 1001,
  hostDeltaBatches: 30,
  finalGeneration: 122,
  memoryContentSha256: "33986edc39e1f5ecb9f050add5e9f534d2e1d81cc31d003ccd5060e1d30806f2",
  reportBytes: 288_041,
  reportSha256: "efa02444d94353422ff0d9a842045025a89a0adb9cf7f90f6d3a87a616b84668",
  firstUnsupported: {
    endpointId: 98,
    endpointAddress: 0x7f000610,
    module: "kernel32.dll",
    symbol: "GetEnvironmentStringsW",
    iatAddress: 0x100d1230,
    arguments: [],
    returnAddress: 0x100a855f,
    instructionCount: 1536,
    faultCode: "endpoint-contract-missing",
  },
};
const environmentStringsWExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 63,
  pointer: 0x7ffdd030,
  instructionCount: 1536,
  returnAddress: 0x100a855f,
  writebackBatches: 64,
  dirtyPages: 108,
  changedBytes: 1015,
  hostDeltaBatches: 30,
  finalGeneration: 123,
  memoryContentSha256: "20109e02acad2ff74cc74520f10b4bba715479f87a1fec2ae000be3f3e179481",
  reportBytes: 291_793,
  reportSha256: "96cb98f09a1d2733442d38ce2736c15c38fd87f3f4fa082e020d623c3fdb6013",
  firstUnsupported: {
    endpointId: 132,
    endpointAddress: 0x7f000830,
    module: "kernel32.dll",
    symbol: "WideCharToMultiByte",
    iatAddress: 0x100d12b8,
    arguments: [0, 0, 0x7ffdd030, 1, 0, 0, 0],
    returnAddress: 0x100a8599,
    instructionCount: 1558,
    faultCode: "endpoint-contract-missing",
  },
};
const wideCharToMultiByteExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 64,
  instructionCount: 1558,
  returnAddress: 0x100a8599,
  writebackBatches: 65,
  dirtyPages: 109,
  changedBytes: 1057,
  hostDeltaBatches: 30,
  finalGeneration: 124,
  memoryContentSha256: "9b2b6b23255736f59b3c02c75bb3dd5adf811fb99e93ce40bc31a601ba55fddb",
  reportBytes: 295_762,
  reportSha256: "ea0eb0f6a14869ca4e75643f01bd42c8c2e7360584c893e7578f6c21423b56cf",
  firstUnsupported: {
    capabilityCluster: "kernel32-private-heap-allocation",
    endpointId: 75,
    endpointAddress: 0x7f0004a0,
    module: "kernel32.dll",
    symbol: "HeapAlloc",
    iatAddress: 0x100d11d4,
    arguments: [0x68010001, 0, 1],
    returnAddress: 0x100a4a33,
    instructionCount: 1591,
    faultCode: "heap-alloc-variant-unsupported",
  },
};
const privateHeapSize1Expected = {
  frozenSemanticCalls: 17,
  supportedCalls: 67,
  heapHandle: 0x68010001,
  guestAddress: 0x20000a18,
  allocationInstructionCount: 1591,
  allocationReturnAddress: 0x100a4a33,
  conversionInstructionCount: 1624,
  conversionReturnAddress: 0x100a85bc,
  freeInstructionCount: 1629,
  freeReturnAddress: 0x100a85d3,
  writebackBatches: 68,
  dirtyPages: 114,
  changedBytes: 1166,
  hostDeltaBatches: 32,
  finalGeneration: 129,
  allocationDelta: {
    sequence: 65,
    fromGeneration: 124,
    generation: 125,
    address: 0x20000000,
    owner: 10,
    payloadSha256: "ad2fa02b3d6607b908899adf2b82cca5ebd6bd63e33709f43f2da67e0a55ac42",
  },
  conversionDelta: {
    sequence: 66,
    fromGeneration: 126,
    generation: 127,
    address: 0x20000000,
    owner: 10,
    payloadSha256: "1504483f3eb1dfa3da39c5141a14dda7233850b774769f59431cf97b46989526",
  },
  memoryContentSha256: "db234b2791244b2ca04dc20c957d3636a04b8a75ab506194d08ee03065bed7f2",
  reportBytes: 308_984,
  reportSha256: "bfecc42fc7b32d21fc86580f81174548e6a26ff2b96f9dee05de51a9e7b4e26e",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 129,
    endpointAddress: 0x7f000800,
    module: "kernel32.dll",
    symbol: "GetLastError",
    iatAddress: 0x100d12ac,
    arguments: [],
    returnAddress: 0x100a55dd,
    instructionCount: 1687,
    faultCode: "endpoint-contract-missing",
  },
};
const getLastErrorExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 68,
  instructionCount: 1687,
  returnAddress: 0x100a55dd,
  result: 87,
  writebackBatches: 69,
  dirtyPages: 115,
  changedBytes: 1179,
  hostDeltaBatches: 32,
  finalGeneration: 130,
  memoryContentSha256: "ea204befffc2c6eebbf5aed3a6e11f792644a35bce5c54a23c996678f2ccb909",
  reportBytes: 312_644,
  reportSha256: "29b3517baa9bc71d3b34b523f0a57c0cb17a0ff09e1dba9a842a0fc1339516b6",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 69,
    endpointAddress: 0x7f000440,
    module: "kernel32.dll",
    symbol: "TlsGetValue",
    iatAddress: 0x100d11bc,
    arguments: [0],
    returnAddress: 0x100a54a0,
    instructionCount: 1695,
    faultCode: "endpoint-contract-missing",
  },
};
const tlsGetValueExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 69,
  instructionCount: 1695,
  returnAddress: 0x100a54a0,
  result: 0x7f020010,
  writebackBatches: 70,
  dirtyPages: 116,
  changedBytes: 1180,
  hostDeltaBatches: 32,
  finalGeneration: 131,
  memoryContentSha256: "3b2a4d71de4d796d93b92b16a2cb0ab1b62b45cab0cdaf173c5d6f09895736ba",
  reportBytes: 316_360,
  reportSha256: "f14401c55cac1c5247050aad297cc19e35cdd2b4f772ccb842568e0de21ee2d9",
  firstUnsupported: {
    capabilityCluster: "kernel32-fiber-local-storage",
    endpointId: 2,
    endpointAddress: 0x7f020010,
    module: "kernel32.dll",
    symbol: "FlsGetValue",
    arguments: [0],
    returnAddress: 0x100a55ec,
    instructionCount: 1703,
    faultCode: "dynamic-export-contract-missing",
  },
};
const flsGetValueExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 70,
  instructionCount: 1703,
  returnAddress: 0x100a55ec,
  result: 0x20000000,
  writebackBatches: 71,
  dirtyPages: 117,
  changedBytes: 1183,
  hostDeltaBatches: 32,
  finalGeneration: 132,
  memoryContentSha256: "de703069e1242705785faa744b240ab843cf6ee1aa7fa78ddf4ea835e2c4e811",
  reportBytes: 320_070,
  reportSha256: "f7672697a5f95f709fc242ce1c31b8f95039152a0d06f516aa042c8dafcacbcf",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 154,
    endpointAddress: 0x7f000990,
    module: "kernel32.dll",
    symbol: "SetLastError",
    iatAddress: 0x100d1310,
    arguments: [87],
    returnAddress: 0x100a5647,
    instructionCount: 1709,
    faultCode: "endpoint-contract-missing",
  },
};
const setLastErrorExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 81,
  instructionCounts: [1709, 1777, 1897],
  returnAddress: 0x100a5647,
  argument: 87,
  writebackBatches: 82,
  dirtyPages: 132,
  changedBytes: 1370,
  hostDeltaBatches: 34,
  finalGeneration: 145,
  memoryContentSha256: "3f212a5faa9d76896ed9f464e344aafe7c1fae60baae03856815a4c89cb417fe",
  reportBytes: 363_609,
  reportSha256: "a915d7b91d65e6b1d4805bc4f0ab62d6444573693402bed9fd473518f56f9657",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 156,
    endpointAddress: 0x7f0009b0,
    module: "kernel32.dll",
    symbol: "GetACP",
    iatAddress: 0x100d1318,
    arguments: [],
    returnAddress: 0x100a6f72,
    instructionCount: 1936,
    faultCode: "endpoint-contract-missing",
  },
};
const getAcpExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 82,
  instructionCounts: [1936],
  returnAddress: 0x100a6f72,
  activeCodePage: 932,
  lastError: 87,
  writebackBatches: 83,
  dirtyPages: 134,
  changedBytes: 1430,
  hostDeltaBatches: 34,
  finalGeneration: 146,
  memoryContentSha256: "7ed5262e4eef60c3f665765d5741872b595205c5022193ec4633194e5ffa25ea",
  reportBytes: 367_384,
  reportSha256: "b897324ff6b85383b2b51be1505e84868e53d9fa5488e5d8f0f4da0ed36b476d",
  firstUnsupported: {
    capabilityCluster: "kernel32-private-heap-allocation",
    endpointId: 75,
    endpointAddress: 0x7f0004a0,
    module: "kernel32.dll",
    symbol: "HeapAlloc",
    iatAddress: 0x100d11d4,
    arguments: [0x68010001, 0, 0x220],
    returnAddress: 0x100a4a33,
    instructionCount: 1978,
    faultCode: "heap-alloc-variant-unsupported",
  },
};
const privateHeapSize544Expected = {
  frozenSemanticCalls: 17,
  supportedCalls: 87,
  heapHandle: 0x68010001,
  guestAddress: 0x20000a20,
  allocationInstructionCount: 1978,
  allocationReturnAddress: 0x100a4a33,
  followupCalls: [
    ["GetLastError", 2049, 0x100a55dd],
    ["TlsGetValue", 2057, 0x100a54a0],
    ["FlsGetValue", 2065, 0x100a55ec],
    ["SetLastError", 2071, 0x100a5647],
  ],
  writebackBatches: 88,
  dirtyPages: 142,
  changedBytes: 2300,
  hostDeltaBatches: 35,
  finalGeneration: 152,
  allocationDelta: {
    sequence: 83,
    fromGeneration: 146,
    generation: 147,
    address: 0x20000000,
    owner: 10,
    payloadSha256: "104d76ea818bcc3969bad0e0a277dc5954a2b175dd508f7952b9eddf98136fb4",
  },
  consumerWriteback: {
    sequence: 84,
    instructionCount: 2049,
    fromGeneration: 147,
    generation: 148,
    changedBytes: 617,
    dirtyPages: [
      { address: 0x006ff000, owner: 4 },
      { address: 0x20000000, owner: 10 },
    ],
    payloadSha256: "e96d08f3b4a9e8663b3b381c23737195780a648af108bd99004ceaf70438d151",
  },
  memoryContentSha256: "c420169199f66f0bec6dc619b5d694e1f4987ea1990ae98f4d6a7773c63fcb26",
  reportBytes: 386_921,
  reportSha256: "355c6c2dfc089c8d6f1ca30ea6cbe3b697f4e7afdfa6ed1ec4f2b0ea15593781",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 155,
    endpointAddress: 0x7f0009a0,
    module: "kernel32.dll",
    symbol: "GetCPInfo",
    iatAddress: 0x100d1314,
    arguments: [932, 0x006ff9c0],
    returnAddress: 0x100a6d13,
    instructionCount: 4523,
    faultCode: "endpoint-contract-missing",
  },
};
const getCpInfoExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 92,
  codePage: 932,
  outputAddress: 0x006ff9c0,
  outputBytes: [
    2, 0, 0, 0, 0x3f, 0, 0x81, 0x9f, 0xe0, 0xfc, 0, 0, 0, 0, 0, 0, 0, 0,
  ],
  instructionCount: 4523,
  returnAddress: 0x100a6d13,
  lastError: 87,
  writebackBatches: 93,
  dirtyPages: 148,
  changedBytes: 2690,
  hostDeltaBatches: 36,
  finalGeneration: 158,
  outputDelta: {
    sequence: 88,
    fromGeneration: 152,
    generation: 153,
    pageAddress: 0x006ff000,
    owner: 4,
    payloadSha256: "6581defd12dd1676cc0b42a7597d4a0c7d3e2ba5daf74e2167f8a083e662df4d",
  },
  consumerWriteback: {
    sequence: 89,
    instructionCount: 5703,
    fromGeneration: 153,
    generation: 154,
    changedBytes: 307,
    dirtyPages: [{ address: 0x006ff000, owner: 4 }],
    payloadSha256: "d71dced9e69c9bf404b67af6fef4ee760f2b5975b86c8ee00e02bb2a9c6dc545",
  },
  followupCalls: [
    ["GetLastError", 5703, 0x100a55dd],
    ["TlsGetValue", 5711, 0x100a54a0],
    ["FlsGetValue", 5719, 0x100a55ec],
    ["SetLastError", 5725, 0x100a5647],
  ],
  memoryContentSha256: "1d0a6694d155aed6f9b12989c7d78ba0f6017c556749f218b9b131ac199eb389",
  reportBytes: 407_009,
  reportSha256: "da95600680cdb922a2952a7045ac5ace1b6f2510f5f553773343854e2f2310a1",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 130,
    endpointAddress: 0x7f000810,
    module: "kernel32.dll",
    symbol: "MultiByteToWideChar",
    iatAddress: 0x100d12b0,
    arguments: [932, 1, 0x006ffdd4, 256, 0, 0, 256],
    returnAddress: 0x100ad691,
    instructionCount: 5793,
    faultCode: "endpoint-contract-missing",
  },
};
const multiByteTableOutputBytes = Array.from({ length: 256 }, (_, byte) => {
  let unit;
  if (byte === 0 || (byte >= 0x81 && byte <= 0x9f) || (byte >= 0xe0 && byte <= 0xfc)) {
    unit = 0x20;
  } else if (byte <= 0x80) {
    unit = byte;
  } else if (byte === 0xa0) {
    unit = 0xf8f0;
  } else if (byte <= 0xdf) {
    unit = 0xff61 + byte - 0xa1;
  } else {
    unit = 0xf8f1 + byte - 0xfd;
  }
  return [unit & 0xff, unit >>> 8];
}).flat();
const multiByteToWideCharExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 94,
  writebackBatches: 95,
  dirtyPages: 150,
  changedBytes: 2741,
  hostDeltaBatches: 37,
  finalGeneration: 161,
  query: {
    arguments: [932, 1, 0x006ffdd4, 256, 0, 0],
    instructionCount: 5793,
    returnAddress: 0x100ad691,
    result: 256,
  },
  conversion: {
    arguments: [932, 1, 0x006ffdd4, 256, 0x006ff738, 256],
    instructionCount: 5886,
    returnAddress: 0x100ad6fb,
    result: 256,
    outputBytes: multiByteTableOutputBytes,
  },
  outputDelta: {
    sequence: 94,
    fromGeneration: 159,
    generation: 160,
    pageAddress: 0x006ff000,
    owner: 4,
    payloadSha256: "be1040f1774da4926f9f0162ff5f0d72360d3af1ccb2afadd0a2e0c30909b73a",
  },
  memoryContentSha256: "eb77024dd956d49c9a1a9f89a0e248220cd6ab6850203978134aeb93b8c4e811",
  reportBytes: 423_814,
  reportSha256: "97655782894eca5f4e97a095d1ee3b64619e4f5ecf076c2be91d9b79ca045718",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 108,
    endpointAddress: 0x7f0006b0,
    module: "kernel32.dll",
    symbol: "GetStringTypeW",
    iatAddress: 0x100d1258,
    arguments: [1, 0x006ff738, 256, 0x006ff9d4],
    returnAddress: 0x100ad70d,
    instructionCount: 5894,
    faultCode: "endpoint-contract-missing",
  },
};
const getStringTypeTableOutputWords = Array.from({ length: 256 }, (_, byte) => {
  if (
    byte === 0 ||
    byte === 0x20 ||
    (byte >= 0x81 && byte <= 0x9f) ||
    (byte >= 0xe0 && byte <= 0xfc)
  ) {
    return 0x0248;
  }
  if ((byte >= 1 && byte <= 8) || (byte >= 14 && byte <= 31) || byte === 127 || byte === 128) {
    return 0x0220;
  }
  if (byte === 9) {
    return 0x0268;
  }
  if (byte >= 10 && byte <= 13) {
    return 0x0228;
  }
  if (
    (byte >= 0x21 && byte <= 0x2f) ||
    (byte >= 0x3a && byte <= 0x40) ||
    (byte >= 0x5b && byte <= 0x60) ||
    (byte >= 0x7b && byte <= 0x7e) ||
    (byte >= 0xa1 && byte <= 0xa5)
  ) {
    return 0x0210;
  }
  if (byte >= 0x30 && byte <= 0x39) {
    return 0x0284;
  }
  if (byte >= 0x41 && byte <= 0x46) {
    return 0x0381;
  }
  if (byte >= 0x47 && byte <= 0x5a) {
    return 0x0301;
  }
  if (byte >= 0x61 && byte <= 0x66) {
    return 0x0382;
  }
  if (byte >= 0x67 && byte <= 0x7a) {
    return 0x0302;
  }
  if (byte >= 0xa6 && byte <= 0xdf) {
    return 0x0300;
  }
  return 0x0200;
});
const getStringTypeTableOutputBytes = getStringTypeTableOutputWords.flatMap((word) => [
  word & 0xff,
  word >>> 8,
]);
const getStringTypeWExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 101,
  writebackBatches: 102,
  dirtyPages: 159,
  changedBytes: 2958,
  hostDeltaBatches: 39,
  finalGeneration: 170,
  call: {
    arguments: [1, 0x006ff738, 256, 0x006ff9d4],
    instructionCount: 5894,
    returnAddress: 0x100ad70d,
    result: 1,
    outputBytes: getStringTypeTableOutputBytes,
  },
  outputDelta: {
    sequence: 95,
    fromGeneration: 161,
    generation: 162,
    pageAddress: 0x006ff000,
    owner: 4,
    payloadSha256: "b4a11394007a1cbd18154b31d864421d2c9ec4062e83b88dafa18421e238dab6",
  },
  consumerWriteback: {
    sequence: 96,
    instructionCount: 5967,
    fromGeneration: 162,
    generation: 163,
    changedBytes: 74,
    dirtyPages: [
      { address: 0x006ff000, owner: 4 },
      { address: 0x20000000, owner: 10 },
    ],
    payloadSha256: "1a343abf4d9e2b61cf0924a9eb44ac80c77131a231ddeebbba1eb93e2a816809",
  },
  followupCalls: [
    ["GetLastError", 5967, 0x100a55dd],
    ["TlsGetValue", 5975, 0x100a54a0],
    ["FlsGetValue", 5983, 0x100a55ec],
    ["SetLastError", 5989, 0x100a5647],
    ["MultiByteToWideChar", 7606, 0x100a7ef7],
    ["MultiByteToWideChar", 7664, 0x100a7f65],
  ],
  memoryContentSha256: "3c26a9fa98427249ef0f37546f25592d18d18e9ade8c07c4ebbd10e2ce6b9aa2",
  reportBytes: 468_010,
  reportSha256: "00facfca5960a6e0e233deab536aa5d01e190d6e7f72e6be7caffb2e8997c425",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 96,
    endpointAddress: 0x7f0005f0,
    module: "kernel32.dll",
    symbol: "LCMapStringW",
    iatAddress: 0x100d1228,
    arguments: [1041, 256, 0x006ff708, 256, 0, 0],
    returnAddress: 0x100a7f81,
    instructionCount: 7675,
    faultCode: "endpoint-contract-missing",
  },
};
const lcMapStringSourceWords = Array.from(
  { length: multiByteTableOutputBytes.length / 2 },
  (_, index) =>
    multiByteTableOutputBytes[index * 2] |
    (multiByteTableOutputBytes[index * 2 + 1] << 8),
);
const lcMapStringLowerWords = lcMapStringSourceWords.map((unit) =>
  (unit >= 0x41 && unit <= 0x5a) || (unit >= 0xff21 && unit <= 0xff3a)
    ? unit + 0x20
    : unit,
);
const lcMapStringUpperWords = lcMapStringSourceWords.map((unit) =>
  unit >= 0x61 && unit <= 0x7a ? unit - 0x20 : unit,
);
const lcMapWordsToBytes = (words) =>
  words.flatMap((word) => [word & 0xff, word >>> 8]);
const lcMapStringExpectedCalls = [
  {
    arguments: [1041, 0x0100, 0x006ff708, 256, 0, 0],
    instructionCount: 7675,
    returnAddress: 0x100a7f81,
    outputBytes: null,
  },
  {
    arguments: [1041, 0x0100, 0x006ff708, 256, 0x006ff4f8, 256],
    instructionCount: 7735,
    returnAddress: 0x100a8020,
    outputBytes: lcMapWordsToBytes(lcMapStringLowerWords),
  },
  {
    arguments: [1041, 0x0200, 0x006ff728, 256, 0, 0],
    instructionCount: 9544,
    returnAddress: 0x100a7f81,
    outputBytes: null,
  },
  {
    arguments: [1041, 0x0200, 0x006ff728, 256, 0x006ff518, 256],
    instructionCount: 9604,
    returnAddress: 0x100a8020,
    outputBytes: lcMapWordsToBytes(lcMapStringUpperWords),
  },
];
const lcMapStringWExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 113,
  writebackBatches: 114,
  dirtyPages: 174,
  changedBytes: 3350,
  hostDeltaBatches: 44,
  finalGeneration: 187,
  outputDeltas: [
    {
      sequence: 103,
      fromGeneration: 171,
      generation: 172,
      pageAddress: 0x006ff000,
      owner: 4,
      payloadSha256: "387148ebbbfe78efb69a41e9c0cde9408f237880a748e69f0765f6d8d2ad4b27",
    },
    {
      sequence: 112,
      fromGeneration: 183,
      generation: 184,
      pageAddress: 0x006ff000,
      owner: 4,
      payloadSha256: "1f5252fecc213757fa3e867aa9886d993f6ca748454b847df7ad211c55dfe83c",
    },
  ],
  followupCalls: [
    ["WideCharToMultiByte", 7749, 0x100a8043],
    ["GetLastError", 7836, 0x100a55dd],
    ["TlsGetValue", 7844, 0x100a54a0],
    ["FlsGetValue", 7852, 0x100a55ec],
    ["SetLastError", 7858, 0x100a5647],
    ["MultiByteToWideChar", 9475, 0x100a7ef7],
    ["MultiByteToWideChar", 9533, 0x100a7f65],
    ["WideCharToMultiByte", 9618, 0x100a8043],
  ],
  memoryContentSha256: "00d38b3de81df69fb459a3ff8ad7f0b53e1e50317f1fc6b546efabab19f20b55",
  reportBytes: 550_988,
  reportSha256: "d56849eac369c5ad5b946c06b27acba844b779e548795efcc464a67bc2c7a889",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 123,
    endpointAddress: 0x7f0007a0,
    module: "kernel32.dll",
    symbol: "InterlockedDecrement",
    iatAddress: 0x100d1294,
    arguments: [0x100f08e0],
    returnAddress: 0x100a7201,
    instructionCount: 12137,
    faultCode: "endpoint-contract-missing",
  },
};
const interlockedDecrementExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 119,
  writebackBatches: 120,
  dirtyPages: 187,
  changedBytes: 3640,
  hostDeltaBatches: 50,
  finalGeneration: 199,
  calls: [
    {
      arguments: [0x100f08e0],
      instructionCount: 12137,
      returnAddress: 0x100a7201,
      before: 1,
      after: 0,
      result: 0,
      delta: {
        sequence: 114,
        fromGeneration: 187,
        generation: 188,
        pageAddress: 0x100f0000,
        owner: 1,
        payloadSha256: "25d935f63b91d0707a2c69ca70a0d27ad2a336e280099bdf4857bf24227bf37d",
      },
    },
    {
      arguments: [0x100f08e0],
      instructionCount: 15816,
      returnAddress: 0x100a72b8,
      before: 0,
      after: -1,
      result: 0xffffffff,
      delta: {
        sequence: 117,
        fromGeneration: 193,
        generation: 194,
        pageAddress: 0x100f0000,
        owner: 1,
        payloadSha256: "11f320560d0c6ca418481dffc089df2c07efdb4a539ae4e7dc3cdec6dd39c8f9",
      },
    },
  ],
  followupCalls: [
    ["InterlockedIncrement", 12147, 0x100a7222],
    ["EnterCriticalSection", 12164, 0x100ab763],
    ["InterlockedIncrement", 15822, 0x100a72d8],
    ["LeaveCriticalSection", 15833, 0x100ab66f],
  ],
  memoryContentSha256: "83a1a4fd36f1f94efc7c0dcd83f1ff12f35729a00158ef7b48746f51896228f3",
  reportBytes: 580_772,
  reportSha256: "6c905456197834909eb965cfa950c05a4587a91486b7b9c72f55253340bb9ff4",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 34,
    endpointAddress: 0x7f000210,
    module: "kernel32.dll",
    symbol: "GetModuleFileNameA",
    iatAddress: 0x100d1130,
    arguments: [0, 0x100ff520, 260],
    returnAddress: 0x100a84c6,
    instructionCount: 15863,
    faultCode: "endpoint-contract-missing",
  },
};
const getModuleFileNameAExpected = {
  frozenSemanticCalls: 17,
  supportedCalls: 152,
  writebackBatches: 153,
  dirtyPages: 229,
  changedBytes: 3920,
  hostDeltaBatches: 51,
  finalGeneration: 233,
  call: {
    arguments: [0, 0x100ff520, 260],
    result: 15,
    instructionCount: 15863,
    returnAddress: 0x100a84c6,
    pathBytes: [...Buffer.from("C:\\Vine\\BGI.exe\0", "ascii")],
    delta: {
      sequence: 120,
      fromGeneration: 199,
      generation: 200,
      pageAddress: 0x100ff000,
      owner: 1,
      payloadSha256: "ba1dab4296a6af9944e54edfbca89515d034ce0207960b404e65a3dfa252b498",
    },
  },
  followupCalls: [
    ["GetLastError", 15935, 0x100a55dd],
    ["SetLastError", 16853, 0x100a5647],
  ],
  memoryContentSha256: "b233490f03ec30f568aa6ddf9868da4f41804150f4bbccfff8a20f2e9d01e943",
  reportBytes: 704_797,
  reportSha256: "3bbeda2ffff7720afc73c7729b52f7168b2989758baf6c7cc7e6a45b870e15e7",
  firstUnsupported: {
    capabilityCluster: "kernel32-private-heap-allocation",
    endpointId: 75,
    endpointAddress: 0x7f0004a0,
    module: "kernel32.dll",
    symbol: "HeapAlloc",
    iatAddress: 0x100d11d4,
    arguments: [0x68010001, 0, 16],
    returnAddress: 0x100a4a33,
    instructionCount: 16965,
    faultCode: "heap-alloc-variant-unsupported",
  },
};
const privateHeapSize16Expected = {
  frozenSemanticCalls: 17,
  supportedCalls: 185,
  writebackBatches: 186,
  dirtyPages: 272,
  changedBytes: 4178,
  hostDeltaBatches: 52,
  finalGeneration: 267,
  heapHandle: 0x68010001,
  guestAddress: 0x20000c40,
  allocationInstructionCount: 16965,
  allocationReturnAddress: 0x100a4a33,
  allocationDelta: {
    sequence: 153,
    fromGeneration: 233,
    generation: 234,
    address: 0x20000000,
    owner: 10,
    payloadSha256: "e542f0cb74547f59cf7579ea2dba8ad7f69b7bd174332b3ec7163768ebd2df03",
  },
  consumerWriteback: {
    sequence: 154,
    instructionCount: 17061,
    dirtyPages: [
      { address: 0x006ff000, owner: 4 },
      { address: 0x20000000, owner: 10 },
    ],
    payloadSha256: "b02aaa44a8d92c0a63162c813c3eba46e7fec72782c3636e28a88d0ef2a6e9b3",
    fromGeneration: 234,
    generation: 235,
    changedBytes: 55,
  },
  followupCalls: [
    ["GetLastError", 17061, 0x100a55dd],
    ["SetLastError", 18007, 0x100a5647],
  ],
  memoryContentSha256: "b648580e9ea1840e7c2703115d144e3b7014040f74a6670a14136808ee2782ff",
  reportBytes: 828_414,
  reportSha256: "887a74c11a1466cfbbfdeebf126ecc6138588c4b1f9e89c39cfdbb290a2ee8e6",
  firstUnsupported: {
    capabilityCluster: "kernel32-private-heap-allocation",
    endpointId: 75,
    endpointAddress: 0x7f0004a0,
    module: "kernel32.dll",
    symbol: "HeapAlloc",
    iatAddress: 0x100d11d4,
    arguments: [0x68010001, 8, 4],
    returnAddress: 0x100ac3cc,
    instructionCount: 18143,
    faultCode: "heap-alloc-variant-unsupported",
  },
};
const privateHeapSize4Expected = {
  frozenSemanticCalls: 17,
  supportedCalls: 186,
  writebackBatches: 187,
  dirtyPages: 274,
  changedBytes: 4194,
  hostDeltaBatches: 53,
  finalGeneration: 269,
  heapHandle: 0x68010001,
  guestAddress: 0x20000c50,
  allocationInstructionCount: 18143,
  allocationReturnAddress: 0x100ac3cc,
  allocationDelta: {
    sequence: 186,
    fromGeneration: 267,
    generation: 268,
    address: 0x20000000,
    owner: 10,
    payloadSha256: "1849087a95d674f3e6d3876d75739a49d1faa5ad948249223d3215bc45d4fed5",
  },
  consumerWriteback: {
    sequence: 187,
    instructionCount: 18180,
    dirtyPages: [
      { address: 0x006ff000, owner: 4 },
      { address: 0x100fe000, owner: 1 },
    ],
    payloadSha256: "1cd6cab32153e17fba2c7a368e562ff6bec446a637e4f510ec285c7ac265689b",
    fromGeneration: 268,
    generation: 269,
    changedBytes: 16,
  },
  memoryContentSha256: "d9abc780ef3d253810695351cb66296b1aeb727e7e8590f73fe20e2c1c73cd2c",
  reportBytes: 833_007,
  reportSha256: "80f28db63bf8400b7ea7d1db5d7552303dec05dc71addb1ac2ed1ecea29eefa1",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 76,
    endpointAddress: 0x7f0004b0,
    module: "kernel32.dll",
    symbol: "HeapFree",
    iatAddress: 0x100d11d8,
    arguments: [0x68010001, 0, 0x20000a18],
    returnAddress: 0x100a48cd,
    instructionCount: 18180,
    faultCode: "endpoint-contract-missing",
  },
};
const privateHeapFreeSize1Expected = {
  frozenSemanticCalls: 17,
  supportedCalls: 197,
  writebackBatches: 198,
  dirtyPages: 300,
  changedBytes: 4423,
  hostDeltaBatches: 54,
  finalGeneration: 281,
  heapHandle: 0x68010001,
  allocationId: 3,
  allocationAddress: 0x20000a18,
  allocationRequestedBytes: 1,
  allocationReservedBytes: 8,
  freeInstructionCount: 18180,
  freeReturnAddress: 0x100a48cd,
  freeDelta: {
    sequence: 187,
    fromGeneration: 269,
    generation: 270,
    address: 0x20000000,
    owner: 10,
    payloadSha256: "93cb85efb43c4586535cabdedf112e538d7fbe792245ed8954482b9815aa092a",
  },
  postFreeEncodePointerCalls: 10,
  memoryContentSha256: "f248e7dafa44a4a036822bd2eda199959e01c5d39090f67fba5f9f15c0de0186",
  reportBytes: 875_554,
  reportSha256: "e0b255e0112fb7d0cbcef12afcd91be63877049fed56fc30b62ce5123f1acd96",
  firstUnsupported: {
    capabilityCluster: "kernel32-private-heap-allocation",
    endpointId: 75,
    endpointAddress: 0x7f0004a0,
    module: "kernel32.dll",
    symbol: "HeapAlloc",
    iatAddress: 0x100d11d4,
    arguments: [0x68010001, 8, 128],
    returnAddress: 0x100ac3cc,
    instructionCount: 18653,
    faultCode: "heap-alloc-variant-unsupported",
  },
};
const privateHeapSize128Expected = {
  frozenSemanticCalls: 17,
  supportedCalls: 199,
  writebackBatches: 200,
  dirtyPages: 304,
  changedBytes: 4444,
  hostDeltaBatches: 55,
  finalGeneration: 284,
  heapHandle: 0x68010001,
  guestAddress: 0x20000c58,
  allocationInstructionCount: 18653,
  allocationReturnAddress: 0x100ac3cc,
  allocationDelta: {
    sequence: 198,
    fromGeneration: 281,
    generation: 282,
    address: 0x20000000,
    owner: 10,
    payloadSha256: "93cb85efb43c4586535cabdedf112e538d7fbe792245ed8954482b9815aa092a",
  },
  encodePointerCall: {
    sequence: 199,
    occurrence: 17,
    instructionCount: 18673,
    returnAddress: 0x100a1dda,
    encodedPointer: 0xec010010,
  },
  consumerWriteback: {
    sequence: 200,
    instructionCount: 18693,
    dirtyPages: [
      { address: 0x006ff000, owner: 4 },
      { address: 0x10153000, owner: 1 },
      { address: 0x20000000, owner: 10 },
    ],
    payloadSha256: "181672ed0948ea489ac06efa8f4c8440ba46bd17c631f9729e7fb92c1f20e913",
    fromGeneration: 283,
    generation: 284,
    changedBytes: 14,
  },
  memoryContentSha256: "5262246193dbe482a09781cac162f4f23f4eabaa4981c5f50ec2c9460ce96a89",
  reportBytes: 883_995,
  reportSha256: "51382513962b57159e178966b16aac2010513a3c58cccaf9b9d6abbfc857d073",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 74,
    endpointAddress: 0x7f000490,
    module: "kernel32.dll",
    symbol: "IsProcessorFeaturePresent",
    iatAddress: 0x100d11d0,
    arguments: [10],
    returnAddress: 0x100a4d87,
    instructionCount: 18693,
    faultCode: "endpoint-contract-missing",
  },
};
const isProcessorFeaturePresentExpected = {
  ...privateHeapSize128Expected,
  supportedCalls: 202,
  writebackBatches: 203,
  dirtyPages: 311,
  changedBytes: 4561,
  hostDeltaBatches: 56,
  finalGeneration: 289,
  processorFeatureCalls: [
    {
      sequence: 200,
      occurrence: 1,
      instructionCount: 18693,
      returnAddress: 0x100a4d87,
      consumerWriteback: {
        sequence: 201,
        instructionCount: 18721,
        dirtyPages: [
          { address: 0x006ff000, owner: 4 },
          { address: 0x10153000, owner: 1 },
        ],
        payloadSha256: "8c4ff44a051a92d9c7f8dd81de98ed1e0447b083bda6af13dcb8ef126c26c322",
        fromGeneration: 284,
        generation: 285,
        changedBytes: 3,
      },
    },
    {
      sequence: 201,
      occurrence: 2,
      instructionCount: 18721,
      returnAddress: 0x100a7e7c,
      consumerWriteback: {
        sequence: 202,
        instructionCount: 18780,
        dirtyPages: [
          { address: 0x006ff000, owner: 4 },
          { address: 0x10153000, owner: 1 },
        ],
        payloadSha256: "0b4547357c675c727b5c40ab53cfe53665e7dfd11561e159e40ec963191e0483",
        fromGeneration: 285,
        generation: 286,
        changedBytes: 24,
      },
    },
  ],
  laterAllocation: {
    sequence: 202,
    occurrence: 8,
    instructionCount: 18780,
    returnAddress: 0x100ac3cc,
    guestAddress: 0x20000cd8,
    delta: {
      sequence: 202,
      fromGeneration: 286,
      generation: 288,
      mappingOperations: 1,
      dirtyPages: [
        { address: 0x20000000, owner: 10 },
        { address: 0x20001000, owner: 10 },
      ],
      payloadBytes: 8192,
      payloadSha256: "5646000ab7257eae19481643eea8c9ac40bfbb41ba4bd5c9ebb51933b756e825",
    },
  },
  memoryContentSha256: "b4e25674b53cc1e159ee02cff2c00610dfbd3499d716c932ee2cd64fc99362e1",
  reportBytes: 896_534,
  reportSha256: "c173fd9d2ae2763f15ddafa0b487bddeba9b934a978ddd840766cfbad870cdb5",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 78,
    endpointAddress: 0x7f0004d0,
    module: "kernel32.dll",
    symbol: "SetUnhandledExceptionFilter",
    iatAddress: 0x100d11e0,
    arguments: [0x100a816f],
    returnAddress: 0x100a81bc,
    instructionCount: 18993,
    faultCode: "endpoint-contract-missing",
  },
};
const setUnhandledExceptionFilterExpected = {
  ...isProcessorFeaturePresentExpected,
  supportedCalls: 206,
  writebackBatches: 207,
  dirtyPages: 316,
  changedBytes: 4690,
  hostDeltaBatches: 57,
  finalGeneration: 294,
  filterCall: {
    sequence: 203,
    occurrence: 1,
    instructionCount: 18993,
    filterAddress: 0x100a816f,
    returnAddress: 0x100a81bc,
    consumerWriteback: {
      sequence: 204,
      instructionCount: 19052,
      dirtyPages: [
        { address: 0x006ff000, owner: 4 },
        { address: 0x7ffde000, owner: 5 },
      ],
      payloadSha256: "67189a41be50b8fa5528b9b5a2ccf6eaf885ae206f347aee46cedb65268b22cd",
      fromGeneration: 289,
      generation: 290,
      changedBytes: 76,
    },
  },
  continuationCalls: [
    {
      sequence: 204,
      symbol: "EnterCriticalSection",
      occurrence: 5,
      instructionCount: 19052,
      arguments: [0x100ff6c0],
      result: { action: "preserve" },
      implementationId: "kernel32.enter-critical-section.v1",
      returnAddress: 0x100ab763,
      stackCleanupBytes: 8,
    },
    {
      sequence: 205,
      symbol: "DecodePointer",
      occurrence: 3,
      instructionCount: 19071,
      arguments: [0xec010010],
      result: { action: "set", value: 0x20000c58 },
      implementationId: "kernel32.decode-pointer.v1",
      returnAddress: 0x100a1d24,
      stackCleanupBytes: 8,
    },
    {
      sequence: 206,
      symbol: "DecodePointer",
      occurrence: 4,
      instructionCount: 19076,
      arguments: [0xec010010],
      result: { action: "set", value: 0x20000c58 },
      implementationId: "kernel32.decode-pointer.v1",
      returnAddress: 0x100a1d31,
      stackCleanupBytes: 8,
    },
  ],
  criticalSectionDelta: {
    sequence: 204,
    fromGeneration: 290,
    generation: 291,
    address: 0x100ff000,
    owner: 1,
    payloadSha256: "01986be151ae34103b6631d77d778644f4a2d931e8cdab261fd41276455f0556",
  },
  memoryContentSha256: "563d391b69ea3dcad1669d68f167bb35fad5b4d48adbd153a9f95ad1fb072f97",
  reportBytes: 912_806,
  reportSha256: "92caa9ef24a63fc1abbac0bfc063361b2c7b9998cd14d2d2ee02e2d5c999defa",
  firstUnsupported: {
    capabilityCluster: "kernel32-api-contract",
    endpointId: 73,
    endpointAddress: 0x7f000480,
    module: "kernel32.dll",
    symbol: "HeapSize",
    iatAddress: 0x100d11cc,
    arguments: [0x68010001, 0, 0x20000c58],
    returnAddress: 0x100a4f77,
    instructionCount: 19096,
    faultCode: "endpoint-contract-missing",
  },
};
const heapSizeExpected = {
  ...setUnhandledExceptionFilterExpected,
  supportedCalls: 210,
  writebackBatches: 211,
  dirtyPages: 324,
  changedBytes: 4757,
  hostDeltaBatches: 58,
  finalGeneration: 299,
  heapSizeCall: {
    sequence: 207,
    occurrence: 1,
    instructionCount: 19096,
    arguments: [0x68010001, 0, 0x20000c58],
    result: 128,
    returnAddress: 0x100a4f77,
    stackCleanupBytes: 16,
  },
  postHeapSizeCalls: [
    {
      sequence: 208,
      symbol: "EncodePointer",
      occurrence: 18,
      instructionCount: 19107,
      arguments: [0x100a8851],
      result: { action: "set", value: 0xec010011 },
      implementationId: "kernel32.encode-pointer.v1",
      returnAddress: 0x100a1daa,
      stackCleanupBytes: 8,
    },
    {
      sequence: 209,
      symbol: "EncodePointer",
      occurrence: 19,
      instructionCount: 19112,
      arguments: [0x20000c5c],
      result: { action: "set", value: 0xec010012 },
      implementationId: "kernel32.encode-pointer.v1",
      returnAddress: 0x100a1db2,
      stackCleanupBytes: 8,
    },
    {
      sequence: 210,
      symbol: "LeaveCriticalSection",
      occurrence: 5,
      instructionCount: 19134,
      arguments: [0x100ff6c0],
      result: { action: "preserve" },
      implementationId: "kernel32.leave-critical-section.v1",
      returnAddress: 0x100ab66f,
      stackCleanupBytes: 8,
    },
  ],
  leaveCriticalSectionDelta: {
    sequence: 210,
    fromGeneration: 297,
    generation: 298,
    address: 0x100ff000,
    owner: 1,
    payloadSha256: "ba1dab4296a6af9944e54edfbca89515d034ce0207960b404e65a3dfa252b498",
  },
  memoryContentSha256: "90c245267e933a01c1e54fa0305bec1d54a16180a3e7465b6a3b6b3c8d3dd0b8",
  reportBytes: 929_421,
  reportSha256: "058ca051ad9b4118ce8854333eafd35aab117dc70da860b28188a2e1afff8fac",
  firstUnsupported: {
    capabilityCluster: "kernel32-critical-section-synchronization",
    endpointId: 147,
    endpointAddress: 0x7f000920,
    module: "kernel32.dll",
    symbol: "InitializeCriticalSection",
    iatAddress: 0x100d12f4,
    arguments: [0x100fd9cc],
    returnAddress: 0x1009b2c3,
    instructionCount: 19185,
    faultCode: "endpoint-contract-missing",
  },
};
const initializeCriticalSectionExpected = {
  ...heapSizeExpected,
  supportedCalls: 267,
  writebackBatches: 268,
  dirtyPages: 406,
  changedBytes: 5855,
  hostDeltaBatches: 67,
  finalGeneration: 365,
  implementedApiContracts: 44,
  initializationCall: {
    sequence: 211,
    occurrence: 1,
    instructionCount: 19185,
    arguments: [0x100fd9cc],
    returnAddress: 0x1009b2c3,
    stackCleanupBytes: 8,
    bytes: [
      0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
      0x00, 0x00, 0x00, 0x00, 0xd0, 0x07, 0x00, 0x02,
    ],
    delta: {
      fromGeneration: 299,
      generation: 300,
      address: 0x100fd000,
      owner: 1,
      payloadSha256: "80eb4797e4759ad5c9f4c484abf155aa7008a2ef8bfdffad9c17319ae36ef0fb",
    },
  },
  occurrenceCounts: {
    HeapSize: 5,
    DecodePointer: 12,
    EncodePointer: 27,
    EnterCriticalSection: 9,
    LeaveCriticalSection: 9,
    GetLastError: 30,
    TlsGetValue: 30,
    FlsGetValue: 30,
    SetLastError: 30,
  },
  memoryContentSha256: "5a7b35e013f2ac108f5d7394dc908ba91a76aaa77dc4e5288d3ee827460c146c",
  reportBytes: 1_154_575,
  reportSha256: "c396de1128554bad61efabc52e1e10f29896a14e923a75d349751cde2eda7686",
  firstUnsupported: {
    capabilityCluster: "initial-thread-stack-growth",
    faultCode: "page-fault",
    instructionCount: 22754,
    instructionPointer: 0x100a47a9,
    previousInstructionPointer: 0x100a47a7,
    faultAddress: 0x006fe000,
    pageFaultErrorCode: 4,
  },
};
const guest = {
  physicalPoolBytes: 2 * 1024 * 1024,
  excludedPhysicalRanges: [{ start: 0x000a0000, end: 0x000c0000, role: "vga-mmio" }],
  instructionBudget: 1,
  maximumExecuteStops: 4_096,
};
const adapterPrivateOffset = {
  instructionCounter: 664,
  memorySize: 812,
};
const stateWord = {
  magic: 0,
  version: 1,
  wordCount: 2,
  eax: 4,
  ecx: 5,
  edx: 6,
  ebx: 7,
  esp: 8,
  ebp: 9,
  esi: 10,
  edi: 11,
  instructionPointer: 12,
  previousInstructionPointer: 13,
  flags: 14,
  cpl: 20,
  protectedMode: 21,
  operandSize32: 22,
  stackSize32: 23,
  inHalt: 24,
  segmentSelectorBase: 28,
  segmentOffsetBase: 36,
  segmentLimitBase: 44,
  segmentMetaBase: 52,
  controlRegisterBase: 60,
};
const segmentIndex = {
  cs: 1,
  fs: 4,
};
const stopReason = {
  budget: 1,
  invalidOpcode: 0x106,
  pageFault: 0x10e,
};

function requireFile(file, command) {
  if (!fs.existsSync(file)) {
    throw new Error(`${file} is missing; run ${command} first`);
  }
}

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function checkedEnd(start, size, limit, label) {
  assert(Number.isSafeInteger(start) && start >= 0, `${label} start is invalid`);
  assert(Number.isSafeInteger(size) && size >= 0, `${label} size is invalid`);
  const end = start + size;
  assert(Number.isSafeInteger(end) && end >= start, `${label} overflows`);
  assert(end <= limit, `${label} exceeds its containing range`);
  return end;
}

function executableFetchWindow(instructionPointer, executablePages) {
  for (let offset = 0; offset < 15; offset += 1) {
    const address = instructionPointer + offset;
    if (address > 0xffff_ffff) {
      return { allowed: false, deniedPage: null };
    }
    const virtualPage = (address & ~(page.size - 1)) >>> 0;
    if (!executablePages.has(virtualPage)) {
      return { allowed: false, deniedPage: virtualPage };
    }
  }
  return { allowed: true, deniedPage: null };
}

function checkedGuestEnd(address, size, label) {
  return checkedEnd(address, size, 0x1_0000_0000, label);
}

function reflectionContentSha256(pages) {
  const hasher = crypto.createHash("sha256");
  for (const reflected of pages) {
    const metadata = Buffer.alloc(11);
    metadata.writeUInt32LE(reflected.address, 0);
    metadata.writeUInt32LE(reflected.owner, 4);
    metadata[8] =
      Number(reflected.protection.read) |
      (Number(reflected.protection.write) << 1) |
      (Number(reflected.protection.execute) << 2);
    metadata[9] = Number(reflected.ownerSealed);
    metadata[10] = Number(reflected.dirty);
    hasher.update(metadata);
    hasher.update(Uint8Array.from(reflected.bytes));
  }
  return hasher.digest("hex");
}

function summarizeOwners(pages) {
  const owners = new Map();
  for (const reflected of pages) {
    let summary = owners.get(reflected.owner);
    if (summary === undefined) {
      summary = {
        owner: reflected.owner,
        pages: 0,
        firstAddress: reflected.address,
        lastAddress: reflected.address,
        readablePages: 0,
        writablePages: 0,
        executablePages: 0,
        hasher: crypto.createHash("sha256"),
      };
      owners.set(reflected.owner, summary);
    }
    summary.pages += 1;
    summary.lastAddress = reflected.address;
    summary.readablePages += Number(reflected.protection.read);
    summary.writablePages += Number(reflected.protection.write);
    summary.executablePages += Number(reflected.protection.execute);
    summary.hasher.update(Uint8Array.from(reflected.bytes));
  }
  return [...owners.values()].map(({ hasher, ...summary }) => ({
    ...summary,
    contentSha256: hasher.digest("hex"),
  }));
}

function sourceIdentity(file) {
  const bytes = fs.readFileSync(file);
  return { bytes: bytes.byteLength, sha256: sha256(bytes) };
}

const checkpointSessionSteps = [
  ["systemTime", "GetSystemTimeAsFileTime", 1, dispatchFirstSystemTimeCall],
  ["processIdentity", "GetCurrentProcessId", 1, dispatchSecondProcessIdCall],
  ["threadIdentity", "GetCurrentThreadId", 1, dispatchThirdThreadIdCall],
  ["monotonicClock", "GetTickCount", 1, dispatchFourthTickCountCall],
  ["performanceCounter", "QueryPerformanceCounter", 1, dispatchFifthPerformanceCounterCall],
  ["processStartup", "GetStartupInfoW", 1, dispatchSixthStartupInfoCall],
  ["heapPolicy", "HeapSetInformation", 1, dispatchSeventhHeapSetInformationCall],
  ["privateHeap", "HeapCreate", 1, dispatchEighthHeapCreateCall],
  ["loadedModule", "GetModuleHandleW", 1, dispatchNinthGetModuleHandleWCall],
  ["dynamicExport", "GetProcAddress", 1, dispatchTenthGetProcAddressCall],
  ["secondDynamicExport", "GetProcAddress", 2, dispatchEleventhGetProcAddressCall],
  ["thirdDynamicExport", "GetProcAddress", 3, dispatchTwelfthGetProcAddressCall],
  ["fourthDynamicExport", "GetProcAddress", 4, dispatchThirteenthGetProcAddressCall],
  ["tlsAllocation", "TlsAlloc", 1, dispatchFourteenthTlsAllocCall],
  ["tlsValue", "TlsSetValue", 1, dispatchFifteenthTlsSetValueCall],
  ["encodedNull", "EncodePointer", 1, dispatchSixteenthEncodePointerCall],
  ["encodedProcessInitializer", "EncodePointer", 2, dispatchSeventeenthEncodePointerCall],
];

async function runCheckpointSession({ child, context, execution, nextMessage, offer }) {
  const selectedSteps = checkpointSessionSteps.slice(0, selectedCheckpoint.supportedCallCount);
  const results = {};
  let continuation = execution;

  for (const [index, [resultKey, symbol, occurrence, dispatch]] of selectedSteps.entries()) {
    const manifestCall = checkpointManifest.checkpoints[index + 1].dispatch;
    assert.equal(manifestCall.module.toLowerCase(), "kernel32.dll");
    assert.equal(manifestCall.symbol, symbol);
    assert.equal(manifestCall.occurrence, occurrence);
    const result = await dispatch({
      child,
      context,
      nextMessage,
      offer,
      ...results,
      execution: continuation,
      continueSession: index + 1 < selectedSteps.length,
    });
    assert(result?.continuation, `${manifestCall.module}!${symbol} did not return a continuation`);
    results[resultKey] = result;
    continuation = result.continuation;
  }

  return results;
}

const checkpointReportBuilders = [
  buildFirstTrapReport,
  buildFirstSystemTimeReport,
  buildSecondProcessIdReport,
  buildThirdThreadIdReport,
  buildFourthTickCountReport,
  buildFifthPerformanceCounterReport,
  buildSixthStartupInfoReport,
  buildSeventhHeapSetInformationReport,
  buildEighthHeapCreateReport,
  buildNinthGetModuleHandleWReport,
  buildTenthGetProcAddressReport,
  buildEleventhGetProcAddressReport,
  buildTwelfthGetProcAddressReport,
  buildThirteenthGetProcAddressReport,
  buildFourteenthTlsAllocReport,
  buildFifteenthTlsSetValueReport,
  buildSixteenthEncodePointerReport,
  buildSeventeenthEncodePointerReport,
];
assert.equal(
  checkpointSessionSteps.length,
  checkpointManifest.checkpoints.length - 1,
  "checkpoint handler registry must cover every manifest call",
);
assert.equal(
  checkpointReportBuilders.length,
  checkpointManifest.checkpoints.length,
  "checkpoint report registry must cover every manifest checkpoint",
);

function buildCheckpointReport(arguments_) {
  const builder = checkpointReportBuilders[selectedCheckpoint.supportedCallCount];
  assert(builder, `no report builder for ${selectedCheckpoint.id}`);
  return builder(arguments_);
}

function buildFirstTrapReport({
  context,
  execution,
  invalidationCalls,
  offer,
  offerMessage,
  ownerSummaries,
  receipt,
  receiptMessage,
  sourceAfter,
  sourceBefore,
}) {
  return {
    schema: "vine.v86-bgi-first-trap",
    schemaVersion: 1,
    source: offer.source,
    sourceSnapshot: {
      immutableAcrossRun: true,
      before: sourceBefore,
      after: sourceAfter,
    },
    contract: {
      largeImageReflection: {
        maximumReflectionPages: protocol.maximumReflectionPages,
        maximumProtocolLineBytes: protocol.maximumLineBytes,
      },
      commandLine: expected.processEnvironment.commandLine,
      stopBeforeDispatchOrReturn: true,
    },
    bootstrap: {
      imageLoadBase: offer.imageLoadBase,
      entryPoint: offer.entryPointAddress,
      initialThreadStack: offer.initialThreadStack,
      initialProcessEnvironment: offer.initialProcessEnvironment,
      fsSelector: 0,
      fsBase: offer.initialProcessEnvironment.tebAddress,
    },
    imports: {
      total: offer.thunks.length,
      declared: offer.thunks.filter(({ disposition }) => disposition === undefined).length,
      unsupported: offer.thunks.filter(
        ({ disposition }) => disposition === "unsupported-trap",
      ).length,
      first: offer.thunks[0],
      last: offer.thunks.at(-1),
    },
    adapter: {
      wasm: expected.wasm,
      physicalPoolBytes: guest.physicalPoolBytes,
      excludedPhysicalRanges: guest.excludedPhysicalRanges,
      pageDirectory: context.pageDirectory,
      pageTables: [...context.pageTables.entries()].map(
        ([directoryIndex, physicalAddress]) => ({ directoryIndex, physicalAddress }),
      ),
      allocatedPages: context.allocations.length,
      reflectedPageAllocations: context.allocations.filter(({ role }) =>
        role.startsWith("reflected-owner-"),
      ).length,
    },
    reflection: {
      offerLineBytes: offerMessage.lineBytes,
      receiptLineBytes: receiptMessage.lineBytes,
      pageSize: offer.reflection.pageSize,
      pages: offer.reflection.pages.length,
      contentSha256: offer.reflection.contentSha256,
      ownerSummaries,
      invalidation: offer.reflection.invalidation,
      acknowledgement: {
        generation: receipt.acknowledgedGeneration,
        callsBeforeAck: invalidationCalls,
        pendingAfterAck: receipt.stats.pendingInvalidation ?? null,
      },
    },
    execution,
    stop: {
      stage: "thunk-dispatch",
      code: "runtime-reachable-owned-import-trap",
      message:
        "BGI entry reached an exact runtime-owned import thunk; API dispatch, return-value fabrication, stack cleanup, and guest resume were not attempted",
    },
  };
}

async function run() {
  requireFile(wasmPath, "npm run prepare:engine");
  requireFile(bgiPath, "the primary title must be installed at its configured path");
  const sourceBefore = sourceIdentity(bgiPath);
  assert.deepEqual(sourceBefore, expected.source);
  const runtimeBindingArguments = declaredDispatches.flatMap(({ module, symbol }) => [
    "--bind-name",
    module,
    symbol,
  ]);

  const probeArguments = [
    "run",
    "--quiet",
    "--locked",
    "-p",
    "vine-pe-probe",
    "--",
    "--v86-reflection-session",
    "--large-image-reflection",
    "--expect-sha256",
    expected.source.sha256,
    "--prepare-memory-pages",
    String(expected.memory.reflectedPages + runtimeHeapPageAllowance),
    "--materialize-image-base",
    `0x${expected.image.loadBase.toString(16)}`,
    "--bind-name",
    "d3d9.dll",
    "Direct3DCreate9",
    ...runtimeBindingArguments,
    "--checkpoint-manifest",
    manifestPath,
    "--checkpoint",
    selectedCheckpoint.id,
    ...(genericRuntimeReentry ? ["--generic-runtime-reentry"] : []),
    ...(genericRuntimeCapability === null
      ? []
      : ["--runtime-capability", genericRuntimeCapability]),
    ...(runtimePolicyPath === null ? [] : ["--runtime-policy", runtimePolicyPath]),
    "--trap-unsupported-imports",
    "--install-callback-return-sentinel",
    "--install-process-entry-return-sentinel",
    "--install-controlled-initial-stack",
    "--install-controlled-process-environment",
    "--install-controlled-process-parameters",
    "--controlled-command-line",
    expected.processEnvironment.commandLine,
    bgiPath,
  ];
  const child = spawn("cargo", probeArguments, {
    cwd: root,
    windowsHide: true,
    stdio: ["pipe", "pipe", "pipe"],
  });
  child.stderr.setEncoding("utf8");
  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  const protocolReader = createProtocolReader(child.stdout);
  const exit = new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => resolve({ code, signal }));
  });
  let sessionCompleted = false;

  const nextMessage = async (label) => {
    const line = await protocolReader.readLine(protocol.maximumLineBytes, label);
    const lineBytes = line.byteLength;
    assert(
      lineBytes <= protocol.maximumLineBytes,
      `${label} exceeds the ${protocol.maximumLineBytes}-byte protocol limit`,
    );
    return { message: JSON.parse(line.toString("utf8")), lineBytes };
  };

  try {
    const offerMessage = await nextMessage("reflection offer");
    const offer = offerMessage.message;
    validateOffer(offer, offerMessage.lineBytes);
    const context = await prepareV86(offer);
    const invalidationCalls = [
      "vine_cpu_state_restore/full_clear_tlb",
      "full_clear_tlb",
      "jit_clear_cache_js",
    ];
    context.wasm.full_clear_tlb();
    context.wasm.jit_clear_cache_js();

    const initialAcknowledgement = `${JSON.stringify({
      schema: "vine.v86-memory-reflection-ack",
      schemaVersion: 1,
      generation: offer.reflection.invalidation.generation,
      dataTlb: true,
      instructionFetch: true,
      generatedCode: true,
    })}\n`;
    if (dispatchSystemTime) {
      child.stdin.write(initialAcknowledgement);
    } else {
      child.stdin.end(initialAcknowledgement);
    }
    const receiptMessage = await nextMessage("reflection receipt");
    const receipt = receiptMessage.message;
    validateReceipt(receipt, offer, context, receiptMessage.lineBytes);
    const runtimeReady = genericRuntimeReentry
      ? await readRuntimeJsonFrame(
          protocolReader,
          runtimeFrame.sessionReady,
          "generic runtime session ready",
        )
      : null;

    const execution = executeToFirstOwnedTrap(offer, context);
    assert.equal(execution.terminal.kind, "owned-import-thunk");
    validateThunk(execution.terminal.thunk, expected.terminal.thunk, !dispatchSystemTime);
    assert.equal(execution.instructionCount, expected.terminal.instructionCount);
    assert.equal(execution.executeStops, expected.terminal.executeStops);
    assert.equal(execution.budgetStops, expected.terminal.budgetStops);
    assert.equal(execution.terminal.returnAddress, expected.terminal.returnAddress);
    assert.deepEqual(execution.terminal.stackWords, expected.terminal.stackWords);
    if (genericRuntimeReentry) {
      assert.equal(
        selectedCheckpoint.supportedCallCount,
        checkpointSessionSteps.length,
        "generic runtime re-entry starts from the frozen full manifest checkpoint",
      );
      const genericResult = await runGenericRuntimeReentry({
        child,
        context,
        execution,
        offer,
        protocolReader,
        runtimeReady,
      });
      child.stdin.end();
      const processExit = await exit;
      assert.equal(processExit.signal, null, `reflection probe terminated by ${processExit.signal}`);
      assert.equal(processExit.code, 0, `reflection probe failed: ${stderr.trim()}`);
      sessionCompleted = true;
      const sourceAfter = sourceIdentity(bgiPath);
      assert.deepEqual(sourceAfter, sourceBefore);
      const report = buildGenericRuntimeReentryReport({
        execution,
        genericResult,
        offer,
        receipt,
        runtimeReady,
        sourceAfter,
        sourceBefore,
      });
      if (genericRuntimeCapability === null) {
        validateGenericRuntimeReentryResult(genericResult);
      } else if (
        genericRuntimeCapability === "kernel32-critical-section-synchronization"
      ) {
        validateCriticalSectionSynchronizationResult(genericResult);
      } else if (genericRuntimeCapability === "kernel32-pointer-decoding") {
        validatePointerDecodingResult(genericResult);
      } else if (initializeCriticalSectionEnabled) {
        validateInitializeCriticalSectionResult(genericResult);
      } else if (heapSizeEnabled) {
        validateHeapSizeResult(genericResult);
      } else if (setUnhandledExceptionFilterEnabled) {
        validateSetUnhandledExceptionFilterResult(genericResult);
      } else if (isProcessorFeaturePresentEnabled) {
        validateIsProcessorFeaturePresentResult(genericResult);
      } else if (privateHeapSize128Enabled) {
        validatePrivateHeapSize128Result(genericResult);
      } else if (privateHeapFreeSize1Enabled) {
        validatePrivateHeapFreeSize1Result(genericResult);
      } else if (privateHeapSize4Enabled) {
        validatePrivateHeapSize4Result(genericResult);
      } else if (privateHeapSize16Enabled) {
        validatePrivateHeapSize16Result(genericResult);
      } else if (getModuleFileNameAEnabled) {
        validateGetModuleFileNameAResult(genericResult);
      } else if (interlockedDecrementEnabled) {
        validateInterlockedDecrementResult(genericResult);
      } else if (lcMapStringWEnabled) {
        validateLCMapStringWResult(genericResult);
      } else if (getStringTypeWEnabled) {
        validateGetStringTypeWResult(genericResult);
      } else if (multiByteToWideCharEnabled) {
        validateMultiByteToWideCharResult(genericResult);
      } else if (getCpInfoEnabled) {
        validateGetCpInfoResult(genericResult);
      } else if (privateHeapSize544Enabled) {
        validatePrivateHeapSize544Result(genericResult);
      } else if (getAcpEnabled) {
        validateGetAcpResult(genericResult);
      } else if (setLastErrorEnabled) {
        validateSetLastErrorResult(genericResult);
      } else if (flsGetValueEnabled) {
        validateFlsGetValueResult(genericResult);
      } else if (tlsGetValueEnabled) {
        validateTlsGetValueResult(genericResult);
      } else if (getLastErrorEnabled) {
        validateGetLastErrorResult(genericResult);
      } else if (privateHeapSize1Enabled) {
        validatePrivateHeapSize1Result(genericResult);
      } else if (wideCharToMultiByteEnabled) {
        validateWideCharToMultiByteResult(genericResult);
      } else if (environmentStringsWEnabled) {
        validateEnvironmentStringsWResult(genericResult);
      } else if (commandLineAEnabled) {
        validateCommandLineAResult(genericResult);
      } else if (setHandleCountEnabled) {
        validateSetHandleCountResult(genericResult);
      } else if (standardErrorFileTypeEnabled) {
        validateStandardErrorFileTypeResult(genericResult);
      } else if (standardErrorHandleEnabled) {
        validateStandardErrorHandleResult(genericResult);
      } else if (standardOutputFileTypeEnabled) {
        validateStandardOutputFileTypeResult(genericResult);
      } else if (standardOutputHandleEnabled) {
        validateStandardOutputHandleResult(genericResult);
      } else if (fileTypeEnabled) {
        validateFileTypeResult(genericResult);
      } else if (standardInputHandleEnabled) {
        validateStandardInputHandleResult(genericResult);
      } else if (privateHeapSecondAllocationEnabled) {
        validatePrivateHeapSecondAllocationResult(genericResult);
      } else if (interlockedIncrementEnabled) {
        validateInterlockedIncrementResult(genericResult);
      } else if (fiberLocalStorageValueEnabled) {
        validateFiberLocalStorageValueResult(genericResult);
      } else if (privateHeapAllocationEnabled) {
        validatePrivateHeapAllocationResult(genericResult);
      } else {
        validateFiberLocalStorageAllocationResult(genericResult);
      }
      fs.mkdirSync(outputDirectory, { recursive: true });
      const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
      fs.writeFileSync(reportPath, reportBytes);
      const checkpointLabel =
        genericRuntimeCapability === null
          ? "bgi generic runtime re-entry"
          : initializeCriticalSectionEnabled
            ? "bgi InitializeCriticalSection"
            : heapSizeEnabled
            ? "bgi HeapSize"
            : setUnhandledExceptionFilterEnabled
            ? "bgi SetUnhandledExceptionFilter"
            : isProcessorFeaturePresentEnabled
            ? "bgi IsProcessorFeaturePresent"
            : privateHeapSize128Enabled
            ? "bgi private-heap size-128 allocation"
            : privateHeapFreeSize1Enabled
            ? "bgi private-heap size-1 free"
            : privateHeapSize4Enabled
            ? "bgi private-heap size-4 allocation"
            : privateHeapSize16Enabled
            ? "bgi private-heap size-16 allocation"
            : getModuleFileNameAEnabled
            ? "bgi GetModuleFileNameA"
            : interlockedDecrementEnabled
            ? "bgi InterlockedDecrement"
            : lcMapStringWEnabled
            ? "bgi LCMapStringW"
            : getStringTypeWEnabled
            ? "bgi GetStringTypeW"
            : multiByteToWideCharEnabled
            ? "bgi MultiByteToWideChar"
            : getCpInfoEnabled
            ? "bgi GetCPInfo"
            : privateHeapSize544Enabled
            ? "bgi private-heap size-544 allocation"
            : getAcpEnabled
            ? "bgi GetACP"
            : setLastErrorEnabled
            ? "bgi SetLastError"
            : flsGetValueEnabled
            ? "bgi FlsGetValue"
            : tlsGetValueEnabled
            ? "bgi TlsGetValue"
            : getLastErrorEnabled
            ? "bgi GetLastError"
            : privateHeapSize1Enabled
            ? "bgi private-heap size-1 allocation"
            : wideCharToMultiByteEnabled
            ? "bgi WideCharToMultiByte"
            : environmentStringsWEnabled
            ? "bgi environment strings W"
            : commandLineAEnabled
            ? "bgi command line A"
            : setHandleCountEnabled
            ? "bgi set handle count"
            : standardErrorFileTypeEnabled
            ? "bgi standard error file type"
            : standardErrorHandleEnabled
            ? "bgi standard error handle"
            : standardOutputFileTypeEnabled
            ? "bgi standard output file type"
            : standardOutputHandleEnabled
            ? "bgi standard output handle"
            : fileTypeEnabled
            ? "bgi file type"
            : standardInputHandleEnabled
            ? "bgi standard input handle"
            : privateHeapSecondAllocationEnabled
            ? "bgi private-heap second allocation"
            : interlockedIncrementEnabled
            ? "bgi interlocked increment"
            : fiberLocalStorageValueEnabled
            ? "bgi fiber-local-storage value"
            : privateHeapAllocationEnabled
            ? "bgi private-heap allocation"
            : fiberLocalStorageAllocationEnabled
            ? "bgi fiber-local-storage allocation"
            : genericRuntimeCapability === "kernel32-pointer-decoding"
              ? "bgi pointer decoding"
              : "bgi critical-section synchronization";
      console.log(
        `${checkpointLabel} passed: ${genericResult.supportedCalls.length} generic calls, ` +
          `first structured stop cluster ${genericResult.fault.capabilityCluster}`,
      );
      console.log(
        `reflection sha256 ${offer.reflection.contentSha256}; report: ` +
          `${path.relative(root, reportPath)}, ${reportBytes.byteLength} bytes, ` +
          `sha256 ${sha256(reportBytes)}`,
      );
      return;
    }
    const callResults = await runCheckpointSession({
      child,
      context,
      execution,
      nextMessage,
      offer,
    });
    const {
      systemTime = null,
      processIdentity = null,
      threadIdentity = null,
      monotonicClock = null,
      performanceCounter = null,
      processStartup = null,
      heapPolicy = null,
      privateHeap = null,
      loadedModule = null,
      dynamicExport = null,
      secondDynamicExport = null,
      thirdDynamicExport = null,
      fourthDynamicExport = null,
      tlsAllocation = null,
      tlsValue = null,
      encodedNull = null,
      encodedProcessInitializer = null,
    } = callResults;
    const processExit = await exit;
    assert.equal(processExit.signal, null, `reflection probe terminated by ${processExit.signal}`);
    assert.equal(processExit.code, 0, `reflection probe failed: ${stderr.trim()}`);
    sessionCompleted = true;
    const sourceAfter = sourceIdentity(bgiPath);
    assert.deepEqual(sourceAfter, sourceBefore);
    const ownerSummaries = summarizeOwners(offer.reflection.pages);
    const report = buildCheckpointReport({
      context,
      dynamicExport,
      encodedNull,
      encodedProcessInitializer,
      execution,
      fourthDynamicExport,
      heapPolicy,
      invalidationCalls,
      loadedModule,
      monotonicClock,
      offer,
      offerMessage,
      ownerSummaries,
      performanceCounter,
      privateHeap,
      processIdentity,
      processStartup,
      receipt,
      receiptMessage,
      secondDynamicExport,
      sourceAfter,
      sourceBefore,
      systemTime,
      thirdDynamicExport,
      threadIdentity,
      tlsAllocation,
      tlsValue,
    });
    fs.mkdirSync(outputDirectory, { recursive: true });
    const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
    assert.equal(report.schema, selectedCheckpoint.evidence.reportSchema);
    const historicalReportIdentity = observeArtifactIdentity(reportBytes, {
      bytes: selectedCheckpoint.evidence.reportBytes,
      sha256: selectedCheckpoint.evidence.reportSha256,
    });
    fs.writeFileSync(reportPath, reportBytes);
    const finalExecution =
      selectedCheckpoint.supportedCallCount === 0
        ? execution
        : callResults[
            checkpointSessionSteps[selectedCheckpoint.supportedCallCount - 1][0]
          ].continuation;
    const nextThunk = finalExecution.terminal.thunk;
    assert.equal(nextThunk.module.toLowerCase(), selectedCheckpoint.nextStop.module.toLowerCase());
    assert.equal(nextThunk.name, selectedCheckpoint.nextStop.symbol);
    assert.equal(finalExecution.instructionCount, selectedCheckpoint.evidence.instructionCount);
    assert.deepEqual(
      finalExecution.terminal.stackWords.slice(
        1,
        selectedCheckpoint.nextStop.arguments.length + 1,
      ),
      selectedCheckpoint.nextStop.arguments.map((argument) => Number.parseInt(argument, 16)),
    );
    console.log(
      `${selectedCheckpoint.id} passed: ${selectedCheckpoint.supportedCallCount} dispatched calls, ` +
        `next ${nextThunk.module}!${nextThunk.name ?? `#${nextThunk.ordinal}`} after ` +
        `${finalExecution.instructionCount} total instructions`,
    );
    console.log(
      `reflection sha256 ${offer.reflection.contentSha256}; report: ` +
        `${path.relative(root, reportPath)}, ${reportBytes.byteLength} bytes, ` +
        `sha256 ${sha256(reportBytes)}`,
    );
    console.log(
      `historical report artifact ${historicalReportIdentity.matches ? "matched" : "changed"}; ` +
        `classification=${historicalReportIdentity.classification}, semantic gate=independent`,
    );
  } catch (error) {
    if (stderr.trim() !== "") {
      error.message = `${error.message}\nprobe stderr:\n${stderr.trim()}`;
    }
    throw error;
  } finally {
    if (!sessionCompleted && child.exitCode === null) {
      child.kill();
    }
  }
}

async function runGenericRuntimeReentry({
  child,
  context,
  execution,
  offer,
  protocolReader,
  runtimeReady,
}) {
  validateGenericRuntimeReady(runtimeReady, offer);
  let generation = runtimeReady.memory.acknowledgedGeneration;
  let continuation = execution;
  const occurrences = new Map();
  const supportedCalls = [];
  const writebacks = [];
  const hostDeltas = [];
  let frozenSemanticCalls = 0;

  const callLimit = initializeCriticalSectionEnabled ? 4096 : 256;
  for (let sequence = 1; sequence <= callLimit; sequence += 1) {
    const terminal = continuation.terminal;
    const ownedTerminal =
      terminal.kind === "owned-import-thunk" ||
      terminal.kind === "owned-dynamic-export-thunk";
    if (!ownedTerminal) {
      assert(
        initializeCriticalSectionEnabled && terminal.kind === "page-fault",
        `generic runtime reached non-owned terminal at call ${sequence}, ` +
          `instruction ${continuation.instructionCount}: ${JSON.stringify(terminal)}`,
      );
    }
    const endpoint = ownedTerminal ? genericTerminalEndpoint(terminal) : null;
    const occurrenceKey =
      endpoint === null ? null : `${endpoint.module.toLowerCase()}!${endpoint.symbol}`;
    const occurrence = occurrenceKey === null ? null : (occurrences.get(occurrenceKey) ?? 0) + 1;

    if (endpoint !== null && sequence <= checkpointManifest.checkpoints.length) {
      const stopCheckpoint = checkpointManifest.checkpoints[sequence - 1];
      assertGenericStopMatchesCheckpoint(
        continuation,
        endpoint,
        occurrence,
        stopCheckpoint,
      );
    }

    const discovery = discoverDirtyMirrorPages(context);
    const writeback = encodeCpuWriteback(generation, discovery.pages);
    await writeRuntimeFrame(
      child.stdin,
      runtimeFrame.cpuWriteback,
      writeback.control,
      writeback.payload,
    );
    const receiptFrame = await readRuntimeFrame(
      protocolReader,
      runtimeFrame.writebackReceipt,
      "CPU writeback receipt",
    );
    assert.equal(receiptFrame.payload.length, 0);
    const writebackReceipt = decodeWritebackReceipt(receiptFrame.control);
    validateGenericWritebackReceipt(writebackReceipt, generation, discovery.pages.length);
    generation = writebackReceipt.generation;
    writebacks.push({
      sequence,
      instructionCount: continuation.instructionCount,
      scannedWritablePages: discovery.scannedWritablePages,
      dirtyPages: discovery.pages.map(({ address, owner }) => ({ address, owner })),
      cleanWritablePages: discovery.cleanWritablePages,
      clearedDirtyBits: discovery.pages.length,
      tlbFlushAfterClear: true,
      controlBytes: writeback.control.length,
      payloadBytes: writeback.payload.length,
      payloadSha256: sha256(writeback.payload),
      receipt: writebackReceipt,
    });

    if (endpoint === null) {
      const reason = "initial-thread-stack-growth";
      await writeRuntimeJsonFrame(child.stdin, runtimeFrame.sessionStop, {
        schema: "vine.v86-generic-runtime-session-stop",
        schemaVersion: 1,
        generation,
        reason,
      });
      const stoppedFrame = await readRuntimeFrame(
        protocolReader,
        runtimeFrame.sessionStopped,
        "generic runtime session stopped",
      );
      assert.equal(stoppedFrame.payload.length, 0);
      const stopped = JSON.parse(stoppedFrame.control.toString("utf8"));
      assert.equal(stopped.schema, "vine.v86-generic-runtime-session-stopped");
      assert.equal(stopped.schemaVersion, 1);
      assert.equal(stopped.reason, reason);
      assert.equal(stopped.memory.generation, generation);
      assert.equal(stopped.memory.acknowledgedGeneration, generation);
      assert.match(stopped.memoryContentSha256, /^[0-9a-f]{64}$/);
      return {
        frozenSemanticCalls,
        supportedCalls,
        writebacks,
        hostDeltas,
        finalGeneration: generation,
        fault: {
          schema: "vine.v86-generic-runtime-execution-fault",
          schemaVersion: 1,
          capabilityCluster: reason,
          endpoint: null,
          fault: {
            class: "fault",
            stage: "guest-execution",
            code: "page-fault",
            message:
              `guest page fault at 0x${terminal.faultAddress.toString(16)} ` +
              `while executing 0x${terminal.instructionPointer.toString(16)}`,
            instructionPointer: terminal.instructionPointer,
            previousInstructionPointer: terminal.previousInstructionPointer,
            faultAddress: terminal.faultAddress,
            pageFaultErrorCode: terminal.pageFaultErrorCode,
          },
          memory: stopped.memory,
          memoryContentSha256: stopped.memoryContentSha256,
        },
        terminal: continuation,
      };
    }

    const trapRequest = {
      schema: "vine.v86-generic-runtime-trap",
      schemaVersion: 1,
      kind:
        terminal.kind === "owned-import-thunk"
          ? "runtime-system-export"
          : "dynamic-export",
      endpointId: endpoint.id,
      endpointAddress: endpoint.address,
      stackPointer: continuation.finalState.esp,
      returnAddress: terminal.returnAddress,
    };
    const criticalSectionBytesBefore =
      endpoint.symbol === "InitializeCriticalSectionAndSpinCount"
        ? Buffer.from(
            context.readVirtualBytes(
              terminal.stackWords[1],
              24,
              "critical-section storage before unsupported dispatch",
            ),
          )
        : null;
    await writeRuntimeJsonFrame(child.stdin, runtimeFrame.trap, trapRequest);

    let response = await readRuntimeFrame(protocolReader, null, "generic API response");
    if (response.kind === runtimeFrame.hostDelta) {
      const delta = decodeRuntimeHostDelta(response.control, response.payload);
      assert.equal(delta.fromGeneration, generation);
      applyRuntimeHostDelta(context, delta);
      generation = delta.generation;
      hostDeltas.push({
        sequence,
        fromGeneration: delta.fromGeneration,
        generation: delta.generation,
        mappingOperations: delta.mappingOperations.length,
        dirtyPages: delta.dirtyPages.map(({ address, owner }) => ({ address, owner })),
        controlBytes: response.control.length,
        payloadBytes: response.payload.length,
        payloadSha256: sha256(response.payload),
        cacheEffects: delta.cacheEffects,
      });
      await writeRuntimeFrame(
        child.stdin,
        runtimeFrame.acknowledgement,
        encodeGeneration(generation),
        Buffer.alloc(0),
      );
      response = await readRuntimeFrame(protocolReader, null, "generic API completion");
    }

    if (response.kind === runtimeFrame.apiFault) {
      assert.equal(response.payload.length, 0);
      const fault = JSON.parse(response.control.toString("utf8"));
      validateGenericRuntimeFault(fault, endpoint, generation);
      if (criticalSectionBytesBefore !== null) {
        assert.deepEqual(
          Buffer.from(
            context.readVirtualBytes(
              terminal.stackWords[1],
              criticalSectionBytesBefore.length,
              "critical-section storage after unsupported dispatch",
            ),
          ),
          criticalSectionBytesBefore,
          "unsupported critical-section dispatch mutated guest storage",
        );
      }
      return {
        frozenSemanticCalls,
        supportedCalls,
        writebacks,
        hostDeltas,
        finalGeneration: generation,
        fault,
        terminal: continuation,
      };
    }
    assert.equal(response.kind, runtimeFrame.apiCompletion);
    assert.equal(response.payload.length, 0);
    const completionFrame = JSON.parse(response.control.toString("utf8"));
    validateGenericCompletionFrame(completionFrame, endpoint, generation);
    occurrences.set(occurrenceKey, occurrence);
    const call = {
      sequence,
      module: endpoint.module,
      symbol: endpoint.symbol,
      occurrence,
      trapInstructionCount: continuation.instructionCount,
      arguments: completionFrame.completion.frame.arguments.flatMap(({ words }) => words),
      result: completionFrame.completion.eax,
      implementationId: completionFrame.completion.implementationId,
      returnAddress: completionFrame.completion.instructionPointer,
      stackPointerBefore: completionFrame.completion.frame.stackPointer,
      stackPointerAfter: completionFrame.completion.stackPointer,
      lastError: completionFrame.completion.lastError,
      guestWrites: completionFrame.completion.guestWrites.map(({ write, result }) => ({
        address: write.address,
        bytes: write.bytes,
        result,
      })),
      ...(completionFrame.flsValue === undefined
        ? {}
        : { flsValue: completionFrame.flsValue }),
      ...(completionFrame.interlockedIncrement === undefined
        ? {}
        : { interlockedIncrement: completionFrame.interlockedIncrement }),
      ...(completionFrame.interlockedDecrement === undefined
        ? {}
        : { interlockedDecrement: completionFrame.interlockedDecrement }),
      ...(completionFrame.heapAllocation === undefined
        ? {}
        : { heapAllocation: completionFrame.heapAllocation }),
      ...(completionFrame.standardHandle === undefined
        ? {}
        : { standardHandle: completionFrame.standardHandle }),
      ...(completionFrame.fileType === undefined
        ? {}
        : { fileType: completionFrame.fileType }),
    };

    const manifestCheckpoint =
      sequence <= selectedCheckpoint.supportedCallCount
        ? checkpointManifest.checkpoints[sequence]
        : null;
    let postReturnGuestWrites = [];
    if (manifestCheckpoint !== null) {
      validateGenericCallAgainstManifest(call, manifestCheckpoint);
      const dispatcherWriteAddresses = new Set(
        completionFrame.completion.guestWrites.map(({ write }) => write.address),
      );
      const dispatcherWrites = manifestCheckpoint.expected.guestWrites.filter(({ address }) =>
        dispatcherWriteAddresses.has(parseManifestInteger(address)),
      );
      postReturnGuestWrites = manifestCheckpoint.expected.guestWrites.filter(
        ({ address }) => !dispatcherWriteAddresses.has(parseManifestInteger(address)),
      );
      validateManifestGuestWrites(context, dispatcherWrites);
      frozenSemanticCalls += 1;
    }
    supportedCalls.push(call);
    restoreGenericApiCompletion(context, continuation.finalState, completionFrame.completion);
    const next = executeToNextOwnedTrap(offer, context, continuation.instructionCount);
    if (manifestCheckpoint !== null) {
      validateManifestGuestWrites(context, postReturnGuestWrites);
      const nextEndpoint = genericTerminalEndpoint(next.terminal);
      const nextKey = `${nextEndpoint.module.toLowerCase()}!${nextEndpoint.symbol}`;
      const nextOccurrence = (occurrences.get(nextKey) ?? 0) + 1;
      assertGenericStopMatchesCheckpoint(
        next,
        nextEndpoint,
        nextOccurrence,
        manifestCheckpoint,
      );
    }
    continuation = next;
  }
  throw new Error(
    `generic runtime re-entry exceeded its bounded ${callLimit}-call limit`,
  );
}

function validateGenericRuntimeReady(ready, offer) {
  assert.equal(ready.schema, "vine.v86-generic-runtime-session-ready");
  assert.equal(ready.schemaVersion, 1);
  assert.equal(ready.processId, controlledProcess.processId);
  assert.equal(ready.currentThreadId, controlledThread.threadId);
  assert.deepEqual(ready.processLifecycle, { state: "running" });
  assert.deepEqual(ready.schedulerState, { state: "runnable" });
  assert.equal(ready.dllLifecycle, "process-attached");
  assert.equal(ready.modules.length, expected.imports.modules + 1);
  assert.equal(ready.dependencyEdges, expected.imports.modules);
  assert.equal(ready.runtimeExportEndpoints, expected.imports.total);
  assert.equal(
    ready.implementedApiContracts,
    declaredDispatches.length +
      (genericRuntimeCapability === null
        ? 0
        : initializeCriticalSectionEnabled
          ? 31
          : heapSizeEnabled
          ? 30
          : setUnhandledExceptionFilterEnabled
          ? 29
          : isProcessorFeaturePresentEnabled
          ? 28
          : privateHeapSize128Enabled
          ? 27
          : privateHeapFreeSize1Enabled
          ? 27
          : privateHeapSize4Enabled
          ? 26
          : privateHeapSize16Enabled
          ? 26
          : getModuleFileNameAEnabled
          ? 26
          : interlockedDecrementEnabled
          ? 25
          : lcMapStringWEnabled
          ? 24
          : getStringTypeWEnabled
          ? 23
          : multiByteToWideCharEnabled
          ? 22
          : getCpInfoEnabled
          ? 21
          : getAcpEnabled
          ? 20
          : setLastErrorEnabled
          ? 19
          : flsGetValueEnabled
          ? 18
          : tlsGetValueEnabled
          ? 17
          : getLastErrorEnabled
          ? 16
          : wideCharToMultiByteEnabled
          ? 15
          : environmentStringsWEnabled
          ? 14
          : commandLineAEnabled
          ? 12
          : setHandleCountEnabled
          ? 11
          : fileTypeEnabled
          ? 10
          : standardInputHandleEnabled
          ? 9
          : privateHeapSecondAllocationEnabled
          ? 8
          : interlockedIncrementEnabled
          ? 8
          : fiberLocalStorageValueEnabled
          ? 7
          : privateHeapAllocationEnabled
          ? 6
          : fiberLocalStorageAllocationEnabled
          ? 5
          : genericRuntimeCapability === "kernel32-pointer-decoding"
            ? 4
            : 3),
  );
  assert.equal(ready.dynamicExportEndpoints, 4);
  assert.equal(ready.memory.generation, offer.reflection.invalidation.generation);
  assert.equal(ready.memory.acknowledgedGeneration, offer.reflection.invalidation.generation);
  assert.equal(ready.memory.dirtyPages, 0);
  assert.equal(ready.memory.pendingInvalidation, undefined);
  assert.equal(ready.attachOrder.length, ready.modules.length);
  const kernel32 = ready.modules.find(({ canonicalName }) => canonicalName === "kernel32.dll");
  assert(kernel32, "generic runtime process has no KERNEL32 module");
  assert.equal(kernel32.handle, controlledLoadedModule.handle.value);
  const main = ready.modules.find(({ handle }) => handle === ready.mainModuleHandle);
  assert(main, "generic runtime process has no main executable module");
  assert.equal(main.canonicalName, "bgi.exe");
}

function genericTerminalEndpoint(terminal) {
  if (terminal.kind === "owned-import-thunk") {
    return {
      kind: "runtime-system-export",
      id: terminal.thunk.id,
      address: terminal.thunk.address,
      module: terminal.thunk.module,
      symbol: terminal.thunk.name ?? `#${terminal.thunk.ordinal}`,
    };
  }
  assert.equal(terminal.kind, "owned-dynamic-export-thunk");
  return {
    kind: "dynamic-export",
    id: terminal.endpoint.id,
    address: terminal.endpoint.address,
    module: terminal.endpoint.module,
    symbol: terminal.endpoint.name,
  };
}

function assertGenericStopMatchesCheckpoint(
  execution,
  endpoint,
  occurrence,
  checkpoint,
) {
  assert.equal(endpoint.module.toLowerCase(), checkpoint.nextStop.module.toLowerCase());
  assert.equal(endpoint.symbol, checkpoint.nextStop.symbol);
  assert.equal(occurrence, checkpoint.nextStop.occurrence);
  assert.equal(execution.instructionCount, checkpoint.evidence.instructionCount);
  assert.deepEqual(
    execution.terminal.stackWords.slice(1, checkpoint.nextStop.arguments.length + 1),
    checkpoint.nextStop.arguments.map(parseManifestInteger),
  );
}

function validateGenericCallAgainstManifest(call, checkpoint) {
  assert.equal(call.module.toLowerCase(), checkpoint.dispatch.module.toLowerCase());
  assert.equal(call.symbol, checkpoint.dispatch.symbol);
  assert.equal(call.occurrence, checkpoint.dispatch.occurrence);
  assert.deepEqual(call.arguments, checkpoint.expected.arguments.map(parseManifestInteger));
  if (checkpoint.expected.result.kind === "void") {
    assert.deepEqual(call.result, { action: "preserve" });
  } else {
    assert.equal(call.result.action, "set");
    assert.equal(call.result.value, parseManifestInteger(checkpoint.expected.result.value));
  }
}

function validateManifestGuestWrites(context, writes) {
  for (const write of writes) {
    const address = parseManifestInteger(write.address);
    if (write.kind === "u32-le") {
      assert.equal(
        context.readVirtualU32(address, `manifest guest write 0x${address.toString(16)}`),
        parseManifestInteger(write.value),
      );
    } else if (write.kind === "u64-le") {
      const bytes = context.readVirtualBytes(
        address,
        8,
        `manifest guest write 0x${address.toString(16)}`,
      );
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      assert.equal(view.getBigUint64(0, true), BigInt(write.value));
    } else if (write.kind === "struct") {
      assert.equal(write.value, controlledProcessStartup.profileId);
      const expectedBytes = Buffer.alloc(controlledProcessStartup.structureSize);
      expectedBytes.writeUInt32LE(controlledProcessStartup.structureSize, 0);
      assert.deepEqual(
        Buffer.from(
          context.readVirtualBytes(
            address,
            expectedBytes.length,
            `manifest guest struct write 0x${address.toString(16)}`,
          ),
        ),
        expectedBytes,
      );
    } else {
      assert.fail(`unsupported manifest guest-write kind ${write.kind}`);
    }
  }
}

function parseManifestInteger(value) {
  if (typeof value === "number") {
    return value;
  }
  assert.equal(typeof value, "string");
  return Number.parseInt(value, value.startsWith("0x") ? 16 : 10);
}

function discoverDirtyMirrorPages(context) {
  const writable = [...context.mappings.entries()]
    .filter(([, mapping]) => mapping.protection.write)
    .sort(([left], [right]) => left - right);
  const pages = [];
  const cleanWritablePages = [];
  for (const [address, mapping] of writable) {
    const pte = context.physicalView.getUint32(mapping.pteAddress, true);
    if ((pte & page.dirty) === 0) {
      cleanWritablePages.push(address);
      continue;
    }
    const bytes = Buffer.from(
      context.physicalMemory.buffer,
      context.physicalMemory.byteOffset + mapping.physicalBase,
      page.size,
    );
    pages.push({ address, owner: mapping.owner, bytes: Buffer.from(bytes) });
    context.physicalView.setUint32(mapping.pteAddress, pte & ~page.dirty, true);
  }
  context.wasm.full_clear_tlb();
  return {
    scannedWritablePages: writable.length,
    pages,
    cleanWritablePages,
  };
}

function encodeCpuWriteback(generation, pages) {
  const control = Buffer.alloc(
    writebackFormat.headerBytes + pages.length * writebackFormat.descriptorBytes,
  );
  control.write(writebackFormat.magic, 0, "ascii");
  const view = dataView(control);
  view.setUint32(8, writebackFormat.version, true);
  view.setUint32(12, page.size, true);
  view.setBigUint64(16, BigInt(generation), true);
  view.setUint32(24, pages.length, true);
  view.setUint32(28, writebackFormat.descriptorBytes, true);
  const payload = Buffer.alloc(pages.length * page.size);
  pages.forEach((dirty, index) => {
    const offset = writebackFormat.headerBytes + index * writebackFormat.descriptorBytes;
    view.setUint32(offset, dirty.address, true);
    view.setUint32(offset + 4, dirty.owner, true);
    dirty.bytes.copy(payload, index * page.size);
  });
  return { control, payload };
}

function decodeWritebackReceipt(control) {
  assert.equal(control.length, 120);
  assert.equal(control.subarray(0, 8).toString("ascii"), "VINEWR01");
  const view = dataView(control);
  assert.equal(view.getUint32(8, true), 1);
  return {
    fromGeneration: safeNumber(view.getBigUint64(16, true), "receipt from generation"),
    generation: safeNumber(view.getBigUint64(24, true), "receipt generation"),
    touchedPages: view.getUint32(32, true),
    changedPages: view.getUint32(36, true),
    unchangedPages: view.getUint32(40, true),
    executablePages: view.getUint32(44, true),
    changedBytes: safeNumber(view.getBigUint64(48, true), "receipt changed bytes"),
    transferredBytes: safeNumber(
      view.getBigUint64(56, true),
      "receipt transferred bytes",
    ),
    contentSha256: control.subarray(64, 96).toString("hex"),
    rustGeneration: safeNumber(view.getBigUint64(96, true), "Rust generation"),
    rustAcknowledgedGeneration: safeNumber(
      view.getBigUint64(104, true),
      "Rust acknowledged generation",
    ),
    rustDirtyPages: view.getUint32(112, true),
    rustPendingInvalidation: Boolean(view.getUint32(116, true)),
  };
}

function validateGenericWritebackReceipt(receipt, generation, dirtyPages) {
  assert.equal(receipt.fromGeneration, generation);
  assert.equal(receipt.touchedPages, dirtyPages);
  assert.equal(receipt.changedPages + receipt.unchangedPages, dirtyPages);
  assert.equal(
    receipt.generation,
    generation + Number(receipt.changedPages > 0),
  );
  assert.equal(receipt.transferredBytes, dirtyPages * page.size);
  assert.equal(receipt.rustGeneration, receipt.generation);
  assert.equal(receipt.rustAcknowledgedGeneration, receipt.generation);
  assert.equal(receipt.rustDirtyPages, 0);
  assert.equal(receipt.rustPendingInvalidation, false);
}

function decodeRuntimeHostDelta(control, payload) {
  assert.equal(control.subarray(0, 8).toString("ascii"), deltaFormat.magic);
  const view = dataView(control);
  assert.equal(view.getUint32(8, true), deltaFormat.version);
  assert.equal(view.getUint32(12, true), page.size);
  const fromGeneration = safeNumber(view.getBigUint64(16, true), "delta from generation");
  const generation = safeNumber(view.getBigUint64(24, true), "delta generation");
  const mappingCount = view.getUint32(32, true);
  assert.equal(view.getUint32(36, true), deltaFormat.mappingDescriptorBytes);
  const dirtyCount = view.getUint32(40, true);
  assert.equal(view.getUint32(44, true), deltaFormat.pageDescriptorBytes);
  const cacheFlags = view.getUint32(48, true);
  assert.equal(
    control.length,
    deltaFormat.headerBytes +
      mappingCount * deltaFormat.mappingDescriptorBytes +
      dirtyCount * deltaFormat.pageDescriptorBytes,
  );
  assert.equal(payload.length, dirtyCount * page.size);
  const mappingOperations = [];
  for (let index = 0; index < mappingCount; index += 1) {
    const offset = deltaFormat.headerBytes + index * deltaFormat.mappingDescriptorBytes;
    mappingOperations.push({
      operation: view.getUint32(offset, true),
      owner: view.getUint32(offset + 4, true),
      regionKind: view.getUint32(offset + 8, true),
      protectionFlags: view.getUint32(offset + 12, true),
      protection: {
        read: Boolean(view.getUint32(offset + 12, true) & 1),
        write: Boolean(view.getUint32(offset + 12, true) & 2),
        execute: Boolean(view.getUint32(offset + 12, true) & 4),
      },
      address: view.getUint32(offset + 16, true),
      byteLength: safeNumber(view.getBigUint64(offset + 20, true), "mapping byte length"),
    });
  }
  const dirtyPages = [];
  const dirtyBase =
    deltaFormat.headerBytes + mappingCount * deltaFormat.mappingDescriptorBytes;
  for (let index = 0; index < dirtyCount; index += 1) {
    const offset = dirtyBase + index * deltaFormat.pageDescriptorBytes;
    const flags = view.getUint32(offset + 8, true);
    dirtyPages.push({
      address: view.getUint32(offset, true),
      owner: view.getUint32(offset + 4, true),
      protection: {
        read: Boolean(flags & 1),
        write: Boolean(flags & 2),
        execute: Boolean(flags & 4),
      },
      ownerSealed: Boolean(flags & 8),
      bytes: payload.subarray(index * page.size, (index + 1) * page.size),
    });
  }
  return {
    fromGeneration,
    generation,
    mappingOperations,
    dirtyPages,
    cacheEffects: {
      flushTlb: Boolean(cacheFlags & 1),
      invalidateInstructionFetch: Boolean(cacheFlags & 2),
      invalidateGeneratedCode: Boolean(cacheFlags & 4),
    },
  };
}

function applyRuntimeHostDelta(context, delta) {
  const dirtyByAddress = new Map(delta.dirtyPages.map((dirty) => [dirty.address, dirty]));
  for (const operation of delta.mappingOperations) {
    const end = checkedEnd(
      operation.address,
      operation.byteLength,
      0x1_0000_0000,
      "runtime host mapping operation",
    );
    if (operation.operation === 1) {
      assert.equal(operation.regionKind, 7, "runtime host reserve is not a heap region");
      assert.deepEqual(operation.protection, {
        read: false,
        write: false,
        execute: false,
      });
      continue;
    }
    assert.equal(operation.operation, 2, "unsupported runtime host mapping operation");
    for (let address = operation.address; address < end; address += page.size) {
      const dirty = dirtyByAddress.get(address);
      assert(dirty, "committed runtime host page has no bytes");
      assert.equal(dirty.owner, operation.owner);
      assert.equal(dirty.ownerSealed, false);
      assert.deepEqual(dirty.protection, operation.protection);
      context.mapRuntimePage(
        address,
        operation.owner,
        dirty.ownerSealed,
        operation.protection,
        dirty.bytes,
        `host-committed-owner-${operation.owner}`,
      );
      dirtyByAddress.delete(address);
    }
  }
  for (const dirty of dirtyByAddress.values()) {
    const mapping = context.mappings.get(dirty.address);
    assert(mapping, `host dirty page 0x${dirty.address.toString(16)} is unmapped`);
    assert.equal(mapping.owner, dirty.owner);
    assert.equal(mapping.ownerSealed, dirty.ownerSealed);
    assert.deepEqual(mapping.protection, dirty.protection);
    context.physicalMemory.set(dirty.bytes, mapping.physicalBase);
  }
  if (delta.cacheEffects.flushTlb || delta.cacheEffects.invalidateInstructionFetch) {
    context.wasm.full_clear_tlb();
  }
  if (delta.cacheEffects.invalidateGeneratedCode) {
    context.wasm.jit_clear_cache_js();
  }
}

function validateGenericCompletionFrame(frame, endpoint, generation) {
  assert.equal(frame.schema, "vine.v86-generic-runtime-completion");
  assert.equal(frame.schemaVersion, 1);
  assert.equal(frame.endpoint.kind, endpoint.kind);
  const actual = frame.endpoint.endpoint;
  assert.equal(actual.id, endpoint.id);
  assert.equal(actual.address, endpoint.address);
  assert.equal(actual.module.toLowerCase(), endpoint.module.toLowerCase());
  if (endpoint.kind === "runtime-system-export") {
    assert.equal(actual.symbol.kind, "name");
    assert.equal(actual.symbol.name, endpoint.symbol);
  } else {
    assert.equal(actual.name, endpoint.symbol);
  }
  assert.equal(frame.completion.frame.endpoint.id, endpoint.id);
  assert.equal(frame.completion.frame.endpoint.address, endpoint.address);
  assert.equal(frame.completion.frame.endpoint.symbol.kind, "name");
  assert.equal(frame.completion.frame.endpoint.symbol.name, endpoint.symbol);
  assert.equal(frame.memory.generation, generation);
  assert.equal(frame.memory.acknowledgedGeneration, generation);
  assert.equal(frame.memory.dirtyPages, 0);
  assert.equal(frame.memory.pendingInvalidation, undefined);
}

function validateGenericRuntimeFault(fault, endpoint, generation) {
  assert.equal(fault.schema, "vine.v86-generic-runtime-fault");
  assert.equal(fault.schemaVersion, 1);
  assert.equal(fault.endpoint.kind, endpoint.kind);
  assert.equal(fault.endpoint.endpoint.id, endpoint.id);
  assert.equal(fault.endpoint.endpoint.address, endpoint.address);
  assert.equal(
    fault.fault.class,
    "unsupported",
    `generic runtime fault: ${JSON.stringify(fault.fault)}`,
  );
  assert.equal(fault.memory.generation, generation);
  assert.equal(fault.memory.acknowledgedGeneration, generation);
  assert.equal(fault.memory.dirtyPages, 0);
  assert.equal(fault.memory.pendingInvalidation, undefined);
  assert.match(fault.memoryContentSha256, /^[0-9a-f]{64}$/u);
}

function restoreGenericApiCompletion(context, trapped, completion) {
  context.wasm.vine_cpu_state_save();
  const words = new Uint32Array(
    context.wasm.memory.buffer,
    context.wasm.vine_cpu_state_ptr(),
    context.wasm.vine_cpu_state_size() / 4,
  );
  assert.equal(words[stateWord.instructionPointer], trapped.instructionPointer);
  assert.equal(words[stateWord.previousInstructionPointer], trapped.previousInstructionPointer);
  assert.equal(words[stateWord.esp], trapped.esp);
  applyRegisterUpdate(words, stateWord.eax, completion.eax);
  applyRegisterUpdate(words, stateWord.edx, completion.edx);
  applyRegisterUpdate(words, stateWord.flags, completion.eflags);
  words[stateWord.esp] = completion.stackPointer;
  words[stateWord.instructionPointer] = completion.instructionPointer;
  words[stateWord.previousInstructionPointer] = completion.instructionPointer;
  assert.equal(context.wasm.vine_cpu_state_restore(), 0);
  const resumed = captureCpuState(context, 0);
  assert.equal(resumed.esp, completion.stackPointer);
  assert.equal(resumed.instructionPointer, completion.instructionPointer);
  assert.equal(resumed.previousInstructionPointer, completion.instructionPointer);
}

function applyRegisterUpdate(words, index, update) {
  if (update.action === "preserve") {
    return;
  }
  assert.equal(update.action, "set");
  words[index] = update.value >>> 0;
}

function validateGenericRuntimeReentryResult(result) {
  assert.equal(result.frozenSemanticCalls, genericRuntimeExpected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, genericRuntimeExpected.supportedCalls);
  assert.equal(result.writebacks.length, genericRuntimeExpected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    genericRuntimeExpected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    genericRuntimeExpected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, genericRuntimeExpected.hostDeltaBatches);
  assert.equal(result.finalGeneration, genericRuntimeExpected.finalGeneration);

  const laterPointerCalls = result.supportedCalls.slice(17);
  assert.deepEqual(
    laterPointerCalls.map(
      ({ symbol, occurrence, arguments: callArguments, result: callResult }) => ({
        symbol,
        occurrence,
        arguments: callArguments,
        result: callResult,
      }),
    ),
    [
      {
        symbol: "EncodePointer",
        occurrence: 3,
        arguments: [0x7f020000],
        result: { action: "set", value: 0xec010003 },
      },
      {
        symbol: "EncodePointer",
        occurrence: 4,
        arguments: [0x7f020010],
        result: { action: "set", value: 0xec010004 },
      },
      {
        symbol: "EncodePointer",
        occurrence: 5,
        arguments: [0x7f020020],
        result: { action: "set", value: 0xec010005 },
      },
      {
        symbol: "EncodePointer",
        occurrence: 6,
        arguments: [0x7f020030],
        result: { action: "set", value: 0xec010006 },
      },
    ],
  );

  const expected = genericRuntimeExpected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, expected.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, expected.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, expected.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, expected.module);
  assert.deepEqual(result.fault.endpoint.endpoint.symbol, {
    kind: "name",
    name: expected.symbol,
  });
  assert.equal(result.fault.endpoint.endpoint.iatAddress, expected.iatAddress);
  assert.equal(result.fault.fault.code, expected.faultCode);
  assert.equal(result.terminal.instructionCount, expected.instructionCount);
  assert.equal(result.terminal.terminal.returnAddress, expected.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, expected.arguments.length + 1),
    expected.arguments,
  );
}

function validateCriticalSectionSynchronizationResult(result) {
  const expected = criticalSectionSynchronizationExpected;
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);

  const initializations = result.supportedCalls.filter(
    ({ symbol }) => symbol === "InitializeCriticalSectionAndSpinCount",
  );
  assert.equal(initializations.length, expected.initializationCalls);
  const unlockedBytes = [
    0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff,
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
    0xa0, 0x0f, 0, 0,
  ];
  initializations.forEach((call, index) => {
    const address = expected.firstStorageAddress + index * expected.storageStride;
    assert.equal(call.occurrence, index + 1);
    assert.deepEqual(call.arguments, [address, expected.spinCount]);
    assert.deepEqual(call.result, { action: "set", value: 1 });
    assert.equal(
      call.implementationId,
      "kernel32.initialize-critical-section-and-spin-count.v1",
    );
    assert.equal(call.guestWrites.length, 1);
    assert.equal(call.guestWrites[0].address, address);
    assert.deepEqual(call.guestWrites[0].bytes, unlockedBytes);
  });

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.deepEqual(result.fault.endpoint.endpoint.symbol, {
    kind: "name",
    name: unsupported.symbol,
  });
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validatePointerDecodingResult(result) {
  const expected = pointerDecodingExpected;
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);

  const initializations = result.supportedCalls.filter(
    ({ symbol }) => symbol === "InitializeCriticalSectionAndSpinCount",
  );
  assert.equal(initializations.length, expected.initializationCalls);
  initializations.forEach((call, index) => {
    assert.deepEqual(call.arguments, [
      expected.firstStorageAddress + index * expected.storageStride,
      expected.spinCount,
    ]);
  });

  const decodes = result.supportedCalls.filter(({ symbol }) => symbol === "DecodePointer");
  assert.equal(decodes.length, expected.decodeCalls);
  assert.deepEqual(decodes[0].arguments, [expected.decodedToken]);
  assert.deepEqual(decodes[0].result, { action: "set", value: expected.decodedSource });
  assert.equal(decodes[0].implementationId, "kernel32.decode-pointer.v1");
  assert.equal(decodes[0].lastError, 87);
  assert.deepEqual(decodes[0].guestWrites, []);
  assert.equal(decodes[0].stackPointerAfter, decodes[0].stackPointerBefore + 8);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "dynamic-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.name, unsupported.name);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-dynamic-export-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateFiberLocalStorageAllocationResult(result) {
  const expected = fiberLocalStorageAllocationExpected;
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);

  assert.equal(
    result.supportedCalls.filter(
      ({ symbol }) => symbol === "InitializeCriticalSectionAndSpinCount",
    ).length,
    expected.initializationCalls,
  );
  assert.equal(
    result.supportedCalls.filter(({ symbol }) => symbol === "DecodePointer").length,
    expected.decodeCalls,
  );

  const allocations = result.supportedCalls.filter(({ symbol }) => symbol === "FlsAlloc");
  assert.equal(allocations.length, expected.allocationCalls);
  assert.deepEqual(allocations[0].arguments, [expected.callback]);
  assert.deepEqual(allocations[0].result, { action: "set", value: expected.logicalIndex });
  assert.equal(allocations[0].implementationId, "kernel32.fls-alloc.v1");
  assert.equal(allocations[0].lastError, 87);
  assert.deepEqual(allocations[0].guestWrites, []);
  assert.equal(allocations[0].stackPointerAfter, allocations[0].stackPointerBefore + 8);
  assert.equal(allocations[0].trapInstructionCount, expected.allocationInstructionCount);
  assert.equal(allocations[0].returnAddress, expected.allocationReturnAddress);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.deepEqual(result.fault.endpoint.endpoint.symbol, {
    kind: "name",
    name: unsupported.symbol,
  });
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validatePrivateHeapAllocationResult(result) {
  const expected = privateHeapAllocationExpected;
  const allocations = result.supportedCalls.filter(({ symbol }) => symbol === "HeapAlloc");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);

  assert.equal(
    result.supportedCalls.filter(({ symbol }) => symbol === "DecodePointer").length,
    expected.decodeCalls,
  );
  const lastDecode = result.supportedCalls.filter(
    ({ symbol }) => symbol === "DecodePointer",
  ).at(-1);
  assert.deepEqual(lastDecode.arguments, [0xec010005]);
  assert.deepEqual(lastDecode.result, { action: "set", value: 0x7f020020 });

  assert.equal(allocations.length, expected.heapAllocationCalls);
  assert.deepEqual(allocations[0].arguments, [
    expected.heapHandle,
    expected.flags,
    expected.byteLength,
  ]);
  assert.deepEqual(allocations[0].result, {
    action: "set",
    value: expected.guestAddress,
  });
  assert.equal(allocations[0].implementationId, "kernel32.heap-alloc.v1");
  assert.equal(allocations[0].lastError, 87);
  assert.deepEqual(allocations[0].guestWrites, []);
  assert.equal(allocations[0].stackPointerAfter, allocations[0].stackPointerBefore + 16);
  assert.equal(allocations[0].trapInstructionCount, expected.allocationInstructionCount);
  assert.equal(allocations[0].returnAddress, expected.allocationReturnAddress);

  const heapDelta = result.hostDeltas.find(({ mappingOperations }) => mappingOperations !== 0);
  assert(heapDelta, "HeapAlloc did not emit a host mapping delta");
  assert.equal(heapDelta.sequence, expected.heapDelta.sequence);
  assert.equal(heapDelta.fromGeneration, expected.heapDelta.fromGeneration);
  assert.equal(heapDelta.generation, expected.heapDelta.generation);
  assert.equal(heapDelta.mappingOperations, expected.heapDelta.mappingOperations);
  assert.deepEqual(heapDelta.dirtyPages, [
    { address: expected.heapDelta.address, owner: expected.heapDelta.owner },
  ]);
  assert.equal(heapDelta.payloadBytes, page.size);
  assert.equal(heapDelta.payloadSha256, expected.heapDelta.payloadSha256);
  assert.deepEqual(heapDelta.cacheEffects, {
    flushTlb: true,
    invalidateInstructionFetch: true,
    invalidateGeneratedCode: true,
  });

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "dynamic-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.name, unsupported.name);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-dynamic-export-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateFiberLocalStorageValueResult(result) {
  const expected = fiberLocalStorageValueExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "FlsSetValue");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);

  assert.equal(calls.length, expected.flsSetValueCalls);
  assert.deepEqual(calls[0].arguments, [expected.logicalIndex, expected.value]);
  assert.deepEqual(calls[0].result, { action: "set", value: 1 });
  assert.equal(calls[0].implementationId, "kernel32.fls-set-value.v1");
  assert.equal(calls[0].lastError, 87);
  assert.deepEqual(calls[0].guestWrites, []);
  assert.equal(calls[0].stackPointerAfter, calls[0].stackPointerBefore + 12);
  assert.equal(calls[0].trapInstructionCount, expected.callInstructionCount);
  assert.equal(calls[0].returnAddress, expected.callReturnAddress);
  assert.deepEqual(calls[0].flsValue, {
    threadId: expected.threadId,
    index: expected.logicalIndex,
    value: expected.value,
  });
  assert.deepEqual(
    result.supportedCalls.slice(-expected.continuationSymbols.length).map(({ symbol }) => symbol),
    expected.continuationSymbols,
  );

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateInterlockedIncrementResult(result) {
  const expected = interlockedIncrementExpected;
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "InterlockedIncrement",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, expected.callCount);
  for (const [index, call] of calls.entries()) {
    const occurrence = expected.calls[index];
    assert.deepEqual(call.arguments, [occurrence.targetAddress]);
    assert.deepEqual(call.result, { action: "set", value: occurrence.after });
    assert.equal(call.implementationId, "kernel32.interlocked-increment.v1");
    assert.equal(call.lastError, 87);
    assert.equal(call.stackPointerAfter, call.stackPointerBefore + 8);
    assert.equal(call.trapInstructionCount, occurrence.instructionCount);
    assert.equal(call.returnAddress, occurrence.returnAddress);
    assert.deepEqual(call.interlockedIncrement, {
      address: occurrence.targetAddress,
      before: occurrence.before,
      after: occurrence.after,
    });
    assert.equal(call.guestWrites.length, 1);
    assert.equal(call.guestWrites[0].address, occurrence.targetAddress);
    assert.deepEqual(call.guestWrites[0].bytes, [occurrence.after, 0, 0, 0]);
    assert.equal(call.guestWrites[0].result.bytesWritten, 4);
    assert.equal(call.guestWrites[0].result.dirtyPages, 1);
    assert.deepEqual(call.guestWrites[0].result.invalidation, {
      generation: occurrence.writeGeneration,
      dataTlb: false,
      instructionFetch: false,
      generatedCode: false,
    });
  }

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validatePrivateHeapSecondAllocationResult(result) {
  const expected = privateHeapSecondAllocationExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "HeapAlloc");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 2);
  assert.deepEqual(calls[0].result, {
    action: "set",
    value: expected.firstGuestAddress,
  });
  assert.deepEqual(calls[0].heapAllocation, {
    heapHandle: expected.heapHandle,
    allocation: {
      id: 1,
      address: expected.firstGuestAddress,
      byteLength: 0x214,
      reservedByteLength: 0x218,
      initialization: "zeroed",
    },
    liveAllocations: 1,
  });
  assert.deepEqual(calls[1].arguments, [
    expected.heapHandle,
    8,
    expected.secondByteLength,
  ]);
  assert.deepEqual(calls[1].result, {
    action: "set",
    value: expected.secondGuestAddress,
  });
  assert.equal(
    calls[1].implementationId,
    "kernel32.heap-alloc.v2",
  );
  assert.equal(calls[1].lastError, 87);
  assert.deepEqual(calls[1].guestWrites, []);
  assert.equal(calls[1].stackPointerAfter, calls[1].stackPointerBefore + 16);
  assert.equal(calls[1].trapInstructionCount, expected.secondInstructionCount);
  assert.equal(calls[1].returnAddress, expected.secondReturnAddress);
  assert.deepEqual(calls[1].heapAllocation, {
    heapHandle: expected.heapHandle,
    allocation: {
      id: 2,
      address: expected.secondGuestAddress,
      byteLength: expected.secondByteLength,
      reservedByteLength: expected.secondByteLength,
      initialization: "zeroed",
    },
    liveAllocations: 2,
  });

  const secondDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.secondDelta.sequence,
  );
  assert(secondDelta, "second HeapAlloc did not emit a host delta");
  assert.equal(secondDelta.fromGeneration, expected.secondDelta.fromGeneration);
  assert.equal(secondDelta.generation, expected.secondDelta.generation);
  assert.equal(secondDelta.mappingOperations, 0);
  assert.deepEqual(secondDelta.dirtyPages, [
    { address: expected.secondDelta.address, owner: expected.secondDelta.owner },
  ]);
  assert.equal(secondDelta.payloadBytes, page.size);
  assert.equal(secondDelta.payloadSha256, expected.secondDelta.payloadSha256);
  assert.deepEqual(secondDelta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateStandardInputHandleResult(result) {
  const expected = standardInputHandleExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "GetStdHandle");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].arguments, [expected.selector]);
  assert.deepEqual(calls[0].result, { action: "set", value: expected.handle });
  assert.equal(calls[0].implementationId, "kernel32.get-std-handle-input.v1");
  assert.equal(calls[0].lastError, 87);
  assert.deepEqual(calls[0].guestWrites, []);
  assert.equal(calls[0].stackPointerAfter, calls[0].stackPointerBefore + 8);
  assert.equal(calls[0].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[0].returnAddress, expected.returnAddress);
  assert.deepEqual(calls[0].standardHandle, {
    owner: "runtime-process-standard-handles",
    variantId: "standard-input-logical-v1",
    namespace: "vine.logical-standard-handle.v1",
    handle: expected.handle,
    selector: expected.selector,
  });

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, "kernel32-api-contract");
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateFileTypeResult(result) {
  const expected = fileTypeExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "GetFileType");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].arguments, [expected.handle]);
  assert.deepEqual(calls[0].result, { action: "set", value: expected.fileType });
  assert.equal(calls[0].implementationId, "kernel32.get-file-type-standard-input.v1");
  assert.equal(calls[0].lastError, 87);
  assert.deepEqual(calls[0].guestWrites, []);
  assert.equal(calls[0].stackPointerAfter, calls[0].stackPointerBefore + 8);
  assert.equal(calls[0].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[0].returnAddress, expected.returnAddress);
  assert.deepEqual(calls[0].fileType, {
    standardHandle: {
      owner: "runtime-process-standard-handles",
      variantId: "standard-input-logical-v1",
      namespace: "vine.logical-standard-handle.v1",
      handle: expected.handle,
      selector: 0xfffffff6,
    },
    fileType: expected.fileType,
  });

  const criticalSection = result.supportedCalls.at(-1);
  assert.equal(criticalSection.symbol, "InitializeCriticalSectionAndSpinCount");
  assert.equal(criticalSection.occurrence, expected.criticalSection.occurrence);
  assert.deepEqual(criticalSection.arguments, [
    expected.criticalSection.address,
    expected.criticalSection.spinCount,
  ]);
  assert.deepEqual(criticalSection.result, { action: "set", value: 1 });
  assert.equal(
    criticalSection.trapInstructionCount,
    expected.criticalSection.instructionCount,
  );
  assert.equal(criticalSection.returnAddress, expected.criticalSection.returnAddress);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, "kernel32-api-contract");
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateStandardOutputHandleResult(result) {
  const expected = standardOutputHandleExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "GetStdHandle");
  const fileTypeCalls = result.supportedCalls.filter(({ symbol }) => symbol === "GetFileType");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 2);
  assert.equal(fileTypeCalls.length, 1);
  assert.deepEqual(calls[0].arguments, [0xfffffff6]);
  assert.deepEqual(calls[0].result, { action: "set", value: expected.inputHandle });
  assert.deepEqual(calls[1].arguments, [expected.outputSelector]);
  assert.deepEqual(calls[1].result, { action: "set", value: expected.outputHandle });
  assert.equal(
    calls[1].implementationId,
    "kernel32.get-std-handle.v2",
  );
  assert.equal(calls[1].lastError, 87);
  assert.deepEqual(calls[1].guestWrites, []);
  assert.equal(calls[1].stackPointerAfter, calls[1].stackPointerBefore + 8);
  assert.equal(calls[1].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[1].returnAddress, expected.returnAddress);
  assert.deepEqual(calls[1].standardHandle, {
    owner: "runtime-process-standard-handles",
    variantId: "standard-output-logical-v1",
    namespace: "vine.logical-standard-handle.v1",
    handle: expected.outputHandle,
    selector: expected.outputSelector,
  });

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, "kernel32-api-contract");
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateStandardOutputFileTypeResult(result) {
  const expected = standardOutputFileTypeExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "GetFileType");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 2);
  assert.deepEqual(calls[1].arguments, [expected.outputHandle]);
  assert.deepEqual(calls[1].result, { action: "set", value: expected.outputFileType });
  assert.equal(calls[1].implementationId, "kernel32.get-file-type.v2");
  assert.equal(calls[1].lastError, 87);
  assert.deepEqual(calls[1].guestWrites, []);
  assert.equal(calls[1].stackPointerAfter, calls[1].stackPointerBefore + 8);
  assert.equal(calls[1].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[1].returnAddress, expected.returnAddress);
  assert.deepEqual(calls[1].fileType, {
    standardHandle: {
      owner: "runtime-process-standard-handles",
      variantId: "standard-output-logical-v1",
      namespace: "vine.logical-standard-handle.v1",
      handle: expected.outputHandle,
      selector: expected.outputSelector,
    },
    fileType: expected.outputFileType,
  });

  const criticalSection = result.supportedCalls.at(-1);
  assert.equal(criticalSection.symbol, "InitializeCriticalSectionAndSpinCount");
  assert.equal(criticalSection.occurrence, expected.criticalSection.occurrence);
  assert.deepEqual(criticalSection.arguments, [
    expected.criticalSection.address,
    expected.criticalSection.spinCount,
  ]);
  assert.deepEqual(criticalSection.result, { action: "set", value: 1 });
  assert.equal(
    criticalSection.trapInstructionCount,
    expected.criticalSection.instructionCount,
  );
  assert.equal(criticalSection.returnAddress, expected.criticalSection.returnAddress);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, "kernel32-api-contract");
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateStandardErrorHandleResult(result) {
  const expected = standardErrorHandleExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "GetStdHandle");
  const fileTypeCalls = result.supportedCalls.filter(({ symbol }) => symbol === "GetFileType");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 3);
  assert.equal(fileTypeCalls.length, 2);
  assert.deepEqual(calls[2].arguments, [expected.selector]);
  assert.deepEqual(calls[2].result, { action: "set", value: expected.handle });
  assert.equal(calls[2].implementationId, "kernel32.get-std-handle.v2");
  assert.equal(calls[2].lastError, 87);
  assert.deepEqual(calls[2].guestWrites, []);
  assert.equal(calls[2].stackPointerAfter, calls[2].stackPointerBefore + 8);
  assert.equal(calls[2].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[2].returnAddress, expected.returnAddress);
  assert.deepEqual(calls[2].standardHandle, {
    owner: "runtime-process-standard-handles",
    variantId: "standard-error-logical-v1",
    namespace: "vine.logical-standard-handle.v1",
    handle: expected.handle,
    selector: expected.selector,
  });

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, "kernel32-api-contract");
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateStandardErrorFileTypeResult(result) {
  const expected = standardErrorFileTypeExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "GetFileType");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 3);
  assert.deepEqual(calls[2].arguments, [expected.handle]);
  assert.deepEqual(calls[2].result, { action: "set", value: expected.fileType });
  assert.equal(calls[2].implementationId, "kernel32.get-file-type.v2");
  assert.equal(calls[2].lastError, 87);
  assert.deepEqual(calls[2].guestWrites, []);
  assert.equal(calls[2].stackPointerAfter, calls[2].stackPointerBefore + 8);
  assert.equal(calls[2].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[2].returnAddress, expected.returnAddress);
  assert.deepEqual(calls[2].fileType, {
    standardHandle: {
      owner: "runtime-process-standard-handles",
      variantId: "standard-error-logical-v1",
      namespace: "vine.logical-standard-handle.v1",
      handle: expected.handle,
      selector: expected.selector,
    },
    fileType: expected.fileType,
  });

  const criticalSection = result.supportedCalls.at(-1);
  assert.equal(criticalSection.symbol, "InitializeCriticalSectionAndSpinCount");
  assert.equal(criticalSection.occurrence, expected.criticalSection.occurrence);
  assert.deepEqual(criticalSection.arguments, [
    expected.criticalSection.address,
    expected.criticalSection.spinCount,
  ]);
  assert.deepEqual(criticalSection.result, { action: "set", value: 1 });
  assert.equal(
    criticalSection.trapInstructionCount,
    expected.criticalSection.instructionCount,
  );
  assert.equal(criticalSection.returnAddress, expected.criticalSection.returnAddress);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, "kernel32-api-contract");
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateSetHandleCountResult(result) {
  const expected = setHandleCountExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "SetHandleCount");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].arguments, [expected.count]);
  assert.deepEqual(calls[0].result, { action: "set", value: expected.count });
  assert.equal(calls[0].implementationId, "kernel32.set-handle-count.v1");
  assert.equal(calls[0].lastError, 87);
  assert.deepEqual(calls[0].guestWrites, []);
  assert.equal(calls[0].stackPointerAfter, calls[0].stackPointerBefore + 8);
  assert.equal(calls[0].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[0].returnAddress, expected.returnAddress);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, "kernel32-api-contract");
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateCommandLineAResult(result) {
  const expected = commandLineAExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "GetCommandLineA");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].arguments, []);
  assert.deepEqual(calls[0].result, { action: "set", value: expected.pointer });
  assert.equal(calls[0].implementationId, "kernel32.get-command-line-a.v1");
  assert.equal(calls[0].lastError, 87);
  assert.deepEqual(calls[0].guestWrites, []);
  assert.equal(calls[0].stackPointerAfter, calls[0].stackPointerBefore + 4);
  assert.equal(calls[0].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[0].returnAddress, expected.returnAddress);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, "kernel32-api-contract");
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateEnvironmentStringsWResult(result) {
  const expected = environmentStringsWExpected;
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "GetEnvironmentStringsW",
  );
  const frees = result.supportedCalls.filter(
    ({ symbol }) => symbol === "FreeEnvironmentStringsW",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 1);
  assert.equal(frees.length, 0);
  assert.deepEqual(calls[0].arguments, []);
  assert.deepEqual(calls[0].result, { action: "set", value: expected.pointer });
  assert.equal(
    calls[0].implementationId,
    "kernel32.get-environment-strings-w.v1",
  );
  assert.equal(calls[0].lastError, 87);
  assert.deepEqual(calls[0].guestWrites, []);
  assert.equal(calls[0].stackPointerAfter, calls[0].stackPointerBefore + 4);
  assert.equal(calls[0].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[0].returnAddress, expected.returnAddress);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, "kernel32-api-contract");
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateWideCharToMultiByteResult(result) {
  const expected = wideCharToMultiByteExpected;
  const conversions = result.supportedCalls.filter(
    ({ symbol }) => symbol === "WideCharToMultiByte",
  );
  const frees = result.supportedCalls.filter(
    ({ symbol }) => symbol === "FreeEnvironmentStringsW",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(conversions.length, 1);
  assert.equal(frees.length, 0);
  assert.deepEqual(conversions[0].arguments, [
    0,
    0,
    environmentStringsWExpected.pointer,
    1,
    0,
    0,
    0,
    0,
  ]);
  assert.deepEqual(conversions[0].result, { action: "set", value: 1 });
  assert.equal(
    conversions[0].implementationId,
    "kernel32.wide-char-to-multi-byte.v1",
  );
  assert.equal(conversions[0].lastError, 87);
  assert.deepEqual(conversions[0].guestWrites, []);
  assert.equal(conversions[0].trapInstructionCount, expected.instructionCount);
  assert.equal(conversions[0].returnAddress, expected.returnAddress);
  assert.equal(
    conversions[0].stackPointerAfter,
    conversions[0].stackPointerBefore + 36,
  );
  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validatePrivateHeapSize1Result(result) {
  const expected = privateHeapSize1Expected;
  const allocations = result.supportedCalls.filter(
    ({ symbol }) => symbol === "HeapAlloc",
  );
  const conversions = result.supportedCalls.filter(
    ({ symbol }) => symbol === "WideCharToMultiByte",
  );
  const frees = result.supportedCalls.filter(
    ({ symbol }) => symbol === "FreeEnvironmentStringsW",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);

  assert.equal(allocations.length, 3);
  assert.deepEqual(allocations[2].arguments, [expected.heapHandle, 0, 1]);
  assert.deepEqual(allocations[2].result, {
    action: "set",
    value: expected.guestAddress,
  });
  assert.equal(allocations[2].implementationId, "kernel32.heap-alloc.v2");
  assert.equal(allocations[2].lastError, 87);
  assert.deepEqual(allocations[2].guestWrites, []);
  assert.equal(
    allocations[2].stackPointerAfter,
    allocations[2].stackPointerBefore + 16,
  );
  assert.equal(
    allocations[2].trapInstructionCount,
    expected.allocationInstructionCount,
  );
  assert.equal(allocations[2].returnAddress, expected.allocationReturnAddress);
  assert.deepEqual(allocations[2].heapAllocation, {
    heapHandle: expected.heapHandle,
    allocation: {
      id: 3,
      address: expected.guestAddress,
      byteLength: 1,
      reservedByteLength: 8,
      initialization: "deterministic-uninitialized",
    },
    liveAllocations: 3,
  });

  assert.equal(conversions.length, 2);
  assert.deepEqual(conversions[1].arguments, [
    0,
    0,
    environmentStringsWExpected.pointer,
    1,
    expected.guestAddress,
    1,
    0,
    0,
  ]);
  assert.deepEqual(conversions[1].result, { action: "set", value: 1 });
  assert.equal(
    conversions[1].implementationId,
    "kernel32.wide-char-to-multi-byte.v1",
  );
  assert.equal(conversions[1].lastError, 87);
  assert.deepEqual(conversions[1].guestWrites, [
    {
      address: expected.guestAddress,
      bytes: [0],
      result: {
        bytesWritten: 1,
        dirtyPages: 1,
        invalidation: {
          generation: expected.conversionDelta.generation,
          dataTlb: false,
          instructionFetch: false,
          generatedCode: false,
        },
      },
    },
  ]);
  assert.equal(
    conversions[1].stackPointerAfter,
    conversions[1].stackPointerBefore + 36,
  );
  assert.equal(
    conversions[1].trapInstructionCount,
    expected.conversionInstructionCount,
  );
  assert.equal(conversions[1].returnAddress, expected.conversionReturnAddress);

  assert.equal(frees.length, 1);
  assert.deepEqual(frees[0].arguments, [environmentStringsWExpected.pointer]);
  assert.deepEqual(frees[0].result, { action: "set", value: 1 });
  assert.equal(
    frees[0].implementationId,
    "kernel32.free-environment-strings-w.v1",
  );
  assert.equal(frees[0].lastError, 87);
  assert.deepEqual(frees[0].guestWrites, []);
  assert.equal(frees[0].stackPointerAfter, frees[0].stackPointerBefore + 8);
  assert.equal(frees[0].trapInstructionCount, expected.freeInstructionCount);
  assert.equal(frees[0].returnAddress, expected.freeReturnAddress);

  for (const deltaExpected of [
    expected.allocationDelta,
    expected.conversionDelta,
  ]) {
    const delta = result.hostDeltas.find(
      ({ sequence }) => sequence === deltaExpected.sequence,
    );
    assert(delta, `host delta ${deltaExpected.sequence} is missing`);
    assert.equal(delta.fromGeneration, deltaExpected.fromGeneration);
    assert.equal(delta.generation, deltaExpected.generation);
    assert.equal(delta.mappingOperations, 0);
    assert.deepEqual(delta.dirtyPages, [
      { address: deltaExpected.address, owner: deltaExpected.owner },
    ]);
    assert.equal(delta.payloadBytes, page.size);
    assert.equal(delta.payloadSha256, deltaExpected.payloadSha256);
    assert.deepEqual(delta.cacheEffects, {
      flushTlb: false,
      invalidateInstructionFetch: false,
      invalidateGeneratedCode: false,
    });
  }

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateGetLastErrorResult(result) {
  const expected = getLastErrorExpected;
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "GetLastError",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].arguments, []);
  assert.deepEqual(calls[0].result, { action: "set", value: expected.result });
  assert.equal(calls[0].implementationId, "kernel32.get-last-error.v1");
  assert.equal(calls[0].lastError, expected.result);
  assert.deepEqual(calls[0].guestWrites, []);
  assert.equal(calls[0].stackPointerAfter, calls[0].stackPointerBefore + 4);
  assert.equal(calls[0].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[0].returnAddress, expected.returnAddress);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateTlsGetValueResult(result) {
  const expected = tlsGetValueExpected;
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "TlsGetValue",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].arguments, expected.firstUnsupported.arguments);
  assert.deepEqual(calls[0].result, { action: "set", value: expected.result });
  assert.equal(calls[0].implementationId, "kernel32.tls-get-value.v1");
  assert.equal(calls[0].lastError, 0);
  assert.deepEqual(calls[0].guestWrites, []);
  assert.equal(calls[0].stackPointerAfter, calls[0].stackPointerBefore + 8);
  assert.equal(calls[0].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[0].returnAddress, expected.returnAddress);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "dynamic-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.name, unsupported.symbol);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-dynamic-export-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateFlsGetValueResult(result) {
  const expected = flsGetValueExpected;
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "FlsGetValue",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].arguments, [0]);
  assert.deepEqual(calls[0].result, { action: "set", value: expected.result });
  assert.equal(calls[0].implementationId, "kernel32.fls-get-value.v1");
  assert.equal(calls[0].lastError, 0);
  assert.deepEqual(calls[0].guestWrites, []);
  assert.equal(calls[0].stackPointerAfter, calls[0].stackPointerBefore + 8);
  assert.equal(calls[0].trapInstructionCount, expected.instructionCount);
  assert.equal(calls[0].returnAddress, expected.returnAddress);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateSetLastErrorResult(result) {
  const expected = setLastErrorExpected;
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "SetLastError",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, expected.instructionCounts.length);
  for (const [index, call] of calls.entries()) {
    assert.equal(call.occurrence, index + 1);
    assert.deepEqual(call.arguments, [expected.argument]);
    assert.deepEqual(call.result, { action: "preserve" });
    assert.equal(call.implementationId, "kernel32.set-last-error.v1");
    assert.equal(call.lastError, expected.argument);
    assert.deepEqual(call.guestWrites, []);
    assert.equal(call.stackPointerAfter, call.stackPointerBefore + 8);
    assert.equal(call.trapInstructionCount, expected.instructionCounts[index]);
    assert.equal(call.returnAddress, expected.returnAddress);
  }

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateGetAcpResult(result) {
  const expected = getAcpExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "GetACP");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, expected.instructionCounts.length);
  for (const [index, call] of calls.entries()) {
    assert.equal(call.occurrence, index + 1);
    assert.deepEqual(call.arguments, []);
    assert.deepEqual(call.result, {
      action: "set",
      value: expected.activeCodePage,
    });
    assert.equal(call.implementationId, "kernel32.get-acp.v1");
    assert.equal(call.lastError, expected.lastError);
    assert.deepEqual(call.guestWrites, []);
    assert.equal(call.stackPointerAfter, call.stackPointerBefore + 4);
    assert.equal(call.trapInstructionCount, expected.instructionCounts[index]);
    assert.equal(call.returnAddress, expected.returnAddress);
  }

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validatePrivateHeapSize544Result(result) {
  const expected = privateHeapSize544Expected;
  const allocations = result.supportedCalls.filter(
    ({ symbol }) => symbol === "HeapAlloc",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(allocations.length, 4);
  const allocation = allocations[3];
  assert.deepEqual(allocation.arguments, [expected.heapHandle, 0, 0x220]);
  assert.deepEqual(allocation.result, {
    action: "set",
    value: expected.guestAddress,
  });
  assert.equal(allocation.implementationId, "kernel32.heap-alloc.v2");
  assert.equal(allocation.lastError, 87);
  assert.deepEqual(allocation.guestWrites, []);
  assert.equal(allocation.stackPointerAfter, allocation.stackPointerBefore + 16);
  assert.equal(
    allocation.trapInstructionCount,
    expected.allocationInstructionCount,
  );
  assert.equal(allocation.returnAddress, expected.allocationReturnAddress);
  assert.deepEqual(allocation.heapAllocation, {
    heapHandle: expected.heapHandle,
    allocation: {
      id: 4,
      address: expected.guestAddress,
      byteLength: 0x220,
      reservedByteLength: 0x220,
      initialization: "deterministic-uninitialized",
    },
    liveAllocations: 4,
  });

  for (const [symbol, instructionCount, returnAddress] of expected.followupCalls) {
    const call = result.supportedCalls.find(
      (candidate) =>
        candidate.symbol === symbol &&
        candidate.trapInstructionCount === instructionCount,
    );
    assert(call, `${symbol} follow-up call is missing`);
    assert.equal(call.returnAddress, returnAddress);
  }

  const allocationDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.allocationDelta.sequence,
  );
  assert(allocationDelta, "size-544 allocation delta is missing");
  assert.equal(
    allocationDelta.fromGeneration,
    expected.allocationDelta.fromGeneration,
  );
  assert.equal(allocationDelta.generation, expected.allocationDelta.generation);
  assert.equal(allocationDelta.mappingOperations, 0);
  assert.deepEqual(allocationDelta.dirtyPages, [
    {
      address: expected.allocationDelta.address,
      owner: expected.allocationDelta.owner,
    },
  ]);
  assert.equal(allocationDelta.payloadBytes, page.size);
  assert.equal(
    allocationDelta.payloadSha256,
    expected.allocationDelta.payloadSha256,
  );
  assert.deepEqual(allocationDelta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });

  const consumerWriteback = result.writebacks.find(
    ({ sequence }) => sequence === expected.consumerWriteback.sequence,
  );
  assert(consumerWriteback, "size-544 consumer writeback is missing");
  assert.equal(
    consumerWriteback.instructionCount,
    expected.consumerWriteback.instructionCount,
  );
  assert.deepEqual(
    consumerWriteback.dirtyPages,
    expected.consumerWriteback.dirtyPages,
  );
  assert.equal(
    consumerWriteback.payloadSha256,
    expected.consumerWriteback.payloadSha256,
  );
  assert.equal(
    consumerWriteback.receipt.fromGeneration,
    expected.consumerWriteback.fromGeneration,
  );
  assert.equal(
    consumerWriteback.receipt.generation,
    expected.consumerWriteback.generation,
  );
  assert.equal(
    consumerWriteback.receipt.changedBytes,
    expected.consumerWriteback.changedBytes,
  );

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateMultiByteToWideCharResult(result) {
  const expected = multiByteToWideCharExpected;
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "MultiByteToWideChar",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 2);

  const [query, conversion] = calls;
  assert.deepEqual(query.arguments, expected.query.arguments);
  assert.deepEqual(query.result, { action: "set", value: expected.query.result });
  assert.equal(query.implementationId, "kernel32.multi-byte-to-wide-char.v1");
  assert.equal(query.lastError, 87);
  assert.equal(query.stackPointerAfter, query.stackPointerBefore + 28);
  assert.equal(query.trapInstructionCount, expected.query.instructionCount);
  assert.equal(query.returnAddress, expected.query.returnAddress);
  assert.deepEqual(query.guestWrites, []);

  assert.deepEqual(conversion.arguments, expected.conversion.arguments);
  assert.deepEqual(conversion.result, {
    action: "set",
    value: expected.conversion.result,
  });
  assert.equal(conversion.implementationId, "kernel32.multi-byte-to-wide-char.v1");
  assert.equal(conversion.lastError, 87);
  assert.equal(conversion.stackPointerAfter, conversion.stackPointerBefore + 28);
  assert.equal(conversion.trapInstructionCount, expected.conversion.instructionCount);
  assert.equal(conversion.returnAddress, expected.conversion.returnAddress);
  assert.equal(conversion.guestWrites.length, 1);
  assert.equal(conversion.guestWrites[0].address, expected.conversion.arguments[4]);
  assert.deepEqual(conversion.guestWrites[0].bytes, expected.conversion.outputBytes);
  assert.deepEqual(conversion.guestWrites[0].result, {
    bytesWritten: expected.conversion.outputBytes.length,
    dirtyPages: 1,
    invalidation: {
      generation: expected.outputDelta.generation,
      dataTlb: false,
      instructionFetch: false,
      generatedCode: false,
    },
  });

  const outputDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.outputDelta.sequence,
  );
  assert(outputDelta, "MultiByteToWideChar output delta is missing");
  assert.equal(outputDelta.fromGeneration, expected.outputDelta.fromGeneration);
  assert.equal(outputDelta.generation, expected.outputDelta.generation);
  assert.equal(outputDelta.mappingOperations, 0);
  assert.deepEqual(outputDelta.dirtyPages, [
    {
      address: expected.outputDelta.pageAddress,
      owner: expected.outputDelta.owner,
    },
  ]);
  assert.equal(outputDelta.payloadBytes, page.size);
  assert.equal(outputDelta.payloadSha256, expected.outputDelta.payloadSha256);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateGetStringTypeWResult(result) {
  const expected = getStringTypeWExpected;
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "GetStringTypeW",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 1);
  const [call] = calls;
  assert.deepEqual(call.arguments, expected.call.arguments);
  assert.deepEqual(call.result, { action: "set", value: expected.call.result });
  assert.equal(call.implementationId, "kernel32.get-string-type-w.v1");
  assert.equal(call.lastError, 87);
  assert.equal(call.stackPointerAfter, call.stackPointerBefore + 20);
  assert.equal(call.trapInstructionCount, expected.call.instructionCount);
  assert.equal(call.returnAddress, expected.call.returnAddress);
  assert.equal(call.guestWrites.length, 1);
  assert.equal(call.guestWrites[0].address, expected.call.arguments[3]);
  const mismatch = call.guestWrites[0].bytes.findIndex(
    (byte, index) => byte !== expected.call.outputBytes[index],
  );
  assert.equal(
    mismatch,
    -1,
    `GetStringTypeW output mismatch at byte ${mismatch}: actual=${call.guestWrites[0].bytes[mismatch]}, expected=${expected.call.outputBytes[mismatch]}`,
  );
  assert.deepEqual(call.guestWrites[0].bytes, expected.call.outputBytes);
  assert.deepEqual(call.guestWrites[0].result, {
    bytesWritten: expected.call.outputBytes.length,
    dirtyPages: 1,
    invalidation: {
      generation: expected.outputDelta.generation,
      dataTlb: false,
      instructionFetch: false,
      generatedCode: false,
    },
  });

  const outputDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.outputDelta.sequence,
  );
  assert(outputDelta, "GetStringTypeW output delta is missing");
  assert.equal(outputDelta.fromGeneration, expected.outputDelta.fromGeneration);
  assert.equal(outputDelta.generation, expected.outputDelta.generation);
  assert.equal(outputDelta.mappingOperations, 0);
  assert.deepEqual(outputDelta.dirtyPages, [
    {
      address: expected.outputDelta.pageAddress,
      owner: expected.outputDelta.owner,
    },
  ]);
  assert.equal(outputDelta.payloadBytes, page.size);
  assert.equal(outputDelta.payloadSha256, expected.outputDelta.payloadSha256);

  const consumerWriteback = result.writebacks.find(
    ({ sequence }) => sequence === expected.consumerWriteback.sequence,
  );
  assert(consumerWriteback, "GetStringTypeW consumer writeback is missing");
  assert.equal(
    consumerWriteback.instructionCount,
    expected.consumerWriteback.instructionCount,
  );
  assert.deepEqual(
    consumerWriteback.dirtyPages,
    expected.consumerWriteback.dirtyPages,
  );
  assert.equal(
    consumerWriteback.payloadSha256,
    expected.consumerWriteback.payloadSha256,
  );
  assert.equal(
    consumerWriteback.receipt.fromGeneration,
    expected.consumerWriteback.fromGeneration,
  );
  assert.equal(
    consumerWriteback.receipt.generation,
    expected.consumerWriteback.generation,
  );
  assert.equal(
    consumerWriteback.receipt.changedBytes,
    expected.consumerWriteback.changedBytes,
  );

  for (const [symbol, instructionCount, returnAddress] of expected.followupCalls) {
    const followup = result.supportedCalls.find(
      (candidate) =>
        candidate.symbol === symbol &&
        candidate.trapInstructionCount === instructionCount,
    );
    assert(followup, `${symbol} follow-up call at ${instructionCount} is missing`);
    assert.equal(followup.returnAddress, returnAddress);
  }

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateLCMapStringWResult(result) {
  const expected = lcMapStringWExpected;
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "LCMapStringW",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, lcMapStringExpectedCalls.length);
  for (const [index, expectedCall] of lcMapStringExpectedCalls.entries()) {
    const call = calls[index];
    assert.deepEqual(call.arguments, expectedCall.arguments);
    assert.deepEqual(call.result, { action: "set", value: 256 });
    assert.equal(call.implementationId, "kernel32.lc-map-string-w.v1");
    assert.equal(call.lastError, 87);
    assert.equal(call.stackPointerAfter, call.stackPointerBefore + 28);
    assert.equal(call.trapInstructionCount, expectedCall.instructionCount);
    assert.equal(call.returnAddress, expectedCall.returnAddress);
    if (expectedCall.outputBytes === null) {
      assert.deepEqual(call.guestWrites, []);
      continue;
    }
    assert.equal(call.guestWrites.length, 1);
    assert.equal(call.guestWrites[0].address, expectedCall.arguments[4]);
    assert.deepEqual(call.guestWrites[0].bytes, expectedCall.outputBytes);
    const outputDelta = expected.outputDeltas[index === 1 ? 0 : 1];
    assert.deepEqual(call.guestWrites[0].result, {
      bytesWritten: expectedCall.outputBytes.length,
      dirtyPages: 1,
      invalidation: {
        generation: outputDelta.generation,
        dataTlb: false,
        instructionFetch: false,
        generatedCode: false,
      },
    });
  }

  for (const outputDelta of expected.outputDeltas) {
    const actual = result.hostDeltas.find(
      ({ sequence }) => sequence === outputDelta.sequence,
    );
    assert(actual, `LCMapStringW output delta ${outputDelta.sequence} is missing`);
    assert.equal(actual.fromGeneration, outputDelta.fromGeneration);
    assert.equal(actual.generation, outputDelta.generation);
    assert.equal(actual.mappingOperations, 0);
    assert.deepEqual(actual.dirtyPages, [
      { address: outputDelta.pageAddress, owner: outputDelta.owner },
    ]);
    assert.equal(actual.payloadBytes, page.size);
    assert.equal(actual.payloadSha256, outputDelta.payloadSha256);
  }

  for (const [symbol, instructionCount, returnAddress] of expected.followupCalls) {
    const followup = result.supportedCalls.find(
      (candidate) =>
        candidate.symbol === symbol &&
        candidate.trapInstructionCount === instructionCount,
    );
    assert(followup, `${symbol} follow-up call at ${instructionCount} is missing`);
    assert.equal(followup.returnAddress, returnAddress);
  }

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validatePrivateHeapSize4Result(result) {
  const expected = privateHeapSize4Expected;
  const allocations = result.supportedCalls.filter(
    ({ symbol }) => symbol === "HeapAlloc",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(allocations.length, 6);
  const allocation = allocations[5];
  assert.deepEqual(allocation.arguments, [expected.heapHandle, 8, 4]);
  assert.deepEqual(allocation.result, {
    action: "set",
    value: expected.guestAddress,
  });
  assert.equal(allocation.implementationId, "kernel32.heap-alloc.v2");
  assert.equal(allocation.lastError, 0);
  assert.deepEqual(allocation.guestWrites, []);
  assert.equal(allocation.stackPointerAfter, allocation.stackPointerBefore + 16);
  assert.equal(
    allocation.trapInstructionCount,
    expected.allocationInstructionCount,
  );
  assert.equal(allocation.returnAddress, expected.allocationReturnAddress);
  assert.deepEqual(allocation.heapAllocation, {
    heapHandle: expected.heapHandle,
    allocation: {
      id: 6,
      address: expected.guestAddress,
      byteLength: 4,
      reservedByteLength: 8,
      initialization: "zeroed",
    },
    liveAllocations: 6,
  });

  const allocationDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.allocationDelta.sequence,
  );
  assert(allocationDelta, "size-4 allocation delta is missing");
  assert.equal(
    allocationDelta.fromGeneration,
    expected.allocationDelta.fromGeneration,
  );
  assert.equal(allocationDelta.generation, expected.allocationDelta.generation);
  assert.equal(allocationDelta.mappingOperations, 0);
  assert.deepEqual(allocationDelta.dirtyPages, [
    {
      address: expected.allocationDelta.address,
      owner: expected.allocationDelta.owner,
    },
  ]);
  assert.equal(allocationDelta.payloadBytes, page.size);
  assert.equal(
    allocationDelta.payloadSha256,
    expected.allocationDelta.payloadSha256,
  );
  assert.deepEqual(allocationDelta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });

  const consumerWriteback = result.writebacks.find(
    ({ sequence }) => sequence === expected.consumerWriteback.sequence,
  );
  assert(consumerWriteback, "size-4 consumer writeback is missing");
  assert.equal(
    consumerWriteback.instructionCount,
    expected.consumerWriteback.instructionCount,
  );
  assert.deepEqual(
    consumerWriteback.dirtyPages,
    expected.consumerWriteback.dirtyPages,
  );
  assert.equal(
    consumerWriteback.payloadSha256,
    expected.consumerWriteback.payloadSha256,
  );
  assert.equal(
    consumerWriteback.receipt.fromGeneration,
    expected.consumerWriteback.fromGeneration,
  );
  assert.equal(
    consumerWriteback.receipt.generation,
    expected.consumerWriteback.generation,
  );
  assert.equal(
    consumerWriteback.receipt.changedBytes,
    expected.consumerWriteback.changedBytes,
  );

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validatePrivateHeapFreeSize1Result(result) {
  const expected = privateHeapFreeSize1Expected;
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);

  const allocations = result.supportedCalls.filter(
    ({ symbol }) => symbol === "HeapAlloc",
  );
  assert.equal(allocations.length, 6);
  const sourceAllocation = allocations.find(
    ({ heapAllocation }) => heapAllocation?.allocation.id === expected.allocationId,
  );
  assert(sourceAllocation, "freed source allocation is missing");
  assert.deepEqual(sourceAllocation.arguments, [
    expected.heapHandle,
    0,
    expected.allocationRequestedBytes,
  ]);
  assert.deepEqual(sourceAllocation.heapAllocation, {
    heapHandle: expected.heapHandle,
    allocation: {
      id: expected.allocationId,
      address: expected.allocationAddress,
      byteLength: expected.allocationRequestedBytes,
      reservedByteLength: expected.allocationReservedBytes,
      initialization: "deterministic-uninitialized",
    },
    liveAllocations: 3,
  });

  const frees = result.supportedCalls.filter(({ symbol }) => symbol === "HeapFree");
  assert.equal(frees.length, 1);
  const free = frees[0];
  assert.equal(free.sequence, expected.freeDelta.sequence);
  assert.equal(free.occurrence, 1);
  assert.deepEqual(free.arguments, [
    expected.heapHandle,
    0,
    expected.allocationAddress,
  ]);
  assert.deepEqual(free.result, { action: "set", value: 1 });
  assert.equal(free.implementationId, "kernel32.heap-free.v1");
  assert.equal(free.lastError, 0);
  assert.deepEqual(free.guestWrites, []);
  assert.equal(free.stackPointerAfter, free.stackPointerBefore + 16);
  assert.equal(free.trapInstructionCount, expected.freeInstructionCount);
  assert.equal(free.returnAddress, expected.freeReturnAddress);

  const freeDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.freeDelta.sequence,
  );
  assert(freeDelta, "HeapFree host delta is missing");
  assert.equal(freeDelta.fromGeneration, expected.freeDelta.fromGeneration);
  assert.equal(freeDelta.generation, expected.freeDelta.generation);
  assert.equal(freeDelta.mappingOperations, 0);
  assert.deepEqual(freeDelta.dirtyPages, [
    { address: expected.freeDelta.address, owner: expected.freeDelta.owner },
  ]);
  assert.equal(freeDelta.payloadBytes, page.size);
  assert.equal(freeDelta.payloadSha256, expected.freeDelta.payloadSha256);
  assert.deepEqual(freeDelta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });

  const postFreeCalls = result.supportedCalls.filter(
    ({ sequence }) => sequence > free.sequence,
  );
  assert.equal(postFreeCalls.length, expected.postFreeEncodePointerCalls);
  assert(
    postFreeCalls.every(
      (call, index) =>
        call.symbol === "EncodePointer" &&
        call.sequence === free.sequence + index + 1 &&
        call.implementationId === "kernel32.encode-pointer.v1" &&
        call.guestWrites.length === 0 &&
        call.lastError === 0,
    ),
    "post-free continuation is not the expected EncodePointer run",
  );

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validatePrivateHeapSize128Result(
  result,
  expected = privateHeapSize128Expected,
  { allocationCount = 7, postAllocationCallCount = 1 } = {},
) {
  const allocations = result.supportedCalls.filter(
    ({ symbol }) => symbol === "HeapAlloc",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(allocations.length, allocationCount);
  const allocation = allocations[6];
  assert.deepEqual(allocation.arguments, [expected.heapHandle, 8, 128]);
  assert.deepEqual(allocation.result, {
    action: "set",
    value: expected.guestAddress,
  });
  assert.equal(allocation.implementationId, "kernel32.heap-alloc.v2");
  assert.equal(allocation.lastError, 0);
  assert.deepEqual(allocation.guestWrites, []);
  assert.equal(allocation.stackPointerAfter, allocation.stackPointerBefore + 16);
  assert.equal(
    allocation.trapInstructionCount,
    expected.allocationInstructionCount,
  );
  assert.equal(allocation.returnAddress, expected.allocationReturnAddress);
  assert.deepEqual(allocation.heapAllocation, {
    heapHandle: expected.heapHandle,
    allocation: {
      id: 7,
      address: expected.guestAddress,
      byteLength: 128,
      reservedByteLength: 128,
      initialization: "zeroed",
    },
    liveAllocations: 6,
  });

  const allocationDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.allocationDelta.sequence,
  );
  assert(allocationDelta, "size-128 allocation delta is missing");
  assert.equal(
    allocationDelta.fromGeneration,
    expected.allocationDelta.fromGeneration,
  );
  assert.equal(allocationDelta.generation, expected.allocationDelta.generation);
  assert.equal(allocationDelta.mappingOperations, 0);
  assert.deepEqual(allocationDelta.dirtyPages, [
    {
      address: expected.allocationDelta.address,
      owner: expected.allocationDelta.owner,
    },
  ]);
  assert.equal(allocationDelta.payloadBytes, page.size);
  assert.equal(
    allocationDelta.payloadSha256,
    expected.allocationDelta.payloadSha256,
  );
  assert.deepEqual(allocationDelta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });

  const postAllocationCalls = result.supportedCalls.filter(
    ({ sequence }) => sequence > allocation.sequence,
  );
  assert.equal(postAllocationCalls.length, postAllocationCallCount);
  const encodePointer = postAllocationCalls[0];
  assert.equal(encodePointer.sequence, expected.encodePointerCall.sequence);
  assert.equal(encodePointer.symbol, "EncodePointer");
  assert.equal(encodePointer.occurrence, expected.encodePointerCall.occurrence);
  assert.deepEqual(encodePointer.arguments, [expected.guestAddress]);
  assert.deepEqual(encodePointer.result, {
    action: "set",
    value: expected.encodePointerCall.encodedPointer,
  });
  assert.equal(encodePointer.implementationId, "kernel32.encode-pointer.v1");
  assert.equal(encodePointer.lastError, 0);
  assert.deepEqual(encodePointer.guestWrites, []);
  assert.equal(encodePointer.stackPointerAfter, encodePointer.stackPointerBefore + 8);
  assert.equal(
    encodePointer.trapInstructionCount,
    expected.encodePointerCall.instructionCount,
  );
  assert.equal(encodePointer.returnAddress, expected.encodePointerCall.returnAddress);

  const consumerWriteback = result.writebacks.find(
    ({ sequence }) => sequence === expected.consumerWriteback.sequence,
  );
  assert(consumerWriteback, "size-128 consumer writeback is missing");
  assert.equal(
    consumerWriteback.instructionCount,
    expected.consumerWriteback.instructionCount,
  );
  assert.deepEqual(
    consumerWriteback.dirtyPages,
    expected.consumerWriteback.dirtyPages,
  );
  assert.equal(
    consumerWriteback.payloadSha256,
    expected.consumerWriteback.payloadSha256,
  );
  assert.equal(
    consumerWriteback.receipt.fromGeneration,
    expected.consumerWriteback.fromGeneration,
  );
  assert.equal(
    consumerWriteback.receipt.generation,
    expected.consumerWriteback.generation,
  );
  assert.equal(
    consumerWriteback.receipt.changedBytes,
    expected.consumerWriteback.changedBytes,
  );

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateIsProcessorFeaturePresentResult(
  result,
  expected = isProcessorFeaturePresentExpected,
  { postAllocationCallCount = 4 } = {},
) {
  validatePrivateHeapSize128Result(result, expected, {
    allocationCount: 8,
    postAllocationCallCount,
  });
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "IsProcessorFeaturePresent",
  );
  assert.equal(calls.length, expected.processorFeatureCalls.length);
  for (const [index, call] of calls.entries()) {
    const expectedCall = expected.processorFeatureCalls[index];
    assert.equal(call.sequence, expectedCall.sequence);
    assert.equal(call.occurrence, expectedCall.occurrence);
    assert.deepEqual(call.arguments, [10]);
    assert.deepEqual(call.result, { action: "set", value: 1 });
    assert.equal(
      call.implementationId,
      "kernel32.is-processor-feature-present.v1",
    );
    assert.equal(call.lastError, 0);
    assert.deepEqual(call.guestWrites, []);
    assert.equal(call.stackPointerAfter, call.stackPointerBefore + 8);
    assert.equal(call.trapInstructionCount, expectedCall.instructionCount);
    assert.equal(call.returnAddress, expectedCall.returnAddress);

    const writeback = result.writebacks.find(
      ({ sequence }) => sequence === expectedCall.consumerWriteback.sequence,
    );
    assert(writeback, `processor-feature consumer ${index + 1} writeback is missing`);
    assert.equal(
      writeback.instructionCount,
      expectedCall.consumerWriteback.instructionCount,
    );
    assert.deepEqual(writeback.dirtyPages, expectedCall.consumerWriteback.dirtyPages);
    assert.equal(writeback.payloadSha256, expectedCall.consumerWriteback.payloadSha256);
    assert.equal(
      writeback.receipt.fromGeneration,
      expectedCall.consumerWriteback.fromGeneration,
    );
    assert.equal(
      writeback.receipt.generation,
      expectedCall.consumerWriteback.generation,
    );
    assert.equal(
      writeback.receipt.changedBytes,
      expectedCall.consumerWriteback.changedBytes,
    );
  }

  const laterAllocation = result.supportedCalls.find(
    ({ sequence }) => sequence === expected.laterAllocation.sequence,
  );
  assert(laterAllocation, "post-feature HeapAlloc is missing");
  assert.equal(laterAllocation.symbol, "HeapAlloc");
  assert.equal(laterAllocation.occurrence, expected.laterAllocation.occurrence);
  assert.deepEqual(laterAllocation.arguments, [expected.heapHandle, 8, 2048]);
  assert.deepEqual(laterAllocation.result, {
    action: "set",
    value: expected.laterAllocation.guestAddress,
  });
  assert.equal(laterAllocation.implementationId, "kernel32.heap-alloc.v2");
  assert.equal(laterAllocation.lastError, 0);
  assert.deepEqual(laterAllocation.guestWrites, []);
  assert.equal(
    laterAllocation.stackPointerAfter,
    laterAllocation.stackPointerBefore + 16,
  );
  assert.equal(
    laterAllocation.trapInstructionCount,
    expected.laterAllocation.instructionCount,
  );
  assert.equal(laterAllocation.returnAddress, expected.laterAllocation.returnAddress);
  assert.deepEqual(laterAllocation.heapAllocation, {
    heapHandle: expected.heapHandle,
    allocation: {
      id: 8,
      address: expected.laterAllocation.guestAddress,
      byteLength: 2048,
      reservedByteLength: 2048,
      initialization: "zeroed",
    },
    liveAllocations: 7,
  });

  const allocationDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.laterAllocation.delta.sequence,
  );
  assert(allocationDelta, "post-feature HeapAlloc delta is missing");
  assert.equal(
    allocationDelta.fromGeneration,
    expected.laterAllocation.delta.fromGeneration,
  );
  assert.equal(allocationDelta.generation, expected.laterAllocation.delta.generation);
  assert.equal(
    allocationDelta.mappingOperations,
    expected.laterAllocation.delta.mappingOperations,
  );
  assert.deepEqual(allocationDelta.dirtyPages, expected.laterAllocation.delta.dirtyPages);
  assert.equal(allocationDelta.payloadBytes, expected.laterAllocation.delta.payloadBytes);
  assert.equal(
    allocationDelta.payloadSha256,
    expected.laterAllocation.delta.payloadSha256,
  );
  assert.deepEqual(allocationDelta.cacheEffects, {
    flushTlb: true,
    invalidateInstructionFetch: true,
    invalidateGeneratedCode: true,
  });
}

function validateSetUnhandledExceptionFilterResult(
  result,
  expected = setUnhandledExceptionFilterExpected,
  { postAllocationCallCount = 8 } = {},
) {
  validateIsProcessorFeaturePresentResult(result, expected, {
    postAllocationCallCount,
  });
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "SetUnhandledExceptionFilter",
  );
  assert.equal(calls.length, 1);
  const call = calls[0];
  assert.equal(call.sequence, expected.filterCall.sequence);
  assert.equal(call.occurrence, expected.filterCall.occurrence);
  assert.deepEqual(call.arguments, [expected.filterCall.filterAddress]);
  assert.deepEqual(call.result, { action: "set", value: 0 });
  assert.equal(call.implementationId, "kernel32.set-unhandled-exception-filter.v1");
  assert.equal(call.lastError, 0);
  assert.deepEqual(call.guestWrites, []);
  assert.equal(call.stackPointerAfter, call.stackPointerBefore + 8);
  assert.equal(call.trapInstructionCount, expected.filterCall.instructionCount);
  assert.equal(call.returnAddress, expected.filterCall.returnAddress);

  const consumerWriteback = result.writebacks.find(
    ({ sequence }) => sequence === expected.filterCall.consumerWriteback.sequence,
  );
  assert(consumerWriteback, "unhandled-exception filter consumer writeback is missing");
  assert.equal(
    consumerWriteback.instructionCount,
    expected.filterCall.consumerWriteback.instructionCount,
  );
  assert.deepEqual(
    consumerWriteback.dirtyPages,
    expected.filterCall.consumerWriteback.dirtyPages,
  );
  assert.equal(
    consumerWriteback.payloadSha256,
    expected.filterCall.consumerWriteback.payloadSha256,
  );
  assert.equal(
    consumerWriteback.receipt.fromGeneration,
    expected.filterCall.consumerWriteback.fromGeneration,
  );
  assert.equal(
    consumerWriteback.receipt.generation,
    expected.filterCall.consumerWriteback.generation,
  );
  assert.equal(
    consumerWriteback.receipt.changedBytes,
    expected.filterCall.consumerWriteback.changedBytes,
  );

  for (const expectedCall of expected.continuationCalls) {
    const continuation = result.supportedCalls.find(
      ({ sequence }) => sequence === expectedCall.sequence,
    );
    assert(continuation, `continuation call ${expectedCall.sequence} is missing`);
    assert.equal(continuation.symbol, expectedCall.symbol);
    assert.equal(continuation.occurrence, expectedCall.occurrence);
    assert.equal(continuation.trapInstructionCount, expectedCall.instructionCount);
    assert.deepEqual(continuation.arguments, expectedCall.arguments);
    assert.deepEqual(continuation.result, expectedCall.result);
    assert.equal(continuation.implementationId, expectedCall.implementationId);
    assert.equal(continuation.returnAddress, expectedCall.returnAddress);
    assert.equal(
      continuation.stackPointerAfter,
      continuation.stackPointerBefore + expectedCall.stackCleanupBytes,
    );
    assert.equal(continuation.lastError, 0);
  }

  const criticalSectionDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.criticalSectionDelta.sequence,
  );
  assert(criticalSectionDelta, "post-filter critical-section delta is missing");
  assert.equal(
    criticalSectionDelta.fromGeneration,
    expected.criticalSectionDelta.fromGeneration,
  );
  assert.equal(
    criticalSectionDelta.generation,
    expected.criticalSectionDelta.generation,
  );
  assert.equal(criticalSectionDelta.mappingOperations, 0);
  assert.deepEqual(criticalSectionDelta.dirtyPages, [
    {
      address: expected.criticalSectionDelta.address,
      owner: expected.criticalSectionDelta.owner,
    },
  ]);
  assert.equal(criticalSectionDelta.payloadBytes, page.size);
  assert.equal(
    criticalSectionDelta.payloadSha256,
    expected.criticalSectionDelta.payloadSha256,
  );
  assert.deepEqual(criticalSectionDelta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });
}

function validateHeapSizeResult(result) {
  const expected = heapSizeExpected;
  validateSetUnhandledExceptionFilterResult(result, expected, {
    postAllocationCallCount: 12,
  });
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "HeapSize");
  assert.equal(calls.length, 1);
  const call = calls[0];
  assert.equal(call.sequence, expected.heapSizeCall.sequence);
  assert.equal(call.occurrence, expected.heapSizeCall.occurrence);
  assert.equal(call.trapInstructionCount, expected.heapSizeCall.instructionCount);
  assert.equal(call.returnAddress, expected.heapSizeCall.returnAddress);
  assert.deepEqual(call.arguments, expected.heapSizeCall.arguments);
  assert.deepEqual(call.result, {
    action: "set",
    value: expected.heapSizeCall.result,
  });
  assert.equal(call.implementationId, "kernel32.heap-size.v1");
  assert.equal(call.lastError, 0);
  assert.deepEqual(call.guestWrites, []);
  assert.equal(
    call.stackPointerAfter,
    call.stackPointerBefore + expected.heapSizeCall.stackCleanupBytes,
  );

  for (const expectedCall of expected.postHeapSizeCalls) {
    const continuation = result.supportedCalls.find(
      ({ sequence }) => sequence === expectedCall.sequence,
    );
    assert(continuation, `post-HeapSize call ${expectedCall.sequence} is missing`);
    assert.equal(continuation.symbol, expectedCall.symbol);
    assert.equal(continuation.occurrence, expectedCall.occurrence);
    assert.equal(continuation.trapInstructionCount, expectedCall.instructionCount);
    assert.deepEqual(continuation.arguments, expectedCall.arguments);
    assert.deepEqual(continuation.result, expectedCall.result);
    assert.equal(continuation.implementationId, expectedCall.implementationId);
    assert.equal(continuation.returnAddress, expectedCall.returnAddress);
    assert.equal(
      continuation.stackPointerAfter,
      continuation.stackPointerBefore + expectedCall.stackCleanupBytes,
    );
    assert.equal(continuation.lastError, 0);
  }

  const criticalSectionDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.leaveCriticalSectionDelta.sequence,
  );
  assert(criticalSectionDelta, "post-HeapSize critical-section delta is missing");
  assert.equal(
    criticalSectionDelta.fromGeneration,
    expected.leaveCriticalSectionDelta.fromGeneration,
  );
  assert.equal(
    criticalSectionDelta.generation,
    expected.leaveCriticalSectionDelta.generation,
  );
  assert.equal(criticalSectionDelta.mappingOperations, 0);
  assert.deepEqual(criticalSectionDelta.dirtyPages, [
    {
      address: expected.leaveCriticalSectionDelta.address,
      owner: expected.leaveCriticalSectionDelta.owner,
    },
  ]);
  assert.equal(
    criticalSectionDelta.payloadSha256,
    expected.leaveCriticalSectionDelta.payloadSha256,
  );
}

function validateInitializeCriticalSectionResult(result) {
  const expected = initializeCriticalSectionExpected;
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);

  const initializations = result.supportedCalls.filter(
    ({ symbol }) => symbol === "InitializeCriticalSection",
  );
  assert.equal(initializations.length, 1);
  const call = initializations[0];
  assert.equal(call.sequence, expected.initializationCall.sequence);
  assert.equal(call.occurrence, expected.initializationCall.occurrence);
  assert.equal(call.trapInstructionCount, expected.initializationCall.instructionCount);
  assert.deepEqual(call.arguments, expected.initializationCall.arguments);
  assert.deepEqual(call.result, { action: "preserve" });
  assert.equal(
    call.implementationId,
    "kernel32.initialize-critical-section.v1",
  );
  assert.equal(call.returnAddress, expected.initializationCall.returnAddress);
  assert.equal(
    call.stackPointerAfter,
    call.stackPointerBefore + expected.initializationCall.stackCleanupBytes,
  );
  assert.equal(call.lastError, 0);
  assert.equal(call.guestWrites.length, 1);
  assert.equal(call.guestWrites[0].address, expected.initializationCall.arguments[0]);
  assert.deepEqual(call.guestWrites[0].bytes, expected.initializationCall.bytes);

  const initializationDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.initializationCall.sequence,
  );
  assert(initializationDelta, "InitializeCriticalSection host delta is missing");
  assert.equal(
    initializationDelta.fromGeneration,
    expected.initializationCall.delta.fromGeneration,
  );
  assert.equal(
    initializationDelta.generation,
    expected.initializationCall.delta.generation,
  );
  assert.equal(initializationDelta.mappingOperations, 0);
  assert.deepEqual(initializationDelta.dirtyPages, [
    {
      address: expected.initializationCall.delta.address,
      owner: expected.initializationCall.delta.owner,
    },
  ]);
  assert.equal(
    initializationDelta.payloadSha256,
    expected.initializationCall.delta.payloadSha256,
  );

  for (const [symbol, count] of Object.entries(expected.occurrenceCounts)) {
    assert.equal(
      result.supportedCalls.filter((call_) => call_.symbol === symbol).length,
      count,
      `${symbol} continuation count changed`,
    );
  }
  const finalCall = result.supportedCalls.at(-1);
  assert.equal(finalCall.sequence, expected.supportedCalls);
  assert.equal(finalCall.symbol, "SetLastError");
  assert.equal(finalCall.occurrence, 30);

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint, null);
  assert.equal(result.fault.fault.class, "fault");
  assert.equal(result.fault.fault.stage, "guest-execution");
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(
    result.fault.fault.instructionPointer,
    unsupported.instructionPointer,
  );
  assert.equal(
    result.fault.fault.previousInstructionPointer,
    unsupported.previousInstructionPointer,
  );
  assert.equal(result.fault.fault.faultAddress, unsupported.faultAddress);
  assert.equal(
    result.fault.fault.pageFaultErrorCode,
    unsupported.pageFaultErrorCode,
  );
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.deepEqual(result.terminal.terminal, {
    kind: "page-fault",
    instructionPointer: unsupported.instructionPointer,
    previousInstructionPointer: unsupported.previousInstructionPointer,
    faultAddress: unsupported.faultAddress,
    pageFaultErrorCode: unsupported.pageFaultErrorCode,
  });
}

function validatePrivateHeapSize16Result(result) {
  const expected = privateHeapSize16Expected;
  const allocations = result.supportedCalls.filter(
    ({ symbol }) => symbol === "HeapAlloc",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(allocations.length, 5);
  const allocation = allocations[4];
  assert.deepEqual(allocation.arguments, [expected.heapHandle, 0, 16]);
  assert.deepEqual(allocation.result, {
    action: "set",
    value: expected.guestAddress,
  });
  assert.equal(allocation.implementationId, "kernel32.heap-alloc.v2");
  assert.equal(allocation.lastError, 0);
  assert.deepEqual(allocation.guestWrites, []);
  assert.equal(allocation.stackPointerAfter, allocation.stackPointerBefore + 16);
  assert.equal(
    allocation.trapInstructionCount,
    expected.allocationInstructionCount,
  );
  assert.equal(allocation.returnAddress, expected.allocationReturnAddress);
  assert.deepEqual(allocation.heapAllocation, {
    heapHandle: expected.heapHandle,
    allocation: {
      id: 5,
      address: expected.guestAddress,
      byteLength: 16,
      reservedByteLength: 16,
      initialization: "deterministic-uninitialized",
    },
    liveAllocations: 5,
  });

  const allocationDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.allocationDelta.sequence,
  );
  assert(allocationDelta, "size-16 allocation delta is missing");
  assert.equal(
    allocationDelta.fromGeneration,
    expected.allocationDelta.fromGeneration,
  );
  assert.equal(allocationDelta.generation, expected.allocationDelta.generation);
  assert.equal(allocationDelta.mappingOperations, 0);
  assert.deepEqual(allocationDelta.dirtyPages, [
    {
      address: expected.allocationDelta.address,
      owner: expected.allocationDelta.owner,
    },
  ]);
  assert.equal(allocationDelta.payloadBytes, page.size);
  assert.equal(
    allocationDelta.payloadSha256,
    expected.allocationDelta.payloadSha256,
  );
  assert.deepEqual(allocationDelta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });

  const consumerWriteback = result.writebacks.find(
    ({ sequence }) => sequence === expected.consumerWriteback.sequence,
  );
  assert(consumerWriteback, "size-16 consumer writeback is missing");
  assert.equal(
    consumerWriteback.instructionCount,
    expected.consumerWriteback.instructionCount,
  );
  assert.deepEqual(
    consumerWriteback.dirtyPages,
    expected.consumerWriteback.dirtyPages,
  );
  assert.equal(
    consumerWriteback.payloadSha256,
    expected.consumerWriteback.payloadSha256,
  );
  assert.equal(
    consumerWriteback.receipt.fromGeneration,
    expected.consumerWriteback.fromGeneration,
  );
  assert.equal(
    consumerWriteback.receipt.generation,
    expected.consumerWriteback.generation,
  );
  assert.equal(
    consumerWriteback.receipt.changedBytes,
    expected.consumerWriteback.changedBytes,
  );

  for (const [symbol, instructionCount, returnAddress] of expected.followupCalls) {
    const followup = result.supportedCalls.find(
      (candidate) =>
        candidate.symbol === symbol &&
        candidate.trapInstructionCount === instructionCount,
    );
    assert(followup, `${symbol} follow-up call at ${instructionCount} is missing`);
    assert.equal(followup.returnAddress, returnAddress);
  }

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateGetModuleFileNameAResult(result) {
  const expected = getModuleFileNameAExpected;
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "GetModuleFileNameA",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 1);
  const call = calls[0];
  assert.deepEqual(call.arguments, expected.call.arguments);
  assert.deepEqual(call.result, { action: "set", value: expected.call.result });
  assert.equal(call.implementationId, "kernel32.get-module-file-name-a.v1");
  assert.equal(call.lastError, 0);
  assert.equal(call.stackPointerAfter, call.stackPointerBefore + 16);
  assert.equal(call.trapInstructionCount, expected.call.instructionCount);
  assert.equal(call.returnAddress, expected.call.returnAddress);
  assert.equal(call.guestWrites.length, 1);
  assert.equal(call.guestWrites[0].address, expected.call.arguments[1]);
  assert.deepEqual(call.guestWrites[0].bytes, expected.call.pathBytes);
  assert.deepEqual(call.guestWrites[0].result, {
    bytesWritten: expected.call.pathBytes.length,
    dirtyPages: 1,
    invalidation: {
      generation: expected.call.delta.generation,
      dataTlb: false,
      instructionFetch: false,
      generatedCode: false,
    },
  });
  const delta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.call.delta.sequence,
  );
  assert(delta, `GetModuleFileNameA delta ${expected.call.delta.sequence} is missing`);
  assert.equal(delta.fromGeneration, expected.call.delta.fromGeneration);
  assert.equal(delta.generation, expected.call.delta.generation);
  assert.equal(delta.mappingOperations, 0);
  assert.deepEqual(delta.dirtyPages, [
    {
      address: expected.call.delta.pageAddress,
      owner: expected.call.delta.owner,
    },
  ]);
  assert.equal(delta.payloadBytes, page.size);
  assert.equal(delta.payloadSha256, expected.call.delta.payloadSha256);

  for (const [symbol, instructionCount, returnAddress] of expected.followupCalls) {
    const followup = result.supportedCalls.find(
      (candidate) =>
        candidate.symbol === symbol &&
        candidate.trapInstructionCount === instructionCount,
    );
    assert(followup, `${symbol} follow-up call at ${instructionCount} is missing`);
    assert.equal(followup.returnAddress, returnAddress);
  }

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateInterlockedDecrementResult(result) {
  const expected = interlockedDecrementExpected;
  const calls = result.supportedCalls.filter(
    ({ symbol }) => symbol === "InterlockedDecrement",
  );
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
        (total, writeback) => total + writeback.dirtyPages.length,
        0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
        (total, writeback) => total + writeback.receipt.changedBytes,
        0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, expected.calls.length);
  for (const [index, expectedCall] of expected.calls.entries()) {
    const call = calls[index];
    assert.deepEqual(call.arguments, expectedCall.arguments);
    assert.deepEqual(call.result, { action: "set", value: expectedCall.result });
    assert.equal(call.implementationId, "kernel32.interlocked-decrement.v1");
    assert.equal(call.lastError, 87);
    assert.equal(call.stackPointerAfter, call.stackPointerBefore + 8);
    assert.equal(call.trapInstructionCount, expectedCall.instructionCount);
    assert.equal(call.returnAddress, expectedCall.returnAddress);
    assert.deepEqual(call.interlockedDecrement, {
      address: expectedCall.arguments[0],
      before: expectedCall.before,
      after: expectedCall.after,
    });
    assert.equal(call.guestWrites.length, 1);
    assert.equal(call.guestWrites[0].address, expectedCall.arguments[0]);
    assert.deepEqual(
      call.guestWrites[0].bytes,
      [...Buffer.from(Uint32Array.of(expectedCall.result).buffer)],
    );
    assert.deepEqual(call.guestWrites[0].result, {
      bytesWritten: 4,
      dirtyPages: 1,
      invalidation: {
        generation: expectedCall.delta.generation,
        dataTlb: false,
        instructionFetch: false,
        generatedCode: false,
      },
    });
    const delta = result.hostDeltas.find(
      ({ sequence }) => sequence === expectedCall.delta.sequence,
    );
    assert(delta, `InterlockedDecrement delta ${expectedCall.delta.sequence} is missing`);
    assert.equal(delta.fromGeneration, expectedCall.delta.fromGeneration);
    assert.equal(delta.generation, expectedCall.delta.generation);
    assert.equal(delta.mappingOperations, 0);
    assert.deepEqual(delta.dirtyPages, [
      {
        address: expectedCall.delta.pageAddress,
        owner: expectedCall.delta.owner,
      },
    ]);
    assert.equal(delta.payloadBytes, page.size);
    assert.equal(delta.payloadSha256, expectedCall.delta.payloadSha256);
  }

  for (const [symbol, instructionCount, returnAddress] of expected.followupCalls) {
    const followup = result.supportedCalls.find(
      (candidate) =>
        candidate.symbol === symbol &&
        candidate.trapInstructionCount === instructionCount,
    );
    assert(followup, `${symbol} follow-up call at ${instructionCount} is missing`);
    assert.equal(followup.returnAddress, returnAddress);
  }

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function validateGetCpInfoResult(result) {
  const expected = getCpInfoExpected;
  const calls = result.supportedCalls.filter(({ symbol }) => symbol === "GetCPInfo");
  assert.equal(result.frozenSemanticCalls, expected.frozenSemanticCalls);
  assert.equal(result.supportedCalls.length, expected.supportedCalls);
  assert.equal(result.writebacks.length, expected.writebackBatches);
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.dirtyPages.length,
      0,
    ),
    expected.dirtyPages,
  );
  assert.equal(
    result.writebacks.reduce(
      (total, writeback) => total + writeback.receipt.changedBytes,
      0,
    ),
    expected.changedBytes,
  );
  assert.equal(result.hostDeltas.length, expected.hostDeltaBatches);
  assert.equal(result.finalGeneration, expected.finalGeneration);
  assert.equal(calls.length, 1);
  const call = calls[0];
  assert.deepEqual(call.arguments, [expected.codePage, expected.outputAddress]);
  assert.deepEqual(call.result, { action: "set", value: 1 });
  assert.equal(call.implementationId, "kernel32.get-cp-info.v1");
  assert.equal(call.lastError, expected.lastError);
  assert.equal(call.stackPointerAfter, call.stackPointerBefore + 12);
  assert.equal(call.trapInstructionCount, expected.instructionCount);
  assert.equal(call.returnAddress, expected.returnAddress);
  assert.equal(call.guestWrites.length, 1);
  assert.equal(call.guestWrites[0].address, expected.outputAddress);
  assert.deepEqual(call.guestWrites[0].bytes, expected.outputBytes);
  assert.deepEqual(call.guestWrites[0].result, {
    bytesWritten: expected.outputBytes.length,
    dirtyPages: 1,
    invalidation: {
      generation: expected.outputDelta.generation,
      dataTlb: false,
      instructionFetch: false,
      generatedCode: false,
    },
  });

  const outputDelta = result.hostDeltas.find(
    ({ sequence }) => sequence === expected.outputDelta.sequence,
  );
  assert(outputDelta, "GetCPInfo output delta is missing");
  assert.equal(outputDelta.fromGeneration, expected.outputDelta.fromGeneration);
  assert.equal(outputDelta.generation, expected.outputDelta.generation);
  assert.equal(outputDelta.mappingOperations, 0);
  assert.deepEqual(outputDelta.dirtyPages, [
    {
      address: expected.outputDelta.pageAddress,
      owner: expected.outputDelta.owner,
    },
  ]);
  assert.equal(outputDelta.payloadBytes, page.size);
  assert.equal(outputDelta.payloadSha256, expected.outputDelta.payloadSha256);
  assert.deepEqual(outputDelta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });

  const consumerWriteback = result.writebacks.find(
    ({ sequence }) => sequence === expected.consumerWriteback.sequence,
  );
  assert(consumerWriteback, "GetCPInfo consumer writeback is missing");
  assert.equal(
    consumerWriteback.instructionCount,
    expected.consumerWriteback.instructionCount,
  );
  assert.deepEqual(
    consumerWriteback.dirtyPages,
    expected.consumerWriteback.dirtyPages,
  );
  assert.equal(
    consumerWriteback.payloadSha256,
    expected.consumerWriteback.payloadSha256,
  );
  assert.equal(
    consumerWriteback.receipt.fromGeneration,
    expected.consumerWriteback.fromGeneration,
  );
  assert.equal(
    consumerWriteback.receipt.generation,
    expected.consumerWriteback.generation,
  );
  assert.equal(
    consumerWriteback.receipt.changedBytes,
    expected.consumerWriteback.changedBytes,
  );

  for (const [symbol, instructionCount, returnAddress] of expected.followupCalls) {
    const followup = result.supportedCalls.find(
      (candidate) =>
        candidate.symbol === symbol &&
        candidate.trapInstructionCount === instructionCount,
    );
    assert(followup, `${symbol} follow-up call is missing`);
    assert.equal(followup.returnAddress, returnAddress);
  }

  const unsupported = expected.firstUnsupported;
  assert.equal(result.fault.capabilityCluster, unsupported.capabilityCluster);
  assert.equal(result.fault.endpoint.kind, "runtime-system-export");
  assert.equal(result.fault.endpoint.endpoint.id, unsupported.endpointId);
  assert.equal(result.fault.endpoint.endpoint.address, unsupported.endpointAddress);
  assert.equal(result.fault.endpoint.endpoint.module, unsupported.module);
  assert.equal(result.fault.endpoint.endpoint.symbol.kind, "name");
  assert.equal(result.fault.endpoint.endpoint.symbol.name, unsupported.symbol);
  assert.equal(result.fault.endpoint.endpoint.iatAddress, unsupported.iatAddress);
  assert.equal(result.fault.fault.code, unsupported.faultCode);
  assert.equal(result.fault.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(result.terminal.instructionCount, unsupported.instructionCount);
  assert.equal(result.terminal.terminal.kind, "owned-import-thunk");
  assert.equal(result.terminal.terminal.returnAddress, unsupported.returnAddress);
  assert.deepEqual(
    result.terminal.terminal.stackWords.slice(1, unsupported.arguments.length + 1),
    unsupported.arguments,
  );
}

function buildGenericRuntimeReentryReport({
  execution,
  genericResult,
  offer,
  receipt,
  runtimeReady,
  sourceAfter,
  sourceBefore,
}) {
  const totalDirtyPages = genericResult.writebacks.reduce(
    (total, writeback) => total + writeback.dirtyPages.length,
    0,
  );
  const totalChangedBytes = genericResult.writebacks.reduce(
    (total, writeback) => total + writeback.receipt.changedBytes,
    0,
  );
  return {
    schema:
      genericRuntimeCapability === null
        ? "vine.v86-bgi-generic-runtime-reentry"
        : initializeCriticalSectionEnabled
          ? "vine.v86-bgi-initialize-critical-section"
          : heapSizeEnabled
          ? "vine.v86-bgi-heap-size"
          : setUnhandledExceptionFilterEnabled
          ? "vine.v86-bgi-set-unhandled-exception-filter"
          : isProcessorFeaturePresentEnabled
          ? "vine.v86-bgi-is-processor-feature-present"
          : privateHeapSize128Enabled
          ? "vine.v86-bgi-private-heap-size-128"
          : privateHeapFreeSize1Enabled
          ? "vine.v86-bgi-private-heap-free-size-1"
          : privateHeapSize4Enabled
          ? "vine.v86-bgi-private-heap-size-4"
          : privateHeapSize16Enabled
          ? "vine.v86-bgi-private-heap-size-16"
          : getModuleFileNameAEnabled
          ? "vine.v86-bgi-get-module-file-name-a"
          : interlockedDecrementEnabled
          ? "vine.v86-bgi-interlocked-decrement"
          : lcMapStringWEnabled
          ? "vine.v86-bgi-lc-map-string-w"
          : getStringTypeWEnabled
          ? "vine.v86-bgi-get-string-type-w"
          : multiByteToWideCharEnabled
          ? "vine.v86-bgi-multi-byte-to-wide-char"
          : getCpInfoEnabled
          ? "vine.v86-bgi-get-cp-info"
          : privateHeapSize544Enabled
          ? "vine.v86-bgi-private-heap-size-544"
          : getAcpEnabled
          ? "vine.v86-bgi-get-acp"
          : setLastErrorEnabled
          ? "vine.v86-bgi-set-last-error"
          : flsGetValueEnabled
          ? "vine.v86-bgi-fls-get-value"
          : tlsGetValueEnabled
          ? "vine.v86-bgi-tls-get-value"
          : getLastErrorEnabled
          ? "vine.v86-bgi-get-last-error"
          : privateHeapSize1Enabled
          ? "vine.v86-bgi-private-heap-size-1"
          : wideCharToMultiByteEnabled
          ? "vine.v86-bgi-wide-char-to-multi-byte"
          : environmentStringsWEnabled
          ? "vine.v86-bgi-environment-strings-w"
          : commandLineAEnabled
          ? "vine.v86-bgi-command-line-a"
          : setHandleCountEnabled
          ? "vine.v86-bgi-set-handle-count"
          : standardErrorFileTypeEnabled
          ? "vine.v86-bgi-standard-error-file-type"
          : standardErrorHandleEnabled
          ? "vine.v86-bgi-standard-error-handle"
          : standardOutputFileTypeEnabled
          ? "vine.v86-bgi-standard-output-file-type"
          : standardOutputHandleEnabled
          ? "vine.v86-bgi-standard-output-handle"
          : fileTypeEnabled
          ? "vine.v86-bgi-file-type"
          : standardInputHandleEnabled
          ? "vine.v86-bgi-standard-input-handle"
          : privateHeapSecondAllocationEnabled
          ? "vine.v86-bgi-private-heap-second-allocation"
          : interlockedIncrementEnabled
          ? "vine.v86-bgi-interlocked-increment"
          : fiberLocalStorageValueEnabled
          ? "vine.v86-bgi-fiber-local-storage-value"
          : privateHeapAllocationEnabled
          ? "vine.v86-bgi-private-heap-allocation"
          : fiberLocalStorageAllocationEnabled
          ? "vine.v86-bgi-fiber-local-storage-allocation"
          : genericRuntimeCapability === "kernel32-pointer-decoding"
            ? "vine.v86-bgi-pointer-decoding"
            : "vine.v86-bgi-critical-section-synchronization",
    schemaVersion: 1,
    source: offer.source,
    sourceSnapshot: {
      immutableAcrossRun: true,
      before: sourceBefore,
      after: sourceAfter,
    },
    contract: {
      runtimeStateAuthority: "rust",
      memoryByteAuthority: "rust-guest-memory",
      executionMirror: "v86-paging",
      cpuWriteback: "automatic-pte-dirty-vinewb01",
      argumentWidthSource: "api-contract",
      apiSpecificReconciliationLists: false,
      apiSpecificRequestsOrReceipts: false,
      compatibilityPolicy: runtimePolicy,
      frozenCheckpoint: selectedCheckpoint.id,
      frozenSupportedCalls: selectedCheckpoint.supportedCallCount,
      continueFromOwnedTrapWithoutNumberedCheckpoint: true,
      ...(genericRuntimeCapability === null
        ? {}
        : { enabledCapability: genericRuntimeCapability }),
    },
    process: runtimeReady,
    reflection: {
      pages: offer.reflection.pages.length,
      generation: offer.reflection.invalidation.generation,
      contentSha256: offer.reflection.contentSha256,
      acknowledgedGeneration: receipt.acknowledgedGeneration,
    },
    entry: execution,
    frozenRegression: {
      semanticCallsValidated: genericResult.frozenSemanticCalls,
      instructionCount: selectedCheckpoint.evidence.instructionCount,
      reportSha256: selectedCheckpoint.evidence.reportSha256,
      reflectionSha256: selectedCheckpoint.evidence.reflectionSha256,
    },
    synchronization: {
      writebackBatches: genericResult.writebacks.length,
      dirtyPages: totalDirtyPages,
      changedBytes: totalChangedBytes,
      hostDeltaBatches: genericResult.hostDeltas.length,
      finalGeneration: genericResult.finalGeneration,
      jsonPageByteArrays: false,
      writebacks: genericResult.writebacks,
      hostDeltas: genericResult.hostDeltas,
    },
    supportedContinuation: genericResult.supportedCalls,
    firstUnsupported: {
      capabilityCluster: genericResult.fault.capabilityCluster,
      endpoint: genericResult.fault.endpoint,
      fault: genericResult.fault.fault,
      memory: genericResult.fault.memory,
      memoryContentSha256: genericResult.fault.memoryContentSha256,
      instructionCount: genericResult.terminal.instructionCount,
      terminal: genericResult.terminal.terminal,
    },
    stop: {
      stage: "runtime-capability",
      code: "first-unsupported-capability-cluster",
      message:
        "BGI resumed through the generic runtime until the first process-owned endpoint without a callable capability contract",
    },
  };
}

function createProtocolReader(stream) {
  const iterator = stream[Symbol.asyncIterator]();
  let buffered = Buffer.alloc(0);
  const fill = async (byteLength, label) => {
    while (buffered.length < byteLength) {
      const next = await iterator.next();
      assert(!next.done, `${label} ended with ${byteLength - buffered.length} bytes missing`);
      buffered = Buffer.concat([buffered, Buffer.from(next.value)]);
    }
  };
  return {
    async readExact(byteLength, label = "protocol stream") {
      await fill(byteLength, label);
      const result = buffered.subarray(0, byteLength);
      buffered = buffered.subarray(byteLength);
      return result;
    },
    async readLine(maximumBytes, label) {
      while (true) {
        const newline = buffered.indexOf(0x0a);
        if (newline !== -1) {
          assert(newline <= maximumBytes, `${label} exceeds the protocol line limit`);
          let result = buffered.subarray(0, newline);
          buffered = buffered.subarray(newline + 1);
          if (result.at(-1) === 0x0d) {
            result = result.subarray(0, result.length - 1);
          }
          return result;
        }
        assert(buffered.length <= maximumBytes, `${label} exceeds the protocol line limit`);
        const next = await iterator.next();
        assert(!next.done, `${label} was not emitted`);
        buffered = Buffer.concat([buffered, Buffer.from(next.value)]);
      }
    },
  };
}

async function readRuntimeFrame(reader, expectedKind, label) {
  const header = await reader.readExact(runtimeFrame.headerBytes, label);
  assert.equal(header.subarray(0, 8).toString("ascii"), runtimeFrame.magic);
  const view = dataView(header);
  assert.equal(view.getUint32(8, true), runtimeFrame.version);
  const kind = view.getUint32(12, true);
  if (expectedKind !== null) {
    assert.equal(kind, expectedKind);
  }
  const controlBytes = view.getUint32(16, true);
  const payloadBytes = view.getUint32(20, true);
  assert(controlBytes <= 1024 * 1024);
  assert(payloadBytes <= protocol.maximumLineBytes);
  return {
    kind,
    control: await reader.readExact(controlBytes, `${label} control`),
    payload: await reader.readExact(payloadBytes, `${label} payload`),
  };
}

async function readRuntimeJsonFrame(reader, expectedKind, label) {
  const frame = await readRuntimeFrame(reader, expectedKind, label);
  assert.equal(frame.payload.length, 0);
  return JSON.parse(frame.control.toString("utf8"));
}

async function writeRuntimeJsonFrame(stream, kind, value) {
  await writeRuntimeFrame(stream, kind, Buffer.from(JSON.stringify(value)), Buffer.alloc(0));
}

async function writeRuntimeFrame(stream, kind, control, payload) {
  const header = Buffer.alloc(runtimeFrame.headerBytes);
  header.write(runtimeFrame.magic, 0, "ascii");
  const view = dataView(header);
  view.setUint32(8, runtimeFrame.version, true);
  view.setUint32(12, kind, true);
  view.setUint32(16, control.length, true);
  view.setUint32(20, payload.length, true);
  const bytes = Buffer.concat([header, control, payload]);
  if (!stream.write(bytes)) {
    await new Promise((resolve) => stream.once("drain", resolve));
  }
}

function encodeGeneration(generation) {
  const bytes = Buffer.alloc(8);
  dataView(bytes).setBigUint64(0, BigInt(generation), true);
  return bytes;
}

function dataView(bytes) {
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
}

function safeNumber(value, label) {
  assert(value <= BigInt(Number.MAX_SAFE_INTEGER), `${label} exceeds safe integer range`);
  return Number(value);
}

function validateOffer(offer, offerLineBytes) {
  assert.equal(offer.schema, "vine.v86-memory-reflection");
  assert.equal(offer.schemaVersion, 1);
  assert.deepEqual(offer.source, expected.source);
  assert.equal(offer.imageLoadBase, expected.image.loadBase);
  assert.equal(offer.entryPointAddress, expected.image.entry);
  assert.equal(offer.imageOwner, expected.image.owner);
  assert.equal(offer.thunks.length, expected.imports.total);
  assert.deepEqual(offer.firstThunk, offer.thunks[0]);
  validateThunk(offer.thunks[0], expected.imports.first, false);
  validateThunk(offer.thunks[1], expected.imports.second, true);
  validateThunk(offer.thunks.at(-1), expected.imports.last, true);
  validateThunk(
    offer.thunks[expected.terminal.thunk.id - 1],
    expected.terminal.thunk,
    !dispatchSystemTime,
  );
  validateThunk(
    offer.thunks[expected.firstCall.nextThunk.identity.id - 1],
    expected.firstCall.nextThunk.identity,
    !dispatchProcessId,
  );
  validateThunk(
    offer.thunks[expected.secondCall.nextThunk.identity.id - 1],
    expected.secondCall.nextThunk.identity,
    !dispatchThreadId,
  );
  validateThunk(
    offer.thunks[expected.thirdCall.nextThunk.identity.id - 1],
    expected.thirdCall.nextThunk.identity,
    !dispatchTickCount,
  );
  validateThunk(
    offer.thunks[expected.fourthCall.nextThunk.identity.id - 1],
    expected.fourthCall.nextThunk.identity,
    !dispatchPerformanceCounter,
  );
  validateThunk(
    offer.thunks[expected.fifthCall.nextThunk.identity.id - 1],
    expected.fifthCall.nextThunk.identity,
    !dispatchStartupInfo,
  );
  validateThunk(
    offer.thunks[expected.sixthCall.nextThunk.identity.id - 1],
    expected.sixthCall.nextThunk.identity,
    !dispatchHeapSetInformation,
  );
  validateThunk(
    offer.thunks[expected.seventhCall.nextThunk.identity.id - 1],
    expected.seventhCall.nextThunk.identity,
    !dispatchHeapCreate,
  );
  validateThunk(
    offer.thunks[expected.eighthCall.nextThunk.identity.id - 1],
    expected.eighthCall.nextThunk.identity,
    !dispatchModuleHandle,
  );
  validateThunk(
    offer.thunks[expected.ninthCall.nextThunk.identity.id - 1],
    expected.ninthCall.nextThunk.identity,
    !dispatchGetProcAddress,
  );
  assert.equal(
    offer.thunks.filter(({ disposition }) => disposition === undefined).length,
    expected.imports.declared,
  );
  assert.equal(
    offer.thunks.filter(({ disposition }) => disposition === "unsupported-trap").length,
    expected.imports.unsupported,
  );
  for (let index = 0; index < offer.thunks.length; index += 1) {
    const thunk = offer.thunks[index];
    assert.equal(thunk.id, index + 1);
    assert.equal(thunk.address, 0x7f000000 + index * 0x10);
    assert.equal(thunk.requesterOwner, expected.image.owner);
  }

  assert.equal(offer.callbackReturnSentinel.id, expected.control.callbackId);
  assert.equal(offer.callbackReturnSentinel.address, expected.control.callbackAddress);
  assert.equal(offer.callbackReturnSentinel.owner, expected.control.owner);
  assert.equal(offer.processEntryReturnSentinel.id, expected.control.processEntryId);
  assert.equal(
    offer.processEntryReturnSentinel.address,
    expected.control.processEntryAddress,
  );
  assert.equal(offer.processEntryReturnSentinel.owner, expected.control.owner);
  assert.equal(offer.initialThreadStack.owner, expected.stack.owner);
  assert.equal(offer.initialThreadStack.stackRange.start, expected.stack.base);
  assert.equal(offer.initialThreadStack.stackTop, expected.stack.top);
  assert.equal(offer.initialThreadStack.initialEsp, expected.stack.initialEsp);

  const environment = offer.initialProcessEnvironment;
  const parameters = environment.processParameters;
  assert.equal(environment.tebOwner, expected.processEnvironment.tebOwner);
  assert.equal(environment.pebOwner, expected.processEnvironment.pebOwner);
  assert.equal(environment.tebAddress, expected.processEnvironment.tebBase);
  assert.equal(environment.pebAddress, expected.processEnvironment.pebBase);
  assert.equal(parameters.processParametersOwner, expected.processEnvironment.processParametersOwner);
  assert.equal(parameters.commandLineOwner, expected.processEnvironment.commandLineOwner);
  assert.equal(
    parameters.processParametersAddress,
    expected.processEnvironment.processParametersBase,
  );
  assert.equal(
    parameters.commandLineBufferAddress,
    expected.processEnvironment.commandLineBase,
  );
  assert.equal(parameters.commandLine, expected.processEnvironment.commandLine);
  assert.equal(
    parameters.commandLineLengthBytes,
    expected.processEnvironment.commandLineLengthBytes,
  );
  assert.equal(
    parameters.commandLineMaximumLengthBytes,
    expected.processEnvironment.commandLineMaximumLengthBytes,
  );
  assert.equal(
    environment.tebContentSha256,
    expected.processEnvironment.tebContentSha256,
  );
  assert.equal(
    environment.pebContentSha256,
    expected.processEnvironment.pebContentSha256,
  );
  assert.equal(
    parameters.processParametersContentSha256,
    expected.processEnvironment.processParametersContentSha256,
  );
  assert.equal(
    parameters.commandLineContentSha256,
    expected.processEnvironment.commandLineContentSha256,
  );
  if (commandLineAEnabled) {
    assert.deepEqual(parameters.ansi, {
      bufferAddress: expected.processEnvironment.ansiBuffer,
      byteLengthWithNul: expected.processEnvironment.ansiByteLengthWithNul,
      encoding: expected.processEnvironment.ansiEncoding,
    });
  } else {
    assert.equal(parameters.ansi, undefined);
  }
  if (environmentStringsWEnabled) {
    assert.deepEqual(parameters.environmentStrings, {
      authorityBufferAddress: expected.processEnvironment.environmentAuthorityBuffer,
      copyBufferAddress: expected.processEnvironment.environmentCopyBuffer,
      byteLengthWithDoubleNul:
        expected.processEnvironment.environmentByteLengthWithDoubleNul,
      encoding: expected.processEnvironment.environmentEncoding,
    });
  } else {
    assert.equal(parameters.environmentStrings, undefined);
  }
  assert.deepEqual(environment.invalidation, {
    generation: expected.processEnvironment.generation,
    dataTlb: true,
    instructionFetch: true,
    generatedCode: true,
  });
  if (dispatchGetProcAddress) {
    assert.deepEqual(offer.dynamicExportThunk, expected.dynamicExport);
  } else {
    assert.equal(offer.dynamicExportThunk, undefined);
  }
  if (dispatchGetProcAddressSecond) {
    assert.deepEqual(offer.dynamicExportThunks, expected.dynamicExports);
  } else {
    assert.equal(offer.dynamicExportThunks, undefined);
  }

  const reflected = offer.reflection;
  assert.equal(reflected.pageSize, page.size);
  assert(reflected.pages.length <= protocol.maximumReflectionPages);
  assert.equal(reflected.pages.length, expected.memory.reflectedPages);
  assert.deepEqual(
    reflected.invalidation,
    dispatchGetProcAddress ? expected.dynamicExport.invalidation : environment.invalidation,
  );
  assert.equal(reflectionContentSha256(reflected.pages), reflected.contentSha256);
  if (expected.reflectionContentSha256 !== null) {
    assert.equal(reflected.contentSha256, expected.reflectionContentSha256);
  }
  assert.equal(offerLineBytes, expected.offerLineBytes);
  for (const reflectedPage of reflected.pages) {
    assert.equal(reflectedPage.ownerSealed, true);
    assert.equal(reflectedPage.dirty, true);
    assert.equal(reflectedPage.bytes.length, page.size);
  }
  validateReflectedLayout(reflected.pages);
}

function validateThunk(thunk, identity, unsupported) {
  for (const [key, value] of Object.entries(identity)) {
    assert.equal(thunk[key], value);
  }
  assert.equal(thunk.kind, identity.ordinal === undefined ? "name" : "ordinal");
  assert.equal(thunk.disposition, unsupported ? "unsupported-trap" : undefined);
}

function validateReflectedLayout(pages) {
  let index = 0;
  assertReflectedPage(pages[index], expected.stack.base, expected.stack.owner, true, false);
  index += 1;
  let imageReadable = 0;
  let imageWritable = 0;
  let imageExecutable = 0;
  for (let pageIndex = 0; pageIndex < expected.image.pages; pageIndex += 1) {
    const reflected = pages[index];
    assert.equal(reflected.address, expected.image.loadBase + pageIndex * page.size);
    assert.equal(reflected.owner, expected.image.owner);
    imageReadable += Number(reflected.protection.read);
    imageWritable += Number(reflected.protection.write);
    imageExecutable += Number(reflected.protection.execute);
    index += 1;
  }
  assert.equal(imageReadable, expected.image.readablePages);
  assert.equal(imageWritable, expected.image.writablePages);
  assert.equal(imageExecutable, expected.image.executablePages);
  assertReflectedPage(pages[index], 0x7f000000, expected.imports.owner, false, true);
  index += 1;
  assertReflectedPage(pages[index], 0x7f001000, expected.imports.owner, false, true);
  index += 1;
  assertReflectedPage(pages[index], expected.control.page, expected.control.owner, false, true);
  index += 1;
  if (dispatchGetProcAddress) {
    assertReflectedPage(
      pages[index],
      expected.dynamicExport.address,
      expected.dynamicExport.owner,
      false,
      true,
    );
    index += 1;
  }
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.processParametersBase,
    expected.processEnvironment.processParametersOwner,
    true,
    false,
  );
  index += 1;
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.commandLineBase,
    expected.processEnvironment.commandLineOwner,
    true,
    false,
  );
  index += 1;
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.tebBase,
    expected.processEnvironment.tebOwner,
    true,
    false,
  );
  index += 1;
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.pebBase,
    expected.processEnvironment.pebOwner,
    true,
    false,
  );
  index += 1;
  assert.equal(index, pages.length);
}

function assertReflectedPage(reflected, address, owner, writable, executable) {
  assert.equal(reflected.address, address);
  assert.equal(reflected.owner, owner);
  assert.deepEqual(reflected.protection, { read: true, write: writable, execute: executable });
}

async function prepareV86(offer) {
  const wasmBytes = fs.readFileSync(wasmPath);
  assert.equal(wasmBytes.byteLength, expected.wasm.bytes);
  assert.equal(sha256(wasmBytes), expected.wasm.sha256);
  const module = new WebAssembly.Module(wasmBytes);
  const imports = WebAssembly.Module.imports(module);
  const exportsDescription = WebAssembly.Module.exports(module);
  assert.equal(imports.length, expected.wasm.imports);
  assert.equal(exportsDescription.length, expected.wasm.exports);
  const exportNames = new Set(exportsDescription.map(({ name }) => name));
  for (const name of [
    "allocate_memory",
    "full_clear_tlb",
    "jit_clear_cache_js",
    "memory",
    "reset_cpu",
    "rust_init",
    "vine_cpu_state_ptr",
    "vine_cpu_state_restore",
    "vine_cpu_state_save",
    "vine_cpu_state_size",
    "vine_cpu_state_version",
    "vine_execute_budget",
    "vine_get_stop_error_code",
    "zero_memory",
  ]) {
    assert(exportNames.has(name), `missing adapter export ${name}`);
  }

  const hostImportCalls = {};
  const environment = {
    __indirect_function_table: new WebAssembly.Table({ element: "anyfunc", initial: 1_924 }),
  };
  for (const imported of imports) {
    if (imported.kind !== "function") {
      continue;
    }
    environment[imported.name] = (...args) => {
      hostImportCalls[imported.name] = (hostImportCalls[imported.name] ?? 0) + 1;
      assert.equal(
        imported.name,
        "microtick",
        `unexpected host import ${imported.name}(${args.join(", ")})`,
      );
      assert.equal(args.length, 0);
      return 0;
    };
  }

  const { instance } = await WebAssembly.instantiate(wasmBytes, { env: environment });
  const wasm = instance.exports;
  wasm.rust_init();
  let privateState = new DataView(wasm.memory.buffer);
  privateState.setUint32(adapterPrivateOffset.memorySize, guest.physicalPoolBytes, true);
  const guestMemoryBase = wasm.allocate_memory(guest.physicalPoolBytes);
  privateState = new DataView(wasm.memory.buffer);
  const physicalMemory = new Uint8Array(
    wasm.memory.buffer,
    guestMemoryBase,
    guest.physicalPoolBytes,
  );
  const physicalView = new DataView(
    physicalMemory.buffer,
    physicalMemory.byteOffset,
    physicalMemory.byteLength,
  );
  wasm.zero_memory(0, guest.physicalPoolBytes);

  const allocations = [];
  let nextPhysicalPage = page.size;
  const allocatePhysicalPage = (role) => {
    for (const excluded of guest.excludedPhysicalRanges) {
      if (nextPhysicalPage >= excluded.start && nextPhysicalPage < excluded.end) {
        nextPhysicalPage = excluded.end;
      }
    }
    checkedEnd(nextPhysicalPage, page.size, guest.physicalPoolBytes, role);
    const address = nextPhysicalPage;
    nextPhysicalPage += page.size;
    physicalMemory.fill(0, address, address + page.size);
    allocations.push({ address, role });
    return address;
  };
  const writePhysicalU32 = (address, value, label) => {
    checkedEnd(address, 4, guest.physicalPoolBytes, label);
    physicalView.setUint32(address, value >>> 0, true);
  };
  const pageDirectory = allocatePhysicalPage("page-directory");
  const pageTables = new Map();
  const mappings = new Map();
  const ensurePageTable = (virtualBase) => {
    const directoryIndex = virtualBase >>> 22;
    let table = pageTables.get(directoryIndex);
    if (table === undefined) {
      table = allocatePhysicalPage(`page-table-${directoryIndex}`);
      pageTables.set(directoryIndex, table);
      writePhysicalU32(
        pageDirectory + directoryIndex * 4,
        table | page.present | page.writable | page.user,
        "page directory entry",
      );
    }
    return table;
  };
  const mapRuntimePage = (virtualBase, owner, ownerSealed, protection, bytes, role) => {
    checkedGuestEnd(virtualBase, page.size, role);
    assert.equal(virtualBase & (page.size - 1), 0);
    assert(!mappings.has(virtualBase), `duplicate mapping 0x${virtualBase.toString(16)}`);
    assert.equal(bytes.length, page.size, `${role} is not one guest page`);
    const table = ensurePageTable(virtualBase);
    const tableIndex = (virtualBase >>> 12) & 0x3ff;
    const physicalBase = allocatePhysicalPage(role);
    writePhysicalU32(
      table + tableIndex * 4,
      physicalBase |
        page.present |
        page.user |
        (protection.write ? page.writable : 0),
      `${role} page table entry`,
    );
    physicalMemory.set(Uint8Array.from(bytes), physicalBase);
    mappings.set(virtualBase, {
      owner,
      ownerSealed,
      physicalBase,
      pteAddress: table + tableIndex * 4,
      protection: { ...protection },
      role,
    });
  };
  for (const reflected of offer.reflection.pages) {
    mapRuntimePage(
      reflected.address,
      reflected.owner,
      reflected.ownerSealed,
      reflected.protection,
      reflected.bytes,
      `reflected-owner-${reflected.owner}`,
    );
  }

  const mappingFor = (address, label) => {
    checkedGuestEnd(address, 1, label);
    const virtualBase = (address & ~(page.size - 1)) >>> 0;
    const mapping = mappings.get(virtualBase);
    assert(mapping, `${label} uses unmapped guest address 0x${address.toString(16)}`);
    return mapping;
  };
  const preflightVirtualRange = (address, size, access, label) => {
    const end = checkedGuestEnd(address, size, label);
    const pages = [];
    for (let current = address; current < end; ) {
      const virtualBase = (current & ~(page.size - 1)) >>> 0;
      const mapping = mappingFor(current, label);
      assert(mapping.protection[access], `${label} is not guest-${access}able`);
      pages.push({
        address: virtualBase,
        owner: mapping.owner,
        protection: { ...mapping.protection },
      });
      current = Math.min(end, virtualBase + page.size);
    }
    return pages;
  };
  const readVirtualBytes = (address, size, label) => {
    preflightVirtualRange(address, size, "read", label);
    const bytes = new Uint8Array(size);
    for (let index = 0; index < size; index += 1) {
      const current = (address + index) >>> 0;
      const mapping = mappingFor(current, label);
      bytes[index] = physicalMemory[mapping.physicalBase + (current & (page.size - 1))];
    }
    return bytes;
  };
  const readVirtualU32 = (address, label) => {
    const bytes = readVirtualBytes(address, 4, label);
    return (
      bytes[0] |
      (bytes[1] << 8) |
      (bytes[2] << 16) |
      (bytes[3] << 24)
    ) >>> 0;
  };
  const applyDirtyPage = (dirty, label) => {
    assert.equal(dirty.address & (page.size - 1), 0, `${label} address is not page aligned`);
    assert.equal(dirty.bytes.length, page.size, `${label} is not one guest page`);
    const mapping = mappingFor(dirty.address, label);
    assert.equal(mapping.owner, dirty.owner, `${label} owner changed`);
    assert.deepEqual(mapping.protection, dirty.protection, `${label} protection changed`);
    physicalMemory.set(Uint8Array.from(dirty.bytes), mapping.physicalBase);
  };

  assert.equal(
    readVirtualU32(offer.initialThreadStack.initialEsp, "entry return sentinel"),
    expected.control.processEntryAddress,
  );
  wasm.reset_cpu();
  assert.equal(wasm.vine_cpu_state_version(), 1);
  assert.equal(wasm.vine_cpu_state_size(), 640);
  wasm.vine_cpu_state_save();
  const words = new Uint32Array(
    wasm.memory.buffer,
    wasm.vine_cpu_state_ptr(),
    wasm.vine_cpu_state_size() / 4,
  );
  assert.equal(words[stateWord.magic], 0x56494e45);
  assert.equal(words[stateWord.version], 1);
  assert.equal(words[stateWord.wordCount], 160);
  words.fill(0, stateWord.eax, stateWord.eax + 8);
  words[stateWord.esp] = offer.initialThreadStack.initialEsp;
  words[stateWord.instructionPointer] = offer.entryPointAddress;
  words[stateWord.previousInstructionPointer] = offer.entryPointAddress;
  words[stateWord.flags] = 2;
  words[stateWord.cpl] = 3;
  words[stateWord.protectedMode] = 1;
  words[stateWord.operandSize32] = 1;
  words[stateWord.stackSize32] = 1;
  words[stateWord.inHalt] = 0;
  for (let index = 0; index < 8; index += 1) {
    words[stateWord.segmentSelectorBase + index] = 0;
    words[stateWord.segmentOffsetBase + index] = 0;
    words[stateWord.segmentLimitBase + index] = 0xffffffff;
    words[stateWord.segmentMetaBase + index] = 0xf2;
  }
  words[stateWord.segmentOffsetBase + segmentIndex.fs] =
    offer.initialProcessEnvironment.tebAddress;
  words[stateWord.segmentMetaBase + segmentIndex.cs] = 0xfa;
  words[stateWord.controlRegisterBase] |= 0x80010001;
  words[stateWord.controlRegisterBase + 3] = pageDirectory;
  words[stateWord.controlRegisterBase + 4] |= 0x200;
  assert.equal(wasm.vine_cpu_state_restore(), 0);
  assert.equal(allocations.length, dispatchGetProcAddress ? 394 : 393);
  assert.deepEqual([...pageTables.keys()], [1, 64, 508, 511]);
  for (const allocation of allocations) {
    for (const excluded of guest.excludedPhysicalRanges) {
      assert(
        allocation.address + page.size <= excluded.start || allocation.address >= excluded.end,
        `${allocation.role} overlaps excluded physical range ${excluded.role}`,
      );
    }
  }

  return {
    allocations,
    hostImportCalls,
    mappings,
    mapRuntimePage,
    pageDirectory,
    pageTables,
    physicalMemory,
    physicalView,
    applyDirtyPage,
    preflightVirtualRange,
    readVirtualBytes,
    readVirtualU32,
    wasm,
  };
}

function validateReceipt(receipt, offer, context, receiptLineBytes) {
  assert.equal(receipt.schema, "vine.v86-memory-reflection-receipt");
  assert.equal(receipt.schemaVersion, 1);
  assert.equal(receipt.acknowledgedGeneration, expected.memory.generation);
  assert.equal(receipt.stats.owners, expected.memory.owners);
  assert.equal(receipt.stats.sealedOwners, expected.memory.owners);
  assert.equal(receipt.stats.regions, expected.memory.owners);
  assert.equal(receipt.stats.reservedPages, expected.memory.reservedPages);
  assert.equal(receipt.stats.committedPages, expected.memory.reflectedPages);
  assert.equal(receipt.stats.physicalPages, expected.memory.reflectedPages);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.generation, expected.memory.generation);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  assert.equal(receipt.iatValue, expected.imports.first.address);
  assert.deepEqual(
    receipt.entryBytes,
    [...context.readVirtualBytes(expected.image.entry, 16, "entry receipt bytes")],
  );
  assert.deepEqual(receipt.thunkBytes, [
    0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
    0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
  ]);
  assert.deepEqual(receipt.callbackReturnBytes, receipt.thunkBytes);
  assert.deepEqual(receipt.processEntryReturnBytes, receipt.thunkBytes);
  if (dispatchGetProcAddress) {
    assert.deepEqual(receipt.dynamicExportBytes, receipt.thunkBytes);
    assert.deepEqual(
      receipt.dynamicExportBytes,
      [...context.readVirtualBytes(expected.dynamicExport.address, 16, "dynamic export bytes")],
    );
  } else {
    assert.equal(receipt.dynamicExportBytes, undefined);
  }
  if (dispatchGetProcAddressSecond) {
    const expectedPrefix = [
      0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
      0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
      0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
      0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
    ];
    if (dispatchGetProcAddressThird) {
      expectedPrefix.push(
        0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
        0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
      );
    }
    if (dispatchGetProcAddressFourth) {
      expectedPrefix.push(
        0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
        0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
      );
    }
    assert.deepEqual(receipt.dynamicExportPagePrefixBytes, expectedPrefix);
  } else {
    assert.equal(receipt.dynamicExportPagePrefixBytes, undefined);
  }
  assert.deepEqual(receipt.initialStackTopBytes, [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10, 0x00, 0x01, 0x7f,
  ]);
  assert.deepEqual(
    receipt.tebBytes,
    [
      ...context.readVirtualBytes(
        expected.processEnvironment.tebBase,
        64,
        "TEB receipt bytes",
      ),
    ],
  );
  assert.deepEqual(
    receipt.pebBytes,
    [
      ...context.readVirtualBytes(
        expected.processEnvironment.pebBase,
        64,
        "PEB receipt bytes",
      ),
    ],
  );
  assert.deepEqual(
    receipt.processParametersBytes,
    [
      ...context.readVirtualBytes(
        expected.processEnvironment.processParametersBase,
        0x48,
        "process parameters receipt bytes",
      ),
    ],
  );
  const expectedCommandLineBytes = Buffer.concat([
    Buffer.from(expected.processEnvironment.commandLine, "utf16le"),
    Buffer.from([0, 0]),
  ]);
  assert.deepEqual(receipt.commandLineBytes, [...expectedCommandLineBytes]);
  assert.equal(receipt.stop.stage, "adapter-execution");
  assert.equal(receipt.stop.code, "entry-to-first-owned-thunk-pending");
  assert.equal(offer.reflection.invalidation.generation, receipt.acknowledgedGeneration);
  assert.equal(receiptLineBytes, expected.receiptLineBytes);
}

async function dispatchFirstSystemTimeCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    8,
    "read",
    "GetSystemTimeAsFileTime stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "GetSystemTimeAsFileTime return address",
  );
  const outputAddress = context.readVirtualU32(
    trapped.esp + 4,
    "GetSystemTimeAsFileTime output pointer",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(outputAddress, execution.terminal.stackWords[1]);

  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "GetSystemTimeAsFileTime indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.terminal.thunk.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.terminal.thunk.iatAddress,
      "GetSystemTimeAsFileTime bound IAT",
    ),
    expected.terminal.thunk.address,
  );

  const outputPages = context.preflightVirtualRange(
    outputAddress,
    8,
    "write",
    "GetSystemTimeAsFileTime FILETIME output",
  );
  assert.equal(outputPages.length, 1);
  assert.equal(outputPages[0].owner, expected.stack.owner);
  assert.equal(outputPages[0].protection.execute, false);
  const stackPageAddress = (outputAddress & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const outputOffset = outputAddress - stackPageAddress;
  const outputBefore = context.readVirtualBytes(
    outputAddress,
    8,
    "FILETIME output before host write",
  );
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page writeback",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const fileTimeBytes = Buffer.alloc(8);
  fileTimeBytes.writeBigUInt64LE(controlledClock.fileTimeTicks);
  assert.equal(Number(controlledClock.fileTimeTicks & 0xffff_ffffn), controlledClock.fileTimeLow);
  assert.equal(
    Number((controlledClock.fileTimeTicks >> 32n) & 0xffff_ffffn),
    controlledClock.fileTimeHigh,
  );

  const request = {
    schema: "vine.v86-system-time-write-request",
    schemaVersion: 1,
    reflectedGeneration: offer.reflection.invalidation.generation,
    thunkId: expected.terminal.thunk.id,
    thunkAddress: expected.terminal.thunk.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes: 4,
    outputAddress,
    stackPageAddress,
    stackPageOwner: outputPages[0].owner,
    stackPageBytes: [...stackPageBytes],
    clockOwner: controlledClock.owner,
    clockSampleId: controlledClock.sampleId,
    clockUtcIso: controlledClock.utcIso,
    fileTimeLow: controlledClock.fileTimeLow,
    fileTimeHigh: controlledClock.fileTimeHigh,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  assert.equal(Buffer.byteLength(requestLine) - 1, expected.firstCall.requestLineBytes);
  child.stdin.write(requestLine);

  const deltaMessage = await nextMessage("GetSystemTimeAsFileTime memory delta");
  const delta = deltaMessage.message;
  assert.equal(delta.schema, "vine.v86-system-time-delta");
  assert.equal(delta.schemaVersion, 1);
  validateThunk(delta.thunk, expected.terminal.thunk, false);
  assert.equal(delta.callingConvention, "stdcall");
  assert.equal(delta.argumentBytes, 4);
  assert.equal(delta.stackPointer, trapped.esp);
  assert.equal(delta.returnAddress, returnAddress);
  assert.equal(delta.outputAddress, outputAddress);
  assert.equal(delta.clockOwner, controlledClock.owner);
  assert.equal(delta.clockSampleId, controlledClock.sampleId);
  assert.equal(delta.clockUtcIso, controlledClock.utcIso);
  assert.equal(delta.fileTimeTicks, String(controlledClock.fileTimeTicks));
  assert.equal(delta.fileTimeLow, controlledClock.fileTimeLow);
  assert.equal(delta.fileTimeHigh, controlledClock.fileTimeHigh);
  assert.deepEqual(delta.stackReconciliation, {
    generation: offer.reflection.invalidation.generation,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: delta.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(delta.stackReconciliation.changedBytes > 0);
  assert.equal(
    delta.stackReconciliation.changedBytes,
    expected.firstCall.reconciledChangedBytes,
  );
  assert.equal(
    delta.stackReconciliation.contentSha256,
    expected.firstCall.stackPageContentSha256,
  );
  assert.equal(delta.write.bytesWritten, 8);
  assert.equal(delta.write.dirtyPages, 1);
  assert.deepEqual(delta.write.invalidation, {
    generation: expected.firstCall.deltaGeneration,
    dataTlb: false,
    instructionFetch: false,
    generatedCode: false,
  });
  assert.equal(delta.delta.fromGeneration, offer.reflection.invalidation.generation);
  assert.equal(delta.delta.generation, expected.firstCall.deltaGeneration);
  assert.equal(delta.delta.pageSize, page.size);
  assert.deepEqual(delta.delta.mappingOperations, []);
  assert.equal(delta.delta.dirtyPages.length, 1);
  const dirty = delta.delta.dirtyPages[0];
  assert.equal(dirty.address, stackPageAddress);
  assert.equal(dirty.owner, expected.stack.owner);
  assert.equal(dirty.ownerSealed, true);
  assert.deepEqual(dirty.protection, { read: true, write: true, execute: false });
  assert.equal(dirty.bytes.length, page.size);
  const expectedStackPage = Uint8Array.from(stackPageBytes);
  expectedStackPage.set(fileTimeBytes, outputOffset);
  assert.deepEqual(dirty.bytes, [...expectedStackPage]);
  assert.equal(delta.delta.transferredBytes, page.size);
  assert.deepEqual(delta.delta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });
  assert.match(delta.delta.contentSha256, /^[0-9a-f]{64}$/);
  if (expected.firstCall.deltaContentSha256 !== null) {
    assert.equal(delta.delta.contentSha256, expected.firstCall.deltaContentSha256);
  }
  if (expected.firstCall.deltaLineBytes !== null) {
    assert.equal(deltaMessage.lineBytes, expected.firstCall.deltaLineBytes);
  }

  context.applyDirtyPage(dirty, "GetSystemTimeAsFileTime stack delta");
  assert.deepEqual(
    [...context.readVirtualBytes(outputAddress, 8, "applied FILETIME output")],
    [...fileTimeBytes],
  );
  const deltaAcknowledgement = `${JSON.stringify({
    schema: "vine.v86-memory-delta-ack",
    schemaVersion: 1,
    generation: delta.delta.generation,
    dataTlb: false,
    instructionFetch: false,
    generatedCode: false,
  })}\n`;
  if (continueSession) {
    child.stdin.write(deltaAcknowledgement);
  } else {
    child.stdin.end(deltaAcknowledgement);
  }
  const receiptMessage = await nextMessage("GetSystemTimeAsFileTime delta receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-system-time-delta-receipt");
  assert.equal(receipt.schemaVersion, 1);
  assert.equal(receipt.acknowledgedGeneration, delta.delta.generation);
  assert.equal(receipt.outputAddress, outputAddress);
  assert.deepEqual(receipt.outputBytes, [...fileTimeBytes]);
  assert.equal(receipt.stats.generation, delta.delta.generation);
  assert.equal(receipt.stats.acknowledgedGeneration, delta.delta.generation);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.firstCall.deltaReceiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.firstCall.deltaReceiptLineBytes);
  }

  const resume = resumeVoidStdcall(
    context,
    trapped,
    returnAddress,
    4,
    "GetSystemTimeAsFileTime",
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.firstCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.firstCall.nextThunk.identity,
      !dispatchProcessId,
    );
    assert.equal(continuation.instructionCount, expected.firstCall.instructionCount);
    assert.equal(continuation.executeStops, expected.firstCall.executeStops);
    assert.equal(continuation.budgetStops, expected.firstCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.firstCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.firstCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      outputAddress: request.outputAddress,
      outputBefore: [...outputBefore],
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    clock: {
      owner: controlledClock.owner,
      sampleId: controlledClock.sampleId,
      utcIso: controlledClock.utcIso,
      fileTimeTicks: String(controlledClock.fileTimeTicks),
      fileTimeLow: controlledClock.fileTimeLow,
      fileTimeHigh: controlledClock.fileTimeHigh,
      bytes: [...fileTimeBytes],
    },
    delta: {
      lineBytes: deltaMessage.lineBytes,
      callingConvention: delta.callingConvention,
      argumentBytes: delta.argumentBytes,
      stackReconciliation: delta.stackReconciliation,
      write: delta.write,
      delta: {
        fromGeneration: delta.delta.fromGeneration,
        generation: delta.delta.generation,
        pageSize: delta.delta.pageSize,
        mappingOperations: delta.delta.mappingOperations.length,
        dirtyPages: delta.delta.dirtyPages.length,
        dirtyPageAddress: dirty.address,
        dirtyPageOwner: dirty.owner,
        transferredBytes: delta.delta.transferredBytes,
        cacheEffects: delta.delta.cacheEffects,
        contentSha256: delta.delta.contentSha256,
      },
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchSecondProcessIdCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4,
    "read",
    "GetCurrentProcessId stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "GetCurrentProcessId return address",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "GetCurrentProcessId indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.firstCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.firstCall.nextThunk.identity.iatAddress,
      "GetCurrentProcessId bound IAT",
    ),
    expected.firstCall.nextThunk.identity.address,
  );

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before process-ID return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-process-id-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.firstCall.deltaGeneration,
    thunkId: expected.firstCall.nextThunk.identity.id,
    thunkAddress: expected.firstCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes: 0,
    stackPageAddress,
    stackPageOwner: framePages[0].owner,
    stackPageBytes: [...stackPageBytes],
    processIdentityOwner: controlledProcess.owner,
    processId: controlledProcess.processId,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.secondCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.secondCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("GetCurrentProcessId return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-process-id-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.firstCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, 0);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.processIdentityOwner, controlledProcess.owner);
  assert.equal(receipt.processId, controlledProcess.processId);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.firstCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  if (expected.secondCall.reconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.secondCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.secondCall.stackPageContentSha256);
  }
  assert.equal(receipt.stats.generation, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.secondCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.secondCall.receiptLineBytes);
  }

  const resume = resumeStdcallResult(
    context,
    trapped,
    returnAddress,
    0,
    controlledProcess.processId,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  if (expected.secondCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.secondCall.nextThunk.identity,
      !dispatchThreadId,
    );
    assert.equal(continuation.instructionCount, expected.secondCall.instructionCount);
    assert.equal(continuation.executeStops, expected.secondCall.executeStops);
    assert.equal(continuation.budgetStops, expected.secondCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.secondCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.secondCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    identity: {
      owner: controlledProcess.owner,
      processId: controlledProcess.processId,
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchThirdThreadIdCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4,
    "read",
    "GetCurrentThreadId stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "GetCurrentThreadId return address",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "GetCurrentThreadId indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.secondCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.secondCall.nextThunk.identity.iatAddress,
      "GetCurrentThreadId bound IAT",
    ),
    expected.secondCall.nextThunk.identity.address,
  );

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before thread-ID return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-thread-id-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.firstCall.deltaGeneration,
    thunkId: expected.secondCall.nextThunk.identity.id,
    thunkAddress: expected.secondCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes: 0,
    stackPageAddress,
    stackPageOwner: framePages[0].owner,
    stackPageBytes: [...stackPageBytes],
    threadIdentityOwner: controlledThread.owner,
    threadId: controlledThread.threadId,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.thirdCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.thirdCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("GetCurrentThreadId return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-thread-id-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.secondCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, 0);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.threadIdentityOwner, controlledThread.owner);
  assert.equal(receipt.threadId, controlledThread.threadId);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.firstCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  if (expected.thirdCall.reconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.thirdCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.thirdCall.stackPageContentSha256);
  }
  assert.equal(receipt.stats.generation, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.thirdCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.thirdCall.receiptLineBytes);
  }

  const resume = resumeThreadIdStdcallResult(
    context,
    trapped,
    returnAddress,
    controlledThread.threadId,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.thirdCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.thirdCall.nextThunk.identity,
      !dispatchTickCount,
    );
    assert.equal(continuation.instructionCount, expected.thirdCall.instructionCount);
    assert.equal(continuation.executeStops, expected.thirdCall.executeStops);
    assert.equal(continuation.budgetStops, expected.thirdCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.thirdCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.thirdCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    identity: {
      owner: controlledThread.owner,
      threadId: controlledThread.threadId,
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchFourthTickCountCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4,
    "read",
    "GetTickCount stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(trapped.esp, "GetTickCount return address");
  assert.equal(returnAddress, execution.terminal.returnAddress);
  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "GetTickCount indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.thirdCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.thirdCall.nextThunk.identity.iatAddress,
      "GetTickCount bound IAT",
    ),
    expected.thirdCall.nextThunk.identity.address,
  );

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before tick-count return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-tick-count-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.firstCall.deltaGeneration,
    thunkId: expected.thirdCall.nextThunk.identity.id,
    thunkAddress: expected.thirdCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes: 0,
    stackPageAddress,
    stackPageOwner: framePages[0].owner,
    stackPageBytes: [...stackPageBytes],
    monotonicClockOwner: controlledMonotonicClock.owner,
    sampleId: controlledMonotonicClock.sampleId,
    pairedWallClockSampleId: controlledMonotonicClock.pairedWallClockSampleId,
    tickCount: controlledMonotonicClock.tickCount,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.fourthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.fourthCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("GetTickCount return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-tick-count-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.thirdCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, 0);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.monotonicClockOwner, controlledMonotonicClock.owner);
  assert.equal(receipt.sampleId, controlledMonotonicClock.sampleId);
  assert.equal(
    receipt.pairedWallClockSampleId,
    controlledMonotonicClock.pairedWallClockSampleId,
  );
  assert.equal(receipt.tickCount, controlledMonotonicClock.tickCount);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.firstCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  if (expected.fourthCall.reconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.fourthCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.fourthCall.stackPageContentSha256);
  }
  assert.equal(receipt.stats.generation, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.fourthCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.fourthCall.receiptLineBytes);
  }

  const resume = resumeTickCountStdcallResult(
    context,
    trapped,
    returnAddress,
    controlledMonotonicClock.tickCount,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.fourthCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.fourthCall.nextThunk.identity,
      !dispatchPerformanceCounter,
    );
    assert.equal(continuation.instructionCount, expected.fourthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.fourthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.fourthCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.fourthCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.fourthCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    clock: {
      owner: controlledMonotonicClock.owner,
      sampleId: controlledMonotonicClock.sampleId,
      pairedWallClockSampleId: controlledMonotonicClock.pairedWallClockSampleId,
      tickCount: controlledMonotonicClock.tickCount,
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchFifthPerformanceCounterCall({
  child,
  continueSession,
  context,
  execution,
  nextMessage,
  offer,
}) {
  const trapped = execution.finalState;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    8,
    "read",
    "QueryPerformanceCounter stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "QueryPerformanceCounter return address",
  );
  const outputAddress = context.readVirtualU32(
    trapped.esp + 4,
    "QueryPerformanceCounter output pointer",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(outputAddress, execution.terminal.stackWords[1]);

  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "QueryPerformanceCounter indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.fourthCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.fourthCall.nextThunk.identity.iatAddress,
      "QueryPerformanceCounter bound IAT",
    ),
    expected.fourthCall.nextThunk.identity.address,
  );

  const outputPages = context.preflightVirtualRange(
    outputAddress,
    8,
    "write",
    "QueryPerformanceCounter LARGE_INTEGER output",
  );
  assert.equal(outputPages.length, 1);
  assert.equal(outputPages[0].owner, expected.stack.owner);
  assert.equal(outputPages[0].protection.execute, false);
  const stackPageAddress = (outputAddress & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const outputOffset = outputAddress - stackPageAddress;
  const outputBefore = context.readVirtualBytes(
    outputAddress,
    8,
    "performance-counter output before host write",
  );
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before performance-counter write",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const counterBytes = Buffer.alloc(8);
  counterBytes.writeBigUInt64LE(controlledPerformanceCounter.ticks);
  assert.equal(
    Number(controlledPerformanceCounter.ticks & 0xffff_ffffn),
    controlledPerformanceCounter.low,
  );
  assert.equal(
    Number((controlledPerformanceCounter.ticks >> 32n) & 0xffff_ffffn),
    controlledPerformanceCounter.high,
  );

  const request = {
    schema: "vine.v86-performance-counter-write-request",
    schemaVersion: 1,
    reflectedGeneration: expected.firstCall.deltaGeneration,
    thunkId: expected.fourthCall.nextThunk.identity.id,
    thunkAddress: expected.fourthCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes: 4,
    outputAddress,
    stackPageAddress,
    stackPageOwner: outputPages[0].owner,
    stackPageBytes: [...stackPageBytes],
    monotonicClockOwner: controlledPerformanceCounter.owner,
    sampleId: controlledPerformanceCounter.sampleId,
    pairedTickSampleId: controlledPerformanceCounter.pairedTickSampleId,
    counterLow: controlledPerformanceCounter.low,
    counterHigh: controlledPerformanceCounter.high,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.fifthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.fifthCall.requestLineBytes);
  }
  child.stdin.write(requestLine);

  const deltaMessage = await nextMessage("QueryPerformanceCounter memory delta");
  const delta = deltaMessage.message;
  assert.equal(delta.schema, "vine.v86-performance-counter-delta");
  assert.equal(delta.schemaVersion, 1);
  validateThunk(delta.thunk, expected.fourthCall.nextThunk.identity, false);
  assert.equal(delta.callingConvention, "stdcall");
  assert.equal(delta.argumentBytes, 4);
  assert.equal(delta.stackPointer, trapped.esp);
  assert.equal(delta.returnAddress, returnAddress);
  assert.equal(delta.outputAddress, outputAddress);
  assert.equal(delta.monotonicClockOwner, controlledPerformanceCounter.owner);
  assert.equal(delta.sampleId, controlledPerformanceCounter.sampleId);
  assert.equal(delta.pairedTickSampleId, controlledPerformanceCounter.pairedTickSampleId);
  assert.equal(delta.counterTicks, String(controlledPerformanceCounter.ticks));
  assert.equal(delta.counterLow, controlledPerformanceCounter.low);
  assert.equal(delta.counterHigh, controlledPerformanceCounter.high);
  assert.deepEqual(delta.stackReconciliation, {
    generation: expected.firstCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: delta.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(delta.stackReconciliation.changedBytes > 0);
  if (expected.fifthCall.reconciledChangedBytes !== null) {
    assert.equal(
      delta.stackReconciliation.changedBytes,
      expected.fifthCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.fifthCall.stackPageContentSha256);
  }
  assert.equal(delta.write.bytesWritten, 8);
  assert.equal(delta.write.dirtyPages, 1);
  assert.deepEqual(delta.write.invalidation, {
    generation: expected.fifthCall.deltaGeneration,
    dataTlb: false,
    instructionFetch: false,
    generatedCode: false,
  });
  assert.equal(delta.delta.fromGeneration, expected.firstCall.deltaGeneration);
  assert.equal(delta.delta.generation, expected.fifthCall.deltaGeneration);
  assert.equal(delta.delta.pageSize, page.size);
  assert.deepEqual(delta.delta.mappingOperations, []);
  assert.equal(delta.delta.dirtyPages.length, 1);
  const dirty = delta.delta.dirtyPages[0];
  assert.equal(dirty.address, stackPageAddress);
  assert.equal(dirty.owner, expected.stack.owner);
  assert.equal(dirty.ownerSealed, true);
  assert.deepEqual(dirty.protection, { read: true, write: true, execute: false });
  assert.equal(dirty.bytes.length, page.size);
  const expectedStackPage = Uint8Array.from(stackPageBytes);
  expectedStackPage.set(counterBytes, outputOffset);
  assert.deepEqual(dirty.bytes, [...expectedStackPage]);
  assert.equal(delta.delta.transferredBytes, page.size);
  assert.deepEqual(delta.delta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });
  assert.match(delta.delta.contentSha256, /^[0-9a-f]{64}$/);
  if (expected.fifthCall.deltaContentSha256 !== null) {
    assert.equal(delta.delta.contentSha256, expected.fifthCall.deltaContentSha256);
  }
  if (expected.fifthCall.deltaLineBytes !== null) {
    assert.equal(deltaMessage.lineBytes, expected.fifthCall.deltaLineBytes);
  }

  context.applyDirtyPage(dirty, "QueryPerformanceCounter stack delta");
  assert.deepEqual(
    [...context.readVirtualBytes(outputAddress, 8, "applied performance-counter output")],
    [...counterBytes],
  );
  const deltaAcknowledgement = `${JSON.stringify({
    schema: "vine.v86-memory-delta-ack",
    schemaVersion: 1,
    generation: delta.delta.generation,
    dataTlb: false,
    instructionFetch: false,
    generatedCode: false,
  })}\n`;
  if (continueSession) {
    child.stdin.write(deltaAcknowledgement);
  } else {
    child.stdin.end(deltaAcknowledgement);
  }

  const receiptMessage = await nextMessage("QueryPerformanceCounter delta receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-performance-counter-delta-receipt");
  assert.equal(receipt.schemaVersion, 1);
  assert.equal(receipt.acknowledgedGeneration, delta.delta.generation);
  assert.equal(receipt.outputAddress, outputAddress);
  assert.deepEqual(receipt.outputBytes, [...counterBytes]);
  assert.equal(receipt.stats.generation, delta.delta.generation);
  assert.equal(receipt.stats.acknowledgedGeneration, delta.delta.generation);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.fifthCall.deltaReceiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.fifthCall.deltaReceiptLineBytes);
  }

  const resume = resumePerformanceCounterStdcallResult(
    context,
    trapped,
    returnAddress,
    1,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.fifthCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.fifthCall.nextThunk.identity,
      expected.fifthCall.nextThunk.unsupported,
    );
    assert.equal(continuation.instructionCount, expected.fifthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.fifthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.fifthCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.fifthCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.fifthCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      outputAddress: request.outputAddress,
      outputBefore: [...outputBefore],
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    clock: {
      owner: controlledPerformanceCounter.owner,
      sampleId: controlledPerformanceCounter.sampleId,
      pairedTickSampleId: controlledPerformanceCounter.pairedTickSampleId,
      ticks: String(controlledPerformanceCounter.ticks),
      low: controlledPerformanceCounter.low,
      high: controlledPerformanceCounter.high,
      bytes: [...counterBytes],
    },
    delta: {
      lineBytes: deltaMessage.lineBytes,
      callingConvention: delta.callingConvention,
      argumentBytes: delta.argumentBytes,
      stackReconciliation: delta.stackReconciliation,
      write: delta.write,
      delta: {
        fromGeneration: delta.delta.fromGeneration,
        generation: delta.delta.generation,
        pageSize: delta.delta.pageSize,
        mappingOperations: delta.delta.mappingOperations.length,
        dirtyPages: delta.delta.dirtyPages.length,
        dirtyPageAddress: dirty.address,
        dirtyPageOwner: dirty.owner,
        transferredBytes: delta.delta.transferredBytes,
        cacheEffects: delta.delta.cacheEffects,
        contentSha256: delta.delta.contentSha256,
      },
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

function encodeX86StartupInfoW(fields) {
  const bytes = Buffer.alloc(controlledProcessStartup.structureSize);
  bytes.writeUInt32LE(fields.cb, 0);
  bytes.writeUInt32LE(fields.lpReserved, 4);
  bytes.writeUInt32LE(fields.lpDesktop, 8);
  bytes.writeUInt32LE(fields.lpTitle, 12);
  bytes.writeUInt32LE(fields.dwX, 16);
  bytes.writeUInt32LE(fields.dwY, 20);
  bytes.writeUInt32LE(fields.dwXSize, 24);
  bytes.writeUInt32LE(fields.dwYSize, 28);
  bytes.writeUInt32LE(fields.dwXCountChars, 32);
  bytes.writeUInt32LE(fields.dwYCountChars, 36);
  bytes.writeUInt32LE(fields.dwFillAttribute, 40);
  bytes.writeUInt32LE(fields.dwFlags, 44);
  bytes.writeUInt16LE(fields.wShowWindow, 48);
  bytes.writeUInt16LE(fields.cbReserved2, 50);
  bytes.writeUInt32LE(fields.lpReserved2, 52);
  bytes.writeUInt32LE(fields.hStdInput, 56);
  bytes.writeUInt32LE(fields.hStdOutput, 60);
  bytes.writeUInt32LE(fields.hStdError, 64);
  return bytes;
}

async function dispatchSixthStartupInfoCall({
  child,
  continueSession,
  context,
  execution,
  nextMessage,
  offer,
}) {
  const trapped = execution.finalState;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    8,
    "read",
    "GetStartupInfoW stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "GetStartupInfoW return address",
  );
  const outputAddress = context.readVirtualU32(
    trapped.esp + 4,
    "GetStartupInfoW output pointer",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(outputAddress, execution.terminal.stackWords[1]);

  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "GetStartupInfoW indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.fifthCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.fifthCall.nextThunk.identity.iatAddress,
      "GetStartupInfoW bound IAT",
    ),
    expected.fifthCall.nextThunk.identity.address,
  );

  const outputPages = context.preflightVirtualRange(
    outputAddress,
    controlledProcessStartup.structureSize,
    "write",
    "GetStartupInfoW STARTUPINFOW output",
  );
  assert.equal(outputPages.length, 1);
  assert.equal(outputPages[0].owner, expected.stack.owner);
  assert.equal(outputPages[0].protection.execute, false);
  const stackPageAddress = (outputAddress & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const outputOffset = outputAddress - stackPageAddress;
  const outputBefore = context.readVirtualBytes(
    outputAddress,
    controlledProcessStartup.structureSize,
    "STARTUPINFOW output before controlled write",
  );
  assert.equal(sha256(outputBefore), expected.sixthCall.outputBeforeSha256);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before STARTUPINFOW write",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const startupInfoBytes = encodeX86StartupInfoW(controlledProcessStartup.fields);
  assert.equal(startupInfoBytes.length, controlledProcessStartup.structureSize);
  assert.equal(sha256(startupInfoBytes), expected.sixthCall.outputBytesSha256);

  const request = {
    schema: "vine.v86-startup-info-write-request",
    schemaVersion: 1,
    reflectedGeneration: expected.fifthCall.deltaGeneration,
    thunkId: expected.fifthCall.nextThunk.identity.id,
    thunkAddress: expected.fifthCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes: 4,
    outputAddress,
    stackPageAddress,
    stackPageOwner: outputPages[0].owner,
    stackPageBytes: [...stackPageBytes],
    processStartupOwner: controlledProcessStartup.owner,
    profileId: controlledProcessStartup.profileId,
    structureSize: controlledProcessStartup.structureSize,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.sixthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.sixthCall.requestLineBytes);
  }
  child.stdin.write(requestLine);

  const deltaMessage = await nextMessage("GetStartupInfoW memory delta");
  const delta = deltaMessage.message;
  assert.equal(delta.schema, "vine.v86-startup-info-delta");
  assert.equal(delta.schemaVersion, 1);
  validateThunk(delta.thunk, expected.fifthCall.nextThunk.identity, false);
  assert.equal(delta.callingConvention, "stdcall");
  assert.equal(delta.argumentBytes, 4);
  assert.equal(delta.stackPointer, trapped.esp);
  assert.equal(delta.returnAddress, returnAddress);
  assert.equal(delta.outputAddress, outputAddress);
  assert.equal(delta.processStartupOwner, controlledProcessStartup.owner);
  assert.equal(delta.profileId, controlledProcessStartup.profileId);
  assert.equal(delta.structureSize, controlledProcessStartup.structureSize);
  assert.deepEqual(delta.startupInfo, controlledProcessStartup.fields);
  assert.deepEqual(delta.outputBytes, [...startupInfoBytes]);
  assert.deepEqual(delta.stackReconciliation, {
    generation: expected.fifthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: delta.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(delta.stackReconciliation.changedBytes > 0);
  if (expected.sixthCall.reconciledChangedBytes !== null) {
    assert.equal(
      delta.stackReconciliation.changedBytes,
      expected.sixthCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.sixthCall.stackPageContentSha256);
  }
  assert.equal(delta.write.bytesWritten, controlledProcessStartup.structureSize);
  assert.equal(delta.write.dirtyPages, 1);
  assert.deepEqual(delta.write.invalidation, {
    generation: expected.sixthCall.deltaGeneration,
    dataTlb: false,
    instructionFetch: false,
    generatedCode: false,
  });
  assert.equal(delta.delta.fromGeneration, expected.fifthCall.deltaGeneration);
  assert.equal(delta.delta.generation, expected.sixthCall.deltaGeneration);
  assert.equal(delta.delta.pageSize, page.size);
  assert.deepEqual(delta.delta.mappingOperations, []);
  assert.equal(delta.delta.dirtyPages.length, 1);
  const dirty = delta.delta.dirtyPages[0];
  assert.equal(dirty.address, stackPageAddress);
  assert.equal(dirty.owner, expected.stack.owner);
  assert.equal(dirty.ownerSealed, true);
  assert.deepEqual(dirty.protection, { read: true, write: true, execute: false });
  assert.equal(dirty.bytes.length, page.size);
  const expectedStackPage = Uint8Array.from(stackPageBytes);
  expectedStackPage.set(startupInfoBytes, outputOffset);
  assert.deepEqual(dirty.bytes, [...expectedStackPage]);
  assert.equal(delta.delta.transferredBytes, page.size);
  assert.deepEqual(delta.delta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });
  assert.match(delta.delta.contentSha256, /^[0-9a-f]{64}$/);
  if (expected.sixthCall.deltaContentSha256 !== null) {
    assert.equal(delta.delta.contentSha256, expected.sixthCall.deltaContentSha256);
  }
  if (expected.sixthCall.deltaLineBytes !== null) {
    assert.equal(deltaMessage.lineBytes, expected.sixthCall.deltaLineBytes);
  }

  context.applyDirtyPage(dirty, "GetStartupInfoW stack delta");
  assert.deepEqual(
    [
      ...context.readVirtualBytes(
        outputAddress,
        controlledProcessStartup.structureSize,
        "applied STARTUPINFOW output",
      ),
    ],
    [...startupInfoBytes],
  );
  const deltaAcknowledgement = `${JSON.stringify({
    schema: "vine.v86-memory-delta-ack",
    schemaVersion: 1,
    generation: delta.delta.generation,
    dataTlb: false,
    instructionFetch: false,
    generatedCode: false,
  })}\n`;
  if (continueSession) {
    child.stdin.write(deltaAcknowledgement);
  } else {
    child.stdin.end(deltaAcknowledgement);
  }

  const receiptMessage = await nextMessage("GetStartupInfoW delta receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-startup-info-delta-receipt");
  assert.equal(receipt.schemaVersion, 1);
  assert.equal(receipt.acknowledgedGeneration, delta.delta.generation);
  assert.equal(receipt.outputAddress, outputAddress);
  assert.deepEqual(receipt.outputBytes, [...startupInfoBytes]);
  assert.equal(receipt.stats.generation, delta.delta.generation);
  assert.equal(receipt.stats.acknowledgedGeneration, delta.delta.generation);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.sixthCall.deltaReceiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.sixthCall.deltaReceiptLineBytes);
  }

  const resume = resumeVoidStdcall(
    context,
    trapped,
    returnAddress,
    4,
    "GetStartupInfoW",
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.sixthCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.sixthCall.nextThunk.identity,
      expected.sixthCall.nextThunk.unsupported,
    );
    assert.equal(continuation.instructionCount, expected.sixthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.sixthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.sixthCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.sixthCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.sixthCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      outputAddress: request.outputAddress,
      outputBefore: [...outputBefore],
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    profile: {
      owner: controlledProcessStartup.owner,
      profileId: controlledProcessStartup.profileId,
      structureSize: controlledProcessStartup.structureSize,
      fields: controlledProcessStartup.fields,
      bytes: [...startupInfoBytes],
      nativeObservation: controlledProcessStartup.nativeObservation,
    },
    delta: {
      lineBytes: deltaMessage.lineBytes,
      callingConvention: delta.callingConvention,
      argumentBytes: delta.argumentBytes,
      stackReconciliation: delta.stackReconciliation,
      write: delta.write,
      delta: {
        fromGeneration: delta.delta.fromGeneration,
        generation: delta.delta.generation,
        pageSize: delta.delta.pageSize,
        mappingOperations: delta.delta.mappingOperations.length,
        dirtyPages: delta.delta.dirtyPages.length,
        dirtyPageAddress: dirty.address,
        dirtyPageOwner: dirty.owner,
        transferredBytes: delta.delta.transferredBytes,
        cacheEffects: delta.delta.cacheEffects,
        contentSha256: delta.delta.contentSha256,
      },
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchSeventhHeapSetInformationCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const argumentBytes = 16;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4 + argumentBytes,
    "read",
    "HeapSetInformation stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "HeapSetInformation return address",
  );
  const arguments_ = Array.from({ length: 4 }, (_, index) =>
    context.readVirtualU32(
      trapped.esp + 4 + index * 4,
      `HeapSetInformation argument ${index}`,
    ),
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.deepEqual(arguments_, [
    controlledHeapPolicy.heapHandle,
    controlledHeapPolicy.informationClass,
    controlledHeapPolicy.informationAddress,
    controlledHeapPolicy.informationLength,
  ]);

  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "HeapSetInformation indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.sixthCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.sixthCall.nextThunk.identity.iatAddress,
      "HeapSetInformation bound IAT",
    ),
    expected.sixthCall.nextThunk.identity.address,
  );

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before HeapSetInformation return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-heap-set-information-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.sixthCall.deltaGeneration,
    thunkId: expected.sixthCall.nextThunk.identity.id,
    thunkAddress: expected.sixthCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes,
    stackPageAddress,
    stackPageOwner: expected.stack.owner,
    stackPageBytes: [...stackPageBytes],
    heapPolicyOwner: controlledHeapPolicy.owner,
    policyId: controlledHeapPolicy.policyId,
    heapHandle: controlledHeapPolicy.heapHandle,
    informationClass: controlledHeapPolicy.informationClass,
    informationAddress: controlledHeapPolicy.informationAddress,
    informationLength: controlledHeapPolicy.informationLength,
    preserveLastError: controlledHeapPolicy.preserveLastError,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.seventhCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.seventhCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("HeapSetInformation return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-heap-set-information-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.sixthCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, argumentBytes);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.heapPolicyOwner, controlledHeapPolicy.owner);
  assert.equal(receipt.policyId, controlledHeapPolicy.policyId);
  assert.equal(receipt.heapHandle, controlledHeapPolicy.heapHandle);
  assert.equal(receipt.informationClass, controlledHeapPolicy.informationClass);
  assert.equal(receipt.informationAddress, controlledHeapPolicy.informationAddress);
  assert.equal(receipt.informationLength, controlledHeapPolicy.informationLength);
  assert.equal(
    receipt.terminationOnCorruptionEnabled,
    controlledHeapPolicy.terminationOnCorruptionEnabled,
  );
  assert.equal(receipt.preserveLastError, controlledHeapPolicy.preserveLastError);
  assert.equal(receipt.result, controlledHeapPolicy.result);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  if (expected.seventhCall.reconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.seventhCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.seventhCall.stackPageContentSha256);
  }
  assert.equal(receipt.stats.generation, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.seventhCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.seventhCall.receiptLineBytes);
  }

  const resume = resumeHeapSetInformationStdcallResult(
    context,
    trapped,
    returnAddress,
    receipt.result,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.seventhCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.seventhCall.nextThunk.identity,
      expected.seventhCall.nextThunk.unsupported,
    );
    assert.equal(continuation.instructionCount, expected.seventhCall.instructionCount);
    assert.equal(continuation.executeStops, expected.seventhCall.executeStops);
    assert.equal(continuation.budgetStops, expected.seventhCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.seventhCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.seventhCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      arguments: arguments_,
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    policy: {
      owner: controlledHeapPolicy.owner,
      policyId: controlledHeapPolicy.policyId,
      heapHandle: controlledHeapPolicy.heapHandle,
      informationClass: controlledHeapPolicy.informationClass,
      informationAddress: controlledHeapPolicy.informationAddress,
      informationLength: controlledHeapPolicy.informationLength,
      terminationOnCorruptionEnabled:
        controlledHeapPolicy.terminationOnCorruptionEnabled,
      preserveLastError: controlledHeapPolicy.preserveLastError,
      nativeObservation: controlledHeapPolicy.nativeObservation,
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchEighthHeapCreateCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const argumentBytes = 12;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4 + argumentBytes,
    "read",
    "HeapCreate stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(trapped.esp, "HeapCreate return address");
  const arguments_ = Array.from({ length: 3 }, (_, index) =>
    context.readVirtualU32(
      trapped.esp + 4 + index * 4,
      `HeapCreate argument ${index}`,
    ),
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.deepEqual(arguments_, [
    controlledPrivateHeap.creationFlags,
    controlledPrivateHeap.initialSize,
    controlledPrivateHeap.maximumSize,
  ]);

  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(callSite, 6, "HeapCreate indirect IAT call");
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.seventhCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.seventhCall.nextThunk.identity.iatAddress,
      "HeapCreate bound IAT",
    ),
    expected.seventhCall.nextThunk.identity.address,
  );

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before HeapCreate return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-heap-create-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.sixthCall.deltaGeneration,
    thunkId: expected.seventhCall.nextThunk.identity.id,
    thunkAddress: expected.seventhCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes,
    stackPageAddress,
    stackPageOwner: expected.stack.owner,
    stackPageBytes: [...stackPageBytes],
    heapStateOwner: controlledPrivateHeap.owner,
    variantId: controlledPrivateHeap.variantId,
    creationFlags: controlledPrivateHeap.creationFlags,
    initialSize: controlledPrivateHeap.initialSize,
    maximumSize: controlledPrivateHeap.maximumSize,
    preserveLastError: controlledPrivateHeap.preserveLastError,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.eighthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.eighthCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("HeapCreate return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-heap-create-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.seventhCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, argumentBytes);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.deepEqual(receipt.logicalHeap, {
    owner: controlledPrivateHeap.owner,
    variantId: controlledPrivateHeap.variantId,
    handle: controlledPrivateHeap.handle,
    kind: controlledPrivateHeap.kind,
    lifecycle: controlledPrivateHeap.lifecycle,
    creationFlags: controlledPrivateHeap.creationFlags,
    initialSize: controlledPrivateHeap.initialSize,
    maximumSize: controlledPrivateHeap.maximumSize,
    growable: controlledPrivateHeap.growable,
    backingMaterialization: controlledPrivateHeap.backingMaterialization,
    liveAllocations: controlledPrivateHeap.liveAllocations,
  });
  assert.equal(receipt.preserveLastError, controlledPrivateHeap.preserveLastError);
  assert.equal(receipt.result, controlledPrivateHeap.handle.value);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  if (expected.eighthCall.reconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.eighthCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.eighthCall.stackPageContentSha256);
  }
  assert.equal(receipt.stats.generation, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.eighthCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.eighthCall.receiptLineBytes);
  }

  const resume = resumeHeapCreateStdcallResult(
    context,
    trapped,
    returnAddress,
    receipt.result,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.eighthCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.eighthCall.nextThunk.identity,
      expected.eighthCall.nextThunk.unsupported,
    );
    assert.equal(continuation.instructionCount, expected.eighthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.eighthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.eighthCall.budgetStops);
    assert.equal(continuation.terminal.returnAddress, expected.eighthCall.nextThunk.returnAddress);
    assert.deepEqual(continuation.terminal.stackWords, expected.eighthCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      arguments: arguments_,
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    heap: {
      ...receipt.logicalHeap,
      preserveLastError: receipt.preserveLastError,
      nativeObservation: controlledPrivateHeap.nativeObservation,
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchNinthGetModuleHandleWCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const argumentBytes = 4;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4 + argumentBytes,
    "read",
    "GetModuleHandleW stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "GetModuleHandleW return address",
  );
  const moduleNameAddress = context.readVirtualU32(
    trapped.esp + 4,
    "GetModuleHandleW module name address",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  const moduleNameByteLength = (controlledLoadedModule.requestedName.length + 1) * 2;
  const moduleNamePages = context.preflightVirtualRange(
    moduleNameAddress,
    moduleNameByteLength,
    "read",
    "GetModuleHandleW UTF-16 module name",
  );
  assert.equal(moduleNamePages.length, 1);
  assert.equal(moduleNamePages[0].owner, expected.image.owner);
  const moduleNameBytes = context.readVirtualBytes(
    moduleNameAddress,
    moduleNameByteLength,
    "GetModuleHandleW UTF-16 module name bytes",
  );
  assert.deepEqual([...moduleNameBytes.subarray(-2)], [0, 0]);
  const moduleName = Buffer.from(moduleNameBytes.subarray(0, -2)).toString("utf16le");
  assert.equal(moduleName, controlledLoadedModule.requestedName);
  const moduleNameBytesSha256 = sha256(moduleNameBytes);

  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "GetModuleHandleW indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.eighthCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.eighthCall.nextThunk.identity.iatAddress,
      "GetModuleHandleW bound IAT",
    ),
    expected.eighthCall.nextThunk.identity.address,
  );

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before GetModuleHandleW return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-get-module-handle-w-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.sixthCall.deltaGeneration,
    thunkId: expected.eighthCall.nextThunk.identity.id,
    thunkAddress: expected.eighthCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes,
    stackPageAddress,
    stackPageOwner: expected.stack.owner,
    stackPageBytes: [...stackPageBytes],
    loadedModuleOwner: controlledLoadedModule.owner,
    variantId: controlledLoadedModule.variantId,
    moduleNameAddress,
    moduleName,
    preserveLastError: controlledLoadedModule.preserveLastError,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.ninthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.ninthCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("GetModuleHandleW return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-get-module-handle-w-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.eighthCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, argumentBytes);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.moduleNameAddress, moduleNameAddress);
  assert.deepEqual(receipt.moduleNameUtf16Bytes, [...moduleNameBytes]);
  assert.deepEqual(receipt.logicalModule, {
    owner: controlledLoadedModule.owner,
    variantId: controlledLoadedModule.variantId,
    handle: controlledLoadedModule.handle,
    kind: controlledLoadedModule.kind,
    lifecycle: controlledLoadedModule.lifecycle,
    requestedName: controlledLoadedModule.requestedName,
    canonicalName: controlledLoadedModule.canonicalName,
    nameMatch: controlledLoadedModule.nameMatch,
    moduleRole: controlledLoadedModule.moduleRole,
    referenceCountChanged: controlledLoadedModule.referenceCountChanged,
    guestImageMaterialization: controlledLoadedModule.guestImageMaterialization,
    exportResolution: controlledLoadedModule.exportResolution,
  });
  assert.equal(receipt.preserveLastError, controlledLoadedModule.preserveLastError);
  assert.equal(receipt.result, controlledLoadedModule.handle.value);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  if (expected.ninthCall.reconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.ninthCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.ninthCall.stackPageContentSha256);
    assert.equal(moduleNameBytesSha256, expected.ninthCall.moduleNameBytesSha256);
  }
  assert.equal(receipt.stats.generation, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.ninthCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.ninthCall.receiptLineBytes);
  }

  const resume = resumeGetModuleHandleWStdcallResult(
    context,
    trapped,
    returnAddress,
    receipt.result,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.ninthCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.ninthCall.nextThunk.identity,
      expected.ninthCall.nextThunk.unsupported,
    );
    assert.equal(continuation.instructionCount, expected.ninthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.ninthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.ninthCall.budgetStops);
    assert.equal(continuation.terminal.returnAddress, expected.ninthCall.nextThunk.returnAddress);
    assert.deepEqual(continuation.terminal.stackWords, expected.ninthCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      arguments: [moduleNameAddress],
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      moduleNameAddress,
      moduleName,
      moduleNameUtf16Bytes: [...moduleNameBytes],
      moduleNameBytesSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    module: {
      ...receipt.logicalModule,
      preserveLastError: receipt.preserveLastError,
      nativeObservation: controlledLoadedModule.nativeObservation,
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchTenthGetProcAddressCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const argumentBytes = 8;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4 + argumentBytes,
    "read",
    "GetProcAddress stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(trapped.esp, "GetProcAddress return address");
  const moduleHandle = context.readVirtualU32(
    trapped.esp + 4,
    "GetProcAddress module handle",
  );
  const procedureNameAddress = context.readVirtualU32(
    trapped.esp + 8,
    "GetProcAddress procedure name address",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(moduleHandle, controlledDynamicExport.moduleHandle.value);
  assert(procedureNameAddress > 0xffff, "controlled GetProcAddress lookup must be by name");
  const procedureNameByteLength = controlledDynamicExport.requestedName.length + 1;
  const procedureNamePages = context.preflightVirtualRange(
    procedureNameAddress,
    procedureNameByteLength,
    "read",
    "GetProcAddress ASCII procedure name",
  );
  assert.equal(procedureNamePages.length, 1);
  assert.equal(procedureNamePages[0].owner, expected.image.owner);
  const procedureNameBytes = context.readVirtualBytes(
    procedureNameAddress,
    procedureNameByteLength,
    "GetProcAddress ASCII procedure name bytes",
  );
  assert.equal(procedureNameBytes.at(-1), 0);
  const procedureName = Buffer.from(procedureNameBytes.subarray(0, -1)).toString("ascii");
  assert.equal(procedureName, controlledDynamicExport.requestedName);
  const procedureNameBytesSha256 = sha256(procedureNameBytes);

  const callSequenceAddress = returnAddress - 14;
  const callSequence = context.readVirtualBytes(
    callSequenceAddress,
    14,
    "GetProcAddress register-indirect IAT call sequence",
  );
  const callView = new DataView(
    callSequence.buffer,
    callSequence.byteOffset,
    callSequence.byteLength,
  );
  assert.deepEqual([...callSequence.subarray(0, 2)], [0x8b, 0x35]);
  assert.equal(callView.getUint32(2, true), expected.ninthCall.nextThunk.identity.iatAddress);
  assert.equal(callSequence[6], 0x68);
  assert.equal(callView.getUint32(7, true), procedureNameAddress);
  assert.equal(callSequence[11], 0x57);
  assert.deepEqual([...callSequence.subarray(12, 14)], [0xff, 0xd6]);
  assert.equal(
    context.readVirtualU32(
      expected.ninthCall.nextThunk.identity.iatAddress,
      "GetProcAddress bound IAT",
    ),
    expected.ninthCall.nextThunk.identity.address,
  );

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before GetProcAddress return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-get-proc-address-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.sixthCall.deltaGeneration,
    thunkId: expected.ninthCall.nextThunk.identity.id,
    thunkAddress: expected.ninthCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes,
    stackPageAddress,
    stackPageOwner: expected.stack.owner,
    stackPageBytes: [...stackPageBytes],
    dynamicExportOwner: controlledDynamicExport.owner,
    variantId: controlledDynamicExport.variantId,
    moduleHandle,
    lookupKind: controlledDynamicExport.lookupKind,
    callVariant: controlledDynamicExport.callVariant,
    procedureNameAddress,
    procedureName,
    preserveLastError: controlledDynamicExport.preserveLastError,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.tenthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.tenthCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("GetProcAddress return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-get-proc-address-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.ninthCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, argumentBytes);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.procedureNameAddress, procedureNameAddress);
  assert.deepEqual(receipt.procedureNameBytes, [...procedureNameBytes]);
  assert.deepEqual(receipt.resolution, {
    owner: controlledDynamicExport.owner,
    variantId: controlledDynamicExport.variantId,
    moduleHandle: controlledDynamicExport.moduleHandle,
    canonicalModule: controlledDynamicExport.canonicalModule,
    lookupKind: controlledDynamicExport.lookupKind,
    callVariant: controlledDynamicExport.callVariant,
    requestedName: controlledDynamicExport.requestedName,
    resolvedName: controlledDynamicExport.resolvedName,
    endpointNamespace: controlledDynamicExport.endpoint.namespace,
    endpointLifetime: controlledDynamicExport.endpoint.lifetime,
    hostAddressImported: controlledDynamicExport.hostAddressImported,
  });
  assert.deepEqual(receipt.endpoint, offer.dynamicExportThunk);
  assert.equal(receipt.preserveLastError, controlledDynamicExport.preserveLastError);
  assert.equal(receipt.result, controlledDynamicExport.endpoint.address);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  if (expected.tenthCall.reconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.tenthCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.tenthCall.stackPageContentSha256);
    assert.equal(procedureNameBytesSha256, expected.tenthCall.procedureNameBytesSha256);
  }
  assert.equal(receipt.stats.generation, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.tenthCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.tenthCall.receiptLineBytes);
  }

  const resume = resumeGetProcAddressStdcallResult(
    context,
    trapped,
    returnAddress,
    receipt.result,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  validateThunk(
    continuation.terminal.thunk,
    expected.tenthCall.nextThunk.identity,
    expected.tenthCall.nextThunk.unsupported,
  );
  if (expected.tenthCall.instructionCount !== null) {
    assert.equal(continuation.instructionCount, expected.tenthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.tenthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.tenthCall.budgetStops);
    assert.equal(continuation.terminal.returnAddress, expected.tenthCall.nextThunk.returnAddress);
    assert.deepEqual(continuation.terminal.stackWords, expected.tenthCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      arguments: [moduleHandle, procedureNameAddress],
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lookupKind: request.lookupKind,
      callVariant: request.callVariant,
      procedureNameAddress,
      procedureName,
      procedureNameBytes: [...procedureNameBytes],
      procedureNameBytesSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    resolution: {
      ...receipt.resolution,
      preserveLastError: receipt.preserveLastError,
      nativeObservation: controlledDynamicExport.nativeObservation,
    },
    endpoint: receipt.endpoint,
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchEleventhGetProcAddressCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const argumentBytes = 8;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4 + argumentBytes,
    "read",
    "second GetProcAddress stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "second GetProcAddress return address",
  );
  const moduleHandle = context.readVirtualU32(
    trapped.esp + 4,
    "second GetProcAddress module handle",
  );
  const procedureNameAddress = context.readVirtualU32(
    trapped.esp + 8,
    "second GetProcAddress procedure name address",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(moduleHandle, controlledSecondDynamicExport.moduleHandle.value);
  assert(procedureNameAddress > 0xffff, "second GetProcAddress lookup must be by name");
  const procedureNameByteLength = controlledSecondDynamicExport.requestedName.length + 1;
  const procedureNamePages = context.preflightVirtualRange(
    procedureNameAddress,
    procedureNameByteLength,
    "read",
    "second GetProcAddress ASCII procedure name",
  );
  assert.equal(procedureNamePages.length, 1);
  assert.equal(procedureNamePages[0].owner, expected.image.owner);
  const procedureNameBytes = context.readVirtualBytes(
    procedureNameAddress,
    procedureNameByteLength,
    "second GetProcAddress ASCII procedure name bytes",
  );
  assert.equal(procedureNameBytes.at(-1), 0);
  const procedureName = Buffer.from(procedureNameBytes.subarray(0, -1)).toString("ascii");
  assert.equal(procedureName, controlledSecondDynamicExport.requestedName);
  const procedureNameBytesSha256 = sha256(procedureNameBytes);

  const callSequenceAddress = returnAddress - 13;
  const callSequence = context.readVirtualBytes(
    callSequenceAddress,
    13,
    "second GetProcAddress persistent-register call sequence",
  );
  const callView = new DataView(
    callSequence.buffer,
    callSequence.byteOffset,
    callSequence.byteLength,
  );
  assert.equal(callSequence[0], 0x68);
  assert.equal(callView.getUint32(1, true), procedureNameAddress);
  assert.equal(callSequence[5], 0x57);
  assert.equal(callSequence[6], 0xa3);
  const firstEndpointStoreAddress = callView.getUint32(7, true);
  assert.equal(firstEndpointStoreAddress, controlledSecondDynamicExport.firstEndpointStoreAddress);
  assert.deepEqual([...callSequence.subarray(11, 13)], [0xff, 0xd6]);
  assert.equal(trapped.esi, expected.tenthCall.nextThunk.identity.address);
  assert.equal(
    context.readVirtualU32(
      expected.tenthCall.nextThunk.identity.iatAddress,
      "second GetProcAddress bound IAT",
    ),
    expected.tenthCall.nextThunk.identity.address,
  );

  const firstEndpointStorePages = context.preflightVirtualRange(
    firstEndpointStoreAddress,
    4,
    "write",
    "BGI first dynamic endpoint store",
  );
  assert.equal(firstEndpointStorePages.length, 1);
  assert.equal(firstEndpointStorePages[0].owner, expected.image.owner);
  assert.equal(firstEndpointStorePages[0].protection.execute, false);
  const firstEndpointStoreValue = context.readVirtualU32(
    firstEndpointStoreAddress,
    "stored first dynamic endpoint",
  );
  assert.equal(firstEndpointStoreValue, controlledDynamicExport.endpoint.address);
  const firstEndpointStorePageAddress =
    (firstEndpointStoreAddress & ~(page.size - 1)) >>> 0;
  const firstEndpointStorePageBytes = context.readVirtualBytes(
    firstEndpointStorePageAddress,
    page.size,
    "current v86 first endpoint store page",
  );
  const firstEndpointStorePageContentSha256 = sha256(firstEndpointStorePageBytes);

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before second GetProcAddress return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-get-proc-address-second-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.sixthCall.deltaGeneration,
    thunkId: expected.tenthCall.nextThunk.identity.id,
    thunkAddress: expected.tenthCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes,
    stackPageAddress,
    stackPageOwner: expected.stack.owner,
    stackPageBytes: [...stackPageBytes],
    firstEndpointStorePageAddress,
    firstEndpointStorePageOwner: firstEndpointStorePages[0].owner,
    firstEndpointStorePageBytes: [...firstEndpointStorePageBytes],
    dynamicExportOwner: controlledSecondDynamicExport.owner,
    variantId: controlledSecondDynamicExport.variantId,
    moduleHandle,
    lookupKind: controlledSecondDynamicExport.lookupKind,
    callVariant: controlledSecondDynamicExport.callVariant,
    indirectCallRegister: controlledSecondDynamicExport.indirectCallRegister,
    indirectCallTarget: trapped.esi,
    procedureNameAddress,
    procedureName,
    firstEndpointAddress: controlledDynamicExport.endpoint.address,
    firstEndpointStoreAddress,
    preserveLastError: controlledSecondDynamicExport.preserveLastError,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 96 * 1024);
  if (expected.eleventhCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.eleventhCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("second GetProcAddress return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-get-proc-address-second-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.tenthCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, argumentBytes);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.procedureNameAddress, procedureNameAddress);
  assert.deepEqual(receipt.procedureNameBytes, [...procedureNameBytes]);
  assert.equal(
    receipt.indirectCallRegister,
    controlledSecondDynamicExport.indirectCallRegister,
  );
  assert.equal(receipt.indirectCallTarget, trapped.esi);
  assert.deepEqual(receipt.resolution, {
    owner: controlledSecondDynamicExport.owner,
    variantId: controlledSecondDynamicExport.variantId,
    moduleHandle: controlledSecondDynamicExport.moduleHandle,
    canonicalModule: controlledSecondDynamicExport.canonicalModule,
    lookupKind: controlledSecondDynamicExport.lookupKind,
    callVariant: controlledSecondDynamicExport.callVariant,
    requestedName: controlledSecondDynamicExport.requestedName,
    resolvedName: controlledSecondDynamicExport.resolvedName,
    endpointNamespace: controlledSecondDynamicExport.endpoint.namespace,
    endpointLifetime: controlledSecondDynamicExport.endpoint.lifetime,
    hostAddressImported: controlledSecondDynamicExport.hostAddressImported,
  });
  assert.deepEqual(receipt.firstEndpoint, controlledDynamicExport.endpoint);
  assert.equal(receipt.firstEndpointStoreAddress, firstEndpointStoreAddress);
  assert.equal(receipt.firstEndpointStoreValue, firstEndpointStoreValue);
  assert.deepEqual(receipt.endpoint, controlledSecondDynamicExport.endpoint);
  assert.equal(receipt.preserveLastError, controlledSecondDynamicExport.preserveLastError);
  assert.equal(receipt.result, controlledSecondDynamicExport.endpoint.address);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  assert.deepEqual(receipt.firstEndpointStoreReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: firstEndpointStorePageAddress,
    owner: expected.image.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.firstEndpointStoreReconciliation.changedBytes,
    contentSha256: firstEndpointStorePageContentSha256,
  });
  assert(receipt.firstEndpointStoreReconciliation.changedBytes > 0);
  if (expected.eleventhCall.stackReconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.eleventhCall.stackReconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.eleventhCall.stackPageContentSha256);
    assert.equal(
      receipt.firstEndpointStoreReconciliation.changedBytes,
      expected.eleventhCall.firstEndpointStoreReconciledChangedBytes,
    );
    assert.equal(
      firstEndpointStorePageContentSha256,
      expected.eleventhCall.firstEndpointStorePageContentSha256,
    );
    assert.equal(
      procedureNameBytesSha256,
      expected.eleventhCall.procedureNameBytesSha256,
    );
  }
  assert.equal(receipt.stats.generation, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.eleventhCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.eleventhCall.receiptLineBytes);
  }

  const resume = resumeSecondGetProcAddressStdcallResult(
    context,
    trapped,
    returnAddress,
    receipt.result,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  validateThunk(
    continuation.terminal.thunk,
    expected.eleventhCall.nextThunk.identity,
    expected.eleventhCall.nextThunk.unsupported,
  );
  const endpointStoreValue = context.readVirtualU32(
    controlledSecondDynamicExport.endpointStoreAddress,
    "stored second dynamic endpoint",
  );
  assert.equal(endpointStoreValue, controlledSecondDynamicExport.endpoint.address);
  if (expected.eleventhCall.instructionCount !== null) {
    assert.equal(continuation.instructionCount, expected.eleventhCall.instructionCount);
    assert.equal(continuation.executeStops, expected.eleventhCall.executeStops);
    assert.equal(continuation.budgetStops, expected.eleventhCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.eleventhCall.nextThunk.returnAddress,
    );
    assert.deepEqual(
      continuation.terminal.stackWords,
      expected.eleventhCall.nextThunk.stackWords,
    );
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      arguments: [moduleHandle, procedureNameAddress],
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      firstEndpointStorePageAddress,
      firstEndpointStorePageOwner: request.firstEndpointStorePageOwner,
      firstEndpointStorePageContentSha256,
      lookupKind: request.lookupKind,
      callVariant: request.callVariant,
      indirectCallRegister: request.indirectCallRegister,
      indirectCallTarget: request.indirectCallTarget,
      procedureNameAddress,
      procedureName,
      procedureNameBytes: [...procedureNameBytes],
      procedureNameBytesSha256,
      firstEndpointAddress: request.firstEndpointAddress,
      firstEndpointStoreAddress,
      firstEndpointStoreValue,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    resolution: {
      ...receipt.resolution,
      preserveLastError: receipt.preserveLastError,
      nativeObservation: controlledSecondDynamicExport.nativeObservation,
    },
    firstEndpoint: receipt.firstEndpoint,
    endpoint: receipt.endpoint,
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    endpointStore: {
      address: controlledSecondDynamicExport.endpointStoreAddress,
      value: endpointStoreValue,
    },
    continuation,
  };
}

async function dispatchTwelfthGetProcAddressCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const argumentBytes = 8;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4 + argumentBytes,
    "read",
    "third GetProcAddress stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "third GetProcAddress return address",
  );
  const moduleHandle = context.readVirtualU32(
    trapped.esp + 4,
    "third GetProcAddress module handle",
  );
  const procedureNameAddress = context.readVirtualU32(
    trapped.esp + 8,
    "third GetProcAddress procedure name address",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(moduleHandle, controlledThirdDynamicExport.moduleHandle.value);
  assert(procedureNameAddress > 0xffff, "third GetProcAddress lookup must be by name");
  const procedureNameByteLength = controlledThirdDynamicExport.requestedName.length + 1;
  const procedureNamePages = context.preflightVirtualRange(
    procedureNameAddress,
    procedureNameByteLength,
    "read",
    "third GetProcAddress ASCII procedure name",
  );
  assert.equal(procedureNamePages.length, 1);
  assert.equal(procedureNamePages[0].owner, expected.image.owner);
  const procedureNameBytes = context.readVirtualBytes(
    procedureNameAddress,
    procedureNameByteLength,
    "third GetProcAddress ASCII procedure name bytes",
  );
  assert.equal(procedureNameBytes.at(-1), 0);
  const procedureName = Buffer.from(procedureNameBytes.subarray(0, -1)).toString("ascii");
  assert.equal(procedureName, controlledThirdDynamicExport.requestedName);
  const procedureNameBytesSha256 = sha256(procedureNameBytes);

  const callSequenceAddress = returnAddress - 13;
  const callSequence = context.readVirtualBytes(
    callSequenceAddress,
    13,
    "third GetProcAddress persistent-register call sequence",
  );
  const callView = new DataView(
    callSequence.buffer,
    callSequence.byteOffset,
    callSequence.byteLength,
  );
  assert.equal(callSequence[0], 0x68);
  assert.equal(callView.getUint32(1, true), procedureNameAddress);
  assert.equal(callSequence[5], 0x57);
  assert.equal(callSequence[6], 0xa3);
  const secondEndpointStoreAddress = callView.getUint32(7, true);
  assert.equal(
    secondEndpointStoreAddress,
    controlledThirdDynamicExport.secondEndpointStoreAddress,
  );
  assert.deepEqual([...callSequence.subarray(11, 13)], [0xff, 0xd6]);
  assert.equal(trapped.esi, expected.eleventhCall.nextThunk.identity.address);
  assert.equal(
    context.readVirtualU32(
      expected.eleventhCall.nextThunk.identity.iatAddress,
      "third GetProcAddress bound IAT",
    ),
    expected.eleventhCall.nextThunk.identity.address,
  );

  const endpointStorePages = context.preflightVirtualRange(
    controlledThirdDynamicExport.firstEndpointStoreAddress,
    8,
    "write",
    "BGI prior dynamic endpoint stores",
  );
  assert.equal(endpointStorePages.length, 1);
  assert.equal(endpointStorePages[0].owner, expected.image.owner);
  assert.equal(endpointStorePages[0].protection.execute, false);
  const firstEndpointStoreValue = context.readVirtualU32(
    controlledThirdDynamicExport.firstEndpointStoreAddress,
    "stored first dynamic endpoint before third lookup",
  );
  const secondEndpointStoreValue = context.readVirtualU32(
    controlledThirdDynamicExport.secondEndpointStoreAddress,
    "stored second dynamic endpoint before third lookup",
  );
  assert.equal(firstEndpointStoreValue, controlledDynamicExport.endpoint.address);
  assert.equal(secondEndpointStoreValue, controlledSecondDynamicExport.endpoint.address);
  const endpointStorePageAddress =
    (controlledThirdDynamicExport.firstEndpointStoreAddress & ~(page.size - 1)) >>> 0;
  const endpointStorePageBytes = context.readVirtualBytes(
    endpointStorePageAddress,
    page.size,
    "current v86 prior endpoint store page",
  );
  const endpointStorePageContentSha256 = sha256(endpointStorePageBytes);

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before third GetProcAddress return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-get-proc-address-third-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.sixthCall.deltaGeneration,
    thunkId: expected.eleventhCall.nextThunk.identity.id,
    thunkAddress: expected.eleventhCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes,
    stackPageAddress,
    stackPageOwner: expected.stack.owner,
    stackPageBytes: [...stackPageBytes],
    endpointStorePageAddress,
    endpointStorePageOwner: endpointStorePages[0].owner,
    endpointStorePageBytes: [...endpointStorePageBytes],
    dynamicExportOwner: controlledThirdDynamicExport.owner,
    variantId: controlledThirdDynamicExport.variantId,
    moduleHandle,
    lookupKind: controlledThirdDynamicExport.lookupKind,
    callVariant: controlledThirdDynamicExport.callVariant,
    indirectCallRegister: controlledThirdDynamicExport.indirectCallRegister,
    indirectCallTarget: trapped.esi,
    procedureNameAddress,
    procedureName,
    firstEndpointAddress: controlledDynamicExport.endpoint.address,
    secondEndpointAddress: controlledSecondDynamicExport.endpoint.address,
    firstEndpointStoreAddress: controlledThirdDynamicExport.firstEndpointStoreAddress,
    secondEndpointStoreAddress,
    preserveLastError: controlledThirdDynamicExport.preserveLastError,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 96 * 1024);
  if (expected.twelfthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.twelfthCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("third GetProcAddress return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-get-proc-address-third-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.eleventhCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, argumentBytes);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.procedureNameAddress, procedureNameAddress);
  assert.deepEqual(receipt.procedureNameBytes, [...procedureNameBytes]);
  assert.equal(receipt.indirectCallRegister, controlledThirdDynamicExport.indirectCallRegister);
  assert.equal(receipt.indirectCallTarget, trapped.esi);
  assert.deepEqual(receipt.resolution, {
    owner: controlledThirdDynamicExport.owner,
    variantId: controlledThirdDynamicExport.variantId,
    moduleHandle: controlledThirdDynamicExport.moduleHandle,
    canonicalModule: controlledThirdDynamicExport.canonicalModule,
    lookupKind: controlledThirdDynamicExport.lookupKind,
    callVariant: controlledThirdDynamicExport.callVariant,
    requestedName: controlledThirdDynamicExport.requestedName,
    resolvedName: controlledThirdDynamicExport.resolvedName,
    endpointNamespace: controlledThirdDynamicExport.endpoint.namespace,
    endpointLifetime: controlledThirdDynamicExport.endpoint.lifetime,
    hostAddressImported: controlledThirdDynamicExport.hostAddressImported,
  });
  assert.deepEqual(receipt.firstEndpoint, controlledDynamicExport.endpoint);
  assert.equal(
    receipt.firstEndpointStoreAddress,
    controlledThirdDynamicExport.firstEndpointStoreAddress,
  );
  assert.equal(receipt.firstEndpointStoreValue, firstEndpointStoreValue);
  assert.deepEqual(receipt.secondEndpoint, controlledSecondDynamicExport.endpoint);
  assert.equal(receipt.secondEndpointStoreAddress, secondEndpointStoreAddress);
  assert.equal(receipt.secondEndpointStoreValue, secondEndpointStoreValue);
  assert.deepEqual(receipt.endpoint, controlledThirdDynamicExport.endpoint);
  assert.equal(receipt.preserveLastError, controlledThirdDynamicExport.preserveLastError);
  assert.equal(receipt.result, controlledThirdDynamicExport.endpoint.address);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  assert.deepEqual(receipt.endpointStoreReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: endpointStorePageAddress,
    owner: expected.image.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.endpointStoreReconciliation.changedBytes,
    contentSha256: endpointStorePageContentSha256,
  });
  assert(receipt.endpointStoreReconciliation.changedBytes > 0);
  if (expected.twelfthCall.stackReconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.twelfthCall.stackReconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.twelfthCall.stackPageContentSha256);
    assert.equal(
      receipt.endpointStoreReconciliation.changedBytes,
      expected.twelfthCall.endpointStoreReconciledChangedBytes,
    );
    assert.equal(
      endpointStorePageContentSha256,
      expected.twelfthCall.endpointStorePageContentSha256,
    );
    assert.equal(procedureNameBytesSha256, expected.twelfthCall.procedureNameBytesSha256);
  }
  assert.equal(receipt.stats.generation, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.twelfthCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.twelfthCall.receiptLineBytes);
  }

  const resume = resumeThirdGetProcAddressStdcallResult(
    context,
    trapped,
    returnAddress,
    receipt.result,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  validateThunk(
    continuation.terminal.thunk,
    expected.twelfthCall.nextThunk.identity,
    expected.twelfthCall.nextThunk.unsupported,
  );
  const endpointStoreValue = context.readVirtualU32(
    controlledThirdDynamicExport.endpointStoreAddress,
    "stored third dynamic endpoint",
  );
  assert.equal(endpointStoreValue, controlledThirdDynamicExport.endpoint.address);
  if (expected.twelfthCall.instructionCount !== null) {
    assert.equal(continuation.instructionCount, expected.twelfthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.twelfthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.twelfthCall.budgetStops);
    assert.equal(continuation.terminal.returnAddress, expected.twelfthCall.nextThunk.returnAddress);
    assert.deepEqual(continuation.terminal.stackWords, expected.twelfthCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      arguments: [moduleHandle, procedureNameAddress],
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      endpointStorePageAddress,
      endpointStorePageOwner: request.endpointStorePageOwner,
      endpointStorePageContentSha256,
      lookupKind: request.lookupKind,
      callVariant: request.callVariant,
      indirectCallRegister: request.indirectCallRegister,
      indirectCallTarget: request.indirectCallTarget,
      procedureNameAddress,
      procedureName,
      procedureNameBytes: [...procedureNameBytes],
      procedureNameBytesSha256,
      firstEndpointAddress: request.firstEndpointAddress,
      firstEndpointStoreAddress: request.firstEndpointStoreAddress,
      firstEndpointStoreValue,
      secondEndpointAddress: request.secondEndpointAddress,
      secondEndpointStoreAddress,
      secondEndpointStoreValue,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    resolution: {
      ...receipt.resolution,
      preserveLastError: receipt.preserveLastError,
      nativeObservation: controlledThirdDynamicExport.nativeObservation,
    },
    firstEndpoint: receipt.firstEndpoint,
    secondEndpoint: receipt.secondEndpoint,
    endpoint: receipt.endpoint,
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    endpointStore: {
      address: controlledThirdDynamicExport.endpointStoreAddress,
      value: endpointStoreValue,
    },
    continuation,
  };
}

async function dispatchThirteenthGetProcAddressCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const argumentBytes = 8;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4 + argumentBytes,
    "read",
    "fourth GetProcAddress stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "fourth GetProcAddress return address",
  );
  const moduleHandle = context.readVirtualU32(
    trapped.esp + 4,
    "fourth GetProcAddress module handle",
  );
  const procedureNameAddress = context.readVirtualU32(
    trapped.esp + 8,
    "fourth GetProcAddress procedure name address",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(moduleHandle, controlledFourthDynamicExport.moduleHandle.value);
  assert(procedureNameAddress > 0xffff, "fourth GetProcAddress lookup must be by name");
  const procedureNameByteLength = controlledFourthDynamicExport.requestedName.length + 1;
  const procedureNamePages = context.preflightVirtualRange(
    procedureNameAddress,
    procedureNameByteLength,
    "read",
    "fourth GetProcAddress ASCII procedure name",
  );
  assert.equal(procedureNamePages.length, 1);
  assert.equal(procedureNamePages[0].owner, expected.image.owner);
  const procedureNameBytes = context.readVirtualBytes(
    procedureNameAddress,
    procedureNameByteLength,
    "fourth GetProcAddress ASCII procedure name bytes",
  );
  assert.equal(procedureNameBytes.at(-1), 0);
  const procedureName = Buffer.from(procedureNameBytes.subarray(0, -1)).toString("ascii");
  assert.equal(procedureName, controlledFourthDynamicExport.requestedName);
  const procedureNameBytesSha256 = sha256(procedureNameBytes);

  const callSequenceAddress = returnAddress - 13;
  const callSequence = context.readVirtualBytes(
    callSequenceAddress,
    13,
    "fourth GetProcAddress persistent-register call sequence",
  );
  const callView = new DataView(
    callSequence.buffer,
    callSequence.byteOffset,
    callSequence.byteLength,
  );
  assert.equal(callSequence[0], 0x68);
  assert.equal(callView.getUint32(1, true), procedureNameAddress);
  assert.equal(callSequence[5], 0x57);
  assert.equal(callSequence[6], 0xa3);
  const thirdEndpointStoreAddress = callView.getUint32(7, true);
  assert.equal(
    thirdEndpointStoreAddress,
    controlledFourthDynamicExport.thirdEndpointStoreAddress,
  );
  assert.deepEqual([...callSequence.subarray(11, 13)], [0xff, 0xd6]);
  assert.equal(trapped.esi, expected.twelfthCall.nextThunk.identity.address);
  assert.equal(
    context.readVirtualU32(
      expected.twelfthCall.nextThunk.identity.iatAddress,
      "fourth GetProcAddress bound IAT",
    ),
    expected.twelfthCall.nextThunk.identity.address,
  );

  const endpointStorePages = context.preflightVirtualRange(
    controlledFourthDynamicExport.firstEndpointStoreAddress,
    12,
    "write",
    "BGI prior dynamic endpoint stores",
  );
  assert.equal(endpointStorePages.length, 1);
  assert.equal(endpointStorePages[0].owner, expected.image.owner);
  assert.equal(endpointStorePages[0].protection.execute, false);
  const firstEndpointStoreValue = context.readVirtualU32(
    controlledFourthDynamicExport.firstEndpointStoreAddress,
    "stored first dynamic endpoint before fourth lookup",
  );
  const secondEndpointStoreValue = context.readVirtualU32(
    controlledFourthDynamicExport.secondEndpointStoreAddress,
    "stored second dynamic endpoint before fourth lookup",
  );
  const thirdEndpointStoreValue = context.readVirtualU32(
    controlledFourthDynamicExport.thirdEndpointStoreAddress,
    "stored third dynamic endpoint before fourth lookup",
  );
  assert.equal(firstEndpointStoreValue, controlledDynamicExport.endpoint.address);
  assert.equal(secondEndpointStoreValue, controlledSecondDynamicExport.endpoint.address);
  assert.equal(thirdEndpointStoreValue, controlledThirdDynamicExport.endpoint.address);
  const endpointStorePageAddress =
    (controlledFourthDynamicExport.firstEndpointStoreAddress & ~(page.size - 1)) >>> 0;
  const endpointStorePageBytes = context.readVirtualBytes(
    endpointStorePageAddress,
    page.size,
    "current v86 prior endpoint store page",
  );
  const endpointStorePageContentSha256 = sha256(endpointStorePageBytes);

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before fourth GetProcAddress return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-get-proc-address-fourth-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.sixthCall.deltaGeneration,
    thunkId: expected.twelfthCall.nextThunk.identity.id,
    thunkAddress: expected.twelfthCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes,
    stackPageAddress,
    stackPageOwner: expected.stack.owner,
    stackPageBytes: [...stackPageBytes],
    endpointStorePageAddress,
    endpointStorePageOwner: endpointStorePages[0].owner,
    endpointStorePageBytes: [...endpointStorePageBytes],
    dynamicExportOwner: controlledFourthDynamicExport.owner,
    variantId: controlledFourthDynamicExport.variantId,
    moduleHandle,
    lookupKind: controlledFourthDynamicExport.lookupKind,
    callVariant: controlledFourthDynamicExport.callVariant,
    indirectCallRegister: controlledFourthDynamicExport.indirectCallRegister,
    indirectCallTarget: trapped.esi,
    procedureNameAddress,
    procedureName,
    firstEndpointAddress: controlledDynamicExport.endpoint.address,
    secondEndpointAddress: controlledSecondDynamicExport.endpoint.address,
    thirdEndpointAddress: controlledThirdDynamicExport.endpoint.address,
    firstEndpointStoreAddress: controlledFourthDynamicExport.firstEndpointStoreAddress,
    secondEndpointStoreAddress: controlledFourthDynamicExport.secondEndpointStoreAddress,
    thirdEndpointStoreAddress,
    preserveLastError: controlledFourthDynamicExport.preserveLastError,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 96 * 1024);
  if (expected.thirteenthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.thirteenthCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("fourth GetProcAddress return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-get-proc-address-fourth-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.twelfthCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, argumentBytes);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.procedureNameAddress, procedureNameAddress);
  assert.deepEqual(receipt.procedureNameBytes, [...procedureNameBytes]);
  assert.equal(receipt.indirectCallRegister, controlledFourthDynamicExport.indirectCallRegister);
  assert.equal(receipt.indirectCallTarget, trapped.esi);
  assert.deepEqual(receipt.resolution, {
    owner: controlledFourthDynamicExport.owner,
    variantId: controlledFourthDynamicExport.variantId,
    moduleHandle: controlledFourthDynamicExport.moduleHandle,
    canonicalModule: controlledFourthDynamicExport.canonicalModule,
    lookupKind: controlledFourthDynamicExport.lookupKind,
    callVariant: controlledFourthDynamicExport.callVariant,
    requestedName: controlledFourthDynamicExport.requestedName,
    resolvedName: controlledFourthDynamicExport.resolvedName,
    endpointNamespace: controlledFourthDynamicExport.endpoint.namespace,
    endpointLifetime: controlledFourthDynamicExport.endpoint.lifetime,
    hostAddressImported: controlledFourthDynamicExport.hostAddressImported,
  });
  assert.deepEqual(receipt.firstEndpoint, controlledDynamicExport.endpoint);
  assert.equal(
    receipt.firstEndpointStoreAddress,
    controlledFourthDynamicExport.firstEndpointStoreAddress,
  );
  assert.equal(receipt.firstEndpointStoreValue, firstEndpointStoreValue);
  assert.deepEqual(receipt.secondEndpoint, controlledSecondDynamicExport.endpoint);
  assert.equal(
    receipt.secondEndpointStoreAddress,
    controlledFourthDynamicExport.secondEndpointStoreAddress,
  );
  assert.equal(receipt.secondEndpointStoreValue, secondEndpointStoreValue);
  assert.deepEqual(receipt.thirdEndpoint, controlledThirdDynamicExport.endpoint);
  assert.equal(receipt.thirdEndpointStoreAddress, thirdEndpointStoreAddress);
  assert.equal(receipt.thirdEndpointStoreValue, thirdEndpointStoreValue);
  assert.deepEqual(receipt.endpoint, controlledFourthDynamicExport.endpoint);
  assert.equal(receipt.preserveLastError, controlledFourthDynamicExport.preserveLastError);
  assert.equal(receipt.result, controlledFourthDynamicExport.endpoint.address);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  assert.deepEqual(receipt.endpointStoreReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: endpointStorePageAddress,
    owner: expected.image.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.endpointStoreReconciliation.changedBytes,
    contentSha256: endpointStorePageContentSha256,
  });
  assert(receipt.endpointStoreReconciliation.changedBytes > 0);
  if (expected.thirteenthCall.stackReconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.thirteenthCall.stackReconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.thirteenthCall.stackPageContentSha256);
    assert.equal(
      receipt.endpointStoreReconciliation.changedBytes,
      expected.thirteenthCall.endpointStoreReconciledChangedBytes,
    );
    assert.equal(
      endpointStorePageContentSha256,
      expected.thirteenthCall.endpointStorePageContentSha256,
    );
    assert.equal(
      procedureNameBytesSha256,
      expected.thirteenthCall.procedureNameBytesSha256,
    );
  }
  assert.equal(receipt.stats.generation, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.thirteenthCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.thirteenthCall.receiptLineBytes);
  }

  const resume = resumeFourthGetProcAddressStdcallResult(
    context,
    trapped,
    returnAddress,
    receipt.result,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  const endpointStoreValue = context.readVirtualU32(
    controlledFourthDynamicExport.endpointStoreAddress,
    "stored fourth dynamic endpoint",
  );
  assert.equal(endpointStoreValue, controlledFourthDynamicExport.endpoint.address);
  if (expected.thirteenthCall.nextTerminal !== null) {
    if (continueSession) {
      assert.equal(continuation.terminal.kind, expected.thirteenthCall.nextTerminal.kind);
      const { disposition: _priorDisposition, ...tlsAllocThunkIdentity } =
        expected.thirteenthCall.nextTerminal.thunk;
      validateThunk(continuation.terminal.thunk, tlsAllocThunkIdentity, false);
      assert.equal(
        continuation.terminal.returnAddress,
        expected.thirteenthCall.nextTerminal.returnAddress,
      );
      assert.deepEqual(
        continuation.terminal.stackWords,
        expected.thirteenthCall.nextTerminal.stackWords,
      );
    } else {
      assert.deepEqual(continuation.terminal, expected.thirteenthCall.nextTerminal);
    }
  }
  if (expected.thirteenthCall.instructionCount !== null) {
    assert.equal(continuation.instructionCount, expected.thirteenthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.thirteenthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.thirteenthCall.budgetStops);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      arguments: [moduleHandle, procedureNameAddress],
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      endpointStorePageAddress,
      endpointStorePageOwner: request.endpointStorePageOwner,
      endpointStorePageContentSha256,
      lookupKind: request.lookupKind,
      callVariant: request.callVariant,
      indirectCallRegister: request.indirectCallRegister,
      indirectCallTarget: request.indirectCallTarget,
      procedureNameAddress,
      procedureName,
      procedureNameBytes: [...procedureNameBytes],
      procedureNameBytesSha256,
      firstEndpointAddress: request.firstEndpointAddress,
      firstEndpointStoreAddress: request.firstEndpointStoreAddress,
      firstEndpointStoreValue,
      secondEndpointAddress: request.secondEndpointAddress,
      secondEndpointStoreAddress: request.secondEndpointStoreAddress,
      secondEndpointStoreValue,
      thirdEndpointAddress: request.thirdEndpointAddress,
      thirdEndpointStoreAddress,
      thirdEndpointStoreValue,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    resolution: {
      ...receipt.resolution,
      preserveLastError: receipt.preserveLastError,
      nativeObservation: controlledFourthDynamicExport.nativeObservation,
    },
    firstEndpoint: receipt.firstEndpoint,
    secondEndpoint: receipt.secondEndpoint,
    thirdEndpoint: receipt.thirdEndpoint,
    endpoint: receipt.endpoint,
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    endpointStore: {
      address: controlledFourthDynamicExport.endpointStoreAddress,
      value: endpointStoreValue,
    },
    continuation,
  };
}

async function dispatchFourteenthTlsAllocCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const argumentBytes = 0;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4,
    "read",
    "TlsAlloc stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(trapped.esp, "TlsAlloc return address");
  assert.equal(returnAddress, execution.terminal.returnAddress);

  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(callSite, 6, "TlsAlloc indirect IAT call");
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), execution.terminal.thunk.iatAddress);
  assert.equal(
    context.readVirtualU32(execution.terminal.thunk.iatAddress, "TlsAlloc bound IAT"),
    execution.terminal.thunk.address,
  );

  const endpointStorePages = context.preflightVirtualRange(
    controlledFourthDynamicExport.firstEndpointStoreAddress,
    16,
    "write",
    "BGI four dynamic endpoint stores before TlsAlloc",
  );
  assert.equal(endpointStorePages.length, 1);
  assert.equal(endpointStorePages[0].owner, expected.image.owner);
  assert.equal(endpointStorePages[0].protection.execute, false);
  const endpointStoreAddresses = [
    controlledFourthDynamicExport.firstEndpointStoreAddress,
    controlledFourthDynamicExport.secondEndpointStoreAddress,
    controlledFourthDynamicExport.thirdEndpointStoreAddress,
    controlledFourthDynamicExport.endpointStoreAddress,
  ];
  const endpointAddresses = [
    controlledDynamicExport.endpoint.address,
    controlledSecondDynamicExport.endpoint.address,
    controlledThirdDynamicExport.endpoint.address,
    controlledFourthDynamicExport.endpoint.address,
  ];
  const endpointStoreValues = endpointStoreAddresses.map((address, index) => {
    const value = context.readVirtualU32(address, `stored dynamic endpoint ${index + 1}`);
    assert.equal(value, endpointAddresses[index]);
    return value;
  });
  const endpointStorePageAddress =
    (endpointStoreAddresses[0] & ~(page.size - 1)) >>> 0;
  const endpointStorePageBytes = context.readVirtualBytes(
    endpointStorePageAddress,
    page.size,
    "current v86 endpoint store page before TlsAlloc return",
  );
  const endpointStorePageContentSha256 = sha256(endpointStorePageBytes);

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before TlsAlloc return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-tls-alloc-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.sixthCall.deltaGeneration,
    thunkId: execution.terminal.thunk.id,
    thunkAddress: execution.terminal.thunk.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes,
    stackPageAddress,
    stackPageOwner: expected.stack.owner,
    stackPageBytes: [...stackPageBytes],
    endpointStorePageAddress,
    endpointStorePageOwner: endpointStorePages[0].owner,
    endpointStorePageBytes: [...endpointStorePageBytes],
    firstEndpointAddress: endpointAddresses[0],
    secondEndpointAddress: endpointAddresses[1],
    thirdEndpointAddress: endpointAddresses[2],
    fourthEndpointAddress: endpointAddresses[3],
    firstEndpointStoreAddress: endpointStoreAddresses[0],
    secondEndpointStoreAddress: endpointStoreAddresses[1],
    thirdEndpointStoreAddress: endpointStoreAddresses[2],
    fourthEndpointStoreAddress: endpointStoreAddresses[3],
    tlsStateOwner: controlledTlsAllocation.owner,
    variantId: controlledTlsAllocation.variantId,
    logicalIndexNamespace: controlledTlsAllocation.index.namespace,
    logicalIndex: controlledTlsAllocation.index.value,
    failureSentinel: controlledTlsAllocation.index.failureSentinel,
    preserveLastError: controlledTlsAllocation.preserveLastError,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 96 * 1024);
  if (expected.fourteenthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.fourteenthCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("TlsAlloc return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-tls-alloc-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  const { disposition: _priorDisposition, ...tlsAllocThunkIdentity } =
    expected.thirteenthCall.nextTerminal.thunk;
  validateThunk(receipt.thunk, tlsAllocThunkIdentity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, argumentBytes);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.deepEqual(receipt.logicalTlsSlot, {
    owner: controlledTlsAllocation.owner,
    variantId: controlledTlsAllocation.variantId,
    index: controlledTlsAllocation.index,
    lifecycle: controlledTlsAllocation.lifecycle,
    storageScope: controlledTlsAllocation.storageScope,
    initializedThreadValues: controlledTlsAllocation.initializedThreadValues,
    backingMaterialization: controlledTlsAllocation.backingMaterialization,
  });
  assert.equal(receipt.preserveLastError, controlledTlsAllocation.preserveLastError);
  assert.equal(receipt.result, controlledTlsAllocation.index.value);
  for (let index = 0; index < endpointStoreAddresses.length; index += 1) {
    const ordinal = ["first", "second", "third", "fourth"][index];
    assert.equal(receipt[`${ordinal}EndpointStoreAddress`], endpointStoreAddresses[index]);
    assert.equal(receipt[`${ordinal}EndpointStoreValue`], endpointStoreValues[index]);
  }
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  assert.deepEqual(receipt.endpointStoreReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: endpointStorePageAddress,
    owner: expected.image.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.endpointStoreReconciliation.changedBytes,
    contentSha256: endpointStorePageContentSha256,
  });
  assert(receipt.endpointStoreReconciliation.changedBytes > 0);
  if (expected.fourteenthCall.stackReconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.fourteenthCall.stackReconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.fourteenthCall.stackPageContentSha256);
    assert.equal(
      receipt.endpointStoreReconciliation.changedBytes,
      expected.fourteenthCall.endpointStoreReconciledChangedBytes,
    );
    assert.equal(
      endpointStorePageContentSha256,
      expected.fourteenthCall.endpointStorePageContentSha256,
    );
  }
  assert.equal(receipt.stats.generation, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.fourteenthCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.fourteenthCall.receiptLineBytes);
  }

  const resume = resumeTlsAllocStdcallResult(
    context,
    trapped,
    returnAddress,
    receipt.result,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  for (let index = 0; index < endpointStoreAddresses.length; index += 1) {
    assert.equal(
      context.readVirtualU32(
        endpointStoreAddresses[index],
        `preserved dynamic endpoint ${index + 1} after TlsAlloc`,
      ),
      endpointAddresses[index],
    );
  }
  if (expected.fourteenthCall.nextTerminal !== null) {
    if (continueSession) {
      assert.equal(continuation.terminal.kind, expected.fourteenthCall.nextTerminal.kind);
      const { disposition: _priorDisposition, ...tlsSetValueThunkIdentity } =
        expected.fourteenthCall.nextTerminal.thunk;
      validateThunk(continuation.terminal.thunk, tlsSetValueThunkIdentity, false);
      assert.equal(
        continuation.terminal.returnAddress,
        expected.fourteenthCall.nextTerminal.returnAddress,
      );
      assert.deepEqual(
        continuation.terminal.stackWords,
        expected.fourteenthCall.nextTerminal.stackWords,
      );
    } else {
      assert.deepEqual(continuation.terminal, expected.fourteenthCall.nextTerminal);
    }
  }
  if (expected.fourteenthCall.instructionCount !== null) {
    assert.equal(continuation.instructionCount, expected.fourteenthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.fourteenthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.fourteenthCall.budgetStops);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      arguments: [],
      stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      endpointStorePageAddress,
      endpointStorePageOwner: request.endpointStorePageOwner,
      endpointStorePageContentSha256,
      endpointAddresses,
      endpointStoreAddresses,
      endpointStoreValues,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    slot: receipt.logicalTlsSlot,
    nativeObservation: controlledTlsAllocation.nativeObservation,
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchFifteenthTlsSetValueCall({
  child,
  continueSession,
  context,
  execution,
  nextMessage,
  offer,
  tlsAllocation,
}) {
  const trapped = execution.finalState;
  const argumentBytes = 8;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4 + argumentBytes,
    "read",
    "TlsSetValue stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(trapped.esp, "TlsSetValue return address");
  const logicalIndex = context.readVirtualU32(trapped.esp + 4, "TlsSetValue index");
  const valueAddress = context.readVirtualU32(trapped.esp + 8, "TlsSetValue value");
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(logicalIndex, controlledTlsValue.index.value);
  assert.equal(valueAddress, controlledTlsValue.endpoint.address);

  const callSequence = context.readVirtualBytes(
    returnAddress - 9,
    9,
    "TlsSetValue push-value / push-index / call-esi sequence",
  );
  assert.deepEqual([...callSequence.subarray(0, 2)], [0xff, 0x35]);
  const callView = new DataView(
    callSequence.buffer,
    callSequence.byteOffset,
    callSequence.byteLength,
  );
  assert.equal(
    callView.getUint32(2, true),
    controlledFourthDynamicExport.secondEndpointStoreAddress,
  );
  assert.equal(callSequence[6], 0x50);
  assert.deepEqual([...callSequence.subarray(7, 9)], [0xff, 0xd6]);
  assert.equal(trapped.esi, execution.terminal.thunk.address);
  assert.equal(
    context.readVirtualU32(execution.terminal.thunk.iatAddress, "TlsSetValue bound IAT"),
    execution.terminal.thunk.address,
  );

  const endpointStoreAddresses = tlsAllocation.request.endpointStoreAddresses;
  const endpointAddresses = tlsAllocation.request.endpointAddresses;
  const endpointStorePages = context.preflightVirtualRange(
    endpointStoreAddresses[0],
    16,
    "write",
    "BGI dynamic endpoint stores before TlsSetValue",
  );
  assert.equal(endpointStorePages.length, 1);
  assert.equal(endpointStorePages[0].owner, expected.image.owner);
  const endpointStoreValues = endpointStoreAddresses.map((address, index) => {
    const value = context.readVirtualU32(
      address,
      `stored dynamic endpoint ${index + 1} before TlsSetValue`,
    );
    assert.equal(value, endpointAddresses[index]);
    return value;
  });
  const endpointStorePageAddress =
    (endpointStoreAddresses[0] & ~(page.size - 1)) >>> 0;
  const endpointStorePageBytes = context.readVirtualBytes(
    endpointStorePageAddress,
    page.size,
    "current v86 endpoint store page before TlsSetValue return",
  );
  const endpointStorePageContentSha256 = sha256(endpointStorePageBytes);

  const tlsIndexStoreAddress = 0x100f0764;
  const tlsIndexStorePageAddress = (tlsIndexStoreAddress & ~(page.size - 1)) >>> 0;
  const tlsIndexStorePages = context.preflightVirtualRange(
    tlsIndexStoreAddress,
    4,
    "write",
    "BGI TLS index store",
  );
  assert.equal(tlsIndexStorePages.length, 1);
  assert.equal(tlsIndexStorePages[0].owner, expected.image.owner);
  assert.equal(
    context.readVirtualU32(tlsIndexStoreAddress, "stored logical TLS index"),
    logicalIndex,
  );
  const tlsIndexStorePageBytes = context.readVirtualBytes(
    tlsIndexStorePageAddress,
    page.size,
    "current v86 TLS index store page",
  );
  const tlsIndexStorePageContentSha256 = sha256(tlsIndexStorePageBytes);

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before TlsSetValue return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-tls-set-value-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.sixthCall.deltaGeneration,
    thunkId: execution.terminal.thunk.id,
    thunkAddress: execution.terminal.thunk.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes,
    indirectCallRegister: "esi",
    indirectCallTarget: trapped.esi,
    stackPageAddress,
    stackPageOwner: expected.stack.owner,
    stackPageBytes: [...stackPageBytes],
    endpointStorePageAddress,
    endpointStorePageOwner: endpointStorePages[0].owner,
    endpointStorePageBytes: [...endpointStorePageBytes],
    tlsIndexStoreAddress,
    tlsIndexStorePageAddress,
    tlsIndexStorePageOwner: tlsIndexStorePages[0].owner,
    tlsIndexStorePageBytes: [...tlsIndexStorePageBytes],
    endpointStoreAddresses,
    endpointAddresses,
    tlsStateOwner: controlledTlsValue.owner,
    allocationVariantId: controlledTlsValue.allocationVariantId,
    logicalIndexNamespace: controlledTlsValue.index.namespace,
    logicalIndex,
    expectedInitializedThreadValues: tlsAllocation.slot.initializedThreadValues,
    threadIdentityOwner: controlledTlsValue.threadIdentity.owner,
    threadId: controlledTlsValue.threadIdentity.threadId,
    valueKind: controlledTlsValue.valueKind,
    valueEndpointNamespace: controlledTlsValue.endpoint.namespace,
    valueEndpointId: controlledTlsValue.endpoint.id,
    valueAddress,
    preserveLastError: controlledTlsValue.preserveLastError,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 96 * 1024);
  if (expected.fifteenthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.fifteenthCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("TlsSetValue return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-tls-set-value-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  const { disposition: _priorDisposition, ...tlsSetValueThunkIdentity } =
    expected.fourteenthCall.nextTerminal.thunk;
  validateThunk(receipt.thunk, tlsSetValueThunkIdentity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, argumentBytes);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.deepEqual(receipt.logicalTlsSlotBefore, tlsAllocation.slot);
  assert.deepEqual(receipt.logicalTlsSlotAfter, {
    ...tlsAllocation.slot,
    initializedThreadValues: 1,
    backingMaterialization: "initial-thread-value-metadata",
  });
  assert.deepEqual(receipt.threadValue, {
    threadIdentityOwner: controlledTlsValue.threadIdentity.owner,
    threadId: controlledTlsValue.threadIdentity.threadId,
    indexNamespace: controlledTlsValue.index.namespace,
    index: controlledTlsValue.index.value,
    valueKind: controlledTlsValue.valueKind,
    endpoint: controlledTlsValue.endpoint,
    lifecycle: "set",
  });
  assert.equal(receipt.preserveLastError, controlledTlsValue.preserveLastError);
  assert.equal(receipt.result, controlledTlsValue.result);
  assert.deepEqual(receipt.endpointStoreAddresses, endpointStoreAddresses);
  assert.deepEqual(receipt.endpointStoreValues, endpointStoreValues);
  assert.equal(receipt.tlsIndexStoreAddress, tlsIndexStoreAddress);
  assert.equal(receipt.tlsIndexStoreValue, logicalIndex);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert.deepEqual(receipt.endpointStoreReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: endpointStorePageAddress,
    owner: expected.image.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.endpointStoreReconciliation.changedBytes,
    contentSha256: endpointStorePageContentSha256,
  });
  assert.deepEqual(receipt.tlsIndexStoreReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: tlsIndexStorePageAddress,
    owner: expected.image.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.tlsIndexStoreReconciliation.changedBytes,
    contentSha256: tlsIndexStorePageContentSha256,
  });
  if (expected.fifteenthCall.stackReconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.fifteenthCall.stackReconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.fifteenthCall.stackPageContentSha256);
    assert.equal(
      receipt.endpointStoreReconciliation.changedBytes,
      expected.fifteenthCall.endpointStoreReconciledChangedBytes,
    );
    assert.equal(
      endpointStorePageContentSha256,
      expected.fifteenthCall.endpointStorePageContentSha256,
    );
    assert.equal(
      receipt.tlsIndexStoreReconciliation.changedBytes,
      expected.fifteenthCall.tlsIndexStoreReconciledChangedBytes,
    );
    assert.equal(
      tlsIndexStorePageContentSha256,
      expected.fifteenthCall.tlsIndexStorePageContentSha256,
    );
  }
  assert.equal(receipt.stats.generation, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.fifteenthCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.fifteenthCall.receiptLineBytes);
  }

  const resume = resumeTlsSetValueStdcallResult(
    context,
    trapped,
    returnAddress,
    receipt.result,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  for (let index = 0; index < endpointStoreAddresses.length; index += 1) {
    assert.equal(
      context.readVirtualU32(
        endpointStoreAddresses[index],
        `preserved dynamic endpoint ${index + 1} after TlsSetValue`,
      ),
      endpointAddresses[index],
    );
  }
  assert.equal(
    context.readVirtualU32(tlsIndexStoreAddress, "logical TLS index after TlsSetValue"),
    logicalIndex,
  );
  if (expected.fifteenthCall.nextTerminal !== null) {
    if (continueSession) {
      assert.equal(continuation.terminal.kind, expected.fifteenthCall.nextTerminal.kind);
      const { disposition: _priorDisposition, ...encodePointerThunkIdentity } =
        expected.fifteenthCall.nextTerminal.thunk;
      validateThunk(continuation.terminal.thunk, encodePointerThunkIdentity, false);
      assert.equal(
        continuation.terminal.returnAddress,
        expected.fifteenthCall.nextTerminal.returnAddress,
      );
      assert.deepEqual(
        continuation.terminal.stackWords,
        expected.fifteenthCall.nextTerminal.stackWords,
      );
    } else {
      assert.deepEqual(continuation.terminal, expected.fifteenthCall.nextTerminal);
    }
  }
  if (expected.fifteenthCall.instructionCount !== null) {
    assert.equal(continuation.instructionCount, expected.fifteenthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.fifteenthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.fifteenthCall.budgetStops);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      arguments: [logicalIndex, valueAddress],
      indirectCallRegister: request.indirectCallRegister,
      indirectCallTarget: request.indirectCallTarget,
      stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      endpointStorePageAddress,
      endpointStorePageOwner: request.endpointStorePageOwner,
      endpointStorePageContentSha256,
      tlsIndexStoreAddress,
      tlsIndexStorePageAddress,
      tlsIndexStorePageOwner: request.tlsIndexStorePageOwner,
      tlsIndexStorePageContentSha256,
      endpointAddresses,
      endpointStoreAddresses,
      endpointStoreValues,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    slotBefore: receipt.logicalTlsSlotBefore,
    slotAfter: receipt.logicalTlsSlotAfter,
    threadValue: receipt.threadValue,
    nativeObservation: controlledTlsValue.nativeObservation,
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchSixteenthEncodePointerCall({
  child,
  continueSession,
  context,
  execution,
  nextMessage,
  offer,
  tlsAllocation,
  tlsValue,
}) {
  const trapped = execution.finalState;
  const argumentBytes = 4;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4 + argumentBytes,
    "read",
    "EncodePointer stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(trapped.esp, "EncodePointer return address");
  const sourcePointer = context.readVirtualU32(trapped.esp + 4, "EncodePointer source pointer");
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(sourcePointer, controlledEncodedNull.sourcePointer);

  const callSequence = context.readVirtualBytes(
    returnAddress - 8,
    8,
    "EncodePointer push-null / direct IAT call sequence",
  );
  assert.deepEqual([...callSequence.subarray(0, 4)], [0x6a, 0x00, 0xff, 0x15]);
  const callView = new DataView(
    callSequence.buffer,
    callSequence.byteOffset,
    callSequence.byteLength,
  );
  assert.equal(callView.getUint32(4, true), execution.terminal.thunk.iatAddress);
  assert.equal(
    context.readVirtualU32(execution.terminal.thunk.iatAddress, "EncodePointer bound IAT"),
    execution.terminal.thunk.address,
  );

  const endpointStoreAddresses = tlsAllocation.request.endpointStoreAddresses;
  const endpointAddresses = tlsAllocation.request.endpointAddresses;
  for (let index = 0; index < endpointStoreAddresses.length; index += 1) {
    assert.equal(
      context.readVirtualU32(
        endpointStoreAddresses[index],
        `preserved dynamic endpoint ${index + 1} before EncodePointer`,
      ),
      endpointAddresses[index],
    );
  }
  assert.equal(
    context.readVirtualU32(
      tlsValue.request.tlsIndexStoreAddress,
      "logical TLS index before EncodePointer",
    ),
    controlledTlsValue.index.value,
  );

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before EncodePointer return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-encode-pointer-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.sixthCall.deltaGeneration,
    thunkId: execution.terminal.thunk.id,
    thunkAddress: execution.terminal.thunk.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes,
    sourcePointer,
    stackPageAddress,
    stackPageOwner: framePages[0].owner,
    stackPageBytes: [...stackPageBytes],
    pointerEncodingOwner: controlledEncodedNull.owner,
    variantId: controlledEncodedNull.variantId,
    encodedPointerNamespace: controlledEncodedNull.namespace,
    encodedPointerValue: controlledEncodedNull.value,
    expectedTlsIndex: controlledTlsValue.index.value,
    expectedTlsThreadId: controlledTlsValue.threadIdentity.threadId,
    expectedTlsValueEndpoint: controlledTlsValue.endpoint.address,
    preserveLastError: controlledEncodedNull.preserveLastError,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 48 * 1024);
  if (expected.sixteenthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.sixteenthCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("EncodePointer return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-encode-pointer-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  const { disposition: _priorDisposition, ...encodePointerThunkIdentity } =
    expected.fifteenthCall.nextTerminal.thunk;
  validateThunk(receipt.thunk, encodePointerThunkIdentity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, argumentBytes);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  const encodedPointer = {
    owner: controlledEncodedNull.owner,
    variantId: controlledEncodedNull.variantId,
    namespace: controlledEncodedNull.namespace,
    value: controlledEncodedNull.value,
    sourceKind: controlledEncodedNull.sourceKind,
    sourcePointer: controlledEncodedNull.sourcePointer,
    lifetime: controlledEncodedNull.lifetime,
    processCookieMaterialization: controlledEncodedNull.processCookieMaterialization,
    nativeResultImported: controlledEncodedNull.nativeResultImported,
  };
  assert.deepEqual(receipt.encodedPointer, encodedPointer);
  assert.deepEqual(receipt.preservedTlsSlot, tlsValue.slotAfter);
  assert.deepEqual(receipt.preservedTlsThreadValue, tlsValue.threadValue);
  assert.equal(receipt.preserveLastError, controlledEncodedNull.preserveLastError);
  assert.equal(receipt.result, controlledEncodedNull.value);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  if (expected.sixteenthCall.stackReconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.sixteenthCall.stackReconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.sixteenthCall.stackPageContentSha256);
  }
  assert.equal(receipt.stats.generation, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.sixteenthCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.sixteenthCall.receiptLineBytes);
  }

  const resume = resumeEncodePointerStdcallResult(
    context,
    trapped,
    returnAddress,
    receipt.result,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  for (let index = 0; index < endpointStoreAddresses.length; index += 1) {
    assert.equal(
      context.readVirtualU32(
        endpointStoreAddresses[index],
        `preserved dynamic endpoint ${index + 1} after EncodePointer`,
      ),
      endpointAddresses[index],
    );
  }
  assert.equal(
    context.readVirtualU32(
      tlsValue.request.tlsIndexStoreAddress,
      "logical TLS index after EncodePointer",
    ),
    controlledTlsValue.index.value,
  );
  assert.deepEqual(receipt.preservedTlsSlot, tlsValue.slotAfter);
  assert.deepEqual(receipt.preservedTlsThreadValue, tlsValue.threadValue);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.sixteenthCall.nextTerminal !== null) {
    assert.deepEqual(continuation.terminal, expected.sixteenthCall.nextTerminal);
  }
  if (expected.sixteenthCall.instructionCount !== null) {
    assert.equal(continuation.instructionCount, expected.sixteenthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.sixteenthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.sixteenthCall.budgetStops);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      arguments: [sourcePointer],
      callProvenance: "direct-iat",
      stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      expectedTlsIndex: request.expectedTlsIndex,
      expectedTlsThreadId: request.expectedTlsThreadId,
      expectedTlsValueEndpoint: request.expectedTlsValueEndpoint,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    encodedPointer: receipt.encodedPointer,
    preservedTlsSlot: receipt.preservedTlsSlot,
    preservedTlsThreadValue: receipt.preservedTlsThreadValue,
    nativeObservation: controlledEncodedNull.nativeObservation,
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchSeventeenthEncodePointerCall({
  child,
  context,
  encodedNull,
  execution,
  nextMessage,
  offer,
  tlsAllocation,
  tlsValue,
}) {
  const trapped = execution.finalState;
  const argumentBytes = 4;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4 + argumentBytes,
    "read",
    "second EncodePointer stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "second EncodePointer return address",
  );
  const sourcePointer = context.readVirtualU32(
    trapped.esp + 4,
    "second EncodePointer source pointer",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(sourcePointer, controlledEncodedProcessInitializer.sourcePointer);

  const callSequence = context.readVirtualBytes(
    returnAddress - 11,
    11,
    "second EncodePointer push-function / direct IAT call sequence",
  );
  assert.equal(callSequence[0], 0x68);
  const callView = new DataView(
    callSequence.buffer,
    callSequence.byteOffset,
    callSequence.byteLength,
  );
  assert.equal(callView.getUint32(1, true), sourcePointer);
  assert.deepEqual([...callSequence.subarray(5, 7)], [0xff, 0x15]);
  assert.equal(callView.getUint32(7, true), execution.terminal.thunk.iatAddress);
  assert.equal(
    context.readVirtualU32(
      execution.terminal.thunk.iatAddress,
      "second EncodePointer bound IAT",
    ),
    execution.terminal.thunk.address,
  );

  const sourcePages = context.preflightVirtualRange(
    sourcePointer,
    controlledEncodedProcessInitializer.sourceFunctionPrefix.length,
    "execute",
    "process-initializer source function",
  );
  assert.equal(sourcePages.length, 1);
  assert.equal(sourcePages[0].address, controlledEncodedProcessInitializer.sourcePageAddress);
  assert.equal(sourcePages[0].owner, expected.image.owner);
  assert.equal(sourcePages[0].protection.execute, true);
  const sourceFunctionPrefix = context.readVirtualBytes(
    sourcePointer,
    controlledEncodedProcessInitializer.sourceFunctionPrefix.length,
    "process-initializer source function prefix",
  );
  assert.deepEqual(
    [...sourceFunctionPrefix],
    controlledEncodedProcessInitializer.sourceFunctionPrefix,
  );
  const sourceFunctionPrefixSha256 = sha256(sourceFunctionPrefix);
  if (expected.seventeenthCall.sourceFunctionPrefixSha256 !== null) {
    assert.equal(
      sourceFunctionPrefixSha256,
      expected.seventeenthCall.sourceFunctionPrefixSha256,
    );
  }

  const endpointStoreAddresses = tlsAllocation.request.endpointStoreAddresses;
  const endpointAddresses = tlsAllocation.request.endpointAddresses;
  for (let index = 0; index < endpointStoreAddresses.length; index += 1) {
    assert.equal(
      context.readVirtualU32(
        endpointStoreAddresses[index],
        `preserved dynamic endpoint ${index + 1} before second EncodePointer`,
      ),
      endpointAddresses[index],
    );
  }
  assert.equal(
    context.readVirtualU32(
      tlsValue.request.tlsIndexStoreAddress,
      "logical TLS index before second EncodePointer",
    ),
    controlledTlsValue.index.value,
  );

  const encodedStoreAddress = controlledEncodedProcessInitializer.encodedStoreAddress;
  const encodedStorePages = context.preflightVirtualRange(
    encodedStoreAddress,
    4,
    "write",
    "process-initializer encoded pointer store",
  );
  assert.equal(encodedStorePages.length, 1);
  assert.equal(encodedStorePages[0].owner, expected.image.owner);
  const encodedStoreBefore = context.readVirtualU32(
    encodedStoreAddress,
    "process-initializer encoded pointer before second EncodePointer return",
  );
  assert.equal(encodedStoreBefore, expected.seventeenthCall.encodedStoreValueBefore);

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before second EncodePointer return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-encode-pointer-second-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.sixthCall.deltaGeneration,
    thunkId: execution.terminal.thunk.id,
    thunkAddress: execution.terminal.thunk.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes,
    sourcePointer,
    sourcePageAddress: sourcePages[0].address,
    sourcePageOwner: sourcePages[0].owner,
    sourceFunctionPrefix: [...sourceFunctionPrefix],
    stackPageAddress,
    stackPageOwner: framePages[0].owner,
    stackPageBytes: [...stackPageBytes],
    pointerEncodingOwner: controlledEncodedProcessInitializer.owner,
    variantId: controlledEncodedProcessInitializer.variantId,
    encodedPointerNamespace: controlledEncodedProcessInitializer.namespace,
    encodedPointerValue: controlledEncodedProcessInitializer.value,
    expectedEncodedNullNamespace: controlledEncodedNull.namespace,
    expectedEncodedNullValue: controlledEncodedNull.value,
    expectedTlsIndex: controlledTlsValue.index.value,
    expectedTlsThreadId: controlledTlsValue.threadIdentity.threadId,
    expectedTlsValueEndpoint: controlledTlsValue.endpoint.address,
    preserveLastError: controlledEncodedProcessInitializer.preserveLastError,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 48 * 1024);
  if (expected.seventeenthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.seventeenthCall.requestLineBytes);
  }
  child.stdin.end(requestLine);

  const receiptMessage = await nextMessage("second EncodePointer return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-encode-pointer-second-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.sixteenthCall.nextTerminal.thunk, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, argumentBytes);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.deepEqual(receipt.sourceFunctionPrefix, [...sourceFunctionPrefix]);
  const encodedPointer = {
    owner: controlledEncodedProcessInitializer.owner,
    variantId: controlledEncodedProcessInitializer.variantId,
    namespace: controlledEncodedProcessInitializer.namespace,
    value: controlledEncodedProcessInitializer.value,
    sourceKind: controlledEncodedProcessInitializer.sourceKind,
    sourcePointer: controlledEncodedProcessInitializer.sourcePointer,
    lifetime: controlledEncodedProcessInitializer.lifetime,
    processCookieMaterialization:
      controlledEncodedProcessInitializer.processCookieMaterialization,
    nativeResultImported: controlledEncodedProcessInitializer.nativeResultImported,
  };
  assert.deepEqual(receipt.encodedPointer, encodedPointer);
  assert.deepEqual(receipt.preservedEncodedNull, encodedNull.encodedPointer);
  assert.deepEqual(receipt.preservedTlsSlot, tlsValue.slotAfter);
  assert.deepEqual(receipt.preservedTlsThreadValue, tlsValue.threadValue);
  assert.equal(
    receipt.preserveLastError,
    controlledEncodedProcessInitializer.preserveLastError,
  );
  assert.equal(receipt.result, controlledEncodedProcessInitializer.value);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.sixthCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  if (expected.seventeenthCall.stackReconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.seventeenthCall.stackReconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.seventeenthCall.stackPageContentSha256);
  }
  assert.equal(receipt.stats.generation, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.sixthCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.seventeenthCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.seventeenthCall.receiptLineBytes);
  }

  const resume = resumeSecondEncodePointerStdcallResult(
    context,
    trapped,
    returnAddress,
    receipt.result,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(
    context.readVirtualU32(
      encodedStoreAddress,
      "stored process-initializer encoded pointer after second EncodePointer",
    ),
    controlledEncodedProcessInitializer.value,
  );
  const encodedStorePageAddress = (encodedStoreAddress & ~(page.size - 1)) >>> 0;
  const encodedStorePageBytes = context.readVirtualBytes(
    encodedStorePageAddress,
    page.size,
    "process-initializer encoded pointer store page after continuation",
  );
  const encodedStorePageContentSha256 = sha256(encodedStorePageBytes);
  if (expected.seventeenthCall.encodedStorePageContentSha256 !== null) {
    assert.equal(
      encodedStorePageContentSha256,
      expected.seventeenthCall.encodedStorePageContentSha256,
    );
  }
  for (let index = 0; index < endpointStoreAddresses.length; index += 1) {
    assert.equal(
      context.readVirtualU32(
        endpointStoreAddresses[index],
        `preserved dynamic endpoint ${index + 1} after second EncodePointer`,
      ),
      endpointAddresses[index],
    );
  }
  assert.equal(
    context.readVirtualU32(
      tlsValue.request.tlsIndexStoreAddress,
      "logical TLS index after second EncodePointer",
    ),
    controlledTlsValue.index.value,
  );
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.seventeenthCall.nextTerminal !== null) {
    assert.deepEqual(continuation.terminal, expected.seventeenthCall.nextTerminal);
  }
  if (expected.seventeenthCall.instructionCount !== null) {
    assert.equal(continuation.instructionCount, expected.seventeenthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.seventeenthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.seventeenthCall.budgetStops);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      arguments: [sourcePointer],
      callProvenance: "push-imm32-direct-iat",
      sourcePageAddress: request.sourcePageAddress,
      sourcePageOwner: request.sourcePageOwner,
      sourceFunctionPrefix: request.sourceFunctionPrefix,
      sourceFunctionPrefixSha256,
      sourceRole: controlledEncodedProcessInitializer.sourceRole,
      stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      expectedEncodedNullNamespace: request.expectedEncodedNullNamespace,
      expectedEncodedNullValue: request.expectedEncodedNullValue,
      expectedTlsIndex: request.expectedTlsIndex,
      expectedTlsThreadId: request.expectedTlsThreadId,
      expectedTlsValueEndpoint: request.expectedTlsValueEndpoint,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    encodedPointer: receipt.encodedPointer,
    preservedEncodedNull: receipt.preservedEncodedNull,
    preservedTlsSlot: receipt.preservedTlsSlot,
    preservedTlsThreadValue: receipt.preservedTlsThreadValue,
    encodedStore: {
      address: encodedStoreAddress,
      valueBefore: encodedStoreBefore,
      valueAfter: controlledEncodedProcessInitializer.value,
      pageAddress: encodedStorePageAddress,
      pageOwner: encodedStorePages[0].owner,
      pageContentSha256: encodedStorePageContentSha256,
    },
    nativeObservation: controlledEncodedProcessInitializer.nativeObservation,
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

function captureCpuState(context, reason) {
  context.wasm.vine_cpu_state_save();
  const words = Uint32Array.from(
    new Uint32Array(
      context.wasm.memory.buffer,
      context.wasm.vine_cpu_state_ptr(),
      context.wasm.vine_cpu_state_size() / 4,
    ),
  );
  const privateState = new DataView(context.wasm.memory.buffer);
  return {
    reason,
    errorCode: context.wasm.vine_get_stop_error_code(),
    instructionCount: privateState.getUint32(adapterPrivateOffset.instructionCounter, true),
    eax: words[stateWord.eax],
    ecx: words[stateWord.ecx],
    edx: words[stateWord.edx],
    ebx: words[stateWord.ebx],
    esp: words[stateWord.esp],
    ebp: words[stateWord.ebp],
    esi: words[stateWord.esi],
    edi: words[stateWord.edi],
    flags: words[stateWord.flags],
    faultAddress: words[stateWord.controlRegisterBase + 2],
    instructionPointer: words[stateWord.instructionPointer],
    previousInstructionPointer: words[stateWord.previousInstructionPointer],
    fsSelector: words[stateWord.segmentSelectorBase + segmentIndex.fs],
    fsBase: words[stateWord.segmentOffsetBase + segmentIndex.fs],
  };
}

function resumeVoidStdcall(context, trapped, returnAddress, argumentBytes, label) {
  assert.equal(argumentBytes, 4);
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    trapped.eax,
    label,
  );
  assert.equal(restored.resumed.eax, trapped.eax, "void API return must not fabricate EAX");
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    voidResultPolicy: "preserve-eax-checkpoint-selection",
  };
}

function resumeStdcallResult(context, trapped, returnAddress, argumentBytes, result) {
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "GetCurrentProcessId",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledProcess.owner,
  };
}

function resumeThreadIdStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 0;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "GetCurrentThreadId",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledThread.owner,
  };
}

function resumeTickCountStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 0;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "GetTickCount",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledMonotonicClock.owner,
  };
}

function resumePerformanceCounterStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 4;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "QueryPerformanceCounter",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: "BOOL-success-after-controlled-counter-write",
  };
}

function resumeHeapSetInformationStdcallResult(
  context,
  trapped,
  returnAddress,
  result,
) {
  const argumentBytes = 16;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "HeapSetInformation",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: "BOOL-success-after-controlled-process-heap-policy",
    lastErrorPolicy: "preserve-existing-thread-last-error-unmodeled",
  };
}

function resumeHeapCreateStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 12;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "HeapCreate",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledPrivateHeap.owner,
    handleNamespace: controlledPrivateHeap.handle.namespace,
    lastErrorPolicy: "preserve-existing-thread-last-error-unmodeled",
  };
}

function resumeGetModuleHandleWStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 4;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "GetModuleHandleW",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledLoadedModule.owner,
    handleNamespace: controlledLoadedModule.handle.namespace,
    lastErrorPolicy: "preserve-existing-thread-last-error-unmodeled",
  };
}

function resumeGetProcAddressStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 8;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "GetProcAddress",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledDynamicExport.owner,
    endpointNamespace: controlledDynamicExport.endpoint.namespace,
    lastErrorPolicy: "preserve-existing-thread-last-error-unmodeled",
  };
}

function resumeSecondGetProcAddressStdcallResult(
  context,
  trapped,
  returnAddress,
  result,
) {
  const argumentBytes = 8;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "second GetProcAddress",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledSecondDynamicExport.owner,
    endpointNamespace: controlledSecondDynamicExport.endpoint.namespace,
    lastErrorPolicy: "preserve-existing-thread-last-error-unmodeled",
  };
}

function resumeThirdGetProcAddressStdcallResult(
  context,
  trapped,
  returnAddress,
  result,
) {
  const argumentBytes = 8;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "third GetProcAddress",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledThirdDynamicExport.owner,
    endpointNamespace: controlledThirdDynamicExport.endpoint.namespace,
    lastErrorPolicy: "preserve-existing-thread-last-error-unmodeled",
  };
}

function resumeFourthGetProcAddressStdcallResult(
  context,
  trapped,
  returnAddress,
  result,
) {
  const argumentBytes = 8;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "fourth GetProcAddress",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledFourthDynamicExport.owner,
    endpointNamespace: controlledFourthDynamicExport.endpoint.namespace,
    lastErrorPolicy: "preserve-existing-thread-last-error-unmodeled",
  };
}

function resumeTlsAllocStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 0;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "TlsAlloc",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledTlsAllocation.owner,
    indexNamespace: controlledTlsAllocation.index.namespace,
    failureSentinel: controlledTlsAllocation.index.failureSentinel,
    lastErrorPolicy: "preserve-existing-thread-last-error-unmodeled",
  };
}

function resumeTlsSetValueStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 8;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "TlsSetValue",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledTlsValue.owner,
    indexNamespace: controlledTlsValue.index.namespace,
    valueKind: controlledTlsValue.valueKind,
    endpointNamespace: controlledTlsValue.endpoint.namespace,
    lastErrorPolicy: "preserve-existing-thread-last-error-unmodeled",
  };
}

function resumeEncodePointerStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 4;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "EncodePointer",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledEncodedNull.owner,
    encodedPointerNamespace: controlledEncodedNull.namespace,
    sourceKind: controlledEncodedNull.sourceKind,
    processCookieMaterialization: controlledEncodedNull.processCookieMaterialization,
    lastErrorPolicy: "preserve-existing-thread-last-error-unmodeled",
  };
}

function resumeSecondEncodePointerStdcallResult(
  context,
  trapped,
  returnAddress,
  result,
) {
  const argumentBytes = 4;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "second EncodePointer",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledEncodedProcessInitializer.owner,
    encodedPointerNamespace: controlledEncodedProcessInitializer.namespace,
    sourceKind: controlledEncodedProcessInitializer.sourceKind,
    sourceRole: controlledEncodedProcessInitializer.sourceRole,
    processCookieMaterialization:
      controlledEncodedProcessInitializer.processCookieMaterialization,
    lastErrorPolicy: "preserve-existing-thread-last-error-unmodeled",
  };
}

function restoreStdcall(context, trapped, returnAddress, argumentBytes, eax, label) {
  context.wasm.vine_cpu_state_save();
  const words = new Uint32Array(
    context.wasm.memory.buffer,
    context.wasm.vine_cpu_state_ptr(),
    context.wasm.vine_cpu_state_size() / 4,
  );
  assert.equal(words[stateWord.instructionPointer], trapped.instructionPointer);
  assert.equal(words[stateWord.previousInstructionPointer], trapped.previousInstructionPointer);
  assert.equal(words[stateWord.esp], trapped.esp);
  assert.equal(words[stateWord.eax], trapped.eax);
  const resumedEsp = checkedGuestEnd(trapped.esp, 4 + argumentBytes, `${label} stdcall cleanup`);
  words[stateWord.eax] = eax >>> 0;
  words[stateWord.esp] = resumedEsp;
  words[stateWord.instructionPointer] = returnAddress;
  words[stateWord.previousInstructionPointer] = returnAddress;
  assert.equal(context.wasm.vine_cpu_state_restore(), 0);
  const resumed = captureCpuState(context, 0);
  assert.equal(resumed.eax, eax >>> 0);
  assert.equal(resumed.esp, resumedEsp);
  assert.equal(resumed.instructionPointer, returnAddress);
  assert.equal(resumed.previousInstructionPointer, returnAddress);
  return { resumed, resumedEsp };
}

function executeToNextOwnedTrap(offer, context, initialInstructionCount) {
  const executablePages = new Set(
    offer.reflection.pages
      .filter(({ ownerSealed, protection }) => ownerSealed && protection.execute)
      .map(({ address }) => address),
  );
  const thunksByAddress = new Map(offer.thunks.map((thunk) => [thunk.address, thunk]));
  const dynamicExportThunks =
    offer.dynamicExportThunks ??
    (offer.dynamicExportThunk === undefined ? [] : [offer.dynamicExportThunk]);
  const dynamicExportsByAddress = new Map(
    dynamicExportThunks.map((endpoint) => [endpoint.address, endpoint]),
  );
  const sentinelsByAddress = new Map([
    [offer.callbackReturnSentinel.address, "callback-return-sentinel"],
    [offer.processEntryReturnSentinel.address, "process-entry-return-sentinel"],
  ]);
  let budgetStops = 0;
  let executeStops = 0;
  let terminal = null;
  let finalState = captureCpuState(context, 0);
  assert.equal(finalState.instructionCount, initialInstructionCount);
  for (let index = 0; index < guest.maximumExecuteStops; index += 1) {
    const before = captureCpuState(context, 0);
    const fetchWindow = executableFetchWindow(before.instructionPointer, executablePages);
    if (!fetchWindow.allowed) {
      terminal = {
        kind: "logical-execute-gate",
        stage: "instruction-fetch",
        code: "cross-page-execute-permission-unimplemented",
        instructionPointer: before.instructionPointer,
        deniedPage: fetchWindow.deniedPage,
        checkedBytes: 15,
      };
      finalState = before;
      break;
    }
    const reason = context.wasm.vine_execute_budget(guest.instructionBudget);
    executeStops += 1;
    const state = captureCpuState(context, reason);
    finalState = state;
    if (reason === stopReason.budget) {
      budgetStops += 1;
      continue;
    }
    terminal = classifyTerminal(
      state,
      thunksByAddress,
      dynamicExportsByAddress,
      sentinelsByAddress,
      context,
    );
    break;
  }
  assert(terminal, "BGI continuation exhausted its bounded window without a terminal stop");
  return {
    instructionBudget: guest.instructionBudget,
    maximumExecuteStops: guest.maximumExecuteStops,
    maximumAdditionalInstructions: guest.instructionBudget * guest.maximumExecuteStops,
    executeStops,
    budgetStops,
    initialInstructionCount,
    additionalInstructionCount: finalState.instructionCount - initialInstructionCount,
    instructionCount: finalState.instructionCount,
    finalState,
    terminal,
    hostImportCalls: context.hostImportCalls,
  };
}

function buildFirstSystemTimeReport({
  context,
  execution,
  invalidationCalls,
  offer,
  offerMessage,
  ownerSummaries,
  receipt,
  receiptMessage,
  sourceAfter,
  sourceBefore,
  systemTime,
}) {
  return {
    schema: "vine.v86-bgi-first-win32-call",
    schemaVersion: 1,
    source: offer.source,
    sourceSnapshot: {
      immutableAcrossRun: true,
      before: sourceBefore,
      after: sourceAfter,
    },
    contract: {
      largeImageReflection: {
        maximumReflectionPages: protocol.maximumReflectionPages,
        maximumProtocolLineBytes: protocol.maximumLineBytes,
      },
      commandLine: expected.processEnvironment.commandLine,
      clockOwner: controlledClock.owner,
      clockSampleId: controlledClock.sampleId,
      oneSupportedServiceDispatch: true,
      stopAtNextOwnedTrap: true,
    },
    bootstrap: {
      imageLoadBase: offer.imageLoadBase,
      entryPoint: offer.entryPointAddress,
      initialThreadStack: offer.initialThreadStack,
      initialProcessEnvironment: offer.initialProcessEnvironment,
      fsSelector: 0,
      fsBase: offer.initialProcessEnvironment.tebAddress,
    },
    imports: {
      total: offer.thunks.length,
      declared: offer.thunks.filter(({ disposition }) => disposition === undefined).length,
      unsupported: offer.thunks.filter(({ disposition }) => disposition === "unsupported-trap")
        .length,
      first: offer.thunks[0],
      supportedSystemTime: offer.thunks[expected.terminal.thunk.id - 1],
      last: offer.thunks.at(-1),
    },
    adapter: {
      wasm: expected.wasm,
      physicalPoolBytes: guest.physicalPoolBytes,
      excludedPhysicalRanges: guest.excludedPhysicalRanges,
      pageDirectory: context.pageDirectory,
      pageTables: [...context.pageTables.entries()].map(
        ([directoryIndex, physicalAddress]) => ({ directoryIndex, physicalAddress }),
      ),
      allocatedPages: context.allocations.length,
      reflectedPageAllocations: context.allocations.filter(({ role }) =>
        role.startsWith("reflected-owner-"),
      ).length,
    },
    reflection: {
      offerLineBytes: offerMessage.lineBytes,
      receiptLineBytes: receiptMessage.lineBytes,
      pageSize: offer.reflection.pageSize,
      pages: offer.reflection.pages.length,
      contentSha256: offer.reflection.contentSha256,
      ownerSummaries,
      invalidation: offer.reflection.invalidation,
      acknowledgement: {
        generation: receipt.acknowledgedGeneration,
        callsBeforeAck: invalidationCalls,
        pendingAfterAck: receipt.stats.pendingInvalidation ?? null,
      },
    },
    firstTrap: execution,
    systemTime: {
      request: systemTime.request,
      clock: systemTime.clock,
      delta: systemTime.delta,
      receipt: systemTime.receipt,
      resume: systemTime.resume,
    },
    continuation: systemTime.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from one checked GetSystemTimeAsFileTime call and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildSecondProcessIdReport({ processIdentity, ...firstCallArguments }) {
  const firstCall = buildFirstSystemTimeReport(firstCallArguments);
  const { oneSupportedServiceDispatch: _firstCallOnly, ...sharedContract } = firstCall.contract;
  return {
    schema: "vine.v86-bgi-second-win32-call",
    schemaVersion: 1,
    source: firstCall.source,
    sourceSnapshot: firstCall.sourceSnapshot,
    contract: {
      ...sharedContract,
      processIdentityOwner: controlledProcess.owner,
      supportedServiceDispatches: 2,
    },
    bootstrap: firstCall.bootstrap,
    imports: {
      ...firstCall.imports,
      supportedProcessId:
        firstCallArguments.offer.thunks[expected.firstCall.nextThunk.identity.id - 1],
    },
    adapter: firstCall.adapter,
    reflection: firstCall.reflection,
    firstTrap: firstCall.firstTrap,
    systemTime: firstCall.systemTime,
    firstContinuation: firstCall.continuation,
    processId: {
      request: processIdentity.request,
      identity: processIdentity.identity,
      receipt: processIdentity.receipt,
      resume: processIdentity.resume,
    },
    continuation: processIdentity.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from checked GetSystemTimeAsFileTime and GetCurrentProcessId calls and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildThirdThreadIdReport({ threadIdentity, ...secondCallArguments }) {
  const secondCall = buildSecondProcessIdReport(secondCallArguments);
  return {
    schema: "vine.v86-bgi-third-win32-call",
    schemaVersion: 1,
    source: secondCall.source,
    sourceSnapshot: secondCall.sourceSnapshot,
    contract: {
      ...secondCall.contract,
      threadIdentityOwner: controlledThread.owner,
      supportedServiceDispatches: 3,
    },
    bootstrap: secondCall.bootstrap,
    imports: {
      ...secondCall.imports,
      supportedThreadId:
        secondCallArguments.offer.thunks[expected.secondCall.nextThunk.identity.id - 1],
    },
    adapter: secondCall.adapter,
    reflection: secondCall.reflection,
    firstTrap: secondCall.firstTrap,
    systemTime: secondCall.systemTime,
    firstContinuation: secondCall.firstContinuation,
    processId: secondCall.processId,
    secondContinuation: secondCall.continuation,
    threadId: {
      request: threadIdentity.request,
      identity: threadIdentity.identity,
      receipt: threadIdentity.receipt,
      resume: threadIdentity.resume,
    },
    continuation: threadIdentity.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from checked GetSystemTimeAsFileTime, GetCurrentProcessId, and GetCurrentThreadId calls and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildFourthTickCountReport({ monotonicClock, ...thirdCallArguments }) {
  const thirdCall = buildThirdThreadIdReport(thirdCallArguments);
  return {
    schema: "vine.v86-bgi-fourth-win32-call",
    schemaVersion: 1,
    source: thirdCall.source,
    sourceSnapshot: thirdCall.sourceSnapshot,
    contract: {
      ...thirdCall.contract,
      monotonicClockOwner: controlledMonotonicClock.owner,
      monotonicClockSampleId: controlledMonotonicClock.sampleId,
      supportedServiceDispatches: 4,
    },
    bootstrap: thirdCall.bootstrap,
    imports: {
      ...thirdCall.imports,
      supportedTickCount:
        thirdCallArguments.offer.thunks[expected.thirdCall.nextThunk.identity.id - 1],
    },
    adapter: thirdCall.adapter,
    reflection: thirdCall.reflection,
    firstTrap: thirdCall.firstTrap,
    systemTime: thirdCall.systemTime,
    firstContinuation: thirdCall.firstContinuation,
    processId: thirdCall.processId,
    secondContinuation: thirdCall.secondContinuation,
    threadId: thirdCall.threadId,
    thirdContinuation: thirdCall.continuation,
    tickCount: {
      request: monotonicClock.request,
      clock: monotonicClock.clock,
      receipt: monotonicClock.receipt,
      resume: monotonicClock.resume,
    },
    continuation: monotonicClock.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from checked GetSystemTimeAsFileTime, GetCurrentProcessId, GetCurrentThreadId, and GetTickCount calls and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildFifthPerformanceCounterReport({ performanceCounter, ...fourthCallArguments }) {
  const fourthCall = buildFourthTickCountReport(fourthCallArguments);
  return {
    schema: "vine.v86-bgi-fifth-win32-call",
    schemaVersion: 1,
    source: fourthCall.source,
    sourceSnapshot: fourthCall.sourceSnapshot,
    contract: {
      ...fourthCall.contract,
      performanceCounterSampleId: controlledPerformanceCounter.sampleId,
      supportedServiceDispatches: 5,
    },
    bootstrap: fourthCall.bootstrap,
    imports: {
      ...fourthCall.imports,
      supportedPerformanceCounter:
        fourthCallArguments.offer.thunks[expected.fourthCall.nextThunk.identity.id - 1],
    },
    adapter: fourthCall.adapter,
    reflection: fourthCall.reflection,
    firstTrap: fourthCall.firstTrap,
    systemTime: fourthCall.systemTime,
    firstContinuation: fourthCall.firstContinuation,
    processId: fourthCall.processId,
    secondContinuation: fourthCall.secondContinuation,
    threadId: fourthCall.threadId,
    thirdContinuation: fourthCall.thirdContinuation,
    tickCount: fourthCall.tickCount,
    fourthContinuation: fourthCall.continuation,
    performanceCounter: {
      request: performanceCounter.request,
      clock: performanceCounter.clock,
      delta: performanceCounter.delta,
      receipt: performanceCounter.receipt,
      resume: performanceCounter.resume,
    },
    continuation: performanceCounter.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from checked GetSystemTimeAsFileTime, GetCurrentProcessId, GetCurrentThreadId, GetTickCount, and QueryPerformanceCounter calls and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildSixthStartupInfoReport({ processStartup, ...fifthCallArguments }) {
  const fifthCall = buildFifthPerformanceCounterReport(fifthCallArguments);
  return {
    schema: "vine.v86-bgi-sixth-win32-call",
    schemaVersion: 1,
    source: fifthCall.source,
    sourceSnapshot: fifthCall.sourceSnapshot,
    contract: {
      ...fifthCall.contract,
      processStartupOwner: controlledProcessStartup.owner,
      startupInfoProfileId: controlledProcessStartup.profileId,
      supportedServiceDispatches: 6,
    },
    bootstrap: fifthCall.bootstrap,
    imports: {
      ...fifthCall.imports,
      supportedStartupInfo:
        fifthCallArguments.offer.thunks[expected.fifthCall.nextThunk.identity.id - 1],
    },
    adapter: fifthCall.adapter,
    reflection: fifthCall.reflection,
    firstTrap: fifthCall.firstTrap,
    systemTime: fifthCall.systemTime,
    firstContinuation: fifthCall.firstContinuation,
    processId: fifthCall.processId,
    secondContinuation: fifthCall.secondContinuation,
    threadId: fifthCall.threadId,
    thirdContinuation: fifthCall.thirdContinuation,
    tickCount: fifthCall.tickCount,
    fourthContinuation: fifthCall.fourthContinuation,
    performanceCounter: fifthCall.performanceCounter,
    fifthContinuation: fifthCall.continuation,
    startupInfo: {
      request: processStartup.request,
      profile: processStartup.profile,
      delta: processStartup.delta,
      receipt: processStartup.receipt,
      resume: processStartup.resume,
    },
    continuation: processStartup.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from checked GetSystemTimeAsFileTime, GetCurrentProcessId, GetCurrentThreadId, GetTickCount, QueryPerformanceCounter, and GetStartupInfoW calls and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildSeventhHeapSetInformationReport({ heapPolicy, ...sixthCallArguments }) {
  const sixthCall = buildSixthStartupInfoReport(sixthCallArguments);
  return {
    schema: "vine.v86-bgi-seventh-win32-call",
    schemaVersion: 1,
    source: sixthCall.source,
    sourceSnapshot: sixthCall.sourceSnapshot,
    contract: {
      ...sixthCall.contract,
      processHeapPolicyOwner: controlledHeapPolicy.owner,
      heapPolicyId: controlledHeapPolicy.policyId,
      supportedServiceDispatches: 7,
    },
    bootstrap: sixthCall.bootstrap,
    imports: {
      ...sixthCall.imports,
      supportedHeapSetInformation:
        sixthCallArguments.offer.thunks[expected.sixthCall.nextThunk.identity.id - 1],
    },
    adapter: sixthCall.adapter,
    reflection: sixthCall.reflection,
    firstTrap: sixthCall.firstTrap,
    systemTime: sixthCall.systemTime,
    firstContinuation: sixthCall.firstContinuation,
    processId: sixthCall.processId,
    secondContinuation: sixthCall.secondContinuation,
    threadId: sixthCall.threadId,
    thirdContinuation: sixthCall.thirdContinuation,
    tickCount: sixthCall.tickCount,
    fourthContinuation: sixthCall.fourthContinuation,
    performanceCounter: sixthCall.performanceCounter,
    fifthContinuation: sixthCall.fifthContinuation,
    startupInfo: sixthCall.startupInfo,
    sixthContinuation: sixthCall.continuation,
    heapSetInformation: {
      request: heapPolicy.request,
      policy: heapPolicy.policy,
      receipt: heapPolicy.receipt,
      resume: heapPolicy.resume,
    },
    continuation: heapPolicy.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from checked GetSystemTimeAsFileTime, GetCurrentProcessId, GetCurrentThreadId, GetTickCount, QueryPerformanceCounter, GetStartupInfoW, and HeapSetInformation calls and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildEighthHeapCreateReport({ privateHeap, ...seventhCallArguments }) {
  const seventhCall = buildSeventhHeapSetInformationReport(seventhCallArguments);
  return {
    schema: "vine.v86-bgi-eighth-win32-call",
    schemaVersion: 1,
    source: seventhCall.source,
    sourceSnapshot: seventhCall.sourceSnapshot,
    contract: {
      ...seventhCall.contract,
      privateHeapStateOwner: controlledPrivateHeap.owner,
      logicalHeapHandleNamespace: controlledPrivateHeap.handle.namespace,
      supportedServiceDispatches: 8,
    },
    bootstrap: seventhCall.bootstrap,
    imports: {
      ...seventhCall.imports,
      supportedHeapCreate:
        seventhCallArguments.offer.thunks[expected.seventhCall.nextThunk.identity.id - 1],
    },
    adapter: seventhCall.adapter,
    reflection: seventhCall.reflection,
    firstTrap: seventhCall.firstTrap,
    systemTime: seventhCall.systemTime,
    firstContinuation: seventhCall.firstContinuation,
    processId: seventhCall.processId,
    secondContinuation: seventhCall.secondContinuation,
    threadId: seventhCall.threadId,
    thirdContinuation: seventhCall.thirdContinuation,
    tickCount: seventhCall.tickCount,
    fourthContinuation: seventhCall.fourthContinuation,
    performanceCounter: seventhCall.performanceCounter,
    fifthContinuation: seventhCall.fifthContinuation,
    startupInfo: seventhCall.startupInfo,
    sixthContinuation: seventhCall.sixthContinuation,
    heapSetInformation: seventhCall.heapSetInformation,
    seventhContinuation: seventhCall.continuation,
    heapCreate: {
      request: privateHeap.request,
      heap: privateHeap.heap,
      receipt: privateHeap.receipt,
      resume: privateHeap.resume,
    },
    continuation: privateHeap.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from eight checked Kernel32 calls through HeapCreate and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildNinthGetModuleHandleWReport({ loadedModule, ...eighthCallArguments }) {
  const eighthCall = buildEighthHeapCreateReport(eighthCallArguments);
  return {
    schema: "vine.v86-bgi-ninth-win32-call",
    schemaVersion: 1,
    source: eighthCall.source,
    sourceSnapshot: eighthCall.sourceSnapshot,
    contract: {
      ...eighthCall.contract,
      loadedModuleOwner: controlledLoadedModule.owner,
      logicalLoadedModuleHandleNamespace: controlledLoadedModule.handle.namespace,
      supportedServiceDispatches: 9,
    },
    bootstrap: eighthCall.bootstrap,
    imports: {
      ...eighthCall.imports,
      supportedGetModuleHandleW:
        eighthCallArguments.offer.thunks[expected.eighthCall.nextThunk.identity.id - 1],
    },
    adapter: eighthCall.adapter,
    reflection: eighthCall.reflection,
    firstTrap: eighthCall.firstTrap,
    systemTime: eighthCall.systemTime,
    firstContinuation: eighthCall.firstContinuation,
    processId: eighthCall.processId,
    secondContinuation: eighthCall.secondContinuation,
    threadId: eighthCall.threadId,
    thirdContinuation: eighthCall.thirdContinuation,
    tickCount: eighthCall.tickCount,
    fourthContinuation: eighthCall.fourthContinuation,
    performanceCounter: eighthCall.performanceCounter,
    fifthContinuation: eighthCall.fifthContinuation,
    startupInfo: eighthCall.startupInfo,
    sixthContinuation: eighthCall.sixthContinuation,
    heapSetInformation: eighthCall.heapSetInformation,
    seventhContinuation: eighthCall.seventhContinuation,
    heapCreate: eighthCall.heapCreate,
    eighthContinuation: eighthCall.continuation,
    getModuleHandleW: {
      request: loadedModule.request,
      module: loadedModule.module,
      receipt: loadedModule.receipt,
      resume: loadedModule.resume,
    },
    continuation: loadedModule.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from nine checked Kernel32 calls through GetModuleHandleW and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildTenthGetProcAddressReport({ dynamicExport, ...ninthCallArguments }) {
  const ninthCall = buildNinthGetModuleHandleWReport(ninthCallArguments);
  return {
    schema: "vine.v86-bgi-tenth-win32-call",
    schemaVersion: 1,
    source: ninthCall.source,
    sourceSnapshot: ninthCall.sourceSnapshot,
    contract: {
      ...ninthCall.contract,
      dynamicExportOwner: controlledDynamicExport.owner,
      dynamicExportEndpointNamespace: controlledDynamicExport.endpoint.namespace,
      hostFunctionAddressImported: false,
      supportedServiceDispatches: 10,
    },
    bootstrap: ninthCall.bootstrap,
    imports: {
      ...ninthCall.imports,
      supportedGetProcAddress:
        ninthCallArguments.offer.thunks[expected.ninthCall.nextThunk.identity.id - 1],
    },
    dynamicExportEndpoint: ninthCallArguments.offer.dynamicExportThunk,
    adapter: ninthCall.adapter,
    reflection: ninthCall.reflection,
    firstTrap: ninthCall.firstTrap,
    systemTime: ninthCall.systemTime,
    firstContinuation: ninthCall.firstContinuation,
    processId: ninthCall.processId,
    secondContinuation: ninthCall.secondContinuation,
    threadId: ninthCall.threadId,
    thirdContinuation: ninthCall.thirdContinuation,
    tickCount: ninthCall.tickCount,
    fourthContinuation: ninthCall.fourthContinuation,
    performanceCounter: ninthCall.performanceCounter,
    fifthContinuation: ninthCall.fifthContinuation,
    startupInfo: ninthCall.startupInfo,
    sixthContinuation: ninthCall.sixthContinuation,
    heapSetInformation: ninthCall.heapSetInformation,
    seventhContinuation: ninthCall.seventhContinuation,
    heapCreate: ninthCall.heapCreate,
    eighthContinuation: ninthCall.eighthContinuation,
    getModuleHandleW: ninthCall.getModuleHandleW,
    ninthContinuation: ninthCall.continuation,
    getProcAddress: {
      request: dynamicExport.request,
      resolution: dynamicExport.resolution,
      endpoint: dynamicExport.endpoint,
      receipt: dynamicExport.receipt,
      resume: dynamicExport.resume,
    },
    continuation: dynamicExport.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from ten checked Kernel32 calls through GetProcAddress(FlsAlloc) and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildEleventhGetProcAddressReport({
  secondDynamicExport,
  ...tenthCallArguments
}) {
  const tenthCall = buildTenthGetProcAddressReport(tenthCallArguments);
  return {
    schema: "vine.v86-bgi-eleventh-win32-call",
    schemaVersion: 1,
    source: tenthCall.source,
    sourceSnapshot: tenthCall.sourceSnapshot,
    contract: {
      ...tenthCall.contract,
      dynamicExportEndpoints: 2,
      firstDynamicExportPreservedInGuestMemory: true,
      supportedServiceDispatches: 11,
    },
    bootstrap: tenthCall.bootstrap,
    imports: tenthCall.imports,
    dynamicExportEndpoints: tenthCallArguments.offer.dynamicExportThunks,
    adapter: tenthCall.adapter,
    reflection: tenthCall.reflection,
    firstTrap: tenthCall.firstTrap,
    systemTime: tenthCall.systemTime,
    firstContinuation: tenthCall.firstContinuation,
    processId: tenthCall.processId,
    secondContinuation: tenthCall.secondContinuation,
    threadId: tenthCall.threadId,
    thirdContinuation: tenthCall.thirdContinuation,
    tickCount: tenthCall.tickCount,
    fourthContinuation: tenthCall.fourthContinuation,
    performanceCounter: tenthCall.performanceCounter,
    fifthContinuation: tenthCall.fifthContinuation,
    startupInfo: tenthCall.startupInfo,
    sixthContinuation: tenthCall.sixthContinuation,
    heapSetInformation: tenthCall.heapSetInformation,
    seventhContinuation: tenthCall.seventhContinuation,
    heapCreate: tenthCall.heapCreate,
    eighthContinuation: tenthCall.eighthContinuation,
    getModuleHandleW: tenthCall.getModuleHandleW,
    ninthContinuation: tenthCall.ninthContinuation,
    firstGetProcAddress: tenthCall.getProcAddress,
    tenthContinuation: tenthCall.continuation,
    secondGetProcAddress: {
      request: secondDynamicExport.request,
      resolution: secondDynamicExport.resolution,
      firstEndpoint: secondDynamicExport.firstEndpoint,
      endpoint: secondDynamicExport.endpoint,
      receipt: secondDynamicExport.receipt,
      resume: secondDynamicExport.resume,
      endpointStore: secondDynamicExport.endpointStore,
    },
    continuation: secondDynamicExport.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from eleven checked Kernel32 calls through GetProcAddress(FlsGetValue), preserved both dynamic endpoints, and reached GetProcAddress(FlsSetValue) without dispatching it",
    },
  };
}

function buildTwelfthGetProcAddressReport({
  thirdDynamicExport,
  ...eleventhCallArguments
}) {
  const eleventhCall = buildEleventhGetProcAddressReport(eleventhCallArguments);
  return {
    schema: "vine.v86-bgi-twelfth-win32-call",
    schemaVersion: 1,
    source: eleventhCall.source,
    sourceSnapshot: eleventhCall.sourceSnapshot,
    contract: {
      ...eleventhCall.contract,
      dynamicExportEndpoints: 3,
      firstTwoDynamicExportsPreservedInGuestMemory: true,
      supportedServiceDispatches: 12,
    },
    bootstrap: eleventhCall.bootstrap,
    imports: eleventhCall.imports,
    dynamicExportEndpoints: eleventhCallArguments.offer.dynamicExportThunks,
    adapter: eleventhCall.adapter,
    reflection: eleventhCall.reflection,
    firstTrap: eleventhCall.firstTrap,
    systemTime: eleventhCall.systemTime,
    firstContinuation: eleventhCall.firstContinuation,
    processId: eleventhCall.processId,
    secondContinuation: eleventhCall.secondContinuation,
    threadId: eleventhCall.threadId,
    thirdContinuation: eleventhCall.thirdContinuation,
    tickCount: eleventhCall.tickCount,
    fourthContinuation: eleventhCall.fourthContinuation,
    performanceCounter: eleventhCall.performanceCounter,
    fifthContinuation: eleventhCall.fifthContinuation,
    startupInfo: eleventhCall.startupInfo,
    sixthContinuation: eleventhCall.sixthContinuation,
    heapSetInformation: eleventhCall.heapSetInformation,
    seventhContinuation: eleventhCall.seventhContinuation,
    heapCreate: eleventhCall.heapCreate,
    eighthContinuation: eleventhCall.eighthContinuation,
    getModuleHandleW: eleventhCall.getModuleHandleW,
    ninthContinuation: eleventhCall.ninthContinuation,
    firstGetProcAddress: eleventhCall.firstGetProcAddress,
    tenthContinuation: eleventhCall.tenthContinuation,
    secondGetProcAddress: eleventhCall.secondGetProcAddress,
    eleventhContinuation: eleventhCall.continuation,
    thirdGetProcAddress: {
      request: thirdDynamicExport.request,
      resolution: thirdDynamicExport.resolution,
      firstEndpoint: thirdDynamicExport.firstEndpoint,
      secondEndpoint: thirdDynamicExport.secondEndpoint,
      endpoint: thirdDynamicExport.endpoint,
      receipt: thirdDynamicExport.receipt,
      resume: thirdDynamicExport.resume,
      endpointStore: thirdDynamicExport.endpointStore,
    },
    continuation: thirdDynamicExport.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from twelve checked Kernel32 calls through GetProcAddress(FlsSetValue), preserved all three dynamic endpoints, and reached GetProcAddress(FlsFree) without dispatching it",
    },
  };
}

function buildThirteenthGetProcAddressReport({
  fourthDynamicExport,
  ...twelfthCallArguments
}) {
  const twelfthCall = buildTwelfthGetProcAddressReport(twelfthCallArguments);
  return {
    schema: "vine.v86-bgi-thirteenth-win32-call",
    schemaVersion: 1,
    source: twelfthCall.source,
    sourceSnapshot: twelfthCall.sourceSnapshot,
    contract: {
      ...twelfthCall.contract,
      dynamicExportEndpoints: 4,
      firstThreeDynamicExportsPreservedInGuestMemory: true,
      supportedServiceDispatches: 13,
    },
    bootstrap: twelfthCall.bootstrap,
    imports: twelfthCall.imports,
    dynamicExportEndpoints: twelfthCallArguments.offer.dynamicExportThunks,
    adapter: twelfthCall.adapter,
    reflection: twelfthCall.reflection,
    firstTrap: twelfthCall.firstTrap,
    systemTime: twelfthCall.systemTime,
    firstContinuation: twelfthCall.firstContinuation,
    processId: twelfthCall.processId,
    secondContinuation: twelfthCall.secondContinuation,
    threadId: twelfthCall.threadId,
    thirdContinuation: twelfthCall.thirdContinuation,
    tickCount: twelfthCall.tickCount,
    fourthContinuation: twelfthCall.fourthContinuation,
    performanceCounter: twelfthCall.performanceCounter,
    fifthContinuation: twelfthCall.fifthContinuation,
    startupInfo: twelfthCall.startupInfo,
    sixthContinuation: twelfthCall.sixthContinuation,
    heapSetInformation: twelfthCall.heapSetInformation,
    seventhContinuation: twelfthCall.seventhContinuation,
    heapCreate: twelfthCall.heapCreate,
    eighthContinuation: twelfthCall.eighthContinuation,
    getModuleHandleW: twelfthCall.getModuleHandleW,
    ninthContinuation: twelfthCall.ninthContinuation,
    firstGetProcAddress: twelfthCall.firstGetProcAddress,
    tenthContinuation: twelfthCall.tenthContinuation,
    secondGetProcAddress: twelfthCall.secondGetProcAddress,
    eleventhContinuation: twelfthCall.eleventhContinuation,
    thirdGetProcAddress: twelfthCall.thirdGetProcAddress,
    twelfthContinuation: twelfthCall.continuation,
    fourthGetProcAddress: {
      request: fourthDynamicExport.request,
      resolution: fourthDynamicExport.resolution,
      firstEndpoint: fourthDynamicExport.firstEndpoint,
      secondEndpoint: fourthDynamicExport.secondEndpoint,
      thirdEndpoint: fourthDynamicExport.thirdEndpoint,
      endpoint: fourthDynamicExport.endpoint,
      receipt: fourthDynamicExport.receipt,
      resume: fourthDynamicExport.resume,
      endpointStore: fourthDynamicExport.endpointStore,
    },
    continuation: fourthDynamicExport.continuation,
    stop: {
      stage: "runtime-trap",
      code: "next-runtime-reachable-owned-trap",
      message:
        "BGI returned from thirteen checked Kernel32 calls through GetProcAddress(FlsFree), preserved all four dynamic endpoints, and reached the next identity-owned runtime trap without dispatching it",
    },
  };
}

function buildFourteenthTlsAllocReport({ tlsAllocation, ...thirteenthCallArguments }) {
  const thirteenthCall = buildThirteenthGetProcAddressReport(thirteenthCallArguments);
  return {
    ...thirteenthCall,
    schema: "vine.v86-bgi-fourteenth-win32-call",
    contract: {
      ...thirteenthCall.contract,
      threadLocalStorageOwner: controlledTlsAllocation.owner,
      logicalTlsIndexNamespace: controlledTlsAllocation.index.namespace,
      allFourDynamicExportsPreservedInGuestMemory: true,
      supportedServiceDispatches: 14,
    },
    imports: {
      ...thirteenthCall.imports,
      supportedTlsAlloc:
        thirteenthCallArguments.offer.thunks[
          expected.thirteenthCall.nextTerminal.thunk.id - 1
        ],
    },
    thirteenthContinuation: thirteenthCall.continuation,
    tlsAlloc: {
      request: tlsAllocation.request,
      slot: tlsAllocation.slot,
      nativeObservation: tlsAllocation.nativeObservation,
      receipt: tlsAllocation.receipt,
      resume: tlsAllocation.resume,
    },
    continuation: tlsAllocation.continuation,
    stop: {
      stage: "runtime-trap",
      code: "next-runtime-reachable-owned-trap",
      message:
        "BGI returned from fourteen checked Kernel32 calls through TlsAlloc, preserved all four dynamic endpoints, and reached the next identity-owned runtime trap without dispatching it",
    },
  };
}

function buildFifteenthTlsSetValueReport({ tlsValue, ...fourteenthCallArguments }) {
  const fourteenthCall = buildFourteenthTlsAllocReport(fourteenthCallArguments);
  return {
    ...fourteenthCall,
    schema: "vine.v86-bgi-fifteenth-win32-call",
    contract: {
      ...fourteenthCall.contract,
      controlledInitialThreadTlsValue: true,
      tlsValueKind: controlledTlsValue.valueKind,
      tlsValueEndpointNamespace: controlledTlsValue.endpoint.namespace,
      threePageReconciliation: true,
      supportedServiceDispatches: 15,
    },
    imports: {
      ...fourteenthCall.imports,
      supportedTlsSetValue:
        fourteenthCallArguments.offer.thunks[
          expected.fourteenthCall.nextTerminal.thunk.id - 1
        ],
    },
    fourteenthContinuation: fourteenthCall.continuation,
    tlsSetValue: {
      request: tlsValue.request,
      slotBefore: tlsValue.slotBefore,
      slotAfter: tlsValue.slotAfter,
      threadValue: tlsValue.threadValue,
      nativeObservation: tlsValue.nativeObservation,
      receipt: tlsValue.receipt,
      resume: tlsValue.resume,
    },
    continuation: tlsValue.continuation,
    stop: {
      stage: "runtime-trap",
      code: "next-runtime-reachable-owned-trap",
      message:
        "BGI returned from fifteen checked Kernel32 calls through TlsSetValue, retained the initial-thread TLS endpoint value, and reached the next identity-owned runtime trap without dispatching it",
    },
  };
}

function buildSixteenthEncodePointerReport({ encodedNull, ...fifteenthCallArguments }) {
  const fifteenthCall = buildFifteenthTlsSetValueReport(fifteenthCallArguments);
  return {
    ...fifteenthCall,
    schema: "vine.v86-bgi-sixteenth-win32-call",
    contract: {
      ...fifteenthCall.contract,
      pointerEncodingOwner: controlledEncodedNull.owner,
      encodedPointerNamespace: controlledEncodedNull.namespace,
      encodedNullToken: controlledEncodedNull.value,
      nativeProcessCookieImported: false,
      retainedInitialThreadTlsValue: true,
      supportedServiceDispatches: 16,
    },
    imports: {
      ...fifteenthCall.imports,
      supportedEncodePointer:
        fifteenthCallArguments.offer.thunks[
          expected.fifteenthCall.nextTerminal.thunk.id - 1
        ],
    },
    fifteenthContinuation: fifteenthCall.continuation,
    encodePointer: {
      request: encodedNull.request,
      encodedPointer: encodedNull.encodedPointer,
      preservedTlsSlot: encodedNull.preservedTlsSlot,
      preservedTlsThreadValue: encodedNull.preservedTlsThreadValue,
      nativeObservation: encodedNull.nativeObservation,
      receipt: encodedNull.receipt,
      resume: encodedNull.resume,
    },
    continuation: encodedNull.continuation,
    stop: {
      stage: "runtime-trap",
      code: "next-runtime-reachable-owned-trap",
      message:
        "BGI returned from sixteen checked Kernel32 calls through EncodePointer(NULL), retained the initial-thread TLS endpoint value, and reached the next identity-owned runtime trap without dispatching it",
    },
  };
}

function buildSeventeenthEncodePointerReport({
  encodedProcessInitializer,
  ...sixteenthCallArguments
}) {
  const sixteenthCall = buildSixteenthEncodePointerReport(sixteenthCallArguments);
  return {
    ...sixteenthCall,
    schema: "vine.v86-bgi-seventeenth-win32-call",
    contract: {
      ...sixteenthCall.contract,
      encodedPointerCount: 2,
      encodedProcessInitializerToken: controlledEncodedProcessInitializer.value,
      processInitializerSourceRole: controlledEncodedProcessInitializer.sourceRole,
      executableSourceValidated: true,
      encodedResultGuestStoreObserved: true,
      nativeProcessCookieImported: false,
      supportedServiceDispatches: 17,
    },
    sixteenthContinuation: sixteenthCall.continuation,
    encodeProcessInitializer: {
      request: encodedProcessInitializer.request,
      encodedPointer: encodedProcessInitializer.encodedPointer,
      preservedEncodedNull: encodedProcessInitializer.preservedEncodedNull,
      preservedTlsSlot: encodedProcessInitializer.preservedTlsSlot,
      preservedTlsThreadValue: encodedProcessInitializer.preservedTlsThreadValue,
      encodedStore: encodedProcessInitializer.encodedStore,
      nativeObservation: encodedProcessInitializer.nativeObservation,
      receipt: encodedProcessInitializer.receipt,
      resume: encodedProcessInitializer.resume,
    },
    continuation: encodedProcessInitializer.continuation,
    stop: {
      stage: "runtime-trap",
      code: "next-runtime-reachable-owned-trap",
      message:
        "BGI returned from seventeen checked Kernel32 calls through EncodePointer(process initializer), stored the controlled encoded guest-function token, and reached the next identity-owned runtime trap without dispatching it",
    },
  };
}

function executeToFirstOwnedTrap(offer, context) {
  const executablePages = new Set(
    offer.reflection.pages
      .filter(({ ownerSealed, protection }) => ownerSealed && protection.execute)
      .map(({ address }) => address),
  );
  const thunksByAddress = new Map(offer.thunks.map((thunk) => [thunk.address, thunk]));
  const dynamicExportThunks =
    offer.dynamicExportThunks ??
    (offer.dynamicExportThunk === undefined ? [] : [offer.dynamicExportThunk]);
  const dynamicExportsByAddress = new Map(
    dynamicExportThunks.map((endpoint) => [endpoint.address, endpoint]),
  );
  const sentinelsByAddress = new Map([
    [offer.callbackReturnSentinel.address, "callback-return-sentinel"],
    [offer.processEntryReturnSentinel.address, "process-entry-return-sentinel"],
  ]);
  const saveState = (reason) => {
    context.wasm.vine_cpu_state_save();
    const words = Uint32Array.from(
      new Uint32Array(
        context.wasm.memory.buffer,
        context.wasm.vine_cpu_state_ptr(),
        context.wasm.vine_cpu_state_size() / 4,
      ),
    );
    const privateState = new DataView(context.wasm.memory.buffer);
    return {
      reason,
      errorCode: context.wasm.vine_get_stop_error_code(),
      instructionCount: privateState.getUint32(
        adapterPrivateOffset.instructionCounter,
        true,
      ),
      eax: words[stateWord.eax],
      ecx: words[stateWord.ecx],
      edx: words[stateWord.edx],
      ebx: words[stateWord.ebx],
      esp: words[stateWord.esp],
      ebp: words[stateWord.ebp],
      esi: words[stateWord.esi],
      edi: words[stateWord.edi],
      flags: words[stateWord.flags],
      faultAddress: words[stateWord.controlRegisterBase + 2],
      instructionPointer: words[stateWord.instructionPointer],
      previousInstructionPointer: words[stateWord.previousInstructionPointer],
      fsSelector: words[stateWord.segmentSelectorBase + segmentIndex.fs],
      fsBase: words[stateWord.segmentOffsetBase + segmentIndex.fs],
    };
  };

  const initial = saveState(0);
  assert.equal(initial.instructionPointer, expected.image.entry);
  assert.equal(initial.esp, expected.stack.initialEsp);
  assert.equal(initial.fsSelector, 0);
  assert.equal(initial.fsBase, expected.processEnvironment.tebBase);
  let budgetStops = 0;
  let executeStops = 0;
  let terminal = null;
  let finalState = initial;
  for (let index = 0; index < guest.maximumExecuteStops; index += 1) {
    const before = saveState(0);
    const fetchWindow = executableFetchWindow(before.instructionPointer, executablePages);
    if (!fetchWindow.allowed) {
      terminal = {
        kind: "logical-execute-gate",
        stage: "instruction-fetch",
        code: "cross-page-execute-permission-unimplemented",
        instructionPointer: before.instructionPointer,
        deniedPage: fetchWindow.deniedPage,
        checkedBytes: 15,
      };
      finalState = before;
      break;
    }
    const reason = context.wasm.vine_execute_budget(guest.instructionBudget);
    executeStops += 1;
    const state = saveState(reason);
    finalState = state;
    if (reason === stopReason.budget) {
      budgetStops += 1;
      continue;
    }
    terminal = classifyTerminal(
      state,
      thunksByAddress,
      dynamicExportsByAddress,
      sentinelsByAddress,
      context,
    );
    break;
  }
  assert(terminal, "BGI exhausted its bounded execution window without a terminal stop");
  return {
    instructionBudget: guest.instructionBudget,
    maximumExecuteStops: guest.maximumExecuteStops,
    maximumInstructions: guest.instructionBudget * guest.maximumExecuteStops,
    executeStops,
    budgetStops,
    instructionCount: finalState.instructionCount,
    finalState: {
      reason: finalState.reason,
      errorCode: finalState.errorCode,
      instructionPointer: finalState.instructionPointer,
      previousInstructionPointer: finalState.previousInstructionPointer,
      faultAddress: finalState.faultAddress,
      eax: finalState.eax,
      ecx: finalState.ecx,
      edx: finalState.edx,
      ebx: finalState.ebx,
      esp: finalState.esp,
      ebp: finalState.ebp,
      esi: finalState.esi,
      edi: finalState.edi,
      flags: finalState.flags,
      fsSelector: finalState.fsSelector,
      fsBase: finalState.fsBase,
    },
    terminal,
    hostImportCalls: context.hostImportCalls,
  };
}

function classifyTerminal(
  state,
  thunksByAddress,
  dynamicExportsByAddress,
  sentinelsByAddress,
  context,
) {
  if (state.reason === stopReason.invalidOpcode) {
    const thunk = thunksByAddress.get(state.instructionPointer);
    if (thunk !== undefined && state.previousInstructionPointer === thunk.address) {
      const stackWords = [];
      for (let index = 0; index < 8; index += 1) {
        stackWords.push(
          context.readVirtualU32(state.esp + index * 4, `trap stack word ${index}`),
        );
      }
      return {
        kind: "owned-import-thunk",
        thunk,
        returnAddress: stackWords[0],
        stackWords,
      };
    }
    const endpoint = dynamicExportsByAddress.get(state.instructionPointer);
    if (endpoint !== undefined && state.previousInstructionPointer === endpoint.address) {
      const stackWords = [];
      for (let index = 0; index < 8; index += 1) {
        stackWords.push(
          context.readVirtualU32(state.esp + index * 4, `dynamic trap stack word ${index}`),
        );
      }
      return {
        kind: "owned-dynamic-export-thunk",
        endpoint,
        returnAddress: stackWords[0],
        stackWords,
      };
    }
    const sentinel = sentinelsByAddress.get(state.instructionPointer);
    if (sentinel !== undefined && state.previousInstructionPointer === state.instructionPointer) {
      return { kind: sentinel, address: state.instructionPointer };
    }
    return {
      kind: "unowned-invalid-opcode",
      instructionPointer: state.instructionPointer,
      previousInstructionPointer: state.previousInstructionPointer,
    };
  }
  if (state.reason === stopReason.pageFault) {
    return {
      kind: "page-fault",
      instructionPointer: state.instructionPointer,
      previousInstructionPointer: state.previousInstructionPointer,
      faultAddress: state.faultAddress,
      pageFaultErrorCode: state.errorCode,
    };
  }
  return {
    kind: "cpu-stop",
    reason: state.reason,
    errorCode: state.errorCode,
    instructionPointer: state.instructionPointer,
    previousInstructionPointer: state.previousInstructionPointer,
  };
}

run().catch((error) => {
  console.error(error.stack ?? error.message);
  process.exitCode = 1;
});
