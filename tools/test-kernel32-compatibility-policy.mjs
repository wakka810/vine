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
}

const manifest = JSON.parse(
  fs.readFileSync(path.join(root, "docs", "current-checkpoint.json"), "utf8"),
);
const summary = manifest.activeRuntimeCheckpoint.kernel32CompatibilityPolicy;
const activePolicy = JSON.parse(fs.readFileSync(path.join(root, summary.source), "utf8"));
assert.equal(activePolicy.id, summary.id);
assert.equal(activePolicy.schema, summary.schema);
assert.equal(activePolicy.schemaVersion, summary.schemaVersion);

const implementation = fs.readFileSync(
  path.join(root, "crates", "vine-core", "src", "win32.rs"),
  "utf8",
);
assert.doesNotMatch(implementation, /\b(?:bgi|reached)\b|caller[_ -]?rva/iu);
assert.doesNotMatch(implementation, /implementation.*first.*two/iu);

console.log(`Kernel32 compatibility policy boundary passed: ${policyFiles.length} policies`);
