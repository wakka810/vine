import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import readline from "node:readline";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

import { resolveV86AdapterProfile } from "./v86-adapter-profile.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const wasmPath = path.resolve(
  process.argv[2] ??
    path.join(root, ".work", "v86-adapter-source", "build", "v86-fallback.wasm"),
);
const outputDirectory = path.join(root, ".work", "v86-memory-delta");
const reportPath = path.join(outputDirectory, "report.json");

const page = { size: 0x1000, present: 1, writable: 2, user: 4 };
const guest = {
  physicalPoolBytes: 2 * 1024 * 1024,
  stackBase: 0x006f_f000,
  stackTop: 0x0070_0000,
};
const stateWord = {
  magic: 0,
  version: 1,
  wordCount: 2,
  eax: 4,
  edx: 6,
  esp: 8,
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
const stopReason = { budget: 1, invalidOpcode: 0x106 };

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function checkedEnd(start, size, limit, label) {
  assert(Number.isSafeInteger(start) && start >= 0, `${label} start is invalid`);
  assert(Number.isSafeInteger(size) && size >= 0, `${label} size is invalid`);
  const end = start + size;
  assert(Number.isSafeInteger(end) && end >= start && end <= limit, `${label} exceeds limit`);
  return end;
}

function requireFile(file, command) {
  if (!fs.existsSync(file)) {
    throw new Error(`${path.relative(root, file)} is missing; run ${command} first`);
  }
}

function writeAck(child, generation) {
  child.stdin.write(
    `${JSON.stringify({
      schema: "vine.memory-transfer-ack",
      schemaVersion: 1,
      generation,
    })}\n`,
  );
}

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
      "vine-memory-delta-session",
    ],
    { cwd: root, stdio: ["pipe", "pipe", "pipe"] },
  );
  let stderr = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  const lines = readline.createInterface({ input: child.stdout, crlfDelay: Infinity });
  const iterator = lines[Symbol.asyncIterator]();
  try {
    const initial = await readMessage(iterator, "initial reflection");
    validateInitial(initial);
    const context = await prepareV86(initial.reflection);
    applyInitialCacheEffects(context);
    writeAck(child, initial.reflection.invalidation.generation);

    const update = await readMessage(iterator, "memory delta");
    validateDelta(update, initial);
    applyDelta(context, update.delta);
    writeAck(child, update.delta.generation);

    const receipt = await readMessage(iterator, "memory delta receipt");
    validateReceipt(receipt, update);
    child.stdin.end();
    const execution = executeGuest(context, initial);
    const executeGate = validateCrossPageExecuteGate(context, initial);
    const exitCode = await new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("close", resolve);
    });
    assert.equal(exitCode, 0, stderr || "memory delta session failed");

    const report = {
      schema: "vine.v86-memory-delta-roundtrip",
      schemaVersion: 1,
      initialGeneration: initial.reflection.invalidation.generation,
      delta: {
        fromGeneration: update.delta.fromGeneration,
        generation: update.delta.generation,
        dirtyPages: update.delta.dirtyPages.length,
        mappingOperations: update.delta.mappingOperations.length,
        transferredBytes: update.delta.transferredBytes,
        cacheEffects: update.delta.cacheEffects,
        contentSha256: update.delta.contentSha256,
      },
      execution,
      executeGate,
      receipt,
    };
    fs.mkdirSync(outputDirectory, { recursive: true });
    const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
    fs.writeFileSync(reportPath, reportBytes);
    console.log(
      `v86 memory delta passed: generation ${update.delta.generation}, ` +
        `${update.delta.transferredBytes} bytes, EDX:EAX ` +
        `0x${execution.edx.toString(16).padStart(8, "0")}` +
        `${execution.eax.toString(16).padStart(8, "0")}`,
    );
    console.log(
      `report: ${path.relative(root, reportPath)}, ${reportBytes.byteLength} bytes, ` +
        `sha256 ${sha256(reportBytes)}`,
    );
  } finally {
    lines.close();
    if (child.exitCode === null) {
      child.kill();
    }
  }
}

async function readMessage(iterator, label) {
  const next = await iterator.next();
  assert(!next.done, `Rust session ended before ${label}`);
  return JSON.parse(next.value);
}

