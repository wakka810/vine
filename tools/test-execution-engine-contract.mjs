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

async function initializeBoundaryEngine({ entryPoint, firstPage, secondPage, secondExecutable }) {
  const boundaryEngine = await instantiateExecutionEngineBytes(adapterBytes);
  const stackAddress = 0x3000;
  boundaryEngine.initialize(
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
          owner: 9,
          protection: protection.code,
          ownerSealed: true,
          dirty: true,
          bytes: firstPage,
        },
        {
          address: dataAddress,
          owner: 9,
          protection: { read: true, write: false, execute: secondExecutable },
          ownerSealed: true,
          dirty: true,
          bytes: secondPage,
        },
        {
          address: stackAddress,
          owner: 9,
          protection: protection.data,
          ownerSealed: true,
          dirty: true,
          bytes: new Uint8Array(pageSize),
        },
      ],
    },
    {
      entryPoint,
      initialStackPointer: stackAddress + pageSize - 16,
      fsBase: 0,
      regions: [
        {
          owner: 9,
          kind: "runtime",
          range: { start: codeAddress, byteLength: pageSize * 3 },
        },
      ],
    },
  );
  return boundaryEngine;
}
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

const secondContext = engine.createCpuContext({
  entryPoint: codeAddress + 10,
  initialStackPointer: dataAddress + pageSize - 32,
  fsBase: 0x1234_0000,
});
assert.deepEqual(
  engine.saveCpuContext(),
  initialContext,
  "creating a thread context mutated the live CPU context",
);
assert.equal(secondContext.eax, 0);
assert.equal(secondContext.esp, dataAddress + pageSize - 32);
assert.equal(secondContext.instructionPointer, codeAddress + 10);
engine.restoreCpuContext(secondContext);
assert.equal(engine.saveCpuContext().instructionPointer, codeAddress + 10);
engine.restoreCpuContext(initialContext);

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
assert.deepEqual(engine.performanceMetrics(), {
  jitCompilations: 0,
  jitClears: 0,
  jitActiveFunctions: 0,
  jitInstructions: 0,
  interpretedInstructions: 3,
});

engine.applyStoppedCpuResume({
  instructionPointer: codeAddress,
  stackPointer: dataAddress + pageSize - 32,
  eax: { action: "set", value: 0xa5a5_5a5a },
  edx: { action: "preserve" },
  eflags: { action: "set", value: 0x202 },
});
const stoppedResume = engine.saveCpuContext();
assert.equal(stoppedResume.instructionPointer, codeAddress);
assert.equal(stoppedResume.previousInstructionPointer, codeAddress);
assert.equal(stoppedResume.esp, dataAddress + pageSize - 32);
assert.equal(stoppedResume.eax, 0xa5a5_5a5a);
assert.equal(stoppedResume.edx, trap.context.edx);
assert.equal(stoppedResume.eflags, 0x202);
engine.restoreCpuContext(trap.context);

const pageEndFirst = new Uint8Array(pageSize);
pageEndFirst[pageSize - 1] = 0x90;
const pageEndSecond = new Uint8Array(pageSize);
pageEndSecond[0] = 0x90;
const pageEndEngine = await initializeBoundaryEngine({
  entryPoint: dataAddress - 1,
  firstPage: pageEndFirst,
  secondPage: pageEndSecond,
  secondExecutable: false,
});
const pageEndInstruction = pageEndEngine.executeBudget(1);
assert.deepEqual(pageEndInstruction.stop, { kind: "budget-yield" });
assert.equal(pageEndInstruction.instructions, 1);
assert.equal(pageEndInstruction.context.instructionPointer, dataAddress);
const nextPageDenied = pageEndEngine.executeBudget(1);
assert.deepEqual(nextPageDenied.stop, {
  kind: "fault",
  fault: "execute-protection",
  errorCode: 0x15,
  engineReason: 3,
  faultAddress: dataAddress,
});
assert.equal(nextPageDenied.instructions, 1);
assert.equal(nextPageDenied.context.instructionPointer, dataAddress);
assert.equal(nextPageDenied.context.previousInstructionPointer, dataAddress);

const crossingFirst = new Uint8Array(pageSize);
crossingFirst.set([0xb8, 0x78], pageSize - 2);
const crossingSecond = new Uint8Array(pageSize);
crossingSecond.set([0x56, 0x34, 0x12]);
const crossingDeniedEngine = await initializeBoundaryEngine({
  entryPoint: dataAddress - 2,
  firstPage: crossingFirst,
  secondPage: crossingSecond,
  secondExecutable: false,
});
assert.equal(
  crossingDeniedEngine.readGuestU32(dataAddress - 2),
  0x3456_78b8,
  "guest u32 read did not preserve a readable cross-page range",
);
const crossingStackBefore = crossingDeniedEngine.readGuestU32(0x3000);
const crossingDenied = crossingDeniedEngine.executeBudget(1);
assert.deepEqual(crossingDenied.stop, {
  kind: "fault",
  fault: "execute-protection",
  errorCode: 0x15,
  engineReason: 3,
  faultAddress: dataAddress,
});
assert.equal(crossingDenied.context.eax, 0);
assert.equal(crossingDenied.context.instructionPointer, dataAddress - 2);
assert.equal(crossingDenied.context.previousInstructionPointer, dataAddress - 2);
assert.equal(crossingDeniedEngine.readGuestU32(0x3000), crossingStackBefore);

