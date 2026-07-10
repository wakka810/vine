import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const defaultWasmPath = path.join(
  root,
  ".work",
  "engine-candidates",
  "v86",
  "build",
  "v86-fallback.wasm",
);
const wasmPath = path.resolve(process.argv[2] ?? defaultWasmPath);
const outputDirectory = path.join(root, ".work", "v86-adapter-spike");
const expectedSourceRevision =
  "2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f";

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

const guest = {
  memorySize: 2 * 1024 * 1024,
  codeAddress: 0x1000,
};

const stopReason = {
  budget: 1,
  halt: 2,
  divideError: 0x100,
  invalidOpcode: 0x106,
  generalProtection: 0x10d,
  pageFault: 0x10e,
};

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

if (!fs.existsSync(wasmPath)) {
  console.error(`adapter Wasm not found: ${path.relative(root, wasmPath)}`);
  console.error("Build the patched v86 fallback artifact before running this check.");
  process.exit(1);
}

const bytes = fs.readFileSync(wasmPath);
const module = new WebAssembly.Module(bytes);
const imports = WebAssembly.Module.imports(module);
const exportsDescription = WebAssembly.Module.exports(module);
const exportNames = new Set(exportsDescription.map(({ name }) => name));

for (const name of requiredExports) {
  assert(exportNames.has(name), `missing adapter export ${name}`);
}
assert(
  !imports.some(({ name }) => name === "cpu_exception_hook"),
  "release adapter must not depend on the debug exception hook",
);

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
  environment[imported.name] = () => {
    hostImportCalls[imported.name] = (hostImportCalls[imported.name] ?? 0) + 1;
    if (imported.name === "run_hardware_timers") {
      return 100;
    }
    return 0;
  };
}

const { instance } = await WebAssembly.instantiate(bytes, { env: environment });
const wasm = instance.exports;
wasm.rust_init();

let state = new DataView(wasm.memory.buffer);
state.setUint32(stateOffset.memorySize, guest.memorySize, true);
const guestMemoryBase = wasm.allocate_memory(guest.memorySize);
state = new DataView(wasm.memory.buffer);
const memory = new Uint8Array(
  wasm.memory.buffer,
  guestMemoryBase,
  guest.memorySize,
);

function configure(program, cpl) {
  wasm.zero_memory(0, guest.memorySize);
  wasm.reset_cpu();
  memory.set(program, guest.codeAddress);

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
  state.setUint32(stateOffset.instructionPointer, guest.codeAddress, true);
  state.setUint32(
    stateOffset.previousInstructionPointer,
    guest.codeAddress,
    true,
  );
  state.setInt32(stateOffset.lastVirtualInstructionPointer, -1, true);
  state.setUint32(stateOffset.flags, 2, true);

  const cr0 = state.getUint32(stateOffset.controlRegisters, true);
  state.setUint32(stateOffset.controlRegisters, cr0 | 1, true);
  const cr4Offset = stateOffset.controlRegisters + 4 * 4;
  const cr4 = state.getUint32(cr4Offset, true);
  state.setUint32(cr4Offset, cr4 | 0x200, true);
  wasm.update_state_flags();
  wasm.full_clear_tlb();
}

function currentState(reason) {
  return {
    reason,
    errorCode: wasm.vine_get_stop_error_code(),
    instructionCount: state.getUint32(stateOffset.instructionCounter, true),
    instructionPointer: state.getUint32(
      stateOffset.instructionPointer,
      true,
    ),
    eax: state.getUint32(stateOffset.registers, true),
  };
}

configure(Uint8Array.from([0x40, 0xeb, 0xfd]), 3);
const zeroBudget = currentState(wasm.vine_execute_budget(0));
const firstBudget = currentState(wasm.vine_execute_budget(7));
const secondBudget = currentState(wasm.vine_execute_budget(5));

assert.deepEqual(zeroBudget, {
  reason: stopReason.budget,
  errorCode: -1,
  instructionCount: 0,
  instructionPointer: 0x1000,
  eax: 0,
});
assert.deepEqual(firstBudget, {
  reason: stopReason.budget,
  errorCode: -1,
  instructionCount: 7,
  instructionPointer: 0x1001,
  eax: 4,
});
assert.deepEqual(secondBudget, {
  reason: stopReason.budget,
  errorCode: -1,
  instructionCount: 12,
  instructionPointer: 0x1000,
  eax: 6,
});

configure(Uint8Array.from([0xb8, 0x2a, 0, 0, 0, 0x0f, 0x0b]), 3);
const invalidOpcode = currentState(wasm.vine_execute_budget(10));
assert.deepEqual(invalidOpcode, {
  reason: stopReason.invalidOpcode,
  errorCode: -1,
  instructionCount: 2,
  instructionPointer: 0x1005,
  eax: 42,
});

configure(Uint8Array.from([0x31, 0xc0, 0x31, 0xd2, 0xf7, 0xf0]), 3);
const divideError = currentState(wasm.vine_execute_budget(10));
assert.deepEqual(divideError, {
  reason: stopReason.divideError,
  errorCode: -1,
  instructionCount: 3,
  instructionPointer: 0x1004,
  eax: 0,
});

