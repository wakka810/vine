import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import readline from "node:readline";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const commandArguments = process.argv.slice(2);
assert(
  commandArguments.length <= 2,
  "usage: run-runtime-v86-bgi-first-trap.mjs [WASM] [BGI.exe]",
);
const wasmPath = path.resolve(
  commandArguments[0] ??
    path.join(root, ".work", "v86-adapter-source", "build", "v86-fallback.wasm"),
);
const bgiPath = path.resolve(
  commandArguments[1] ??
    "V:\\softs\\\u30b5\u30af\u30e9\u30ce\u8a69\\BGI.exe",
);
const outputDirectory = path.join(root, ".work", "v86-bgi-first-trap");
const reportPath = path.join(outputDirectory, "report.json");

const expected = {
  source: {
    bytes: 1_193_472,
    sha256: "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1",
  },
  wasm: {
    bytes: 1_882_836,
    sha256: "f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26",
    imports: 22,
    exports: 809,
  },
  image: {
    owner: 1,
    loadBase: 0x10000000,
    entry: 0x100a383e,
    pages: 380,
    readablePages: 380,
    writablePages: 100,
    executablePages: 208,
  },
  imports: {
    owner: 2,
    pages: 2,
    total: 330,
    declared: 1,
    unsupported: 329,
    first: {
      id: 1,
      address: 0x7f000000,
      iatAddress: 0x100d14f4,
      module: "d3d9.dll",
      name: "Direct3DCreate9",
    },
    second: {
      id: 2,
      address: 0x7f000010,
      iatAddress: 0x100d1050,
      module: "DINPUT8.dll",
      name: "DirectInput8Create",
    },
    last: {
      id: 330,
      address: 0x7f001490,
      iatAddress: 0x100d1058,
      module: "DSOUND.dll",
      ordinal: 11,
    },
  },
  control: {
    owner: 3,
    page: 0x7f010000,
    callbackId: 1,
    callbackAddress: 0x7f010000,
    processEntryId: 2,
    processEntryAddress: 0x7f010010,
  },
  stack: {
    owner: 4,
    base: 0x006ff000,
    top: 0x00700000,
    initialEsp: 0x006ffffc,
  },
  processEnvironment: {
    tebOwner: 5,
    pebOwner: 6,
    processParametersOwner: 7,
    commandLineOwner: 8,
    tebBase: 0x7ffde000,
    pebBase: 0x7ffdf000,
    processParametersBase: 0x7ffdc000,
    commandLineBase: 0x7ffdd000,
    commandLine: "BGI.exe",
    commandLineLengthBytes: 14,
    commandLineMaximumLengthBytes: 16,
    tebContentSha256: "5d4ad643f294d6976304f87d2912ec4a0b1f774fae1fd9a94de59749bfdcd67e",
    pebContentSha256: "780207feb049a4c658816bf0c986bb386ec04214722a21c47936cdec01ee4d39",
    processParametersContentSha256:
      "ac17565d250da1d27dc2429b2c81bd242fe26d67373495b2238eadbb762b6d33",
    commandLineContentSha256:
      "c9b83dbe193708a3418517c43a4dadd26ca50a4006b00bd11701f3bb6ef0e6cc",
  },
  memory: {
    owners: 8,
    reservedPages: 417,
    reflectedPages: 388,
    generation: 16,
  },
  reflectionContentSha256:
    "f68f79bb13b3c5a11a2d145a8ee772a0bfd5c04dfec7502f50065cc748bc3cb0",
  offerLineBytes: 4_904_601,
  receiptLineBytes: 1_468,
  terminal: {
    thunk: {
      id: 104,
      address: 0x7f000670,
      iatAddress: 0x100d1248,
      module: "KERNEL32.dll",
      name: "GetSystemTimeAsFileTime",
    },
    instructionCount: 19,
    executeStops: 19,
    budgetStops: 18,
    returnAddress: 0x100a88d2,
    stackWords: [0x100a88d2, 0x006fffec, 0, 0, 0, 0, 0, 0],
  },
  report: {
    bytes: 9_591,
    sha256: "a74ed888aa95923a1bcb5c20ad5a58e945ab269419d834a19eefa10d6b33be3b",
  },
};

