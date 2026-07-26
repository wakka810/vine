import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const legacyPolicyFiles = [
  "fixtures/runtime-policies/kernel32-controlled-v1.json",
  "fixtures/runtime-policies/kernel32-heap-two-v1.json",
  "fixtures/runtime-policies/kernel32-standard-output-v1.json",
  "fixtures/runtime-policies/kernel32-standard-output-file-type-v1.json",
  "fixtures/runtime-policies/kernel32-standard-error-v1.json",
  "fixtures/runtime-policies/kernel32-standard-error-file-type-v1.json",
  "fixtures/runtime-policies/kernel32-set-handle-count-v1.json",
  "fixtures/runtime-policies/kernel32-command-line-a-v1.json",
  "fixtures/runtime-policies/kernel32-environment-strings-w-v1.json",
  "fixtures/runtime-policies/kernel32-wide-char-to-multi-byte-v1.json",
  "fixtures/runtime-policies/kernel32-private-heap-size-1-v1.json",
  "fixtures/runtime-policies/kernel32-private-heap-size-544-v1.json",
  "fixtures/runtime-policies/kernel32-get-cp-info-v1.json",
  "fixtures/runtime-policies/kernel32-multi-byte-to-wide-char-v1.json",
  "fixtures/runtime-policies/kernel32-get-string-type-w-v1.json",
  "fixtures/runtime-policies/kernel32-lc-map-string-w-v1.json",
  "fixtures/runtime-policies/kernel32-get-module-file-name-a-v1.json",
  "fixtures/runtime-policies/kernel32-private-heap-size-16-v1.json",
  "fixtures/runtime-policies/kernel32-private-heap-size-4-v1.json",
  "fixtures/runtime-policies/kernel32-private-heap-free-size-1-v1.json",
  "fixtures/runtime-policies/kernel32-private-heap-size-128-v1.json",
  "fixtures/runtime-policies/kernel32-is-processor-feature-present-v1.json",
  "fixtures/runtime-policies/kernel32-heap-size-v1.json",
  "fixtures/runtime-policies/kernel32-initialize-critical-section-v1.json",
  "fixtures/runtime-policies/kernel32-multi-byte-to-wide-char-cp-acp-v1.json",
  "fixtures/runtime-policies/kernel32-set-current-directory-a-v1.json",
  "fixtures/runtime-policies/kernel32-create-mutex-a-v1.json",
];
const policyFiles = [
  ...legacyPolicyFiles,
  "fixtures/runtime-policies/kernel32-bounded-heap-v2.json",
  "fixtures/runtime-policies/kernel32-get-system-info-v1.json",
  "fixtures/runtime-policies/kernel32-get-system-default-lang-id-v1.json",
  "fixtures/runtime-policies/kernel32-get-drive-type-a-v1.json",
  "fixtures/runtime-policies/kernel32-get-version-ex-a-v1.json",
  "fixtures/runtime-policies/kernel32-create-waitable-timer-a-v1.json",
  "fixtures/runtime-policies/kernel32-large-heap-allocation-v1.json",
  "fixtures/runtime-policies/kernel32-source-file-read-v1.json",
  "fixtures/runtime-policies/kernel32-d3dx-runtime-module-v1.json",
  "fixtures/runtime-policies/kernel32-create-thread-v1.json",
  "fixtures/runtime-policies/kernel32-set-thread-priority-v1.json",
  "fixtures/runtime-policies/kernel32-virtual-alloc-reserve-v1.json",
  "fixtures/runtime-policies/kernel32-set-waitable-timer-v1.json",
  "fixtures/runtime-policies/kernel32-wait-for-single-object-ex-v1.json",
  "fixtures/runtime-policies/kernel32-sleep-zero-v1.json",
  "fixtures/runtime-policies/kernel32-global-memory-status-v1.json",
  "fixtures/runtime-policies/kernel32-get-file-attributes-a-v1.json",
  "fixtures/runtime-policies/kernel32-create-directory-a-v1.json",
];

