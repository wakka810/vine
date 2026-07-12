import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const docsDirectory = path.join(root, "docs");
const tick = String.fromCharCode(96);
const errors = [];

const checkpointPath = path.join(docsDirectory, "current-checkpoint.json");
try {
  const checkpoint = JSON.parse(fs.readFileSync(checkpointPath, "utf8"));
  if (
    checkpoint.schema !== "vine.current-checkpoint" ||
    checkpoint.schemaVersion !== 1 ||
    checkpoint.checkpoint !== "bgi-seventeenth-win32-call" ||
    checkpoint.entryInstructionCount !== 238 ||
    checkpoint.supportedCallCount !== 17 ||
    checkpoint.lastSupportedCallId !== 89 ||
    checkpoint.lastSupportedCall !== "KERNEL32.dll!EncodePointer" ||
    checkpoint.memoryGeneration !== 30 ||
    checkpoint.runtimeKernelCheckpoint !== "v86-browser-engine-gate" ||
    checkpoint.runtimeStateAuthority !== "rust" ||
    checkpoint.runtimeStateScope !== "single-process-controlled-initial-thread" ||
    JSON.stringify(checkpoint.runtimeStateOwns) !==
      JSON.stringify([
        "process",
        "thread-last-error",
        "clock-startup",
        "heap-module-endpoint-registries",
        "tls-encoded-pointers",
        "callback-scheduler-host-events",
      ]) ||
    checkpoint.callbackFrameStack !== true ||
    checkpoint.hostCompletionModel !== "scheduler-event" ||
    checkpoint.genericDispatcherApiCount !== 13 ||
    checkpoint.genericDispatcherCallOccurrenceCount !== 17 ||
    JSON.stringify(checkpoint.genericDispatcherApis) !==
      JSON.stringify([
        "KERNEL32.dll!GetSystemTimeAsFileTime",
        "KERNEL32.dll!GetCurrentProcessId",
        "KERNEL32.dll!GetCurrentThreadId",
        "KERNEL32.dll!GetTickCount",
        "KERNEL32.dll!QueryPerformanceCounter",
        "KERNEL32.dll!GetStartupInfoW",
        "KERNEL32.dll!HeapSetInformation",
        "KERNEL32.dll!HeapCreate",
        "KERNEL32.dll!GetModuleHandleW",
        "KERNEL32.dll!GetProcAddress",
        "KERNEL32.dll!TlsAlloc",
        "KERNEL32.dll!TlsSetValue",
        "KERNEL32.dll!EncodePointer",
      ]) ||
    checkpoint.allSupportedCallsUseGenericDispatcher !== true ||
    checkpoint.checkpointManifestized !== true ||
    checkpoint.checkpointManifest !== "fixtures/checkpoints/bgi-startup-win32.json" ||
    checkpoint.checkpointManifestSchemaVersion !== 1 ||
    checkpoint.checkpointManifestEntryCount !== 18 ||
    checkpoint.genericSessionRunner !== true ||
    checkpoint.apiSpecificCheckpointCliModes !== false ||
    checkpoint.bespokeCheckpointExpansionFrozen !== true ||
    checkpoint.controlledProcessId !== 4096 ||
    checkpoint.controlledThreadId !== 8192 ||
    checkpoint.controlledTickCount !== 3600000 ||
    checkpoint.controlledPerformanceCounter !== 12345678901 ||
    checkpoint.controlledStartupInfoProfile !==
      "startupinfow-v68-zero-optional-fields" ||
    checkpoint.controlledHeapPolicy !== "heap-enable-termination-on-corruption" ||
    checkpoint.controlledPrivateHeapVariant !== "heapcreate-growable-initial-4096" ||
    checkpoint.logicalPrivateHeapHandleNamespace !== "vine.logical-private-heap.v1" ||
    checkpoint.logicalPrivateHeapHandle !== 0x68010001 ||
    checkpoint.controlledLoadedModuleVariant !==
      "getmodulehandlew-kernel32-loaded-runtime-module" ||
    checkpoint.logicalLoadedModuleHandleNamespace !== "vine.logical-loaded-module.v1" ||
    checkpoint.logicalLoadedModuleHandle !== 0x4d010001 ||
    checkpoint.controlledDynamicExportVariant !==
      "getprocaddress-kernel32-flsalloc-runtime-endpoint" ||
    checkpoint.dynamicExportEndpointNamespace !== "vine.dynamic-export-endpoint.v1" ||
    checkpoint.dynamicExportEndpointId !== 1 ||
    checkpoint.dynamicExportEndpoint !== 0x7f020000 ||
    checkpoint.controlledSecondDynamicExportVariant !==
      "getprocaddress-kernel32-flsgetvalue-runtime-endpoint" ||
    checkpoint.controlledThirdDynamicExportVariant !==
      "getprocaddress-kernel32-flssetvalue-runtime-endpoint" ||
    checkpoint.secondDynamicExportEndpointId !== 2 ||
    checkpoint.secondDynamicExportEndpoint !== 0x7f020010 ||
    checkpoint.firstDynamicExportStoreAddress !== 0x100feebc ||
    checkpoint.thirdDynamicExportEndpointId !== 3 ||
    checkpoint.thirdDynamicExportEndpoint !== 0x7f020020 ||
    checkpoint.secondDynamicExportStoreAddress !== 0x100feec0 ||
    checkpoint.controlledFourthDynamicExportVariant !==
      "getprocaddress-kernel32-flsfree-runtime-endpoint" ||
    checkpoint.dynamicExportEndpointCount !== 4 ||
    checkpoint.fourthDynamicExportEndpointId !== 4 ||
    checkpoint.fourthDynamicExportEndpoint !== 0x7f020030 ||
    checkpoint.thirdDynamicExportStoreAddress !== 0x100feec4 ||
    checkpoint.fourthDynamicExportStoreAddress !== 0x100feec8 ||
    checkpoint.controlledTlsAllocationVariant !== "tlsalloc-first-controlled-index" ||
    checkpoint.logicalTlsIndexNamespace !== "vine.logical-tls-index.v1" ||
    checkpoint.logicalTlsIndex !== 0 ||
    checkpoint.tlsOutOfIndexes !== 0xffff_ffff ||
    checkpoint.controlledTlsValueVariant !==
      "tlssetvalue-initial-thread-flsgetvalue-endpoint" ||
    checkpoint.controlledTlsThreadId !== 8192 ||
    checkpoint.logicalTlsValueEndpointId !== 2 ||
    checkpoint.logicalTlsValueEndpoint !== 0x7f020010 ||
    checkpoint.tlsIndexStoreAddress !== 0x100f0764 ||
    checkpoint.controlledPointerEncodingVariant !==
      "encodepointer-null-process-token" ||
    checkpoint.logicalEncodedPointerNamespace !== "vine.logical-encoded-pointer.v1" ||
    checkpoint.logicalEncodedNull !== 0xec01_0001 ||
    checkpoint.controlledProcessInitializerEncodingVariant !==
      "encodepointer-process-initializer-function-token" ||
    checkpoint.processInitializerSource !== 0x100a_9608 ||
    checkpoint.logicalEncodedProcessInitializer !== 0xec01_0002 ||
    checkpoint.encodedProcessInitializerStoreAddress !== 0x100f_f62c ||
    checkpoint.nativeProcessCookieImported !== false ||
    checkpoint.ownedTrapId !== 89 ||
    checkpoint.ownedTrap !== "KERNEL32.dll!EncodePointer" ||
    checkpoint.ownedTrapSymbol !== "EncodePointer" ||
    checkpoint.ownedTrapArgument !== 0x7f02_0000 ||
    checkpoint.memoryProtocol !== "binary-page-transport-v1" ||
    checkpoint.memoryDeltaProtocol !== "memory-delta-v1" ||
    checkpoint.byteAuthority !== "rust-guest-memory" ||
    checkpoint.executionMirror !== "v86-paging" ||
    checkpoint.dirtyDiscovery !== "v86-pte-dirty-bit" ||
    checkpoint.writebackSynchronization !== "bounded-slice-end" ||
    checkpoint.writebackGenerationPolicy !== "one-generation-per-changed-batch" ||
    JSON.stringify(checkpoint.binaryMemoryTransports) !==
      JSON.stringify(["VINERF01", "VINEDL01", "VINEWB01", "VINEFR01"]) ||
    JSON.stringify(checkpoint.automaticWritebackEvidence) !==
      JSON.stringify({
        reflectedPages: 9,
        writablePages: 8,
        firstSliceDirtyPages: 7,
        firstSliceChangedBytes: 49,
        secondSliceDirtyPages: 4,
        secondSliceChangedBytes: 13,
        initialGeneration: 18,
        finalGeneration: 21,
        reportBytes: 4578,
        reportSha256: "5eab2884b70f8b1a48d98dff8021b29a1690296c03dcae0183b5c9fdc10368b8",
      }) ||
    checkpoint.memoryTransaction !== "changed-state-frame-before-image-journal-v1" ||
    checkpoint.memoryTransactionCopiesBaselinePages !== false ||
    JSON.stringify(checkpoint.memoryTransactionScalingEvidence) !==
      JSON.stringify({
        baselinePages: 1024,
        baselineBytes: 4194304,
        beginJournaledFrameBytes: 0,
        oneChangedPagePeakBytes: 4096,
        twoChangedPagesPeakBytes: 8192,
      }) ||
    checkpoint.browserRuntimeProtocol !== "vine.browser-runtime-worker.v1" ||
    checkpoint.browserRuntimeWorker !== true ||
    JSON.stringify(checkpoint.browserRuntimeCoreWasm) !==
      JSON.stringify({
        imports: 0,
        bytes: 658861,
        sha256: "0c2647e6d07f7f47cabe022d0560a72ced0a851f508ab1820feb004f7fd8bf23",
      }) ||
    JSON.stringify(checkpoint.browserRuntimeFixture) !==
      JSON.stringify({
        bytes: 2560,
        sha256: "24c98ec43bc76dcd0c1ddbbbf68c2ea1d86207bc2ebea2aea536a4757bf4189f",
        nativeExitCode: 0,
      }) ||
    JSON.stringify(checkpoint.browserRuntimeEvidence) !==
      JSON.stringify({
        chromeVersion: "150.0.7871.115",
        guestInstructions: 46,
        cpuSlices: 10,
        budgetYields: 3,
        hostRequests: 2,
        callbackEntries: 2,
        maximumCallbackDepth: 2,
        writebackBatches: 7,
        writebackPages: 9,
        finalGeneration: 21,
        pixelRgba: [255, 0, 0, 255],
        toneHz: 880,
        persistentByte: 90,
        exitCode: 0,
        cancelPendingHostRequests: 1,
        cancelPendingHostCallsAfterCleanup: 0,
        cancelPendingCallbacksAfterCleanup: 0,
        watchdogInstructions: 4,
        watchdogPendingHostCallsAfterCleanup: 0,
        watchdogPendingCallbacksAfterCleanup: 0,
      }) ||
    JSON.stringify(checkpoint.browserStressEvidence) !==
      JSON.stringify({
        durationMilliseconds: 1800073.5,
        sessions: 11326,
        memorySamples: 31,
        failedSessions: 0,
        firstUsedJsHeapBytes: 960969,
        lastUsedJsHeapBytes: 994282,
        heapSlopeBytesPerMinute: 136.21389950213228,
        maximumHeartbeatGapMilliseconds: 16.28499999642372,
        reportBytes: 7039,
        reportSha256: "13a7b579bb13841aa073de4dcf5ed7db0743485810e12b3159272205fbb43bb3",
      }) ||
    JSON.stringify(checkpoint.v86EngineGate) !==
      JSON.stringify({
        decision: "go",
        configuration: "fixtures/benchmarks/v86-browser-v1.json",
        sourceCommit: "a0d99e5722391deda4fad0c2ef67f2af155b929e",
        sourceDirty: false,
        chromeVersion: "150.0.7871.115",
        integerInstructionsPerSecond: 134122853.20309904,
        x87InstructionsPerSecond: 52536027.07970423,
        sse2InstructionsPerSecond: 83711951.00537346,
        adapterTrapRoundTripsPerSecond: 254517.68884038288,
        productionApiRoundTripsPerSecond: 11278.615544947608,
        activeSteadyMemoryBytes: 14581661,
        backgroundThroughputRatio: 0.9852770260106688,
        audioMaximumDeliveryGapMilliseconds: 51.314999997615814,
        thresholdFailures: 0,
        reportBytes: 18161,
        reportSha256: "e50ad355361f91e01baae1dcb6f62fb813b2ddc1202466682c60eda7e959a1e7",
      }) ||
    checkpoint.nextCheckpoint !== "runtime-object-model-foundation" ||
    checkpoint.pausedBgiCheckpoint !== "eighteenth-supported-win32-call-return"
  ) {
    errors.push("docs/current-checkpoint.json: unsupported or inconsistent checkpoint values");
  }
  const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
  if (!readme.includes(`checkpoint is \`${checkpoint.checkpoint}\``)) {
    errors.push("README.md: current checkpoint does not match the checkpoint manifest");
  }
  if (!readme.includes(`thunk ID \`${checkpoint.ownedTrapId}\``)) {
    errors.push("README.md: owned trap ID does not match the checkpoint manifest");
  }
  if (!fs.existsSync(path.join(root, checkpoint.checkpointManifest))) {
    errors.push("docs/current-checkpoint.json: runtime checkpoint manifest is missing");
  }
} catch (error) {
  errors.push(`docs/current-checkpoint.json: ${error.message}`);
}

