import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

import { resolveV86AdapterProfile } from "./v86-adapter-profile.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const wasmPath = path.resolve(
  process.argv[2] ??
    path.join(root, ".work", "v86-adapter-source", "build", "v86-fallback.wasm"),
);
const outputDirectory = path.join(root, ".work", "v86-memory-writeback");
const reportPath = path.join(outputDirectory, "report.json");

const frame = {
  magic: "VINEFR01",
  version: 1,
  headerBytes: 24,
  initialReflection: 1,
  acknowledgement: 2,
  cpuWriteback: 3,
  writebackReceipt: 4,
  hostDelta: 5,
};
const reflectionFormat = {
  magic: "VINERF01",
  version: 1,
  headerBytes: 40,
  descriptorBytes: 16,
};
const writebackFormat = {
  magic: "VINEWB01",
  version: 1,
  headerBytes: 32,
  descriptorBytes: 8,
};
const deltaFormat = {
  magic: "VINEDL01",
  version: 2,
  headerBytes: 56,
  mappingDescriptorBytes: 40,
  pageDescriptorBytes: 16,
};
const page = {
  size: 0x1000,
  present: 1,
  writable: 2,
  user: 4,
  dirty: 0x40,
};
const guest = {
  physicalPoolBytes: 2 * 1024 * 1024,
  entryPoint: 0x1000_0000,
  stackBase: 0x2000_0000,
  heapBase: 0x3000_0000,
  surfaceBase: 0x4000_0000,
  codeTargetBase: 0x5000_0000,
  scratchBase: 0x6000_0000,
};
const adapterPrivateOffset = { memorySize: 812, instructionCounter: 664 };
const stateWord = {
  magic: 0,
  version: 1,
  wordCount: 2,
  eax: 4,
  ecx: 5,
  esp: 8,
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
const stopReason = { invalidOpcode: 0x106 };

async function main() {
  requireFile(wasmPath, "npm run prepare:engine");
  const child = spawn(
    "cargo",
    [
      "run",
      "--quiet",
      "--locked",
      "-p",
      "vine-pe-probe",
      "--bin",
      "vine-memory-writeback-session",
    ],
    { cwd: root, stdio: ["pipe", "pipe", "pipe"] },
  );
  let stderr = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  const reader = createReader(child.stdout);

  try {
    const initialFrame = await readFrame(reader, frame.initialReflection);
    const reflection = decodeReflection(initialFrame.control, initialFrame.payload);
    assert.equal(reflection.pages.length, 9);
    const context = await prepareV86(reflection);
    applyInitialCacheEffects(context, reflection.cacheEffects);
    await writeFrame(
      child.stdin,
      frame.acknowledgement,
      encodeGeneration(reflection.generation),
      Buffer.alloc(0),
    );

    const firstExecution = executeGuest(context, {
      instructionPointer: guest.entryPoint + 0x3b,
      eax: 0x1122_3344,
      ecx: 0,
      esp: guest.stackBase + 0x0ffc,
      edi: guest.heapBase + 0x1010,
    });
    const firstDiscovery = discoverDirtyPages(context, [
      guest.stackBase,
      guest.stackBase + page.size,
      guest.heapBase,
      guest.heapBase + page.size,
      guest.surfaceBase,
      guest.surfaceBase + page.size,
      guest.codeTargetBase,
    ]);
    validateGuestBytes(context);
    const firstWriteback = encodeWriteback(
      reflection.generation,
      firstDiscovery.pages,
      context,
    );
    await writeFrame(
      child.stdin,
      frame.cpuWriteback,
      firstWriteback.control,
      firstWriteback.payload,
    );
    const firstReceiptFrame = await readFrame(reader, frame.writebackReceipt);
    assert.equal(firstReceiptFrame.payload.length, 0);
    const firstReceipt = decodeReceipt(firstReceiptFrame.control);
    validateReceipt(firstReceipt, {
      fromGeneration: reflection.generation,
      dirtyPages: firstDiscovery.pages.length,
      changedBytes: 49,
    });

    const hostDeltaFrame = await readFrame(reader, frame.hostDelta);
    const hostDelta = decodeHostDelta(hostDeltaFrame.control, hostDeltaFrame.payload);
    assert.equal(hostDelta.fromGeneration, firstReceipt.generation);
    assert.equal(hostDelta.generation, firstReceipt.generation + 1);
    assert.equal(hostDelta.mappingOperations.length, 0);
    assert.deepEqual(hostDelta.dirtyPages.map(({ address }) => address), [guest.scratchBase]);
    applyHostDelta(context, hostDelta);
    await writeFrame(
      child.stdin,
      frame.acknowledgement,
      encodeGeneration(hostDelta.generation),
      Buffer.alloc(0),
    );

    prepareSecondSlice(context);
    const secondExecution = executeGuest(context, {
      instructionPointer: guest.entryPoint + 0x100 + 0x25,
      eax: 0x1122_3344,
      ecx: 0,
      esp: guest.stackBase + 0x0ffc,
      edi: guest.heapBase + 0x1010,
    });
    const secondDiscovery = discoverDirtyPages(context, [
      guest.stackBase,
      guest.heapBase + page.size,
      guest.surfaceBase,
      guest.codeTargetBase,
    ]);
    validateSecondGuestBytes(context);
    const secondWriteback = encodeWriteback(
      hostDelta.generation,
      secondDiscovery.pages,
      context,
    );
    await writeFrame(
      child.stdin,
      frame.cpuWriteback,
      secondWriteback.control,
      secondWriteback.payload,
    );
    child.stdin.end();
    const secondReceiptFrame = await readFrame(reader, frame.writebackReceipt);
    assert.equal(secondReceiptFrame.payload.length, 0);
    const secondReceipt = decodeReceipt(secondReceiptFrame.control);
    validateReceipt(secondReceipt, {
      fromGeneration: hostDelta.generation,
      dirtyPages: secondDiscovery.pages.length,
      changedBytes: 13,
    });
    const exitCode = await new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("close", resolve);
    });
    assert.equal(exitCode, 0, stderr || "memory writeback session failed");

    const report = {
      schema: "vine.v86-memory-writeback",
      schemaVersion: 1,
      byteAuthority: "rust-guest-memory",
      dirtyDiscovery: "v86-pte-dirty-bit",
      initialGeneration: reflection.generation,
      reflection: {
        pages: reflection.pages.length,
        writablePages: reflection.pages.filter(({ protection }) => protection.write).length,
        controlBytes: initialFrame.control.length,
        payloadBytes: initialFrame.payload.length,
        payloadSha256: sha256(initialFrame.payload),
      },
      hostDelta: {
        fromGeneration: hostDelta.fromGeneration,
        generation: hostDelta.generation,
        mappingOperations: hostDelta.mappingOperations.length,
        dirtyPages: hostDelta.dirtyPages.length,
        controlBytes: hostDeltaFrame.control.length,
        payloadBytes: hostDeltaFrame.payload.length,
        payloadSha256: sha256(hostDeltaFrame.payload),
        cacheEffects: hostDelta.cacheEffects,
        jsonPageByteArrays: false,
      },
      slices: [
        sliceReport(
          firstExecution,
          firstDiscovery,
          firstWriteback,
          firstReceipt,
        ),
        sliceReport(
          secondExecution,
          secondDiscovery,
          secondWriteback,
          secondReceipt,
        ),
      ],
    };
    fs.mkdirSync(outputDirectory, { recursive: true });
    const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
    fs.writeFileSync(reportPath, reportBytes);
    console.log(
      `v86 memory writeback passed: ${firstDiscovery.pages.length}+` +
        `${secondDiscovery.pages.length} dirty pages, generation ` +
        `${reflection.generation}->${firstReceipt.generation}->` +
        `${hostDelta.generation}->${secondReceipt.generation}`,
    );
    console.log(
      `report: ${path.relative(root, reportPath)}, ${reportBytes.length} bytes, ` +
        `sha256 ${sha256(reportBytes)}`,
    );
  } finally {
    if (child.exitCode === null) {
      child.kill();
    }
  }
}