for (const relative of policyFiles) {
  const policy = JSON.parse(fs.readFileSync(path.join(root, relative), "utf8"));
  assert.equal(policy.schema, "vine.kernel32-compatibility-policy");
  assert([1, 2].includes(policy.schemaVersion));
  assert.match(policy.id, /^[a-z0-9-]{1,128}$/u);
  assert(policy.standardHandleSelectors.length > 0);
  assert.equal(
    new Set(policy.standardHandleSelectors).size,
    policy.standardHandleSelectors.length,
  );
  if (policy.schemaVersion === 1) {
    assert.equal(policy.heapAllocator, undefined);
    assert(policy.heapAllocations.length > 0);
    const heapVariants = policy.heapAllocations.flatMap(({ flags, byteLengths }) =>
      byteLengths.map((byteLength) => `${flags}:${byteLength}`),
    );
    assert.equal(new Set(heapVariants).size, heapVariants.length);
  } else {
    assert.equal(policy.heapAllocations, undefined);
    assert.deepEqual(policy.heapAllocator.allowedFlags, [0, 8]);
    assert.equal(policy.heapAllocator.maximumAllocationBytes, 16 * 1024 * 1024);
    const largeAllocationBytes = policy.heapAllocator.largeAllocationBytes ?? [];
    assert.equal(new Set(largeAllocationBytes).size, largeAllocationBytes.length);
    assert(largeAllocationBytes.every((byteLength) => byteLength === 64 * 1024 * 1024));
  }
  const heapFreeFlags = policy.heapFreeFlags ?? [];
  assert.equal(new Set(heapFreeFlags).size, heapFreeFlags.length);
  assert(heapFreeFlags.every((flags) => flags === 0));
  const heapSizeFlags = policy.heapSizeFlags ?? [];
  assert.equal(new Set(heapSizeFlags).size, heapSizeFlags.length);
  assert(heapSizeFlags.every((flags) => flags === 0 || flags === 1));
  const criticalSection = policy.initializeCriticalSection ?? null;
  if (criticalSection !== null) {
    assert.equal(typeof criticalSection.spinCountField, "number");
    const stateFlags = criticalSection.spinCountField & 0xff000000;
    assert(stateFlags === 0 || stateFlags === 0x02000000);
  }
  const processorFeatures = policy.processorFeatures ?? [];
  assert.equal(
    new Set(processorFeatures.map(({ index }) => index)).size,
    processorFeatures.length,
  );
  assert(processorFeatures.every(({ index, present }) =>
    Number.isInteger(index) && index >= 0 && index <= 0xffffffff &&
      typeof present === "boolean"));
  const threadAffinity = policy.threadAffinity ?? null;
  if (threadAffinity !== null) {
    assert(
      Number.isInteger(threadAffinity.processMask) &&
        threadAffinity.processMask > 0 &&
        threadAffinity.processMask <= 0xffffffff,
    );
  }
  const createThread = policy.createThread ?? null;
  if (createThread !== null) {
    assert.equal(policy.schemaVersion, 2);
    assert.deepEqual(createThread, {
      defaultStackReserveBytes: 1024 * 1024,
      defaultStackCommitBytes: 4096,
      allowedCreationFlags: [0, 4],
      maximumThreads: 256,
    });
  }
  const threadPriority = policy.threadPriority ?? null;
  if (threadPriority !== null) {
    assert.equal(policy.schemaVersion, 2);
    assert(threadPriority.allowedPriorities.length > 0);
    assert.equal(
      new Set(threadPriority.allowedPriorities).size,
      threadPriority.allowedPriorities.length,
    );
    assert(
      threadPriority.allowedPriorities.every((priority) =>
        [-15, -2, -1, 0, 1, 2, 15].includes(priority),
      ),
    );
  }
  const systemInfo = policy.systemInfo ?? null;
  if (systemInfo !== null) {
    assert.equal(policy.schemaVersion, 2);
    assert(threadAffinity !== null);
    assert.deepEqual(systemInfo, {
      processorArchitecture: 0,
      pageSize: 4096,
      minimumApplicationAddress: 0x00010000,
      maximumApplicationAddress: 0x7ffeffff,
      processorType: 586,
      allocationGranularity: 65536,
      processorLevel: 6,
      processorRevision: 0,
    });
  }
  const memoryStatus = policy.memoryStatus ?? null;
  if (memoryStatus !== null) {
    assert.equal(policy.schemaVersion, 2);
    assert(systemInfo !== null);
    assert.deepEqual(memoryStatus, {
      memoryLoadPercent: 0,
      totalPhysicalBytes: 0x7fff_ffff,
      availablePhysicalBytes: 0x7fff_ffff,
      totalPageFileBytes: 0xffff_ffff,
      availablePageFileBytes: 0xffff_ffff,
      availableVirtualBytes: 0x6000_0000,
    });
  }
  const virtualAlloc = policy.virtualAlloc ?? null;
  if (virtualAlloc !== null) {
    assert.equal(policy.schemaVersion, 2);
    assert(systemInfo !== null);
    assert.deepEqual(virtualAlloc, {
      requireNullPreferredAddress: true,
      allowedReserveByteLengths: [96 * 1024 * 1024],
      allowedAllocationTypes: [0x0010_2000],
      allowedProtections: [4],
    });
  }
  const tickCount = policy.tickCount ?? null;
  if (tickCount !== null) {
    assert(
      Number.isInteger(tickCount.incrementPerRead) &&
        tickCount.incrementPerRead >= 1 &&
        tickCount.incrementPerRead <= 1000,
    );
  }
  const osVersionInfoA = policy.osVersionInfoA ?? null;
  if (osVersionInfoA !== null) {
    assert.equal(policy.schemaVersion, 2);
    assert(systemInfo !== null);
    assert.deepEqual(osVersionInfoA, {
      majorVersion: 10,
      minorVersion: 0,
      buildNumber: 19045,
      platformId: 2,
      servicePack: "",
      servicePackMajor: 0,
      servicePackMinor: 0,
      suiteMask: 0x0100,
      productType: 1,
    });
  }
  const systemDefaultLangId = policy.systemDefaultLangId ?? null;
  if (systemDefaultLangId !== null) {
    assert.equal(systemDefaultLangId, 0x0411);
    assert(policy.lcMapStringW !== undefined);
  }
  const multibyte = policy.multiByteToWideChar ?? null;
  if (multibyte?.allowActiveCodePageSelector !== undefined) {
    assert.equal(typeof multibyte.allowActiveCodePageSelector, "boolean");
  }
  const currentDirectory = policy.setCurrentDirectoryA ?? null;
  if (currentDirectory !== null) {
    assert.equal(currentDirectory.processImageDirectoryOnly, true);
    assert(
      Number.isInteger(currentDirectory.maximumPathBytes) &&
        currentDirectory.maximumPathBytes >= 4 &&
        currentDirectory.maximumPathBytes <= 32768,
    );
    assert.equal(multibyte?.allowActiveCodePageSelector, true);
  }
  const sourceFiles = policy.sourceFiles ?? null;
  if (sourceFiles !== null) {
    assert.deepEqual(sourceFiles.allowedDesiredAccess, [0x8000_0000, 0x4000_0000]);
    assert.deepEqual(sourceFiles.allowedShareModes, [1, 0]);
    assert.deepEqual(sourceFiles.allowedCreationDispositions, [3, 4, 2]);
    assert.deepEqual(sourceFiles.allowedFlagsAndAttributes, [0x0800_0020]);
    assert.equal(sourceFiles.maximumPathBytes, 32_768);
    assert.equal(sourceFiles.maximumReadBytes, 1024 * 1024);
    assert.equal(sourceFiles.maximumWriteBytes, 1024 * 1024);
    assert.equal(multibyte?.allowActiveCodePageSelector, true);
    const fileAttributes = sourceFiles.getFileAttributesA ?? null;
    if (fileAttributes !== null) {
      assert.deepEqual(fileAttributes, {
        regularFileAttributes: 0x20,
        directoryAttributes: 0x30,
      });
    }
  }
  const driveType = policy.getDriveTypeA ?? null;
  if (driveType !== null) {
    assert.equal(driveType.processImageDriveOnly, true);
    assert(
      Number.isInteger(driveType.maximumPathBytes) &&
        driveType.maximumPathBytes >= 4 &&
        driveType.maximumPathBytes <= 32768,
    );
  }
  const mutex = policy.createMutexA ?? null;
  if (mutex !== null) {
    assert.equal(typeof mutex.allowNamed, "boolean");
    assert.equal(typeof mutex.allowUnnamed, "boolean");
    assert(mutex.allowNamed || mutex.allowUnnamed);
    assert(
      Number.isInteger(mutex.maximumNameBytes) &&
        mutex.maximumNameBytes >= 2 &&
        mutex.maximumNameBytes <= 32768,
    );
  }
  const waitableTimer = policy.createWaitableTimerA ?? null;
  if (waitableTimer !== null) {
    assert.equal(policy.schemaVersion, 2);
    assert.equal(typeof waitableTimer.allowManualReset, "boolean");
    assert.equal(typeof waitableTimer.allowAutoReset, "boolean");
    assert.equal(waitableTimer.allowUnnamed, true);
    assert(waitableTimer.allowManualReset || waitableTimer.allowAutoReset);
  }
  const setWaitableTimer = policy.setWaitableTimer ?? null;
  if (setWaitableTimer !== null) {
    assert.equal(policy.schemaVersion, 2);
    assert(waitableTimer !== null);
    assert.deepEqual(setWaitableTimer, {
      minimumRelativeDueTime100ns: 1,
      maximumRelativeDueTime100ns: 100_000,
      allowOneShot: true,
      allowCompletionRoutine: false,
      allowResume: false,
    });
  }
  const sleepZero = policy.sleepZero ?? false;
  assert.equal(typeof sleepZero, "boolean");
  if (sleepZero) {
    assert.equal(policy.schemaVersion, 2);
    assert(
      policy.id === "kernel32-sleep-zero-v1" ||
        policy.id === "kernel32-global-memory-status-v1" ||
        policy.id === "kernel32-get-file-attributes-a-v1" ||
        policy.id === "kernel32-create-directory-a-v1",
    );
  }
  const loadLibraryA = policy.loadLibraryA ?? null;
  if (loadLibraryA !== null) {
    assert.equal(policy.schemaVersion, 2);
    assert(
      Number.isInteger(loadLibraryA.maximumModuleNameBytes) &&
        loadLibraryA.maximumModuleNameBytes >= 1 &&
        loadLibraryA.maximumModuleNameBytes <= 260,
    );
    assert(loadLibraryA.runtimeSystemModules.length > 0);
    assert.equal(
      new Set(loadLibraryA.runtimeSystemModules.map(({ canonicalName }) => canonicalName.toLowerCase())).size,
      loadLibraryA.runtimeSystemModules.length,
    );
    for (const module of loadLibraryA.runtimeSystemModules) {
      assert.equal(module.canonicalName, module.canonicalName.toLowerCase());
      assert.match(module.canonicalName, /^[\x21-\x7e]+\.dll$/u);
      assert(module.exports.length > 0);
      assert.equal(new Set(module.exports).size, module.exports.length);
    }
  }
}

