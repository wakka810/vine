import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import readline from "node:readline";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const wasmPath = path.resolve(
  process.argv[2] ??
    path.join(root, ".work", "v86-adapter-source", "build", "v86-fallback.wasm"),
);
const fixturePath = path.resolve(
  process.argv[3] ?? path.join(root, ".work", "toolchain-spike", "abi-smoke.exe"),
);
const outputDirectory = path.join(root, ".work", "v86-entry-ingress");
const reportPath = path.join(outputDirectory, "report.json");

const expected = {
  source: {
    bytes: 2_560,
    sha256: "60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7",
  },
  wasm: {
    bytes: 1_882_836,
    sha256: "f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26",
    imports: 22,
    exports: 809,
  },
  imageLoadBase: 0x10000000,
  entry: 0x10001000,
  firstThunk: {
    id: 1,
    address: 0x7f000000,
    iatAddress: 0x10002034,
    returnAddress: 0x1000100a,
    arguments: [19, 23],
  },
  reflectionContentSha256:
    "3a3510468616980e99ccf36ccba29083acd8edffb01f1bbfdfd60e3b4a1d297e",
  report: {
    bytes: 4_831,
    sha256: "9b32ed50a4fa53d60a2743267230d43acf754bb6bc490851dd754955e70cbcb8",
  },
};