function validateInitial(initial) {
  assert.equal(initial.schema, "vine.memory-delta-initial");
  assert.equal(initial.schemaVersion, 1);
  assert.equal(initial.entryPoint, 0x1000_0000);
  assert.equal(initial.dataAddress, 0x2000_0000);
  assert.equal(initial.expectedLow, 0x4433_2211);
  assert.equal(initial.expectedHigh, 0x8877_6655);
  assert.equal(initial.crossPageInstruction, 0x3000_0ffe);
  assert.equal(initial.reflection.pageSize, page.size);
  assert.equal(initial.reflection.pages.length, 4);
  assert.deepEqual(
    initial.reflection.pages.map(({ address, protection }) => [address, protection]),
    [
      [0x1000_0000, { read: true, write: false, execute: true }],
      [0x2000_0000, { read: true, write: true, execute: false }],
      [0x3000_0000, { read: true, write: false, execute: true }],
      [0x3000_1000, { read: true, write: false, execute: false }],
    ],
  );
  for (const reflected of initial.reflection.pages) {
    assert.equal(reflected.ownerSealed, true);
    assert.equal(reflected.dirty, true);
    assert.equal(reflected.bytes.length, page.size);
  }
}

function validateDelta(update, initial) {
  assert.equal(update.schema, "vine.memory-delta-update");
  assert.equal(update.schemaVersion, 1);
  const delta = update.delta;
  assert.equal(delta.fromGeneration, initial.reflection.invalidation.generation);
  assert.equal(delta.generation, delta.fromGeneration + 1);
  assert.equal(delta.pageSize, page.size);
  assert.deepEqual(delta.mappingOperations, []);
  assert.equal(delta.dirtyPages.length, 1);
  assert.equal(delta.dirtyPages[0].address, initial.dataAddress);
  assert.equal(delta.dirtyPages[0].bytes.length, page.size);
  assert.deepEqual(delta.dirtyPages[0].bytes.slice(0, 8), [
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
  ]);
  assert.deepEqual(delta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });
  assert.equal(delta.transferredBytes, page.size);
  assert.match(delta.contentSha256, /^[0-9a-f]{64}$/);
}

