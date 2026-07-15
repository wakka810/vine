import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const adapterPath = path.join(
  root,
  ".work",
  "v86-adapter-source",
  "build",
  "v86-fallback.wasm",
);
assert(fs.existsSync(adapterPath), "prepared execution engine adapter is missing");

const runtimeLoop = fs.readFileSync(
  path.join(root, "browser", "runtime-worker", "runtime-worker.ts"),
  "utf8",
);
for (const forbidden of [
  /\bV86Mirror\b/u,
  /v86StopReason/u,
  /adapterPrivateOffset/u,
  /pteAddress/u,
  /full_clear_tlb/u,
]) {
  assert.doesNotMatch(runtimeLoop, forbidden, `Runtime Worker leaked ${forbidden}`);
}

const outputDirectory = path.join(root, ".work", "execution-engine-contract");
fs.mkdirSync(outputDirectory, { recursive: true });
const bundle = path.join(outputDirectory, "execution-engine.mjs");
await build({
  entryPoints: [
    path.join(root, "browser", "runtime-worker", "execution-engine.ts"),
  ],
  bundle: true,
  format: "esm",
  outfile: bundle,
  platform: "node",
  target: "node24",
});
const { ExecutionMemoryAccessFault, instantiateExecutionEngineBytes } = await import(
  `${pathToFileURL(bundle).href}?v=${Date.now()}`
);

const adapterBytes = new Uint8Array(fs.readFileSync(adapterPath));
const engine = await instantiateExecutionEngineBytes(adapterBytes);
const identity = engine.identity();
const profile = JSON.parse(
  fs.readFileSync(
    path.join(root, "fixtures", "execution-engines", "v86-browser-v1.json"),
    "utf8",
  ),
);
const expectedArtifact = profile.artifacts.find(
  (artifact) =>
    artifact.bytes === adapterBytes.byteLength && artifact.sha256 === identity.artifact.sha256,
);
assert(expectedArtifact !== undefined, "adapter is not listed in the execution engine profile");
assert.deepEqual(identity, {
  schema: profile.schema,
  schemaVersion: profile.schemaVersion,
  engine: profile.engine,
  sourceRevision: profile.sourceRevision,
  cpuContextAbi: profile.cpuContextAbi,
  artifact: {
    bytes: expectedArtifact.bytes,
    sha256: expectedArtifact.sha256,
  },
});
const reorderedIdentity = {
  artifact: structuredClone(identity.artifact),
  cpuContextAbi: structuredClone(identity.cpuContextAbi),
  sourceRevision: identity.sourceRevision,
  engine: identity.engine,
  schemaVersion: identity.schemaVersion,
  schema: identity.schema,
};
assert.deepEqual(
  (await instantiateExecutionEngineBytes(adapterBytes, reorderedIdentity)).identity(),
  identity,
  "execution engine identity comparison depends on property order",
);

const adapterReport = JSON.parse(
  fs.readFileSync(path.join(root, ".work", "v86-adapter-spike", "report.json"), "utf8"),
);
assert.equal(adapterReport.sourceRevision, profile.sourceRevision);
assert.equal(adapterReport.results.cpuState.version, profile.cpuContextAbi.version);
assert.equal(adapterReport.results.cpuState.size, profile.cpuContextAbi.bytes);
assert.equal(adapterReport.results.invalidOpcode.reason, adapterReport.stopReason.invalidOpcode);
assert.equal(adapterReport.results.pageFault.reason, adapterReport.stopReason.pageFault);
assert.equal(adapterReport.results.halt.reason, adapterReport.stopReason.halt);

const pageSize = 4096;
const codeAddress = 0x1000;
const dataAddress = 0x2000;
const code = new Uint8Array(pageSize);
code.set([
  0xc7, 0x05, 0x00, 0x20, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12,
  0x0f, 0x0b,
]);
const protection = {
  code: { read: true, write: false, execute: true },
  data: { read: true, write: true, execute: false },
};
engine.initialize(
  {
    generation: 1n,
    cacheEffects: {
      flushTlb: false,
      invalidateInstructionFetch: false,
      invalidateGeneratedCode: false,
    },
    pages: [
      {
        address: codeAddress,
        owner: 1,
        protection: protection.code,
        ownerSealed: true,
        dirty: true,
        bytes: code,
      },
      {
        address: dataAddress,
        owner: 1,
        protection: protection.data,
        ownerSealed: true,
        dirty: true,
        bytes: new Uint8Array(pageSize),
      },
    ],
  },
  {
    entryPoint: codeAddress,
    initialStackPointer: dataAddress + pageSize - 16,
    fsBase: 0,
    regions: [
      {
        owner: 1,
        kind: "runtime",
        range: { start: codeAddress, byteLength: pageSize * 2 },
      },
    ],
  },
);

