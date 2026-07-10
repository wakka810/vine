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
  entryReturn: 0x00500f00,
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
  guest.stackBase,
  guest.stackTop - guest.stackBase,
  "guest stack",
);
assert.equal(guest.stackTop - guest.stackBase, 0x100000);
assert(pe.imageBase + pe.sizeOfImage <= guest.trapPage, "PE image overlaps trap page");
assert(guest.trapPage + 0x1000 <= guest.stackBase, "trap page overlaps guest stack");
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
    esp: words[savedStateWord.esp],
    eip: words[savedStateWord.instructionPointer],
    previousEip: words[savedStateWord.previousInstructionPointer],
  };
};
const assertCalleeSaved = (words) => {
  assert.equal(words[savedStateWord.ebx], calleeSaved.ebx);
  assert.equal(words[savedStateWord.ebp], calleeSaved.ebp);
  assert.equal(words[savedStateWord.esi], calleeSaved.esi);
  assert.equal(words[savedStateWord.edi], calleeSaved.edi);
};

const budgetEvents = [];
let diagnostic = null;
let finalState = null;
for (let stopCount = 0; stopCount < 16 && diagnostic === null; stopCount += 1) {
  const reason = wasm.vine_execute_budget(guest.instructionBudget);
  const state = saveState(reason);
  assertCalleeSaved(state.words);

  if (reason === stopReason.budget) {
    budgetEvents.push({ eip: state.eip, esp: state.esp });
    continue;
  }

  assert.equal(
    reason,
    stopReason.invalidOpcode,
    `unexpected stop reason 0x${reason.toString(16)} at 0x${state.eip.toString(16)}`,
  );
  assert.equal(state.errorCode, -1);
  assert.notEqual(state.eip, guest.entryReturn, "fixture returned before host dispatch");
  const service = ownedServices.get(state.eip);
  assert(service, `#UD at unowned guest address 0x${state.eip.toString(16)}`);
  assert.equal(state.previousEip, state.eip);
  assert.equal(state.esp, 0x006fffec);

  const returnAddress = readGuestU32(state.esp, "host service return address");
  const callInstructionSize = 6;
  const callSite = returnAddress - callInstructionSize;
  assertGuestRange(callSite, callInstructionSize, "guest import call");
  const callSection = pe.sections.find(
    ({ virtualAddress, virtualSize, characteristics }) =>
      (characteristics & 0x20000000) !== 0 &&
      callSite >= pe.imageBase + virtualAddress &&
      returnAddress <= pe.imageBase + virtualAddress + virtualSize,
  );
  assert(callSection, "guest import call is outside an executable image section");
  assert.deepEqual(
    memory.slice(callSite, callSite + 2),
    Uint8Array.from([0xff, 0x15]),
    "guest return does not follow CALL DWORD PTR [absolute]",
  );
  assert.equal(
    readGuestU32(callSite + 2, "guest import call IAT operand"),
    service.iatAddress,
  );
  assert.equal(
    readGuestU32(service.iatAddress, "guest import call resolved IAT"),
    service.thunkAddress,
  );
  assert.equal(readGuestU32(state.esp + 4, "GetModuleHandleA argument"), 0);

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
assert.deepEqual(budgetEvents, [{ eip: 0x00401004, esp: 0x006ffff0 }]);
assert.equal(finalState.instructionCount, 5);
assert.deepEqual(diagnostic, {
  kind: "unsupported-host-service",
  requester: "window-pixel.exe",
  module: "kernel32.dll",
  symbol: "GetModuleHandleA",
  guestCallSite: 0x00401004,
  guestCallSiteRva: 0x1004,
  guestReturnAddress: 0x0040100a,
  guestReturnRva: 0x100a,
  iatAddress: 0x004020c0,
  iatRva: 0x20c0,
  thunkAddress: 0x00500000,
  engineStopReason: stopReason.invalidOpcode,
  engineErrorCode: -1,
  stackPointer: 0x006fffec,
});
assert.equal(readGuestU32(0x00403004, "unreached window class size"), 0);
assert.deepEqual(Object.keys(hostImportCalls), ["microtick"]);
assert(hostImportCalls.microtick > 0);

console.log(
  `runtime window loader slice passed: imports=${resolvedImports.length}, ` +
    `stop=${diagnostic.kind}, service=${diagnostic.module}!${diagnostic.symbol}, ` +
    `call=0x${diagnostic.guestCallSite.toString(16)}, ` +
    `budget stops=${budgetEvents.length}, instructions=${finalState.instructionCount}`,
);
