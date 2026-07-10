import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const defaultWasmPath = path.join(
  root,
  ".work",
  "v86-adapter-source",
  "build",
  "v86-fallback.wasm",
);
const defaultExePath = path.join(
  root,
  ".work",
  "toolchain-spike",
  "window-pixel.exe",
);
const wasmPath = path.resolve(process.argv[2] ?? defaultWasmPath);
const exePath = path.resolve(process.argv[3] ?? defaultExePath);

const expectedExe = {
  fileName: "window-pixel.exe",
  size: 4_096,
  sha256: "ad8f4a0bddd759ac52b18119e60838c3ad13d29994449f38b0393fdedde454e9",
  imageBase: 0x00400000,
  entry: 0x00401000,
  imageSize: 0x5000,
  headerSize: 0x400,
};
const expectedWasm = {
  size: 1_882_836,
  sha256: "f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26",
  importCount: 22,
  exportCount: 809,
};
const expectedImports = [
  { module: "kernel32.dll", symbol: "GetModuleHandleA", iatRva: 0x20c0 },
  { module: "user32.dll", symbol: "BeginPaint", iatRva: 0x20c8 },
  { module: "user32.dll", symbol: "CreateWindowExA", iatRva: 0x20cc },
  { module: "user32.dll", symbol: "DefWindowProcA", iatRva: 0x20d0 },
  { module: "user32.dll", symbol: "DestroyWindow", iatRva: 0x20d4 },
  { module: "user32.dll", symbol: "DispatchMessageA", iatRva: 0x20d8 },
  { module: "user32.dll", symbol: "EndPaint", iatRva: 0x20dc },
  { module: "user32.dll", symbol: "GetMessageA", iatRva: 0x20e0 },
  { module: "user32.dll", symbol: "InvalidateRect", iatRva: 0x20e4 },
  { module: "user32.dll", symbol: "PostQuitMessage", iatRva: 0x20e8 },
  { module: "user32.dll", symbol: "RegisterClassExA", iatRva: 0x20ec },
  { module: "user32.dll", symbol: "ShowWindow", iatRva: 0x20f0 },
  { module: "gdi32.dll", symbol: "SetPixel", iatRva: 0x20f8 },
];
const expectedWindowClass = {
  address: 0x00403004,
  nameAddress: 0x00402000,
  name: "VineWindowPixelFixture",
  wndProc: 0x00401260,
  words: [
    48,
    0,
    0x00401260,
    0,
    0,
    0x00400000,
    0,
    0,
    0,
    0,
    0x00402000,
    0,
  ],
};
const expectedCreateWindow = {
  variant: "createwindowexa-v12-local-ascii-wm-create",
  callSite: 0x00401095,
  returnAddress: 0x0040109b,
  iatAddress: 0x004020cc,
  thunkAddress: 0x00500020,
  className: "VineWindowPixelFixture",
  windowName: "VineWindowPixelFixture",
  logicalHandle: 0x57010001,
  arguments: [
    0,
    0x00402000,
    0x00402000,
    0x00cf0000,
    0x80000000,
    0x80000000,
    160,
    120,
    0,
    0,
    0x00400000,
    0,
  ],
  createStructWords: [
    0,
    0x00400000,
    0,
    0,
    120,
    160,
    0x80000000,
    0x80000000,
    0x00cf0000,
    0x00402000,
    0x00402000,
    0,
  ],
  createStructAddress: 0x00501000,
  callbackStackPointer: 0x006fffac,
  callbackReturnStackPointer: 0x006fffc0,
};
const expectedShowWindow = {
  variant: "showwindow-sw-show-hidden-to-visible",
  callSite: 0x004010a8,
  returnAddress: 0x004010ae,
  iatAddress: 0x004020f0,
  thunkAddress: 0x005000b0,
  stackPointer: 0x006fffe8,
  arguments: [0x57010001, 5],
};
const expectedInvalidateRect = {
  variant: "invalidaterect-whole-client-no-erase",
  callSite: 0x004010b3,
  returnAddress: 0x004010b9,
  iatAddress: 0x004020e4,
  thunkAddress: 0x00500080,
  stackPointer: 0x006fffe4,
  arguments: [0x57010001, 0, 0],
};
const expectedGetMessage = {
  variant: "getmessagea-unfiltered-generated-paint-zero-metadata",
  callSite: 0x004010cc,
  returnAddress: 0x004010d2,
  iatAddress: 0x004020e0,
  thunkAddress: 0x00500070,
  stackPointer: 0x006fffe0,
  arguments: [0x00403034, 0, 0, 0],
  messageWords: [0x57010001, 0x000f, 0, 0, 0, 0, 0],
};
const expectedDispatchMessage = {
  variant: "dispatchmessagea-generated-wm-paint-callback",
  callSite: 0x004010f5,
  returnAddress: 0x004010f7,
  iatAddress: 0x004020d8,
  thunkAddress: 0x00500050,
  stackPointer: 0x006fffec,
  arguments: [0x00403034],
  callbackTarget: 0x00401260,
  callbackStackPointer: 0x006fffd8,
  callbackReturnStackPointer: 0x006fffec,
};
const expectedBeginPaint = {
  callSite: 0x0040133d,
  returnAddress: 0x00401343,
  iatAddress: 0x004020c8,
  thunkAddress: 0x00500010,
  stackPointer: 0x006fffc8,
  arguments: [0x57010001, 0x0040308c],
};
const expectedSections = [
  {
    name: ".text",
    virtualSize: 0x409,
    virtualAddress: 0x1000,
    rawSize: 0x600,
    rawOffset: 0x400,
    characteristics: 0x60000020,
  },
  {
    name: ".rdata",
    virtualSize: 0x1f8,
    virtualAddress: 0x2000,
    rawSize: 0x200,
    rawOffset: 0x0a00,
    characteristics: 0x40000040,
  },
  {
    name: ".data",
    virtualSize: 0x0cc,
    virtualAddress: 0x3000,
    rawSize: 0x200,
    rawOffset: 0x0c00,
    characteristics: 0xc0000040,
  },
  {
    name: ".reloc",
    virtualSize: 0x0b8,
    virtualAddress: 0x4000,
    rawSize: 0x200,
    rawOffset: 0x0e00,
    characteristics: 0x42000040,
  },
];
const guest = {
  memorySize: 0x00800000,
  trapPage: 0x00500000,
  thunkStride: 0x10,
  callbackReturn: 0x00500e00,
  entryReturn: 0x00500f00,
  callbackScratch: 0x00501000,
  callbackScratchSize: 0x1000,
  stackBase: 0x00600000,
  stackTop: 0x00700000,
  instructionBudget: 3,
};
const stopReason = {
  budget: 1,
  invalidOpcode: 0x106,
};

const privateStateOffset = {
  registers: 64,
  flags: 120,
  segmentAccessBytes: 512,
  instructionPointer: 556,
  previousInstructionPointer: 560,
  controlRegisters: 580,
  cpl: 612,
  inHalt: 616,
  lastVirtualInstructionPointer: 620,
  instructionCounter: 664,
  segmentIsNull: 724,
  segmentOffsets: 736,
  segmentLimits: 768,
  protectedMode: 800,
  operandSize32: 804,
  stackSize32: 808,
  memorySize: 812,
};
const savedStateWord = {
  magic: 0,
  version: 1,
  wordCount: 2,
  eax: 4,
  ebx: 7,
  esp: 8,
  ebp: 9,
  esi: 10,
  edi: 11,
  instructionPointer: 12,
  previousInstructionPointer: 13,
};
const calleeSaved = {
  ebx: 0x11223344,
  ebp: 0x55667788,
  esi: 0x99aabbcc,
  edi: 0xddeeff00,
};

