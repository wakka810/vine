import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import readline from "node:readline";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const commandArguments = process.argv.slice(2);
const dispatchSystemTimeOnly = commandArguments.includes("--dispatch-system-time");
const dispatchProcessIdOnly = commandArguments.includes("--dispatch-process-id");
const dispatchThreadIdOnly = commandArguments.includes("--dispatch-thread-id");
const dispatchTickCountOnly = commandArguments.includes("--dispatch-tick-count");
const dispatchPerformanceCounterOnly = commandArguments.includes(
  "--dispatch-performance-counter",
);
const dispatchStartupInfoOnly = commandArguments.includes("--dispatch-startup-info");
const dispatchHeapSetInformationOnly = commandArguments.includes(
  "--dispatch-heap-set-information",
);
const dispatchHeapCreateOnly = commandArguments.includes("--dispatch-heap-create");
const dispatchModuleHandleOnly = commandArguments.includes("--dispatch-module-handle");
const dispatchGetProcAddressOnly = commandArguments.includes("--dispatch-get-proc-address");
const dispatchGetProcAddressSecondOnly = commandArguments.includes(
  "--dispatch-get-proc-address-second",
);
const dispatchGetProcAddressThirdOnly = commandArguments.includes(
  "--dispatch-get-proc-address-third",
);
const dispatchGetProcAddressFourthOnly = commandArguments.includes(
  "--dispatch-get-proc-address-fourth",
);
const dispatchTlsAllocOnly = commandArguments.includes("--dispatch-tls-alloc");
const dispatchTlsSetValueOnly = commandArguments.includes("--dispatch-tls-set-value");
const dispatchEncodePointerOnly = commandArguments.includes("--dispatch-encode-pointer");
const dispatchEncodePointer = dispatchEncodePointerOnly;
const dispatchTlsSetValue = dispatchTlsSetValueOnly || dispatchEncodePointer;
const dispatchTlsAlloc = dispatchTlsAllocOnly || dispatchTlsSetValue;
const dispatchGetProcAddressFourth =
  dispatchGetProcAddressFourthOnly || dispatchTlsAlloc;
const dispatchGetProcAddressThird =
  dispatchGetProcAddressThirdOnly || dispatchGetProcAddressFourth;
const dispatchGetProcAddressSecond =
  dispatchGetProcAddressSecondOnly || dispatchGetProcAddressThird;
