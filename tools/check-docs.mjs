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
    checkpoint.runtimeKernelCheckpoint !== "bgi-fiber-local-storage-value" ||
    checkpoint.runtimeStateAuthority !== "rust" ||
    checkpoint.runtimeStateScope !==
      "single-process-controlled-two-thread-wait-message-heap-module-dll-lifecycle-critical-section-pointer-decoding-fls-allocation-private-heap-allocation-fls-value" ||
    JSON.stringify(checkpoint.runtimeStateOwns) !==
      JSON.stringify([
        "process",
        "thread-last-error",
        "clock-startup",
        "heap-module-endpoint-registries",
        "tls-encoded-pointers",
        "callback-scheduler-host-events",
        "typed-object-handle-lifetimes",
        "process-module-graph",
        "process-thread-lifecycle",
        "controlled-thread-registry",
        "controlled-wait-queue",
        "controlled-message-queue",
        "controlled-private-heap-backing",
        "mapped-module-image-export-registry",
        "guest-import-runtime-export-binding",
        "dll-process-callback-lifecycle",
        "process-critical-section-synchronization",
        "process-fiber-local-storage-allocation",
        "process-private-heap-allocation",
        "thread-fiber-local-storage-values",
      ]) ||
    checkpoint.callbackFrameStack !== true ||
    checkpoint.hostCompletionModel !== "scheduler-event" ||
    checkpoint.genericDispatcherApiCount !== 20 ||
    checkpoint.genericDispatcherCallOccurrenceCount !== 42 ||
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
        "KERNEL32.dll!DecodePointer",
        "KERNEL32.dll!InitializeCriticalSectionAndSpinCount",
        "KERNEL32.dll!EnterCriticalSection",
        "KERNEL32.dll!LeaveCriticalSection",
        "KERNEL32.dll!FlsAlloc",
        "KERNEL32.dll!FlsSetValue",
        "KERNEL32.dll!HeapAlloc",
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
    JSON.stringify(checkpoint.runtimeObjectModel) !==
      JSON.stringify({
        schema: "vine.runtime-object-table.v1",
        handleEncoding: "type8-generation8-slot16",
        objectKinds: ["process", "thread", "private-heap", "module"],
        initialObjectCount: 2,
        initialHandleCount: 2,
        initialProcessHandle: 0x5001_0001,
        initialThreadHandle: 0x5401_0001,
        firstReusedThreadHandle: 0x5402_0001,
        staleHandleError: "runtime-handle-stale",
        typeMismatchError: "runtime-handle-type-mismatch",
        subsystemCloseError: "runtime-handle-close-policy",
        waitableKinds: ["process", "thread"],
        runtimeCoreWasmBytes: 687523,
        runtimeCoreWasmSha256:
          "ad1941f95e96577f2e84462d534b95ca9e09f39d1e39e9f4f922ba0fac00b633",
        browserBenchmarkDecision: "go",
        browserBenchmarkThresholdFailures: 0,
      }) ||
    JSON.stringify(checkpoint.runtimeModuleGraph) !==
      JSON.stringify({
        schema: "vine.runtime-module-graph.v1",
        canonicalIdentity: "ascii-case-insensitive-basename",
        moduleClasses: [
          "main-executable",
          "guest-dynamic-library",
          "runtime-system-module",
        ],
        loadStates: [
          "registered",
          "ready",
          "process-attached",
          "process-detached",
        ],
        referenceOwners: [
          "process-root",
          "runtime-service",
          "static-import",
          "dynamic-load-thread",
        ],
        moduleLimit: 4096,
        referenceOwnerLimitPerModule: 4096,
        controlledModuleCount: 4,
        controlledDependencyEdgeCount: 4,
        controlledAttachOrder: [
          "codec.dll",
          "kernel32.dll",
          "engine.dll",
          "game.exe",
        ],
        controlledDetachOrder: [
          "game.exe",
          "engine.dll",
          "kernel32.dll",
          "codec.dll",
        ],
        runtimeCoreWasmBytes: 690816,
        runtimeCoreWasmSha256:
          "5bfa7b7141a3f0a30acd1c58e6a28afe2c5fc1e8f8b5e5859c97e3068a11cac0",
        browserWorkerGuestInstructions: 46,
        browserWorkerExitCode: 0,
        browserBenchmarkDecision: "go",
        browserBenchmarkThresholdFailures: 0,
        bgiSupportedCallCount: 17,
        bgiInstructionCount: 238,
        bgiReportSha256:
          "ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4",
      }) ||
    JSON.stringify(checkpoint.runtimeProcessThreadLifecycle) !==
      JSON.stringify({
        schema: "vine.runtime-process-thread-lifecycle.v1",
        processStates: [
          "bootstrapping",
          "running",
          "exit-requested",
          "exited",
        ],
        threadStates: ["initialized", "running", "exit-requested", "exited"],
        schedulerBoundaryStates: ["initialized", "terminating", "terminated"],
        exitRequestWaitState: "nonsignaled",
        processExitSignalBatch: ["thread", "process"],
        terminalStateOrder: ["thread", "scheduler", "process"],
        controlledCancelledHostRequests: 1,
        controlledCancelledCallbackFrames: 1,
        controlledDiscardedSchedulerEvents: 0,
        wholeRuntimeStateClone: false,
        maximumPlannedWaitObjectRecords: 2,
        runtimeCoreWasmBytes: 710027,
        runtimeCoreWasmSha256:
          "a641572313cf256ea28ff5c2b53b7b4a04bc6a7f9eab294780a59eed5bb4c38f",
        browserWorkerGuestInstructions: 46,
        browserWorkerExitCode: 0,
        browserBenchmarkDecision: "go",
        browserBenchmarkThresholdFailures: 0,
        bgiSupportedCallCount: 17,
        bgiInstructionCount: 238,
        bgiReportSha256:
          "ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4",
      }) ||
    JSON.stringify(checkpoint.runtimeThreadRegistry) !==
      JSON.stringify({
        schema: "vine.runtime-thread-registry.v1",
        initialThreadId: 8192,
        secondThreadId: 4097,
        secondThreadHandle: "0x54010002",
        perThreadState: [
          "last-error",
          "tls-values",
          "callback-frames",
          "scheduler-lane",
        ],
        explicitCurrentThreadSelection: true,
        threadOnlyExitCode: 23,
        processTerminationOrder: [
          "thread:4097",
          "thread:8192",
          "process:4096",
        ],
        controlledCancelledHostRequests: 2,
        controlledCancelledCallbackFrames: 2,
        controlledDiscardedSchedulerEvents: 0,
        wholeRuntimeStateClone: false,
        maximumPlannedWaitObjectRecords: 3,
        runtimeCoreWasmBytes: 732779,
        runtimeCoreWasmSha256:
          "dcbade608dc66c75c3179d4d7b67d7c0ddc32891c6769b1fdc38e20dd70d4ab8",
        browserWorkerGuestInstructions: 46,
        browserWorkerExitCode: 0,
        browserBenchmarkDecision: "go",
        browserBenchmarkThresholdFailures: 0,
        bgiSupportedCallCount: 17,
        bgiInstructionCount: 238,
        bgiReportSha256:
          "ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4",
      }) ||
    JSON.stringify(checkpoint.runtimeWaitQueue) !==
      JSON.stringify({
        schema: "vine.runtime-wait-queue.v1",
        waiterThreadId: 8192,
        signalerThreadId: 4097,
        waitedHandle: "0x54010002",
        registrationId: 1,
        pendingSchedulerState: "waiting-object",
        signalSource: "thread-exit-completion",
        completionExitCode: 23,
        wakeSchedulerState: "runnable",
        wakeEvent: "wait-satisfied",
        deterministicWakeOrder: ["registration:1"],
        alreadySignaledImmediate: true,
        processExitWaitRegistrationsCancelled: 1,
        wholeRuntimeStateClone: false,
        runtimeCoreWasmBytes: 765531,
        runtimeCoreWasmSha256:
          "3b92ff588f944db29e59cd46ae9a873bbb1d52a1433958499c6eaddd641d59aa",
        browserWorkerGuestInstructions: 46,
        browserWorkerExitCode: 0,
        browserBenchmarkDecision: "go",
        browserBenchmarkThresholdFailures: 0,
        bgiSupportedCallCount: 17,
        bgiInstructionCount: 238,
        bgiReportSha256:
          "ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4",
      }) ||
    JSON.stringify(checkpoint.runtimeMessageQueue) !==
      JSON.stringify({
        schema: "vine.runtime-message-queue.v1",
        producerThreadId: 4097,
        ownerThreadId: 8192,
        messageIds: [1, 2, 3],
        queueCapacityPerThread: 4096,
        emptySchedulerState: "waiting-message",
        wakeSchedulerState: "runnable",
        wakeEvent: "message-ready",
        firstWakeMessageId: 1,
        firstWakeQueueDepth: 1,
        secondQueuedMessageCreatesWake: false,
        secondWakeMessageId: 3,
        processExitMessagesDiscarded: 2,
        postClonesWholeQueue: false,
        runtimeCoreWasmBytes: 769885,
        runtimeCoreWasmSha256:
          "db29e4846e40dc840bcf79ad70e613e2c32d251fcb05aeece1bf94ba09299db3",
        browserWorkerGuestInstructions: 46,
        browserWorkerExitCode: 0,
        browserBenchmarkDecision: "go",
        browserBenchmarkThresholdFailures: 0,
        bgiSupportedCallCount: 17,
        bgiInstructionCount: 238,
        bgiReportSha256:
          "ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4",
      }) ||
    JSON.stringify(checkpoint.runtimeHeapBacking) !==
      JSON.stringify({
        schema: "vine.runtime-private-heap-backing.v1",
        arenaBase: "0x20000000",
        arenaEndExclusive: "0x40000000",
        backingLimit: 32,
        capacityBytesPerHeap: 16777216,
        allocationAlignmentBytes: 8,
        maximumLiveAllocationsPerHeap: 65536,
        allocationIds: [1, 2, 3],
        allocationAddresses: ["0x20000000", "0x20000218", "0x20000000"],
        firstRequestedBytes: 532,
        firstReservedBytes: 536,
        secondRequestedBytes: 2048,
        initializationBytes: {
          zeroed: 0,
          deterministicUninitialized: 165,
          freed: 221,
        },
        allocationPolicy: "lowest-address-first-fit",
        allocationIdentityOnReuse: "monotonic-new-id",
        memoryAuthority: "rust-guest-memory",
        memoryOwnerKind: "anonymous",
        memoryRegionKind: "heap",
        memoryProtection: "rw-nx",
        memoryMutation: "guest-memory-transaction-before-targeted-runtime-commit",
        collisionRollbackError: "memory-region-overlap",
        doubleFreeError: "heap-allocation-double-free",
        invalidFreeError: "heap-allocation-pointer-invalid",
        processExitCleanupBoundary: "exit-request-to-completion",
        processExitCompletionGate: "process-heaps-live-at-exit-completion",
        directTerminationGate: "process-heaps-live-at-direct-termination",
        controlledFinalMemoryGeneration: 13,
        controlledFinalAcknowledgedGeneration: 13,
        guestHeapAllocDispatch: true,
        guestHeapAllocSupportedVariant: "heap-zero-memory-0x214",
        guestHeapAllocReturnAddress: "0x20000000",
        broadHeapAllocatorPolicy: false,
        runtimeCoreWasmBytes: 772038,
        runtimeCoreWasmSha256:
          "aa0d5d69da04fd9a9b6678d8e034e525f81b191351a1603b7fc5fe0f5420d85c",
        browserWorkerGuestInstructions: 46,
        browserWorkerExitCode: 0,
        browserBenchmarkDecision: "go",
        browserBenchmarkThresholdFailures: 0,
        bgiSupportedCallCount: 17,
        bgiInstructionCount: 238,
        bgiReportSha256:
          "ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4",
      }) ||
    JSON.stringify(checkpoint.runtimeModuleImages) !==
      JSON.stringify({
        schema: "vine.runtime-module-image-registry.v1",
        fixtureBytes: 2048,
        fixtureSha256:
          "a9c2530df2a89317048ae9380e4202c8bd41994d1ff0e14b43fa578bb32c4878",
        moduleLimit: 4096,
        forwarderDepthLimit: 32,
        lookupNameBytesLimit: 1024,
        controlledMappedModuleCount: 5,
        directNameAddress: "0x10001000",
        directOrdinalAddress: "0x10001010",
        bridgeModule: "bridge.dll",
        nameForwarder: "vine_fixture.fixture_add",
        ordinalForwarder: "vine_fixture.#2",
        zeroEntryPointRepresentation: "none",
        memoryIdentity: "sealed-image-owner-source-range-pages",
        registrationFailureCodes: [
          "module-image-owner-unsealed",
          "module-image-identity-duplicate",
          "module-image-range-overlap",
        ],
        resolutionFailureCodes: [
          "export-forwarder-target-module-unknown",
          "export-forwarder-cycle",
        ],
        registryStatePreservedOnFailure: true,
        guestImportBinding: false,
        dllMainCallbacks: false,
        runtimeCoreWasmBytes: 791970,
        runtimeCoreWasmSha256:
          "94b0a39b7c862016f02a2af6c050cb196dab4a4badfe40bad48070827b60ca23",
        browserWorkerGuestInstructions: 46,
        browserWorkerExitCode: 0,
        browserBenchmarkDecision: "go",
        browserBenchmarkThresholdFailures: 0,
        bgiSupportedCallCount: 17,
        bgiInstructionCount: 238,
        bgiReportSha256:
          "ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4",
      }) ||
    JSON.stringify(checkpoint.runtimeDllLifecycle) !==
      JSON.stringify({
        schema: "vine.runtime-dll-lifecycle.v1",
        mainFixtureBytes: 2560,
        mainFixtureSha256:
          "df9553bc5c10dae44d0b6f25f0a96c2a6fcda903895b7ffecebade641bff980a",
        engineFixtureBytes: 2560,
        engineFixtureSha256:
          "7154f24bec58d4e080de8bb98b512f53b425a54dcde273679c193cb1daaaf90b",
        codecFixtureBytes: 2560,
        codecFixtureSha256:
          "6e872f31dddb4932f06e0908c0cbe536a552dca24f338d174fa7e538a21c3d55",
        nativeExitCode: 0,
        controlledModuleCount: 4,
        controlledDependencyEdgeCount: 3,
        mappedGuestImportCount: 3,
        runtimeSystemImportCount: 1,
        runtimeExportEndpointId: 1,
        runtimeExportEndpointAddress: "0x7f000000",
        runtimeExportArenaPages: 1,
        runtimeExportArenaSha256:
          "4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059",
        controlledIatTargets: [
          "0x30001000",
          "0x30001010",
          "0x31001000",
          "0x7f000000",
        ],
        controlledReflectionPages: 15,
        controlledReflectionGeneration: 12,
        graphAttachOrder: [
          "kernel32.dll",
          "lifecycle_codec.dll",
          "lifecycle_engine.dll",
          "dll-lifecycle.exe",
        ],
        callbackAttachOrder: [
          "lifecycle_codec.dll",
          "lifecycle_engine.dll",
        ],
        attachFailureModule: "lifecycle_engine.dll",
        attachFailureUnwindOrder: ["lifecycle_codec.dll"],
        attachFailureRetryable: true,
        callbackFrameIds: [1, 2, 3, 4, 5, 6, 7],
        callbackDetachOrder: [
          "lifecycle_engine.dll",
          "lifecycle_codec.dll",
        ],
        graphDetachOrder: [
          "dll-lifecycle.exe",
          "lifecycle_engine.dll",
          "lifecycle_codec.dll",
          "kernel32.dll",
        ],
        missingDependencyError: "module-image-name-unknown",
        wrongFrameError: "dll-callback-frame-mismatch",
        callbackGraphMutationError:
          "module-graph-mutation-during-dll-callback-forbidden",
        loadLibraryFreeLibrary: false,
        tlsCallbacks: false,
        moduleRetirement: false,
        runtimeCoreWasmBytes: 794331,
        runtimeCoreWasmSha256:
          "bbc24294dbd5acf79d9cff0a753193dda1dd5e8c6dfc4026cd811d86c9b2e05d",
        browserWorkerGuestInstructions: 46,
        browserWorkerExitCode: 0,
        browserBenchmarkDecision: "go",
        browserBenchmarkThresholdFailures: 0,
        bgiSupportedCallCount: 17,
        bgiInstructionCount: 238,
        bgiReportSha256:
          "ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4",
      }) ||
    JSON.stringify(checkpoint.bgiGenericRuntimeReentry) !==
      JSON.stringify({
        schema: "vine.v86-bgi-generic-runtime-reentry.v1",
        frozenCheckpoint: "bgi-seventeenth-win32-call",
        frozenSemanticCallsValidated: 17,
        frozenInstructionCount: 238,
        frozenReportSha256:
          "ab4070d9b0f86e4821f7ea4d89b57a44d83829b3032f04c1edf40fde49db2da4",
        reflectionPages: 389,
        reflectionGeneration: 30,
        reflectionSha256:
          "0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a",
        processModuleCount: 15,
        processDependencyEdgeCount: 14,
        processMainModuleHandle: "0x4d01000f",
        runtimeExportEndpointCount: 330,
        implementedApiContractCount: 13,
        dynamicExportEndpointCount: 4,
        genericSupportedCallCount: 21,
        encodePointerOccurrenceCount: 6,
        writebackBatches: 22,
        writebackDirtyPages: 38,
        writebackChangedBytes: 268,
        hostDeltaBatches: 3,
        finalMemoryGeneration: 52,
        firstUnsupportedInstructionCount: 266,
        firstUnsupportedCapabilityCluster: "kernel32-critical-section-synchronization",
        firstUnsupportedEndpointId: 100,
        firstUnsupportedEndpointAddress: "0x7f000630",
        firstUnsupportedApi: "KERNEL32.dll!InitializeCriticalSectionAndSpinCount",
        firstUnsupportedArguments: ["0x100ff630", 4000],
        firstUnsupportedIatAddress: "0x100d1238",
        firstUnsupportedFault: "endpoint-contract-missing",
        apiSpecificReconciliationLists: false,
        apiSpecificRequestsOrReceipts: false,
        binaryCpuWriteback: "VINEWB01",
        binaryHostDelta: "VINEDL01",
        binaryFrame: "VINEFR01",
        reportBytes: 96244,
        reportSha256:
          "a3aa1a78c688bd3af5fab86a0753a8449c1d5ca79f2b6d5d2dded2558b9d48fa",
      }) ||
    JSON.stringify(checkpoint.bgiCriticalSectionSynchronization) !==
      JSON.stringify({
        schema: "vine.v86-bgi-critical-section-synchronization.v1",
        nativeTraceBytes: 8094273,
        nativeTraceLines: 14542,
        nativeTraceSha256:
          "c668e8e6bd831442d4f0f5e3e2937395b7cd2bc003aa96a67b8872417cc53567",
        nativeTraceFrida: "17.15.4",
        nativeTraceWindowsBuild: 26200,
        nativeTraceDiagnostics: 0,
        nativeOracleProfile: "x86-uncontended-recursive-v1",
        x86StructureBytes: 24,
        criticalSectionLimit: 4096,
        spinCount: 4000,
        nativeInitializationCallerRva: "0x000ab5e7",
        nativeInitializationFirstAddress: "0x0033f630",
        mappedInitializationFirstAddress: "0x100ff630",
        initializationCount: 14,
        initializationStride: 24,
        initializedFields: [-1, -1, 0, 0, 0, 4000],
        initializationResult: 1,
        lastErrorPolicy: "preserve",
        nativeEnterCallerRva: "0x000ab763",
        nativeLeaveCallerRva: "0x000ab66f",
        sameThreadRecursion: true,
        crossThreadContention: "critical-section-contention-unimplemented",
        deleteObserved: true,
        deleteImplemented: false,
        implementedApiContractCount: 16,
        genericSupportedCallCount: 35,
        criticalSectionInitializationCalls: 14,
        reflectionPages: 389,
        reflectionGeneration: 27,
        frozenCompatibilityGeneration: 30,
        reflectionSha256:
          "0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a",
        writebackBatches: 36,
        writebackDirtyPages: 65,
        writebackChangedBytes: 344,
        hostDeltaBatches: 17,
        finalMemoryGeneration: 80,
        firstUnsupportedInstructionCount: 575,
        firstUnsupportedCapabilityCluster: "kernel32-pointer-encoding",
        firstUnsupportedEndpointId: 88,
        firstUnsupportedEndpointAddress: "0x7f000570",
        firstUnsupportedApi: "KERNEL32.dll!DecodePointer",
        firstUnsupportedArguments: ["0xec010003"],
        firstUnsupportedIatAddress: "0x100d1208",
        firstUnsupportedReturnAddress: "0x100a5924",
        firstUnsupportedFault: "endpoint-contract-missing",
        apiSpecificReconciliationLists: false,
        apiSpecificRequestsOrReceipts: false,
        reportBytes: 167719,
        reportSha256:
          "cf0ea737b7eefc00e95c17a7dc3185c7f917fa7c8f3a10e009ccbd2943f06f69",
      }) ||
    JSON.stringify(checkpoint.bgiPointerDecoding) !==
      JSON.stringify({
        schema: "vine.v86-bgi-pointer-decoding.v1",
        nativePrimaryTraceBytes: 2213858,
        nativePrimaryTraceLines: 6979,
        nativePrimaryTraceSha256:
          "71a79c89aa9511cf0eca37c952d24130765212705ab34be08de56b1529045fa3",
        nativeConfirmationTraceBytes: 312314,
        nativeConfirmationTraceLines: 1040,
        nativeConfirmationTraceSha256:
          "4309d18d16ad3dc6b5390df421d7edc4137c712221cea65378b91774e31ef04c",
        nativeDecodeCallerRva: "0x000a5924",
        nativePrimaryEncodedInput: "0x61c0a638",
        nativeConfirmationEncodedInput: "0xc7c1eb7e",
        nativeDecodedTarget: "KERNEL32.dll!FlsAlloc",
        lastErrorPolicy: "preserve",
        tokenNamespace: "vine.logical-encoded-pointer.v1",
        decodedToken: "0xec010003",
        decodedSource: "0x7f020000",
        acceptsOnlyRegisteredTokens: true,
        nativeProcessCookieEmulated: false,
        guestWrites: 0,
        implementedApiContractCount: 17,
        genericSupportedCallCount: 36,
        decodePointerCalls: 1,
        reflectionPages: 389,
        reflectionGeneration: 27,
        frozenCompatibilityGeneration: 30,
        reflectionSha256:
          "0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a",
        writebackBatches: 37,
        writebackDirtyPages: 66,
        writebackChangedBytes: 348,
        hostDeltaBatches: 17,
        finalMemoryGeneration: 81,
        firstUnsupportedInstructionCount: 577,
        firstUnsupportedCapabilityCluster: "kernel32-fiber-local-storage",
        firstUnsupportedEndpointKind: "dynamic-export",
        firstUnsupportedEndpointId: 1,
        firstUnsupportedEndpointAddress: "0x7f020000",
        firstUnsupportedApi: "KERNEL32.dll!FlsAlloc",
        firstUnsupportedArguments: ["0x100a5666"],
        firstUnsupportedReturnAddress: "0x100a5926",
        firstUnsupportedFault: "dynamic-export-contract-missing",
        apiSpecificReconciliationLists: false,
        apiSpecificRequestsOrReceipts: false,
        reportBytes: 171356,
        reportSha256:
          "5072319018e311156da1a619ac3edd1d7f9ad3c1c26b5b0d8215a11e872b7c9e",
      }) ||
    JSON.stringify(checkpoint.bgiFiberLocalStorageAllocation) !==
      JSON.stringify({
        schema: "vine.v86-bgi-fiber-local-storage-allocation.v1",
        nativePrimaryTraceBytes: 441487,
        nativePrimaryTraceLines: 1459,
        nativePrimaryTraceSha256:
          "2a505ca8fd7339896e651c9fe7ddc9e60b604b92d878c9d01cf10600a944f281",
        nativePrimaryApiPairs: 694,
        nativeConfirmationTraceBytes: 453151,
        nativeConfirmationTraceLines: 1498,
        nativeConfirmationTraceSha256:
          "d3f3e4829b9dbb564a6956c581bbb46607f97bb0a1651f9481ea2d61d57bc186",
        nativeConfirmationApiPairs: 713,
        nativeMaximumCallDepth: 3,
        nativeDiagnostics: 0,
        nativeCallInstructionRva: "0x000a5924",
        nativeCallerReturnRva: "0x000a5926",
        nativeCallbackRva: "0x000a5666",
        nativeIndex: 9,
        lastErrorPolicy: "preserve",
        indexNamespace: "vine.logical-fls-index.v1",
        logicalIndex: 0,
        indexLimit: 128,
        flsOutOfIndexes: "0xffffffff",
        callback: "0x100a5666",
        callbackMustBeExecutable: true,
        nullCallbackAccepted: true,
        processOwned: true,
        perThreadValuesImplemented: false,
        freeImplemented: false,
        teardownCallbackImplemented: false,
        exhaustionReturnImplemented: false,
        dynamicEndpointUsesGenericDispatcher: true,
        dispatcherIdentityKey: "binding-kind-and-endpoint-id",
        implementedApiContractCount: 18,
        genericSupportedCallCount: 37,
        flsAllocCalls: 1,
        reflectionPages: 389,
        reflectionGeneration: 27,
        frozenCompatibilityGeneration: 30,
        reflectionSha256:
          "0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a",
        writebackBatches: 38,
        writebackDirtyPages: 68,
        writebackChangedBytes: 399,
        hostDeltaBatches: 17,
        finalMemoryGeneration: 82,
        firstUnsupportedInstructionCount: 618,
        firstUnsupportedCapabilityCluster: "kernel32-private-heap-allocation",
        firstUnsupportedEndpointKind: "runtime-system-export",
        firstUnsupportedEndpointId: 75,
        firstUnsupportedEndpointAddress: "0x7f0004a0",
        firstUnsupportedApi: "KERNEL32.dll!HeapAlloc",
        firstUnsupportedArguments: ["0x68010001", "0x00000008", "0x00000214"],
        firstUnsupportedIatAddress: "0x100d11d4",
        firstUnsupportedReturnAddress: "0x100ac3cc",
        firstUnsupportedFault: "endpoint-contract-missing",
        apiSpecificReconciliationLists: false,
        apiSpecificRequestsOrReceipts: false,
        reportBytes: 175123,
        reportSha256:
          "26ed41c2a7e8266a47431da9fd737d26752e5812f057400b9715047990e6695c",
      }) ||
    JSON.stringify(checkpoint.bgiPrivateHeapAllocation) !==
      JSON.stringify({
        schema: "vine.v86-bgi-private-heap-allocation.v1",
        nativeTraceBytes: 6870828,
        nativeTraceLines: 21822,
        nativeTraceSha256:
          "9bdf000eedb8844a6e8db4732d025e7dd3a33854bea47175e26c5d5a3e678682",
        nativeCallerReturnRva: "0x000ac3cc",
        nativeHeapZeroMemoryFlags: "0x00000008",
        nativeFirstByteLength: 532,
        nativeFirstResult: "0x04a405c0",
        nativeFirstLastErrorBefore: 0,
        nativeFirstLastErrorAfter: 0,
        nativeDeferredSecondByteLength: 2048,
        logicalHeapHandle: "0x68010001",
        allocationId: 1,
        guestAddress: "0x20000000",
        requestedBytes: 532,
        reservedBytes: 536,
        initialization: "zeroed",
        initialCommitBytes: 4096,
        lastErrorPolicy: "preserve",
        implementedApiContractCount: 19,
        genericSupportedCallCount: 39,
        heapAllocCalls: 1,
        decodePointerCalls: 2,
        reflectionPages: 389,
        reflectionGeneration: 27,
        reflectionSha256:
          "0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a",
        writebackBatches: 40,
        writebackDirtyPages: 70,
        writebackChangedBytes: 415,
        hostDeltaBatches: 18,
        heapDeltaSequence: 38,
        heapDeltaFromGeneration: 82,
        heapDeltaGeneration: 85,
        heapDeltaMappingOperations: 2,
        heapDeltaOwner: 10,
        heapDeltaDirtyPage: "0x20000000",
        heapDeltaPayloadBytes: 4096,
        heapDeltaPayloadSha256:
          "ad7facb2586fc6e966c004d7d1d16b024f5805ff7cb47c7a85dabd8b48892ca7",
        finalMemoryGeneration: 87,
        firstUnsupportedInstructionCount: 644,
        firstUnsupportedCapabilityCluster: "kernel32-fiber-local-storage",
        firstUnsupportedEndpointKind: "dynamic-export",
        firstUnsupportedEndpointId: 3,
        firstUnsupportedEndpointAddress: "0x7f020020",
        firstUnsupportedApi: "KERNEL32.dll!FlsSetValue",
        firstUnsupportedArguments: ["0x00000000", "0x20000000"],
        firstUnsupportedReturnAddress: "0x100a5955",
        firstUnsupportedFault: "dynamic-export-contract-missing",
        heapFreeImplemented: false,
        heapDestroyDispatchImplemented: false,
        heapReAllocImplemented: false,
        zeroSizeImplemented: false,
        resourceFailureReturnImplemented: false,
        secondAllocationImplemented: false,
        apiSpecificReconciliationLists: false,
        apiSpecificRequestsOrReceipts: false,
        reportBytes: 183102,
        reportSha256:
          "d4081cfeca7648435b8a4f49c56b7abaec483dd8cd181343c2c543d2586df2b4",
      }) ||
    JSON.stringify(checkpoint.bgiFiberLocalStorageValue) !==
      JSON.stringify({
        schema: "vine.v86-bgi-fiber-local-storage-value.v1",
        nativePrimaryTraceBytes: 441487,
        nativePrimaryTraceLines: 1459,
        nativePrimaryTraceSha256:
          "2a505ca8fd7339896e651c9fe7ddc9e60b604b92d878c9d01cf10600a944f281",
        nativeConfirmationTraceBytes: 453151,
        nativeConfirmationTraceLines: 1498,
        nativeConfirmationTraceSha256:
          "d3f3e4829b9dbb564a6956c581bbb46607f97bb0a1651f9481ea2d61d57bc186",
        nativeCallerReturnRva: "0x000a5955",
        nativeIndex: 9,
        nativePrimaryValue: "0x043605c0",
        nativeConfirmationValue: "0x03e505c0",
        nativeResult: 1,
        nativePrimaryLastErrorBefore: 87,
        nativePrimaryLastErrorAfter: 87,
        nativeConfirmationLastErrorBefore: 0,
        nativeConfirmationLastErrorAfter: 0,
        lastErrorPolicy: "preserve",
        logicalIndex: 0,
        logicalValue: "0x20000000",
        logicalThreadId: 8192,
        perThreadValueState: true,
        implementedApiContractCount: 20,
        genericSupportedCallCount: 42,
        flsSetValueCalls: 1,
        reflectionPages: 389,
        reflectionGeneration: 27,
        reflectionSha256:
          "0b32ce6e6c8d1c291f7fb51928c8da2401f2b0ab67dcf27c4665a2d13c41205a",
        writebackBatches: 43,
        writebackDirtyPages: 75,
        writebackChangedBytes: 497,
        hostDeltaBatches: 19,
        finalMemoryGeneration: 91,
        downstreamGetModuleHandleWInstructionCount: 676,
        downstreamEnterCriticalSectionInstructionCount: 699,
        firstUnsupportedInstructionCount: 707,
        firstUnsupportedCapabilityCluster: "kernel32-api-contract",
        firstUnsupportedEndpointKind: "runtime-system-export",
        firstUnsupportedEndpointId: 124,
        firstUnsupportedEndpointAddress: "0x7f0007b0",
        firstUnsupportedApi: "KERNEL32.dll!InterlockedIncrement",
        firstUnsupportedArguments: ["0x100f08e0"],
        firstUnsupportedIatAddress: "0x100d1298",
        firstUnsupportedReturnAddress: "0x100a5577",
        firstUnsupportedFault: "endpoint-contract-missing",
        flsGetValueImplemented: false,
        flsFreeImplemented: false,
        teardownCallbackImplemented: false,
        apiSpecificReconciliationLists: false,
        apiSpecificRequestsOrReceipts: false,
        reportBytes: 195821,
        reportSha256:
          "f6698bdfb51d0420bd3a85b2b2525b3cd09e155fde529a4997283548382e742f",
      }) ||
    checkpoint.nextCheckpoint !== "kernel32-interlocked-increment" ||
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