function requireFile(file, command) {
  if (!fs.existsSync(file)) {
    throw new Error(
      `${path.relative(root, file)} is missing; run ${command} first`,
    );
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

function parsePe32(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const u16 = (offset, label) => {
    checkedEnd(offset, 2, bytes.byteLength, label);
    return view.getUint16(offset, true);
  };
  const u32 = (offset, label) => {
    checkedEnd(offset, 4, bytes.byteLength, label);
    return view.getUint32(offset, true);
  };
  const cStringAtOffset = (offset, label) => {
    assert(offset < bytes.byteLength, `${label} starts outside the file`);
    let end = offset;
    while (end < bytes.byteLength && bytes[end] !== 0) {
      end += 1;
    }
    assert(end < bytes.byteLength, `${label} is not terminated`);
    return Buffer.from(bytes.subarray(offset, end)).toString("ascii");
  };

  assert.equal(u16(0, "DOS magic"), 0x5a4d, "fixture is not an MZ image");
  const peOffset = u32(0x3c, "PE header offset");
  assert.equal(u32(peOffset, "PE signature"), 0x00004550, "invalid PE signature");
  const coffOffset = peOffset + 4;
  assert.equal(u16(coffOffset, "machine"), 0x014c, "fixture is not i386");
  const sectionCount = u16(coffOffset + 2, "section count");
  const optionalSize = u16(coffOffset + 16, "optional header size");
  const optionalOffset = coffOffset + 20;
  checkedEnd(optionalOffset, optionalSize, bytes.byteLength, "optional header");
  assert.equal(u16(optionalOffset, "optional magic"), 0x010b, "fixture is not PE32");
  assert(optionalSize >= 224, "PE32 optional header is too small");

  const entryRva = u32(optionalOffset + 16, "entry RVA");
  const imageBase = u32(optionalOffset + 28, "image base");
  const sizeOfImage = u32(optionalOffset + 56, "image size");
  const sizeOfHeaders = u32(optionalOffset + 60, "header size");
  const directoryCount = u32(optionalOffset + 92, "directory count");
  assert(directoryCount > 12, "PE32 image has no IAT directory");
  const directoryOffset = optionalOffset + 96;
  checkedEnd(
    directoryOffset,
    directoryCount * 8,
    optionalOffset + optionalSize,
    "data directories",
  );
  const importDirectory = {
    rva: u32(directoryOffset + 8, "import directory RVA"),
    size: u32(directoryOffset + 12, "import directory size"),
  };
  const iatDirectory = {
    rva: u32(directoryOffset + 12 * 8, "IAT directory RVA"),
    size: u32(directoryOffset + 12 * 8 + 4, "IAT directory size"),
  };

  const sections = [];
  const sectionTable = optionalOffset + optionalSize;
  checkedEnd(sectionTable, sectionCount * 40, bytes.byteLength, "section table");
  for (let index = 0; index < sectionCount; index += 1) {
    const offset = sectionTable + index * 40;
    let nameEnd = offset;
    while (nameEnd < offset + 8 && bytes[nameEnd] !== 0) {
      nameEnd += 1;
    }
    sections.push({
      name: Buffer.from(bytes.subarray(offset, nameEnd)).toString("ascii"),
      virtualSize: u32(offset + 8, "section virtual size"),
      virtualAddress: u32(offset + 12, "section RVA"),
      rawSize: u32(offset + 16, "section raw size"),
      rawOffset: u32(offset + 20, "section raw offset"),
      characteristics: u32(offset + 36, "section characteristics"),
    });
  }

  const rvaToFileOffset = (rva, size, label) => {
    if (rva < sizeOfHeaders) {
      checkedEnd(rva, size, Math.min(sizeOfHeaders, bytes.byteLength), label);
      return rva;
    }
    const section = sections.find(
      ({ virtualAddress, rawSize }) =>
        rva >= virtualAddress && rva - virtualAddress + size <= rawSize,
    );
    assert(section, `${label} is not backed by section raw data`);
    const offset = section.rawOffset + (rva - section.virtualAddress);
    checkedEnd(offset, size, bytes.byteLength, label);
    return offset;
  };
  const cStringAtRva = (rva, label) =>
    cStringAtOffset(rvaToFileOffset(rva, 1, label), label);

  const imports = [];
  const descriptors = [];
  assert(
    importDirectory.rva !== 0 && importDirectory.size >= 20,
    "missing import directory",
  );
  const importEnd = importDirectory.rva + importDirectory.size;
  for (let descriptorRva = importDirectory.rva; ; descriptorRva += 20) {
    assert(
      descriptorRva + 20 <= importEnd,
      "unterminated import descriptor table",
    );
    const descriptorOffset = rvaToFileOffset(
      descriptorRva,
      20,
      "import descriptor",
    );
    const originalFirstThunk = u32(descriptorOffset, "import lookup RVA");
    const nameRva = u32(descriptorOffset + 12, "import DLL name RVA");
    const firstThunk = u32(descriptorOffset + 16, "import IAT RVA");
    if (originalFirstThunk === 0 && nameRva === 0 && firstThunk === 0) {
      break;
    }
    assert(nameRva !== 0 && firstThunk !== 0, "incomplete import descriptor");
    const module = cStringAtRva(nameRva, "import DLL name").toLowerCase();
    const lookupRva = originalFirstThunk || firstThunk;
    let importCount = 0;
    for (let index = 0; ; index += 1) {
      const lookupOffset = rvaToFileOffset(
        lookupRva + index * 4,
        4,
        "import lookup entry",
      );
      const lookup = u32(lookupOffset, "import lookup value");
      if (lookup === 0) {
        importCount = index;
        break;
      }
      assert.equal(
        lookup >>> 31,
        0,
        "ordinal import is outside this fixed W2 fixture",
      );
      const hintNameOffset = rvaToFileOffset(lookup, 3, "import hint/name");
      imports.push({
        module,
        symbol: cStringAtOffset(hintNameOffset + 2, "import name"),
        hint: u16(hintNameOffset, "import hint"),
        lookup,
        iatRva: firstThunk + index * 4,
      });
    }
    descriptors.push({
      module,
      lookupRva,
      iatRva: firstThunk,
      importCount,
      terminatorRva: firstThunk + importCount * 4,
    });
  }

  return {
    entryRva,
    imageBase,
    sizeOfImage,
    sizeOfHeaders,
    importDirectory,
    iatDirectory,
    sections,
    descriptors,
    imports,
  };
}

requireFile(wasmPath, "npm run prepare:engine");
requireFile(exePath, "npm run check:toolchain");

const exeBytes = fs.readFileSync(exePath);
const sourceHash = sha256(exeBytes);
assert.equal(exeBytes.byteLength, expectedExe.size, "unexpected W2 fixture size");
assert.equal(sourceHash, expectedExe.sha256, "window-pixel.exe is not the fixed W2 fixture");
const pe = parsePe32(exeBytes);
assert.equal(pe.imageBase, expectedExe.imageBase);
assert.equal(pe.imageBase + pe.entryRva, expectedExe.entry);
assert.equal(pe.sizeOfImage, expectedExe.imageSize);
assert.equal(pe.sizeOfHeaders, expectedExe.headerSize);
assert.deepEqual(pe.importDirectory, { rva: 0x2030, size: 0x50 });
assert.deepEqual(pe.iatDirectory, { rva: 0x20c0, size: 0x40 });
assert.deepEqual(pe.sections, expectedSections);
assert.deepEqual(
  pe.descriptors,
  [
    {
      module: "kernel32.dll",
      lookupRva: 0x2080,
      iatRva: 0x20c0,
      importCount: 1,
      terminatorRva: 0x20c4,
    },
    {
      module: "user32.dll",
      lookupRva: 0x2088,
      iatRva: 0x20c8,
      importCount: 11,
      terminatorRva: 0x20f4,
    },
    {
      module: "gdi32.dll",
      lookupRva: 0x20b8,
      iatRva: 0x20f8,
      importCount: 1,
      terminatorRva: 0x20fc,
    },
  ],
);
assert.deepEqual(
  pe.imports.map(({ module, symbol, hint, iatRva }) => ({
    module,
    symbol,
    hint,
    iatRva,
  })),
  expectedImports.map((entry) => ({ ...entry, hint: 0 })),
);

const wasmBytes = fs.readFileSync(wasmPath);
assert.equal(wasmBytes.byteLength, expectedWasm.size, "unexpected adapter Wasm size");
assert.equal(
  sha256(wasmBytes),
  expectedWasm.sha256,
  "adapter Wasm does not match the pinned W2 artifact",
);
const wasmModule = new WebAssembly.Module(wasmBytes);
const imports = WebAssembly.Module.imports(wasmModule);
const exportsDescription = WebAssembly.Module.exports(wasmModule);
assert.equal(imports.length, expectedWasm.importCount);
assert.equal(exportsDescription.length, expectedWasm.exportCount);
const exportNames = new Set(exportsDescription.map(({ name }) => name));
const requiredExports = [
  "allocate_memory",
  "full_clear_tlb",
  "memory",
  "reset_cpu",
  "rust_init",
  "update_state_flags",
  "vine_cpu_state_ptr",
  "vine_cpu_state_restore",
  "vine_cpu_state_save",
  "vine_cpu_state_size",
  "vine_cpu_state_version",
  "vine_execute_budget",
  "vine_get_stop_error_code",
  "zero_memory",
];
for (const name of requiredExports) {
  assert(exportNames.has(name), `missing adapter export ${name}`);
}
assert(!imports.some(({ name }) => name === "cpu_exception_hook"));

const hostImportCalls = {};
const allowedHostImports = new Map([["microtick", () => 0]]);
const environment = {
  __indirect_function_table: new WebAssembly.Table({
    element: "anyfunc",
    initial: 1_924,
  }),
};
for (const imported of imports) {
  if (imported.kind !== "function") {
    continue;
  }
  environment[imported.name] = (...args) => {
    hostImportCalls[imported.name] = (hostImportCalls[imported.name] ?? 0) + 1;
    const implementation = allowedHostImports.get(imported.name);
    assert(implementation, `unexpected v86 host import call ${imported.name}`);
    return implementation(...args);
  };
}

const { instance } = await WebAssembly.instantiate(wasmBytes, { env: environment });
const wasm = instance.exports;
wasm.rust_init();
let privateState = new DataView(wasm.memory.buffer);
privateState.setUint32(privateStateOffset.memorySize, guest.memorySize, true);
const guestMemoryBase = wasm.allocate_memory(guest.memorySize);
privateState = new DataView(wasm.memory.buffer);
const memory = new Uint8Array(
  wasm.memory.buffer,
  guestMemoryBase,
  guest.memorySize,
);
const memoryView = new DataView(memory.buffer, memory.byteOffset, memory.byteLength);

const assertGuestRange = (address, size, label) =>
  checkedEnd(address, size, guest.memorySize, label);
const readGuestU32 = (address, label) => {
  assertGuestRange(address, 4, label);
  return memoryView.getUint32(address, true);
};
const writeGuestU32 = (address, value, label) => {
  assertGuestRange(address, 4, label);
  memoryView.setUint32(address, value >>> 0, true);
};
const assertTrapPageRange = (address, size, label) => {
  assert(address >= guest.trapPage, `${label} starts before the trap page`);
  checkedEnd(address, size, guest.trapPage + 0x1000, label);
};

wasm.zero_memory(0, guest.memorySize);
wasm.reset_cpu();
assertGuestRange(pe.imageBase, pe.sizeOfImage, "PE image");
assertGuestRange(guest.trapPage, 0x1000, "runtime trap page");
assertGuestRange(
  guest.callbackScratch,
  guest.callbackScratchSize,
  "runtime callback scratch",
);
assertGuestRange(
  guest.stackBase,
  guest.stackTop - guest.stackBase,
  "guest stack",
);
assert.equal(guest.stackTop - guest.stackBase, 0x100000);
assert(pe.imageBase + pe.sizeOfImage <= guest.trapPage, "PE image overlaps trap page");
assert(
  guest.trapPage + 0x1000 <= guest.callbackScratch,
  "trap page overlaps callback scratch",
);
assert(
  guest.callbackScratch + guest.callbackScratchSize <= guest.stackBase,
  "callback scratch overlaps guest stack",
);
assert(pe.sizeOfHeaders <= pe.sizeOfImage, "PE headers exceed image size");
assert(pe.sizeOfHeaders <= exeBytes.byteLength, "PE headers exceed fixture file");
memory.set(exeBytes.subarray(0, pe.sizeOfHeaders), pe.imageBase);
for (const section of pe.sections) {
  const mappedSize = Math.max(section.virtualSize, section.rawSize);
  assert(
    section.virtualAddress + mappedSize <= pe.sizeOfImage,
    `${section.name} exceeds SizeOfImage`,
  );
  const target = pe.imageBase + section.virtualAddress;
  assertGuestRange(target, mappedSize, `${section.name} mapping`);
  if (section.rawSize !== 0) {
    checkedEnd(
      section.rawOffset,
      section.rawSize,
      exeBytes.byteLength,
      `${section.name} raw data`,
    );
    memory.set(
      exeBytes.subarray(section.rawOffset, section.rawOffset + section.rawSize),
      target,
    );
  }
}
assert.equal(sha256(exeBytes), sourceHash, "mapping mutated the immutable fixture bytes");

for (const imported of pe.imports) {
  assert.equal(
    readGuestU32(pe.imageBase + imported.iatRva, `${imported.symbol} original IAT`),
    imported.lookup,
  );
}
for (const { terminatorRva } of pe.descriptors) {
  assert.equal(
    readGuestU32(pe.imageBase + terminatorRva, "original IAT terminator"),
    0,
  );
}

memory.fill(0xf4, guest.trapPage, guest.trapPage + 0x1000);
const ownedServices = new Map();
const resolvedImports = pe.imports.map((imported, index) => {
  const thunkAddress = guest.trapPage + index * guest.thunkStride;
  assertTrapPageRange(thunkAddress, 2, "runtime import thunk");
  const iatAddress = pe.imageBase + imported.iatRva;
  memory.set(Uint8Array.from([0x0f, 0x0b]), thunkAddress);
  writeGuestU32(iatAddress, thunkAddress, `${imported.symbol} resolved IAT`);
  const service = {
    requester: expectedExe.fileName,
    module: imported.module,
    symbol: imported.symbol,
    iatRva: imported.iatRva,
    iatAddress,
    thunkAddress,
  };
  assert(!ownedServices.has(thunkAddress), "duplicate owned thunk address");
  ownedServices.set(thunkAddress, service);
  return service;
});
assert.equal(resolvedImports.length, 13);
assert.equal(ownedServices.size, 13);
const mainModule = Object.freeze({
  name: expectedExe.fileName,
  baseAddress: pe.imageBase,
  imageSize: pe.sizeOfImage,
  sha256: sourceHash,
});
const paintLifecycle = Object.freeze({
  none: "none",
  queued: "queued",
  inFlight: "in-flight",
});
const mainGuestThread = Object.seal({
  id: 1,
  messageState: Object.seal({
    generatedPaintLane: [],
    inFlightPaint: null,
    nextSequence: 1,
  }),
});
const moduleIdentity = (module) =>
  `${module.sha256}@${module.baseAddress.toString(16)}`;
const logicalClassesByKey = new Map();
const logicalClassesByAtom = new Map();
let nextClassAtom = 0xc000;
const localClassKey = (module, className) =>
  `${moduleIdentity(module)}:local:A:${className}`;
const logicalWindowsByHandle = new Map();
const windowHandleTag = 0x57;
let nextWindowSlot = 1;
const encodeWindowHandle = (generation, slot) =>
  ((windowHandleTag << 24) | (generation << 16) | slot) >>> 0;
const allocateLogicalWindow = (fields) => {
  assert(nextWindowSlot > 0 && nextWindowSlot <= 0xffff, "window slot space exhausted");
  const slot = nextWindowSlot;
  const generation = 1;
  const handle = encodeWindowHandle(generation, slot);
  assert.notEqual(handle, 0);
  assert(!logicalWindowsByHandle.has(handle), "duplicate logical HWND");
  const window = Object.seal({
    ...fields,
    handle,
    type: "window",
    typeTag: windowHandleTag,
    generation,
    slot,
    lifecycle: "creating",
    visible: false,
  });
  logicalWindowsByHandle.set(handle, window);
  nextWindowSlot += 1;
  return window;
};
const lookupLogicalWindow = (handle) => {
  assert(Number.isInteger(handle) && handle > 0, "logical HWND is invalid");
  const normalizedHandle = handle >>> 0;
  const window = logicalWindowsByHandle.get(normalizedHandle);
  assert(window, `unknown logical HWND 0x${normalizedHandle.toString(16)}`);
  assert.equal(window.type, "window");
  assert(
    window.lifecycle === "creating" || window.lifecycle === "created",
    "logical HWND is not live",
  );
  assert.equal(normalizedHandle >>> 24, windowHandleTag);
  assert.equal((normalizedHandle >>> 16) & 0xff, window.generation);
  assert.equal(normalizedHandle & 0xffff, window.slot);
  return window;
};
const maxGeneratedPaintCandidates = 16;
const enqueueGeneratedPaint = (window) => {
  assert.equal(window.ownerThreadId, mainGuestThread.id);
  assert(window.updateRegion, "generated paint requires an update region");
  const lane = mainGuestThread.messageState.generatedPaintLane;
  const existing = lane.filter(
    ({ kind, hwnd }) => kind === "generated-paint" && hwnd === window.handle,
  );
  if (window.paintState === paintLifecycle.queued) {
    assert.equal(existing.length, 1, "queued paint requires one generated candidate");
    assert.equal(existing[0].sequence, window.paintSequence);
    return {
      candidate: existing[0],
      sequence: window.paintSequence,
      enqueued: false,
    };
  }
  if (window.paintState === paintLifecycle.inFlight) {
    assert.equal(existing.length, 0, "in-flight paint still has a queued candidate");
    assert(Number.isInteger(window.paintSequence) && window.paintSequence > 0);
    return {
      candidate: null,
      sequence: window.paintSequence,
      enqueued: false,
    };
  }
  assert.equal(window.paintState, paintLifecycle.none);
  assert.equal(window.paintSequence, null);
  assert.equal(existing.length, 0, "unowned generated paint candidate");
  assert(
    lane.length < maxGeneratedPaintCandidates,
    "generated paint lane limit exceeded",
  );
  const candidate = Object.freeze({
    sequence: mainGuestThread.messageState.nextSequence,
    kind: "generated-paint",
    source: "window-update-region",
    ownerThreadId: mainGuestThread.id,
    hwnd: window.handle,
    message: 0x000f,
    wParam: 0,
    lParam: 0,
  });
  mainGuestThread.messageState.nextSequence += 1;
  lane.push(candidate);
  window.paintState = paintLifecycle.queued;
  window.paintSequence = candidate.sequence;
  return { candidate, sequence: candidate.sequence, enqueued: true };
};
for (const service of resolvedImports) {
  assert.equal(
    readGuestU32(service.iatAddress, `${service.symbol} patched IAT`),
    service.thunkAddress,
  );
  assert.deepEqual(
    memory.slice(service.thunkAddress, service.thunkAddress + 2),
    Uint8Array.from([0x0f, 0x0b]),
  );
}
for (const { terminatorRva } of pe.descriptors) {
  assert.equal(
    readGuestU32(pe.imageBase + terminatorRva, "patched IAT terminator"),
    0,
  );
}
assertTrapPageRange(guest.entryReturn, 2, "entry return sentinel");
assert(!ownedServices.has(guest.entryReturn), "entry return overlaps an import thunk");
memory.set(Uint8Array.from([0x0f, 0x0b]), guest.entryReturn);
assertTrapPageRange(guest.callbackReturn, 2, "callback return sentinel");
assert(!ownedServices.has(guest.callbackReturn), "callback return overlaps an import thunk");
assert.notEqual(guest.callbackReturn, guest.entryReturn);
memory.set(Uint8Array.from([0x0f, 0x0b]), guest.callbackReturn);

const cpl = 3;
for (let index = 0; index < 8; index += 1) {
  privateState.setUint8(privateStateOffset.segmentIsNull + index, 0);
  privateState.setUint32(privateStateOffset.segmentOffsets + index * 4, 0, true);
  privateState.setUint32(privateStateOffset.segmentLimits + index * 4, 0xffffffff, true);
  privateState.setUint8(
    privateStateOffset.segmentAccessBytes + index,
    0x80 | (cpl << 5) | 0x10 | 0x02,
  );
}
privateState.setUint8(
  privateStateOffset.segmentAccessBytes + 1,
  0x80 | (cpl << 5) | 0x10 | 0x08 | 0x02,
);
privateState.setUint8(privateStateOffset.protectedMode, 1);
privateState.setUint8(privateStateOffset.operandSize32, 1);
privateState.setUint8(privateStateOffset.stackSize32, 1);
privateState.setUint8(privateStateOffset.cpl, cpl);
privateState.setUint8(privateStateOffset.inHalt, 0);
privateState.setUint32(privateStateOffset.instructionPointer, expectedExe.entry, true);
privateState.setUint32(
  privateStateOffset.previousInstructionPointer,
  expectedExe.entry,
  true,
);
privateState.setInt32(privateStateOffset.lastVirtualInstructionPointer, -1, true);
privateState.setUint32(privateStateOffset.flags, 2, true);
const registerOffset = (index) => privateStateOffset.registers + index * 4;
const initialEsp = guest.stackTop - 4;
privateState.setUint32(registerOffset(3), calleeSaved.ebx, true);
privateState.setUint32(registerOffset(4), initialEsp, true);
privateState.setUint32(registerOffset(5), calleeSaved.ebp, true);
privateState.setUint32(registerOffset(6), calleeSaved.esi, true);
privateState.setUint32(registerOffset(7), calleeSaved.edi, true);
writeGuestU32(initialEsp, guest.entryReturn, "entry return sentinel");
const cr0 = privateState.getUint32(privateStateOffset.controlRegisters, true);
privateState.setUint32(
  privateStateOffset.controlRegisters,
  (cr0 | 1) & ~0x80000000,
  true,
);
const cr4Offset = privateStateOffset.controlRegisters + 4 * 4;
privateState.setUint32(
  cr4Offset,
  privateState.getUint32(cr4Offset, true) | 0x200,
  true,
);
wasm.update_state_flags();
wasm.full_clear_tlb();

assert.equal(wasm.vine_cpu_state_version(), 1);
assert.equal(wasm.vine_cpu_state_size(), 640);
const saveState = (reason) => {
  wasm.vine_cpu_state_save();
  const words = new Uint32Array(
    wasm.memory.buffer,
    wasm.vine_cpu_state_ptr(),
    wasm.vine_cpu_state_size() / 4,
  );
  assert.equal(words[savedStateWord.magic], 0x56494e45);
  assert.equal(words[savedStateWord.version], 1);
  assert.equal(words[savedStateWord.wordCount], 160);
  return {
    reason,
    errorCode: wasm.vine_get_stop_error_code(),
    instructionCount: privateState.getUint32(
      privateStateOffset.instructionCounter,
      true,
    ),
    words,
    eax: words[savedStateWord.eax],
    esp: words[savedStateWord.esp],
    eip: words[savedStateWord.instructionPointer],
    previousEip: words[savedStateWord.previousInstructionPointer],
  };
};
const resume = (state, { eax = state.eax, esp, eip }) => {
  const preserved = new Map(
    [
      savedStateWord.ebx,
      savedStateWord.ebp,
      savedStateWord.esi,
      savedStateWord.edi,
    ].map((index) => [index, state.words[index]]),
  );
  state.words[savedStateWord.eax] = eax >>> 0;
  state.words[savedStateWord.esp] = esp >>> 0;
  state.words[savedStateWord.instructionPointer] = eip >>> 0;
  state.words[savedStateWord.previousInstructionPointer] = eip >>> 0;
  assert.equal(wasm.vine_cpu_state_restore(), 0);
  wasm.vine_cpu_state_save();
  const restoredWords = new Uint32Array(
    wasm.memory.buffer,
    wasm.vine_cpu_state_ptr(),
    wasm.vine_cpu_state_size() / 4,
  );
  assert.equal(restoredWords[savedStateWord.eax], eax >>> 0);
  assert.equal(restoredWords[savedStateWord.esp], esp >>> 0);
  assert.equal(restoredWords[savedStateWord.instructionPointer], eip >>> 0);
  assert.equal(restoredWords[savedStateWord.previousInstructionPointer], eip >>> 0);
  for (const [index, value] of preserved) {
    assert.equal(restoredWords[index], value);
  }
};
const assertInvariantRegisters = (words) => {
  assert.equal(words[savedStateWord.ebx], calleeSaved.ebx);
  assert.equal(words[savedStateWord.ebp], calleeSaved.ebp);
};
const executableImageSectionForRange = (address, size) => {
  assertGuestRange(address, size, "executable image range");
  const end = address + size;
  return pe.sections.find(
    ({ virtualAddress, virtualSize, characteristics }) =>
      (characteristics & 0x20000000) !== 0 &&
      address >= pe.imageBase + virtualAddress &&
      end <= pe.imageBase + virtualAddress + virtualSize,
  );
};
const readGuestAsciiZ = (address, maxBytes, label) => {
  assert(address > 0xffff, `${label} integer-atom form is unsupported`);
  assert(Number.isInteger(maxBytes) && maxBytes > 0, `${label} bound is invalid`);
  const bytes = [];
  for (let index = 0; index < maxBytes; index += 1) {
    assertGuestRange(address + index, 1, label);
    const value = memory[address + index];
    if (value === 0) {
      assert(bytes.length > 0, `${label} is empty`);
      return Buffer.from(bytes).toString("ascii");
    }
    assert(value <= 0x7f, `${label} contains non-ASCII data`);
    bytes.push(value);
  }
  assert.fail(`${label} is not NUL-terminated within ${maxBytes} bytes`);
};
const writeGuestWords = (address, words, label) => {
  assertGuestRange(address, words.length * 4, label);
  for (const [index, value] of words.entries()) {
    writeGuestU32(address + index * 4, value, `${label} word ${index}`);
  }
};
const resolveGuestCallback = (callback) => {
  assert.equal(callback.moduleIdentity, moduleIdentity(mainModule));
  assert.equal(callback.kind, "window-procedure-a");
  assert.equal(callback.callingConvention, "stdcall");
  assert.equal(callback.argumentBytes, 16);
  const address = mainModule.baseAddress + callback.entryRva;
  assert(Number.isSafeInteger(address) && address <= 0xffffffff);
  assert(executableImageSectionForRange(address, 1), "guest callback is no longer executable");
  return address >>> 0;
};
const decodeImportCall = (state, service) => {
  const returnAddress = readGuestU32(state.esp, "host service return address");
  assert.equal(
    readGuestU32(service.iatAddress, "guest import call resolved IAT"),
    service.thunkAddress,
  );
  const absoluteCallSize = 6;
  const absoluteCallSite = returnAddress - absoluteCallSize;
  assertGuestRange(absoluteCallSite, absoluteCallSize, "absolute guest import call");
  if (
    memory[absoluteCallSite] === 0xff &&
    memory[absoluteCallSite + 1] === 0x15
  ) {
    assert(
      executableImageSectionForRange(absoluteCallSite, absoluteCallSize),
      "absolute guest import call is outside an executable image section",
    );
    assert.equal(
      readGuestU32(absoluteCallSite + 2, "guest import call IAT operand"),
      service.iatAddress,
    );
    return {
      callSite: absoluteCallSite,
      returnAddress,
      callEncoding: "iat-indirect",
    };
  }

  const registerCallSize = 2;
  const registerCallSite = returnAddress - registerCallSize;
  assertGuestRange(registerCallSite, registerCallSize, "register guest import call");
  assert(
    executableImageSectionForRange(registerCallSite, registerCallSize),
    "register guest import call is outside an executable image section",
  );
  assert.deepEqual(
    memory.slice(registerCallSite, returnAddress),
    Uint8Array.from([0xff, 0xd6]),
    "fixed register import call is not CALL ESI",
  );
  assert.equal(state.words[savedStateWord.esi], service.thunkAddress);
  return {
    callSite: registerCallSite,
    returnAddress,
    callEncoding: "register-indirect-esi",
  };
};

const budgetEvents = [];
const implementedServiceEvents = [];
const callbackEvents = [];
const hostCallFrames = [];
const maxHostCallDepth = 2;
let nextHostCallId = 1;
const pushHostCallFrame = (frame) => {
  assert(hostCallFrames.length < maxHostCallDepth, "host call depth exceeded");
  const stored = Object.seal({ ...frame, id: nextHostCallId });
  nextHostCallId += 1;
  hostCallFrames.push(stored);
  return stored;
};
const peekHostCallFrame = () => hostCallFrames.at(-1) ?? null;
const popHostCallFrame = (kind) => {
  const frame = hostCallFrames.pop();
  assert(frame, "host call stack underflow");
  assert.equal(frame.kind, kind);
  return frame;
};
let diagnostic = null;
let finalState = null;
let executeStopCount = 0;
for (let stopCount = 0; stopCount < 64 && diagnostic === null; stopCount += 1) {
  const reason = wasm.vine_execute_budget(guest.instructionBudget);
  executeStopCount += 1;
  const state = saveState(reason);
  assertInvariantRegisters(state.words);

  if (reason === stopReason.budget) {
    budgetEvents.push({
      eip: state.eip,
      esp: state.esp,
      esi: state.words[savedStateWord.esi],
    });
    continue;
  }

  assert.equal(
    reason,
    stopReason.invalidOpcode,
    `unexpected stop reason 0x${reason.toString(16)} at 0x${state.eip.toString(16)}`,
  );
  assert.equal(state.errorCode, -1);
  assert.equal(state.previousEip, state.eip);

  if (state.eip === guest.callbackReturn) {
    const callbackFrame = peekHostCallFrame();
    assert(callbackFrame);
    assert.equal(callbackFrame.kind, "guest-callback");
    assert.equal(callbackFrame.returnSentinel, guest.callbackReturn);
    assert.equal(state.esp, callbackFrame.expectedReturnEsp);
    assert.equal(state.eax, 0, "fixed WM_CREATE callback rejected window creation");
    assert.equal(state.words[savedStateWord.esi], mainModule.baseAddress);
    const apiFrame = hostCallFrames.at(-2);
    assert(apiFrame);
    assert.equal(apiFrame.kind, "api");
    assert.equal(apiFrame.id, callbackFrame.parentId);
    assert.equal(callbackFrame.expectedReturnEsp, apiFrame.outerEsp);
    assert.equal(apiFrame.phase, "callback");
    assert.equal(apiFrame.service.symbol, "CreateWindowExA");
    assert.equal(
      readGuestU32(callbackFrame.entryEsp, "WndProc callback return sentinel"),
      callbackFrame.returnSentinel,
    );
    assert.deepEqual(
      Array.from({ length: callbackFrame.argumentBytes / 4 }, (_, index) =>
        readGuestU32(
          callbackFrame.entryEsp + 4 + index * 4,
          `WndProc callback argument ${index}`,
        ),
      ),
      callbackFrame.arguments,
    );
    assert.equal(
      readGuestU32(apiFrame.outerEsp, "pending CreateWindowExA return address"),
      apiFrame.returnAddress,
    );
    assert.deepEqual(
      Array.from({ length: apiFrame.argumentBytes / 4 }, (_, index) =>
        readGuestU32(
          apiFrame.outerEsp + 4 + index * 4,
          `pending CreateWindowExA argument ${index}`,
        ),
      ),
      apiFrame.arguments,
    );
    assert.equal(lookupLogicalWindow(apiFrame.window.handle), apiFrame.window);
    assert.equal(apiFrame.window.lifecycle, "creating");
    assert.equal(readGuestU32(0x0040305c, "WM_CREATE count"), 1);
    assert.equal(readGuestU32(0x00403068, "window event count"), 1);
    assert.equal(readGuestU32(0x0040306c, "first window event"), 1);
    assert.equal(readGuestU32(0x00403058, "window failure code"), 0);
    assert.deepEqual(
      Array.from({ length: 12 }, (_, index) =>
        readGuestU32(
          apiFrame.createStructAddress + index * 4,
          `CREATESTRUCTA word ${index}`,
        ),
      ),
      expectedCreateWindow.createStructWords,
    );
    assert.equal(popHostCallFrame("guest-callback"), callbackFrame);
    apiFrame.window.lifecycle = "created";
    apiFrame.phase = "returning";
    callbackEvents.push({
      kind: "guest-callback-return",
      callId: callbackFrame.id,
      parentCallId: apiFrame.id,
      role: callbackFrame.role,
      target: callbackFrame.target,
      result: state.eax,
      stackPointer: state.esp,
      instructionCount: state.instructionCount,
    });
    const completedApiFrame = popHostCallFrame("api");
    assert.equal(completedApiFrame, apiFrame);
    implementedServiceEvents.push({
      kind: "host-service-return",
      callId: apiFrame.id,
      requester: apiFrame.service.requester,
      module: apiFrame.service.module,
      symbol: apiFrame.service.symbol,
      variant: expectedCreateWindow.variant,
      implementationStatus: "partial",
      guestCallSite: apiFrame.callSite,
      guestCallSiteRva: apiFrame.callSite - pe.imageBase,
      guestReturnAddress: apiFrame.returnAddress,
      guestReturnRva: apiFrame.returnAddress - pe.imageBase,
      iatAddress: apiFrame.service.iatAddress,
      iatRva: apiFrame.service.iatRva,
      thunkAddress: apiFrame.service.thunkAddress,
      callingConvention: "stdcall",
      argumentBytes: apiFrame.argumentBytes,
      arguments: [...apiFrame.arguments],
      result: apiFrame.window.handle,
      className: apiFrame.className,
      windowName: apiFrame.windowName,
      callbackCallId: callbackFrame.id,
      callbackMessage: 1,
      callbackResult: state.eax,
      stackPointerBefore: apiFrame.outerEsp,
      stackPointerAfter: apiFrame.resumeEsp,
    });
    resume(state, {
      eax: apiFrame.window.handle,
      esp: apiFrame.resumeEsp,
      eip: apiFrame.returnAddress,
    });
    continue;
  }

  assert.notEqual(state.eip, guest.entryReturn, "fixture returned before host dispatch");
  const service = ownedServices.get(state.eip);
  assert(service, `#UD at unowned guest address 0x${state.eip.toString(16)}`);

  const { callSite, returnAddress, callEncoding } = decodeImportCall(state, service);

  if (hostCallFrames.length !== 0) {
    const callbackFrame = peekHostCallFrame();
    const apiFrame = hostCallFrames.find(({ kind }) => kind === "api");
    const isExpectedBeginPaint =
      service.module === "user32.dll" &&
      service.symbol === "BeginPaint" &&
      callbackFrame?.role === "window-procedure-wm-paint" &&
      apiFrame?.service.symbol === "DispatchMessageA";
    if (isExpectedBeginPaint) {
      const window = lookupLogicalWindow(expectedCreateWindow.logicalHandle);
      assert.equal(apiFrame.id, 3);
      assert.equal(callbackFrame.id, 4);
      assert.equal(callbackFrame.parentId, apiFrame.id);
      assert.equal(apiFrame.window, window);
      assert.equal(service.iatAddress, expectedBeginPaint.iatAddress);
      assert.equal(service.thunkAddress, expectedBeginPaint.thunkAddress);
      assert.equal(callSite, expectedBeginPaint.callSite);
      assert.equal(returnAddress, expectedBeginPaint.returnAddress);
      assert.equal(callEncoding, "iat-indirect");
      assert.equal(state.esp, expectedBeginPaint.stackPointer);
      assert.equal(state.eax, 1);
      assert.equal(state.words[savedStateWord.esi], window.handle);
      assert.equal(
        state.words[savedStateWord.edi],
        expectedGetMessage.thunkAddress,
      );
      const beginPaintArguments = Array.from({ length: 2 }, (_, index) =>
        readGuestU32(state.esp + 4 + index * 4, `BeginPaint argument ${index}`),
      );
      assert.deepEqual(beginPaintArguments, expectedBeginPaint.arguments);
      assert.deepEqual(window.updateRegion, { kind: "whole-client" });
      assert.equal(window.erasePending, false);
      assert.equal(window.paintState, paintLifecycle.inFlight);
      assert.equal(window.paintSequence, 1);
      assert.equal(mainGuestThread.messageState.generatedPaintLane.length, 0);
      assert(mainGuestThread.messageState.inFlightPaint);
      diagnostic = {
        kind: "unsupported-reentrant-host-service",
        parentCallId: callbackFrame.id,
        rootCallId: apiFrame.id,
        rootModule: apiFrame.service.module,
        rootSymbol: apiFrame.service.symbol,
        rootVariant: apiFrame.variant,
        callbackRole: callbackFrame.role,
        callbackTarget: callbackFrame.target,
        requester: service.requester,
        module: service.module,
        symbol: service.symbol,
        guestCallSite: callSite,
        guestCallSiteRva: callSite - pe.imageBase,
        guestReturnAddress: returnAddress,
        guestReturnRva: returnAddress - pe.imageBase,
        iatAddress: service.iatAddress,
        iatRva: service.iatRva,
        thunkAddress: service.thunkAddress,
        callEncoding,
        engineStopReason: state.reason,
        engineErrorCode: state.errorCode,
        stackPointer: state.esp,
      };
      finalState = state;
      continue;
    }
    if (
      apiFrame?.service.symbol === "CreateWindowExA" &&
      apiFrame.window?.lifecycle === "creating"
    ) {
      apiFrame.window.lifecycle = "create-failed";
      logicalWindowsByHandle.delete(apiFrame.window.handle);
    }
    hostCallFrames.length = 0;
    diagnostic = {
      kind: "unsupported-reentrant-host-service",
      parentCallId: callbackFrame?.parentId ?? null,
      requester: service.requester,
      module: service.module,
      symbol: service.symbol,
      guestCallSite: callSite,
      guestReturnAddress: returnAddress,
      callEncoding,
      stackPointer: state.esp,
    };
    finalState = state;
    continue;
  }

  const isGetModuleHandleA =
    service.module === "kernel32.dll" && service.symbol === "GetModuleHandleA";
  const moduleName = isGetModuleHandleA
    ? readGuestU32(state.esp + 4, "GetModuleHandleA module name")
    : null;
  if (isGetModuleHandleA && moduleName === 0) {
    assert.equal(state.esp, 0x006fffec);
    assert.equal(implementedServiceEvents.length, 0, "GetModuleHandleA repeated");
    assert.equal(state.words[savedStateWord.esi], calleeSaved.esi);
    const argumentBytes = 4;
    const resumeEsp = state.esp + 4 + argumentBytes;
    assert.equal(readGuestU32(resumeEsp, "saved entry ESI"), calleeSaved.esi);
    implementedServiceEvents.push({
      kind: "host-service-return",
      requester: service.requester,
      module: service.module,
      symbol: service.symbol,
      variant: "module-name-null",
      implementationStatus: "partial",
      guestCallSite: callSite,
      guestCallSiteRva: callSite - pe.imageBase,
      guestReturnAddress: returnAddress,
      guestReturnRva: returnAddress - pe.imageBase,
      iatAddress: service.iatAddress,
      iatRva: service.iatRva,
      thunkAddress: service.thunkAddress,
      callingConvention: "stdcall",
      argumentBytes,
      arguments: [moduleName],
      result: mainModule.baseAddress,
      stackPointerBefore: state.esp,
      stackPointerAfter: resumeEsp,
    });
    resume(state, {
      eax: mainModule.baseAddress,
      esp: resumeEsp,
      eip: returnAddress,
    });
    continue;
  }

  const isRegisterClassExA =
    service.module === "user32.dll" && service.symbol === "RegisterClassExA";
  if (isRegisterClassExA && implementedServiceEvents.length === 1) {
    assert.equal(state.esp, 0x006fffec);
    assert.equal(state.eax, mainModule.baseAddress);
    assert.equal(state.words[savedStateWord.esi], mainModule.baseAddress);
    const classAddress = readGuestU32(state.esp + 4, "RegisterClassExA class pointer");
    assert.equal(classAddress, expectedWindowClass.address);
    assertGuestRange(classAddress, 48, "WNDCLASSEXA");
    const classWords = Array.from({ length: 12 }, (_, index) =>
      readGuestU32(classAddress + index * 4, `WNDCLASSEXA word ${index}`),
    );
    assert.deepEqual(classWords, expectedWindowClass.words);
    const [
      cbSize,
      style,
      wndProc,
      classExtraBytes,
      windowExtraBytes,
      instance,
      icon,
      cursor,
      background,
      menuName,
      classNameAddress,
      smallIcon,
    ] = classWords;
    assert.equal(cbSize, 48);
    assert(executableImageSectionForRange(wndProc, 1), "WndProc is not executable");
    assert.equal(instance, mainModule.baseAddress);
    assert.deepEqual(
      [style, classExtraBytes, windowExtraBytes, icon, cursor, background, menuName, smallIcon],
      [0, 0, 0, 0, 0, 0, 0, 0],
    );
    assert.equal(classNameAddress, expectedWindowClass.nameAddress);
    const className = readGuestAsciiZ(classNameAddress, 64, "window class name");
    assert.equal(className, expectedWindowClass.name);
    const wndProcCallback = Object.freeze({
      kind: "window-procedure-a",
      moduleIdentity: moduleIdentity(mainModule),
      entryRva: wndProc - mainModule.baseAddress,
      callingConvention: "stdcall",
      argumentBytes: 16,
    });
    assert.equal(resolveGuestCallback(wndProcCallback), wndProc);
    const key = localClassKey(mainModule, className);
    assert(!logicalClassesByKey.has(key), "duplicate local window class");
    assert(nextClassAtom >= 0xc000 && nextClassAtom <= 0xffff);
    const atom = nextClassAtom;
    assert((atom & 0xffff) !== 0);
    assert(!logicalClassesByAtom.has(atom), "duplicate logical class atom");
    const logicalClass = Object.freeze({
      key,
      atom,
      scope: "local",
      encoding: "A-ascii",
      name: className,
      moduleIdentity: moduleIdentity(mainModule),
      wndProcCallback,
      style,
      classExtraBytes,
      windowExtraBytes,
    });
    logicalClassesByKey.set(key, logicalClass);
    logicalClassesByAtom.set(atom, logicalClass);
    nextClassAtom += 1;
    const argumentBytes = 4;
    const resumeEsp = state.esp + 4 + argumentBytes;
    implementedServiceEvents.push({
      kind: "host-service-return",
      requester: service.requester,
      module: service.module,
      symbol: service.symbol,
      variant: "wndclassexa-v48-local-ascii",
      implementationStatus: "partial",
      guestCallSite: callSite,
      guestCallSiteRva: callSite - pe.imageBase,
      guestReturnAddress: returnAddress,
      guestReturnRva: returnAddress - pe.imageBase,
      iatAddress: service.iatAddress,
      iatRva: service.iatRva,
      thunkAddress: service.thunkAddress,
      callingConvention: "stdcall",
      argumentBytes,
      arguments: [classAddress],
      result: atom,
      className,
      wndProc,
      stackPointerBefore: state.esp,
      stackPointerAfter: resumeEsp,
    });
    resume(state, { eax: atom, esp: resumeEsp, eip: returnAddress });
    continue;
  }

  const isCreateWindowExA =
    service.module === "user32.dll" && service.symbol === "CreateWindowExA";
  if (isCreateWindowExA && implementedServiceEvents.length === 2) {
    assert.equal(state.esp, 0x006fffc0);
    assert.equal(state.eax, logicalClassesByKey.values().next().value.atom);
    assert.equal(state.words[savedStateWord.esi], mainModule.baseAddress);
    const createArguments = Array.from({ length: 12 }, (_, index) =>
      readGuestU32(state.esp + 4 + index * 4, `CreateWindowExA argument ${index}`),
    );
    assert.deepEqual(createArguments, expectedCreateWindow.arguments);
    const [
      extendedStyle,
      classNameAddress,
      windowNameAddress,
      style,
      x,
      y,
      width,
      height,
      parent,
      menu,
      instance,
      createParameter,
    ] = createArguments;
    assert.deepEqual(
      [extendedStyle, style, x, y, width, height, parent, menu, instance, createParameter],
      [
        0,
        0x00cf0000,
        0x80000000,
        0x80000000,
        160,
        120,
        0,
        0,
        mainModule.baseAddress,
        0,
      ],
    );
    assert(width > 0 && height > 0, "window dimensions must be positive");
    const className = readGuestAsciiZ(classNameAddress, 64, "CreateWindowExA class name");
    const windowName = readGuestAsciiZ(windowNameAddress, 64, "CreateWindowExA window name");
    assert.equal(className, expectedCreateWindow.className);
    assert.equal(windowName, expectedCreateWindow.windowName);
    const classKey = localClassKey(mainModule, className);
    const logicalClass = logicalClassesByKey.get(classKey);
    assert(logicalClass, "CreateWindowExA class is not registered");
    assert.equal(logicalClass.moduleIdentity, moduleIdentity(mainModule));
    assert.equal(logicalClassesByAtom.get(logicalClass.atom), logicalClass);
    const callbackTarget = resolveGuestCallback(logicalClass.wndProcCallback);
    const window = allocateLogicalWindow({
      moduleIdentity: moduleIdentity(mainModule),
      classKey,
      classAtom: logicalClass.atom,
      className,
      title: windowName,
      extendedStyle,
      style,
      requestedPosition: Object.freeze({ x: "use-default", y: "use-default" }),
      requestedOuterSize: Object.freeze({ width, height }),
      parent: null,
      menu: null,
      ownerThreadId: mainGuestThread.id,
      updateRegion: null,
      erasePending: false,
      paintState: paintLifecycle.none,
      paintSequence: null,
      showCommand: null,
    });
    assert.equal(window.handle, expectedCreateWindow.logicalHandle);
    const createStructAddress = guest.callbackScratch;
    // CREATESTRUCTA stores the CreateWindow arguments in Win32 structure order.
    const createStructWords = [
      createParameter,
      instance,
      menu,
      parent,
      height,
      width,
      y,
      x,
      style,
      windowNameAddress,
      classNameAddress,
      extendedStyle,
    ];
    assert.deepEqual(createStructWords, expectedCreateWindow.createStructWords);
    assert(
      createStructAddress >= guest.callbackScratch &&
        createStructAddress + createStructWords.length * 4 <=
          guest.callbackScratch + guest.callbackScratchSize,
      "CREATESTRUCTA exceeds callback scratch",
    );
    writeGuestWords(createStructAddress, createStructWords, "CREATESTRUCTA");
    const callbackArguments = [window.handle, 1, 0, createStructAddress];
    const callbackFrameBytes = 4 + logicalClass.wndProcCallback.argumentBytes;
    const callbackStackPointer = state.esp - callbackFrameBytes;
    assert.equal(callbackStackPointer, expectedCreateWindow.callbackStackPointer);
    assertGuestRange(callbackStackPointer, callbackFrameBytes, "WndProc callback frame");
    writeGuestU32(callbackStackPointer, guest.callbackReturn, "WndProc return sentinel");
    writeGuestWords(callbackStackPointer + 4, callbackArguments, "WndProc arguments");
    const argumentBytes = createArguments.length * 4;
    const apiFrame = pushHostCallFrame({
      kind: "api",
      service,
      phase: "callback",
      callSite,
      returnAddress,
      outerEsp: state.esp,
      argumentBytes,
      resumeEsp: state.esp + 4 + argumentBytes,
      arguments: Object.freeze([...createArguments]),
      className,
      windowName,
      createStructAddress,
      window,
    });
    const callbackFrame = pushHostCallFrame({
      parentId: apiFrame.id,
      kind: "guest-callback",
      role: "window-procedure-wm-create",
      target: callbackTarget,
      returnSentinel: guest.callbackReturn,
      entryEsp: callbackStackPointer,
      expectedReturnEsp: callbackStackPointer + callbackFrameBytes,
      argumentBytes: logicalClass.wndProcCallback.argumentBytes,
      arguments: Object.freeze([...callbackArguments]),
    });
    assert.equal(callbackFrame.expectedReturnEsp, expectedCreateWindow.callbackReturnStackPointer);
    callbackEvents.push({
      kind: "guest-callback-enter",
      callId: callbackFrame.id,
      parentCallId: apiFrame.id,
      role: callbackFrame.role,
      target: callbackTarget,
      returnSentinel: guest.callbackReturn,
      arguments: [...callbackArguments],
      stackPointer: callbackStackPointer,
    });
    resume(state, { esp: callbackStackPointer, eip: callbackTarget });
    continue;
  }

  const isShowWindow =
    service.module === "user32.dll" && service.symbol === "ShowWindow";
  if (isShowWindow && implementedServiceEvents.length === 3) {
    const window = lookupLogicalWindow(expectedCreateWindow.logicalHandle);
    assert.equal(hostCallFrames.length, 0);
    assert.equal(state.esp, expectedShowWindow.stackPointer);
    assert.equal(state.eax, window.handle);
    assert.equal(state.words[savedStateWord.esi], window.handle);
    assert.equal(window.lifecycle, "created");
    assert.equal(window.visible, false);
    assert.equal(window.showCommand, null);
    const showArguments = Array.from({ length: 2 }, (_, index) =>
      readGuestU32(state.esp + 4 + index * 4, `ShowWindow argument ${index}`),
    );
    assert.deepEqual(showArguments, expectedShowWindow.arguments);
    const [handle, command] = showArguments;
    assert.equal(lookupLogicalWindow(handle), window);
    assert.equal(command, 5);
    const wasVisible = window.visible;
    window.visible = true;
    window.showCommand = command;
    const argumentBytes = showArguments.length * 4;
    const resumeEsp = state.esp + 4 + argumentBytes;
    assert.equal(resumeEsp, 0x006ffff4);
    assert.equal(readGuestU32(resumeEsp, "saved entry ESI"), calleeSaved.esi);
    implementedServiceEvents.push({
      kind: "host-service-return",
      requester: service.requester,
      module: service.module,
      symbol: service.symbol,
      variant: expectedShowWindow.variant,
      implementationStatus: "partial",
      guestCallSite: callSite,
      guestCallSiteRva: callSite - pe.imageBase,
      guestReturnAddress: returnAddress,
      guestReturnRva: returnAddress - pe.imageBase,
      iatAddress: service.iatAddress,
      iatRva: service.iatRva,
      thunkAddress: service.thunkAddress,
      callingConvention: "stdcall",
      argumentBytes,
      arguments: [...showArguments],
      result: wasVisible ? 1 : 0,
      visibilityBefore: wasVisible,
      visibilityAfter: window.visible,
      stackPointerBefore: state.esp,
      stackPointerAfter: resumeEsp,
    });
    resume(state, { eax: wasVisible ? 1 : 0, esp: resumeEsp, eip: returnAddress });
    continue;
  }

  const isInvalidateRect =
    service.module === "user32.dll" && service.symbol === "InvalidateRect";
  if (isInvalidateRect && implementedServiceEvents.length === 4) {
    const window = lookupLogicalWindow(expectedCreateWindow.logicalHandle);
    assert.equal(hostCallFrames.length, 0);
    assert.equal(state.esp, expectedInvalidateRect.stackPointer);
    assert.equal(state.eax, 0);
    assert.equal(state.words[savedStateWord.esi], window.handle);
    assert.equal(window.lifecycle, "created");
    assert.equal(window.visible, true);
    assert.equal(window.showCommand, 5);
    assert.equal(window.updateRegion, null);
    assert.equal(window.erasePending, false);
    assert.equal(window.paintState, paintLifecycle.none);
    assert.equal(window.paintSequence, null);
    assert.equal(window.ownerThreadId, mainGuestThread.id);
    assert.equal(mainGuestThread.messageState.generatedPaintLane.length, 0);
    const invalidateArguments = Array.from({ length: 3 }, (_, index) =>
      readGuestU32(state.esp + 4 + index * 4, `InvalidateRect argument ${index}`),
    );
    assert.deepEqual(invalidateArguments, expectedInvalidateRect.arguments);
    const [handle, rectangle, erase] = invalidateArguments;
    assert.equal(lookupLogicalWindow(handle), window);
    assert.equal(rectangle, 0);
    assert.equal(erase, 0);
    window.updateRegion = Object.freeze({ kind: "whole-client" });
    window.erasePending = window.erasePending || erase !== 0;
    const queuedPaint = enqueueGeneratedPaint(window);
    assert.equal(queuedPaint.enqueued, true);
    assert(queuedPaint.candidate);
    const argumentBytes = invalidateArguments.length * 4;
    const resumeEsp = state.esp + 4 + argumentBytes;
    assert.equal(resumeEsp, 0x006ffff4);
    assert.equal(readGuestU32(resumeEsp, "saved entry ESI"), calleeSaved.esi);
    implementedServiceEvents.push({
      kind: "host-service-return",
      requester: service.requester,
      module: service.module,
      symbol: service.symbol,
      variant: expectedInvalidateRect.variant,
      implementationStatus: "partial",
      guestCallSite: callSite,
      guestCallSiteRva: callSite - pe.imageBase,
      guestReturnAddress: returnAddress,
      guestReturnRva: returnAddress - pe.imageBase,
      iatAddress: service.iatAddress,
      iatRva: service.iatRva,
      thunkAddress: service.thunkAddress,
      callingConvention: "stdcall",
      argumentBytes,
      arguments: [...invalidateArguments],
      result: 1,
      updateRegion: window.updateRegion.kind,
      eraseRequested: false,
      erasePendingAfter: window.erasePending,
      paintStateAfter: window.paintState,
      paintCandidateEnqueued: queuedPaint.enqueued,
      paintCandidateSequence: queuedPaint.sequence,
      queueOwnerThreadId: mainGuestThread.id,
      queueLane: "generated-paint",
      generatedPaintDepthAfter:
        mainGuestThread.messageState.generatedPaintLane.length,
      stackPointerBefore: state.esp,
      stackPointerAfter: resumeEsp,
    });
    resume(state, { eax: 1, esp: resumeEsp, eip: returnAddress });
    continue;
  }

  const isGetMessageA =
    service.module === "user32.dll" && service.symbol === "GetMessageA";
  if (isGetMessageA && implementedServiceEvents.length === 5) {
    const window = lookupLogicalWindow(expectedCreateWindow.logicalHandle);
    assert.equal(hostCallFrames.length, 0);
    assert.equal(service.iatAddress, expectedGetMessage.iatAddress);
    assert.equal(service.thunkAddress, expectedGetMessage.thunkAddress);
    assert.equal(callSite, expectedGetMessage.callSite);
    assert.equal(returnAddress, expectedGetMessage.returnAddress);
    assert.equal(callEncoding, "iat-indirect");
    assert.equal(state.esp, expectedGetMessage.stackPointer);
    assert.equal(state.eax, 1);
    assert.equal(state.words[savedStateWord.esi], window.handle);
    assert.equal(state.words[savedStateWord.edi], calleeSaved.edi);
    assert.deepEqual(window.updateRegion, { kind: "whole-client" });
    assert.equal(window.erasePending, false);
    assert.equal(window.paintState, paintLifecycle.queued);
    assert.equal(window.paintSequence, 1);
    assert.equal(window.ownerThreadId, mainGuestThread.id);
    assert.equal(mainGuestThread.messageState.inFlightPaint, null);
    const lane = mainGuestThread.messageState.generatedPaintLane;
    assert.equal(lane.length, 1);
    const [candidate] = lane;
    assert(Object.isFrozen(candidate));
    assert.deepEqual(candidate, {
      sequence: 1,
      kind: "generated-paint",
      source: "window-update-region",
      ownerThreadId: mainGuestThread.id,
      hwnd: window.handle,
      message: 0x000f,
      wParam: 0,
      lParam: 0,
    });
    const getMessageArguments = Array.from({ length: 4 }, (_, index) =>
      readGuestU32(state.esp + 4 + index * 4, `GetMessageA argument ${index}`),
    );
    assert.deepEqual(getMessageArguments, expectedGetMessage.arguments);
    const [messageAddress, windowFilter, minimumMessage, maximumMessage] =
      getMessageArguments;
    assert.equal(windowFilter, 0);
    assert.equal(minimumMessage, 0);
    assert.equal(maximumMessage, 0);
    assertGuestRange(messageAddress, 28, "GetMessageA MSG output");
    const materializedMessageWords = Object.freeze([
      candidate.hwnd,
      candidate.message,
      candidate.wParam,
      candidate.lParam,
      0,
      0,
      0,
    ]);
    assert.deepEqual(materializedMessageWords, expectedGetMessage.messageWords);
    writeGuestWords(messageAddress, materializedMessageWords, "GetMessageA MSG");
    assert.equal(lane.shift(), candidate);
    const inFlightPaint = Object.freeze({
      ...candidate,
      time: 0,
      point: Object.freeze({ x: 0, y: 0 }),
    });
    mainGuestThread.messageState.inFlightPaint = inFlightPaint;
    window.paintState = paintLifecycle.inFlight;
    assert.equal(lane.length, 0);
    const argumentBytes = getMessageArguments.length * 4;
    const resumeEsp = state.esp + 4 + argumentBytes;
    assert.equal(resumeEsp, 0x006ffff4);
    implementedServiceEvents.push({
      kind: "host-service-return",
      requester: service.requester,
      module: service.module,
      symbol: service.symbol,
      variant: expectedGetMessage.variant,
      implementationStatus: "partial",
      guestCallSite: callSite,
      guestCallSiteRva: callSite - pe.imageBase,
      guestReturnAddress: returnAddress,
      guestReturnRva: returnAddress - pe.imageBase,
      iatAddress: service.iatAddress,
      iatRva: service.iatRva,
      thunkAddress: service.thunkAddress,
      callEncoding,
      callingConvention: "stdcall",
      argumentBytes,
      arguments: [...getMessageArguments],
      result: 1,
      materializedMessage: [...materializedMessageWords],
      messageTime: 0,
      messagePoint: { x: 0, y: 0 },
      paintStateBefore: paintLifecycle.queued,
      paintStateAfter: window.paintState,
      paintCandidateSequence: candidate.sequence,
      queueOwnerThreadId: mainGuestThread.id,
      generatedPaintDepthBefore: 1,
      generatedPaintDepthAfter: lane.length,
      stackPointerBefore: state.esp,
      stackPointerAfter: resumeEsp,
    });
    resume(state, { eax: 1, esp: resumeEsp, eip: returnAddress });
    continue;
  }

  const isDispatchMessageA =
    service.module === "user32.dll" && service.symbol === "DispatchMessageA";
  if (isDispatchMessageA && implementedServiceEvents.length === 6) {
    const window = lookupLogicalWindow(expectedCreateWindow.logicalHandle);
    const logicalClass = logicalClassesByAtom.get(window.classAtom);
    assert(logicalClass);
    assert.equal(hostCallFrames.length, 0);
    assert.equal(service.iatAddress, expectedDispatchMessage.iatAddress);
    assert.equal(service.thunkAddress, expectedDispatchMessage.thunkAddress);
    assert.equal(callSite, expectedDispatchMessage.callSite);
    assert.equal(returnAddress, expectedDispatchMessage.returnAddress);
    assert.equal(callEncoding, "register-indirect-esi");
    assert.equal(state.esp, expectedDispatchMessage.stackPointer);
    assert.equal(state.eax, 1);
    assert.equal(state.words[savedStateWord.esi], service.thunkAddress);
    assert.equal(
      state.words[savedStateWord.edi],
      expectedGetMessage.thunkAddress,
    );
    const dispatchArguments = [
      readGuestU32(state.esp + 4, "DispatchMessageA MSG pointer"),
    ];
    assert.deepEqual(dispatchArguments, expectedDispatchMessage.arguments);
    const [messageAddress] = dispatchArguments;
    const messageWords = Object.freeze(
      Array.from({ length: 7 }, (_, index) =>
        readGuestU32(messageAddress + index * 4, `DispatchMessageA MSG word ${index}`),
      ),
    );
    assert.deepEqual(messageWords, expectedGetMessage.messageWords);
    const inFlightPaint = mainGuestThread.messageState.inFlightPaint;
    assert(inFlightPaint);
    assert(Object.isFrozen(inFlightPaint));
    assert.equal(inFlightPaint.ownerThreadId, mainGuestThread.id);
    assert.equal(inFlightPaint.sequence, window.paintSequence);
    assert.deepEqual(
      [inFlightPaint.hwnd, inFlightPaint.message, inFlightPaint.wParam, inFlightPaint.lParam],
      messageWords.slice(0, 4),
    );
    assert.equal(window.ownerThreadId, mainGuestThread.id);
    assert.deepEqual(window.updateRegion, { kind: "whole-client" });
    assert.equal(window.erasePending, false);
    assert.equal(window.paintState, paintLifecycle.inFlight);
    assert.equal(window.paintSequence, 1);
    assert.equal(mainGuestThread.messageState.generatedPaintLane.length, 0);
    const callbackTarget = resolveGuestCallback(logicalClass.wndProcCallback);
    assert.equal(callbackTarget, expectedDispatchMessage.callbackTarget);
    const callbackArguments = messageWords.slice(0, 4);
    const callbackFrameBytes = 4 + logicalClass.wndProcCallback.argumentBytes;
    const callbackStackPointer = state.esp - callbackFrameBytes;
    assert.equal(callbackStackPointer, expectedDispatchMessage.callbackStackPointer);
    assertGuestRange(callbackStackPointer, callbackFrameBytes, "WM_PAINT callback frame");
    writeGuestU32(callbackStackPointer, guest.callbackReturn, "WndProc return sentinel");
    writeGuestWords(callbackStackPointer + 4, callbackArguments, "WM_PAINT arguments");
    const argumentBytes = dispatchArguments.length * 4;
    const apiFrame = pushHostCallFrame({
      kind: "api",
      service,
      variant: expectedDispatchMessage.variant,
      phase: "callback",
      callSite,
      callEncoding,
      returnAddress,
      outerEsp: state.esp,
      argumentBytes,
      resumeEsp: state.esp + 4 + argumentBytes,
      arguments: Object.freeze([...dispatchArguments]),
      message: messageWords,
      paintSequence: inFlightPaint.sequence,
      window,
    });
    assert.equal(apiFrame.id, 3);
    assert.equal(apiFrame.resumeEsp, 0x006ffff4);
    const callbackFrame = pushHostCallFrame({
      parentId: apiFrame.id,
      kind: "guest-callback",
      role: "window-procedure-wm-paint",
      target: callbackTarget,
      returnSentinel: guest.callbackReturn,
      entryEsp: callbackStackPointer,
      expectedReturnEsp: callbackStackPointer + callbackFrameBytes,
      argumentBytes: logicalClass.wndProcCallback.argumentBytes,
      arguments: Object.freeze([...callbackArguments]),
    });
    assert.equal(callbackFrame.id, 4);
    assert.equal(
      callbackFrame.expectedReturnEsp,
      expectedDispatchMessage.callbackReturnStackPointer,
    );
    callbackEvents.push({
      kind: "guest-callback-enter",
      callId: callbackFrame.id,
      parentCallId: apiFrame.id,
      role: callbackFrame.role,
      target: callbackTarget,
      returnSentinel: guest.callbackReturn,
      arguments: [...callbackArguments],
      stackPointer: callbackStackPointer,
      instructionCount: state.instructionCount,
    });
    resume(state, { esp: callbackStackPointer, eip: callbackTarget });
    continue;
  }

  diagnostic = {
    kind: "unsupported-host-service",
    requester: service.requester,
    module: service.module,
    symbol: service.symbol,
    guestCallSite: callSite,
    guestCallSiteRva: callSite - pe.imageBase,
    guestReturnAddress: returnAddress,
    guestReturnRva: returnAddress - pe.imageBase,
    iatAddress: service.iatAddress,
    iatRva: service.iatRva,
    thunkAddress: service.thunkAddress,
    engineStopReason: state.reason,
    engineErrorCode: state.errorCode,
    stackPointer: state.esp,
  };
  finalState = state;
}

assert(diagnostic, "runtime loader slice did not reach an owned host service thunk");
assert(finalState);
assert.equal(executeStopCount, 37);
assert.deepEqual(budgetEvents, [
  { eip: 0x00401004, esp: 0x006ffff0, esi: calleeSaved.esi },
  { eip: 0x00401014, esp: 0x006ffff4, esi: mainModule.baseAddress },
  { eip: 0x0040102d, esp: 0x006ffff4, esi: mainModule.baseAddress },
  { eip: 0x005000a0, esp: 0x006fffec, esi: mainModule.baseAddress },
  { eip: 0x0040104e, esp: 0x006fffc4, esi: mainModule.baseAddress },
  { eip: 0x0040105e, esp: 0x006fffc4, esi: mainModule.baseAddress },
  { eip: 0x00401076, esp: 0x006fffc4, esi: mainModule.baseAddress },
  { eip: 0x0040108e, esp: 0x006fffc4, esi: mainModule.baseAddress },
  { eip: 0x00401268, esp: 0x006fffa8, esi: mainModule.baseAddress },
  { eip: 0x0040126f, esp: 0x006fffa8, esi: mainModule.baseAddress },
  { eip: 0x0040127a, esp: 0x006fffa8, esi: mainModule.baseAddress },
  { eip: 0x00401288, esp: 0x006fffa8, esi: mainModule.baseAddress },
  { eip: 0x0040129e, esp: 0x006fffa8, esi: mainModule.baseAddress },
  { eip: 0x00401406, esp: 0x006fffac, esi: mainModule.baseAddress },
  {
    eip: 0x004010a5,
    esp: 0x006ffff4,
    esi: expectedCreateWindow.logicalHandle,
  },
  {
    eip: expectedShowWindow.thunkAddress,
    esp: expectedShowWindow.stackPointer,
    esi: expectedCreateWindow.logicalHandle,
  },
  {
    eip: expectedInvalidateRect.callSite,
    esp: expectedShowWindow.stackPointer,
    esi: expectedCreateWindow.logicalHandle,
  },
  {
    eip: 0x004010c3,
    esp: 0x006ffff0,
    esi: expectedCreateWindow.logicalHandle,
  },
  {
    eip: expectedGetMessage.callSite,
    esp: 0x006fffe4,
    esi: expectedCreateWindow.logicalHandle,
  },
  {
    eip: 0x004010dc,
    esp: 0x006ffff4,
    esi: expectedDispatchMessage.thunkAddress,
  },
  {
    eip: 0x0040110e,
    esp: 0x006ffff4,
    esi: expectedDispatchMessage.thunkAddress,
  },
  {
    eip: 0x0040111d,
    esp: 0x006ffff4,
    esi: expectedDispatchMessage.thunkAddress,
  },
  {
    eip: expectedDispatchMessage.thunkAddress,
    esp: expectedDispatchMessage.stackPointer,
    esi: expectedDispatchMessage.thunkAddress,
  },
  {
    eip: 0x00401268,
    esp: 0x006fffd4,
    esi: expectedDispatchMessage.thunkAddress,
  },
  {
    eip: 0x004012cf,
    esp: 0x006fffd4,
    esi: expectedCreateWindow.logicalHandle,
  },
  {
    eip: 0x004012d9,
    esp: 0x006fffd4,
    esi: expectedCreateWindow.logicalHandle,
  },
  {
    eip: 0x0040132b,
    esp: 0x006fffd4,
    esi: expectedCreateWindow.logicalHandle,
  },
  {
    eip: 0x0040133c,
    esp: 0x006fffd0,
    esi: expectedCreateWindow.logicalHandle,
  },
]);
assert.equal(budgetEvents.length, 28);
assert.equal(executeStopCount - budgetEvents.length, 9);
assert.equal(finalState.instructionCount, 101);
assert.deepEqual(implementedServiceEvents, [
  {
    kind: "host-service-return",
    requester: "window-pixel.exe",
    module: "kernel32.dll",
    symbol: "GetModuleHandleA",
    variant: "module-name-null",
    implementationStatus: "partial",
    guestCallSite: 0x00401004,
    guestCallSiteRva: 0x1004,
    guestReturnAddress: 0x0040100a,
    guestReturnRva: 0x100a,
    iatAddress: 0x004020c0,
    iatRva: 0x20c0,
    thunkAddress: 0x00500000,
    callingConvention: "stdcall",
    argumentBytes: 4,
    arguments: [0],
    result: 0x00400000,
    stackPointerBefore: 0x006fffec,
    stackPointerAfter: 0x006ffff4,
  },
  {
    kind: "host-service-return",
    requester: "window-pixel.exe",
    module: "user32.dll",
    symbol: "RegisterClassExA",
    variant: "wndclassexa-v48-local-ascii",
    implementationStatus: "partial",
    guestCallSite: 0x0040103c,
    guestCallSiteRva: 0x103c,
    guestReturnAddress: 0x00401042,
    guestReturnRva: 0x1042,
    iatAddress: 0x004020ec,
    iatRva: 0x20ec,
    thunkAddress: 0x005000a0,
    callingConvention: "stdcall",
    argumentBytes: 4,
    arguments: [0x00403004],
    result: 0x0000c000,
    className: "VineWindowPixelFixture",
    wndProc: 0x00401260,
    stackPointerBefore: 0x006fffec,
    stackPointerAfter: 0x006ffff4,
  },
  {
    kind: "host-service-return",
    callId: 1,
    requester: "window-pixel.exe",
    module: "user32.dll",
    symbol: "CreateWindowExA",
    variant: expectedCreateWindow.variant,
    implementationStatus: "partial",
    guestCallSite: expectedCreateWindow.callSite,
    guestCallSiteRva: expectedCreateWindow.callSite - pe.imageBase,
    guestReturnAddress: expectedCreateWindow.returnAddress,
    guestReturnRva: expectedCreateWindow.returnAddress - pe.imageBase,
    iatAddress: expectedCreateWindow.iatAddress,
    iatRva: expectedCreateWindow.iatAddress - pe.imageBase,
    thunkAddress: expectedCreateWindow.thunkAddress,
    callingConvention: "stdcall",
    argumentBytes: 48,
    arguments: expectedCreateWindow.arguments,
    result: expectedCreateWindow.logicalHandle,
    className: expectedCreateWindow.className,
    windowName: expectedCreateWindow.windowName,
    callbackCallId: 2,
    callbackMessage: 1,
    callbackResult: 0,
    stackPointerBefore: 0x006fffc0,
    stackPointerAfter: 0x006ffff4,
  },
  {
    kind: "host-service-return",
    requester: "window-pixel.exe",
    module: "user32.dll",
    symbol: "ShowWindow",
    variant: expectedShowWindow.variant,
    implementationStatus: "partial",
    guestCallSite: expectedShowWindow.callSite,
    guestCallSiteRva: expectedShowWindow.callSite - pe.imageBase,
    guestReturnAddress: expectedShowWindow.returnAddress,
    guestReturnRva: expectedShowWindow.returnAddress - pe.imageBase,
    iatAddress: expectedShowWindow.iatAddress,
    iatRva: expectedShowWindow.iatAddress - pe.imageBase,
    thunkAddress: expectedShowWindow.thunkAddress,
    callingConvention: "stdcall",
    argumentBytes: 8,
    arguments: expectedShowWindow.arguments,
    result: 0,
    visibilityBefore: false,
    visibilityAfter: true,
    stackPointerBefore: expectedShowWindow.stackPointer,
    stackPointerAfter: 0x006ffff4,
  },
  {
    kind: "host-service-return",
    requester: "window-pixel.exe",
    module: "user32.dll",
    symbol: "InvalidateRect",
    variant: expectedInvalidateRect.variant,
    implementationStatus: "partial",
    guestCallSite: expectedInvalidateRect.callSite,
    guestCallSiteRva: expectedInvalidateRect.callSite - pe.imageBase,
    guestReturnAddress: expectedInvalidateRect.returnAddress,
    guestReturnRva: expectedInvalidateRect.returnAddress - pe.imageBase,
    iatAddress: expectedInvalidateRect.iatAddress,
    iatRva: expectedInvalidateRect.iatAddress - pe.imageBase,
    thunkAddress: expectedInvalidateRect.thunkAddress,
    callingConvention: "stdcall",
    argumentBytes: 12,
    arguments: expectedInvalidateRect.arguments,
    result: 1,
    updateRegion: "whole-client",
    eraseRequested: false,
    erasePendingAfter: false,
    paintStateAfter: paintLifecycle.queued,
    paintCandidateEnqueued: true,
    paintCandidateSequence: 1,
    queueOwnerThreadId: mainGuestThread.id,
    queueLane: "generated-paint",
    generatedPaintDepthAfter: 1,
    stackPointerBefore: expectedInvalidateRect.stackPointer,
    stackPointerAfter: 0x006ffff4,
  },
  {
    kind: "host-service-return",
    requester: "window-pixel.exe",
    module: "user32.dll",
    symbol: "GetMessageA",
    variant: expectedGetMessage.variant,
    implementationStatus: "partial",
    guestCallSite: expectedGetMessage.callSite,
    guestCallSiteRva: expectedGetMessage.callSite - pe.imageBase,
    guestReturnAddress: expectedGetMessage.returnAddress,
    guestReturnRva: expectedGetMessage.returnAddress - pe.imageBase,
    iatAddress: expectedGetMessage.iatAddress,
    iatRva: expectedGetMessage.iatAddress - pe.imageBase,
    thunkAddress: expectedGetMessage.thunkAddress,
    callEncoding: "iat-indirect",
    callingConvention: "stdcall",
    argumentBytes: 16,
    arguments: expectedGetMessage.arguments,
    result: 1,
    materializedMessage: expectedGetMessage.messageWords,
    messageTime: 0,
    messagePoint: { x: 0, y: 0 },
    paintStateBefore: paintLifecycle.queued,
    paintStateAfter: paintLifecycle.inFlight,
    paintCandidateSequence: 1,
    queueOwnerThreadId: mainGuestThread.id,
    generatedPaintDepthBefore: 1,
    generatedPaintDepthAfter: 0,
    stackPointerBefore: expectedGetMessage.stackPointer,
    stackPointerAfter: 0x006ffff4,
  },
]);
assert.deepEqual(diagnostic, {
  kind: "unsupported-reentrant-host-service",
  parentCallId: 4,
  rootCallId: 3,
  rootModule: "user32.dll",
  rootSymbol: "DispatchMessageA",
  rootVariant: expectedDispatchMessage.variant,
  callbackRole: "window-procedure-wm-paint",
  callbackTarget: expectedDispatchMessage.callbackTarget,
  requester: "window-pixel.exe",
  module: "user32.dll",
  symbol: "BeginPaint",
  guestCallSite: expectedBeginPaint.callSite,
  guestCallSiteRva: expectedBeginPaint.callSite - pe.imageBase,
  guestReturnAddress: expectedBeginPaint.returnAddress,
  guestReturnRva: expectedBeginPaint.returnAddress - pe.imageBase,
  iatAddress: expectedBeginPaint.iatAddress,
  iatRva: expectedBeginPaint.iatAddress - pe.imageBase,
  thunkAddress: expectedBeginPaint.thunkAddress,
  callEncoding: "iat-indirect",
  engineStopReason: stopReason.invalidOpcode,
  engineErrorCode: -1,
  stackPointer: expectedBeginPaint.stackPointer,
});
assert.deepEqual(
  Array.from({ length: 2 }, (_, index) =>
    readGuestU32(finalState.esp + 4 + index * 4, `BeginPaint argument ${index}`),
  ),
  expectedBeginPaint.arguments,
);
assert.equal(logicalClassesByKey.size, 1);
assert.equal(logicalClassesByAtom.size, 1);
const logicalClass = logicalClassesByAtom.get(0xc000);
assert(logicalClass);
assert.equal(logicalClassesByKey.get(logicalClass.key), logicalClass);
assert(Object.isFrozen(logicalClass));
assert(Object.isFrozen(logicalClass.wndProcCallback));
assert.deepEqual(logicalClass, {
  key: localClassKey(mainModule, expectedWindowClass.name),
  atom: 0xc000,
  scope: "local",
  encoding: "A-ascii",
  name: expectedWindowClass.name,
  moduleIdentity: moduleIdentity(mainModule),
  wndProcCallback: {
    kind: "window-procedure-a",
    moduleIdentity: moduleIdentity(mainModule),
    entryRva: expectedWindowClass.wndProc - mainModule.baseAddress,
    callingConvention: "stdcall",
    argumentBytes: 16,
  },
  style: 0,
  classExtraBytes: 0,
  windowExtraBytes: 0,
});
assert.deepEqual(callbackEvents, [
  {
    kind: "guest-callback-enter",
    callId: 2,
    parentCallId: 1,
    role: "window-procedure-wm-create",
    target: expectedWindowClass.wndProc,
    returnSentinel: guest.callbackReturn,
    arguments: [
      expectedCreateWindow.logicalHandle,
      1,
      0,
      expectedCreateWindow.createStructAddress,
    ],
    stackPointer: expectedCreateWindow.callbackStackPointer,
  },
  {
    kind: "guest-callback-return",
    callId: 2,
    parentCallId: 1,
    role: "window-procedure-wm-create",
    target: expectedWindowClass.wndProc,
    result: 0,
    stackPointer: expectedCreateWindow.callbackReturnStackPointer,
    instructionCount: 50,
  },
  {
    kind: "guest-callback-enter",
    callId: 4,
    parentCallId: 3,
    role: "window-procedure-wm-paint",
    target: expectedDispatchMessage.callbackTarget,
    returnSentinel: guest.callbackReturn,
    arguments: [expectedCreateWindow.logicalHandle, 0x000f, 0, 0],
    stackPointer: expectedDispatchMessage.callbackStackPointer,
    instructionCount: 83,
  },
]);
assert.equal(hostCallFrames.length, 2);
const [dispatchApiFrame, paintCallbackFrame] = hostCallFrames;
assert(Object.isSealed(dispatchApiFrame));
assert.equal(dispatchApiFrame.id, 3);
assert.equal(dispatchApiFrame.kind, "api");
assert.equal(dispatchApiFrame.service.module, "user32.dll");
assert.equal(dispatchApiFrame.service.symbol, "DispatchMessageA");
assert.equal(dispatchApiFrame.variant, expectedDispatchMessage.variant);
assert.equal(dispatchApiFrame.phase, "callback");
assert.equal(dispatchApiFrame.callSite, expectedDispatchMessage.callSite);
assert.equal(dispatchApiFrame.callEncoding, "register-indirect-esi");
assert.equal(dispatchApiFrame.returnAddress, expectedDispatchMessage.returnAddress);
assert.equal(dispatchApiFrame.outerEsp, expectedDispatchMessage.stackPointer);
assert.equal(dispatchApiFrame.argumentBytes, 4);
assert.equal(dispatchApiFrame.resumeEsp, 0x006ffff4);
assert.deepEqual(dispatchApiFrame.arguments, expectedDispatchMessage.arguments);
assert.deepEqual(dispatchApiFrame.message, expectedGetMessage.messageWords);
assert.equal(dispatchApiFrame.paintSequence, 1);
assert.equal(dispatchApiFrame.window.handle, expectedCreateWindow.logicalHandle);
assert(Object.isSealed(paintCallbackFrame));
assert.equal(paintCallbackFrame.id, 4);
assert.equal(paintCallbackFrame.parentId, dispatchApiFrame.id);
assert.equal(paintCallbackFrame.kind, "guest-callback");
assert.equal(paintCallbackFrame.role, "window-procedure-wm-paint");
assert.equal(paintCallbackFrame.target, expectedDispatchMessage.callbackTarget);
assert.equal(paintCallbackFrame.returnSentinel, guest.callbackReturn);
assert.equal(paintCallbackFrame.entryEsp, expectedDispatchMessage.callbackStackPointer);
assert.equal(
  paintCallbackFrame.expectedReturnEsp,
  expectedDispatchMessage.callbackReturnStackPointer,
);
assert.equal(paintCallbackFrame.argumentBytes, 16);
assert.deepEqual(paintCallbackFrame.arguments, [
  expectedCreateWindow.logicalHandle,
  0x000f,
  0,
  0,
]);
assert.equal(logicalWindowsByHandle.size, 1);
assert.equal(nextWindowSlot, 2);
assert.equal(nextHostCallId, 5);
const logicalWindow = lookupLogicalWindow(expectedCreateWindow.logicalHandle);
assert(Object.isSealed(logicalWindow));
assert(Object.isFrozen(logicalWindow.requestedPosition));
assert(Object.isFrozen(logicalWindow.requestedOuterSize));
assert(Object.isFrozen(logicalWindow.updateRegion));
assert.deepEqual(logicalWindow, {
  handle: expectedCreateWindow.logicalHandle,
  type: "window",
  typeTag: windowHandleTag,
  generation: 1,
  slot: 1,
  lifecycle: "created",
  visible: true,
  moduleIdentity: moduleIdentity(mainModule),
  classKey: logicalClass.key,
  classAtom: logicalClass.atom,
  className: expectedCreateWindow.className,
  title: expectedCreateWindow.windowName,
  extendedStyle: 0,
  style: 0x00cf0000,
  requestedPosition: { x: "use-default", y: "use-default" },
  requestedOuterSize: { width: 160, height: 120 },
  parent: null,
  menu: null,
  ownerThreadId: mainGuestThread.id,
  updateRegion: { kind: "whole-client" },
  erasePending: false,
  paintState: paintLifecycle.inFlight,
  paintSequence: 1,
  showCommand: 5,
});
assert.equal(dispatchApiFrame.window, logicalWindow);
assert(Object.isSealed(mainGuestThread));
assert(Object.isSealed(mainGuestThread.messageState));
assert.equal(mainGuestThread.messageState.nextSequence, 2);
assert.equal(mainGuestThread.messageState.generatedPaintLane.length, 0);
const inFlightPaint = mainGuestThread.messageState.inFlightPaint;
assert(inFlightPaint);
assert(Object.isFrozen(inFlightPaint));
assert(Object.isFrozen(inFlightPaint.point));
assert.deepEqual(inFlightPaint, {
  sequence: 1,
  kind: "generated-paint",
  source: "window-update-region",
  ownerThreadId: mainGuestThread.id,
  hwnd: logicalWindow.handle,
  message: 0x000f,
  wParam: 0,
  lParam: 0,
  time: 0,
  point: { x: 0, y: 0 },
});
assert.deepEqual(
  Array.from({ length: 7 }, (_, index) =>
    readGuestU32(expectedGetMessage.arguments[0] + index * 4, `final MSG word ${index}`),
  ),
  expectedGetMessage.messageWords,
);
assert.deepEqual(
  Array.from({ length: 16 }, (_, index) =>
    readGuestU32(expectedBeginPaint.arguments[1] + index * 4, `final PAINTSTRUCT word ${index}`),
  ),
  Array(16).fill(0),
);
assert.equal(readGuestU32(0x00403050, "final dispatch count"), 1);
assert.equal(readGuestU32(0x00403054, "final dispatched paint count"), 1);
assert.equal(readGuestU32(0x00403058, "final window failure code"), 0);
assert.equal(readGuestU32(0x0040305c, "final WM_CREATE count"), 1);
assert.equal(readGuestU32(0x00403060, "final WM_PAINT count"), 1);
assert.equal(readGuestU32(0x00403064, "final WM_DESTROY count"), 0);
assert.equal(readGuestU32(0x00403068, "final window event count"), 2);
assert.equal(readGuestU32(0x0040306c, "final first window event"), 1);
assert.equal(readGuestU32(0x00403070, "final second window event"), 0x000f);
assert.equal(readGuestU32(0x00403074, "final third window event"), 0);
assert.equal(readGuestU32(0x00403078, "final BeginPaint count"), 1);
assert.equal(readGuestU32(0x0040307c, "final EndPaint count"), 0);
assert.equal(readGuestU32(0x00403080, "final SetPixel count"), 0);
assert.equal(readGuestU32(0x00403084, "final destroy success count"), 0);
assert.equal(readGuestU32(0x00403088, "final PostQuitMessage count"), 0);
assert.equal(readGuestU32(0x00403000, "final pixel result"), 0xffffffff);
assert.equal(finalState.eax, 1);
assert.equal(finalState.words[savedStateWord.esi], logicalWindow.handle);
assert.equal(finalState.words[savedStateWord.edi], expectedGetMessage.thunkAddress);
assert.equal(finalState.esp, expectedBeginPaint.stackPointer);
assert.equal(finalState.eip, expectedBeginPaint.thunkAddress);
assert.equal(finalState.previousEip, expectedBeginPaint.thunkAddress);
assert.equal(
  readGuestU32(expectedBeginPaint.stackPointer, "BeginPaint return address"),
  expectedBeginPaint.returnAddress,
);
assert.deepEqual(
  Array.from({ length: 2 }, (_, index) =>
    readGuestU32(
      expectedBeginPaint.stackPointer + 4 + index * 4,
      `preserved BeginPaint argument ${index}`,
    ),
  ),
  expectedBeginPaint.arguments,
);
assert.equal(
  readGuestU32(expectedDispatchMessage.callbackStackPointer, "WM_PAINT return sentinel"),
  guest.callbackReturn,
);
assert.deepEqual(
  Array.from({ length: 4 }, (_, index) =>
    readGuestU32(
      expectedDispatchMessage.callbackStackPointer + 4 + index * 4,
      `preserved WM_PAINT argument ${index}`,
    ),
  ),
  [expectedCreateWindow.logicalHandle, 0x000f, 0, 0],
);
assert.equal(
  readGuestU32(expectedDispatchMessage.stackPointer, "DispatchMessageA return address"),
  expectedDispatchMessage.returnAddress,
);
assert.equal(
  readGuestU32(expectedDispatchMessage.stackPointer + 4, "DispatchMessageA MSG argument"),
  expectedDispatchMessage.arguments[0],
);
assert.equal(readGuestU32(0x006ffff4, "saved entry ESI"), calleeSaved.esi);
assert.equal(readGuestU32(0x006ffff8, "saved entry EDI"), calleeSaved.edi);
assert.equal(readGuestU32(0x006ffffc, "entry return sentinel"), guest.entryReturn);
assert.deepEqual(
  memory.slice(guest.callbackReturn, guest.callbackReturn + 2),
  Uint8Array.from([0x0f, 0x0b]),
);
assert.deepEqual(Object.keys(hostImportCalls), ["microtick"]);
assert(hostImportCalls.microtick > 0);

console.log(
  `runtime window message dispatch ingress passed: imports=${resolvedImports.length}, ` +
    `completed-variants=${implementedServiceEvents.length}, callback-events=${callbackEvents.length}, ` +
    `hwnd=0x${logicalWindow.handle.toString(16)}, message=0x${inFlightPaint.message.toString(16)}, ` +
    `paint-state=${logicalWindow.paintState}, ` +
    `paint-candidates=${mainGuestThread.messageState.generatedPaintLane.length}, ` +
    `stop=${diagnostic.module}!${diagnostic.symbol}, ` +
    `call=0x${diagnostic.guestCallSite.toString(16)}, ` +
    `budget stops=${budgetEvents.length}, execute stops=${executeStopCount}, ` +
    `instructions=${finalState.instructionCount}`,
);