const dispatchGetProcAddress = dispatchGetProcAddressOnly || dispatchGetProcAddressSecond;
const dispatchModuleHandle = dispatchModuleHandleOnly || dispatchGetProcAddress;
const dispatchHeapCreate = dispatchHeapCreateOnly || dispatchModuleHandle;
const dispatchHeapSetInformation = dispatchHeapSetInformationOnly || dispatchHeapCreate;
const dispatchStartupInfo = dispatchStartupInfoOnly || dispatchHeapSetInformation;
const dispatchPerformanceCounter = dispatchPerformanceCounterOnly || dispatchStartupInfo;
const dispatchTickCount = dispatchTickCountOnly || dispatchPerformanceCounter;
const dispatchThreadId = dispatchThreadIdOnly || dispatchTickCount;
const dispatchProcessId = dispatchProcessIdOnly || dispatchThreadId;
const dispatchSystemTime = dispatchSystemTimeOnly || dispatchProcessId;
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-system-time").length,
  Number(dispatchSystemTimeOnly),
  "--dispatch-system-time may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-process-id").length,
  Number(dispatchProcessIdOnly),
  "--dispatch-process-id may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-thread-id").length,
  Number(dispatchThreadIdOnly),
  "--dispatch-thread-id may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-tick-count").length,
  Number(dispatchTickCountOnly),
  "--dispatch-tick-count may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-performance-counter").length,
  Number(dispatchPerformanceCounterOnly),
  "--dispatch-performance-counter may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-startup-info").length,
  Number(dispatchStartupInfoOnly),
  "--dispatch-startup-info may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-heap-set-information")
    .length,
  Number(dispatchHeapSetInformationOnly),
  "--dispatch-heap-set-information may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-heap-create").length,
  Number(dispatchHeapCreateOnly),
  "--dispatch-heap-create may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-module-handle").length,
  Number(dispatchModuleHandleOnly),
  "--dispatch-module-handle may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-get-proc-address").length,
  Number(dispatchGetProcAddressOnly),
  "--dispatch-get-proc-address may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-get-proc-address-second")
    .length,
  Number(dispatchGetProcAddressSecondOnly),
  "--dispatch-get-proc-address-second may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-get-proc-address-third")
    .length,
  Number(dispatchGetProcAddressThirdOnly),
  "--dispatch-get-proc-address-third may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-get-proc-address-fourth")
    .length,
  Number(dispatchGetProcAddressFourthOnly),
  "--dispatch-get-proc-address-fourth may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-tls-alloc").length,
  Number(dispatchTlsAllocOnly),
  "--dispatch-tls-alloc may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-tls-set-value").length,
  Number(dispatchTlsSetValueOnly),
  "--dispatch-tls-set-value may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-encode-pointer").length,
  Number(dispatchEncodePointerOnly),
  "--dispatch-encode-pointer may be specified only once",
);
assert(
  Number(dispatchSystemTimeOnly) +
    Number(dispatchProcessIdOnly) +
    Number(dispatchThreadIdOnly) +
    Number(dispatchTickCountOnly) +
    Number(dispatchPerformanceCounterOnly) +
    Number(dispatchStartupInfoOnly) +
    Number(dispatchHeapSetInformationOnly) +
    Number(dispatchHeapCreateOnly) +
    Number(dispatchModuleHandleOnly) +
    Number(dispatchGetProcAddressOnly) +
    Number(dispatchGetProcAddressSecondOnly) +
    Number(dispatchGetProcAddressThirdOnly) +
    Number(dispatchGetProcAddressFourthOnly) +
    Number(dispatchTlsAllocOnly) +
    Number(dispatchTlsSetValueOnly) +
    Number(dispatchEncodePointerOnly) <=
    1,
  "dispatch modes are mutually exclusive",
);
const positionalArguments = commandArguments.filter(
  (argument) =>
    argument !== "--dispatch-system-time" &&
    argument !== "--dispatch-process-id" &&
    argument !== "--dispatch-thread-id" &&
    argument !== "--dispatch-tick-count" &&
    argument !== "--dispatch-performance-counter" &&
    argument !== "--dispatch-startup-info" &&
    argument !== "--dispatch-heap-set-information" &&
    argument !== "--dispatch-heap-create" &&
    argument !== "--dispatch-module-handle" &&
    argument !== "--dispatch-get-proc-address" &&
    argument !== "--dispatch-get-proc-address-second" &&
    argument !== "--dispatch-get-proc-address-third" &&
    argument !== "--dispatch-get-proc-address-fourth" &&
    argument !== "--dispatch-tls-alloc" &&
    argument !== "--dispatch-tls-set-value" &&
    argument !== "--dispatch-encode-pointer",
);
assert(
  positionalArguments.length <= 2 && positionalArguments.every((argument) => !argument.startsWith("--")),
  "usage: run-runtime-v86-bgi-first-trap.mjs [--dispatch-system-time | --dispatch-process-id | --dispatch-thread-id | --dispatch-tick-count | --dispatch-performance-counter | --dispatch-startup-info | --dispatch-heap-set-information | --dispatch-heap-create | --dispatch-module-handle | --dispatch-get-proc-address | --dispatch-get-proc-address-second | --dispatch-get-proc-address-third | --dispatch-get-proc-address-fourth | --dispatch-tls-alloc | --dispatch-tls-set-value | --dispatch-encode-pointer] [WASM] [BGI.exe]",
);
const wasmPath = path.resolve(
  positionalArguments[0] ??
    path.join(root, ".work", "v86-adapter-source", "build", "v86-fallback.wasm"),
);
const bgiPath = path.resolve(
  positionalArguments[1] ??
    "V:\\softs\\\u30b5\u30af\u30e9\u30ce\u8a69\\BGI.exe",
);
const outputDirectory = path.join(
  root,
  ".work",
  dispatchEncodePointer
    ? "v86-bgi-sixteenth-win32-call"
    : dispatchTlsSetValue
    ? "v86-bgi-fifteenth-win32-call"
    : dispatchTlsAlloc
    ? "v86-bgi-fourteenth-win32-call"
    : dispatchGetProcAddressFourth
    ? "v86-bgi-thirteenth-win32-call"
    : dispatchGetProcAddressThird
    ? "v86-bgi-twelfth-win32-call"
    : dispatchGetProcAddressSecond
    ? "v86-bgi-eleventh-win32-call"
    : dispatchGetProcAddress
    ? "v86-bgi-tenth-win32-call"
    : dispatchModuleHandle
    ? "v86-bgi-ninth-win32-call"
    : dispatchHeapCreate
      ? "v86-bgi-eighth-win32-call"
    : dispatchHeapSetInformation
    ? "v86-bgi-seventh-win32-call"
    : dispatchStartupInfo
      ? "v86-bgi-sixth-win32-call"
    : dispatchPerformanceCounter
      ? "v86-bgi-fifth-win32-call"
    : dispatchTickCount
      ? "v86-bgi-fourth-win32-call"
      : dispatchThreadId
        ? "v86-bgi-third-win32-call"
        : dispatchProcessId
          ? "v86-bgi-second-win32-call"
          : dispatchSystemTime
            ? "v86-bgi-first-win32-call"
            : "v86-bgi-first-trap",
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

const expected = {
  source: {
    bytes: 1_193_472,
    sha256: "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1",
  },
  wasm: {
    bytes: 1_882_836,
    sha256: "f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26",
    imports: 22,
    exports: 809,
  },
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
    total: 330,
    declared: dispatchEncodePointer
      ? 14
      : dispatchTlsSetValue
      ? 13
      : dispatchTlsAlloc
      ? 12
      : dispatchGetProcAddress
      ? 11
      : dispatchModuleHandle
      ? 10
      : dispatchHeapCreate
        ? 9
      : dispatchHeapSetInformation
        ? 8
      : dispatchStartupInfo
        ? 7
      : dispatchPerformanceCounter
        ? 6
      : dispatchTickCount
        ? 5
      : dispatchThreadId
        ? 4
        : dispatchProcessId
          ? 3
          : dispatchSystemTime
            ? 2
            : 1,
    unsupported: dispatchEncodePointer
      ? 316
      : dispatchTlsSetValue
      ? 317
      : dispatchTlsAlloc
      ? 318
      : dispatchGetProcAddress
      ? 319
      : dispatchModuleHandle
      ? 320
      : dispatchHeapCreate
        ? 321
      : dispatchHeapSetInformation
        ? 322
      : dispatchStartupInfo
        ? 323
      : dispatchPerformanceCounter
        ? 324
      : dispatchTickCount
        ? 325
      : dispatchThreadId
        ? 326
        : dispatchProcessId
          ? 327
          : dispatchSystemTime
            ? 328
            : 329,
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
      "ac17565d250da1d27dc2429b2c81bd242fe26d67373495b2238eadbb762b6d33",
    commandLineContentSha256:
      "c9b83dbe193708a3418517c43a4dadd26ca50a4006b00bd11701f3bb6ef0e6cc",
    generation: 24,
  },
  dynamicExport: controlledDynamicExport.endpoint,
  dynamicExports: dispatchGetProcAddressFourth
    ? [
        controlledDynamicExport.endpoint,
        controlledSecondDynamicExport.endpoint,
        controlledThirdDynamicExport.endpoint,
        controlledFourthDynamicExport.endpoint,
      ]
    : dispatchGetProcAddressThird
    ? [
        controlledDynamicExport.endpoint,
        controlledSecondDynamicExport.endpoint,
        controlledThirdDynamicExport.endpoint,
      ]
    : [controlledDynamicExport.endpoint, controlledSecondDynamicExport.endpoint],
  memory: {
    owners: dispatchGetProcAddress ? 9 : 8,
    reservedPages: dispatchGetProcAddress ? 433 : 417,
    reflectedPages: dispatchGetProcAddress ? 389 : 388,
    generation: dispatchGetProcAddress ? 27 : 24,
  },
  reflectionContentSha256: dispatchGetProcAddressFourth
    ? "0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a"
    : dispatchGetProcAddressThird
    ? "8025131a9a8687ec01f68969f6cda2c102eaff9258a45bfcd29c5c5c3ae4b344"
    : dispatchGetProcAddressSecond
    ? "de382ad25a1487a1c8b038f48eace933d0270cf745820419b776689c87a50eeb"
    : dispatchGetProcAddress
    ? "e7a3cc956e167db7e078feefb30eb1d9f2204c8e17ca008f72c4359cce752456"
    : "f68f79bb13b3c5a11a2d145a8ee772a0bfd5c04dfec7502f50065cc748bc3cb0",
  offerLineBytes: 4_904_602,
  receiptLineBytes: 1_494,
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
  report: {
    bytes: 9_592,
    sha256: "4ed39b15de85c188b52dbafd0eda9c35e4d88f3c4aa8a9079c98f6d9d9f03840",
  },
  firstCall: {
    offerLineBytes: 4_904_569,
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
    reportBytes: 14_540,
    reportSha256: "7457cdfb50affd0003a3860e6959d5803cc9e7edd1c9bb0358454a6046984c16",
  },
  secondCall: {
    offerLineBytes: 4_904_536,
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
    reportBytes: 18_472,
    reportSha256: "bbce0252bd7215b0643118c49bef61c724e3c8c0f80279ede1cd5c3ba1b94e67",
  },
  thirdCall: {
    offerLineBytes: 4_904_503,
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
    reportBytes: 22_383,
    reportSha256: "d6bf15bec208ed1ced26970ff96a9d9f0faebbcec55da19cfbf27588b452d4fa",
  },
  fourthCall: {
    offerLineBytes: 4_904_470,
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
    reportBytes: 26_558,
    reportSha256: "79924bf3817104a9d2762b730534d8f74dc1ca1adfd17aa75c3855bcdf9bf111",
  },
  fifthCall: {
    offerLineBytes: 4_904_437,
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
    reportBytes: 31_427,
    reportSha256: "eb1e83e3a2a75483d2606ce7e92eca36acfdc8d15b69a8381813e8c738974ad1",
  },
  sixthCall: {
    offerLineBytes: 4_904_404,
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
    reportBytes: 39_031,
    reportSha256: "208e5930bdb17da0242fd4567dd8900467eb512992a1930a7e8ede7107c8232c",
  },
  seventhCall: {
    offerLineBytes: 4_904_371,
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
    reportBytes: 43_980,
    reportSha256: "09fab7aeda311661aef516ff3ff2156e0fa3293c6c495e5e601503ca44fabc7f",
  },
  eighthCall: {
    offerLineBytes: 4_904_338,
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
    reportBytes: 49_699,
    reportSha256: "f8c8ca1e678bb1e118248a6626825ddf585dc240aa59e1abc428429cbab3a1ad",
  },
  ninthCall: {
    offerLineBytes: 4_904_305,
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
    reportBytes: 56_729,
    reportSha256: "147a3599cf942af03034c7f410d333b35c0360812936e39a7473c43008d8d5be",
  },
  tenthCall: {
    offerLineBytes: 4_921_244,
    reflectionReceiptLineBytes: 1_579,
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
    reportBytes: 65_858,
    reportSha256: "54bcc15a033dbe870b81d1c2637949d71568d048202d8f3cc355097ca76332cf",
  },
  eleventhCall: {
    offerLineBytes: 4_922_145,
    reflectionReceiptLineBytes: 1_736,
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
    reportBytes: 76_844,
    reportSha256: "40d37072965e4ceefd760ac8111006e9cfeacab62a43fca621c2f7e3d81359b5",
  },
  twelfthCall: {
    offerLineBytes: 4_922_584,
    reflectionReceiptLineBytes: 1_798,
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
    reportBytes: 89_289,
    reportSha256: "28aa2899a7e4fb403c5d839b21e0bb19c05563b9bd19604372bbf1a16a39fbb4",
  },
  thirteenthCall: {
    offerLineBytes: 4_923_019,
    reflectionReceiptLineBytes: 1_860,
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
    reportBytes: 103_179,
    reportSha256: "1cfe9d56fd5c5f9494ffdcc017c6ad189e2900b70125f0eda67d3839326230d4",
  },
  fourteenthCall: {
    offerLineBytes: 4_922_986,
    reflectionReceiptLineBytes: 1_860,
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
    reportBytes: 110_236,
    reportSha256: "4d2ec198114434d1f46e97d07bb3c91861122462682eecad268a810b16ade0bb",
  },
  fifteenthCall: {
    offerLineBytes: 4_922_953,
    reflectionReceiptLineBytes: 1_860,
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
    reportBytes: 120_499,
    reportSha256: "25907e7ae254788313d0feccdae9dac152592c3210ea5bf6956a6b1cac9f960f",
  },
  sixteenthCall: {
    offerLineBytes: 4_922_920,
    reflectionReceiptLineBytes: 1_860,
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
    reportBytes: 129_530,
    reportSha256: "310d90e97f3b33b8c466d0a0e769fb4fe85a4b3cb2a55f191a0480d662f0032b",
  },
};