const page = {
  size: 0x1000,
  present: 1,
  writable: 2,
  user: 4,
};
const guest = {
  physicalPoolBytes: 2 * 1024 * 1024,
  stackBase: 0x006ff000,
  stackTop: 0x00700000,
  entryReturn: 0xdeadc0de,
  instructionBudget: 3,
};
const adapterPrivateOffset = {
  memorySize: 812,
};
const stateWord = {
  magic: 0,
  version: 1,
  wordCount: 2,
  gprBase: 4,
  eax: 4,
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
const stopReason = {
  budget: 1,
  invalidOpcode: 0x106,
};
const calleeSaved = {
  ebx: 0x11223344,
  ebp: 0x55667788,
  esi: 0x99aabbcc,
  edi: 0xddeeff00,
};

function requireFile(file, command) {
  if (!fs.existsSync(file)) {
    throw new Error(`${path.relative(root, file)} is missing; run ${command} first`);
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

function checkedGuestEnd(address, size, label) {
  return checkedEnd(address, size, 0x1_0000_0000, label);
}

function expectedReflectedPages() {
  return [
    [0x10000000, 1, { read: true, write: false, execute: false }],
    [0x10001000, 1, { read: true, write: false, execute: true }],
    [0x10002000, 1, { read: true, write: false, execute: false }],
    [0x10003000, 1, { read: true, write: true, execute: false }],
    [0x10004000, 1, { read: true, write: false, execute: false }],
    [0x7f000000, 2, { read: true, write: false, execute: true }],
  ];
}

function reflectionContentSha256(pages) {
  const hasher = crypto.createHash("sha256");
  for (const reflected of pages) {
    const metadata = Buffer.alloc(11);
    metadata.writeUInt32LE(reflected.address, 0);
    metadata.writeUInt32LE(reflected.owner, 4);
    const protectionBits =
      Number(reflected.protection.read) |
      (Number(reflected.protection.write) << 1) |
      (Number(reflected.protection.execute) << 2);
    metadata[8] = protectionBits;
    metadata[9] = Number(reflected.ownerSealed);
    metadata[10] = Number(reflected.dirty);
    hasher.update(metadata);
    hasher.update(Uint8Array.from(reflected.bytes));
  }
  return hasher.digest("hex");
}

async function run() {
  requireFile(wasmPath, "npm run prepare:engine");
  requireFile(fixturePath, "npm run check:toolchain");
  const fixtureBytes = fs.readFileSync(fixturePath);
  assert.equal(fixtureBytes.byteLength, expected.source.bytes);
  assert.equal(sha256(fixtureBytes), expected.source.sha256);

  const child = spawn(
    "cargo",
    [
      "run",
      "--quiet",
      "--locked",
      "-p",
      "vine-pe-probe",
      "--",
      "--v86-reflection-session",
      "--expect-sha256",
      expected.source.sha256,
      "--prepare-memory-pages",
      "6",
      "--materialize-image-base",
      "0x10000000",
      "--bind-name",
      "vine_fixture.dll",
      "fixture_add",
      "--bind-name",
      "vine_fixture.dll",
      "fixture_call_callback",
      fixturePath,
    ],
    {
      cwd: root,
      windowsHide: true,
      stdio: ["pipe", "pipe", "pipe"],
    },
  );
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
    assert(next.value.length <= 512 * 1024, `${label} exceeds the 512 KiB protocol limit`);
    return JSON.parse(next.value);
  };

  try {
    const offer = await nextMessage("reflection offer");
    validateOffer(offer);

    const wasmContext = await prepareV86(offer);
    const invalidationCalls = [
      "vine_cpu_state_restore/full_clear_tlb",
      "full_clear_tlb",
      "jit_clear_cache_js",
    ];
    wasmContext.wasm.full_clear_tlb();
    wasmContext.wasm.jit_clear_cache_js();

    child.stdin.end(
      `${JSON.stringify({
        schema: "vine.v86-memory-reflection-ack",
        schemaVersion: 1,
        generation: offer.reflection.invalidation.generation,
        dataTlb: true,
        instructionFetch: true,
        generatedCode: true,
      })}\n`,
    );

    const receipt = await nextMessage("reflection receipt");
    validateReceipt(receipt, offer, wasmContext);
    const processExit = await exit;
    assert.equal(processExit.signal, null, `reflection probe terminated by ${processExit.signal}`);
    assert.equal(processExit.code, 0, `reflection probe failed: ${stderr.trim()}`);
    sessionCompleted = true;

    const execution = executeToFirstThunk(offer, wasmContext);
    const report = {
      schema: "vine.v86-entry-ingress",
      schemaVersion: 1,
      source: offer.source,
      adapter: {
        wasm: {
          bytes: expected.wasm.bytes,
          sha256: expected.wasm.sha256,
          imports: expected.wasm.imports,
          exports: expected.wasm.exports,
        },
        physicalPoolBytes: guest.physicalPoolBytes,
        pageDirectory: wasmContext.pageDirectory,
        pageTables: [...wasmContext.pageTables.entries()].map(
          ([directoryIndex, physicalAddress]) => ({ directoryIndex, physicalAddress }),
        ),
        allocatedPages: wasmContext.allocations,
      },
      reflection: {
        invalidation: offer.reflection.invalidation,
        contentSha256: offer.reflection.contentSha256,
        pages: offer.reflection.pages.map((reflected) => ({
          address: reflected.address,
          owner: reflected.owner,
          ownerSealed: reflected.ownerSealed,
          protection: reflected.protection,
          dirty: reflected.dirty,
          contentSha256: sha256(Uint8Array.from(reflected.bytes)),
        })),
        acknowledgement: {
          generation: receipt.acknowledgedGeneration,
          callsBeforeAck: invalidationCalls,
          pendingAfterAck: receipt.stats.pendingInvalidation ?? null,
        },
      },
      bootstrap: {
        kind: "harness-only-one-page-flat-cpl3",
        stackBase: guest.stackBase,
        stackTop: guest.stackTop,
        initialEsp: guest.stackTop - 4,
        entryReturn: guest.entryReturn,
        entryPoint: offer.entryPointAddress,
      },
      execution,
      stop: {
        stage: "thunk-dispatch",
        code: "owned-import-service-unimplemented",
        message:
          "the controlled entry reached exact runtime-owned thunk ID 1; host service dispatch and guest resume were not attempted",
      },
    };
    fs.mkdirSync(outputDirectory, { recursive: true });
    const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
    assert.equal(reportBytes.byteLength, expected.report.bytes);
    assert.equal(sha256(reportBytes), expected.report.sha256);
    fs.writeFileSync(reportPath, reportBytes);
    console.log(
      `v86 entry ingress passed: generation ${receipt.acknowledgedGeneration}, ` +
        `${execution.budgetStops} budget stops, thunk ID ${execution.thunkId} at ` +
        `0x${execution.instructionPointer.toString(16)}`,
    );
    console.log(
      `report: ${path.relative(root, reportPath)}, ${reportBytes.byteLength} bytes, ` +
        `sha256 ${sha256(reportBytes)}`,
    );
  } finally {
    lineReader.close();
    if (!sessionCompleted && child.exitCode === null) {
      child.kill();
    }
  }
}

function validateOffer(offer) {
  assert.equal(offer.schema, "vine.v86-memory-reflection");
  assert.equal(offer.schemaVersion, 1);
  assert.equal(offer.source.bytes, expected.source.bytes);
  assert.equal(offer.source.sha256, expected.source.sha256);
  assert.equal(offer.imageLoadBase, expected.imageLoadBase);
  assert.equal(offer.entryPointAddress, expected.entry);
  assert.equal(offer.imageOwner, 1);
  assert.equal(offer.firstThunk.id, expected.firstThunk.id);
  assert.equal(offer.firstThunk.address, expected.firstThunk.address);
  assert.equal(offer.firstThunk.iatAddress, expected.firstThunk.iatAddress);
  assert.equal(offer.firstThunk.module, "vine_fixture.dll");
  assert.equal(offer.firstThunk.kind, "name");
  assert.equal(offer.firstThunk.name, "fixture_add");
  assert.deepEqual(offer.reflection.invalidation, {
    generation: 4,
    dataTlb: true,
    instructionFetch: true,
    generatedCode: true,
  });
  assert.equal(offer.reflection.pageSize, page.size);
  assert.equal(offer.reflection.pages.length, 6);
  assert.deepEqual(
    offer.reflection.pages.map(({ address, owner, protection }) => [
      address,
      owner,
      protection,
    ]),
    expectedReflectedPages(),
  );
  for (const reflected of offer.reflection.pages) {
    assert.equal(reflected.ownerSealed, true);
    assert.equal(reflected.dirty, true);
    assert.equal(reflected.bytes.length, page.size);
    for (const byte of reflected.bytes) {
      assert(Number.isInteger(byte) && byte >= 0 && byte <= 0xff);
    }
  }
  assert.equal(
    reflectionContentSha256(offer.reflection.pages),
    offer.reflection.contentSha256,
  );
  assert.equal(offer.reflection.contentSha256, expected.reflectionContentSha256);
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
      assert.equal(imported.name, "microtick", `unexpected host import ${imported.name}`);
      assert.equal(args.length, 0);
      return 0;
    };
  }

  const { instance } = await WebAssembly.instantiate(wasmBytes, { env: environment });
  const wasm = instance.exports;
  wasm.rust_init();
  let privateState = new DataView(wasm.memory.buffer);
  privateState.setUint32(
    adapterPrivateOffset.memorySize,
    guest.physicalPoolBytes,
    true,
  );
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
  const mapPage = (virtualBase, protection, bytes, role) => {
    checkedGuestEnd(virtualBase, page.size, role);
    assert.equal(virtualBase & (page.size - 1), 0);
    assert.equal(bytes.byteLength, page.size);
    assert(!mappings.has(virtualBase), `duplicate mapping 0x${virtualBase.toString(16)}`);
    const table = ensurePageTable(virtualBase);
    const tableIndex = (virtualBase >>> 12) & 0x3ff;
    const pteAddress = table + tableIndex * 4;
    const physicalBase = allocatePhysicalPage(role);
    writePhysicalU32(
      pteAddress,
      physicalBase |
        page.present |
        page.user |
        (protection.write ? page.writable : 0),
      `${role} page table entry`,
    );
    physicalMemory.set(bytes, physicalBase);
    mappings.set(virtualBase, {
      virtualBase,
      physicalBase,
      pteAddress,
      protection: { ...protection },
      role,
    });
  };

  for (const reflected of offer.reflection.pages) {
    mapPage(
      reflected.address,
      reflected.protection,
      Uint8Array.from(reflected.bytes),
      `reflected-owner-${reflected.owner}`,
    );
  }
  mapPage(
    guest.stackBase,
    { read: true, write: true, execute: false },
    new Uint8Array(page.size),
    "harness-stack",
  );

  const mappingFor = (address, label) => {
    checkedGuestEnd(address, 1, label);
    const virtualBase = (address & ~(page.size - 1)) >>> 0;
    const mapping = mappings.get(virtualBase);
    assert(mapping, `${label} uses unmapped guest address 0x${address.toString(16)}`);
    return mapping;
  };
  const writeVirtualU32 = (address, value, label) => {
    checkedGuestEnd(address, 4, label);
    for (let index = 0; index < 4; index += 1) {
      const current = (address + index) >>> 0;
      const mapping = mappingFor(current, label);
      physicalMemory[mapping.physicalBase + (current & (page.size - 1))] =
        value >>> (index * 8);
    }
  };
  const readVirtualBytes = (address, size, label) => {
    checkedGuestEnd(address, size, label);
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

  const initialEsp = guest.stackTop - 4;
  writeVirtualU32(initialEsp, guest.entryReturn, "entry return sentinel");
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
  words.fill(0, stateWord.gprBase, stateWord.gprBase + 8);
  words[stateWord.ebx] = calleeSaved.ebx;
  words[stateWord.esp] = initialEsp;
  words[stateWord.ebp] = calleeSaved.ebp;
  words[stateWord.esi] = calleeSaved.esi;
  words[stateWord.edi] = calleeSaved.edi;
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
  words[stateWord.segmentMetaBase + 1] = 0xfa;
  words[stateWord.controlRegisterBase] |= 0x80010001;
  words[stateWord.controlRegisterBase + 3] = pageDirectory;
  words[stateWord.controlRegisterBase + 4] |= 0x200;
  assert.equal(wasm.vine_cpu_state_restore(), 0);

  return {
    allocations,
    hostImportCalls,
    mappings,
    pageDirectory,
    pageTables,
    physicalMemory,
    readVirtualBytes,
    readVirtualU32,
    wasm,
  };
}

function validateReceipt(receipt, offer, context) {
  assert.equal(receipt.schema, "vine.v86-memory-reflection-receipt");
  assert.equal(receipt.schemaVersion, 1);
  assert.equal(
    receipt.acknowledgedGeneration,
    offer.reflection.invalidation.generation,
  );
  assert.equal(receipt.stats.generation, 4);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  assert.equal(receipt.iatValue, expected.firstThunk.address);
  assert.deepEqual(
    receipt.entryBytes,
    [...context.readVirtualBytes(expected.entry, 16, "reflected entry bytes")],
  );
  assert.deepEqual(
    receipt.thunkBytes,
    [...context.readVirtualBytes(expected.firstThunk.address, 16, "reflected thunk bytes")],
  );
  assert.deepEqual(receipt.thunkBytes, [
    0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
    0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
  ]);
  assert.equal(receipt.stop.stage, "adapter-execution");
  assert.equal(receipt.stop.code, "entry-to-first-owned-thunk-pending");
}

function executeToFirstThunk(offer, context) {
  let budgetStops = 0;
  let executeStops = 0;
  let finalState = null;
  const executablePages = new Set(
    offer.reflection.pages
      .filter(({ ownerSealed, protection }) => ownerSealed && protection.execute)
      .map(({ address }) => address),
  );
  const saveState = (reason) => {
    context.wasm.vine_cpu_state_save();
    const words = Uint32Array.from(
      new Uint32Array(
        context.wasm.memory.buffer,
        context.wasm.vine_cpu_state_ptr(),
        context.wasm.vine_cpu_state_size() / 4,
      ),
    );
    return {
      reason,
      errorCode: context.wasm.vine_get_stop_error_code(),
      words,
      eax: words[stateWord.eax],
      esp: words[stateWord.esp],
      instructionPointer: words[stateWord.instructionPointer],
      previousInstructionPointer: words[stateWord.previousInstructionPointer],
    };
  };

  for (let index = 0; index < 16 && finalState === null; index += 1) {
    const before = saveState(0);
    const currentPage = (before.instructionPointer & ~(page.size - 1)) >>> 0;
    assert(
      executablePages.has(currentPage),
      `execute gate rejected EIP 0x${before.instructionPointer.toString(16)}`,
    );
    const reason = context.wasm.vine_execute_budget(guest.instructionBudget);
    executeStops += 1;
    const state = saveState(reason);
    assert.equal(state.words[stateWord.ebx], calleeSaved.ebx);
    assert.equal(state.words[stateWord.ebp], calleeSaved.ebp);
    assert.equal(state.words[stateWord.esi], calleeSaved.esi);
    assert.equal(state.words[stateWord.edi], calleeSaved.edi);
    if (reason === stopReason.budget) {
      budgetStops += 1;
      continue;
    }
    assert.equal(reason, stopReason.invalidOpcode);
    assert.equal(state.errorCode, -1);
    assert.equal(state.instructionPointer, offer.firstThunk.address);
    assert.equal(state.previousInstructionPointer, offer.firstThunk.address);
    finalState = state;
  }
  assert(finalState, "controlled entry did not reach an owned thunk within 16 slices");
  assert.equal(finalState.esp, guest.stackTop - 16);
  const returnAddress = context.readVirtualU32(finalState.esp, "fixture_add return address");
  const left = context.readVirtualU32(finalState.esp + 4, "fixture_add left argument");
  const right = context.readVirtualU32(finalState.esp + 8, "fixture_add right argument");
  assert.equal(returnAddress, expected.firstThunk.returnAddress);
  assert.deepEqual([left, right], expected.firstThunk.arguments);
  assert.equal(
    context.readVirtualU32(expected.firstThunk.iatAddress, "bound fixture_add IAT"),
    expected.firstThunk.address,
  );
  assert.deepEqual(context.hostImportCalls, { microtick: 1 });

  return {
    instructionBudget: guest.instructionBudget,
    executeStops,
    budgetStops,
    stopReason: finalState.reason,
    stopErrorCode: finalState.errorCode,
    thunkId: offer.firstThunk.id,
    instructionPointer: finalState.instructionPointer,
    previousInstructionPointer: finalState.previousInstructionPointer,
    esp: finalState.esp,
    returnAddress,
    arguments: [left, right],
    service: "vine_fixture.dll!fixture_add",
  };
}

await run();