const page = {
  size: 0x1000,
  present: 1,
  writable: 2,
  user: 4,
};
const protocol = {
  maximumLineBytes: 8 * 1024 * 1024,
  maximumReflectionPages: 400,
};
const guest = {
  physicalPoolBytes: 2 * 1024 * 1024,
  excludedPhysicalRanges: [{ start: 0x000a0000, end: 0x000c0000, role: "vga-mmio" }],
  instructionBudget: 1,
  maximumExecuteStops: 4_096,
};
const adapterPrivateOffset = {
  instructionCounter: 664,
  memorySize: 812,
};
const stateWord = {
  magic: 0,
  version: 1,
  wordCount: 2,
  eax: 4,
  ecx: 5,
  edx: 6,
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
const segmentIndex = {
  cs: 1,
  fs: 4,
};
const stopReason = {
  budget: 1,
  invalidOpcode: 0x106,
  pageFault: 0x10e,
};

function requireFile(file, command) {
  if (!fs.existsSync(file)) {
    throw new Error(`${file} is missing; run ${command} first`);
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

function reflectionContentSha256(pages) {
  const hasher = crypto.createHash("sha256");
  for (const reflected of pages) {
    const metadata = Buffer.alloc(11);
    metadata.writeUInt32LE(reflected.address, 0);
    metadata.writeUInt32LE(reflected.owner, 4);
    metadata[8] =
      Number(reflected.protection.read) |
      (Number(reflected.protection.write) << 1) |
      (Number(reflected.protection.execute) << 2);
    metadata[9] = Number(reflected.ownerSealed);
    metadata[10] = Number(reflected.dirty);
    hasher.update(metadata);
    hasher.update(Uint8Array.from(reflected.bytes));
  }
  return hasher.digest("hex");
}

function summarizeOwners(pages) {
  const owners = new Map();
  for (const reflected of pages) {
    let summary = owners.get(reflected.owner);
    if (summary === undefined) {
      summary = {
        owner: reflected.owner,
        pages: 0,
        firstAddress: reflected.address,
        lastAddress: reflected.address,
        readablePages: 0,
        writablePages: 0,
        executablePages: 0,
        hasher: crypto.createHash("sha256"),
      };
      owners.set(reflected.owner, summary);
    }
    summary.pages += 1;
    summary.lastAddress = reflected.address;
    summary.readablePages += Number(reflected.protection.read);
    summary.writablePages += Number(reflected.protection.write);
    summary.executablePages += Number(reflected.protection.execute);
    summary.hasher.update(Uint8Array.from(reflected.bytes));
  }
  return [...owners.values()].map(({ hasher, ...summary }) => ({
    ...summary,
    contentSha256: hasher.digest("hex"),
  }));
}

function sourceIdentity(file) {
  const bytes = fs.readFileSync(file);
  return { bytes: bytes.byteLength, sha256: sha256(bytes) };
}

async function run() {
  requireFile(wasmPath, "npm run prepare:engine");
  requireFile(bgiPath, "the primary title must be installed at its configured path");
  const sourceBefore = sourceIdentity(bgiPath);
  assert.deepEqual(sourceBefore, expected.source);

  const probeArguments = [
    "run",
    "--quiet",
    "--locked",
    "-p",
    "vine-pe-probe",
    "--",
    "--v86-reflection-session",
    "--large-image-reflection",
    "--expect-sha256",
    expected.source.sha256,
    "--prepare-memory-pages",
    String(expected.memory.reflectedPages),
    "--materialize-image-base",
    `0x${expected.image.loadBase.toString(16)}`,
    "--bind-name",
    "d3d9.dll",
    "Direct3DCreate9",
    "--trap-unsupported-imports",
    "--install-callback-return-sentinel",
    "--install-process-entry-return-sentinel",
    "--install-controlled-initial-stack",
    "--install-controlled-process-environment",
    "--install-controlled-process-parameters",
    "--controlled-command-line",
    expected.processEnvironment.commandLine,
    bgiPath,
  ];
  const child = spawn("cargo", probeArguments, {
    cwd: root,
    windowsHide: true,
    stdio: ["pipe", "pipe", "pipe"],
  });
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
    const lineBytes = Buffer.byteLength(next.value);
    assert(
      lineBytes <= protocol.maximumLineBytes,
      `${label} exceeds the ${protocol.maximumLineBytes}-byte protocol limit`,
    );
    return { message: JSON.parse(next.value), lineBytes };
  };

  try {
    const offerMessage = await nextMessage("reflection offer");
    const offer = offerMessage.message;
    validateOffer(offer, offerMessage.lineBytes);
    const context = await prepareV86(offer);
    const invalidationCalls = [
      "vine_cpu_state_restore/full_clear_tlb",
      "full_clear_tlb",
      "jit_clear_cache_js",
    ];
    context.wasm.full_clear_tlb();
    context.wasm.jit_clear_cache_js();

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
    const receiptMessage = await nextMessage("reflection receipt");
    const receipt = receiptMessage.message;
    validateReceipt(receipt, offer, context, receiptMessage.lineBytes);
    const processExit = await exit;
    assert.equal(processExit.signal, null, `reflection probe terminated by ${processExit.signal}`);
    assert.equal(processExit.code, 0, `reflection probe failed: ${stderr.trim()}`);
    sessionCompleted = true;

    const execution = executeToFirstOwnedTrap(offer, context);
    assert.equal(execution.terminal.kind, "owned-import-thunk");
    validateThunk(execution.terminal.thunk, expected.terminal.thunk, true);
    assert.equal(execution.instructionCount, expected.terminal.instructionCount);
    assert.equal(execution.executeStops, expected.terminal.executeStops);
    assert.equal(execution.budgetStops, expected.terminal.budgetStops);
    assert.equal(execution.terminal.returnAddress, expected.terminal.returnAddress);
    assert.deepEqual(execution.terminal.stackWords, expected.terminal.stackWords);
    const sourceAfter = sourceIdentity(bgiPath);
    assert.deepEqual(sourceAfter, sourceBefore);
    const ownerSummaries = summarizeOwners(offer.reflection.pages);
    const report = {
      schema: "vine.v86-bgi-first-trap",
      schemaVersion: 1,
      source: offer.source,
      sourceSnapshot: {
        immutableAcrossRun: true,
        before: sourceBefore,
        after: sourceAfter,
      },
      contract: {
        largeImageReflection: {
          maximumReflectionPages: protocol.maximumReflectionPages,
          maximumProtocolLineBytes: protocol.maximumLineBytes,
        },
        commandLine: expected.processEnvironment.commandLine,
        stopBeforeDispatchOrReturn: true,
      },
      bootstrap: {
        imageLoadBase: offer.imageLoadBase,
        entryPoint: offer.entryPointAddress,
        initialThreadStack: offer.initialThreadStack,
        initialProcessEnvironment: offer.initialProcessEnvironment,
        fsSelector: 0,
        fsBase: offer.initialProcessEnvironment.tebAddress,
      },
      imports: {
        total: offer.thunks.length,
        declared: offer.thunks.filter(({ disposition }) => disposition === undefined).length,
        unsupported: offer.thunks.filter(
          ({ disposition }) => disposition === "unsupported-trap",
        ).length,
        first: offer.thunks[0],
        last: offer.thunks.at(-1),
      },
      adapter: {
        wasm: expected.wasm,
        physicalPoolBytes: guest.physicalPoolBytes,
        excludedPhysicalRanges: guest.excludedPhysicalRanges,
        pageDirectory: context.pageDirectory,
        pageTables: [...context.pageTables.entries()].map(
          ([directoryIndex, physicalAddress]) => ({ directoryIndex, physicalAddress }),
        ),
        allocatedPages: context.allocations.length,
        reflectedPageAllocations: context.allocations.filter(({ role }) =>
          role.startsWith("reflected-owner-"),
        ).length,
      },
      reflection: {
        offerLineBytes: offerMessage.lineBytes,
        receiptLineBytes: receiptMessage.lineBytes,
        pageSize: offer.reflection.pageSize,
        pages: offer.reflection.pages.length,
        contentSha256: offer.reflection.contentSha256,
        ownerSummaries,
        invalidation: offer.reflection.invalidation,
        acknowledgement: {
          generation: receipt.acknowledgedGeneration,
          callsBeforeAck: invalidationCalls,
          pendingAfterAck: receipt.stats.pendingInvalidation ?? null,
        },
      },
      execution,
      stop: {
        stage: "thunk-dispatch",
        code: "runtime-reachable-owned-import-trap",
        message:
          "BGI entry reached an exact runtime-owned import thunk; API dispatch, return-value fabrication, stack cleanup, and guest resume were not attempted",
      },
    };
    fs.mkdirSync(outputDirectory, { recursive: true });
    const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
    if (expected.report.bytes !== null) {
      assert.equal(reportBytes.byteLength, expected.report.bytes);
      assert.equal(sha256(reportBytes), expected.report.sha256);
    }
    fs.writeFileSync(reportPath, reportBytes);
    console.log(
      `v86 BGI first trap passed: ${offer.reflection.pages.length} pages, ` +
        `${execution.instructionCount} instructions, thunk ID ${execution.terminal.thunk.id} ` +
        `${execution.terminal.thunk.module}!${execution.terminal.thunk.name ?? `#${execution.terminal.thunk.ordinal}`}`,
    );
    console.log(
      `reflection sha256 ${offer.reflection.contentSha256}; report: ` +
        `${path.relative(root, reportPath)}, ${reportBytes.byteLength} bytes, ` +
        `sha256 ${sha256(reportBytes)}`,
    );
  } finally {
    lineReader.close();
    if (!sessionCompleted && child.exitCode === null) {
      child.kill();
    }
  }
}

function validateOffer(offer, offerLineBytes) {
  assert.equal(offer.schema, "vine.v86-memory-reflection");
  assert.equal(offer.schemaVersion, 1);
  assert.deepEqual(offer.source, expected.source);
  assert.equal(offer.imageLoadBase, expected.image.loadBase);
  assert.equal(offer.entryPointAddress, expected.image.entry);
  assert.equal(offer.imageOwner, expected.image.owner);
  assert.equal(offer.thunks.length, expected.imports.total);
  assert.deepEqual(offer.firstThunk, offer.thunks[0]);
  validateThunk(offer.thunks[0], expected.imports.first, false);
  validateThunk(offer.thunks[1], expected.imports.second, true);
  validateThunk(offer.thunks.at(-1), expected.imports.last, true);
  assert.equal(
    offer.thunks.filter(({ disposition }) => disposition === undefined).length,
    expected.imports.declared,
  );
  assert.equal(
    offer.thunks.filter(({ disposition }) => disposition === "unsupported-trap").length,
    expected.imports.unsupported,
  );
  for (let index = 0; index < offer.thunks.length; index += 1) {
    const thunk = offer.thunks[index];
    assert.equal(thunk.id, index + 1);
    assert.equal(thunk.address, 0x7f000000 + index * 0x10);
    assert.equal(thunk.requesterOwner, expected.image.owner);
  }

  assert.equal(offer.callbackReturnSentinel.id, expected.control.callbackId);
  assert.equal(offer.callbackReturnSentinel.address, expected.control.callbackAddress);
  assert.equal(offer.callbackReturnSentinel.owner, expected.control.owner);
  assert.equal(offer.processEntryReturnSentinel.id, expected.control.processEntryId);
  assert.equal(
    offer.processEntryReturnSentinel.address,
    expected.control.processEntryAddress,
  );
  assert.equal(offer.processEntryReturnSentinel.owner, expected.control.owner);
  assert.equal(offer.initialThreadStack.owner, expected.stack.owner);
  assert.equal(offer.initialThreadStack.stackRange.start, expected.stack.base);
  assert.equal(offer.initialThreadStack.stackTop, expected.stack.top);
  assert.equal(offer.initialThreadStack.initialEsp, expected.stack.initialEsp);

  const environment = offer.initialProcessEnvironment;
  const parameters = environment.processParameters;
  assert.equal(environment.tebOwner, expected.processEnvironment.tebOwner);
  assert.equal(environment.pebOwner, expected.processEnvironment.pebOwner);
  assert.equal(environment.tebAddress, expected.processEnvironment.tebBase);
  assert.equal(environment.pebAddress, expected.processEnvironment.pebBase);
  assert.equal(parameters.processParametersOwner, expected.processEnvironment.processParametersOwner);
  assert.equal(parameters.commandLineOwner, expected.processEnvironment.commandLineOwner);
  assert.equal(
    parameters.processParametersAddress,
    expected.processEnvironment.processParametersBase,
  );
  assert.equal(
    parameters.commandLineBufferAddress,
    expected.processEnvironment.commandLineBase,
  );
  assert.equal(parameters.commandLine, expected.processEnvironment.commandLine);
  assert.equal(
    parameters.commandLineLengthBytes,
    expected.processEnvironment.commandLineLengthBytes,
  );
  assert.equal(
    parameters.commandLineMaximumLengthBytes,
    expected.processEnvironment.commandLineMaximumLengthBytes,
  );
  assert.equal(
    environment.tebContentSha256,
    expected.processEnvironment.tebContentSha256,
  );
  assert.equal(
    environment.pebContentSha256,
    expected.processEnvironment.pebContentSha256,
  );
  assert.equal(
    parameters.processParametersContentSha256,
    expected.processEnvironment.processParametersContentSha256,
  );
  assert.equal(
    parameters.commandLineContentSha256,
    expected.processEnvironment.commandLineContentSha256,
  );
  assert.deepEqual(environment.invalidation, {
    generation: expected.memory.generation,
    dataTlb: true,
    instructionFetch: true,
    generatedCode: true,
  });

  const reflected = offer.reflection;
  assert.equal(reflected.pageSize, page.size);
  assert(reflected.pages.length <= protocol.maximumReflectionPages);
  assert.equal(reflected.pages.length, expected.memory.reflectedPages);
  assert.deepEqual(reflected.invalidation, environment.invalidation);
  assert.equal(reflectionContentSha256(reflected.pages), reflected.contentSha256);
  if (expected.reflectionContentSha256 !== null) {
    assert.equal(reflected.contentSha256, expected.reflectionContentSha256);
  }
  if (expected.offerLineBytes !== null) {
    assert.equal(offerLineBytes, expected.offerLineBytes);
  }
  for (const reflectedPage of reflected.pages) {
    assert.equal(reflectedPage.ownerSealed, true);
    assert.equal(reflectedPage.dirty, true);
    assert.equal(reflectedPage.bytes.length, page.size);
  }
  validateReflectedLayout(reflected.pages);
}

function validateThunk(thunk, identity, unsupported) {
  for (const [key, value] of Object.entries(identity)) {
    assert.equal(thunk[key], value);
  }
  assert.equal(thunk.kind, identity.ordinal === undefined ? "name" : "ordinal");
  assert.equal(thunk.disposition, unsupported ? "unsupported-trap" : undefined);
}

function validateReflectedLayout(pages) {
  let index = 0;
  assertReflectedPage(pages[index], expected.stack.base, expected.stack.owner, true, false);
  index += 1;
  let imageReadable = 0;
  let imageWritable = 0;
  let imageExecutable = 0;
  for (let pageIndex = 0; pageIndex < expected.image.pages; pageIndex += 1) {
    const reflected = pages[index];
    assert.equal(reflected.address, expected.image.loadBase + pageIndex * page.size);
    assert.equal(reflected.owner, expected.image.owner);
    imageReadable += Number(reflected.protection.read);
    imageWritable += Number(reflected.protection.write);
    imageExecutable += Number(reflected.protection.execute);
    index += 1;
  }
  assert.equal(imageReadable, expected.image.readablePages);
  assert.equal(imageWritable, expected.image.writablePages);
  assert.equal(imageExecutable, expected.image.executablePages);
  assertReflectedPage(pages[index], 0x7f000000, expected.imports.owner, false, true);
  index += 1;
  assertReflectedPage(pages[index], 0x7f001000, expected.imports.owner, false, true);
  index += 1;
  assertReflectedPage(pages[index], expected.control.page, expected.control.owner, false, true);
  index += 1;
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.processParametersBase,
    expected.processEnvironment.processParametersOwner,
    true,
    false,
  );
  index += 1;
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.commandLineBase,
    expected.processEnvironment.commandLineOwner,
    true,
    false,
  );
  index += 1;
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.tebBase,
    expected.processEnvironment.tebOwner,
    true,
    false,
  );
  index += 1;
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.pebBase,
    expected.processEnvironment.pebOwner,
    true,
    false,
  );
  index += 1;
  assert.equal(index, pages.length);
}

