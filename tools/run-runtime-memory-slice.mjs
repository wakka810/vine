import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const wasmPath = path.resolve(
  process.argv[2] ??
    path.join(root, ".work", "v86-adapter-source", "build", "v86-fallback.wasm"),
);
const fixturePath = path.resolve(
  process.argv[3] ??
    path.join(root, ".work", "toolchain-spike", "high-memory.exe"),
);
const outputDirectory = path.join(root, ".work", "v86-memory-spike");

const expected = {
  sourceRevision: "2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f",
  wasm: {
    bytes: 1_882_836,
    sha256: "f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26",
    imports: 22,
    exports: 809,
  },
  fixture: {
    bytes: 3_072,
    sha256: "084bca4dbd9c60a178f751e7c6a87e0b2865fd45cdff8d6135468886316a7aa7",
    imageBase: 0xf1000000,
    imageSize: 0x6000,
    entry: 0xf1001000,
  },
  exports: {
    fixture_cross_page_write: 0xf1001060,
    fixture_entry: 0xf1001000,
    fixture_nonexec_code: 0xf1004000,
    fixture_read_nonpresent: 0xf1001040,
    fixture_readonly: 0xf1005000,
    fixture_return_trap: 0xf1001070,
    fixture_writable: 0xf1003000,
    fixture_write_nonpresent: 0xf1001050,
    fixture_write_readonly: 0xf1001020,
    fixture_write_writable: 0xf1001030,
  },
};

