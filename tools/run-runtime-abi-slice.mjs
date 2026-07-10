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
  "abi-smoke.exe",
);
const wasmPath = path.resolve(process.argv[2] ?? defaultWasmPath);
const exePath = path.resolve(process.argv[3] ?? defaultExePath);

const expectedExeSha256 =
  "60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7";
const expectedWasm = {
  size: 1_882_836,
  sha256: "f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26",
  importCount: 22,
  exportCount: 809,
};
const expected = {
  imageBase: 0x00400000,
  entry: 0x00401000,
  addIat: 0x00402034,
  callbackIat: 0x00402038,
  callback: 0x00401030,
  result: 0x00403000,
};
const guest = {
  memorySize: 0x00800000,
  trapPage: 0x00500000,
  addThunk: 0x00500000,
  callbackThunk: 0x00500010,
  callbackReturn: 0x00500100,
  entryReturn: 0x00500110,
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
  checkedEnd(directoryOffset, directoryCount * 8, optionalOffset + optionalSize, "data directories");
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
    const section = sections.find(({ virtualAddress, rawSize }) =>
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
  assert(importDirectory.rva !== 0 && importDirectory.size >= 20, "missing import directory");
  const importEnd = importDirectory.rva + importDirectory.size;
  for (let descriptorRva = importDirectory.rva; ; descriptorRva += 20) {
    assert(descriptorRva + 20 <= importEnd, "unterminated import descriptor table");
    const descriptorOffset = rvaToFileOffset(descriptorRva, 20, "import descriptor");
    const originalFirstThunk = u32(descriptorOffset, "import lookup RVA");
    const nameRva = u32(descriptorOffset + 12, "import DLL name RVA");
    const firstThunk = u32(descriptorOffset + 16, "import IAT RVA");
    if (originalFirstThunk === 0 && nameRva === 0 && firstThunk === 0) {
      break;
    }
    assert(nameRva !== 0 && firstThunk !== 0, "incomplete import descriptor");
    const dll = cStringAtRva(nameRva, "import DLL name");
    const lookupRva = originalFirstThunk || firstThunk;
    for (let index = 0; ; index += 1) {
      const lookupOffset = rvaToFileOffset(
        lookupRva + index * 4,
        4,
        "import lookup entry",
      );
      const lookup = u32(lookupOffset, "import lookup value");
      if (lookup === 0) {
        break;
      }
      assert.equal(lookup >>> 31, 0, "ordinal import is outside this W1 fixture");
      const hintNameOffset = rvaToFileOffset(lookup, 3, "import hint/name");
      imports.push({
        dll,
        name: cStringAtOffset(hintNameOffset + 2, "import name"),
        iatRva: firstThunk + index * 4,
      });
    }
  }

  return {
    entryRva,
    imageBase,
    sizeOfImage,
    sizeOfHeaders,
    importDirectory,
    iatDirectory,
    sections,
    imports,
  };
}

requireFile(wasmPath, "npm run prepare:engine");
requireFile(exePath, "npm run check:toolchain");

const exeBytes = fs.readFileSync(exePath);
assert.equal(
  crypto.createHash("sha256").update(exeBytes).digest("hex"),
  expectedExeSha256,
  "abi-smoke.exe does not match the fixed W1 fixture",
);
const pe = parsePe32(exeBytes);
assert.equal(pe.imageBase, expected.imageBase);
assert.equal(pe.imageBase + pe.entryRva, expected.entry);
assert.equal(pe.sizeOfImage, 0x5000);
assert.equal(pe.sizeOfHeaders, 0x400);
assert(pe.sizeOfHeaders <= exeBytes.byteLength, "PE headers exceed the fixture file");
assert.deepEqual(pe.importDirectory, { rva: 0x2000, size: 0x28 });
assert.deepEqual(
  pe.imports.map(({ dll, name, iatRva }) => ({
    dll: dll.toLowerCase(),
    name,
    iatRva,
  })),
  [
    { dll: "vine_fixture.dll", name: "fixture_add", iatRva: 0x2034 },
    { dll: "vine_fixture.dll", name: "fixture_call_callback", iatRva: 0x2038 },
  ],
);
assert.equal(pe.imageBase + pe.imports[0].iatRva, expected.addIat);
assert.equal(pe.imageBase + pe.imports[1].iatRva, expected.callbackIat);
assert.deepEqual(pe.iatDirectory, { rva: 0x2034, size: 0x0c });
assert.deepEqual(pe.sections, [
  {
    name: ".text",
    virtualSize: 0x36,
    virtualAddress: 0x1000,
    rawSize: 0x200,
    rawOffset: 0x400,
    characteristics: 0x60000020,
  },
  {
    name: ".rdata",
    virtualSize: 0x77,
    virtualAddress: 0x2000,
    rawSize: 0x200,
    rawOffset: 0x600,
    characteristics: 0x40000040,
  },
  {
    name: ".data",
    virtualSize: 0x4,
    virtualAddress: 0x3000,
    rawSize: 0,
    rawOffset: 0,
    characteristics: 0xc0000040,
  },
  {
    name: ".reloc",
    virtualSize: 0x14,
    virtualAddress: 0x4000,
    rawSize: 0x200,
    rawOffset: 0x800,
    characteristics: 0x42000040,
  },
]);
const dataSection = pe.sections.find(({ name }) => name === ".data");
assert(dataSection, "fixture has no .data section");
assert.equal(dataSection.virtualAddress, 0x3000);
assert.equal(dataSection.virtualSize, 4);
assert.equal(dataSection.rawSize, 0);
const textSection = pe.sections.find(({ name }) => name === ".text");
assert(textSection, "fixture has no .text section");
assert(textSection.characteristics & 0x20000000, "fixture .text is not executable");
assert(
  expected.callback >= pe.imageBase + textSection.virtualAddress &&
    expected.callback < pe.imageBase + textSection.virtualAddress + textSection.virtualSize,
  "guest callback is outside executable fixture code",
);

const wasmBytes = fs.readFileSync(wasmPath);
assert.equal(wasmBytes.byteLength, expectedWasm.size, "unexpected adapter Wasm size");
assert.equal(
  crypto.createHash("sha256").update(wasmBytes).digest("hex"),
  expectedWasm.sha256,
  "adapter Wasm does not match the pinned W1 artifact",
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
assert.equal(readGuestU32(expected.result, "zero-filled result"), 0);
assert.equal(
  readGuestU32(expected.addIat, "unpatched fixture_add IAT"),
  0x2040,
);
assert.equal(
  readGuestU32(expected.callbackIat, "unpatched callback IAT"),
  0x204e,
);
assert.equal(
  readGuestU32(pe.imageBase + pe.iatDirectory.rva + 8, "IAT terminator"),
  0,
);

memory.fill(0xf4, guest.trapPage, guest.trapPage + 0x1000);
for (const address of [
  guest.addThunk,
  guest.callbackThunk,
  guest.callbackReturn,
  guest.entryReturn,
]) {
  memory.set(Uint8Array.from([0x0f, 0x0b]), address);
}
writeGuestU32(expected.addIat, guest.addThunk, "fixture_add IAT");
writeGuestU32(expected.callbackIat, guest.callbackThunk, "fixture callback IAT");
assert.equal(readGuestU32(expected.addIat, "fixture_add IAT"), guest.addThunk);
assert.equal(readGuestU32(expected.callbackIat, "fixture callback IAT"), guest.callbackThunk);
assert.equal(
  readGuestU32(pe.imageBase + pe.iatDirectory.rva + 8, "patched IAT terminator"),
  0,
);
for (const address of [
  guest.addThunk,
  guest.callbackThunk,
  guest.callbackReturn,
  guest.entryReturn,
]) {
  assert.deepEqual(memory.slice(address, address + 2), Uint8Array.from([0x0f, 0x0b]));
}

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
privateState.setUint32(privateStateOffset.instructionPointer, expected.entry, true);
privateState.setUint32(
  privateStateOffset.previousInstructionPointer,
  expected.entry,
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
  state.words[savedStateWord.eax] = eax >>> 0;
  state.words[savedStateWord.esp] = esp >>> 0;
  state.words[savedStateWord.instructionPointer] = eip >>> 0;
  state.words[savedStateWord.previousInstructionPointer] = eip >>> 0;
  assert.equal(wasm.vine_cpu_state_restore(), 0);
};
const assertCalleeSaved = (words) => {
  assert.equal(words[savedStateWord.ebx], calleeSaved.ebx);
  assert.equal(words[savedStateWord.ebp], calleeSaved.ebp);
  assert.equal(words[savedStateWord.esi], calleeSaved.esi);
  assert.equal(words[savedStateWord.edi], calleeSaved.edi);
};

const ownedTraps = new Map([
  [guest.addThunk, "fixture_add"],
  [guest.callbackThunk, "fixture_call_callback"],
  [guest.callbackReturn, "callback_return"],
  [guest.entryReturn, "entry_return"],
]);
const events = [];
let budgetStops = 0;
let callbackFrame = null;
let finalState = null;

for (let stopCount = 0; stopCount < 64 && finalState === null; stopCount += 1) {
  const reason = wasm.vine_execute_budget(guest.instructionBudget);
  const state = saveState(reason);
  assertCalleeSaved(state.words);

  if (reason === stopReason.budget) {
    budgetStops += 1;
    events.push({ kind: "budget", eip: state.eip, esp: state.esp });
    continue;
  }

  const trap = ownedTraps.get(state.eip);
  assert.equal(
    reason,
    stopReason.invalidOpcode,
    `unexpected stop reason 0x${reason.toString(16)} at 0x${state.eip.toString(16)}`,
  );
  assert.equal(state.errorCode, -1);
  assert(trap, `#UD at unowned guest address 0x${state.eip.toString(16)}`);
  assert.equal(state.previousEip, state.eip);

  if (trap === "fixture_add") {
    assert.equal(state.esp, 0x006ffff0);
    const returnAddress = readGuestU32(state.esp, "fixture_add return address");
    const left = readGuestU32(state.esp + 4, "fixture_add left");
    const right = readGuestU32(state.esp + 8, "fixture_add right");
    assert.equal(returnAddress, 0x0040100a);
    assert.deepEqual([left, right], [19, 23]);
    events.push({ kind: trap, args: [left, right], result: left + right });
    resume(state, {
      eax: left + right,
      esp: state.esp + 4,
      eip: returnAddress,
    });
    continue;
  }

  if (trap === "fixture_call_callback") {
    assert.equal(state.esp, 0x006ffff0);
    assert.equal(callbackFrame, null);
    const returnAddress = readGuestU32(state.esp, "callback service return address");
    const target = readGuestU32(state.esp + 4, "callback target");
    const value = readGuestU32(state.esp + 8, "callback value");
    assert.equal(returnAddress, 0x00401019);
    assert.equal(target, expected.callback);
    assert.equal(value, 42);
    callbackFrame = { outerEsp: state.esp, returnAddress };
    writeGuestU32(state.esp - 4, value, "callback argument");
    writeGuestU32(state.esp - 8, guest.callbackReturn, "callback return sentinel");
    events.push({ kind: trap, target, value });
    resume(state, {
      esp: state.esp - 8,
      eip: target,
    });
    continue;
  }

  if (trap === "callback_return") {
    assert(callbackFrame, "callback returned without an active host call");
    assert.equal(state.esp, callbackFrame.outerEsp - 4);
    assert.equal(state.eax, 43);
    events.push({ kind: trap, result: state.eax });
    const { outerEsp, returnAddress } = callbackFrame;
    callbackFrame = null;
    resume(state, {
      eax: 43,
      esp: outerEsp + 4,
      eip: returnAddress,
    });
    continue;
  }

  assert.equal(trap, "entry_return");
  assert.equal(callbackFrame, null);
  assert.equal(state.eax, 0);
  assert.equal(state.esp, guest.stackTop);
  assert.equal(readGuestU32(expected.result, "fixture result"), 43);
  events.push({ kind: trap, result: state.eax });
  finalState = state;
}

assert(finalState, "runtime ABI slice did not reach the entry return sentinel");
assert.equal(budgetStops, 5, "unexpected bounded scheduler yield count");
assert.equal(finalState.instructionCount, 21, "unexpected guest instruction count");
assert.equal(finalState.eip, guest.entryReturn);
assert.equal(finalState.previousEip, guest.entryReturn);
assert.deepEqual(
  events.filter(({ kind }) => kind === "budget").map(({ eip }) => eip),
  [0x00500000, 0x00401013, 0x00500100, 0x00401027, 0x0040102f],
);
assert.deepEqual(
  events.filter(({ kind }) => kind !== "budget").map(({ kind }) => kind),
  ["fixture_add", "fixture_call_callback", "callback_return", "entry_return"],
);
assert.deepEqual(Object.keys(hostImportCalls), ["microtick"]);
assert(hostImportCalls.microtick > 0);

console.log(
  `runtime ABI slice passed: result=43, EAX=0, ESP=0x${finalState.esp.toString(16)}, ` +
    `budget stops=${budgetStops}, instructions=${finalState.instructionCount}`,
);