const crossingAllowedEngine = await initializeBoundaryEngine({
  entryPoint: dataAddress - 2,
  firstPage: crossingFirst,
  secondPage: crossingSecond,
  secondExecutable: true,
});
const crossingAllowed = crossingAllowedEngine.executeBudget(1);
assert.deepEqual(crossingAllowed.stop, { kind: "budget-yield" });
assert.equal(crossingAllowed.instructions, 1);
assert.equal(crossingAllowed.context.eax, 0x1234_5678);
assert.equal(crossingAllowed.context.instructionPointer, dataAddress + 3);
crossingAllowedEngine.applyMemoryDelta({
  fromGeneration: 1n,
  generation: 2n,
  cacheEffects: {
    flushTlb: true,
    invalidateInstructionFetch: true,
    invalidateGeneratedCode: true,
  },
  mappingOperations: [
    {
      operation: "protect",
      owner: 9,
      protection: { read: true, write: false, execute: false },
      range: { base: dataAddress, byteLength: pageSize },
    },
  ],
  pages: [],
});
const protectedAfterSlice = crossingAllowedEngine.executeBudget(1);
assert.deepEqual(protectedAfterSlice.stop, {
  kind: "fault",
  fault: "execute-protection",
  errorCode: 0x15,
  engineReason: 3,
  faultAddress: dataAddress + 3,
});
assert.equal(protectedAfterSlice.context.instructionPointer, dataAddress + 3);

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

const pageOnlyBytes = Uint8Array.from(mappedBytes);
new DataView(pageOnlyBytes.buffer).setUint32(0, 0x1357_9bdf, true);
engine.applyMemoryDelta({
  fromGeneration: 2n,
  generation: 3n,
  cacheEffects: {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  },
  mappingOperations: [],
  pages: [
    {
      address: mappedAddress,
      owner: 2,
      protection: protection.data,
      ownerSealed: true,
      dirty: true,
      bytes: pageOnlyBytes,
    },
  ],
});
assert.equal(engine.readGuestU32(mappedAddress), 0x1357_9bdf);
assert.throws(
  () =>
    engine.applyMemoryDelta({
      fromGeneration: 3n,
      generation: 4n,
      cacheEffects: {
        flushTlb: false,
        invalidateInstructionFetch: false,
        invalidateGeneratedCode: false,
      },
      mappingOperations: [],
      pages: [
        {
          address: mappedAddress,
          owner: 3,
          protection: protection.data,
          ownerSealed: true,
          dirty: true,
          bytes: mappedBytes,
        },
      ],
    }),
  /owner does not match/u,
);
assert.equal(engine.readGuestU32(mappedAddress), 0x1357_9bdf);

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
assert.equal(engine.readGuestU32(mappedAddress), 0x1357_9bdf);

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