function validateReceipt(receipt, update) {
  assert.equal(receipt.schema, "vine.memory-delta-receipt");
  assert.equal(receipt.schemaVersion, 1);
  assert.equal(receipt.acknowledgedGeneration, update.delta.generation);
  assert.deepEqual(receipt.valueBytes, [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
  assert.equal(receipt.stats.generation, update.delta.generation);
  assert.equal(receipt.stats.acknowledgedGeneration, update.delta.generation);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
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
  assert.equal(wasm.vine_adapter_abi_version(), 1);
  assert.equal(wasm.vine_set_memory_size(guest.physicalPoolBytes), 0);
  const guestMemoryBase = wasm.allocate_memory(guest.physicalPoolBytes);
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

  let nextPhysicalPage = page.size;
  const mappings = new Map();
  const pageTables = new Map();
  const allocatePage = () => {
    const address = nextPhysicalPage;
    checkedEnd(address, page.size, guest.physicalPoolBytes, "physical page");
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
  const mapPage = (virtualBase, protection, bytes) => {
    assert.equal(virtualBase & (page.size - 1), 0);
    assert.equal(bytes.length, page.size);
    assert(!mappings.has(virtualBase));
    const table = ensurePageTable(virtualBase);
    const pteAddress = table + ((virtualBase >>> 12) & 0x3ff) * 4;
    const physicalBase = allocatePage();
    physicalView.setUint32(
      pteAddress,
      physicalBase |
        page.present |
        page.user |
        (protection.write ? page.writable : 0),
      true,
    );
    physicalMemory.set(bytes, physicalBase);
    mappings.set(virtualBase, { physicalBase, protection: { ...protection }, pteAddress });
  };
  for (const reflected of reflection.pages) {
    mapPage(reflected.address, reflected.protection, Uint8Array.from(reflected.bytes));
  }
  mapPage(
    guest.stackBase,
    { read: true, write: true, execute: false },
    new Uint8Array(page.size),
  );

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
  words[stateWord.esp] = guest.stackTop;
  words[stateWord.instructionPointer] = 0x1000_0000;
  words[stateWord.previousInstructionPointer] = 0x1000_0000;
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
  return { mapPage, mappings, physicalMemory, physicalView, wasm };
}

function applyInitialCacheEffects(context) {
  context.wasm.full_clear_tlb();
  context.wasm.jit_clear_cache_js();
}

function applyDelta(context, delta) {
  const dirtyByAddress = new Map(delta.dirtyPages.map((dirty) => [dirty.address, dirty]));
  for (const operation of delta.mappingOperations) {
    const start = operation.range.start;
    const end = start + operation.range.byteLength;
    if (operation.operation === "reserve") {
      continue;
    }
    if (operation.operation === "commit") {
      for (let address = start; address < end; address += page.size) {
        const dirty = dirtyByAddress.get(address);
        assert(dirty, "committed delta page has no bytes");
        context.mapPage(address, operation.protection, Uint8Array.from(dirty.bytes));
        dirtyByAddress.delete(address);
      }
      continue;
    }
    if (operation.operation === "protect") {
      for (let address = start; address < end; address += page.size) {
        const mapping = context.mappings.get(address);
        assert(mapping, "protected delta page is not mapped");
        mapping.protection = { ...operation.protection };
        context.physicalView.setUint32(
          mapping.pteAddress,
          mapping.physicalBase |
            page.present |
            page.user |
            (operation.protection.write ? page.writable : 0),
          true,
        );
      }
      continue;
    }
    assert(operation.operation === "decommit" || operation.operation === "release");
    for (let address = start; address < end; address += page.size) {
      const mapping = context.mappings.get(address);
      if (mapping) {
        context.physicalView.setUint32(mapping.pteAddress, 0, true);
        context.mappings.delete(address);
      }
    }
  }
  for (const dirty of dirtyByAddress.values()) {
    const mapping = context.mappings.get(dirty.address);
    assert(mapping, `dirty page 0x${dirty.address.toString(16)} is not mapped`);
    context.physicalMemory.set(Uint8Array.from(dirty.bytes), mapping.physicalBase);
  }
  if (delta.cacheEffects.flushTlb || delta.cacheEffects.invalidateInstructionFetch) {
    context.wasm.full_clear_tlb();
  }
  if (delta.cacheEffects.invalidateGeneratedCode) {
    context.wasm.jit_clear_cache_js();
  }
}

function executeGuest(context, initial) {
  let budgetStops = 0;
  let reason = 0;
  for (let attempt = 0; attempt < 8; attempt += 1) {
    reason = context.wasm.vine_execute_budget(3);
    if (reason === stopReason.budget) {
      budgetStops += 1;
      continue;
    }
    break;
  }
  assert.equal(reason, stopReason.invalidOpcode);
  assert.equal(context.wasm.vine_get_stop_error_code(), -1);
  context.wasm.vine_cpu_state_save();
  const words = new Uint32Array(
    context.wasm.memory.buffer,
    context.wasm.vine_cpu_state_ptr(),
    context.wasm.vine_cpu_state_size() / 4,
  );
  assert.equal(words[stateWord.instructionPointer], initial.entryPoint + 11);
  assert.equal(words[stateWord.previousInstructionPointer], initial.entryPoint + 11);
  assert.equal(words[stateWord.eax], initial.expectedLow);
  assert.equal(words[stateWord.edx], initial.expectedHigh);
  return {
    stopReason: reason,
    budgetStops,
    instructionPointer: words[stateWord.instructionPointer],
    eax: words[stateWord.eax],
    edx: words[stateWord.edx],
  };
}

function validateCrossPageExecuteGate(context, initial) {
  const maximumInstructionBytes = 15;
  const start = initial.crossPageInstruction;
  const currentPage = (start & ~(page.size - 1)) >>> 0;
  const current = context.mappings.get(currentPage);
  assert(current?.protection.execute, "cross-page fixture must start in executable memory");
  for (let offset = 0; offset < maximumInstructionBytes; offset += 1) {
    const address = (start + offset) >>> 0;
    const virtualPage = (address & ~(page.size - 1)) >>> 0;
    const mapping = context.mappings.get(virtualPage);
    if (!mapping?.protection.execute) {
      const stop = {
        stage: "instruction-fetch",
        code: "cross-page-execute-permission-unimplemented",
        instructionPointer: start,
        deniedPage: virtualPage,
        checkedBytes: maximumInstructionBytes,
        executed: false,
      };
      assert.equal(stop.deniedPage, 0x3000_1000);
      return stop;
    }
  }
  assert.fail("cross-page instruction incorrectly passed the conservative execute gate");
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack : error);
  process.exitCode = 1;
});