function decodeReflection(control, payload) {
  assert.equal(control.subarray(0, 8).toString("ascii"), reflectionFormat.magic);
  const view = dataView(control);
  assert.equal(view.getUint32(8, true), reflectionFormat.version);
  assert.equal(view.getUint32(12, true), page.size);
  const generation = safeNumber(view.getBigUint64(16, true), "reflection generation");
  const pageCount = view.getUint32(24, true);
  assert.equal(view.getUint32(28, true), reflectionFormat.descriptorBytes);
  assert.equal(
    control.length,
    reflectionFormat.headerBytes + pageCount * reflectionFormat.descriptorBytes,
  );
  assert.equal(payload.length, pageCount * page.size);
  const cacheFlags = view.getUint32(32, true);
  const pages = [];
  for (let index = 0; index < pageCount; index += 1) {
    const offset = reflectionFormat.headerBytes + index * reflectionFormat.descriptorBytes;
    const flags = view.getUint32(offset + 8, true);
    pages.push({
      address: view.getUint32(offset, true),
      owner: view.getUint32(offset + 4, true),
      protection: {
        read: Boolean(flags & 1),
        write: Boolean(flags & 2),
        execute: Boolean(flags & 4),
      },
      ownerSealed: Boolean(flags & 8),
      dirty: Boolean(flags & 16),
      bytes: payload.subarray(index * page.size, (index + 1) * page.size),
    });
  }
  assert(pages.every(({ ownerSealed, dirty }) => ownerSealed && dirty));
  return {
    generation,
    cacheEffects: {
      flushTlb: Boolean(cacheFlags & 1),
      invalidateInstructionFetch: Boolean(cacheFlags & 2),
      invalidateGeneratedCode: Boolean(cacheFlags & 4),
    },
    pages,
  };
}

