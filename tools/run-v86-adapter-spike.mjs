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
  "v86-adapter-source",
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
  segmentIsNull: 724,
  segmentOffsets: 736,
  segmentLimits: 768,
  protectedMode: 800,
  operandSize32: 804,
  stackSize32: 808,
  fpuStackEmpty: 816,
  xmm: 832,
  fpuStackPtr: 1032,
  fpuSt: 1152,
};

const guest = {
  memorySize: 2 * 1024 * 1024,
  codeAddress: 0x1000,
};

const stopReason = {
  budget: 1,
  halt: 2,
  executeProtection: 3,
  divideError: 0x100,
  invalidOpcode: 0x106,
  generalProtection: 0x10d,
  pageFault: 0x10e,
};

const requiredExports = [
  "allocate_memory",
  "full_clear_tlb",
  "get_jit_config",
  "jit_clear_cache_js",
  "memory",
  "reset_cpu",
  "rust_init",
  "update_state_flags",
  "vine_adapter_abi_version",
  "vine_cpu_state_ptr",
  "vine_cpu_state_restore",
  "vine_cpu_state_save",
  "vine_cpu_state_size",
  "vine_cpu_state_version",
  "vine_dirty_page_count",
  "vine_dirty_page_tracking_version",
  "vine_dirty_pages_overflowed",
  "vine_dirty_pages_ptr",
  "vine_execute_budget",
  "vine_get_stop_error_code",
  "vine_instruction_counter",
  "vine_reset_execute_permissions",
  "vine_reset_dirty_pages",
  "vine_set_memory_size",
  "vine_set_page_executable",
  "set_jit_config",
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
const wasmTableOffset = 1_024;
const wasmTableSize = 900;
const wasmTable = new WebAssembly.Table({
  element: "anyfunc",
  initial: wasmTableOffset + wasmTableSize,
});
let wasm;
let jitImports;
const environment = {
  __indirect_function_table: wasmTable,
};

for (const imported of imports) {
  if (imported.kind !== "function") {
    continue;
  }
  environment[imported.name] = (...arguments_) => {
    hostImportCalls[imported.name] = (hostImportCalls[imported.name] ?? 0) + 1;
    if (imported.name === "codegen_finalize") {
      const [index, start, stateFlags, pointer, byteLength] = arguments_;
      assert(wasm !== undefined && jitImports !== undefined);
      assert(Number.isInteger(index) && index >= 0 && index < wasmTableSize);
      assert(
        Number.isInteger(pointer) &&
          Number.isInteger(byteLength) &&
          pointer >= 0 &&
          byteLength > 0 &&
          pointer + byteLength <= wasm.memory.buffer.byteLength,
      );
      const code = Uint8Array.from(
        new Uint8Array(wasm.memory.buffer, pointer, byteLength),
      );
      const generated = new WebAssembly.Module(code);
      const generatedExports = new WebAssembly.Instance(generated, {
        e: jitImports,
      }).exports;
      assert.equal(typeof generatedExports.f, "function");
      queueMicrotask(() => {
        wasmTable.set(index + wasmTableOffset, generatedExports.f);
        wasm.codegen_finalize_finished(index, start, stateFlags);
      });
      return 0;
    }
    if (imported.name === "jit_clear_func") {
      const [index] = arguments_;
      assert(Number.isInteger(index) && index >= 0 && index < wasmTableSize);
      wasmTable.set(index + wasmTableOffset, null);
      return 0;
    }
    if (imported.name === "run_hardware_timers") {
      return 100;
    }
    return 0;
  };
}

const { instance } = await WebAssembly.instantiate(bytes, { env: environment });
wasm = instance.exports;
jitImports = { m: wasm.memory };
for (const [name, value] of Object.entries(wasm)) {
  if (!name.startsWith("_") && !name.startsWith("zstd") && !name.endsWith("_js")) {
    jitImports[name] = value;
  }
}
const jitInstructionLimit = wasm.vine_jit_instruction_limit;
let jitInstructionLimitCalls = 0;
jitImports.vine_jit_instruction_limit = () => {
  const limit = jitInstructionLimit();
  jitInstructionLimitCalls += 1;
  return limit;
};
const jitFindCacheEntryInPage = wasm.jit_find_cache_entry_in_page;
let jitFindCacheEntryInPageCalls = 0;
jitImports.jit_find_cache_entry_in_page = (...arguments_) => {
  jitFindCacheEntryInPageCalls += 1;
  return jitFindCacheEntryInPage(...arguments_);
};
wasm.rust_init();
assert.equal(wasm.vine_adapter_abi_version(), 1);
assert.equal(wasm.vine_dirty_page_tracking_version(), 1);
assert.equal(wasm.vine_dirty_page_count(), 0);
assert.equal(wasm.vine_dirty_pages_overflowed(), 0);

let state = new DataView(wasm.memory.buffer);
assert.equal(wasm.vine_set_memory_size(guest.memorySize), 0);
const guestMemoryBase = wasm.allocate_memory(guest.memorySize);
let memory;

function refreshViews() {
  if (state.buffer === wasm.memory.buffer) {
    return;
  }
  state = new DataView(wasm.memory.buffer);
  memory = new Uint8Array(
    wasm.memory.buffer,
    guestMemoryBase,
    guest.memorySize,
  );
}

refreshViews();

function configure(program, cpl) {
  wasm.zero_memory(0, guest.memorySize);
  wasm.reset_cpu();
  wasm.vine_reset_execute_permissions();
  assert.equal(wasm.vine_set_page_executable(guest.codeAddress, 1), 0);
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
  refreshViews();
  return {
    reason,
    errorCode: wasm.vine_get_stop_error_code(),
    instructionCount: wasm.vine_instruction_counter(),
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

const dirtyProgram = Uint8Array.from([
  0xc7, 0x05, 0x00, 0x20, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12,
  0xf4,
]);
configure(dirtyProgram, 0);
const guestView = new DataView(memory.buffer, memory.byteOffset, memory.byteLength);
guestView.setUint32(0x3000, 0x4000 | 0x07, true);
guestView.setUint32(0x4000 + 1 * 4, 0x1000 | 0x07, true);
guestView.setUint32(0x4000 + 2 * 4, 0x2000 | 0x07, true);
state.setUint32(stateOffset.controlRegisters + 3 * 4, 0x3000, true);
state.setUint32(
  stateOffset.controlRegisters,
  state.getUint32(stateOffset.controlRegisters, true) | 0x8000_0000,
  true,
);
wasm.vine_reset_dirty_pages();
wasm.full_clear_tlb();
const dirtyWrite = currentState(wasm.vine_execute_budget(10));
assert.equal(dirtyWrite.reason, stopReason.halt);
assert.equal(guestView.getUint32(0x2000, true), 0x1234_5678);
assert.equal(wasm.vine_dirty_page_count(), 1);
assert.equal(wasm.vine_dirty_pages_overflowed(), 0);
const dirtyPages = new Uint32Array(
  wasm.memory.buffer,
  wasm.vine_dirty_pages_ptr(),
  wasm.vine_dirty_page_count(),
);
assert.deepEqual([...dirtyPages], [0x2000]);
wasm.vine_reset_dirty_pages();
assert.equal(wasm.vine_dirty_page_count(), 0);

const pageFaultRestartProgram = Uint8Array.from([
  // mov dword ptr [0x2000], 0x12345678
  0xc7, 0x05, 0x00, 0x20, 0x00, 0x00, 0x78, 0x56, 0x34, 0x12,
  0xf4,
]);
configure(pageFaultRestartProgram, 0);
const pageFaultRestartView = new DataView(
  memory.buffer,
  memory.byteOffset,
  memory.byteLength,
);
pageFaultRestartView.setUint32(0x3000, 0x4000 | 0x07, true);
pageFaultRestartView.setUint32(0x4000 + 1 * 4, 0x1000 | 0x07, true);
pageFaultRestartView.setUint32(0x4000 + 2 * 4, 0, true);
state.setUint32(stateOffset.controlRegisters + 3 * 4, 0x3000, true);
state.setUint32(
  stateOffset.controlRegisters,
  state.getUint32(stateOffset.controlRegisters, true) | 0x8000_0000,
  true,
);
wasm.vine_reset_dirty_pages();
wasm.full_clear_tlb();
const pageFaultBeforeCommit = currentState(wasm.vine_execute_budget(10));
assert.equal(pageFaultBeforeCommit.reason, stopReason.pageFault);
assert.equal(pageFaultBeforeCommit.errorCode, 2);
assert.equal(
  pageFaultBeforeCommit.instructionPointer,
  guest.codeAddress,
  "write page fault did not retain the faulting instruction",
);
memory.fill(0xa5, 0x2000, 0x3000);
pageFaultRestartView.setUint32(0x4000 + 2 * 4, 0x2000 | 0x07, true);
wasm.full_clear_tlb();
const pageFaultAfterCommit = currentState(wasm.vine_execute_budget(10));
assert.equal(pageFaultAfterCommit.reason, stopReason.halt);
assert.equal(
  pageFaultRestartView.getUint32(0x2000, true),
  0x1234_5678,
  "faulting write was not replayed after the page became present",
);
wasm.vine_reset_dirty_pages();

const jitPageFaultRestartProgram = Uint8Array.from([
  // mov ecx, 0x7fffffff; movups xmm0, [0x2100]
  0xb9, 0xff, 0xff, 0xff, 0x7f,
  0x0f, 0x10, 0x05, 0x00, 0x21, 0x00, 0x00,
  // loop: movups [0x2ff8], xmm0; dec ecx; jnz loop; hlt
  0x0f, 0x11, 0x05, 0xf8, 0x2f, 0x00, 0x00,
  0x49,
  0x75, 0xf6,
  0xf4,
]);
wasm.jit_clear_cache_js();
wasm.set_jit_config(0, 0);
configure(jitPageFaultRestartProgram, 0);
let jitPageFaultRestartView = new DataView(
  memory.buffer,
  memory.byteOffset,
  memory.byteLength,
);
jitPageFaultRestartView.setUint32(0x3000, 0x4000 | 0x07, true);
jitPageFaultRestartView.setUint32(0x4000 + 1 * 4, 0x1000 | 0x07, true);
jitPageFaultRestartView.setUint32(0x4000 + 2 * 4, 0x2000 | 0x07, true);
jitPageFaultRestartView.setUint32(0x4000 + 3 * 4, 0x5000 | 0x07, true);
const jitPageFaultPattern = Uint8Array.from(
  { length: 16 },
  (_, index) => 0x60 + index,
);
memory.set(jitPageFaultPattern, 0x2100);
state.setUint32(stateOffset.controlRegisters + 3 * 4, 0x3000, true);
state.setUint32(
  stateOffset.controlRegisters,
  state.getUint32(stateOffset.controlRegisters, true) | 0x8000_0000,
  true,
);
const jitPageFaultCompileCallsBefore = hostImportCalls.codegen_finalize ?? 0;
assert.equal(
  currentState(wasm.vine_execute_budget(524_289)).reason,
  stopReason.budget,
);
await Promise.resolve();
refreshViews();
jitPageFaultRestartView = new DataView(
  memory.buffer,
  memory.byteOffset,
  memory.byteLength,
);
assert(
  (hostImportCalls.codegen_finalize ?? 0) > jitPageFaultCompileCallsBefore,
  "page-fault restart loop did not compile",
);
wasm.vine_cpu_state_save();
const jitPageFaultState = new Uint32Array(
  wasm.memory.buffer,
  wasm.vine_cpu_state_ptr(),
  wasm.vine_cpu_state_size() / Uint32Array.BYTES_PER_ELEMENT,
);
jitPageFaultState[5] = 1;
jitPageFaultState[12] = guest.codeAddress + 12;
jitPageFaultState[13] = guest.codeAddress + 12;
jitPageFaultState[24] = 0;
assert.equal(wasm.vine_cpu_state_restore(), 0);
memory.fill(0xa5, 0x2ff8, 0x3000);
memory.fill(0xa5, 0x5000, 0x5008);
jitPageFaultRestartView.setUint32(0x4000 + 3 * 4, 0, true);
wasm.vine_reset_dirty_pages();
wasm.full_clear_tlb();
const jitPageFaultBeforeCommit = currentState(
  wasm.vine_execute_budget(4096),
);
assert.equal(jitPageFaultBeforeCommit.reason, stopReason.pageFault);
assert.equal(jitPageFaultBeforeCommit.errorCode, 2);
assert.equal(
  jitPageFaultBeforeCommit.instructionPointer,
  guest.codeAddress + 12,
  "compiled write page fault did not retain the faulting instruction",
);
jitPageFaultRestartView.setUint32(0x4000 + 3 * 4, 0x5000 | 0x07, true);
wasm.full_clear_tlb();
const jitPageFaultAfterCommit = currentState(wasm.vine_execute_budget(4096));
assert.equal(jitPageFaultAfterCommit.reason, stopReason.halt);
assert.deepEqual(
  [...memory.subarray(0x2ff8, 0x3000), ...memory.subarray(0x5000, 0x5008)],
  [...jitPageFaultPattern],
  "compiled faulting write was not replayed after the page became present",
);
wasm.vine_reset_dirty_pages();


const jitProgram = Uint8Array.from([
  0xb9, 0xff, 0xff, 0xff, 0x7f,
  0xb8, 0xdf, 0x9b, 0x57, 0x13,
  0xbb, 0xe0, 0xac, 0x68, 0x24,
  0x01, 0xd8,
  0x31, 0xc3,
  0xc1, 0xc0, 0x05,
  0x6b, 0xd8, 0x03,
  0x49,
  0x75, 0xf3,
]);
configure(jitProgram, 3);
const jitWarmupBudget = 524_289;
const jitWarmup = currentState(wasm.vine_execute_budget(jitWarmupBudget));
assert.equal(jitWarmup.reason, stopReason.budget);
assert.equal(jitWarmup.instructionCount, jitWarmupBudget);
assert((hostImportCalls.codegen_finalize ?? 0) > 0, "hot loop did not compile");
await Promise.resolve();
const jitContinuationBefore = wasm.vine_instruction_counter();
const jitContinuationBudget = 12_345;
const jitContinuation = currentState(wasm.vine_execute_budget(jitContinuationBudget));
assert.equal(jitContinuation.reason, stopReason.budget);
assert.equal(
  jitContinuation.instructionCount - jitContinuationBefore,
  jitContinuationBudget,
);
assert(jitInstructionLimitCalls > 0, "compiled JIT block did not enforce its slice limit");

wasm.jit_clear_cache_js();
wasm.set_jit_config(4, 0);
assert.equal(wasm.get_jit_config(4), 0);
configure(jitProgram, 3);
const relaxedJitWarmup = currentState(wasm.vine_execute_budget(jitWarmupBudget));
assert.equal(relaxedJitWarmup.reason, stopReason.budget);
await Promise.resolve();
const relaxedJitBefore = wasm.vine_instruction_counter();
const relaxedJitLimitCallsBefore = jitInstructionLimitCalls;
const relaxedJitContinuation = currentState(
  wasm.vine_execute_budget(jitContinuationBudget),
);
assert.equal(relaxedJitContinuation.reason, stopReason.budget);
const relaxedJitDelta =
  relaxedJitContinuation.instructionCount - relaxedJitBefore;
assert(
  relaxedJitDelta >= jitContinuationBudget,
  "relaxed JIT execution stopped before its requested budget",
);
assert(
  relaxedJitDelta <= jitContinuationBudget + 100_003,
  "relaxed JIT execution exceeded one compiled block of budget overshoot",
);
assert.equal(
  jitInstructionLimitCalls,
  relaxedJitLimitCallsBefore,
  "relaxed JIT block retained the exact-budget host call",
);
wasm.set_jit_config(4, 1);
assert.equal(wasm.get_jit_config(4), 1);
wasm.set_jit_config(5, 100_000);
assert.equal(wasm.get_jit_config(5), 100_000);
wasm.set_jit_config(5, 200_000);
assert.equal(wasm.get_jit_config(5), 200_000);

const crossPageCallProgram = new Uint8Array(0x1018);
crossPageCallProgram.set([
  0xb9, 0x40, 0x0d, 0x03, 0x00,
  0x49,
  0x75, 0xfd,
  0xbf, 0xa0, 0x86, 0x01, 0x00,
  0xb8, 0x00, 0x30, 0x00, 0x00,
  0xbe, 0x78, 0x56, 0x34, 0x12,
  0xe8, 0xe4, 0x0f, 0x00, 0x00,
  0x4f,
  0x75, 0xf8,
  0xf4,
]);
crossPageCallProgram.set([
  0x8b, 0x48, 0x74,
  0x8b, 0x50, 0x18,
  0x89, 0x34, 0x8a,
  0x8b, 0x50, 0x74,
  0x42,
  0x3b, 0x50, 0x10,
  0x1b, 0xc9,
  0x23, 0xca,
  0x89, 0x48, 0x74,
  0xc3,
], 0x1000);

async function runCrossPageCall(jitEnabled) {
  wasm.jit_clear_cache_js();
  wasm.set_jit_config(0, jitEnabled ? 0 : 1);
  configure(crossPageCallProgram, 0);
  assert.equal(wasm.vine_set_page_executable(guest.codeAddress + 0x1000, 1), 0);
  const view = new DataView(memory.buffer, memory.byteOffset, memory.byteLength);
  state.setUint32(stateOffset.registers + 4 * 4, 0x1f000, true);
  view.setUint32(0x3000 + 0x10, 16, true);
  view.setUint32(0x3000 + 0x18, 0x3200, true);
  view.setUint32(0x3000 + 0x74, 0, true);
  const compileCallsBefore = hostImportCalls.codegen_finalize ?? 0;
  const cacheLookupCallsBefore = jitFindCacheEntryInPageCalls;
  let reason = stopReason.budget;
  let slices = 0;
  while (reason === stopReason.budget) {
    reason = wasm.vine_execute_budget(250_000);
    await Promise.resolve();
    refreshViews();
    slices += 1;
    assert(slices < 16, "cross-page call loop did not halt within its bounded slices");
  }
  assert.equal(reason, stopReason.halt);
  const resultView = new DataView(memory.buffer, memory.byteOffset, memory.byteLength);
  return {
    registers: Array.from({ length: 8 }, (_, index) =>
      state.getUint32(stateOffset.registers + index * 4, true)),
    flags: state.getUint32(stateOffset.flags, true),
    instructionCount: wasm.vine_instruction_counter(),
    index: resultView.getUint32(0x3000 + 0x74, true),
    values: Array.from({ length: 16 }, (_, index) =>
      resultView.getUint32(0x3200 + index * 4, true)),
    compileCalls: (hostImportCalls.codegen_finalize ?? 0) - compileCallsBefore,
    cacheLookupCalls: jitFindCacheEntryInPageCalls - cacheLookupCallsBefore,
    compiled: (hostImportCalls.codegen_finalize ?? 0) > compileCallsBefore,
  };
}

const crossPageReference = await runCrossPageCall(false);
const crossPageJit = await runCrossPageCall(true);
assert.equal(crossPageJit.compileCalls, 1, "direct callee escaped its caller JIT module");
assert.equal(crossPageJit.cacheLookupCalls, 0, "JIT return crossed the cache-lookup boundary");
assert.deepEqual(
  { ...crossPageJit, compileCalls: 0, compiled: false },
  crossPageReference,
  "cross-page direct-call JIT diverged from the interpreter",
);
assert.equal(crossPageJit.index, 0);
assert.deepEqual(crossPageJit.values, Array(16).fill(0x1234_5678));

wasm.jit_clear_cache_js();
wasm.set_jit_config(0, 0);

const branchingLoopProgram = Uint8Array.from([
  0xb9, 0xa0, 0x86, 0x01, 0x00,
  0x31, 0xc0,
  0xf7, 0xc1, 0x01, 0x00, 0x00, 0x00,
  0x74, 0x05,
  0x83, 0xc0, 0x03,
  0xeb, 0x05,
  0x35, 0x55, 0x00, 0x00, 0x00,
  0xd1, 0xc0,
  0x49,
  0x75, 0xe9,
  0xf4,
]);

async function runBranchingLoop(disableJit, budget) {
  wasm.jit_clear_cache_js();
  wasm.set_jit_config(0, disableJit ? 1 : 0);
  configure(branchingLoopProgram, 0);
  const compileCallsBefore = hostImportCalls.codegen_finalize ?? 0;
  let reason = stopReason.budget;
  let slices = 0;
  while (reason === stopReason.budget) {
    reason = wasm.vine_execute_budget(budget);
    await Promise.resolve();
    refreshViews();
    slices += 1;
    assert(slices < 32, "branching loop did not halt within its bounded slices");
  }
  assert.equal(reason, stopReason.halt);
  return {
    instructionCount: wasm.vine_instruction_counter(),
    instructionPointer: state.getUint32(stateOffset.instructionPointer, true),
    eax: state.getUint32(stateOffset.registers, true),
    ecx: state.getUint32(stateOffset.registers + 4, true),
    edx: state.getUint32(stateOffset.registers + 8, true),
    ebx: state.getUint32(stateOffset.registers + 12, true),
    slices,
    compiled: (hostImportCalls.codegen_finalize ?? 0) > compileCallsBefore,
  };
}

const branchingInterpreter = await runBranchingLoop(true, 1_000_000);
const branchingJit = await runBranchingLoop(false, 65_537);
assert.equal(branchingJit.compiled, true, "branching loop did not reach compiled execution");
assert.deepEqual(
  { ...branchingJit, slices: 0, compiled: false },
  { ...branchingInterpreter, slices: 0, compiled: false },
  "bounded JIT continuation diverged from the interpreter",
);

const packedSimdProgram = Uint8Array.from([
  0xb9, 0xa0, 0x86, 0x01, 0x00,
  0xf3, 0x0f, 0x6f, 0x05, 0x00, 0x20, 0x00, 0x00,
  0xf3, 0x0f, 0x6f, 0x0d, 0x10, 0x20, 0x00, 0x00,
  0x66, 0x0f, 0x60, 0x05, 0x10, 0x20, 0x00, 0x00,
  0x66, 0x0f, 0x60, 0xc8,
  0x66, 0x0f, 0x67, 0x05, 0x20, 0x20, 0x00, 0x00,
  0x66, 0x0f, 0x67, 0xc8,
  0x66, 0x0f, 0xf9, 0x05, 0x20, 0x20, 0x00, 0x00,
  0x66, 0x0f, 0xf9, 0xc8,
  0x66, 0x0f, 0xd5, 0x05, 0x10, 0x20, 0x00, 0x00,
  0x66, 0x0f, 0xd5, 0xc8,
  0x66, 0x0f, 0x71, 0xe0, 0x04,
  0x66, 0x0f, 0x71, 0xd1, 0x03,
  0x66, 0x0f, 0xfd, 0xc1,
  0x0f, 0x58, 0x05, 0x00, 0x20, 0x00, 0x00,
  0x0f, 0x58, 0xc1,
  0x0f, 0x5c, 0x05, 0x10, 0x20, 0x00, 0x00,
  0x0f, 0x5c, 0xc1,
  0x0f, 0x59, 0x05, 0x20, 0x20, 0x00, 0x00,
  0x0f, 0x59, 0xc1,
  0x0f, 0xc6, 0x05, 0x10, 0x20, 0x00, 0x00, 0x1b,
  0x0f, 0xc6, 0xc1, 0x4e,
  0x49,
  0x75, 0x85,
  0x66, 0x0f, 0x61, 0x05, 0x10, 0x20, 0x00, 0x00,
  0x66, 0x0f, 0x61, 0xc1,
  0x66, 0x0f, 0x69, 0x05, 0x00, 0x20, 0x00, 0x00,
  0x66, 0x0f, 0x69, 0xc1,
  0x66, 0x0f, 0x6b, 0x05, 0x20, 0x20, 0x00, 0x00,
  0x66, 0x0f, 0x6b, 0xc1,
  0xf3, 0x0f, 0x5b, 0x05, 0x20, 0x20, 0x00, 0x00,
  0xf3, 0x0f, 0x5b, 0xc9,
  0xf3, 0x0f, 0x7f, 0x05, 0x30, 0x20, 0x00, 0x00,
  0xf3, 0x0f, 0x7f, 0x0d, 0x40, 0x20, 0x00, 0x00,
  0xf4,
]);

async function runPackedSimd(disableJit) {
  wasm.jit_clear_cache_js();
  wasm.set_jit_config(0, disableJit ? 1 : 0);
  configure(packedSimdProgram, 0);
  memory.set([
    0x00, 0x01, 0x7f, 0x80, 0xc8, 0xff, 0x10, 0x20,
    0x34, 0x12, 0xcc, 0xed, 0x55, 0xaa, 0xfe, 0x7f,
  ], 0x2000);
  memory.set([
    0xff, 0xc8, 0x80, 0x7f, 0x01, 0x00, 0x40, 0x10,
    0x78, 0x56, 0x21, 0x43, 0x00, 0x80, 0x02, 0x01,
  ], 0x2010);
  memory.set([
    0x00, 0x00, 0x00, 0x4f, 0x00, 0x00, 0xc0, 0x7f,
    0x01, 0x00, 0x00, 0xcf, 0xff, 0xff, 0xff, 0x4e,
  ], 0x2020);
  const compileCallsBefore = hostImportCalls.codegen_finalize ?? 0;
  let reason = stopReason.budget;
  let slices = 0;
  while (reason === stopReason.budget) {
    reason = wasm.vine_execute_budget(250_000);
    await Promise.resolve();
    refreshViews();
    slices += 1;
    assert(slices < 16, "packed SIMD loop did not halt within its bounded slices");
  }
  assert.equal(reason, stopReason.halt);
  return {
    output: [...memory.subarray(0x2030, 0x2050)],
    instructionCount: wasm.vine_instruction_counter(),
    compiled: (hostImportCalls.codegen_finalize ?? 0) > compileCallsBefore,
  };
}

const packedSimdInterpreter = await runPackedSimd(true);
assert.deepEqual(packedSimdInterpreter.output.slice(0, 16), [
  0x00, 0x00, 0x00, 0x80,
  0x00, 0x00, 0x00, 0x80,
  0x00, 0x00, 0x00, 0x80,
  0x80, 0xff, 0xff, 0x7f,
]);
const packedSimdJit = await runPackedSimd(false);
assert.equal(packedSimdJit.compiled, true, "packed SIMD loop did not compile");
assert.deepEqual(
  { ...packedSimdJit, compiled: false },
  packedSimdInterpreter,
  "packed SIMD JIT diverged from the interpreter",
);

const mmxMoveProgram = Uint8Array.from([
  0xb9, 0xa0, 0x86, 0x01, 0x00,
  0x0f, 0x6f, 0x05, 0x00, 0x21, 0x00, 0x00,
  0x0f, 0x6f, 0xc8,
  0x0f, 0x7f, 0xca,
  0xf3, 0x0f, 0xd6, 0xda,
  0xf2, 0x0f, 0xd6, 0xe3,
  0x0f, 0x7f, 0x25, 0x08, 0x21, 0x00, 0x00,
  0x49,
  0x75, 0xe1,
  0xf4,
]);

async function runMmxMoves(disableJit) {
  wasm.jit_clear_cache_js();
  wasm.set_jit_config(0, disableJit ? 1 : 0);
  configure(mmxMoveProgram, 0);
  memory.set([0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef], 0x2100);
  const compileCallsBefore = hostImportCalls.codegen_finalize ?? 0;
  let reason = stopReason.budget;
  let slices = 0;
  while (reason === stopReason.budget) {
    reason = wasm.vine_execute_budget(250_000);
    await Promise.resolve();
    refreshViews();
    slices += 1;
    assert(slices < 16, "MMX move loop did not halt within its bounded slices");
  }
  assert.equal(reason, stopReason.halt);
  return {
    output: [...memory.subarray(0x2108, 0x2110)],
    xmm3: [
      ...new Uint8Array(wasm.memory.buffer, stateOffset.xmm + 3 * 16, 16),
    ],
    mmxMetadata: [0, 1, 2, 4].map((register) =>
      state.getUint16(stateOffset.fpuSt + register * 16 + 8, true)),
    fpuStackEmpty: state.getUint8(stateOffset.fpuStackEmpty),
    fpuStackPtr: state.getUint8(stateOffset.fpuStackPtr),
    instructionCount: wasm.vine_instruction_counter(),
    compiled: (hostImportCalls.codegen_finalize ?? 0) > compileCallsBefore,
  };
}

const mmxMoveInterpreter = await runMmxMoves(true);
const mmxMoveJit = await runMmxMoves(false);
assert.equal(mmxMoveJit.compiled, true, "MMX move loop did not compile");
assert.deepEqual(
  { ...mmxMoveJit, compiled: false },
  mmxMoveInterpreter,
  "MMX move JIT diverged from the interpreter",
);
assert.deepEqual(mmxMoveJit.output, [
  0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
]);
assert.deepEqual(mmxMoveJit.xmm3, [
  0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
  0, 0, 0, 0, 0, 0, 0, 0,
]);
assert.deepEqual(mmxMoveJit.mmxMetadata, [0xffff, 0xffff, 0xffff, 0xffff]);
assert.equal(mmxMoveJit.fpuStackEmpty, 0);
assert.equal(mmxMoveJit.fpuStackPtr, 0);

const forbiddenCalls = Object.keys(hostImportCalls).filter(
  (name) =>
    name.startsWith("mmap_") ||
    name.startsWith("io_port_"),
);
assert.deepEqual(forbiddenCalls, []);

let executionIsland = null;
if (exportNames.has("vine_execution_island_abi_version")) {
  assert.equal(wasm.vine_execution_island_abi_version(), 1);
  assert.equal(wasm.vine_bridge_fast_path_abi_version(), 3);
  const fixture = fs.readFileSync(
    path.join(root, ".work", "toolchain-spike", "browser-vertical.exe"),
  );
  assert.equal(wasm.vine_bridge_input_resize(fixture.byteLength), 0);
  new Uint8Array(
    wasm.memory.buffer,
    wasm.vine_bridge_input_ptr(),
    fixture.byteLength,
  ).set(fixture);
  assert.equal(wasm.vine_bridge_prepare(fixture.byteLength, 0x1357_9bdf), 0);
  const prepared = JSON.parse(
    new TextDecoder().decode(
      new Uint8Array(
        wasm.memory.buffer,
        wasm.vine_bridge_metadata_ptr(),
        wasm.vine_bridge_metadata_len(),
      ),
    ),
  );
  const generation = BigInt(prepared.generation);
  assert.equal(
    wasm.vine_bridge_acknowledge(
      Number(generation & 0xffff_ffffn),
      Number(generation >> 32n),
    ),
    0,
  );
  const blendProgram = [
    0x0f, 0x6f, 0x04, 0x01, 0xf3, 0x0f, 0xd6, 0xc0, 0x0f, 0x6f, 0x00, 0xf3, 0x0f,
    0xd6, 0xe0, 0x66, 0x0f, 0x60, 0xe1, 0x66, 0x0f, 0x60, 0xc1, 0x66, 0x0f, 0xf9,
    0xe0, 0x66, 0x0f, 0xd5, 0xe2, 0x66, 0x0f, 0x6f, 0xd8, 0x66, 0x0f, 0x71, 0xe4,
    0x07, 0x66, 0x0f, 0xfd, 0xdc, 0x66, 0x0f, 0x67, 0xd9, 0xf2, 0x0f, 0xd6, 0xc3,
    0x0f, 0x7f, 0x00, 0x83, 0xc0, 0x08, 0x4e, 0x75, 0xc3, 0xf4,
  ];
  const hotLoopCases = [
    ...[
      { name: "page-crossing blend", source: 0x2f00, destination: 0x4000 },
      { name: "forward aliased blend", source: 0x3000, destination: 0x3008 },
      { name: "backward aliased blend", source: 0x3008, destination: 0x3000 },
      { name: "in-place blend", source: 0x3000, destination: 0x3000 },
      { name: "unaligned blend fallback", source: 0x2fff, destination: 0x4001 },
      { name: "nonzero unpack blend fallback", source: 0x2f00, destination: 0x4000, unpack: 0x17 },
      { name: "disabled SIMD blend", source: 0x2000, destination: 0x4000, cr0: 4 },
      { name: "pending SIMD context switch", source: 0x2000, destination: 0x4000, cr0: 8 },
      { name: "disabled OS SIMD blend", source: 0x2000, destination: 0x4000, cr4: 0 },
      { name: "code-aliased blend fallback", source: 0x2000, destination: 0x4000,
        pageOverrides: [[0x4000, 0x1007]], extraOutput: [0x1000, 0x1040] },
    ].map(({ name, source, destination, unpack = 0, cr0 = 0, cr4 = 0x200,
      pageOverrides, extraOutput }) => ({
      name,
      program: blendProgram,
      pageOverrides,
      extraOutput,
      setup() {
        for (let index = 0x2000; index < 0x5000; index += 1) {
          memory[index] = (index * 17 + (index >>> 4) * 29) & 0xff;
        }
        state.setUint32(stateOffset.registers, destination, true);
        state.setUint32(stateOffset.registers + 4, (source - destination) >>> 0, true);
        state.setUint32(stateOffset.registers + 6 * 4, 128, true);
        state.setUint32(stateOffset.flags, 3, true);
        state.setUint32(stateOffset.controlRegisters, 1 | cr0, true);
        state.setUint32(stateOffset.controlRegisters + 4 * 4, cr4, true);
        for (let lane = 0; lane < 8; lane += 1) {
          state.setUint16(stateOffset.xmm + 16 + lane * 2, unpack, true);
          state.setUint16(stateOffset.xmm + 32 + lane * 2, [0, 1, 63, 64, 127, 128, 0x7fff, 0xffff][lane], true);
        }
      },
      output: [destination, destination + 1024],
    })),
    {
      name: "aligned SIMD copy",
      program: [
        0x0f, 0x18, 0x80, 0x00, 0x02, 0x00, 0x00, 0x0f, 0x28, 0x00, 0x0f, 0x2b, 0x04,
        0x02, 0x83, 0xc0, 0x10, 0x49, 0x75, 0xec, 0xf4,
      ],
      setup() {
        for (let index = 0; index < 1024; index += 1) {
          memory[0x2000 + index] = (index * 29 + 7) & 0xff;
        }
        state.setUint32(stateOffset.registers, 0x2000, true);
        state.setUint32(stateOffset.registers + 4, 64, true);
        state.setUint32(stateOffset.registers + 8, 0x1000, true);
        state.setUint32(stateOffset.flags, 3, true);
      },
      output: [0x3000, 0x3400],
    },
    {
      name: "bounded byte copy",
      program: [
        0x89, 0x85, 0xf8, 0xfc, 0xff, 0xff, 0x3b, 0xc1, 0x73, 0x09, 0x8a, 0x14, 0x30,
        0x88, 0x14, 0x38, 0x40, 0xeb, 0xed, 0xf4,
      ],
      setup() {
        for (let index = 0; index < 80; index += 1) {
          memory[0x2000 + index] = (index * 17 + 3) & 0xff;
        }
        memory.fill(0xee, 0x3000, 0x3050);
        state.setUint32(stateOffset.registers, 3, true);
        state.setUint32(stateOffset.registers + 4, 67, true);
        state.setUint32(stateOffset.registers + 8, 0x1234_5600, true);
        state.setUint32(stateOffset.registers + 5 * 4, 0x5000, true);
        state.setUint32(stateOffset.registers + 6 * 4, 0x2000, true);
        state.setUint32(stateOffset.registers + 7 * 4, 0x3000, true);
      },
      output: [0x3000, 0x3050],
    },
    {
      name: "overlapping forward copy",
      program: [
        0x8b, 0x7d, 0x08, 0x8a, 0x06, 0x88, 0x07, 0x47, 0x46, 0x49, 0x89, 0x7d, 0x08,
        0x75, 0xf1, 0xf4,
      ],
      setup() {
        memory.set([0x12, 0x34, 0x56], 0x2000);
        state.setUint32(stateOffset.registers, 0xabcd_e000, true);
        state.setUint32(stateOffset.registers + 4, 48, true);
        state.setUint32(stateOffset.registers + 5 * 4, 0x4000, true);
        state.setUint32(stateOffset.registers + 6 * 4, 0x2000, true);
        new DataView(memory.buffer, memory.byteOffset).setUint32(0x4008, 0x2003, true);
      },
      output: [0x2000, 0x2033],
    },
    {
      name: "byte copy with aliased loop local",
      program: [
        0x89, 0x85, 0xf8, 0xfc, 0xff, 0xff, 0x3b, 0xc1, 0x73, 0x09, 0x8a, 0x14, 0x30,
        0x88, 0x14, 0x38, 0x40, 0xeb, 0xed, 0xf4,
      ],
      setup() {
        for (let index = 0; index < 8; index += 1) {
          memory[0x2000 + index] = index * 17 + 3;
        }
        state.setUint32(stateOffset.registers + 4, 8, true);
        state.setUint32(stateOffset.registers + 5 * 4, 0x2308, true);
        state.setUint32(stateOffset.registers + 6 * 4, 0x2000, true);
        state.setUint32(stateOffset.registers + 7 * 4, 0x3000, true);
      },
      output: [0x3000, 0x3008],
      extraOutput: [0x2000, 0x2008],
    },
    {
      name: "bounded unsigned sort",
      program: [
        0x8b, 0xf3, 0x3b, 0xdf, 0x7d, 0x21, 0x8d, 0x9b, 0x00, 0x00, 0x00, 0x00,
        0x8b, 0x84, 0xb5, 0xf8, 0xf7, 0xff, 0xff, 0x8b, 0x0a, 0x3b, 0xc8, 0x76,
        0x09, 0x89, 0x02, 0x89, 0x8c, 0xb5, 0xf8, 0xf7, 0xff, 0xff, 0x46, 0x3b,
        0xf7, 0x7c, 0xe5, 0x43, 0x83, 0xc2, 0x04, 0xff, 0x8d, 0xf0, 0xe7, 0xff,
        0xff, 0x75, 0xcd, 0x33, 0xc9, 0xf4,
      ],
      setup() {
        const values = [0xffff_ffff, 3, 0x8000_0000, 3, 0, 9, 1, 7];
        const view = new DataView(memory.buffer, memory.byteOffset);
        values.forEach((value, index) => view.setUint32(0x67f8 + index * 4, value, true));
        view.setUint32(0x57f0, values.length - 1, true);
        state.setUint32(stateOffset.registers + 3 * 4, 1, true);
        state.setUint32(stateOffset.registers + 5 * 4, 0x7000, true);
        state.setUint32(stateOffset.registers + 6 * 4, 0x7654_3210, true);
        state.setUint32(stateOffset.registers + 7 * 4, values.length, true);
        state.setUint32(stateOffset.registers + 2 * 4, 0x67f8, true);
      },
      output: [0x67f8, 0x6818],
      extraOutput: [0x57f0, 0x57f4],
    },
    {
      name: "bounded Huffman walk",
      program: [
        0x33, 0xc9, 0x85, 0xd2, 0x75, 0x08, 0x8a, 0x1f, 0x47, 0xba, 0x08, 0x00,
        0x00, 0x00, 0x0f, 0xb6, 0xf3, 0xc1, 0xee, 0x07, 0x8d, 0x0c, 0x8e, 0x8b,
        0x4c, 0x88, 0x08, 0x8b, 0xf1, 0x03, 0xf6, 0x02, 0xdb, 0x4a, 0x83, 0x3c,
        0xf0, 0x00, 0x75, 0xda, 0xf4,
      ],
      setup() {
        const view = new DataView(memory.buffer, memory.byteOffset);
        view.setUint32(0x4000, 1, true);
        view.setUint32(0x4008, 3, true);
        view.setUint32(0x400c, 1, true);
        view.setUint32(0x4010, 1, true);
        view.setUint32(0x4018, 2, true);
        view.setUint32(0x401c, 3, true);
        view.setUint32(0x4020, 0, true);
        memory[0x5000] = 0xa0;
        state.setUint32(stateOffset.registers, 0x4000, true);
        state.setUint32(stateOffset.registers + 2 * 4, 0x1234_5600, true);
        state.setUint32(stateOffset.registers + 6 * 4, 0x7654_3210, true);
        state.setUint32(stateOffset.registers + 7 * 4, 0x5000, true);
      },
      output: [0x5000, 0x5001],
    },
  ];
  async function runHotLoop(test, useIsland, budget, warm = false) {
    const program = [...test.program];
    if (warm) {
      // Enter the repeated blend by fallthrough and a warm JIT backedge.
      program.unshift(0xbb, 0x00, 0x02, 0x00, 0x00,
        0xb8, ...[0, 8, 16, 24].map((shift) => test.output[0] >>> shift & 0xff),
        0xbe, 0x80, 0x00, 0x00, 0x00);
      program.splice(-1, 1, 0x4b, 0x75, (5 - (program.length + 2)) & 0xff, 0xf4);
    }
    configure(Uint8Array.from(program), 0);
    test.setup();
    const pageTable = new DataView(memory.buffer, memory.byteOffset);
    pageTable.setUint32(0x1f0000, 0x1f1007, true);
    for (let page = 0; page < guest.memorySize / 4096; page += 1) {
      pageTable.setUint32(0x1f1000 + page * 4, page * 4096 | 7, true);
    }
    for (const [address, entry] of test.pageOverrides ?? []) {
      pageTable.setUint32(0x1f1000 + (address >>> 12) * 4, entry, true);
    }
    state.setUint32(stateOffset.controlRegisters + 3 * 4, 0x1f0000, true);
    state.setUint32(stateOffset.controlRegisters,
      state.getUint32(stateOffset.controlRegisters, true) | 0x80000000, true);
    wasm.full_clear_tlb();
    wasm.vine_reset_dirty_pages();
    let reason;
    let slices = 0;
    do {
      reason = useIsland
        ? wasm.vine_execution_island_execute_budget(budget, 0, 0, 0, 0)
        : wasm.vine_execute_budget(budget);
      await Promise.resolve();
      refreshViews();
      assert(++slices < 64, "warm native loop exceeded its bounded slices");
    } while (warm && reason === stopReason.budget);
    wasm.vine_cpu_state_save();
    const cpuState = Array.from(
      new Uint32Array(
        wasm.memory.buffer,
        wasm.vine_cpu_state_ptr(),
        wasm.vine_cpu_state_size() / Uint32Array.BYTES_PER_ELEMENT,
      ),
    );
    const dirtyPages = Array.from(
      new Uint32Array(
        wasm.memory.buffer,
        wasm.vine_dirty_pages_ptr(),
        wasm.vine_dirty_page_count(),
      ),
    ).sort((left, right) => left - right);
    return {
      reason,
      instructionCount: wasm.vine_instruction_counter(),
      cpuState,
      dirtyPages,
      output: [...memory.subarray(...test.output)],
      extraOutput: test.extraOutput === undefined
        ? null
        : [...memory.subarray(...test.extraOutput)],
    };
  }
  wasm.set_jit_config(0, 1);
  for (const test of hotLoopCases) {
    for (const budget of [1, 3, 6, 8, 16, 17, 18, 33, 34, 35, 97, 100_000]) {
      const reference = await runHotLoop(test, false, budget);
      const accelerated = await runHotLoop(test, true, budget);
      assert.deepEqual(accelerated, reference,
        `${test.name} budget ${budget} hot loop diverged from the interpreter`);
    }
  }
  for (const test of hotLoopCases.slice(0, 3)) {
    wasm.jit_clear_cache_js();
    wasm.set_jit_config(0, 1);
    const reference = await runHotLoop(test, false, 100_000, true);
    wasm.jit_clear_cache_js();
    wasm.set_jit_config(0, 0);
    const compiledBefore = hostImportCalls.codegen_finalize ?? 0;
    const jitBefore = wasm.vine_jit_retired_instructions_low();
    const accelerated = await runHotLoop(test, true, 100_000, true);
    assert.deepEqual(accelerated, reference, `${test.name} warm JIT handoff diverged`);
    const jitInstructions = (wasm.vine_jit_retired_instructions_low() - jitBefore) >>> 0;
    assert((hostImportCalls.codegen_finalize ?? 0) > compiledBefore && jitInstructions > 0,
      `${test.name} did not warm the surrounding JIT code`);
    assert(jitInstructions < accelerated.instructionCount / 2,
      `${test.name} was compiled over the native island loop`);
  }
  wasm.set_jit_config(0, 0);
  assert.equal(wasm.vine_execution_island_execute_budget(0, 0, 0, 0, 0), stopReason.budget);
  const islandGeneration =
    BigInt(wasm.vine_execution_island_generation_low()) |
    (BigInt(wasm.vine_execution_island_generation_high()) << 32n);
  assert.equal(islandGeneration, generation);
  executionIsland = {
    abiVersion: wasm.vine_execution_island_abi_version(),
    runtimeFastPathVersion: wasm.vine_bridge_fast_path_abi_version(),
    preparedGeneration: generation.toString(),
  };

  const runtimeFirst = new WebAssembly.Instance(module, { env: environment }).exports;
  wasm = runtimeFirst;
  jitImports = { m: wasm.memory };
  for (const [name, value] of Object.entries(wasm)) {
    if (!name.startsWith("_") && !name.startsWith("zstd") && !name.endsWith("_js")) {
      jitImports[name] = value;
    }
  }
  assert.equal(wasm.vine_bridge_input_resize(fixture.byteLength), 0);
  new Uint8Array(
    wasm.memory.buffer,
    wasm.vine_bridge_input_ptr(),
    fixture.byteLength,
  ).set(fixture);
  assert.equal(wasm.vine_bridge_prepare(fixture.byteLength, 0x2468_ace1), 0);
  wasm.rust_init();
  assert.equal(wasm.vine_set_memory_size(64 * 1024), 0);
  assert(wasm.allocate_memory(64 * 1024) > 0);
  executionIsland.runtimeFirstInitialization = true;
}

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
    dirtyWrite,
    pageFaultRestart: {
      beforeCommit: pageFaultBeforeCommit,
      afterCommit: pageFaultAfterCommit,
    },
    jitPageFaultRestart: {
      beforeCommit: jitPageFaultBeforeCommit,
      afterCommit: jitPageFaultAfterCommit,
    },
    jitWarmup,
    jitContinuation,
    relaxedJitWarmup,
    relaxedJitContinuation,
    branchingInterpreter,
    branchingJit,
    executionIsland,
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
  `v86 adapter spike passed: exact interpreter budgets 0/7/5 and JIT budgets ` +
    `${jitWarmupBudget}/${jitContinuationBudget}, ` +
    `CPL3 #DE/#UD/#GP/#PF, state v${cpuState.version}, ` +
    `x87=${cpuState.x87Result}, SSE=0x${cpuState.sseResultBits.toString(16)}`,
);