const stateOffset = {
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

const page = {
  size: 0x1000,
  present: 1,
  writable: 2,
  user: 4,
  accessed: 0x20,
  dirty: 0x40,
};

const guest = {
  physicalPoolBytes: 2 * 1024 * 1024,
  stackBase: 0xdfffc000,
  stackTop: 0xe0000000,
  nonpresentAddress: 0xe1000000,
  crossPageAddress: 0xe2000ffe,
  crossPageBase: 0xe2000000,
};

const stopReason = {
  budget: 1,
  invalidOpcode: 0x106,
  pageFault: 0x10e,
};

const permission = {
  read: 0x40000000,
  write: 0x80000000,
  execute: 0x20000000,
};

const sha256 = (bytes) =>
  crypto.createHash("sha256").update(bytes).digest("hex");

function requireFile(file, prerequisite) {
  if (!fs.existsSync(file)) {
    console.error(`required file not found: ${path.relative(root, file)}`);
    console.error(`Run ${prerequisite} first.`);
    process.exit(1);
  }
}

function checkedEnd(start, size, limit, label) {
  assert(Number.isSafeInteger(start) && start >= 0, `${label} start is invalid`);
  assert(Number.isSafeInteger(size) && size >= 0, `${label} size is invalid`);
  const end = start + size;
  assert(Number.isSafeInteger(end) && end <= limit, `${label} exceeds its backing data`);
  return end;
}

function checkedGuestEnd(address, size, label) {
  return checkedEnd(address, size, 0x1_0000_0000, label);
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
    assert(offset < bytes.byteLength, `${label} starts outside the image`);
    let end = offset;
    while (end < bytes.byteLength && bytes[end] !== 0) {
      end += 1;
    }
    assert(end < bytes.byteLength, `${label} is unterminated`);
    return Buffer.from(bytes.subarray(offset, end)).toString("ascii");
  };

  assert.equal(u16(0, "DOS magic"), 0x5a4d);
  const peOffset = u32(0x3c, "PE header offset");
  assert.equal(u32(peOffset, "PE signature"), 0x00004550);
  const coffOffset = peOffset + 4;
  assert.equal(u16(coffOffset, "machine"), 0x014c);
  const sectionCount = u16(coffOffset + 2, "section count");
  const characteristics = u16(coffOffset + 18, "COFF characteristics");
  const optionalSize = u16(coffOffset + 16, "optional header size");
  const optionalOffset = coffOffset + 20;
  checkedEnd(optionalOffset, optionalSize, bytes.byteLength, "optional header");
  assert.equal(u16(optionalOffset, "optional magic"), 0x010b);
  assert(optionalSize >= 224, "PE32 optional header is too small");

  const entryRva = u32(optionalOffset + 16, "entry RVA");
  const imageBase = u32(optionalOffset + 28, "image base");
  const sectionAlignment = u32(optionalOffset + 32, "section alignment");
  const fileAlignment = u32(optionalOffset + 36, "file alignment");
  const sizeOfImage = u32(optionalOffset + 56, "image size");
  const sizeOfHeaders = u32(optionalOffset + 60, "header size");
  const dllCharacteristics = u16(optionalOffset + 70, "DLL characteristics");
  const directoryCount = u32(optionalOffset + 92, "directory count");
  assert(directoryCount >= 1, "PE32 image has no export directory slot");
  const directoryOffset = optionalOffset + 96;
  checkedEnd(
    directoryOffset,
    directoryCount * 8,
    optionalOffset + optionalSize,
    "data directories",
  );
  const exportDirectory = {
    rva: u32(directoryOffset, "export directory RVA"),
    size: u32(directoryOffset + 4, "export directory size"),
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

  assert(exportDirectory.rva !== 0 && exportDirectory.size >= 40);
  const exportOffset = rvaToFileOffset(
    exportDirectory.rva,
    40,
    "export directory",
  );
  const ordinalBase = u32(exportOffset + 16, "export ordinal base");
  const functionCount = u32(exportOffset + 20, "export function count");
  const nameCount = u32(exportOffset + 24, "export name count");
  const functionTableRva = u32(exportOffset + 28, "export function table RVA");
  const nameTableRva = u32(exportOffset + 32, "export name table RVA");
  const ordinalTableRva = u32(exportOffset + 36, "export ordinal table RVA");
  assert.equal(functionCount, nameCount, "fixture exports must all be named");

  const exports = {};
  for (let index = 0; index < nameCount; index += 1) {
    const nameRvaOffset = rvaToFileOffset(
      nameTableRva + index * 4,
      4,
      "export name RVA",
    );
    const ordinalOffset = rvaToFileOffset(
      ordinalTableRva + index * 2,
      2,
      "export ordinal",
    );
    const name = cStringAtRva(u32(nameRvaOffset, "export name RVA"), "export name");
    const ordinalIndex = u16(ordinalOffset, "export ordinal index");
    assert(ordinalIndex < functionCount, `${name} has an invalid export ordinal`);
    const functionOffset = rvaToFileOffset(
      functionTableRva + ordinalIndex * 4,
      4,
      "export function RVA",
    );
    const rva = u32(functionOffset, "export function RVA");
    assert(!(name in exports), `duplicate export ${name}`);
    exports[name] = {
      ordinal: ordinalBase + ordinalIndex,
      rva,
      address: (imageBase + rva) >>> 0,
    };
  }

  return {
    characteristics,
    dllCharacteristics,
    entryRva,
    entry: (imageBase + entryRva) >>> 0,
    exportDirectory,
    exports,
    fileAlignment,
    imageBase,
    sectionAlignment,
    sections,
    sizeOfHeaders,
    sizeOfImage,
  };
}

class SparseFault extends Error {
  constructor({ address, kind, present }) {
    const write = kind === "write";
    const instructionFetch = kind === "execute";
    const errorCode =
      (present ? 1 : 0) | (write ? 2 : 0) | 4 | (instructionFetch ? 16 : 0);
    super(
      `sparse ${kind} fault at 0x${address.toString(16)} ` +
        `(error 0x${errorCode.toString(16)})`,
    );
    this.address = address >>> 0;
    this.kind = kind;
    this.present = present;
    this.errorCode = errorCode;
  }
}

class CheckedSparseMemory {
  constructor() {
    this.pages = new Map();
  }

  mapPage(virtualBase, permissions, bytes) {
    checkedGuestEnd(virtualBase, page.size, "sparse page");
    assert.equal(virtualBase & (page.size - 1), 0);
    assert.equal(bytes.byteLength, page.size);
    assert(!this.pages.has(virtualBase), `duplicate sparse page 0x${virtualBase.toString(16)}`);
    this.pages.set(virtualBase, {
      permissions: { ...permissions },
      bytes: Uint8Array.from(bytes),
    });
  }

  preflight(address, size, kind) {
    assert(Number.isInteger(size) && size > 0);
    const end = checkedGuestEnd(address, size, `sparse ${kind}`);
    const pages = [];
    for (
      let current = Math.floor(address / page.size) * page.size;
      current < end;
      current += page.size
    ) {
      const mapped = this.pages.get(current >>> 0);
      if (!mapped) {
        throw new SparseFault({ address: current >>> 0, kind, present: false });
      }
      if (!mapped.permissions[kind === "execute" ? "execute" : kind]) {
        throw new SparseFault({ address: address >>> 0, kind, present: true });
      }
      pages.push({ virtualBase: current >>> 0, mapped });
    }
    return pages;
  }

  readU32(address) {
    this.preflight(address, 4, "read");
    let value = 0;
    for (let index = 0; index < 4; index += 1) {
      const byteAddress = (address + index) >>> 0;
      const virtualBase = Math.floor(byteAddress / page.size) * page.size;
      const mapped = this.pages.get(virtualBase);
      value |= mapped.bytes[byteAddress & (page.size - 1)] << (index * 8);
    }
    return value >>> 0;
  }

  writeU32(address, value) {
    this.preflight(address, 4, "write");
    for (let index = 0; index < 4; index += 1) {
      const byteAddress = (address + index) >>> 0;
      const virtualBase = Math.floor(byteAddress / page.size) * page.size;
      const mapped = this.pages.get(virtualBase);
      mapped.bytes[byteAddress & (page.size - 1)] = value >>> (index * 8);
    }
  }

  checkExecute(address) {
    this.preflight(address, 1, "execute");
  }

  bytesAt(address, size) {
    const result = [];
    for (let index = 0; index < size; index += 1) {
      const byteAddress = (address + index) >>> 0;
      const virtualBase = Math.floor(byteAddress / page.size) * page.size;
      const mapped = this.pages.get(virtualBase);
      assert(mapped, `sparse byte 0x${byteAddress.toString(16)} is unmapped`);
      result.push(mapped.bytes[byteAddress & (page.size - 1)]);
    }
    return result;
  }
}

function captureSparseFault(callback) {
  try {
    callback();
  } catch (error) {
    assert(error instanceof SparseFault, `unexpected sparse error: ${error}`);
    return {
      address: error.address,
      kind: error.kind,
      present: error.present,
      errorCode: error.errorCode,
    };
  }
  assert.fail("expected sparse access to fault");
}

requireFile(wasmPath, "npm run prepare:engine");
requireFile(fixturePath, "npm run check:toolchain");

const fixtureBytes = fs.readFileSync(fixturePath);
assert.equal(fixtureBytes.byteLength, expected.fixture.bytes);
assert.equal(sha256(fixtureBytes), expected.fixture.sha256);
const pe = parsePe32(fixtureBytes);
assert.equal(pe.imageBase, expected.fixture.imageBase);
assert.equal(pe.sizeOfImage, expected.fixture.imageSize);
assert.equal(pe.entry, expected.fixture.entry);
assert.equal(pe.sectionAlignment, page.size);
assert.equal(pe.fileAlignment, 0x200);
assert.equal(pe.sizeOfHeaders, 0x400);
assert(pe.characteristics & 0x20, "fixture is not large-address-aware");
assert(pe.characteristics & 0x1, "fixture relocations are not stripped");
assert(pe.dllCharacteristics & 0x100, "fixture is not NX compatible");
assert.deepEqual(
  Object.fromEntries(
    Object.entries(pe.exports).map(([name, value]) => [name, value.address]),
  ),
  expected.exports,
);
assert.deepEqual(
  pe.sections.map(({ name, virtualAddress, virtualSize, rawSize, characteristics }) => ({
    name,
    virtualAddress,
    virtualSize,
    rawSize,
    characteristics,
  })),
  [
    { name: ".text", virtualAddress: 0x1000, virtualSize: 0x72, rawSize: 0x200, characteristics: 0x60000020 },
    { name: ".rdata", virtualAddress: 0x2000, virtualSize: 0x16d, rawSize: 0x200, characteristics: 0x40000040 },
    { name: ".data", virtualAddress: 0x3000, virtualSize: 0x4, rawSize: 0, characteristics: 0xc0000040 },
    { name: ".nxprobe", virtualAddress: 0x4000, virtualSize: 0x6, rawSize: 0x200, characteristics: 0x40000040 },
    { name: ".rprobe", virtualAddress: 0x5000, virtualSize: 0x4, rawSize: 0x200, characteristics: 0x40000040 },
  ],
);

const wasmBytes = fs.readFileSync(wasmPath);
assert.equal(wasmBytes.byteLength, expected.wasm.bytes);
assert.equal(sha256(wasmBytes), expected.wasm.sha256);
const wasmModule = new WebAssembly.Module(wasmBytes);
const imports = WebAssembly.Module.imports(wasmModule);
const exportsDescription = WebAssembly.Module.exports(wasmModule);
assert.equal(imports.length, expected.wasm.imports);
assert.equal(exportsDescription.length, expected.wasm.exports);
const exportNames = new Set(exportsDescription.map(({ name }) => name));
for (const name of [
  "allocate_memory",
  "full_clear_tlb",
  "memory",
  "reset_cpu",
  "rust_init",
  "update_state_flags",
  "vine_execute_budget",
  "vine_get_stop_error_code",
  "zero_memory",
]) {
  assert(exportNames.has(name), `missing adapter export ${name}`);
}

const hostImportCalls = {};
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
    assert.equal(imported.name, "microtick", `unexpected host import ${imported.name}`);
    assert.equal(args.length, 0);
    return 0;
  };
}

