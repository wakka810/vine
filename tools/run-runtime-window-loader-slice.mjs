import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import {
  browserPresentEventLogPrefix,
  createFixedW2BrowserPresentEvent,
} from "../spikes/browser/w2-first-pixel/present-event.mjs";

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
  variant: "beginpaint-whole-client-harness-surface",
  callSite: 0x0040133d,
  returnAddress: 0x00401343,
  iatAddress: 0x004020c8,
  thunkAddress: 0x00500010,
  stackPointer: 0x006fffc8,
  arguments: [0x57010001, 0x0040308c],
};
const expectedPaint = {
  logicalDc: 0x48010001,
  clientBounds: Object.freeze({ left: 0, top: 0, right: 2, bottom: 2 }),
  paintStructWords: Object.freeze([
    0x48010001,
    0,
    0,
    0,
    2,
    2,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
  ]),
};
const expectedSetPixel = {
  variant: "setpixel-harness-surface-exact-point",
  callSite: 0x00401357,
  returnAddress: 0x0040135d,
  iatAddress: 0x004020f8,
  thunkAddress: 0x005000c0,
  stackPointer: 0x006fffc0,
  arguments: [expectedPaint.logicalDc, 1, 1, 0x000000ff],
};
const expectedEndPaint = {
  variant: "endpaint-matching-active-session",
  callSite: 0x0040138d,
  returnAddress: 0x00401393,
  iatAddress: 0x004020dc,
  thunkAddress: 0x00500060,
  stackPointer: 0x006fffc8,
  arguments: expectedBeginPaint.arguments,
};
const expectedDestroyWindow = {
  variant: "destroywindow-synchronous-wm-destroy",
  callSite: 0x004013d6,
  returnAddress: 0x004013dc,
  iatAddress: 0x004020d4,
  thunkAddress: 0x00500040,
  stackPointer: 0x006fffcc,
  arguments: [expectedCreateWindow.logicalHandle],
  callbackStackPointer: 0x006fffb8,
  callbackReturnStackPointer: 0x006fffcc,
};
const expectedPostQuitMessage = {
  variant: "postquitmessage-thread-slot-exit-zero",
  callSite: 0x004013fd,
  returnAddress: 0x00401403,
  iatAddress: 0x004020e8,
  thunkAddress: 0x00500090,
  stackPointer: 0x006fffac,
  arguments: [0],
};
const expectedQuitMessage = {
  variant: "getmessagea-unfiltered-thread-quit-zero-metadata",
  callSite: 0x00401102,
  returnAddress: 0x00401104,
  iatAddress: 0x004020e0,
  thunkAddress: 0x00500070,
  stackPointer: 0x006fffe0,
  arguments: expectedGetMessage.arguments,
  messageWords: [0, 0x0012, 0, 0, 0, 0, 0],
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
  painting: "painting",
});
const mainGuestThread = Object.seal({
  id: 1,
  messageState: Object.seal({
    generatedPaintLane: [],
    inFlightPaint: null,
    quitSlot: null,
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
const retiredLogicalWindows = new Map();
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
const paintDcHandleTag = 0x48;
const logicalPaintDcsByHandle = new Map();
const retiredPaintDcs = new Map();
const retiredPaintSessions = [];
let nextPaintDcSlot = 1;
let nextPaintSessionId = 1;
const allocateLogicalPaintDc = (fields) => {
  assert(nextPaintDcSlot > 0 && nextPaintDcSlot <= 0xffff, "paint DC slot space exhausted");
  const slot = nextPaintDcSlot;
  const generation = 1;
  const handle =
    ((paintDcHandleTag << 24) | (generation << 16) | slot) >>> 0;
  assert(!logicalPaintDcsByHandle.has(handle), "duplicate logical HDC");
  const dc = Object.seal({
    ...fields,
    handle,
    type: "paint-dc",
    typeTag: paintDcHandleTag,
    generation,
    slot,
    lifecycle: "active",
  });
  logicalPaintDcsByHandle.set(handle, dc);
  nextPaintDcSlot += 1;
  return dc;
};
const lookupLogicalPaintDc = (handle) => {
  const normalizedHandle = handle >>> 0;
  const dc = logicalPaintDcsByHandle.get(normalizedHandle);
  assert(dc, `unknown logical HDC 0x${normalizedHandle.toString(16)}`);
  assert.equal(dc.type, "paint-dc");
  assert.equal(dc.lifecycle, "active");
  assert.equal(normalizedHandle >>> 24, paintDcHandleTag);
  return dc;
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
  const registerOpcode = memory[registerCallSite + 1];
  assert.equal(memory[registerCallSite], 0xff);
  assert(
    registerOpcode === 0xd6 || registerOpcode === 0xd7,
    "fixed register import call is not CALL ESI or CALL EDI",
  );
  const registerWord =
    registerOpcode === 0xd6 ? savedStateWord.esi : savedStateWord.edi;
  assert.equal(state.words[registerWord], service.thunkAddress);
  return {
    callSite: registerCallSite,
    returnAddress,
    callEncoding:
      registerOpcode === 0xd6
        ? "register-indirect-esi"
        : "register-indirect-edi",
  };
};

const budgetEvents = [];
const implementedServiceEvents = [];
const callbackEvents = [];
const browserPresentEvents = [];
const hostCallFrames = [];
const maxHostCallDepth = 4;
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
let completion = null;
let finalState = null;
let executeStopCount = 0;
for (
  let stopCount = 0;
  stopCount < 128 && diagnostic === null && completion === null;
  stopCount += 1
) {
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
    const apiFrame = hostCallFrames.at(-2);
    assert(apiFrame);
    assert.equal(apiFrame.kind, "api");
    assert.equal(apiFrame.id, callbackFrame.parentId);
    assert.equal(callbackFrame.expectedReturnEsp, apiFrame.outerEsp);
    assert.equal(apiFrame.phase, "callback");
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
    const callbackReturnEvent = {
      kind: "guest-callback-return",
      callId: callbackFrame.id,
      parentCallId: apiFrame.id,
      role: callbackFrame.role,
      target: callbackFrame.target,
      result: state.eax,
      stackPointer: state.esp,
      instructionCount: state.instructionCount,
    };

    if (callbackFrame.role === "window-procedure-wm-create") {
      assert.equal(apiFrame.service.symbol, "CreateWindowExA");
      assert.equal(state.eax, 0, "fixed WM_CREATE callback rejected window creation");
      assert.equal(state.words[savedStateWord.esi], mainModule.baseAddress);
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
      callbackEvents.push(callbackReturnEvent);
      assert.equal(popHostCallFrame("api"), apiFrame);
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

    if (callbackFrame.role === "window-procedure-wm-destroy") {
      assert.equal(apiFrame.service.symbol, "DestroyWindow");
      assert.equal(apiFrame.id, 5);
      assert.equal(callbackFrame.id, 6);
      assert.equal(state.eax, 0);
      assert.equal(state.words[savedStateWord.esi], apiFrame.window.handle);
      assert.equal(apiFrame.window.lifecycle, "destroying");
      assert.equal(logicalWindowsByHandle.get(apiFrame.window.handle), apiFrame.window);
      assert(mainGuestThread.messageState.quitSlot);
      assert.equal(readGuestU32(0x00403064, "WM_DESTROY count"), 1);
      assert.equal(readGuestU32(0x00403068, "window event count"), 3);
      assert.equal(readGuestU32(0x00403074, "third window event"), 2);
      assert.equal(popHostCallFrame("guest-callback"), callbackFrame);
      callbackEvents.push(callbackReturnEvent);
      apiFrame.window.lifecycle = "destroyed";
      apiFrame.window.visible = false;
      assert.equal(logicalWindowsByHandle.delete(apiFrame.window.handle), true);
      retiredLogicalWindows.set(apiFrame.window.handle, apiFrame.window);
      apiFrame.phase = "returning";
      const rootCallId = hostCallFrames[0].id;
      assert.equal(popHostCallFrame("api"), apiFrame);
      implementedServiceEvents.push({
        kind: "host-service-return",
        callId: apiFrame.id,
        parentCallId: apiFrame.parentId,
        rootCallId,
        requester: apiFrame.service.requester,
        module: apiFrame.service.module,
        symbol: apiFrame.service.symbol,
        variant: expectedDestroyWindow.variant,
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
        result: 1,
        callbackCallId: callbackFrame.id,
        callbackMessage: 2,
        callbackResult: state.eax,
        stackPointerBefore: apiFrame.outerEsp,
        stackPointerAfter: apiFrame.resumeEsp,
      });
      resume(state, { eax: 1, esp: apiFrame.resumeEsp, eip: apiFrame.returnAddress });
      continue;
    }

    assert.equal(callbackFrame.role, "window-procedure-wm-paint");
    assert.equal(apiFrame.service.symbol, "DispatchMessageA");
    assert.equal(state.eax, 0);
    assert.equal(state.words[savedStateWord.esi], expectedDispatchMessage.thunkAddress);
    assert.equal(apiFrame.window.lifecycle, "destroyed");
    assert.equal(retiredLogicalWindows.get(apiFrame.window.handle), apiFrame.window);
    assert.equal(mainGuestThread.messageState.inFlightPaint, null);
    assert.equal(apiFrame.window.paintState, paintLifecycle.none);
    assert.equal(apiFrame.window.activePaintSession, null);
    assert.equal(popHostCallFrame("guest-callback"), callbackFrame);
    callbackEvents.push(callbackReturnEvent);
    apiFrame.phase = "returning";
    assert.equal(popHostCallFrame("api"), apiFrame);
    implementedServiceEvents.push({
      kind: "host-service-return",
      callId: apiFrame.id,
      requester: apiFrame.service.requester,
      module: apiFrame.service.module,
      symbol: apiFrame.service.symbol,
      variant: expectedDispatchMessage.variant,
      implementationStatus: "partial",
      guestCallSite: apiFrame.callSite,
      guestCallSiteRva: apiFrame.callSite - pe.imageBase,
      guestReturnAddress: apiFrame.returnAddress,
      guestReturnRva: apiFrame.returnAddress - pe.imageBase,
      iatAddress: apiFrame.service.iatAddress,
      iatRva: apiFrame.service.iatRva,
      thunkAddress: apiFrame.service.thunkAddress,
      callEncoding: apiFrame.callEncoding,
      callingConvention: "stdcall",
      argumentBytes: apiFrame.argumentBytes,
      arguments: [...apiFrame.arguments],
      result: state.eax,
      callbackCallId: callbackFrame.id,
      callbackMessage: 0x000f,
      callbackResult: state.eax,
      stackPointerBefore: apiFrame.outerEsp,
      stackPointerAfter: apiFrame.resumeEsp,
    });
    resume(state, {
      eax: state.eax,
      esp: apiFrame.resumeEsp,
      eip: apiFrame.returnAddress,
    });
    continue;
  }

  if (state.eip === guest.entryReturn) {
    assert.equal(hostCallFrames.length, 0);
    completion = Object.freeze({
      kind: "fixture-entry-return",
      returnSentinel: guest.entryReturn,
      result: state.eax,
      stackPointer: state.esp,
      instructionCount: state.instructionCount,
    });
    finalState = state;
    continue;
  }

  const service = ownedServices.get(state.eip);
  assert(service, `#UD at unowned guest address 0x${state.eip.toString(16)}`);

  const { callSite, returnAddress, callEncoding } = decodeImportCall(state, service);

  if (hostCallFrames.length !== 0) {
    const callbackFrame = peekHostCallFrame();
    assert(callbackFrame);
    assert.equal(callbackFrame.kind, "guest-callback");
    const apiFrame = hostCallFrames.at(-2);
    assert(apiFrame);
    assert.equal(apiFrame.kind, "api");
    assert.equal(callbackFrame.parentId, apiFrame.id);
    const rootApiFrame = hostCallFrames[0];
    assert.equal(rootApiFrame.kind, "api");

    const isBeginPaint =
      service.module === "user32.dll" &&
      service.symbol === "BeginPaint" &&
      callbackFrame.role === "window-procedure-wm-paint" &&
      apiFrame.service.symbol === "DispatchMessageA";
    if (isBeginPaint) {
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
      const inFlightPaint = mainGuestThread.messageState.inFlightPaint;
      assert(inFlightPaint);
      assert.equal(inFlightPaint.sequence, window.paintSequence);
      assert.equal(inFlightPaint.sequence, apiFrame.paintSequence);
      assert.equal(window.activePaintSession, null);
      assert.equal(logicalPaintDcsByHandle.size, 0);
      const dc = allocateLogicalPaintDc({
        ownerThreadId: mainGuestThread.id,
        windowHandle: window.handle,
        paintSequence: inFlightPaint.sequence,
        bounds: window.clientSurface.bounds,
      });
      assert.equal(dc.handle, expectedPaint.logicalDc);
      const session = Object.seal({
        id: nextPaintSessionId,
        ownerThreadId: mainGuestThread.id,
        windowHandle: window.handle,
        paintStructAddress: beginPaintArguments[1],
        paintSequence: inFlightPaint.sequence,
        updateRegion: window.updateRegion,
        bounds: window.clientSurface.bounds,
        dc,
        lifecycle: "active",
      });
      nextPaintSessionId += 1;
      window.activePaintSession = session;
      window.updateRegion = null;
      window.erasePending = false;
      window.paintState = paintLifecycle.painting;
      mainGuestThread.messageState.inFlightPaint = null;
      writeGuestWords(
        session.paintStructAddress,
        expectedPaint.paintStructWords,
        "BeginPaint PAINTSTRUCT",
      );
      const argumentBytes = beginPaintArguments.length * 4;
      const resumeEsp = state.esp + 4 + argumentBytes;
      assert.equal(resumeEsp, 0x006fffd4);
      implementedServiceEvents.push({
        kind: "host-service-return",
        parentCallId: callbackFrame.id,
        rootCallId: rootApiFrame.id,
        requester: service.requester,
        module: service.module,
        symbol: service.symbol,
        variant: expectedBeginPaint.variant,
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
        arguments: [...beginPaintArguments],
        result: dc.handle,
        paintSessionId: session.id,
        paintSequence: session.paintSequence,
        updateRegion: session.updateRegion.kind,
        clientBounds: { ...session.bounds },
        paintStructWords: [...expectedPaint.paintStructWords],
        stackPointerBefore: state.esp,
        stackPointerAfter: resumeEsp,
      });
      resume(state, { eax: dc.handle, esp: resumeEsp, eip: returnAddress });
      continue;
    }

    const isSetPixel =
      service.module === "gdi32.dll" &&
      service.symbol === "SetPixel" &&
      callbackFrame.role === "window-procedure-wm-paint" &&
      apiFrame.service.symbol === "DispatchMessageA";
    if (isSetPixel) {
      const window = apiFrame.window;
      assert.equal(callSite, expectedSetPixel.callSite);
      assert.equal(returnAddress, expectedSetPixel.returnAddress);
      assert.equal(service.iatAddress, expectedSetPixel.iatAddress);
      assert.equal(service.thunkAddress, expectedSetPixel.thunkAddress);
      assert.equal(callEncoding, "iat-indirect");
      assert.equal(state.esp, expectedSetPixel.stackPointer);
      const arguments_ = Array.from({ length: 4 }, (_, index) =>
        readGuestU32(state.esp + 4 + index * 4, `SetPixel argument ${index}`),
      );
      assert.deepEqual(arguments_, expectedSetPixel.arguments);
      const [dcHandle, x, y, color] = arguments_;
      const dc = lookupLogicalPaintDc(dcHandle);
      const session = window.activePaintSession;
      assert(session);
      assert.equal(session.lifecycle, "active");
      assert.equal(session.dc, dc);
      assert.equal(session.paintSequence, apiFrame.paintSequence);
      assert.equal(dc.paintSequence, session.paintSequence);
      assert.equal(dc.ownerThreadId, mainGuestThread.id);
      assert.equal(dc.windowHandle, window.handle);
      assert.equal(window.paintState, paintLifecycle.painting);
      assert(x >= session.bounds.left && x < session.bounds.right);
      assert(y >= session.bounds.top && y < session.bounds.bottom);
      assert.equal(color, 0x000000ff);
      window.clientSurface.pixels.set(`${x},${y}`, color);
      const argumentBytes = arguments_.length * 4;
      const resumeEsp = state.esp + 4 + argumentBytes;
      assert.equal(resumeEsp, 0x006fffd4);
      implementedServiceEvents.push({
        kind: "host-service-return",
        parentCallId: callbackFrame.id,
        rootCallId: rootApiFrame.id,
        requester: service.requester,
        module: service.module,
        symbol: service.symbol,
        variant: expectedSetPixel.variant,
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
        arguments: [...arguments_],
        result: color,
        paintSessionId: session.id,
        paintSequence: session.paintSequence,
        stackPointerBefore: state.esp,
        stackPointerAfter: resumeEsp,
      });
      resume(state, { eax: color, esp: resumeEsp, eip: returnAddress });
      continue;
    }

    const isEndPaint =
      service.module === "user32.dll" &&
      service.symbol === "EndPaint" &&
      callbackFrame.role === "window-procedure-wm-paint" &&
      apiFrame.service.symbol === "DispatchMessageA";
    if (isEndPaint) {
      const window = apiFrame.window;
      assert.equal(callSite, expectedEndPaint.callSite);
      assert.equal(returnAddress, expectedEndPaint.returnAddress);
      assert.equal(service.iatAddress, expectedEndPaint.iatAddress);
      assert.equal(service.thunkAddress, expectedEndPaint.thunkAddress);
      assert.equal(callEncoding, "iat-indirect");
      assert.equal(state.esp, expectedEndPaint.stackPointer);
      const arguments_ = Array.from({ length: 2 }, (_, index) =>
        readGuestU32(state.esp + 4 + index * 4, `EndPaint argument ${index}`),
      );
      assert.deepEqual(arguments_, expectedEndPaint.arguments);
      const [handle, paintStructAddress] = arguments_;
      assert.equal(lookupLogicalWindow(handle), window);
      const session = window.activePaintSession;
      assert(session);
      assert.equal(session.paintStructAddress, paintStructAddress);
      assert.equal(session.lifecycle, "active");
      assert.equal(session.paintSequence, apiFrame.paintSequence);
      assert.equal(lookupLogicalPaintDc(session.dc.handle), session.dc);
      assert.equal(session.dc.paintSequence, session.paintSequence);
      assert.deepEqual(
        Array.from({ length: 16 }, (_, index) =>
          readGuestU32(paintStructAddress + index * 4, `EndPaint PAINTSTRUCT word ${index}`),
        ),
        expectedPaint.paintStructWords,
      );
      session.lifecycle = "retired";
      session.dc.lifecycle = "retired";
      assert.equal(logicalPaintDcsByHandle.delete(session.dc.handle), true);
      retiredPaintDcs.set(session.dc.handle, session.dc);
      retiredPaintSessions.push(session);
      window.activePaintSession = null;
      window.paintState = paintLifecycle.none;
      window.paintSequence = null;
      const setPixelForPresent = implementedServiceEvents.at(-1);
      assert.equal(setPixelForPresent.symbol, "SetPixel");
      const [, pixelX, pixelY, colorRef] = setPixelForPresent.arguments;
      assert.equal(window.clientSurface.pixels.get(`${pixelX},${pixelY}`), colorRef);
      browserPresentEvents.push(
        createFixedW2BrowserPresentEvent({
          bounds: window.clientSurface.bounds,
          paintSequence: session.paintSequence,
          pixels: [{ x: pixelX, y: pixelY, colorRef }],
        }),
      );
      const argumentBytes = arguments_.length * 4;
      const resumeEsp = state.esp + 4 + argumentBytes;
      assert.equal(resumeEsp, 0x006fffd4);
      implementedServiceEvents.push({
        kind: "host-service-return",
        parentCallId: callbackFrame.id,
        rootCallId: rootApiFrame.id,
        requester: service.requester,
        module: service.module,
        symbol: service.symbol,
        variant: expectedEndPaint.variant,
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
        arguments: [...arguments_],
        result: 1,
        paintSessionId: session.id,
        paintSequence: session.paintSequence,
        stackPointerBefore: state.esp,
        stackPointerAfter: resumeEsp,
      });
      resume(state, { eax: 1, esp: resumeEsp, eip: returnAddress });
      continue;
    }

    const isDestroyWindow =
      service.module === "user32.dll" &&
      service.symbol === "DestroyWindow" &&
      callbackFrame.role === "window-procedure-wm-paint" &&
      apiFrame.service.symbol === "DispatchMessageA";
    if (isDestroyWindow) {
      const window = apiFrame.window;
      assert.equal(rootApiFrame, apiFrame);
      assert.equal(callSite, expectedDestroyWindow.callSite);
      assert.equal(returnAddress, expectedDestroyWindow.returnAddress);
      assert.equal(service.iatAddress, expectedDestroyWindow.iatAddress);
      assert.equal(service.thunkAddress, expectedDestroyWindow.thunkAddress);
      assert.equal(callEncoding, "iat-indirect");
      assert.equal(state.esp, expectedDestroyWindow.stackPointer);
      const arguments_ = [readGuestU32(state.esp + 4, "DestroyWindow HWND")];
      assert.deepEqual(arguments_, expectedDestroyWindow.arguments);
      assert.equal(lookupLogicalWindow(arguments_[0]), window);
      assert.equal(window.lifecycle, "created");
      assert.equal(window.paintState, paintLifecycle.none);
      assert.equal(window.paintSequence, null);
      assert.equal(window.activePaintSession, null);
      assert.equal(logicalPaintDcsByHandle.size, 0);
      const logicalClass = logicalClassesByAtom.get(window.classAtom);
      assert(logicalClass);
      const callbackTarget = resolveGuestCallback(logicalClass.wndProcCallback);
      const callbackArguments = [window.handle, 2, 0, 0];
      const callbackFrameBytes = 4 + logicalClass.wndProcCallback.argumentBytes;
      const callbackStackPointer = state.esp - callbackFrameBytes;
      assert.equal(callbackStackPointer, expectedDestroyWindow.callbackStackPointer);
      writeGuestU32(callbackStackPointer, guest.callbackReturn, "WM_DESTROY return sentinel");
      writeGuestWords(callbackStackPointer + 4, callbackArguments, "WM_DESTROY arguments");
      window.lifecycle = "destroying";
      const argumentBytes = 4;
      const destroyApiFrame = pushHostCallFrame({
        parentId: callbackFrame.id,
        kind: "api",
        service,
        variant: expectedDestroyWindow.variant,
        phase: "callback",
        callSite,
        callEncoding,
        returnAddress,
        outerEsp: state.esp,
        argumentBytes,
        resumeEsp: state.esp + 4 + argumentBytes,
        arguments: Object.freeze([...arguments_]),
        window,
      });
      assert.equal(destroyApiFrame.id, 5);
      const destroyCallbackFrame = pushHostCallFrame({
        parentId: destroyApiFrame.id,
        kind: "guest-callback",
        role: "window-procedure-wm-destroy",
        target: callbackTarget,
        returnSentinel: guest.callbackReturn,
        entryEsp: callbackStackPointer,
        expectedReturnEsp: callbackStackPointer + callbackFrameBytes,
        argumentBytes: logicalClass.wndProcCallback.argumentBytes,
        arguments: Object.freeze([...callbackArguments]),
      });
      assert.equal(destroyCallbackFrame.id, 6);
      assert.equal(
        destroyCallbackFrame.expectedReturnEsp,
        expectedDestroyWindow.callbackReturnStackPointer,
      );
      callbackEvents.push({
        kind: "guest-callback-enter",
        callId: destroyCallbackFrame.id,
        parentCallId: destroyApiFrame.id,
        role: destroyCallbackFrame.role,
        target: callbackTarget,
        returnSentinel: guest.callbackReturn,
        arguments: [...callbackArguments],
        stackPointer: callbackStackPointer,
        instructionCount: state.instructionCount,
      });
      resume(state, { esp: callbackStackPointer, eip: callbackTarget });
      continue;
    }

    const isPostQuitMessage =
      service.module === "user32.dll" &&
      service.symbol === "PostQuitMessage" &&
      callbackFrame.role === "window-procedure-wm-destroy" &&
      apiFrame.service.symbol === "DestroyWindow";
    if (isPostQuitMessage) {
      assert.equal(rootApiFrame.service.symbol, "DispatchMessageA");
      assert.equal(callSite, expectedPostQuitMessage.callSite);
      assert.equal(returnAddress, expectedPostQuitMessage.returnAddress);
      assert.equal(service.iatAddress, expectedPostQuitMessage.iatAddress);
      assert.equal(service.thunkAddress, expectedPostQuitMessage.thunkAddress);
      assert.equal(callEncoding, "iat-indirect");
      assert.equal(state.esp, expectedPostQuitMessage.stackPointer);
      const arguments_ = [readGuestU32(state.esp + 4, "PostQuitMessage exit code")];
      assert.deepEqual(arguments_, expectedPostQuitMessage.arguments);
      assert.equal(mainGuestThread.messageState.quitSlot, null);
      const quitSlot = Object.freeze({
        sequence: mainGuestThread.messageState.nextSequence,
        kind: "thread-quit",
        source: "PostQuitMessage",
        ownerThreadId: mainGuestThread.id,
        hwnd: 0,
        message: 0x0012,
        wParam: arguments_[0],
        lParam: 0,
      });
      mainGuestThread.messageState.nextSequence += 1;
      mainGuestThread.messageState.quitSlot = quitSlot;
      const argumentBytes = 4;
      const resumeEsp = state.esp + 4 + argumentBytes;
      assert.equal(resumeEsp, 0x006fffb4);
      implementedServiceEvents.push({
        kind: "host-service-return",
        parentCallId: callbackFrame.id,
        rootCallId: rootApiFrame.id,
        requester: service.requester,
        module: service.module,
        symbol: service.symbol,
        variant: expectedPostQuitMessage.variant,
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
        arguments: [...arguments_],
        result: null,
        quitSequence: quitSlot.sequence,
        stackPointerBefore: state.esp,
        stackPointerAfter: resumeEsp,
      });
      resume(state, { eax: state.eax, esp: resumeEsp, eip: returnAddress });
      continue;
    }

    diagnostic = {
      kind: "unsupported-reentrant-host-service",
      parentCallId: callbackFrame.id,
      rootCallId: rootApiFrame.id,
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
      clientSurface: Object.seal({
        kind: "harness-local-logical-surface",
        bounds: expectedPaint.clientBounds,
        pixels: new Map(),
      }),
      parent: null,
      menu: null,
      ownerThreadId: mainGuestThread.id,
      updateRegion: null,
      erasePending: false,
      paintState: paintLifecycle.none,
      paintSequence: null,
      activePaintSession: null,
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

  if (isGetMessageA && implementedServiceEvents.length === 12) {
    assert.equal(hostCallFrames.length, 0);
    assert.equal(logicalWindowsByHandle.size, 0);
    assert.equal(retiredLogicalWindows.size, 1);
    assert.equal(service.iatAddress, expectedQuitMessage.iatAddress);
    assert.equal(service.thunkAddress, expectedQuitMessage.thunkAddress);
    assert.equal(callSite, expectedQuitMessage.callSite);
    assert.equal(returnAddress, expectedQuitMessage.returnAddress);
    assert.equal(callEncoding, "register-indirect-edi");
    assert.equal(state.esp, expectedQuitMessage.stackPointer);
    assert.equal(state.eax, 0);
    assert.equal(state.words[savedStateWord.esi], expectedDispatchMessage.thunkAddress);
    assert.equal(state.words[savedStateWord.edi], expectedQuitMessage.thunkAddress);
    const arguments_ = Array.from({ length: 4 }, (_, index) =>
      readGuestU32(state.esp + 4 + index * 4, `quit GetMessageA argument ${index}`),
    );
    assert.deepEqual(arguments_, expectedQuitMessage.arguments);
    const [messageAddress, windowFilter, minimumMessage, maximumMessage] = arguments_;
    assert.deepEqual([windowFilter, minimumMessage, maximumMessage], [0, 0, 0]);
    const quitSlot = mainGuestThread.messageState.quitSlot;
    assert(quitSlot);
    assert(Object.isFrozen(quitSlot));
    assert.equal(quitSlot.source, "PostQuitMessage");
    assert.equal(quitSlot.ownerThreadId, mainGuestThread.id);
    assert.equal(quitSlot.sequence, 2);
    assert.deepEqual(
      [quitSlot.hwnd, quitSlot.message, quitSlot.wParam, quitSlot.lParam],
      expectedQuitMessage.messageWords.slice(0, 4),
    );
    writeGuestWords(messageAddress, expectedQuitMessage.messageWords, "GetMessageA WM_QUIT MSG");
    mainGuestThread.messageState.quitSlot = null;
    const argumentBytes = arguments_.length * 4;
    const resumeEsp = state.esp + 4 + argumentBytes;
    assert.equal(resumeEsp, 0x006ffff4);
    implementedServiceEvents.push({
      kind: "host-service-return",
      requester: service.requester,
      module: service.module,
      symbol: service.symbol,
      variant: expectedQuitMessage.variant,
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
      arguments: [...arguments_],
      result: 0,
      materializedMessage: [...expectedQuitMessage.messageWords],
      quitSequence: quitSlot.sequence,
      stackPointerBefore: state.esp,
      stackPointerAfter: resumeEsp,
    });
    resume(state, { eax: 0, esp: resumeEsp, eip: returnAddress });
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

assert.equal(diagnostic, null, `unexpected runtime diagnostic: ${JSON.stringify(diagnostic)}`);
assert(finalState);
assert.deepEqual(completion, {
  kind: "fixture-entry-return",
  returnSentinel: guest.entryReturn,
  result: 0,
  stackPointer: guest.stackTop,
  instructionCount: 231,
});
assert.equal(executeStopCount, 83);
const expectedBudgetEvents = [
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
  { eip: 0x004012cf, esp: 0x006fffd4, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x004012d9, esp: 0x006fffd4, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x0040132b, esp: 0x006fffd4, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x0040133c, esp: 0x006fffd0, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x0040134d, esp: 0x006fffd4, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x00401356, esp: 0x006fffc8, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x0040136c, esp: 0x006fffd4, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x0040138c, esp: 0x006fffd0, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x004013d6, esp: 0x006fffd0, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x00401268, esp: 0x006fffb4, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x004012a3, esp: 0x006fffb4, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x004012b1, esp: 0x006fffb4, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x004012c3, esp: 0x006fffb4, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x00401311, esp: 0x006fffb4, esi: expectedCreateWindow.logicalHandle },
  { eip: expectedPostQuitMessage.thunkAddress, esp: 0x006fffac, esi: expectedCreateWindow.logicalHandle },
  { eip: guest.callbackReturn, esp: 0x006fffcc, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x004013e6, esp: 0x006fffd4, esi: expectedCreateWindow.logicalHandle },
  { eip: 0x00401406, esp: 0x006fffd8, esi: expectedDispatchMessage.thunkAddress },
  { eip: 0x004010fd, esp: 0x006fffe8, esi: expectedDispatchMessage.thunkAddress },
  ...[
    0x0040112f,
    0x00401138,
    0x00401169,
    0x00401177,
    0x00401187,
    0x00401191,
    0x0040119c,
    0x004011a6,
    0x004011b1,
    0x004011bc,
    0x004011ca,
    0x004011d5,
    0x004011e0,
    0x004011eb,
    0x004011f8,
    0x00401207,
    0x00401216,
    0x00401225,
    0x00401237,
    0x00401245,
    0x0040124f,
    0x00401179,
  ].map((eip) => ({
    eip,
    esp: 0x006ffff4,
    esi: expectedDispatchMessage.thunkAddress,
  })),
  { eip: guest.entryReturn, esp: guest.stackTop, esi: calleeSaved.esi },
];
assert.deepEqual(budgetEvents, expectedBudgetEvents);
assert.equal(budgetEvents.length, 66);
assert.equal(executeStopCount - budgetEvents.length, 17);
assert.equal(finalState.instructionCount, 231);

assert.equal(implementedServiceEvents.length, 13);
assert.deepEqual(
  implementedServiceEvents.map(({ module, symbol }) => `${module}!${symbol}`),
  [
    "kernel32.dll!GetModuleHandleA",
    "user32.dll!RegisterClassExA",
    "user32.dll!CreateWindowExA",
    "user32.dll!ShowWindow",
    "user32.dll!InvalidateRect",
    "user32.dll!GetMessageA",
    "user32.dll!BeginPaint",
    "gdi32.dll!SetPixel",
    "user32.dll!EndPaint",
    "user32.dll!PostQuitMessage",
    "user32.dll!DestroyWindow",
    "user32.dll!DispatchMessageA",
    "user32.dll!GetMessageA",
  ],
);
const beginPaintEvent = implementedServiceEvents[6];
assert.equal(beginPaintEvent.variant, expectedBeginPaint.variant);
assert.equal(beginPaintEvent.result, expectedPaint.logicalDc);
assert.deepEqual(
  [beginPaintEvent.parentCallId, beginPaintEvent.rootCallId, beginPaintEvent.paintSequence],
  [4, 3, 1],
);
assert.deepEqual(beginPaintEvent.paintStructWords, expectedPaint.paintStructWords);
assert.deepEqual(beginPaintEvent.clientBounds, expectedPaint.clientBounds);
const setPixelEvent = implementedServiceEvents[7];
assert.equal(setPixelEvent.result, 0x000000ff);
assert.deepEqual(setPixelEvent.arguments, expectedSetPixel.arguments);
assert.deepEqual(
  [setPixelEvent.parentCallId, setPixelEvent.rootCallId, setPixelEvent.paintSequence],
  [4, 3, 1],
);
const endPaintEvent = implementedServiceEvents[8];
assert.equal(endPaintEvent.result, 1);
assert.equal(endPaintEvent.paintSessionId, beginPaintEvent.paintSessionId);
assert.deepEqual(
  [endPaintEvent.parentCallId, endPaintEvent.rootCallId, endPaintEvent.paintSequence],
  [4, 3, 1],
);
const postQuitEvent = implementedServiceEvents[9];
assert.equal(postQuitEvent.result, null);
assert.equal(postQuitEvent.quitSequence, 2);
assert.deepEqual([postQuitEvent.parentCallId, postQuitEvent.rootCallId], [6, 3]);
const destroyEvent = implementedServiceEvents[10];
assert.deepEqual(
  [
    destroyEvent.callId,
    destroyEvent.parentCallId,
    destroyEvent.rootCallId,
    destroyEvent.callbackCallId,
    destroyEvent.callbackMessage,
    destroyEvent.result,
  ],
  [5, 4, 3, 6, 2, 1],
);
const dispatchEvent = implementedServiceEvents[11];
assert.deepEqual(
  [dispatchEvent.callId, dispatchEvent.callbackCallId, dispatchEvent.callbackMessage, dispatchEvent.result],
  [3, 4, 0x000f, 0],
);
const quitGetMessageEvent = implementedServiceEvents[12];
assert.equal(quitGetMessageEvent.callEncoding, "register-indirect-edi");
assert.equal(quitGetMessageEvent.result, 0);
assert.deepEqual(quitGetMessageEvent.materializedMessage, expectedQuitMessage.messageWords);

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
  {
    kind: "guest-callback-enter",
    callId: 6,
    parentCallId: 5,
    role: "window-procedure-wm-destroy",
    target: expectedWindowClass.wndProc,
    returnSentinel: guest.callbackReturn,
    arguments: [expectedCreateWindow.logicalHandle, 2, 0, 0],
    stackPointer: expectedDestroyWindow.callbackStackPointer,
    instructionCount: 124,
  },
  {
    kind: "guest-callback-return",
    callId: 6,
    parentCallId: 5,
    role: "window-procedure-wm-destroy",
    target: expectedWindowClass.wndProc,
    result: 0,
    stackPointer: expectedDestroyWindow.callbackReturnStackPointer,
    instructionCount: 147,
  },
  {
    kind: "guest-callback-return",
    callId: 4,
    parentCallId: 3,
    role: "window-procedure-wm-paint",
    target: expectedDispatchMessage.callbackTarget,
    result: 0,
    stackPointer: expectedDispatchMessage.callbackReturnStackPointer,
    instructionCount: 155,
  },
]);
assert.equal(hostCallFrames.length, 0);
assert.equal(nextHostCallId, 7);

assert.equal(logicalClassesByKey.size, 1);
assert.equal(logicalClassesByAtom.size, 1);
const logicalClass = logicalClassesByAtom.get(0xc000);
assert(logicalClass);
assert.equal(logicalClassesByKey.get(logicalClass.key), logicalClass);
assert(Object.isFrozen(logicalClass));
assert(Object.isFrozen(logicalClass.wndProcCallback));
assert.equal(logicalWindowsByHandle.size, 0);
assert.equal(retiredLogicalWindows.size, 1);
const logicalWindow = retiredLogicalWindows.get(expectedCreateWindow.logicalHandle);
assert(logicalWindow);
assert(Object.isSealed(logicalWindow));
assert.equal(logicalWindow.lifecycle, "destroyed");
assert.equal(logicalWindow.visible, false);
assert.equal(logicalWindow.updateRegion, null);
assert.equal(logicalWindow.erasePending, false);
assert.equal(logicalWindow.paintState, paintLifecycle.none);
assert.equal(logicalWindow.paintSequence, null);
assert.equal(logicalWindow.activePaintSession, null);
assert.deepEqual(logicalWindow.clientSurface.bounds, expectedPaint.clientBounds);
assert.equal(logicalWindow.clientSurface.pixels.size, 1);
assert.equal(logicalWindow.clientSurface.pixels.get("1,1"), 0x000000ff);
assert.deepEqual(logicalWindow.requestedOuterSize, { width: 160, height: 120 });
assert.notDeepEqual(logicalWindow.clientSurface.bounds, {
  left: 0,
  top: 0,
  right: logicalWindow.requestedOuterSize.width,
  bottom: logicalWindow.requestedOuterSize.height,
});

assert.equal(logicalPaintDcsByHandle.size, 0);
assert.equal(retiredPaintDcs.size, 1);
const retiredDc = retiredPaintDcs.get(expectedPaint.logicalDc);
assert(retiredDc);
assert.equal(retiredDc.lifecycle, "retired");
assert.equal(retiredDc.windowHandle, logicalWindow.handle);
assert.equal(retiredDc.paintSequence, 1);
assert.equal(retiredPaintSessions.length, 1);
const [retiredPaintSession] = retiredPaintSessions;
assert.equal(retiredPaintSession.lifecycle, "retired");
assert.equal(retiredPaintSession.dc, retiredDc);
assert.equal(retiredPaintSession.paintSequence, 1);
assert.equal(retiredPaintSession.updateRegion.kind, "whole-client");
assert.equal(nextPaintDcSlot, 2);
assert.equal(nextPaintSessionId, 2);

assert(Object.isSealed(mainGuestThread));
assert(Object.isSealed(mainGuestThread.messageState));
assert.equal(mainGuestThread.messageState.nextSequence, 3);
assert.equal(mainGuestThread.messageState.generatedPaintLane.length, 0);
assert.equal(mainGuestThread.messageState.inFlightPaint, null);
assert.equal(mainGuestThread.messageState.quitSlot, null);
assert.deepEqual(
  Array.from({ length: 7 }, (_, index) =>
    readGuestU32(expectedGetMessage.arguments[0] + index * 4, `final MSG word ${index}`),
  ),
  expectedQuitMessage.messageWords,
);
assert.deepEqual(
  Array.from({ length: 16 }, (_, index) =>
    readGuestU32(
      expectedBeginPaint.arguments[1] + index * 4,
      `final PAINTSTRUCT word ${index}`,
    ),
  ),
  expectedPaint.paintStructWords,
);

for (const [address, label, expected] of [
  [0x00403050, "dispatch count", 1],
  [0x00403054, "dispatched paint count", 1],
  [0x00403058, "window failure code", 0],
  [0x0040305c, "WM_CREATE count", 1],
  [0x00403060, "WM_PAINT count", 1],
  [0x00403064, "WM_DESTROY count", 1],
  [0x00403068, "window event count", 3],
  [0x0040306c, "first window event", 1],
  [0x00403070, "second window event", 0x000f],
  [0x00403074, "third window event", 2],
  [0x00403078, "BeginPaint count", 1],
  [0x0040307c, "EndPaint count", 1],
  [0x00403080, "SetPixel count", 1],
  [0x00403084, "destroy success count", 1],
  [0x00403088, "PostQuitMessage count", 1],
  [0x00403000, "pixel result", 0x000000ff],
]) {
  assert.equal(readGuestU32(address, `final ${label}`), expected);
}

assert.equal(finalState.eax, 0);
assert.equal(finalState.words[savedStateWord.ebx], calleeSaved.ebx);
assert.equal(finalState.words[savedStateWord.ebp], calleeSaved.ebp);
assert.equal(finalState.words[savedStateWord.esi], calleeSaved.esi);
assert.equal(finalState.words[savedStateWord.edi], calleeSaved.edi);
assert.equal(finalState.esp, guest.stackTop);
assert.equal(finalState.eip, guest.entryReturn);
assert.equal(finalState.previousEip, guest.entryReturn);
assert.equal(readGuestU32(0x006ffff4, "saved entry ESI"), calleeSaved.esi);
assert.equal(readGuestU32(0x006ffff8, "saved entry EDI"), calleeSaved.edi);
assert.equal(readGuestU32(0x006ffffc, "entry return sentinel"), guest.entryReturn);
assert.deepEqual(
  memory.slice(guest.callbackReturn, guest.callbackReturn + 2),
  Uint8Array.from([0x0f, 0x0b]),
);
assert.deepEqual(Object.keys(hostImportCalls), ["microtick"]);
assert(hostImportCalls.microtick > 0);

assert.equal(browserPresentEvents.length, 1);
const [browserPresentEvent] = browserPresentEvents;
assert.equal(browserPresentEvent.source.paintSequence, retiredPaintSession.paintSequence);
console.log(`${browserPresentEventLogPrefix}${JSON.stringify(browserPresentEvent)}`);

console.log(
  `runtime window paint and quit flow passed: imports=${resolvedImports.length}, ` +
    `completed-variants=${implementedServiceEvents.length}, callback-events=${callbackEvents.length}, ` +
    `hwnd=0x${logicalWindow.handle.toString(16)}, hdc=0x${retiredDc.handle.toString(16)}, ` +
    `pixel=0x${logicalWindow.clientSurface.pixels.get("1,1").toString(16)}, ` +
    `budget stops=${budgetEvents.length}, execute stops=${executeStopCount}, ` +
    `instructions=${finalState.instructionCount}`,
);