configure(Uint8Array.from([0xf4]), 3);
const generalProtection = currentState(wasm.vine_execute_budget(10));
assert.deepEqual(generalProtection, {
  reason: stopReason.generalProtection,
  errorCode: 0,
  instructionCount: 1,
  instructionPointer: 0x1000,
  eax: 0,
});

configure(Uint8Array.from([0xf4]), 3);
const cr0 = state.getUint32(stateOffset.controlRegisters, true);
state.setUint32(stateOffset.controlRegisters, cr0 | 0x80000000, true);
wasm.full_clear_tlb();
const pageFault = currentState(wasm.vine_execute_budget(10));
assert.deepEqual(pageFault, {
  reason: stopReason.pageFault,
  errorCode: 4,
  instructionCount: 0,
  instructionPointer: 0x1000,
  eax: 0,
});

configure(Uint8Array.from([0xf4]), 0);
const halt = currentState(wasm.vine_execute_budget(10));
assert.deepEqual(halt, {
  reason: stopReason.halt,
  errorCode: -1,
  instructionCount: 1,
  instructionPointer: 0x1001,
  eax: 0,
});

const stateProgram = Uint8Array.from([
  0xb8, 0x0a, 0x00, 0x00, 0x00,
  0xd9, 0xe8,
  0xc7, 0x05, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3f,
  0xf3, 0x0f, 0x10, 0x05, 0x00, 0x20, 0x00, 0x00,
  0xf3, 0x0f, 0x58, 0xc0,
  0x05, 0x05, 0x00, 0x00, 0x00,
  0xd9, 0xe8,
  0xde, 0xc1,
  0xdb, 0x1d, 0x04, 0x20, 0x00, 0x00,
  0xf3, 0x0f, 0x58, 0xc0,
  0xf3, 0x0f, 0x11, 0x05, 0x08, 0x20, 0x00, 0x00,
  0xf4,
]);
configure(stateProgram, 0);
const statePrefix = currentState(wasm.vine_execute_budget(5));
assert.equal(statePrefix.reason, stopReason.budget);
assert.equal(statePrefix.instructionPointer, 0x101d);
assert.equal(statePrefix.eax, 10);

assert.equal(wasm.vine_cpu_state_version(), 1);
assert.equal(wasm.vine_cpu_state_size(), 640);
wasm.vine_cpu_state_save();
const stateBuffer = new Uint32Array(
  wasm.memory.buffer,
  wasm.vine_cpu_state_ptr(),
  wasm.vine_cpu_state_size() / 4,
);
const savedState = stateBuffer.slice();
assert.equal(savedState[0], 0x56494e45);
assert.equal(savedState[1], 1);
assert.equal(savedState[2], 160);

wasm.reset_cpu();
assert.equal(state.getUint32(stateOffset.registers, true), 0);
stateBuffer.set(savedState);
assert.equal(wasm.vine_cpu_state_restore(), 0);
const restoredState = currentState(0);
assert.equal(restoredState.instructionPointer, 0x101d);
assert.equal(restoredState.eax, 10);

const stateResults = new DataView(memory.buffer, memory.byteOffset);
const stateContinuation = currentState(wasm.vine_execute_budget(20));
assert.deepEqual(stateContinuation, {
  reason: stopReason.halt,
  errorCode: -1,
  instructionCount: 7,
  instructionPointer: guest.codeAddress + stateProgram.byteLength,
  eax: 15,
});
assert.equal(stateResults.getInt32(0x2004, true), 2);
assert.equal(stateResults.getUint32(0x2008, true), 0x40800000);

stateBuffer[0] = 0;
assert.equal(wasm.vine_cpu_state_restore(), 1);
stateBuffer.set(savedState);
assert.equal(wasm.vine_cpu_state_restore(), 0);

const cpuState = {
  version: wasm.vine_cpu_state_version(),
  size: wasm.vine_cpu_state_size(),
  prefix: statePrefix,
  restored: restoredState,
  continuation: stateContinuation,
  x87Result: stateResults.getInt32(0x2004, true),
  sseResultBits: stateResults.getUint32(0x2008, true),
};

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
  assert.equal(sourceRevision, expectedSourceRevision);
}

const report = {
  schemaVersion: 1,
  sourceRevision,
  artifact: {
    path: path.relative(root, wasmPath),
    size: bytes.byteLength,
    sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    importCount: imports.length,
    exportCount: exportsDescription.length,
  },
  stopReason,
  results: {
    zeroBudget,
    firstBudget,
    secondBudget,
    invalidOpcode,
    divideError,
    generalProtection,
    pageFault,
    halt,
    cpuState,
  },
  hostImportCalls,
};

fs.rmSync(outputDirectory, { recursive: true, force: true });
fs.mkdirSync(outputDirectory, { recursive: true });
fs.writeFileSync(
  path.join(outputDirectory, "report.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);

console.log(
  `v86 adapter spike passed: exact budgets 0/7/5, ` +
    `CPL3 #DE/#UD/#GP/#PF, state v${cpuState.version}, ` +
    `x87=${cpuState.x87Result}, SSE=0x${cpuState.sseResultBits.toString(16)}`,
);