const { instance } = await WebAssembly.instantiate(wasmBytes, { env: environment });
const wasm = instance.exports;
wasm.rust_init();
let state = new DataView(wasm.memory.buffer);
state.setUint32(stateOffset.memorySize, guest.physicalPoolBytes, true);
const guestMemoryBase = wasm.allocate_memory(guest.physicalPoolBytes);
state = new DataView(wasm.memory.buffer);
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
const readPhysicalU32 = (address, label) => {
  checkedEnd(address, 4, guest.physicalPoolBytes, label);
  return physicalView.getUint32(address, true);
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
const mapPage = (virtualBase, permissions, role) => {
  checkedGuestEnd(virtualBase, page.size, role);
  assert.equal(virtualBase & (page.size - 1), 0);
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
      (permissions.write ? page.writable : 0),
    `${role} page table entry`,
  );
  const mapping = {
    virtualBase,
    physicalBase,
    pteAddress,
    permissions: { ...permissions },
    role,
  };
  mappings.set(virtualBase, mapping);
  return mapping;
};
const mappingFor = (address, label) => {
  checkedGuestEnd(address, 1, label);
  const virtualBase = (address & ~(page.size - 1)) >>> 0;
  const mapping = mappings.get(virtualBase);
  assert(mapping, `${label} uses unmapped guest address 0x${address.toString(16)}`);
  return mapping;
};
const writeVirtualBytes = (address, bytes, label) => {
  checkedGuestEnd(address, bytes.byteLength, label);
  for (let index = 0; index < bytes.byteLength; index += 1) {
    const current = (address + index) >>> 0;
    const mapping = mappingFor(current, label);
    physicalMemory[mapping.physicalBase + (current & (page.size - 1))] = bytes[index];
  }
};
const readVirtualBytes = (address, size, label) => {
  checkedGuestEnd(address, size, label);
  const result = new Uint8Array(size);
  for (let index = 0; index < size; index += 1) {
    const current = (address + index) >>> 0;
    const mapping = mappingFor(current, label);
    result[index] = physicalMemory[mapping.physicalBase + (current & (page.size - 1))];
  }
  return result;
};
const writeVirtualU32 = (address, value, label) => {
  const bytes = Uint8Array.from([
    value & 0xff,
    (value >>> 8) & 0xff,
    (value >>> 16) & 0xff,
    value >>> 24,
  ]);
  writeVirtualBytes(address, bytes, label);
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

const headerPermissions = { read: true, write: false, execute: false };
for (let offset = 0; offset < pe.sizeOfHeaders; offset += page.size) {
  mapPage((pe.imageBase + offset) >>> 0, headerPermissions, "PE headers");
}
writeVirtualBytes(pe.imageBase, fixtureBytes.subarray(0, pe.sizeOfHeaders), "PE headers");

for (const section of pe.sections) {
  const permissions = {
    read: Boolean(section.characteristics & permission.read),
    write: Boolean(section.characteristics & permission.write),
    execute: Boolean(section.characteristics & permission.execute),
  };
  const mappedSize = Math.max(section.virtualSize, section.rawSize);
  for (let offset = 0; offset < mappedSize; offset += page.size) {
    mapPage(
      (pe.imageBase + section.virtualAddress + offset) >>> 0,
      permissions,
      `section ${section.name}`,
    );
  }
  if (section.rawSize !== 0) {
    checkedEnd(section.rawOffset, section.rawSize, fixtureBytes.byteLength, `${section.name} raw data`);
    writeVirtualBytes(
      (pe.imageBase + section.virtualAddress) >>> 0,
      fixtureBytes.subarray(section.rawOffset, section.rawOffset + section.rawSize),
      `${section.name} raw data`,
    );
  }
}

for (let address = guest.stackBase; address < guest.stackTop; address += page.size) {
  mapPage(address, { read: true, write: true, execute: false }, "guest stack");
}
mapPage(
  guest.crossPageBase,
  { read: true, write: true, execute: false },
  "cross-page first page",
);
writeVirtualBytes(guest.crossPageAddress, Uint8Array.from([0xa5, 0x5a]), "cross-page sentinel");

const initialAllocationCount = allocations.length;
const initialMappingSnapshot = [...mappings.values()].map((mapping) => ({
  ...mapping,
  permissions: { ...mapping.permissions },
  bytes: physicalMemory.slice(mapping.physicalBase, mapping.physicalBase + page.size),
}));

const registerOffset = (index) => stateOffset.registers + index * 4;
const stackPointer = guest.stackTop - 4;
const configureCpu = (instructionPointer, { reset = true, clearTlb = true } = {}) => {
  if (reset) {
    wasm.reset_cpu();
  }
  const cpl = 3;
  for (let index = 0; index < 8; index += 1) {
    state.setUint8(stateOffset.segmentIsNull + index, 0);
    state.setUint32(stateOffset.segmentOffsets + index * 4, 0, true);
    state.setUint32(stateOffset.segmentLimits + index * 4, 0xffffffff, true);
    state.setUint8(
      stateOffset.segmentAccessBytes + index,
      0x80 | (cpl << 5) | 0x10 | 0x02,
    );
  }
  state.setUint8(
    stateOffset.segmentAccessBytes + 1,
    0x80 | (cpl << 5) | 0x10 | 0x08 | 0x02,
  );
  state.setUint8(stateOffset.protectedMode, 1);
  state.setUint8(stateOffset.operandSize32, 1);
  state.setUint8(stateOffset.stackSize32, 1);
  state.setUint8(stateOffset.cpl, cpl);
  state.setUint8(stateOffset.inHalt, 0);
  state.setUint32(stateOffset.instructionPointer, instructionPointer, true);
  state.setUint32(stateOffset.previousInstructionPointer, instructionPointer, true);
  state.setInt32(stateOffset.lastVirtualInstructionPointer, -1, true);
  state.setUint32(stateOffset.flags, 2, true);
  state.setUint32(registerOffset(0), 0, true);
  state.setUint32(registerOffset(4), stackPointer, true);
  writeVirtualU32(stackPointer, expected.exports.fixture_return_trap, "return trap");

  const cr0 = state.getUint32(stateOffset.controlRegisters, true);
  state.setUint32(
    stateOffset.controlRegisters,
    cr0 | 0x80010001,
    true,
  );
  state.setUint32(stateOffset.controlRegisters + 3 * 4, pageDirectory, true);
  const cr4Offset = stateOffset.controlRegisters + 4 * 4;
  state.setUint32(cr4Offset, state.getUint32(cr4Offset, true) | 0x200, true);
  wasm.update_state_flags();
  if (clearTlb) {
    wasm.full_clear_tlb();
  }
};
const captureState = (reason, initialInstructionCount = 0) => ({
  reason,
  errorCode: wasm.vine_get_stop_error_code(),
  instructionCount:
    state.getUint32(stateOffset.instructionCounter, true) - initialInstructionCount,
  instructionPointer: state.getUint32(stateOffset.instructionPointer, true),
  previousInstructionPointer: state.getUint32(
    stateOffset.previousInstructionPointer,
    true,
  ),
  faultAddress: state.getUint32(stateOffset.controlRegisters + 2 * 4, true),
  eax: state.getUint32(registerOffset(0), true),
  esp: state.getUint32(registerOffset(4), true),
});
const execute = (address, options = {}) => {
  configureCpu(address, options);
  const initialInstructionCount = state.getUint32(stateOffset.instructionCounter, true);
  const reason = wasm.vine_execute_budget(64);
  return captureState(reason, initialInstructionCount);
};

const entry = execute(expected.exports.fixture_entry);
assert.equal(entry.reason, stopReason.invalidOpcode);
assert.equal(entry.errorCode, -1);
assert.equal(entry.instructionPointer, expected.exports.fixture_return_trap);
assert.equal(entry.previousInstructionPointer, expected.exports.fixture_return_trap);
assert.equal(entry.eax, 0);
assert.equal(entry.esp, guest.stackTop);
assert.equal(readVirtualU32(expected.exports.fixture_writable, "entry result"), 0x11223345);

const readonlyBefore = readVirtualU32(expected.exports.fixture_readonly, "readonly before");
const writeReadonly = execute(expected.exports.fixture_write_readonly);
assert.equal(writeReadonly.reason, stopReason.pageFault);
assert.equal(writeReadonly.errorCode, 7);
assert.equal(writeReadonly.faultAddress, expected.exports.fixture_readonly);
assert.equal(
  writeReadonly.previousInstructionPointer,
  expected.exports.fixture_write_readonly,
);
assert.equal(readVirtualU32(expected.exports.fixture_readonly, "readonly after"), readonlyBefore);

const readNonpresent = execute(expected.exports.fixture_read_nonpresent);
assert.equal(readNonpresent.reason, stopReason.pageFault);
assert.equal(readNonpresent.errorCode, 4);
assert.equal(readNonpresent.faultAddress, guest.nonpresentAddress);
assert.equal(
  readNonpresent.previousInstructionPointer,
  expected.exports.fixture_read_nonpresent,
);

const writeNonpresent = execute(expected.exports.fixture_write_nonpresent);
assert.equal(writeNonpresent.reason, stopReason.pageFault);
assert.equal(writeNonpresent.errorCode, 6);
assert.equal(writeNonpresent.faultAddress, guest.nonpresentAddress);
assert.equal(
  writeNonpresent.previousInstructionPointer,
  expected.exports.fixture_write_nonpresent,
);

const crossPageBefore = [...readVirtualBytes(guest.crossPageAddress, 2, "cross-page before")];
const crossPageWrite = execute(expected.exports.fixture_cross_page_write);
assert.equal(crossPageWrite.reason, stopReason.pageFault);
assert.equal(crossPageWrite.errorCode, 6);
assert.equal(crossPageWrite.faultAddress, guest.crossPageBase + page.size);
assert.equal(
  crossPageWrite.previousInstructionPointer,
  expected.exports.fixture_cross_page_write,
);
assert.deepEqual(
  [...readVirtualBytes(guest.crossPageAddress, 2, "cross-page after")],
  crossPageBefore,
  "faulting cross-page write must not partially modify the first page",
);

const executeNonpresent = execute(guest.nonpresentAddress);
assert.equal(executeNonpresent.reason, stopReason.pageFault);
assert.equal(executeNonpresent.errorCode, 4);
assert.equal(executeNonpresent.faultAddress, guest.nonpresentAddress);
assert.equal(executeNonpresent.instructionCount, 0);

const executeNonexec = execute(expected.exports.fixture_nonexec_code);
assert.equal(executeNonexec.reason, stopReason.invalidOpcode);
assert.equal(executeNonexec.instructionPointer, expected.exports.fixture_return_trap);
assert.equal(executeNonexec.eax, 0x12345678);
assert.equal(
  mappingFor(expected.exports.fixture_nonexec_code, "non-executable probe")
    .permissions.execute,
  false,
);

const writableMapping = mappingFor(expected.exports.fixture_writable, "writable probe");
const writablePrime = execute(expected.exports.fixture_write_writable);
assert.equal(writablePrime.reason, stopReason.invalidOpcode);
assert.equal(readVirtualU32(expected.exports.fixture_writable, "writable prime"), 0x55667788);
writeVirtualU32(expected.exports.fixture_writable, 0, "writable stale reset");
writePhysicalU32(
  writableMapping.pteAddress,
  readPhysicalU32(writableMapping.pteAddress, "writable PTE") & ~page.writable,
  "writable PTE read-only transition",
);
const stalePermission = execute(expected.exports.fixture_write_writable, {
  reset: false,
  clearTlb: false,
});
assert.equal(stalePermission.reason, stopReason.invalidOpcode);
assert.equal(
  readVirtualU32(expected.exports.fixture_writable, "stale permission result"),
  0x55667788,
);
writeVirtualU32(expected.exports.fixture_writable, 0, "invalidated permission reset");
const invalidatedPermission = execute(expected.exports.fixture_write_writable, {
  reset: false,
  clearTlb: true,
});
assert.equal(invalidatedPermission.reason, stopReason.pageFault);
assert.equal(invalidatedPermission.errorCode, 7);
assert.equal(invalidatedPermission.faultAddress, expected.exports.fixture_writable);
assert.equal(
  readVirtualU32(expected.exports.fixture_writable, "invalidated permission result"),
  0,
);
writePhysicalU32(
  writableMapping.pteAddress,
  readPhysicalU32(writableMapping.pteAddress, "writable PTE restore") | page.writable,
  "writable PTE restore",
);
wasm.full_clear_tlb();

const nonexecMapping = mappingFor(expected.exports.fixture_nonexec_code, "code remap probe");
const remapPrime = execute(expected.exports.fixture_nonexec_code);
assert.equal(remapPrime.eax, 0x12345678);
const replacementPhysicalPage = allocatePhysicalPage("code remap replacement");
physicalMemory.set(
  Uint8Array.from([0xb8, 0x21, 0x43, 0x65, 0x87, 0xc3]),
  replacementPhysicalPage,
);
const originalNonexecPte = readPhysicalU32(nonexecMapping.pteAddress, "nonexec PTE");
writePhysicalU32(
  nonexecMapping.pteAddress,
  replacementPhysicalPage | (originalNonexecPte & (page.size - 1)),
  "nonexec remap PTE",
);
const staleRemap = execute(expected.exports.fixture_nonexec_code, {
  reset: false,
  clearTlb: false,
});
assert.equal(staleRemap.reason, stopReason.invalidOpcode);
assert.equal(staleRemap.eax, 0x12345678);
const invalidatedRemap = execute(expected.exports.fixture_nonexec_code, {
  reset: false,
  clearTlb: true,
});
assert.equal(invalidatedRemap.reason, stopReason.invalidOpcode);
assert.equal(invalidatedRemap.eax, 0x87654321);
writePhysicalU32(nonexecMapping.pteAddress, originalNonexecPte, "nonexec PTE restore");
wasm.full_clear_tlb();

const entryPageState = {
  code: readPhysicalU32(
    mappingFor(expected.exports.fixture_entry, "entry page state").pteAddress,
    "entry PTE",
  ),
  readonly: readPhysicalU32(
    mappingFor(expected.exports.fixture_readonly, "readonly page state").pteAddress,
    "readonly PTE",
  ),
  writable: readPhysicalU32(writableMapping.pteAddress, "writable PTE state"),
};
assert(entryPageState.code & page.accessed);
assert(entryPageState.readonly & page.accessed);
assert(entryPageState.writable & page.accessed);
assert(entryPageState.writable & page.dirty);

const sparse = new CheckedSparseMemory();
for (const mapping of initialMappingSnapshot) {
  sparse.mapPage(mapping.virtualBase, mapping.permissions, mapping.bytes);
}
assert.equal(sparse.readU32(expected.exports.fixture_readonly), 0x11223344);
sparse.checkExecute(expected.exports.fixture_entry);
sparse.writeU32(expected.exports.fixture_writable, 0x11223345);
assert.equal(sparse.readU32(expected.exports.fixture_writable), 0x11223345);

const sparseWriteReadonly = captureSparseFault(() =>
  sparse.writeU32(expected.exports.fixture_readonly, 0xa1b2c3d4),
);
assert.deepEqual(sparseWriteReadonly, {
  address: expected.exports.fixture_readonly,
  kind: "write",
  present: true,
  errorCode: 7,
});
const sparseReadNonpresent = captureSparseFault(() =>
  sparse.readU32(guest.nonpresentAddress),
);
assert.deepEqual(sparseReadNonpresent, {
  address: guest.nonpresentAddress,
  kind: "read",
  present: false,
  errorCode: 4,
});
const sparseWriteNonpresent = captureSparseFault(() =>
  sparse.writeU32(guest.nonpresentAddress, 0x55667788),
);
assert.deepEqual(sparseWriteNonpresent, {
  address: guest.nonpresentAddress,
  kind: "write",
  present: false,
  errorCode: 6,
});
const sparseExecuteNonexec = captureSparseFault(() =>
  sparse.checkExecute(expected.exports.fixture_nonexec_code),
);
assert.deepEqual(sparseExecuteNonexec, {
  address: expected.exports.fixture_nonexec_code,
  kind: "execute",
  present: true,
  errorCode: 21,
});
const sparseExecuteNonpresent = captureSparseFault(() =>
  sparse.checkExecute(guest.nonpresentAddress),
);
assert.deepEqual(sparseExecuteNonpresent, {
  address: guest.nonpresentAddress,
  kind: "execute",
  present: false,
  errorCode: 20,
});
const sparseCrossBefore = sparse.bytesAt(guest.crossPageAddress, 2);
const sparseCrossPageWrite = captureSparseFault(() =>
  sparse.writeU32(guest.crossPageAddress, 0x89abcdef),
);
assert.deepEqual(sparseCrossPageWrite, {
  address: guest.crossPageBase + page.size,
  kind: "write",
  present: false,
  errorCode: 6,
});
assert.deepEqual(sparse.bytesAt(guest.crossPageAddress, 2), sparseCrossBefore);

const forbiddenCalls = Object.keys(hostImportCalls).filter(
  (name) =>
    name.startsWith("mmap_") ||
    name.startsWith("io_port_") ||
    name === "codegen_finalize",
);
assert.deepEqual(forbiddenCalls, []);

let sourceRevision = null;
const candidateSourceRoot = path.resolve(path.dirname(wasmPath), "..");
try {
  sourceRevision = execFileSync(
    "git",
    ["-C", candidateSourceRoot, "rev-parse", "HEAD"],
    { encoding: "utf8" },
  ).trim();
} catch {
  // An externally built artifact can still be checked without its source tree.
}
if (sourceRevision !== null) {
  assert.equal(sourceRevision, expected.sourceRevision);
}

const mappedVirtualBytes = mappings.size * page.size;
const initialPhysicalBytes = initialAllocationCount * page.size;
const report = {
  schemaVersion: 1,
  sourceRevision,
  artifacts: {
    adapter: {
      path: path.relative(root, wasmPath),
      bytes: wasmBytes.byteLength,
      sha256: sha256(wasmBytes),
    },
    fixture: {
      path: path.relative(root, fixturePath),
      bytes: fixtureBytes.byteLength,
      sha256: sha256(fixtureBytes),
      imageBase: pe.imageBase,
      imageSize: pe.sizeOfImage,
      sections: pe.sections.map(({ name, virtualAddress, characteristics }) => ({
        name,
        virtualAddress,
        read: Boolean(characteristics & permission.read),
        write: Boolean(characteristics & permission.write),
        execute: Boolean(characteristics & permission.execute),
      })),
    },
  },
  mapping: {
    pageSize: page.size,
    guestAddressBytes: 0x1_0000_0000,
    physicalPoolBytes: guest.physicalPoolBytes,
    pageDirectoryPhysical: pageDirectory,
    pageTablePages: pageTables.size,
    mappedVirtualPages: mappings.size,
    mappedVirtualBytes,
    initiallyAllocatedPhysicalPages: initialAllocationCount,
    initiallyAllocatedPhysicalBytes: initialPhysicalBytes,
    highestMappedVirtualPage: Math.max(...mappings.keys()),
    identityMapped: false,
  },
  v86Paging: {
    entry,
    writeReadonly,
    readNonpresent,
    writeNonpresent,
    crossPageWrite,
    crossPageFirstPageUnchanged: true,
    executeNonpresent,
    executeNonexec,
    nonExecutableSectionWasExecuted: true,
    instructionFetchFaultBit: false,
    protectionTransition: {
      staleWithoutInvalidation: stalePermission,
      blockedAfterFullClearTlb: invalidatedPermission,
    },
    pageRemap: {
      staleValueWithoutInvalidation: staleRemap.eax,
      valueAfterFullClearTlb: invalidatedRemap.eax,
    },
    pteState: {
      codeAccessed: Boolean(entryPageState.code & page.accessed),
      readonlyAccessed: Boolean(entryPageState.readonly & page.accessed),
      writableAccessed: Boolean(entryPageState.writable & page.accessed),
      writableDirty: Boolean(entryPageState.writable & page.dirty),
    },
  },
  checkedSparseReference: {
    cpuIntegrated: false,
    mappedPages: sparse.pages.size,
    backingBytes: sparse.pages.size * page.size,
    writeReadonly: sparseWriteReadonly,
    readNonpresent: sparseReadNonpresent,
    writeNonpresent: sparseWriteNonpresent,
    executeNonexec: sparseExecuteNonexec,
    executeNonpresent: sparseExecuteNonpresent,
    crossPageWrite: sparseCrossPageWrite,
    crossPageFirstPageUnchanged: true,
  },
  comparison: {
    provisionalCandidate: "v86-paging-with-vine-region-table-and-bounded-physical-pool",
    v86PagingCpuIntegrated: true,
    v86PagingHighAddressExecution: true,
    v86PagingReadWriteProtection: true,
    v86PagingExecuteProtection: false,
    v86PagingRequiresExplicitTlbInvalidation: true,
    checkedSparseHasFullReferenceProtection: true,
    checkedSparseCpuIntegration: false,
    remainingGates: [
      "adapter-owned execute permission and instruction-fetch fault classification",
      "bounded physical page allocator and page-table ownership API",
      "generated-code invalidation after protect or remap",
      "snapshot and restore of region, page-table, and physical-page state",
      "Chromium Worker memory ceiling and performance",
    ],
  },
  hostImportCalls,
};

assert.equal(report.mapping.identityMapped, false);
assert(expected.fixture.imageBase > guest.physicalPoolBytes);
assert.equal(report.v86Paging.nonExecutableSectionWasExecuted, true);
assert.equal(report.comparison.v86PagingExecuteProtection, false);
assert.equal(report.comparison.checkedSparseCpuIntegration, false);

fs.rmSync(outputDirectory, { recursive: true, force: true });
fs.mkdirSync(outputDirectory, { recursive: true });
fs.writeFileSync(
  path.join(outputDirectory, "report.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);

console.log(
  `runtime memory slice passed: high EIP=0x${entry.instructionPointer.toString(16)}, ` +
    `R/W #PF=0x${writeReadonly.errorCode.toString(16)}, ` +
    `cross-page atomic fault, NX gap observed, mapped=${mappedVirtualBytes / 1024} KiB`,
);