function decodeHostDelta(control, payload) {
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
    const addressAllocationId = safeNumber(
      view.getBigUint64(offset + 28, true),
      "mapping address allocation ID",
    );
    assert.equal(view.getUint32(offset + 36, true), 0);
    mappingOperations.push({
      operation: view.getUint32(offset, true),
      ...(addressAllocationId === 0 ? {} : { addressAllocationId }),
      owner: view.getUint32(offset + 4, true),
      regionKind: view.getUint32(offset + 8, true),
      protectionFlags: view.getUint32(offset + 12, true),
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

function applyHostDelta(context, delta) {
  assert.deepEqual(delta.mappingOperations, []);
  for (const dirty of delta.dirtyPages) {
    const mapping = context.mappings.get(dirty.address);
    assert(mapping, `host dirty page 0x${dirty.address.toString(16)} is unmapped`);
    assert.equal(mapping.owner, dirty.owner);
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

async function prepareV86(reflection) {
  const wasmBytes = fs.readFileSync(wasmPath);
  const adapterProfile = resolveV86AdapterProfile(wasmBytes);
  const module = new WebAssembly.Module(wasmBytes);
  const imports = WebAssembly.Module.imports(module);
  assert.equal(imports.length, adapterProfile.wasm.imports);
  assert.equal(WebAssembly.Module.exports(module).length, adapterProfile.wasm.exports);
  const environment = {
    __indirect_function_table: new WebAssembly.Table({ element: "anyfunc", initial: 1_924 }),
  };
  for (const imported of imports) {
    if (imported.kind === "function") {
      environment[imported.name] = (...args) => {
        assert.equal(imported.name, "microtick");
        assert.equal(args.length, 0);
        return 0;
      };
    }
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
  const physicalView = dataView(physicalMemory);
  wasm.zero_memory(0, guest.physicalPoolBytes);

  let nextPhysicalPage = page.size;
  const mappings = new Map();
  const pageTables = new Map();
  const allocatePage = () => {
    const address = nextPhysicalPage;
    assert(address + page.size <= guest.physicalPoolBytes, "physical pool exhausted");
    nextPhysicalPage += page.size;
    physicalMemory.fill(0, address, address + page.size);
    return address;
  };
  const pageDirectory = allocatePage();
  const ensurePageTable = (virtualBase) => {
    const directoryIndex = virtualBase >>> 22;
    if (!pageTables.has(directoryIndex)) {
      const table = allocatePage();
      pageTables.set(directoryIndex, table);
      physicalView.setUint32(
        pageDirectory + directoryIndex * 4,
        table | page.present | page.writable | page.user,
        true,
      );
    }
    return pageTables.get(directoryIndex);
  };
  for (const reflected of reflection.pages) {
    assert.equal(reflected.address & (page.size - 1), 0);
    const table = ensurePageTable(reflected.address);
    const pteAddress = table + ((reflected.address >>> 12) & 0x3ff) * 4;
    const physicalBase = allocatePage();
    physicalView.setUint32(
      pteAddress,
      physicalBase |
        page.present |
        page.user |
        (reflected.protection.write ? page.writable : 0),
      true,
    );
    physicalMemory.set(reflected.bytes, physicalBase);
    mappings.set(reflected.address, {
      address: reflected.address,
      owner: reflected.owner,
      protection: reflected.protection,
      physicalBase,
      pteAddress,
    });
  }

  wasm.reset_cpu();
  wasm.vine_cpu_state_save();
  const words = new Uint32Array(
    wasm.memory.buffer,
    wasm.vine_cpu_state_ptr(),
    wasm.vine_cpu_state_size() / 4,
  );
  assert.equal(words[stateWord.magic], 0x5649_4e45);
  assert.equal(words[stateWord.version], 1);
  assert.equal(words[stateWord.wordCount], 160);
  words.fill(0, 4, 12);
  words[stateWord.esp] = guest.stackBase + 2 * page.size;
  words[stateWord.instructionPointer] = guest.entryPoint;
  words[stateWord.previousInstructionPointer] = guest.entryPoint;
  words[stateWord.flags] = 2;
  words[stateWord.cpl] = 3;
  words[stateWord.protectedMode] = 1;
  words[stateWord.operandSize32] = 1;
  words[stateWord.stackSize32] = 1;
  words[stateWord.inHalt] = 0;
  for (let index = 0; index < 8; index += 1) {
    words[stateWord.segmentSelectorBase + index] = 0;
    words[stateWord.segmentOffsetBase + index] = 0;
    words[stateWord.segmentLimitBase + index] = 0xffff_ffff;
    words[stateWord.segmentMetaBase + index] = 0xf2;
  }
  words[stateWord.segmentMetaBase + 1] = 0xfa;
  words[stateWord.controlRegisterBase] |= 0x8001_0001;
  words[stateWord.controlRegisterBase + 3] = pageDirectory;
  words[stateWord.controlRegisterBase + 4] |= 0x200;
  assert.equal(wasm.vine_cpu_state_restore(), 0);
  return {
    adapterProfile,
    mappings,
    physicalMemory,
    physicalView,
    privateState,
    wasm,
  };
}

function applyInitialCacheEffects(context, cacheEffects) {
  assert(cacheEffects.flushTlb);
  assert(cacheEffects.invalidateInstructionFetch);
  assert(cacheEffects.invalidateGeneratedCode);
  context.wasm.full_clear_tlb();
  context.wasm.jit_clear_cache_js();
}

function executeGuest(context, expected) {
  const before = context.privateState.getUint32(adapterPrivateOffset.instructionCounter, true);
  const reason = context.wasm.vine_execute_budget(100);
  assert.equal(reason, stopReason.invalidOpcode);
  assert.equal(context.wasm.vine_get_stop_error_code(), -1);
  context.wasm.vine_cpu_state_save();
  const words = new Uint32Array(
    context.wasm.memory.buffer,
    context.wasm.vine_cpu_state_ptr(),
    context.wasm.vine_cpu_state_size() / 4,
  );
  const after = context.privateState.getUint32(adapterPrivateOffset.instructionCounter, true);
  assert.equal(words[stateWord.instructionPointer], expected.instructionPointer);
  assert.equal(words[stateWord.previousInstructionPointer], expected.instructionPointer);
  assert.equal(words[stateWord.eax], expected.eax);
  assert.equal(words[stateWord.ecx], expected.ecx);
  assert.equal(words[stateWord.esp], expected.esp);
  assert.equal(words[stateWord.edi], expected.edi);
  return {
    stopReason: reason,
    instructions: (after - before) >>> 0,
    instructionPointer: words[stateWord.instructionPointer],
    eax: words[stateWord.eax],
    ecx: words[stateWord.ecx],
    esp: words[stateWord.esp],
    edi: words[stateWord.edi],
  };
}

function prepareSecondSlice(context) {
  context.wasm.vine_cpu_state_save();
  const words = new Uint32Array(
    context.wasm.memory.buffer,
    context.wasm.vine_cpu_state_ptr(),
    context.wasm.vine_cpu_state_size() / 4,
  );
  words[stateWord.instructionPointer] = guest.entryPoint + 0x100;
  words[stateWord.previousInstructionPointer] = guest.entryPoint + 0x100;
  words[stateWord.inHalt] = 0;
  assert.equal(context.wasm.vine_cpu_state_restore(), 0);
}

function discoverDirtyPages(context, expectedDirtyAddresses) {
  const writable = [...context.mappings.values()]
    .filter(({ protection }) => protection.write)
    .sort((left, right) => left.address - right.address);
  const pages = [];
  const cleanWritablePages = [];
  for (const mapping of writable) {
    const pte = context.physicalView.getUint32(mapping.pteAddress, true);
    if (pte & page.dirty) {
      pages.push(mapping);
      context.physicalView.setUint32(mapping.pteAddress, pte & ~page.dirty, true);
    } else {
      cleanWritablePages.push(mapping);
    }
  }
  assert.deepEqual(
    pages.map(({ address }) => address),
    expectedDirtyAddresses,
  );
  assert.deepEqual(
    cleanWritablePages.map(({ address }) => address),
    writable
      .map(({ address }) => address)
      .filter((address) => !expectedDirtyAddresses.includes(address)),
  );
  for (const mapping of pages) {
    assert.equal(
      context.physicalView.getUint32(mapping.pteAddress, true) & page.dirty,
      0,
    );
  }
  context.wasm.full_clear_tlb();
  return {
    scannedWritablePages: writable.length,
    pages,
    cleanWritablePages,
    clearedDirtyBits: pages.length,
  };
}

function validateGuestBytes(context) {
  assert.deepEqual(readGuest(context, guest.stackBase + 0x0ffc, 4), [0xcc, 0xbb, 0xaa, 0x99]);
  assert.deepEqual(readGuest(context, guest.stackBase + 0x1000, 4), [0x88, 0x77, 0x66, 0x55]);
  const heap = readGuest(context, guest.heapBase + 0x0ff0, 32);
  for (let offset = 0; offset < heap.length; offset += 4) {
    assert.deepEqual(heap.slice(offset, offset + 4), [0x44, 0x33, 0x22, 0x11]);
  }
  assert.deepEqual(readGuest(context, guest.surfaceBase + 0x0ffc, 4), [0xdd, 0xcc, 0xbb, 0xaa]);
  assert.deepEqual(readGuest(context, guest.surfaceBase + 0x1000, 4), [0x44, 0x33, 0x22, 0x11]);
  assert.deepEqual(readGuest(context, guest.codeTargetBase, 1), [0xcc]);
  assert.deepEqual(readGuest(context, guest.scratchBase, 1), [0]);
}

function validateSecondGuestBytes(context) {
  assert.deepEqual(readGuest(context, guest.stackBase, 4), [0x04, 0x03, 0x02, 0x01]);
  assert.deepEqual(
    readGuest(context, guest.heapBase + 0x1020, 4),
    [0x08, 0x07, 0x06, 0x05],
  );
  assert.deepEqual(
    readGuest(context, guest.surfaceBase + 0x10, 4),
    [0x0c, 0x0b, 0x0a, 0x09],
  );
  assert.deepEqual(readGuest(context, guest.codeTargetBase, 1), [0xcd]);
  assert.deepEqual(
    readGuest(context, guest.scratchBase, 4),
    [0xee, 0xdd, 0xcc, 0xbb],
  );
}

function readGuest(context, address, byteLength) {
  const bytes = [];
  for (let offset = 0; offset < byteLength; offset += 1) {
    const current = address + offset;
    const virtualPage = (current & ~(page.size - 1)) >>> 0;
    const mapping = context.mappings.get(virtualPage);
    assert(mapping, `guest address 0x${current.toString(16)} is unmapped`);
    bytes.push(
      context.physicalMemory[mapping.physicalBase + (current & (page.size - 1))],
    );
  }
  return bytes;
}

function encodeWriteback(generation, dirtyPages, context) {
  const control = Buffer.alloc(
    writebackFormat.headerBytes + dirtyPages.length * writebackFormat.descriptorBytes,
  );
  control.write(writebackFormat.magic, 0, "ascii");
  const view = dataView(control);
  view.setUint32(8, writebackFormat.version, true);
  view.setUint32(12, page.size, true);
  view.setBigUint64(16, BigInt(generation), true);
  view.setUint32(24, dirtyPages.length, true);
  view.setUint32(28, writebackFormat.descriptorBytes, true);
  const payload = Buffer.alloc(dirtyPages.length * page.size);
  for (let index = 0; index < dirtyPages.length; index += 1) {
    const mapping = dirtyPages[index];
    const descriptor = writebackFormat.headerBytes + index * writebackFormat.descriptorBytes;
    view.setUint32(descriptor, mapping.address, true);
    view.setUint32(descriptor + 4, mapping.owner, true);
    Buffer.from(
      context.physicalMemory.buffer,
      context.physicalMemory.byteOffset + mapping.physicalBase,
      page.size,
    ).copy(payload, index * page.size);
  }
  return { control, payload };
}

function decodeReceipt(control) {
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
    changedBytes: safeNumber(view.getBigUint64(48, true), "changed bytes"),
    transferredBytes: safeNumber(view.getBigUint64(56, true), "transferred bytes"),
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

function validateReceipt(receipt, expected) {
  assert.equal(receipt.fromGeneration, expected.fromGeneration);
  assert.equal(receipt.generation, expected.fromGeneration + 1);
  assert.equal(receipt.touchedPages, expected.dirtyPages);
  assert.equal(receipt.changedPages, expected.dirtyPages);
  assert.equal(receipt.unchangedPages, 0);
  assert.equal(receipt.executablePages, 1);
  assert.equal(receipt.changedBytes, expected.changedBytes);
  assert.equal(receipt.transferredBytes, expected.dirtyPages * page.size);
  assert.match(receipt.contentSha256, /^[0-9a-f]{64}$/);
  assert.equal(receipt.rustGeneration, receipt.generation);
  assert.equal(receipt.rustAcknowledgedGeneration, receipt.generation);
  assert.equal(receipt.rustDirtyPages, 0);
  assert.equal(receipt.rustPendingInvalidation, false);
}

function sliceReport(execution, discovery, writeback, receipt) {
  return {
    execution,
    discovery: {
      scannedWritablePages: discovery.scannedWritablePages,
      dirtyPages: discovery.pages.map(({ address }) => ({
        address,
        role: pageRole(address),
      })),
      cleanWritablePages: discovery.cleanWritablePages.map(({ address }) => ({
        address,
        role: pageRole(address),
      })),
      clearedDirtyBits: discovery.clearedDirtyBits,
      tlbFlushAfterClear: true,
    },
    transport: {
      controlBytes: writeback.control.length,
      payloadBytes: writeback.payload.length,
      payloadSha256: sha256(writeback.payload),
      jsonPageByteArrays: false,
    },
    receipt,
  };
}

function pageRole(address) {
  if (address >= guest.stackBase && address < guest.stackBase + 2 * page.size) {
    return "stack";
  }
  if (address >= guest.heapBase && address < guest.heapBase + 2 * page.size) {
    return "heap";
  }
  if (address >= guest.surfaceBase && address < guest.surfaceBase + 2 * page.size) {
    return "surface";
  }
  if (address === guest.codeTargetBase) {
    return "executable-target";
  }
  if (address === guest.scratchBase) {
    return "untouched-scratch";
  }
  return "other";
}

function createReader(stream) {
  const iterator = stream[Symbol.asyncIterator]();
  let buffered = Buffer.alloc(0);
  return {
    async readExact(byteLength) {
      while (buffered.length < byteLength) {
        const next = await iterator.next();
        assert(!next.done, `binary stream ended with ${byteLength - buffered.length} bytes missing`);
        buffered = Buffer.concat([buffered, Buffer.from(next.value)]);
      }
      const result = buffered.subarray(0, byteLength);
      buffered = buffered.subarray(byteLength);
      return result;
    },
  };
}

async function readFrame(reader, expectedKind) {
  const header = await reader.readExact(frame.headerBytes);
  assert.equal(header.subarray(0, 8).toString("ascii"), frame.magic);
  const view = dataView(header);
  assert.equal(view.getUint32(8, true), frame.version);
  assert.equal(view.getUint32(12, true), expectedKind);
  const controlBytes = view.getUint32(16, true);
  const payloadBytes = view.getUint32(20, true);
  assert(controlBytes <= 1024 * 1024);
  assert(payloadBytes <= 64 * 1024 * 1024);
  return {
    control: await reader.readExact(controlBytes),
    payload: await reader.readExact(payloadBytes),
  };
}

async function writeFrame(stream, kind, control, payload) {
  const header = Buffer.alloc(frame.headerBytes);
  header.write(frame.magic, 0, "ascii");
  const view = dataView(header);
  view.setUint32(8, frame.version, true);
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

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function requireFile(file, command) {
  if (!fs.existsSync(file)) {
    throw new Error(`${path.relative(root, file)} is missing; run ${command} first`);
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack : error);
  process.exitCode = 1;
});