const initialContext = engine.saveCpuContext();
assert.equal(initialContext.version, profile.cpuContextAbi.version);
assert.equal(initialContext.byteLength, profile.cpuContextAbi.bytes);
assert.equal(initialContext.opaqueWords.byteLength, profile.cpuContextAbi.bytes);
assert.equal(initialContext.instructionPointer, codeAddress);

const budget = engine.executeBudget(1);
assert.deepEqual(budget.stop, { kind: "budget-yield" });
assert.equal(budget.instructions, 1);
const changed = engine.collectChangedPages();
assert.equal(changed.length, 1);
assert.equal(changed[0].address, dataAddress);
assert.equal(changed[0].owner, 1);
assert.equal(new DataView(changed[0].bytes.buffer).getUint32(0, true), 0x1234_5678);

engine.restoreCpuContext(initialContext);
assert.equal(engine.saveCpuContext().instructionPointer, codeAddress);
const trap = engine.executeBudget(16);
assert.deepEqual(trap.stop, {
  kind: "trap",
  trap: "invalid-opcode",
  errorCode: -1,
});
assert.equal(trap.context.instructionPointer, codeAddress + 10);

const mappedAddress = 0x4000;
const mappedBytes = new Uint8Array(pageSize);
new DataView(mappedBytes.buffer).setUint32(0, 0xa5a5_5a5a, true);
engine.applyMemoryDelta({
  fromGeneration: 1n,
  generation: 2n,
  cacheEffects: {
    flushTlb: true,
    invalidateInstructionFetch: true,
    invalidateGeneratedCode: true,
  },
  mappingOperations: [
    {
      operation: "reserve",
      addressAllocationId: 41,
      owner: 2,
      kind: "heap",
      range: { base: mappedAddress, byteLength: pageSize },
    },
    {
      operation: "commit",
      addressAllocationId: 41,
      owner: 2,
      protection: protection.data,
      range: { base: mappedAddress, byteLength: pageSize },
    },
  ],
  pages: [
    {
      address: mappedAddress,
      owner: 2,
      protection: protection.data,
      ownerSealed: true,
      dirty: true,
      bytes: mappedBytes,
    },
  ],
});
assert.equal(engine.readGuestU32(mappedAddress), 0xa5a5_5a5a);
assert.deepEqual(engine.cacheEffectCounts(), {
  tlbFlushes: 1,
  instructionFetchInvalidations: 1,
  generatedCodeInvalidations: 1,
});

assert.throws(
  () =>
    engine.applyMemoryDelta({
      fromGeneration: 2n,
      generation: 3n,
      cacheEffects: {
        flushTlb: false,
        invalidateInstructionFetch: false,
        invalidateGeneratedCode: false,
      },
      mappingOperations: [
        {
          operation: "protect",
          addressAllocationId: 42,
          owner: 2,
          protection: { read: true, write: false, execute: false },
          range: { base: mappedAddress, byteLength: pageSize },
        },
      ],
      pages: [],
    }),
  /address allocation mismatch/u,
);
assert.equal(engine.readGuestU32(mappedAddress), 0xa5a5_5a5a);

engine.applyMemoryDelta({
  fromGeneration: 2n,
  generation: 3n,
  cacheEffects: {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  },
  mappingOperations: [
    {
      operation: "protect",
      addressAllocationId: 41,
      owner: 2,
      protection: { read: true, write: false, execute: false },
      range: { base: mappedAddress, byteLength: pageSize },
    },
    {
      operation: "decommit",
      addressAllocationId: 41,
      owner: 2,
      range: { base: mappedAddress, byteLength: pageSize },
    },
    {
      operation: "release",
      addressAllocationId: 41,
      owner: 2,
      range: { base: mappedAddress, byteLength: pageSize },
    },
  ],
  pages: [],
});
assert.throws(
  () => engine.readGuestU32(mappedAddress),
  (error) =>
    error instanceof ExecutionMemoryAccessFault &&
    error.code === "execution-address-unmapped",
);

const wrongRevision = structuredClone(identity);
wrongRevision.sourceRevision = "0".repeat(40);
await assert.rejects(
  () => instantiateExecutionEngineBytes(adapterBytes, wrongRevision),
  /source revision, ABI, or artifact profile mismatch/u,
);
const wrongAbi = structuredClone(identity);
wrongAbi.cpuContextAbi.version += 1;
await assert.rejects(
  () => instantiateExecutionEngineBytes(adapterBytes, wrongAbi),
  /source revision, ABI, or artifact profile mismatch/u,
);
const tampered = Uint8Array.from(adapterBytes);
tampered[tampered.length - 1] ^= 1;
await assert.rejects(
  () => instantiateExecutionEngineBytes(tampered),
  /artifact identity mismatch/u,
);

console.log(
  `ExecutionEngine contract passed: ${identity.engine} ${identity.sourceRevision.slice(0, 12)}, ` +
    `CPU context v${initialContext.version}/${initialContext.byteLength} bytes`,
);