function assertReflectedPage(reflected, address, owner, writable, executable) {
  assert.equal(reflected.address, address);
  assert.equal(reflected.owner, owner);
  assert.deepEqual(reflected.protection, { read: true, write: writable, execute: executable });
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
      assert.equal(
        imported.name,
        "microtick",
        `unexpected host import ${imported.name}(${args.join(", ")})`,
      );
      assert.equal(args.length, 0);
      return 0;
    };
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
  const physicalView = new DataView(
    physicalMemory.buffer,
    physicalMemory.byteOffset,
    physicalMemory.byteLength,
  );
  wasm.zero_memory(0, guest.physicalPoolBytes);

  const allocations = [];
  let nextPhysicalPage = page.size;
  const allocatePhysicalPage = (role) => {
    for (const excluded of guest.excludedPhysicalRanges) {
      if (nextPhysicalPage >= excluded.start && nextPhysicalPage < excluded.end) {
        nextPhysicalPage = excluded.end;
      }
    }
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
  for (const reflected of offer.reflection.pages) {
    const virtualBase = reflected.address;
    checkedGuestEnd(virtualBase, page.size, "reflected page");
    assert.equal(virtualBase & (page.size - 1), 0);
    assert(!mappings.has(virtualBase), `duplicate mapping 0x${virtualBase.toString(16)}`);
    const table = ensurePageTable(virtualBase);
    const tableIndex = (virtualBase >>> 12) & 0x3ff;
    const physicalBase = allocatePhysicalPage(`reflected-owner-${reflected.owner}`);
    writePhysicalU32(
      table + tableIndex * 4,
      physicalBase |
        page.present |
        page.user |
        (reflected.protection.write ? page.writable : 0),
      "reflected page table entry",
    );
    physicalMemory.set(Uint8Array.from(reflected.bytes), physicalBase);
    mappings.set(virtualBase, {
      physicalBase,
      protection: { ...reflected.protection },
      role: `reflected-owner-${reflected.owner}`,
    });
  }

  const mappingFor = (address, label) => {
    checkedGuestEnd(address, 1, label);
    const virtualBase = (address & ~(page.size - 1)) >>> 0;
    const mapping = mappings.get(virtualBase);
    assert(mapping, `${label} uses unmapped guest address 0x${address.toString(16)}`);
    return mapping;
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

  assert.equal(
    readVirtualU32(offer.initialThreadStack.initialEsp, "entry return sentinel"),
    expected.control.processEntryAddress,
  );
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
  words.fill(0, stateWord.eax, stateWord.eax + 8);
  words[stateWord.esp] = offer.initialThreadStack.initialEsp;
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
  words[stateWord.segmentOffsetBase + segmentIndex.fs] =
    offer.initialProcessEnvironment.tebAddress;
  words[stateWord.segmentMetaBase + segmentIndex.cs] = 0xfa;
  words[stateWord.controlRegisterBase] |= 0x80010001;
  words[stateWord.controlRegisterBase + 3] = pageDirectory;
  words[stateWord.controlRegisterBase + 4] |= 0x200;
  assert.equal(wasm.vine_cpu_state_restore(), 0);
  assert.equal(allocations.length, 393);
  assert.deepEqual([...pageTables.keys()], [1, 64, 508, 511]);
  for (const allocation of allocations) {
    for (const excluded of guest.excludedPhysicalRanges) {
      assert(
        allocation.address + page.size <= excluded.start || allocation.address >= excluded.end,
        `${allocation.role} overlaps excluded physical range ${excluded.role}`,
      );
    }
  }

  return {
    allocations,
    hostImportCalls,
    mappings,
    pageDirectory,
    pageTables,
    readVirtualBytes,
    readVirtualU32,
    wasm,
  };
}

function validateReceipt(receipt, offer, context, receiptLineBytes) {
  assert.equal(receipt.schema, "vine.v86-memory-reflection-receipt");
  assert.equal(receipt.schemaVersion, 1);
  assert.equal(receipt.acknowledgedGeneration, expected.memory.generation);
  assert.equal(receipt.stats.owners, expected.memory.owners);
  assert.equal(receipt.stats.sealedOwners, expected.memory.owners);
  assert.equal(receipt.stats.regions, expected.memory.owners);
  assert.equal(receipt.stats.reservedPages, expected.memory.reservedPages);
  assert.equal(receipt.stats.committedPages, expected.memory.reflectedPages);
  assert.equal(receipt.stats.physicalPages, expected.memory.reflectedPages);
  assert.equal(receipt.stats.dirtyPages, expected.memory.reflectedPages);
  assert.equal(receipt.stats.generation, expected.memory.generation);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  assert.equal(receipt.iatValue, expected.imports.first.address);
  assert.deepEqual(
    receipt.entryBytes,
    [...context.readVirtualBytes(expected.image.entry, 16, "entry receipt bytes")],
  );
  assert.deepEqual(receipt.thunkBytes, [
    0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
    0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
  ]);
  assert.deepEqual(receipt.callbackReturnBytes, receipt.thunkBytes);
  assert.deepEqual(receipt.processEntryReturnBytes, receipt.thunkBytes);
  assert.deepEqual(receipt.initialStackTopBytes, [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10, 0x00, 0x01, 0x7f,
  ]);
  assert.deepEqual(
    receipt.tebBytes,
    [
      ...context.readVirtualBytes(
        expected.processEnvironment.tebBase,
        64,
        "TEB receipt bytes",
      ),
    ],
  );
  assert.deepEqual(
    receipt.pebBytes,
    [
      ...context.readVirtualBytes(
        expected.processEnvironment.pebBase,
        64,
        "PEB receipt bytes",
      ),
    ],
  );
  assert.deepEqual(
    receipt.processParametersBytes,
    [
      ...context.readVirtualBytes(
        expected.processEnvironment.processParametersBase,
        0x48,
        "process parameters receipt bytes",
      ),
    ],
  );
  const expectedCommandLineBytes = Buffer.concat([
    Buffer.from(expected.processEnvironment.commandLine, "utf16le"),
    Buffer.from([0, 0]),
  ]);
  assert.deepEqual(receipt.commandLineBytes, [...expectedCommandLineBytes]);
  assert.equal(receipt.stop.stage, "adapter-execution");
  assert.equal(receipt.stop.code, "entry-to-first-owned-thunk-pending");
  assert.equal(offer.reflection.invalidation.generation, receipt.acknowledgedGeneration);
  if (expected.receiptLineBytes !== null) {
    assert.equal(receiptLineBytes, expected.receiptLineBytes);
  }
}

function executeToFirstOwnedTrap(offer, context) {
  const executablePages = new Set(
    offer.reflection.pages
      .filter(({ ownerSealed, protection }) => ownerSealed && protection.execute)
      .map(({ address }) => address),
  );
  const thunksByAddress = new Map(offer.thunks.map((thunk) => [thunk.address, thunk]));
  const sentinelsByAddress = new Map([
    [offer.callbackReturnSentinel.address, "callback-return-sentinel"],
    [offer.processEntryReturnSentinel.address, "process-entry-return-sentinel"],
  ]);
  const saveState = (reason) => {
    context.wasm.vine_cpu_state_save();
    const words = Uint32Array.from(
      new Uint32Array(
        context.wasm.memory.buffer,
        context.wasm.vine_cpu_state_ptr(),
        context.wasm.vine_cpu_state_size() / 4,
      ),
    );
    const privateState = new DataView(context.wasm.memory.buffer);
    return {
      reason,
      errorCode: context.wasm.vine_get_stop_error_code(),
      instructionCount: privateState.getUint32(
        adapterPrivateOffset.instructionCounter,
        true,
      ),
      eax: words[stateWord.eax],
      ecx: words[stateWord.ecx],
      edx: words[stateWord.edx],
      ebx: words[stateWord.ebx],
      esp: words[stateWord.esp],
      ebp: words[stateWord.ebp],
      esi: words[stateWord.esi],
      edi: words[stateWord.edi],
      flags: words[stateWord.flags],
      faultAddress: words[stateWord.controlRegisterBase + 2],
      instructionPointer: words[stateWord.instructionPointer],
      previousInstructionPointer: words[stateWord.previousInstructionPointer],
      fsSelector: words[stateWord.segmentSelectorBase + segmentIndex.fs],
      fsBase: words[stateWord.segmentOffsetBase + segmentIndex.fs],
    };
  };

  const initial = saveState(0);
  assert.equal(initial.instructionPointer, expected.image.entry);
  assert.equal(initial.esp, expected.stack.initialEsp);
  assert.equal(initial.fsSelector, 0);
  assert.equal(initial.fsBase, expected.processEnvironment.tebBase);
  let budgetStops = 0;
  let executeStops = 0;
  let terminal = null;
  let finalState = initial;
  for (let index = 0; index < guest.maximumExecuteStops; index += 1) {
    const before = saveState(0);
    const currentPage = (before.instructionPointer & ~(page.size - 1)) >>> 0;
    if (!executablePages.has(currentPage)) {
      terminal = {
        kind: "logical-execute-gate",
        instructionPointer: before.instructionPointer,
      };
      finalState = before;
      break;
    }
    const reason = context.wasm.vine_execute_budget(guest.instructionBudget);
    executeStops += 1;
    const state = saveState(reason);
    finalState = state;
    if (reason === stopReason.budget) {
      budgetStops += 1;
      continue;
    }
    terminal = classifyTerminal(state, thunksByAddress, sentinelsByAddress, context);
    break;
  }
  assert(terminal, "BGI exhausted its bounded execution window without a terminal stop");
  return {
    instructionBudget: guest.instructionBudget,
    maximumExecuteStops: guest.maximumExecuteStops,
    maximumInstructions: guest.instructionBudget * guest.maximumExecuteStops,
    executeStops,
    budgetStops,
    instructionCount: finalState.instructionCount,
    finalState: {
      reason: finalState.reason,
      errorCode: finalState.errorCode,
      instructionPointer: finalState.instructionPointer,
      previousInstructionPointer: finalState.previousInstructionPointer,
      faultAddress: finalState.faultAddress,
      eax: finalState.eax,
      ecx: finalState.ecx,
      edx: finalState.edx,
      ebx: finalState.ebx,
      esp: finalState.esp,
      ebp: finalState.ebp,
      esi: finalState.esi,
      edi: finalState.edi,
      flags: finalState.flags,
      fsSelector: finalState.fsSelector,
      fsBase: finalState.fsBase,
    },
    terminal,
    hostImportCalls: context.hostImportCalls,
  };
}

function classifyTerminal(state, thunksByAddress, sentinelsByAddress, context) {
  if (state.reason === stopReason.invalidOpcode) {
    const thunk = thunksByAddress.get(state.instructionPointer);
    if (thunk !== undefined && state.previousInstructionPointer === thunk.address) {
      const stackWords = [];
      for (let index = 0; index < 8; index += 1) {
        stackWords.push(
          context.readVirtualU32(state.esp + index * 4, `trap stack word ${index}`),
        );
      }
      return {
        kind: "owned-import-thunk",
        thunk,
        returnAddress: stackWords[0],
        stackWords,
      };
    }
    const sentinel = sentinelsByAddress.get(state.instructionPointer);
    if (sentinel !== undefined && state.previousInstructionPointer === state.instructionPointer) {
      return { kind: sentinel, address: state.instructionPointer };
    }
    return {
      kind: "unowned-invalid-opcode",
      instructionPointer: state.instructionPointer,
      previousInstructionPointer: state.previousInstructionPointer,
    };
  }
  if (state.reason === stopReason.pageFault) {
    return {
      kind: "page-fault",
      instructionPointer: state.instructionPointer,
      previousInstructionPointer: state.previousInstructionPointer,
      faultAddress: state.faultAddress,
      pageFaultErrorCode: state.errorCode,
    };
  }
  return {
    kind: "cpu-stop",
    reason: state.reason,
    errorCode: state.errorCode,
    instructionPointer: state.instructionPointer,
    previousInstructionPointer: state.previousInstructionPointer,
  };
}

run().catch((error) => {
  console.error(error.stack ?? error.message);
  process.exitCode = 1;
});