const growthEngine = await instantiateExecutionEngineBytes(adapterBytes, undefined, {
  maximumPhysicalPages: 600,
});
growthEngine.initialize(
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
const capacityBeforeGrowth = growthEngine.memoryCapacity();
assert.equal(capacityBeforeGrowth.physicalPoolBytes, 64 * 1024);
const expansionPageCount = 512;
const expansionBase = 0x2000_0000;
const expansionRange = { base: expansionBase, byteLength: expansionPageCount * pageSize };
const expandedPages = Array.from({ length: expansionPageCount }, (_, index) => ({
  address: expansionBase + index * pageSize,
  owner: 3,
  protection: protection.data,
  ownerSealed: true,
  dirty: true,
  bytes: new Uint8Array(pageSize),
}));
growthEngine.applyMemoryDelta({
  fromGeneration: 1n,
  generation: 2n,
  cacheEffects: {
    flushTlb: true,
    invalidateInstructionFetch: true,
    invalidateGeneratedCode: true,
  },
  mappingOperations: [
    { operation: "reserve", owner: 3, kind: "anonymous", range: expansionRange },
    {
      operation: "commit",
      owner: 3,
      protection: protection.data,
      range: expansionRange,
    },
  ],
  pages: expandedPages,
});
const capacityAfterGrowth = growthEngine.memoryCapacity();
assert.equal(capacityAfterGrowth.growths, 1);
assert(capacityAfterGrowth.physicalPoolBytes > 2 * 1024 * 1024);
assert(capacityAfterGrowth.physicalPoolBytes <= capacityAfterGrowth.maximumPhysicalPoolBytes);
assert.deepEqual(growthEngine.executeBudget(1).stop, { kind: "budget-yield" });

growthEngine.applyMemoryDelta({
  fromGeneration: 2n,
  generation: 3n,
  cacheEffects: {
    flushTlb: true,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  },
  mappingOperations: [
    {
      operation: "decommit",
      owner: 3,
      range: { base: expansionBase, byteLength: pageSize },
    },
  ],
  pages: [],
});
const capacityAfterRelease = growthEngine.memoryCapacity();
assert(capacityAfterRelease.releasedPages > capacityAfterGrowth.releasedPages);
const reusedBytes = new Uint8Array(pageSize);
reusedBytes[0] = 0x5a;
growthEngine.applyMemoryDelta({
  fromGeneration: 3n,
  generation: 4n,
  cacheEffects: {
    flushTlb: true,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  },
  mappingOperations: [
    {
      operation: "commit",
      owner: 3,
      protection: protection.data,
      range: { base: expansionBase, byteLength: pageSize },
    },
  ],
  pages: [
    {
      address: expansionBase,
      owner: 3,
      protection: protection.data,
      ownerSealed: true,
      dirty: true,
      bytes: reusedBytes,
    },
  ],
});
const capacityAfterReuse = growthEngine.memoryCapacity();
assert(capacityAfterReuse.reusedPages > capacityAfterRelease.reusedPages);
assert.equal(growthEngine.readGuestU32(expansionBase), 0x5a);

growthEngine.applyMemoryDelta({
  fromGeneration: 4n,
  generation: 5n,
  cacheEffects: {
    flushTlb: true,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  },
  mappingOperations: [{ operation: "release", owner: 3, range: expansionRange }],
  pages: [],
});
const capacityAfterRangeRelease = growthEngine.memoryCapacity();
assert(capacityAfterRangeRelease.pageTablePages < capacityAfterReuse.pageTablePages);
assert(capacityAfterRangeRelease.freePhysicalPages > capacityAfterReuse.freePhysicalPages);

const exhaustedEngine = await instantiateExecutionEngineBytes(adapterBytes, undefined, {
  maximumPhysicalPages: 4,
});
const overLimitPages = Array.from({ length: 5 }, (_, index) => ({
  address: codeAddress + index * pageSize,
  owner: 1,
  protection: index === 0 ? protection.code : protection.data,
  ownerSealed: true,
  dirty: true,
  bytes: index === 0 ? code : new Uint8Array(pageSize),
}));
const beforeExhaustion = exhaustedEngine.memoryContractState();
assert.throws(
  () =>
    exhaustedEngine.initialize(
      {
        generation: 1n,
        cacheEffects: {
          flushTlb: false,
          invalidateInstructionFetch: false,
          invalidateGeneratedCode: false,
        },
        pages: overLimitPages,
      },
      {
        entryPoint: codeAddress,
        initialStackPointer: dataAddress + pageSize - 16,
        regions: [
          {
            owner: 1,
            kind: "runtime",
            range: { start: codeAddress, byteLength: pageSize * 5 },
          },
        ],
      },
    ),
  (error) =>
    error?.code === "v86-physical-pool-exhausted" &&
    error.requestedPhysicalPages === 5 &&
    error.maximumPhysicalPages === 4,
);
assert.equal(exhaustedEngine.memoryContractState(), beforeExhaustion);

exhaustedEngine.initialize(
  {
    generation: 1n,
    cacheEffects: {
      flushTlb: false,
      invalidateInstructionFetch: false,
      invalidateGeneratedCode: false,
    },
    pages: overLimitPages.slice(0, 2),
  },
  {
    entryPoint: codeAddress,
    initialStackPointer: dataAddress + pageSize - 16,
    regions: [
      {
        owner: 1,
        kind: "runtime",
        range: { start: codeAddress, byteLength: pageSize * 2 },
      },
    ],
  },
);
const deltaExhaustionRange = { base: expansionBase, byteLength: pageSize * 3 };
const beforeDeltaExhaustion = exhaustedEngine.memoryContractState();
const capacityBeforeDeltaExhaustion = exhaustedEngine.memoryCapacity();
const contextBeforeDeltaExhaustion = exhaustedEngine.saveCpuContext();
assert.throws(
  () =>
    exhaustedEngine.applyMemoryDelta({
      fromGeneration: 1n,
      generation: 2n,
      cacheEffects: {
        flushTlb: true,
        invalidateInstructionFetch: false,
        invalidateGeneratedCode: false,
      },
      mappingOperations: [
        {
          operation: "reserve",
          owner: 3,
          kind: "anonymous",
          range: deltaExhaustionRange,
        },
        {
          operation: "commit",
          owner: 3,
          protection: protection.data,
          range: deltaExhaustionRange,
        },
      ],
      pages: overLimitPages.slice(2).map((page, index) => ({
        ...page,
        address: expansionBase + index * pageSize,
        owner: 3,
      })),
    }),
  (error) =>
    error?.code === "v86-physical-pool-exhausted" &&
    error.requestedPhysicalPages === 5 &&
    error.maximumPhysicalPages === 4,
);
assert.equal(exhaustedEngine.memoryContractState(), beforeDeltaExhaustion);
assert.deepEqual(exhaustedEngine.memoryCapacity(), capacityBeforeDeltaExhaustion);
assert.deepEqual(exhaustedEngine.saveCpuContext(), contextBeforeDeltaExhaustion);

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