const files = fs
  .readdirSync(docsDirectory)
  .filter((file) => file.endsWith(".md"))
  .sort()
  .map((file) => path.join(docsDirectory, file));

for (const file of files) {
  const text = fs.readFileSync(file, "utf8");
  const relativeFile = path.relative(root, file);

  if (!text.endsWith("\n")) {
    errors.push(`${relativeFile}: missing final newline`);
  }

  const lines = text.split("\n");
  lines.forEach((line, index) => {
    if (/[ \t]+$/.test(line)) {
      errors.push(`${relativeFile}:${index + 1}: trailing whitespace`);
    }
  });

  const fenceCount = lines.filter((line) =>
    line.startsWith(tick.repeat(3)),
  ).length;
  if (fenceCount % 2 !== 0) {
    errors.push(`${relativeFile}: unbalanced code fences (${fenceCount})`);
  }

  let table = [];
  let inFence = false;
  const flushTable = () => {
    if (table.length === 0) {
      return;
    }

    const pipeCounts = new Set(
      table.map(({ line }) => (line.match(/\|/g) ?? []).length),
    );
    if (pipeCounts.size > 1) {
      errors.push(
        `${relativeFile}:${table[0].lineNumber}: inconsistent table columns`,
      );
    }
    table = [];
  };

  lines.forEach((line, index) => {
    if (line.startsWith(tick.repeat(3))) {
      flushTable();
      inFence = !inFence;
      return;
    }

    if (!inFence && line.startsWith("|") && line.endsWith("|")) {
      table.push({ line, lineNumber: index + 1 });
      return;
    }
    flushTable();
  });
  flushTable();

  for (const match of text.matchAll(/\]\(([^)]+)\)/g)) {
    const target = match[1];
    if (/^(?:https?:|mailto:|#)/.test(target)) {
      continue;
    }

    const targetPath = target.split("#", 1)[0];
    if (targetPath.length === 0) {
      continue;
    }

    const resolved = path.resolve(path.dirname(file), targetPath);
    if (!fs.existsSync(resolved)) {
      errors.push(`${relativeFile}: broken link ${target}`);
    }
  }
}