const manifest = JSON.parse(
  fs.readFileSync(path.join(root, "docs", "current-checkpoint.json"), "utf8"),
);
const summary = manifest.activeRuntimeCheckpoint.kernel32CompatibilityPolicy;
const activePolicy = JSON.parse(fs.readFileSync(path.join(root, summary.source), "utf8"));
assert.equal(activePolicy.id, summary.id);
assert.equal(activePolicy.schema, summary.schema);
assert.equal(activePolicy.schemaVersion, summary.schemaVersion);
assert.equal(
  activePolicy.multiByteToWideChar.allowActiveCodePageSelector,
  summary.multiByteAllowActiveCodePageSelector,
);
assert.deepEqual(activePolicy.multiByteToWideChar.allowedFlags, summary.multiByteAllowedFlags);
assert.equal(
  activePolicy.setCurrentDirectoryA.processImageDirectoryOnly,
  summary.setCurrentDirectoryProcessImageDirectoryOnly,
);
assert.equal(
  activePolicy.setCurrentDirectoryA.maximumPathBytes,
  summary.setCurrentDirectoryMaximumPathBytes,
);
assert.equal(
  activePolicy.getDriveTypeA.processImageDriveOnly,
  summary.getDriveTypeProcessImageDriveOnly,
);
assert.equal(
  activePolicy.getDriveTypeA.maximumPathBytes,
  summary.getDriveTypeMaximumPathBytes,
);
assert.deepEqual(activePolicy.heapAllocator.allowedFlags, summary.heapAllocator.allowedFlags);
assert.equal(
  activePolicy.heapAllocator.maximumAllocationBytes,
  summary.heapAllocator.maximumAllocationBytes,
);
assert.deepEqual(
  activePolicy.heapAllocator.largeAllocationBytes ?? [],
  summary.heapAllocator.largeAllocationBytes,
);
assert.equal(activePolicy.threadAffinity.processMask, summary.threadAffinityProcessMask);
assert.deepEqual(
  activePolicy.threadPriority?.allowedPriorities ?? null,
  summary.threadPriorityAllowedPriorities ?? null,
);
assert.equal(activePolicy.tickCount.incrementPerRead, summary.tickCountIncrementPerRead);
assert.deepEqual(activePolicy.systemInfo ?? null, summary.systemInfo ?? null);
assert.deepEqual(activePolicy.memoryStatus ?? null, summary.memoryStatus ?? null);
assert.equal(activePolicy.systemDefaultLangId, summary.systemDefaultLangId);
assert.deepEqual(activePolicy.osVersionInfoA, summary.osVersionInfoA);

const implementation = fs.readFileSync(
  path.join(root, "crates", "vine-core", "src", "win32.rs"),
  "utf8",
);
assert.doesNotMatch(implementation, /\b(?:bgi|reached)\b|caller[_ -]?rva/iu);
assert.doesNotMatch(implementation, /implementation.*first.*two/iu);

console.log(`Kernel32 compatibility policy boundary passed: ${policyFiles.length} policies`);