const page = {
  size: 0x1000,
  present: 1,
  writable: 2,
  user: 4,
};
const protocol = {
  maximumLineBytes: 8 * 1024 * 1024,
  maximumReflectionPages: 400,
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

async function run() {
  requireFile(wasmPath, "npm run prepare:engine");
  requireFile(bgiPath, "the primary title must be installed at its configured path");
  const sourceBefore = sourceIdentity(bgiPath);
  assert.deepEqual(sourceBefore, expected.source);

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
    String(expected.memory.reflectedPages),
    "--materialize-image-base",
    `0x${expected.image.loadBase.toString(16)}`,
    "--bind-name",
    "d3d9.dll",
    "Direct3DCreate9",
    ...(dispatchSystemTime
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "GetSystemTimeAsFileTime",
          "--system-time-delta-session",
        ]
      : []),
    ...(dispatchProcessId
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "GetCurrentProcessId",
          "--process-id-return-session",
        ]
      : []),
    ...(dispatchThreadId
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "GetCurrentThreadId",
          "--thread-id-return-session",
        ]
      : []),
    ...(dispatchTickCount
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "GetTickCount",
          "--tick-count-return-session",
        ]
      : []),
    ...(dispatchPerformanceCounter
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "QueryPerformanceCounter",
          "--performance-counter-return-session",
        ]
      : []),
    ...(dispatchStartupInfo
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "GetStartupInfoW",
          "--startup-info-return-session",
        ]
      : []),
    ...(dispatchHeapSetInformation
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "HeapSetInformation",
          "--heap-set-information-return-session",
        ]
      : []),
    ...(dispatchHeapCreate
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "HeapCreate",
          "--heap-create-return-session",
        ]
      : []),
    ...(dispatchModuleHandle
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "GetModuleHandleW",
          "--get-module-handle-w-return-session",
        ]
      : []),
    ...(dispatchGetProcAddress
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "GetProcAddress",
          "--get-proc-address-return-session",
        ]
      : []),
    ...(dispatchGetProcAddressSecond
      ? ["--get-proc-address-second-return-session"]
      : []),
    ...(dispatchGetProcAddressThird
      ? ["--get-proc-address-third-return-session"]
      : []),
    ...(dispatchGetProcAddressFourth
      ? ["--get-proc-address-fourth-return-session"]
      : []),
    ...(dispatchTlsAlloc
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "TlsAlloc",
          "--tls-alloc-return-session",
        ]
      : []),
    ...(dispatchTlsSetValue
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "TlsSetValue",
          "--tls-set-value-return-session",
        ]
      : []),
    ...(dispatchEncodePointer
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "EncodePointer",
          "--encode-pointer-return-session",
        ]
      : []),
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
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  const lineReader = readline.createInterface({ input: child.stdout, crlfDelay: Infinity });
  const lines = lineReader[Symbol.asyncIterator]();
  const exit = new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => resolve({ code, signal }));
  });
  let sessionCompleted = false;

  const nextMessage = async (label) => {
    const next = await lines.next();
    assert(!next.done, `${label} was not emitted; probe stderr: ${stderr.trim()}`);
    const lineBytes = Buffer.byteLength(next.value);
    assert(
      lineBytes <= protocol.maximumLineBytes,
      `${label} exceeds the ${protocol.maximumLineBytes}-byte protocol limit`,
    );
    return { message: JSON.parse(next.value), lineBytes };
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

    const execution = executeToFirstOwnedTrap(offer, context);
    assert.equal(execution.terminal.kind, "owned-import-thunk");
    validateThunk(execution.terminal.thunk, expected.terminal.thunk, !dispatchSystemTime);
    assert.equal(execution.instructionCount, expected.terminal.instructionCount);
    assert.equal(execution.executeStops, expected.terminal.executeStops);
    assert.equal(execution.budgetStops, expected.terminal.budgetStops);
    assert.equal(execution.terminal.returnAddress, expected.terminal.returnAddress);
    assert.deepEqual(execution.terminal.stackWords, expected.terminal.stackWords);
    const systemTime = dispatchSystemTime
      ? await dispatchFirstSystemTimeCall({
          child,
          context,
          execution,
          nextMessage,
          offer,
          continueSession: dispatchProcessId,
        })
      : null;
    const processIdentity = dispatchProcessId
      ? await dispatchSecondProcessIdCall({
          child,
          context,
          execution: systemTime.continuation,
          nextMessage,
          offer,
          continueSession: dispatchThreadId,
        })
      : null;
    const threadIdentity = dispatchThreadId
      ? await dispatchThirdThreadIdCall({
          child,
          context,
          execution: processIdentity.continuation,
          nextMessage,
          offer,
          continueSession: dispatchTickCount,
        })
      : null;
    const monotonicClock = dispatchTickCount
      ? await dispatchFourthTickCountCall({
          child,
          context,
          execution: threadIdentity.continuation,
          nextMessage,
          offer,
          continueSession: dispatchPerformanceCounter,
        })
      : null;
    const performanceCounter = dispatchPerformanceCounter
      ? await dispatchFifthPerformanceCounterCall({
          child,
          context,
          execution: monotonicClock.continuation,
          nextMessage,
          offer,
          continueSession: dispatchStartupInfo,
        })
      : null;
    const processStartup = dispatchStartupInfo
      ? await dispatchSixthStartupInfoCall({
          child,
          context,
          execution: performanceCounter.continuation,
          nextMessage,
          offer,
          continueSession: dispatchHeapSetInformation,
        })
      : null;
    const heapPolicy = dispatchHeapSetInformation
      ? await dispatchSeventhHeapSetInformationCall({
          child,
          context,
          execution: processStartup.continuation,
          nextMessage,
          offer,
          continueSession: dispatchHeapCreate,
        })
      : null;
    const privateHeap = dispatchHeapCreate
      ? await dispatchEighthHeapCreateCall({
          child,
          context,
          execution: heapPolicy.continuation,
          nextMessage,
          offer,
          continueSession: dispatchModuleHandle,
        })
      : null;
    const loadedModule = dispatchModuleHandle
      ? await dispatchNinthGetModuleHandleWCall({
          child,
          context,
          execution: privateHeap.continuation,
          nextMessage,
          offer,
          continueSession: dispatchGetProcAddress,
        })
      : null;
    const dynamicExport = dispatchGetProcAddress
      ? await dispatchTenthGetProcAddressCall({
          child,
          context,
          execution: loadedModule.continuation,
          nextMessage,
          offer,
          continueSession: dispatchGetProcAddressSecond,
        })
      : null;
    const secondDynamicExport = dispatchGetProcAddressSecond
      ? await dispatchEleventhGetProcAddressCall({
          child,
          context,
          execution: dynamicExport.continuation,
          nextMessage,
          offer,
          continueSession: dispatchGetProcAddressThird,
        })
      : null;
    const thirdDynamicExport = dispatchGetProcAddressThird
      ? await dispatchTwelfthGetProcAddressCall({
          child,
          context,
          execution: secondDynamicExport.continuation,
          nextMessage,
          offer,
          continueSession: dispatchGetProcAddressFourth,
        })
      : null;
    const fourthDynamicExport = dispatchGetProcAddressFourth
      ? await dispatchThirteenthGetProcAddressCall({
          child,
          context,
          execution: thirdDynamicExport.continuation,
          nextMessage,
          offer,
          continueSession: dispatchTlsAlloc,
        })
      : null;
    const tlsAllocation = dispatchTlsAlloc
      ? await dispatchFourteenthTlsAllocCall({
          child,
          context,
          execution: fourthDynamicExport.continuation,
          nextMessage,
          offer,
          continueSession: dispatchTlsSetValue,
        })
      : null;
    const tlsValue = dispatchTlsSetValue
      ? await dispatchFifteenthTlsSetValueCall({
          child,
          context,
          execution: tlsAllocation.continuation,
          nextMessage,
          offer,
          tlsAllocation,
          continueSession: dispatchEncodePointer,
        })
      : null;
    const encodedNull = dispatchEncodePointer
      ? await dispatchSixteenthEncodePointerCall({
          child,
          context,
          execution: tlsValue.continuation,
          nextMessage,
          offer,
          tlsAllocation,
          tlsValue,
        })
      : null;
    const processExit = await exit;
    assert.equal(processExit.signal, null, `reflection probe terminated by ${processExit.signal}`);
    assert.equal(processExit.code, 0, `reflection probe failed: ${stderr.trim()}`);
    sessionCompleted = true;
    const sourceAfter = sourceIdentity(bgiPath);
    assert.deepEqual(sourceAfter, sourceBefore);
    const ownerSummaries = summarizeOwners(offer.reflection.pages);
    const report = dispatchEncodePointer
      ? buildSixteenthEncodePointerReport({
          context,
          dynamicExport,
          encodedNull,
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
        })
      : dispatchTlsSetValue
      ? buildFifteenthTlsSetValueReport({
          context,
          dynamicExport,
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
        })
      : dispatchTlsAlloc
      ? buildFourteenthTlsAllocReport({
          context,
          dynamicExport,
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
        })
      : dispatchGetProcAddressFourth
      ? buildThirteenthGetProcAddressReport({
          context,
          dynamicExport,
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
        })
      : dispatchGetProcAddressThird
      ? buildTwelfthGetProcAddressReport({
          context,
          dynamicExport,
          execution,
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
        })
      : dispatchGetProcAddressSecond
      ? buildEleventhGetProcAddressReport({
          context,
          dynamicExport,
          execution,
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
          threadIdentity,
        })
      : dispatchGetProcAddress
      ? buildTenthGetProcAddressReport({
          context,
          dynamicExport,
          execution,
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
          sourceAfter,
          sourceBefore,
          systemTime,
          threadIdentity,
        })
      : dispatchModuleHandle
      ? buildNinthGetModuleHandleWReport({
          context,
          execution,
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
          sourceAfter,
          sourceBefore,
          systemTime,
          threadIdentity,
        })
      : dispatchHeapCreate
        ? buildEighthHeapCreateReport({
          context,
          execution,
          heapPolicy,
          invalidationCalls,
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
          sourceAfter,
          sourceBefore,
          systemTime,
          threadIdentity,
        })
      : dispatchHeapSetInformation
        ? buildSeventhHeapSetInformationReport({
          context,
          execution,
          heapPolicy,
          invalidationCalls,
          monotonicClock,
          offer,
          offerMessage,
          ownerSummaries,
          performanceCounter,
          processIdentity,
          processStartup,
          receipt,
          receiptMessage,
          sourceAfter,
          sourceBefore,
          systemTime,
          threadIdentity,
        })
      : dispatchStartupInfo
        ? buildSixthStartupInfoReport({
          context,
          execution,
          invalidationCalls,
          monotonicClock,
          offer,
          offerMessage,
          ownerSummaries,
          performanceCounter,
          processIdentity,
          processStartup,
          receipt,
          receiptMessage,
          sourceAfter,
          sourceBefore,
          systemTime,
          threadIdentity,
        })
      : dispatchPerformanceCounter
        ? buildFifthPerformanceCounterReport({
          context,
          execution,
          invalidationCalls,
          monotonicClock,
          offer,
          offerMessage,
          ownerSummaries,
          performanceCounter,
          processIdentity,
          receipt,
          receiptMessage,
          sourceAfter,
          sourceBefore,
          systemTime,
          threadIdentity,
        })
      : dispatchTickCount
        ? buildFourthTickCountReport({
            context,
            execution,
            invalidationCalls,
            monotonicClock,
            offer,
            offerMessage,
            ownerSummaries,
            processIdentity,
            receipt,
            receiptMessage,
            sourceAfter,
            sourceBefore,
            systemTime,
            threadIdentity,
          })
        : dispatchThreadId
          ? buildThirdThreadIdReport({
              context,
              execution,
              invalidationCalls,
              offer,
              offerMessage,
              ownerSummaries,
              processIdentity,
              receipt,
              receiptMessage,
              sourceAfter,
              sourceBefore,
              systemTime,
              threadIdentity,
            })
          : dispatchProcessId
            ? buildSecondProcessIdReport({
                context,
                execution,
                invalidationCalls,
                offer,
                offerMessage,
                ownerSummaries,
                processIdentity,
                receipt,
                receiptMessage,
                sourceAfter,
                sourceBefore,
                systemTime,
              })
            : dispatchSystemTime
              ? buildFirstSystemTimeReport({
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
                })
              : {
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
    fs.mkdirSync(outputDirectory, { recursive: true });
    const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
    const expectedReport = dispatchEncodePointer
      ? { bytes: expected.sixteenthCall.reportBytes, sha256: expected.sixteenthCall.reportSha256 }
      : dispatchTlsSetValue
      ? { bytes: expected.fifteenthCall.reportBytes, sha256: expected.fifteenthCall.reportSha256 }
      : dispatchTlsAlloc
      ? { bytes: expected.fourteenthCall.reportBytes, sha256: expected.fourteenthCall.reportSha256 }
      : dispatchGetProcAddressFourth
      ? { bytes: expected.thirteenthCall.reportBytes, sha256: expected.thirteenthCall.reportSha256 }
      : dispatchGetProcAddressThird
      ? { bytes: expected.twelfthCall.reportBytes, sha256: expected.twelfthCall.reportSha256 }
      : dispatchGetProcAddressSecond
      ? { bytes: expected.eleventhCall.reportBytes, sha256: expected.eleventhCall.reportSha256 }
      : dispatchGetProcAddress
      ? { bytes: expected.tenthCall.reportBytes, sha256: expected.tenthCall.reportSha256 }
      : dispatchModuleHandle
      ? { bytes: expected.ninthCall.reportBytes, sha256: expected.ninthCall.reportSha256 }
      : dispatchHeapCreate
        ? { bytes: expected.eighthCall.reportBytes, sha256: expected.eighthCall.reportSha256 }
      : dispatchHeapSetInformation
        ? { bytes: expected.seventhCall.reportBytes, sha256: expected.seventhCall.reportSha256 }
      : dispatchStartupInfo
        ? { bytes: expected.sixthCall.reportBytes, sha256: expected.sixthCall.reportSha256 }
      : dispatchPerformanceCounter
        ? { bytes: expected.fifthCall.reportBytes, sha256: expected.fifthCall.reportSha256 }
      : dispatchTickCount
        ? { bytes: expected.fourthCall.reportBytes, sha256: expected.fourthCall.reportSha256 }
        : dispatchThreadId
          ? { bytes: expected.thirdCall.reportBytes, sha256: expected.thirdCall.reportSha256 }
          : dispatchProcessId
            ? { bytes: expected.secondCall.reportBytes, sha256: expected.secondCall.reportSha256 }
            : dispatchSystemTime
              ? { bytes: expected.firstCall.reportBytes, sha256: expected.firstCall.reportSha256 }
              : expected.report;
    if (expectedReport.bytes !== null) {
      assert.equal(reportBytes.byteLength, expectedReport.bytes);
      assert.equal(sha256(reportBytes), expectedReport.sha256);
    }
    fs.writeFileSync(reportPath, reportBytes);
    if (dispatchEncodePointer) {
      console.log(
        `v86 BGI sixteenth Win32 call passed: encoded NULL token ` +
          `0x${encodedNull.encodedPointer.value.toString(16)}, next terminal ` +
          `${encodedNull.continuation.terminal.kind} after ` +
          `${encodedNull.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchTlsSetValue) {
      console.log(
        `v86 BGI fifteenth Win32 call passed: TLS value endpoint ` +
          `0x${tlsValue.threadValue.endpoint.address.toString(16)}, next terminal ` +
          `${tlsValue.continuation.terminal.kind} after ` +
          `${tlsValue.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchTlsAlloc) {
      console.log(
        `v86 BGI fourteenth Win32 call passed: logical TLS index ` +
          `${tlsAllocation.slot.index.value}, next terminal ` +
          `${tlsAllocation.continuation.terminal.kind} after ` +
          `${tlsAllocation.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchGetProcAddressFourth) {
      console.log(
        `v86 BGI thirteenth Win32 call passed: dynamic export endpoint ` +
          `0x${fourthDynamicExport.endpoint.address.toString(16)}, next terminal ` +
          `${fourthDynamicExport.continuation.terminal.kind} after ` +
          `${fourthDynamicExport.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchGetProcAddressThird) {
      console.log(
        `v86 BGI twelfth Win32 call passed: dynamic export endpoint ` +
          `0x${thirdDynamicExport.endpoint.address.toString(16)}, next thunk ID ` +
          `${thirdDynamicExport.continuation.terminal.thunk.id} after ` +
          `${thirdDynamicExport.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchGetProcAddressSecond) {
      console.log(
        `v86 BGI eleventh Win32 call passed: dynamic export endpoint ` +
          `0x${secondDynamicExport.endpoint.address.toString(16)}, next thunk ID ` +
          `${secondDynamicExport.continuation.terminal.thunk.id} after ` +
          `${secondDynamicExport.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchGetProcAddress) {
      console.log(
        `v86 BGI tenth Win32 call passed: dynamic export endpoint ` +
          `0x${dynamicExport.endpoint.address.toString(16)}, next thunk ID ` +
          `${dynamicExport.continuation.terminal.thunk.id} after ` +
          `${dynamicExport.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchModuleHandle) {
      console.log(
        `v86 BGI ninth Win32 call passed: logical loaded module ` +
          `0x${loadedModule.module.handle.value.toString(16)}, next thunk ID ` +
          `${loadedModule.continuation.terminal.thunk.id} after ` +
          `${loadedModule.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchHeapCreate) {
      console.log(
        `v86 BGI eighth Win32 call passed: logical private heap ` +
          `0x${privateHeap.heap.handle.value.toString(16)}, next thunk ID ` +
          `${privateHeap.continuation.terminal.thunk.id} after ` +
          `${privateHeap.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchHeapSetInformation) {
      console.log(
        `v86 BGI seventh Win32 call passed: heap policy ` +
          `${heapPolicy.policy.policyId}, next thunk ID ` +
          `${heapPolicy.continuation.terminal.thunk.id} after ` +
          `${heapPolicy.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchStartupInfo) {
      console.log(
        `v86 BGI sixth Win32 call passed: controlled STARTUPINFOW ` +
          `${processStartup.profile.profileId}, next thunk ID ` +
          `${processStartup.continuation.terminal.thunk.id} after ` +
          `${processStartup.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchPerformanceCounter) {
      console.log(
        `v86 BGI fifth Win32 call passed: performance counter ` +
          `${controlledPerformanceCounter.ticks}, next thunk ID ` +
          `${performanceCounter.continuation.terminal.thunk.id} after ` +
          `${performanceCounter.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchTickCount) {
      console.log(
        `v86 BGI fourth Win32 call passed: tick count ${controlledMonotonicClock.tickCount}, ` +
          `next thunk ID ${monotonicClock.continuation.terminal.thunk.id} after ` +
          `${monotonicClock.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchThreadId) {
      console.log(
        `v86 BGI third Win32 call passed: thread ID ${controlledThread.threadId}, ` +
          `next thunk ID ${threadIdentity.continuation.terminal.thunk.id} after ` +
          `${threadIdentity.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchProcessId) {
      console.log(
        `v86 BGI second Win32 call passed: process ID ${controlledProcess.processId}, ` +
          `next thunk ID ${processIdentity.continuation.terminal.thunk.id} after ` +
          `${processIdentity.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchSystemTime) {
      console.log(
        `v86 BGI first Win32 call passed: FILETIME generation ` +
          `${systemTime.delta.delta.generation}, next thunk ID ` +
          `${systemTime.continuation.terminal.thunk.id} after ` +
          `${systemTime.continuation.instructionCount} total instructions`,
      );
    } else {
      console.log(
        `v86 BGI first trap passed: ${offer.reflection.pages.length} pages, ` +
          `${execution.instructionCount} instructions, thunk ID ${execution.terminal.thunk.id} ` +
          `${execution.terminal.thunk.module}!${execution.terminal.thunk.name ?? `#${execution.terminal.thunk.ordinal}`}`,
      );
    }
    console.log(
      `reflection sha256 ${offer.reflection.contentSha256}; report: ` +
        `${path.relative(root, reportPath)}, ${reportBytes.byteLength} bytes, ` +
        `sha256 ${sha256(reportBytes)}`,
    );
  } finally {
    lineReader.close();
    if (!sessionCompleted && child.exitCode === null) {
      child.kill();
    }
  }
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
  const expectedOfferLineBytes = dispatchEncodePointer
    ? expected.sixteenthCall.offerLineBytes
    : dispatchTlsSetValue
    ? expected.fifteenthCall.offerLineBytes
    : dispatchTlsAlloc
    ? expected.fourteenthCall.offerLineBytes
    : dispatchGetProcAddressFourth
    ? expected.thirteenthCall.offerLineBytes
    : dispatchGetProcAddressThird
    ? expected.twelfthCall.offerLineBytes
    : dispatchGetProcAddressSecond
    ? expected.eleventhCall.offerLineBytes
    : dispatchGetProcAddress
    ? expected.tenthCall.offerLineBytes
    : dispatchModuleHandle
    ? expected.ninthCall.offerLineBytes
    : dispatchHeapCreate
      ? expected.eighthCall.offerLineBytes
    : dispatchHeapSetInformation
      ? expected.seventhCall.offerLineBytes
    : dispatchStartupInfo
      ? expected.sixthCall.offerLineBytes
    : dispatchPerformanceCounter
      ? expected.fifthCall.offerLineBytes
    : dispatchTickCount
      ? expected.fourthCall.offerLineBytes
      : dispatchThreadId
        ? expected.thirdCall.offerLineBytes
        : dispatchProcessId
          ? expected.secondCall.offerLineBytes
          : dispatchSystemTime
            ? expected.firstCall.offerLineBytes
            : expected.offerLineBytes;
  if (expectedOfferLineBytes !== null) {
    assert.equal(offerLineBytes, expectedOfferLineBytes);
  }
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
  for (const reflected of offer.reflection.pages) {
    const virtualBase = reflected.address;
    checkedGuestEnd(virtualBase, page.size, "reflected page");
    assert.equal(virtualBase & (page.size - 1), 0);
    assert(!mappings.has(virtualBase), `duplicate mapping 0x${virtualBase.toString(16)}`);
    const table = ensurePageTable(virtualBase);
    const tableIndex = (virtualBase >>> 12) & 0x3ff;
    const physicalBase = allocatePhysicalPage(`reflected-owner-${reflected.owner}`);
    writePhysicalU32(
      table + tableIndex * 4,
      physicalBase |
        page.present |
        page.user |
        (reflected.protection.write ? page.writable : 0),
      "reflected page table entry",
    );
    physicalMemory.set(Uint8Array.from(reflected.bytes), physicalBase);
    mappings.set(virtualBase, {
      owner: reflected.owner,
      physicalBase,
      protection: { ...reflected.protection },
      role: `reflected-owner-${reflected.owner}`,
    });
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
    pageDirectory,
    pageTables,
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
  const expectedReceiptLineBytes = dispatchEncodePointer
    ? expected.sixteenthCall.reflectionReceiptLineBytes
    : dispatchTlsSetValue
    ? expected.fifteenthCall.reflectionReceiptLineBytes
    : dispatchTlsAlloc
    ? expected.fourteenthCall.reflectionReceiptLineBytes
    : dispatchGetProcAddressFourth
    ? expected.thirteenthCall.reflectionReceiptLineBytes
    : dispatchGetProcAddressThird
    ? expected.twelfthCall.reflectionReceiptLineBytes
    : dispatchGetProcAddressSecond
    ? expected.eleventhCall.reflectionReceiptLineBytes
    : dispatchGetProcAddress
    ? expected.tenthCall.reflectionReceiptLineBytes
    : expected.receiptLineBytes;
  if (expectedReceiptLineBytes !== null) {
    assert.equal(receiptLineBytes, expectedReceiptLineBytes);
  }
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
  child.stdin.end(requestLine);

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