const definitions = new Map();
const addDefinition = (id, location) => {
  const locations = definitions.get(id) ?? [];
  locations.push(location);
  definitions.set(id, locations);
};

for (const file of files) {
  const relativeFile = path.relative(root, file);
  fs.readFileSync(file, "utf8")
    .split("\n")
    .forEach((line, index) => {
      let match = line.match(/^\| ((?:REQ|NFR|CON)-[A-Z0-9-]+) \|/);
      match ??= line.match(/^\| ((?:A|Q|R)-[A-Z0-9-]+) \|/);
      match ??= line.match(/^\| (ADR-[A-Z0-9-]+) \|/);
      match ??= line.match(
        new RegExp(`^- ${tick}(AC-[A-Z0-9-]+)${tick}:`),
      );
      if (match) {
        addDefinition(match[1], `${relativeFile}:${index + 1}`);
      }
    });
}

for (const [id, locations] of definitions) {
  if (locations.length > 1) {
    errors.push(`duplicate definition ${id}: ${locations.join(", ")}`);
  }
}

const references = new Map();
for (const file of files) {
  const relativeFile = path.relative(root, file);
  fs.readFileSync(file, "utf8")
    .split("\n")
    .forEach((line, index) => {
      for (const match of line.matchAll(
        /(?<!-)\b(?:REQ|NFR|CON|A|Q|R|ADR|AC)-[A-Z0-9]+(?:-[A-Z0-9]+)*/g,
      )) {
        const locations = references.get(match[0]) ?? [];
        locations.push(`${relativeFile}:${index + 1}`);
        references.set(match[0], locations);
      }
    });
}

for (const [id, locations] of references) {
  const isGroupReference = [...definitions.keys()].some((definition) =>
    definition.startsWith(`${id}-`),
  );
  if (!definitions.has(id) && !isGroupReference) {
    errors.push(
      `undefined reference ${id}: ${[...new Set(locations)].slice(0, 4).join(", ")}`,
    );
  }
}

const tasks = new Map();
const roadmap = path.join(docsDirectory, "roadmap.md");
fs.readFileSync(roadmap, "utf8")
  .split("\n")
  .forEach((line, index) => {
    const match = line.match(/^\| ((?:P\d+[A-Z]?|F)-[A-Z0-9-]+) \|/);
    if (!match) {
      return;
    }
    const locations = tasks.get(match[1]) ?? [];
    locations.push(index + 1);
    tasks.set(match[1], locations);
  });

for (const [id, locations] of tasks) {
  if (locations.length > 1) {
    errors.push(
      `duplicate task ${id}: docs/roadmap.md:${locations.join(",")}`,
    );
  }
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}

console.log(
  `documentation check passed: ${files.length} files, ${definitions.size} IDs, ${tasks.size} tasks`,
);
