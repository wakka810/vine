import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const policyFiles = [
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

for (const relative of policyFiles) {
  const policy = JSON.parse(fs.readFileSync(path.join(root, relative), "utf8"));
  assert.equal(policy.schema, "vine.kernel32-compatibility-policy");
  assert.equal(policy.schemaVersion, 1);
  assert.match(policy.id, /^[a-z0-9-]{1,128}$/u);
  assert(policy.standardHandleSelectors.length > 0);
  assert.equal(
    new Set(policy.standardHandleSelectors).size,
    policy.standardHandleSelectors.length,
  );
  assert(policy.heapAllocations.length > 0);
  const heapVariants = policy.heapAllocations.flatMap(({ flags, byteLengths }) =>
    byteLengths.map((byteLength) => `${flags}:${byteLength}`),
  );
  assert.equal(new Set(heapVariants).size, heapVariants.length);
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

const implementation = fs.readFileSync(
  path.join(root, "crates", "vine-core", "src", "win32.rs"),
  "utf8",
);
assert.doesNotMatch(implementation, /\b(?:bgi|reached)\b|caller[_ -]?rva/iu);
assert.doesNotMatch(implementation, /implementation.*first.*two/iu);

console.log(`Kernel32 compatibility policy boundary passed: ${policyFiles.length} policies`);
