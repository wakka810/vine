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

async function checkCountedLoops() {
  // Synthetic x86 image arithmetic: parameterized registers, addresses and data.
  function countedMap({a=0, alpha=3, base=5, source=6, dest=7, count=2, mm=0, other=1, zero=7} = {}) {
    const p = [], labels = {}, fixups = [];
    const u32 = n => [n & 255, n >>> 8 & 255, n >>> 16 & 255, n >>> 24 & 255];
    const mem = (r, b, offset=0, index=null) => index === null
      ? [0x80 | r << 3 | b, ...u32(offset)]
      : [0x84 | r << 3, 0xc0 | index << 3 | b, ...u32(offset)];
    const branch = (op, target) => { p.push(op, 0); fixups.push([p.length - 1, target]); };
    labels.start = p.length;
    p.push(0x0f, 0xb6, ...mem(a, alpha), 0x40 | alpha, 0xd3, 0xe0 | a);
    p.push(0x03, ...mem(a, base, 0)); branch(0x7e, 'tail');
    p.push(0x0f, 0x6e, ...mem(mm, source), 0x81, 0xf8 | a, ...u32(256)); branch(0x73, 'store');
    p.push(0xc1, 0xe8 | a, 1, 0x0f, 0x6e, ...mem(other, dest));
    for (const r of [mm, other]) p.push(0x0f, 0x60, 0xc0 | r << 3 | zero);
    p.push(0x0f, 0xf9, 0xc0 | mm << 3 | other, 0x0f, 0xd5, ...mem(mm, base, 128, a));
    p.push(0x0f, 0x71, 0xe0 | mm, 7, 0x0f, 0xfd, 0xc0 | mm << 3 | other, 0x0f, 0x67, 0xc0 | mm << 3 | zero);
    labels.store = p.length; p.push(0x0f, 0x7e, ...mem(mm, dest));
    labels.tail = p.length; p.push(0x83, 0xc0 | dest, 4, 0x83, 0xc0 | source, 4, 0x48 | count);
    branch(0x75, 'start'); p.push(0xf4);
    for (const [at, target] of fixups) {
      const displacement = labels[target] - at - 1;
      assert(displacement >= -128 && displacement <= 127); p[at] = displacement & 255;
    }
    return {program: Uint8Array.from(p), registers: {a, alpha, base, source, dest, count, mm, other, zero}};
  }

  async function run(test, disabled) {
    wasm.jit_clear_cache_js(); wasm.set_jit_config(0, Number(disabled));
    wasm.set_jit_config(4, 1); wasm.set_jit_config(5, 32);
    const {program: map, registers: r} = countedMap(test.roles);
    const rgb = Uint8Array.from([
      0x0f, 0xb6, 0x46, 2, 0x0f, 0xb6, 0x5e, 1, 0xc1, 0xe0, 8, 0x0b, 0xc3,
      0x0f, 0xb6, 0x1e, 0xc1, 0xe0, 8, 0x09, 0xd8, 0x89, 0x07,
      0x83, 0xc7, 4, 0x83, 0xc6, 3, 0x4a, 0x75, 0xe0, 0xf4,
    ]);
    const program = test.rgb ? rgb : map;
    configure(program, 0);
    const view = new DataView(memory.buffer, memory.byteOffset, memory.byteLength);
    const pixels = test.pixels ?? 4096, src = 0x40000, dst = test.alias ? src + 4 : test.codeWrite ? 0x1800 : 0x140000;
    const coeff = test.cross ? 0x120f00 : 0x120000, alpha = 0xc0000;
    for (let i = 0; i < pixels * 4 + 4; i++) memory[src + i] = (i * 37 + (i >> 8) * 13) & 255;
    if (!test.alias) memory.fill(0x55, dst, dst + pixels * 4);
    for (let i = 0; i < pixels; i++) memory[alpha + i] = i * 23 & 255;
    view.setInt32(coeff, 0, true);
    for (let i = 0; i < 128; i++) for (let lane = 0; lane < 4; lane++) view.setUint16(coeff + 128 + i * 8 + lane * 2, (i * 113 + lane * 17011) & 65535, true);
    for (let i = 0; i < 8; i++) {
      state.setUint32(stateOffset.registers + i * 4, 0, true);
      state.setBigUint64(stateOffset.fpuSt + i * 16, 0x8765432112345678n + BigInt(i), true);
      state.setUint16(stateOffset.fpuSt + i * 16 + 8, 0x1234 + i, true);
    }
    const reg = (index, v) => state.setUint32(stateOffset.registers + index * 4, v, true);
    reg(r.source, src); reg(r.dest, dst); reg(r.alpha, alpha); reg(r.base, coeff); reg(r.count, pixels);
    reg(1, 0); state.setBigUint64(stateOffset.fpuSt + r.zero * 16, 0n, true);
    state.setUint8(stateOffset.fpuStackPtr, 3); state.setUint8(stateOffset.fpuStackEmpty, 0x55);
    view.setUint32(0x1f0000, 0x1f1007, true);
    for (let page = 0; page < guest.memorySize / 4096; page++) view.setUint32(0x1f1000 + page * 4, page * 4096 | 7, true);
    if (test.noncontiguous) {
      memory.copyWithin(0x160000, 0x121000, 0x122000);
      view.setUint32(0x1f1000 + 0x121 * 4, 0x160007, true);
    }
    if (test.fault) {
      const page = (test.fault === 'read' ? src : dst) + 4096;
      view.setUint32(0x1f1000 + (page >>> 12) * 4, test.fault === 'readonly' ? page | 5 : 0, true);
    }
    state.setUint32(stateOffset.controlRegisters + 3 * 4, 0x1f0000, true);
    state.setUint32(stateOffset.controlRegisters, state.getUint32(stateOffset.controlRegisters, true) | 0x80010000 | (test.ts ? 8 : 0), true);
    wasm.full_clear_tlb(); wasm.vine_reset_dirty_pages();
    let before = wasm.vine_affine_iterations?.() ?? 0;
    const stops = [];
    for (let slice = 0; ; slice++) {
      assert(slice < 2000);
      if (slice === 2 && test.cold) {
        // Warmed code, cold conditional streams: no access may fault until the
        // branch actually uses it. Restart to exercise a completed skip prefix.
        before = wasm.vine_affine_iterations?.() ?? 0;
        memory.fill(0, alpha, alpha + pixels);
        if (test.firstRead !== undefined) memory[alpha + test.firstRead] = 128;
        reg(r.source, src); reg(r.dest, dst); reg(r.alpha, alpha); reg(r.count, pixels);
        state.setUint32(stateOffset.instructionPointer, guest.codeAddress, true);
        state.setUint32(stateOffset.previousInstructionPointer, guest.codeAddress, true);
        if (test.removeStream) {
          const address = test.removeStream === 'source' ? src : dst;
          for (let page = address >>> 12; page < (address + pixels * 4) >>> 12; page++)
            view.setUint32(0x1f1000 + page * 4, test.streamReadonly ? page * 4096 | 5 : 0, true);
        }
        wasm.full_clear_tlb();
      }
      if (slice === 2 && test.rearm) wasm.vine_reset_dirty_pages();
      if (slice === 2 && (test.allZero || test.bias !== undefined || test.shift !== undefined)) {
        if (test.allZero) memory.fill(0, alpha, alpha + pixels);
        view.setInt32(coeff, test.bias ?? 0, true); reg(1, test.shift ?? 0);
        state.setUint8(stateOffset.fpuStackEmpty, 0x55); state.setUint8(stateOffset.fpuStackPtr, 3);
      }
      const reason = wasm.vine_execute_budget(slice < 2 ? 8193 : test.budget ?? 8193);
      await Promise.resolve(); refreshViews();
      stops.push({reason, ip: state.getUint32(stateOffset.instructionPointer, true), count: wasm.vine_instruction_counter(),
        registers: Array.from({length: 8}, (_,i) => state.getUint32(stateOffset.registers + i * 4, true)),
        flags: wasm.get_eflags(), mmx: Array.from(new Uint8Array(wasm.memory.buffer, stateOffset.fpuSt, 128)),
        tags: state.getUint8(stateOffset.fpuStackEmpty), top: state.getUint8(stateOffset.fpuStackPtr)});
      if (reason !== stopReason.budget) break;
    }
    const result = {stops, hash: crypto.createHash('sha256').update(memory.subarray(dst, dst + pixels * 4)).digest('hex'),
      dirty: Array.from(new Uint32Array(wasm.memory.buffer, wasm.vine_dirty_pages_ptr(), wasm.vine_dirty_page_count())).sort((a,b)=>a-b)};
    return {result, iterations: (wasm.vine_affine_iterations?.() ?? 0) - before};
  }
  const cases = [
    {name: 'unused unmapped source', cold: true, removeStream: 'source'},
    {name: 'unused unmapped destination', cold: true, removeStream: 'destination'},
    {name: 'unused readonly destination', cold: true, removeStream: 'destination', streamReadonly: true},
    {name: 'cold first source read', cold: true, firstRead: 0},
    {name: 'cold late source read', cold: true, firstRead: 700},
    {name: 'cold boundary source read', cold: true, firstRead: 1024},
    {name: 'cold final source read', cold: true, firstRead: 4095},
    {name: 'cold missing late source read', cold: true, removeStream: 'source', firstRead: 700},
    {name: 'cold missing first source read', cold: true, removeStream: 'source', firstRead: 0},
    {name: 'mixed map'}, {name: 'small exact budget', budget: 4097}, {name: 'interpreter small budget', budget: 301},
    {name: 'renamed registers', roles: {a:2, alpha:6, base:7, source:3, dest:5, count:0, mm:4, other:2, zero:6}},
    {name: 'overlapping streams', alias: true}, {name: 'cross-page invariant', cross: true},
    {name: 'no MMX branch', allZero: true},
    {name: 'signed minimum', bias: -2147483648}, {name: 'signed overflow', bias: 2147483647},
    {name: 'variable shift', shift: 3, bias: -101},
    {name: 'late read fault', fault: 'read'}, {name: 'late write fault', fault: 'write'},
    {name: 'late readonly fault', fault: 'readonly'}, {name: 'MMX task fault', ts: true},
    {name: 'integer pixel conversion', rgb: true},
    {name: 'noncontiguous invariant', cross: true, noncontiguous: true},
    {name: 'dirty-page rearm', rearm: true},
    {name: 'code-page destination', codeWrite: true, pixels: 128},
  ];
  for (const test of cases) {
    const reference = await run(test, true), candidate = await run(test, false);
    const coldReadFault = test.cold && test.removeStream === 'source' && test.firstRead !== undefined;
    if (test.fault || test.ts || coldReadFault) {
      // Existing v86 counts the whole faulting basic block. Compare the architectural
      // state exactly, and retain the measured accounting delta instead of hiding it.
      const delta = candidate.result.stops.at(-1).count - reference.result.stops.at(-1).count;
      assert.equal(delta, test.fault === 'read' || coldReadFault ? 2 : test.fault === 'write' ? 3 : 0);

      candidate.result.stops.at(-1).count -= delta;
    }
    assert.deepEqual(candidate.result, reference.result, test.name);
    if (wasm.vine_affine_iterations && test.cold && test.firstRead === undefined)
      assert(candidate.iterations > 1000, test.name + ' did not accelerate unused mappings');
    if (wasm.vine_affine_iterations && ['mixed map', 'renamed registers', 'overlapping streams', 'cross-page invariant', 'integer pixel conversion'].includes(test.name))
      assert(candidate.iterations > 0, test.name + ' did not accelerate');

  }
}
await checkCountedLoops();
async function checkVectorWindows() {
  const batchCount = () => wasm.vine_vector_batches?.() ?? 0;
  const imm = n => [n & 255, n >>> 8 & 255, n >>> 16 & 255, n >>> 24 & 255];
  const gather = [0xf3,0x0f,0x7e,0x1e,0x66,0x0f,0x61,0xdf,0x0f,0x5b,0xdb,
    0x0f,0x6f,0x0c,0x1e,0x0f,0x67,0xc9,0x0f,0x7e,0xc8];
  for (let lane=0; lane<4; lane++) {
    gather.push(0x0f,0xb6,0xc8);
    for (const [r,offset] of [[4,0],[5,2048]]) for (let half=0; half<2; half++)
      gather.push(0x66,0x0f,0xc4,0x84|(r<<3),0x8a,...imm(offset+half*2),lane*2+half);
    if (lane<3) gather.push(0xc1,0xe8,8);
  }
  gather.push(0x0f,0x58,0xe3,0xf3,0x0f,0x5b,0xe4,0x66,0x0f,0x6b,0xe4,
    0x66,0x0f,0x67,0xe4,0x66,0x0f,0xf7,0xe0,
    0x0f,0x58,0xeb,0xf3,0x0f,0x5b,0xed,0x66,0x0f,0x6b,0xed,
    0x66,0x0f,0x67,0xed,0x66,0x0f,0xf7,0xe9);
  const transform = [
    0xf3,0x0f,0x7e,0x1e,0x66,0x0f,0x61,0xdb,0x66,0x0f,0x72,0xe3,16,0x0f,0x5b,0xdb,
    0x0f,0x59,0x1a,0x0f,0x28,0xe3,0x0f,0x58,0x62,16,0x0f,0x5c,0xe3,
    0x0f,0x29,0x27,0xf3,0x0f,0x10,0x6f,4,0x0f,0xc6,0x2f,0x64,
    0x0f,0xc6,0xec,0x88,0x0f,0x59,0xec,0xf3,0x0f,0x5b,0xed,
    0x66,0x0f,0x72,0xe5,255,0x66,0x0f,0x6b,0xed,0x66,0x0f,0x71,0xe5,19,
    0xf3,0x0f,0xd6,0xd6,0x66,0x0f,0xef,0xc0,0x66,0x0f,0xfd,0xea,
    0x66,0x0f,0x69,0xe5,0x66,0x0f,0xeb,0xe2,0x66,0x0f,0x6f,0xf4,
    0x66,0x0f,0x7f,0x77,16];
  async function run(kind,test,disabled) {
    // Neither window begins at a BB entry. Integer instructions separate them,
    // and ADC consumes flags after the second window.
    const p = kind==='gather' ? [0x90,...gather] : [0x90,0x31,0xc0,...transform,0x41,...transform,0x83,0xd0,0];
    p.push(0xff,0x4d,0xfc); p.push(0x0f,0x85,...imm(-(p.length+6)),0xf4);
    wasm.jit_clear_cache_js(); wasm.set_jit_config(0,Number(disabled)); wasm.set_jit_config(5,32);
    configure(Uint8Array.from(p),0);
    let view=new DataView(memory.buffer,memory.byteOffset,memory.byteLength);
    const step=async budget => {
      const reason=wasm.vine_execute_budget(budget); await Promise.resolve(); refreshViews();
      view=new DataView(memory.buffer,memory.byteOffset,memory.byteLength); return reason;
    };
    const reg=(r,n)=>state.setUint32(stateOffset.registers+r*4,n,true);
    view.setUint32(0x1f0000,0x1f1007,true);
    for (let page=0;page<512;page++) view.setUint32(0x1f1000+page*4,page*4096|7,true);
    state.setUint32(stateOffset.controlRegisters+12,0x1f0000,true);
    state.setUint32(stateOffset.controlRegisters,state.getUint32(stateOffset.controlRegisters,true)|0x80010000,true);
    wasm.full_clear_tlb();
    const crossing=['cross','noncontiguous','second-missing'].includes(test);
    const coeff=crossing?(kind==='gather'?0x120800:0x120ff0):0x120000;
    let dst=test==='alias'?coeff:test==='dst-cross'?0x140ff0:
      (test==='unaligned'||kind==='gather'?0x140001:0x140000);
    for (let i=0;i<8;i++) {
      view.setInt16(0x40000+i*2,i*67-128,true);
      view.setInt16(0x50000+i*2,[0,127,255,512,-5,53,8191,85][i],true);
    }
    for (let i=0;i<1024;i++) view.setFloat32(coeff+i*4,(i%32-16)*1.25,true);
    if (test==='special') for (let i=0;i<1024;i++) view.setUint32(coeff+i*4,
      [0x7fc00123,0xffc00987,0x7f800000,0xff800000,0x4f000000,0xcf000000,1,0x80000001][i%8],true);
    memory.fill(0x55,dst,dst+32);
    [0,0,coeff,0x10000,0x1e000,0x20000,0x40000,dst].forEach((n,r)=>reg(r,n));
    new Uint8Array(wasm.memory.buffer,stateOffset.xmm,128).fill(0);
    for (let i=0;i<32;i++) state.setUint8(stateOffset.xmm+i,(i%3?0:0x80)|i);
    state.setUint32(824,0x1f80,true); view.setInt32(0x1fffc,262144,true);
    const before=batchCount();
    for(let i=0;i<3;i++) assert.equal(await step(500003),stopReason.budget);
    const warmed=batchCount();
    if (!disabled&&wasm.vine_vector_batches&&test!=='unaligned') assert(warmed>before,kind+': compiler did not execute');
    state.setUint32(stateOffset.instructionPointer,guest.codeAddress,true);
    state.setUint32(stateOffset.previousInstructionPointer,guest.codeAddress,true);
    view.setInt32(0x1fffc,65536,true);
    if(test==='code-destination') {
      dst=0x1800;reg(7,dst);memory.fill(0x55,dst,dst+32);
      view.setInt32(0x1fffc,256,true);
    }
    if(test==='missing') {view.setUint32(0x1f1000+(coeff>>>12)*4,0,true);wasm.full_clear_tlb();}
    if(test==='readonly') {view.setUint32(0x1f1000+(dst>>>12)*4,(dst&~4095)|5,true);wasm.full_clear_tlb();}
    if(test==='rearm') wasm.vine_reset_dirty_pages();
    if(test==='remap'||test==='noncontiguous') {
      const page=test==='remap'?coeff&~4095:0x121000;
      memory.copyWithin(0x160000,page,page+4096);
      view.setUint32(0x1f1000+(page>>>12)*4,0x160007,true);wasm.full_clear_tlb();
    }
    if(test==='second-missing') {view.setUint32(0x1f1000+0x121*4,0,true);wasm.full_clear_tlb();}
    if(test==='code') {memory[guest.codeAddress]=0x40;wasm.jit_dirty_cache(guest.codeAddress,guest.codeAddress+1);}
    if(test==='mxcsr') state.setUint32(824,0x3f80,true);
    if(test==='ts'||test==='em') state.setUint32(stateOffset.controlRegisters,
      state.getUint32(stateOffset.controlRegisters,true)|(test==='ts'?8:4),true);
    if(test==='cr4') state.setUint32(stateOffset.controlRegisters+16,0,true);
    if(test==='mask-zero'||test==='mask-all') new Uint8Array(wasm.memory.buffer,stateOffset.xmm,32).fill(test==='mask-zero'?0:255);
    const stops=[];let reason;
    for(let i=0;i<2000;i++) {
      reason=await step(i===0?500003:[1,4095,4096,8193,100129][i%5]);
      stops.push({reason,count:wasm.vine_instruction_counter(),error:wasm.vine_get_stop_error_code(),
        ip:state.getUint32(stateOffset.instructionPointer,true),
        regs:Array.from({length:8},(_,r)=>state.getUint32(stateOffset.registers+r*4,true)),flags:wasm.get_eflags(),
        mxcsr:state.getUint32(824,true),xmm:Array.from(new Uint32Array(wasm.memory.buffer,stateOffset.xmm,32)),
        top:state.getUint8(stateOffset.fpuStackPtr),tags:state.getUint8(stateOffset.fpuStackEmpty),
        mmx:Array.from({length:8},(_,r)=>Array.from(new Uint8Array(wasm.memory.buffer,stateOffset.fpuSt+r*16,10))),
        output:Array.from(memory.subarray(dst,dst+32))});
      if(reason!==stopReason.budget) break;
    }
    assert.notEqual(reason,stopReason.budget);
    const batches=batchCount()-warmed;
    if(!disabled&&wasm.vine_vector_batches&&test==='plain') assert(batches>0,'warmed window must run before tiny-budget fragmentation');
    if(!disabled&&['mxcsr','ts','em','cr4'].includes(test)) assert.equal(batches,0,kind+'/'+test);
    return stops;
  }
  for(const kind of ['gather','transform']) {
    const tests=['plain','cross','noncontiguous','second-missing','alias','missing','readonly','remap','rearm','code','code-destination','mxcsr','ts','em','cr4'];
    tests.push(...(kind==='gather'?['special','mask-zero','mask-all']:['unaligned','dst-cross']));
    for(const test of tests) {
      const reference=await run(kind,test,true),candidate=await run(kind,test,false);
      assert.equal(candidate.length,reference.length,kind+'/'+test);
      for(let i=0;i<reference.length;i++) {
        // Existing v86 charges the unretired BB tail on faults. Its EIP, error,
        // bytes and registers must still match; normal slices count exactly.
        if(reference[i].reason>2) {
          const excess=candidate[i].count-reference[i].count;
          assert(excess>=0&&excess<64);
          candidate[i].count-=excess;
        }
        if(test==='code-destination') {
          // Existing SMC exits precharge the remaining BB tail (v84 baseline).
          const excess=kind==='gather'?1:7;
          assert.equal(candidate[i].count-reference[i].count,excess,kind);
          candidate[i].count-=excess;
        }
        assert.deepEqual(candidate[i],reference[i],kind+'/'+test+'/'+i);
      }
    }
  }
}
await checkVectorWindows();
// A side entry may reach a counted loop after the enclosing JIT has exhausted
// its allowance. Disabling the outer reloop check makes that case deterministic.
if (wasm.vine_affine_iterations) {
  wasm.jit_clear_cache_js();
  wasm.set_jit_config(0, 0); wasm.set_jit_config(2, 0);
  wasm.set_jit_config(4, 0); wasm.set_jit_config(5, 32);
  const program = Uint8Array.from([
    0x49, 0x75, 0xfd, // long bounded prefix: DEC ECX; JNZ
    0x0f, 0xb6, 0x46, 2, 0x0f, 0xb6, 0x5e, 1, 0xc1, 0xe0, 8, 0x0b, 0xc3,
    0x0f, 0xb6, 0x1e, 0xc1, 0xe0, 8, 0x09, 0xd8, 0x89, 0x07,
    0x83, 0xc7, 4, 0x83, 0xc6, 3, 0x4a, 0x75, 0xe0, 0xf4,
  ]);
  configure(program, 0);
  const setReg = (r, value) => state.setUint32(stateOffset.registers + r * 4, value, true);
  setReg(1, 100000); setReg(2, 8192); setReg(6, 0x40000); setReg(7, 0x140000);
  let warmed;
  for (let i=0; i<20; i++) {
    warmed = wasm.vine_execute_budget(50000); await Promise.resolve(); refreshViews();
    if (warmed !== stopReason.budget) break;
  }
  assert.equal(warmed, stopReason.halt);
  setReg(1, 100000); setReg(2, 8192); setReg(6, 0x40000); setReg(7, 0x140000);
  state.setUint8(stateOffset.inHalt, 0);
  state.setUint32(stateOffset.instructionPointer, guest.codeAddress, true);
  state.setUint32(stateOffset.previousInstructionPointer, guest.codeAddress, true);
  for (let i=0; i<8192*3; i++) memory[0x40000+i]=(i*19)&255;
  const before = wasm.vine_affine_iterations();
  let reason;
  for (let i=0; i<20; i++) {
    reason = wasm.vine_execute_budget(50000);
    await Promise.resolve(); refreshViews();
    if (reason !== stopReason.budget) break;
  }
  assert.equal(reason, stopReason.halt);
  assert(wasm.vine_affine_iterations() - before > 4000,
    'exhausted enclosing JIT budget stranded a counted loop on the slow path: ' + (wasm.vine_affine_iterations() - before));
  for (let i=0; i<8192; i++) assert.deepEqual(
    memory.subarray(0x140000+i*4,0x140000+i*4+4),
    Uint8Array.of(memory[0x40000+i*3],memory[0x40001+i*3],memory[0x40002+i*3],0));
  wasm.set_jit_config(2, 1); wasm.set_jit_config(4, 1);
}
wasm.set_jit_config(5, 200_000);
wasm.jit_clear_cache_js();

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

// Lazy EFLAGS must survive compiled branches, helper calls, exact-budget exits
// and a fault after earlier arithmetic has retired. Exercise all operand widths.
const lazyFlagsProgram = [
  0xb9, 0xa0, 0x86, 0x01, 0, // mov ecx, 100000
  0xbc, 0, 0xf0, 0x01, 0, // mov esp, 0x1f000
  0xb8, 0xff, 0xff, 0xff, 0x7f,
  0xba, 0x67, 0x45, 0x23, 0x81,
];
const lazyFlagsLoop = lazyFlagsProgram.length;
lazyFlagsProgram.push(
  0x01, 0xd0, // add eax, edx
  0x14, 0x7f, // adc al, 0x7f (helper consumes and produces flags)
  0x66, 0x1d, 0xff, 0xff, // sbb ax, 0xffff
  0x40, 0x66, 0x48, // inc eax; dec ax (preserve carry)
  0xa8, 0x80, 0x74, 0x02, 0x34, 0x55, // test al; jz; xor al
  0x66, 0x39, 0xd0, // cmp ax, dx
  0x0f, 0x92, 0xc3, 0x0f, 0x9c, 0xc7, // setb bl; setl bh
  0xf9, 0xf5, 0xf8, // stc; cmc; clc
  0x31, 0xc2, 0x28, 0xd0, // xor edx, eax; sub al, dl
  0x9f, 0x9e, // lahf helper; sahf
  0xd1, 0xc2, // rol edx, 1
  0xd3, 0xe0, 0xd3, 0xea, 0xd3, 0xfb, // shifts include zero and masked counts
  0x6b, 0xdb, 0x7f, // imul ebx, ebx, 127
  0x0f, 0xa3, 0xc8, 0x11, 0xd0, // bt eax, ecx; adc eax, edx
  0xfd, 0xfc, // std; cld
  0x9c, 0x5e, // pushf helper; pop esi
  0x01, 0xf0, // flags stay live across integer-flag-transparent x87/MMX helpers
  0xd9, 0x05, 0, 0x20, 0, 0, // fld dword [0x2000]
  0xd8, 0x0d, 0x04, 0x20, 0, 0, // fmul dword [0x2004]
  0xd9, 0x1d, 0x08, 0x20, 0, 0, // fstp dword [0x2008]
  0x0f, 0xef, 0xc0, 0x0f, 0xd5, 0xc0, 0x0f, 0x67, 0xc0, // MMX
  0x0f, 0x77, 0x11, 0xd0, // emms; adc eax, edx consumes preserved carry
  0x66, 0x0f, 0x57, 0xc0, // xorpd xmm0, xmm0
  0x66, 0x0f, 0x2e, 0xc0, 0x0f, 0x94, 0xc3, // comisd; setz bl consumes new flags
  0x01, 0xf0, // add eax, esi: every lazy-flag field is live at the fault
  0x89, 0x35, 0, 0x30, 0, 0, // mov [0x3000], esi
  0xa3, 0x04, 0x30, 0, 0, // mov [0x3004], eax
  0x49, 0x0f, 0x85,
);
lazyFlagsProgram.push(...new Uint8Array(
  new Int32Array([lazyFlagsLoop - lazyFlagsProgram.length - 4]).buffer), 0xf4);

async function runLazyFlags(disableJit, budget, fault = false) {
  wasm.jit_clear_cache_js();
  wasm.set_jit_config(0, disableJit ? 1 : 0);
  configure(Uint8Array.from(lazyFlagsProgram), 0);
  let data = new DataView(memory.buffer, memory.byteOffset);
  data.setFloat32(0x2000, 1.5, true);
  data.setFloat32(0x2004, -0.25, true);
  data.setUint32(0x1f0000, 0x1f1007, true);
  for (let page = 0; page < guest.memorySize / 4096; page += 1) {
    data.setUint32(0x1f1000 + page * 4, page * 4096 | 7, true);
  }
  state.setUint32(stateOffset.controlRegisters + 3 * 4, 0x1f0000, true);
  state.setUint32(stateOffset.controlRegisters,
    state.getUint32(stateOffset.controlRegisters, true) | 0x80010000, true);
  wasm.full_clear_tlb();
  const jitBefore = wasm.vine_jit_retired_instructions_low();
  let reason = stopReason.budget;
  let slices = 0;
  while (reason === stopReason.budget) {
    reason = wasm.vine_execute_budget(budget);
    await Promise.resolve();
    refreshViews();
    assert(++slices < 1024, "lazy flags loop exceeded its bounded slices");
    if (fault && slices === 1) {
      assert.equal(reason, stopReason.budget);
      state.setUint32(stateOffset.instructionPointer, guest.codeAddress + lazyFlagsLoop, true);
      data = new DataView(memory.buffer, memory.byteOffset);
      data.setUint32(0x1f100c, 0x3005, true); // read-only output page
      wasm.full_clear_tlb();
    }
  }
  assert.equal(reason, fault ? stopReason.pageFault : stopReason.halt);
  if (!disableJit) assert(wasm.vine_jit_retired_instructions_low() > jitBefore,
    "lazy flags loop did not execute compiled code");
  wasm.vine_cpu_state_save();
  return {
    reason,
    errorCode: wasm.vine_get_stop_error_code(),
    state: [...new Uint32Array(wasm.memory.buffer,
      wasm.vine_cpu_state_ptr(), wasm.vine_cpu_state_size() / 4)],
    instructionCount: wasm.vine_instruction_counter(),
    output: [...memory.subarray(0x3000, 0x3008)],
  };
}
const lazyFlagsReference = await runLazyFlags(true, 250_000);
for (const budget of [250_000, 65_537, 8_193]) {
  assert.deepEqual(await runLazyFlags(false, budget), lazyFlagsReference,
    `lazy flags JIT diverged at budget ${budget}`);
}
const lazyFlagsFaultReference = await runLazyFlags(true, 250_000, true);
const lazyFlagsFaultCompiled = await runLazyFlags(false, 250_000, true);
assert(lazyFlagsFaultCompiled.instructionCount >= lazyFlagsFaultReference.instructionCount
  && lazyFlagsFaultCompiled.instructionCount <= lazyFlagsFaultReference.instructionCount + 40);
// v86's fault accounting includes the unretired block tail and retains its entry
// in previous_ip; EIP, lazy flags, registers and output bytes still must match.
for (const result of [lazyFlagsFaultReference, lazyFlagsFaultCompiled]) {
  delete result.instructionCount;
  result.state.splice(13, 1);
}
assert.deepEqual(lazyFlagsFaultCompiled, lazyFlagsFaultReference,
  "lazy flags JIT fault changed architectural state");

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

const mmxMoveProgram = [
  0xb9, 0xa0, 0x86, 0x01, 0x00,
  0xb8, 0x98, 0xba, 0xdc, 0xfe,
];
const mmxMoveLoop = mmxMoveProgram.length;
mmxMoveProgram.push(
  0x0f, 0x6f, 0x05, 0x00, 0x21, 0x00, 0x00,
  0x0f, 0x6f, 0xc8,
  0x0f, 0x7f, 0xca,
  0xf3, 0x0f, 0xd6, 0xda,
  0xf2, 0x0f, 0xd6, 0xe3,
  0x0f, 0x7f, 0x25, 0x08, 0x21, 0x00, 0x00,
);
const mmxUnpackCases = [
  { code: [0x0f, 0x6e, 0xe8], register: 5, expected: [0x98, 0xba, 0xdc, 0xfe, 0, 0, 0, 0] },
  { code: [0x0f, 0x6e, 0x35, 0x00, 0x21, 0, 0], register: 6, expected: [1, 0x23, 0x45, 0x67, 0, 0, 0, 0] },
  { code: [0x0f, 0x60, 0xee], register: 5, expected: [0x98, 1, 0xba, 0x23, 0xdc, 0x45, 0xfe, 0x67] },
  { code: [0x0f, 0x60, 0x35, 0x00, 0x21, 0, 0], register: 6, expected: [1, 1, 0x23, 0x23, 0x45, 0x45, 0x67, 0x67] },
  { code: [0x0f, 0x60, 0xed], register: 5, expected: [0x98, 0x98, 1, 1, 0xba, 0xba, 0x23, 0x23] },
  { code: [0x0f, 0x61, 0xf5], register: 6, expected: [1, 1, 0x98, 0x98, 0x23, 0x23, 1, 1] },
  { code: [0x0f, 0x61, 0x35, 0x00, 0x21, 0, 0], register: 6, expected: [1, 1, 1, 0x23, 0x98, 0x98, 0x45, 0x67] },
  { code: [0x0f, 0x61, 0xed], register: 5, expected: [0x98, 0x98, 0x98, 0x98, 1, 1, 1, 1] },
];
for (const [index, { code, register }] of mmxUnpackCases.entries()) {
  mmxMoveProgram.push(...code, 0x0f, 0x7f, 0x05 | register << 3, 0x20 + index * 8, 0x21, 0, 0);
}
mmxMoveProgram.push(0x49, 0x0f, 0x85);
mmxMoveProgram.push(...new Uint8Array(new Int32Array([mmxMoveLoop - mmxMoveProgram.length - 4]).buffer), 0xf4);

async function runMmxMoves(disableJit, budget = 250_000) {
  wasm.jit_clear_cache_js();
  wasm.set_jit_config(0, disableJit ? 1 : 0);
  configure(Uint8Array.from(mmxMoveProgram), 0);
  memory.set([0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef], 0x2100);
  const compileCallsBefore = hostImportCalls.codegen_finalize ?? 0;
  let reason = stopReason.budget;
  let slices = 0;
  while (reason === stopReason.budget) {
    reason = wasm.vine_execute_budget(budget);
    await Promise.resolve();
    refreshViews();
    slices += 1;
    assert(slices < 512, "MMX move/unpack loop did not halt within its bounded slices");
  }
  assert.equal(reason, stopReason.halt);
  return {
    output: [...memory.subarray(0x2108, 0x2110)],
    unpack: [...memory.subarray(0x2120, 0x2120 + mmxUnpackCases.length * 8)],
    xmm3: [
      ...new Uint8Array(wasm.memory.buffer, stateOffset.xmm + 3 * 16, 16),
    ],
    mmxMetadata: [0, 1, 2, 4, 5, 6].map((register) =>
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
assert.deepEqual(mmxMoveJit.unpack, mmxUnpackCases.flatMap(({ expected }) => expected));
assert.deepEqual(mmxMoveJit.mmxMetadata, [0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff]);
assert.equal(mmxMoveJit.fpuStackEmpty, 0);
assert.equal(mmxMoveJit.fpuStackPtr, 0);
for (const budget of [8_193, 32_771]) {
  assert.deepEqual(await runMmxMoves(false, budget), mmxMoveJit,
    `MMX move/unpack JIT diverged at short budget ${budget}`);
}

for (const opcode of [0x6e, 0x60, 0x61, 0xd5, 0xfd, 0xf9, 0x67, 0xe5]) {
  const width = [0x6e, 0x60, 0x61].includes(opcode) ? 4 : 8;
  for (const disableJit of [true, false]) {
    wasm.jit_clear_cache_js();
    wasm.set_jit_config(0, Number(disableJit));
    configure(Uint8Array.from([
      0xb9, 0xa0, 0x86, 0x01, 0x00,
      0x0f, opcode, 0x3d, 0x100 - width, 0x2f, 0, 0,
      0x49, 0x75, 0xf6, 0xf4,
    ]), 0);
    let data = new DataView(memory.buffer, memory.byteOffset, memory.byteLength);
    data.setUint32(0x1f0000, 0x1f1007, true);
    data.setUint32(0x1f1004, 0x1007, true);
    data.setUint32(0x1f1008, 0x2007, true);
    data.setUint32(0x1f100c, 0, true);
    data.setUint32(0x2ffc, 0xfedcba98, true);
    state.setUint32(stateOffset.controlRegisters + 12, 0x1f0000, true);
    state.setUint32(stateOffset.controlRegisters,
      state.getUint32(stateOffset.controlRegisters, true) | 0x80000000, true);
    wasm.full_clear_tlb();
    const jitBefore = wasm.vine_jit_retired_instructions_low();
    for (const budget of [250_000, 8_193]) {
      assert.equal(wasm.vine_execute_budget(budget), stopReason.budget,
        `MMX memory operand read beyond its ${width} bytes at the page boundary`);
      await Promise.resolve();
      refreshViews();
    }
    if (!disableJit) assert(wasm.vine_jit_retired_instructions_low() !== jitBefore);
    data = new DataView(memory.buffer, memory.byteOffset, memory.byteLength);
    data.setUint32(0x1f1008, 0, true);
    const mm7 = stateOffset.fpuSt + 7 * 16;
    state.setBigUint64(mm7, 0x0123456789abcdefn, true);
    state.setUint16(mm7 + 8, 0x2345, true);
    state.setUint8(stateOffset.fpuStackEmpty, 0x55);
    state.setUint8(stateOffset.fpuStackPtr, 3);
    state.setUint32(stateOffset.instructionPointer, guest.codeAddress + 5, true);
    wasm.full_clear_tlb();
    assert.equal(wasm.vine_execute_budget(4096), stopReason.pageFault);
    assert.equal(wasm.vine_get_stop_error_code(), 0);
    assert.equal(state.getUint32(stateOffset.instructionPointer, true), guest.codeAddress + 5);
    assert.equal(state.getBigUint64(mm7, true), 0x0123456789abcdefn);
    assert.equal(state.getUint16(mm7 + 8, true), 0x2345);
    assert.equal(state.getUint8(stateOffset.fpuStackEmpty), 0x55);
    assert.equal(state.getUint8(stateOffset.fpuStackPtr), 3);
    data.setUint32(0x1f1008, 0x2007, true);
    wasm.full_clear_tlb();
    assert.equal(wasm.vine_execute_budget(400_000), stopReason.halt);
    assert.equal(state.getUint16(mm7 + 8, true), 0xffff);
    assert.equal(state.getUint8(stateOffset.fpuStackEmpty), 0);
    assert.equal(state.getUint8(stateOffset.fpuStackPtr), 0);
  }
}

const mmxArithmeticCases = [];
const mmxWords = [0, 1, 0x7fff, 0x8000, 0xffff, 0xff, 0x100, 0x8001];
const signedWord = (n) => n << 16 >> 16;
const wordBytes = (words) => words.flatMap(n => [n & 0xff, n >>> 8 & 0xff]);
for (const opcode of [0xd5, 0xfd, 0xf9, 0x67, 0xe5]) {
  for (const variant of ["memory", "register", "alias"]) {
    for (let pattern = 0; pattern < 2; pattern += 1) {
      const a = mmxWords.slice(pattern * 4, pattern * 4 + 4);
      const b = variant === "alias" ? a : mmxWords.slice((1 - pattern) * 4, (1 - pattern) * 4 + 4).reverse();
      const expected = opcode === 0x67
        ? [...a, ...b].map(n => Math.max(0, Math.min(255, signedWord(n))))
        : wordBytes(a.map((n, i) => opcode === 0xd5 ? n * b[i]
          : opcode === 0xfd ? n + b[i] : opcode === 0xf9 ? n - b[i]
            : signedWord(n) * signedWord(b[i]) >> 16));
      mmxArithmeticCases.push({ opcode, variant, a: wordBytes(a), b: wordBytes(b), expected });
    }
  }
}
for (const group of [2, 4, 6]) {
  for (const shift of [0, 1, 15, 16, 17, 255]) {
    const a = [0x8001, 0xffff, 0x7fff, 0x5555];
    const expected = wordBytes(a.map(n => group === 4 ? signedWord(n) >> Math.min(shift, 15)
      : shift >= 16 ? 0 : group === 2 ? n >>> shift : n << shift));
    mmxArithmeticCases.push({ group, shift, a: wordBytes(a), b: Array(8).fill(0), expected });
  }
}
const mmxArithmeticProgram = [0xb9, 0, 0x20, 0, 0];
const mmxArithmeticStart = mmxArithmeticProgram.length;
const imm32 = n => [n & 255, n >>> 8 & 255, n >>> 16 & 255, n >>> 24 & 255];
for (const [i, test] of mmxArithmeticCases.entries()) {
  const r = i % 8, s = (r + 1) % 8;
  mmxArithmeticProgram.push(0x0f, 0x6f, 0x05 | r << 3, ...imm32(0x4000 + i * 16));
  if (test.group !== undefined) {
    mmxArithmeticProgram.push(0x0f, 0x71, 0xc0 | test.group << 3 | r, test.shift);
  } else if (test.variant === "memory") {
    mmxArithmeticProgram.push(0x0f, test.opcode, 0x05 | r << 3, ...imm32(0x4008 + i * 16));
  } else {
    if (test.variant === "register") {
      mmxArithmeticProgram.push(0x0f, 0x6f, 0x05 | s << 3, ...imm32(0x4008 + i * 16));
    }
    mmxArithmeticProgram.push(0x0f, test.opcode, 0xc0 | r << 3 | (test.variant === "alias" ? r : s));
  }
  mmxArithmeticProgram.push(0x0f, 0x7f, 0x05 | r << 3, ...imm32(0x6000 + i * 8));
}
mmxArithmeticProgram.push(0x49, 0x0f, 0x85,
  ...imm32(mmxArithmeticStart - mmxArithmeticProgram.length - 7), 0xf4);
assert(mmxArithmeticProgram.length < 4096);
async function runMmxArithmetic(disableJit, budget = 250_000, iterations = 8192) {
  wasm.jit_clear_cache_js();
  wasm.set_jit_config(0, disableJit ? 1 : 0);
  const program = Uint8Array.from(mmxArithmeticProgram);
  new DataView(program.buffer).setUint32(1, iterations, true);
  configure(program, 0);
  for (const [i, test] of mmxArithmeticCases.entries()) memory.set([...test.a, ...test.b], 0x4000 + i * 16);
  const before = wasm.vine_jit_retired_instructions_low();
  let reason = stopReason.budget, slices = 0;
  while (reason === stopReason.budget) {
    reason = wasm.vine_execute_budget(budget);
    await Promise.resolve();
    refreshViews();
    assert(++slices < 512, "MMX arithmetic exceeded its bounded slices");
  }
  assert.equal(reason, stopReason.halt);
  if (!disableJit) assert(wasm.vine_jit_retired_instructions_low() > before,
    "MMX arithmetic did not execute compiled instructions");
  const output = [...memory.subarray(0x6000, 0x6000 + mmxArithmeticCases.length * 8)];
  assert.deepEqual(output, mmxArithmeticCases.flatMap(test => test.expected));
  wasm.vine_cpu_state_save();
  return { output, instructions: wasm.vine_instruction_counter(),
    state: [...new Uint32Array(wasm.memory.buffer, wasm.vine_cpu_state_ptr(), wasm.vine_cpu_state_size() / 4)] };
}
const mmxArithmeticReference = await runMmxArithmetic(true);
assert.deepEqual(await runMmxArithmetic(false), mmxArithmeticReference);
assert.deepEqual(await runMmxArithmetic(false, 8_193), mmxArithmeticReference);
if (process.argv.includes("--measure-mmx")) {
  const reference = await runMmxArithmetic(true, 250_000, 262_144);
  const milliseconds = [];
  for (let trial = 0; trial < 7; trial += 1) {
    const started = performance.now();
    assert.deepEqual(await runMmxArithmetic(false, 250_000, 262_144), reference);
    milliseconds.push(performance.now() - started);
  }
  console.log(JSON.stringify({ mmxArithmetic: { instructions: reference.instructions,
    medianMilliseconds: milliseconds.toSorted((a, b) => a - b)[3], milliseconds } }));
}

const x87MemoryProgram = [0xb9, 0xa0, 0x86, 0x01, 0x00];
const x87LoopStart = x87MemoryProgram.length;
x87MemoryProgram.push(0xd9, 0x05, 0x00, 0x20, 0x00, 0x00);
for (const [index, operation] of [0, 1, 4, 5, 6, 7].entries()) {
  x87MemoryProgram.push(0xd8, 0x05 | operation << 3, 4 + index * 4, 0x20, 0, 0);
}
x87MemoryProgram.push(
  0xd9, 0x15, 0x1c, 0x20, 0, 0,
  0xd9, 0x1d, 0x20, 0x20, 0, 0,
  0x49, 0x75,
);
x87MemoryProgram.push((x87LoopStart - x87MemoryProgram.length - 1) & 0xff, 0xf4);

async function runX87Memory(disableJit, values, controlWord, budget = 250_000, fault = null,
  program = x87MemoryProgram, stackTop = 0) {
  wasm.jit_clear_cache_js();
  wasm.set_jit_config(0, disableJit ? 1 : 0);
  configure(Uint8Array.from(program), 0);
  new Uint8Array(wasm.memory.buffer, stateOffset.fpuSt, 8 * 16).fill(0);
  state.setUint8(stateOffset.fpuStackPtr, stackTop);
  wasm.set_control_word(controlWord);
  const data = new DataView(memory.buffer, memory.byteOffset);
  values.forEach((value, index) => data.setUint32(0x2000 + index * 4, value, true));
  data.setUint32(0x1f0000, 0x1f1007, true);
  for (let page = 0; page < guest.memorySize / 4096; page += 1) {
    data.setUint32(0x1f1000 + page * 4, page * 4096 | 7, true);
  }
  state.setUint32(stateOffset.controlRegisters + 3 * 4, 0x1f0000, true);
  state.setUint32(stateOffset.controlRegisters,
    state.getUint32(stateOffset.controlRegisters, true) | 0x80000000, true);
  wasm.full_clear_tlb();
  wasm.vine_reset_dirty_pages();
  const jitBefore = wasm.vine_jit_retired_instructions_low();
  const compiledBefore = hostImportCalls.codegen_finalize ?? 0;
  let reason = stopReason.budget;
  let slices = 0;
  while (reason === stopReason.budget) {
    reason = wasm.vine_execute_budget(budget);
    await Promise.resolve();
    refreshViews();
    assert(++slices < 512, "x87 memory loop exceeded its bounded slices");
    if (fault !== null && slices === 1) {
      assert.equal(reason, stopReason.budget);
      state.setUint32(stateOffset.instructionPointer, guest.codeAddress + x87LoopStart, true);
      state.setUint8(stateOffset.fpuStackEmpty, 0xff);
      state.setUint8(stateOffset.fpuStackPtr, 0);
      // Fault only after the loop is compiled. A failed store must not pop ST0.
      data.setUint32(0x1f1000 + 2 * 4, fault === "read" ? 0 : 0x2005, true);
      state.setUint32(stateOffset.controlRegisters,
        state.getUint32(stateOffset.controlRegisters, true) | 0x10000, true);
      wasm.full_clear_tlb();
    }
  }
  assert.equal(reason, fault === null ? stopReason.halt : stopReason.pageFault);
  if (fault === "write") assert.equal(state.getUint8(stateOffset.fpuStackPtr), 7);
  if (!disableJit) {
    assert((wasm.vine_jit_retired_instructions_low() - jitBefore) >>> 0,
      `x87 memory loop did not execute compiled instructions: retired=${wasm.vine_instruction_counter()}, compilations=${(hostImportCalls.codegen_finalize ?? 0) - compiledBefore}`);
  }
  wasm.vine_cpu_state_save();
  return {
    reason,
    errorCode: wasm.vine_get_stop_error_code(),
    state: Array.from(new Uint32Array(wasm.memory.buffer,
      wasm.vine_cpu_state_ptr(), wasm.vine_cpu_state_size() / 4)),
    instructionCount: wasm.vine_instruction_counter(),
    output: [...memory.subarray(0x201c, 0x2024)],
    dirtyPages: Array.from(new Uint32Array(wasm.memory.buffer,
      wasm.vine_dirty_pages_ptr(), wasm.vine_dirty_page_count())).sort((a, b) => a - b),
  };
}

const x87MemoryCases = [
  [0x3fc00000, 0x3e800000, 0xbf000000, 0x42000000, 0x40b80000, 0x3f333333, 0x4112147b],
  [0x4b800000, 0x3f800000, 0x3f800000, 0x4b800000, 0x3f800000, 0x40400000, 0x3f800000],
  [0x00000001, 0x80000001, 0x3f800000, 0x80000000, 0x00800001, 0x7f7fffff, 0x00800001],
  [0x80000000, 0x80000000, 0x3f800000, 0x00000000, 0x80000000, 0x3f800000, 0x80000000],
  [0x7f800000, 0xff800000, 0x00000000, 0x3f800000, 0x3f800000, 0x00000000, 0x7f800000],
  [0x7fc01234, 0x3f800000, 0x7f801234, 0xffc05678, 0x3f800000, 0x3f800000, 0x3f800000],
];
for (const precision of [0, 2, 3]) {
  for (const rounding of [0, 1, 2, 3]) {
    const controlWord = 0x7f | precision << 8 | rounding << 10;
    for (const values of x87MemoryCases) {
      const reference = await runX87Memory(true, values, controlWord);
      const compiled = await runX87Memory(false, values, controlWord);
      assert.deepEqual(compiled, reference,
        `x87 memory JIT diverged at control word ${controlWord.toString(16)}`);
    }
  }
}
if (exportNames.has("vine_fpu_push_m32")) {
  const reference = await runX87Memory(true, x87MemoryCases[0], 0x37f);
  for (const budget of [8_193, 32_771, 65_537]) {
    const compiled = await runX87Memory(false, x87MemoryCases[0], 0x37f, budget);
    assert.deepEqual(compiled, reference,
      `x87 memory JIT diverged at short budget ${budget}`);
  }
}
for (const fault of ["read", "write"]) {
  const { instructionCount: interpretedCount, ...reference } =
    await runX87Memory(true, x87MemoryCases[0], 0x37f, 250_000, fault);
  const { instructionCount: compiledCount, ...compiled } =
    await runX87Memory(false, x87MemoryCases[0], 0x37f, 250_000, fault);
  // v86 accounts the complete compiled block on a fault; architectural state
  // and dirty bytes must still match, with at most the unexecuted loop tail counted.
  assert(compiledCount >= interpretedCount && compiledCount <= interpretedCount + 10);
  // Its internal previous_ip also retains the compiled entry on a fault. The
  // actual EIP (word 12), x87 registers, status, tags and all other state must match.
  assert.equal(compiled.state[13], guest.codeAddress + x87LoopStart);
  assert.equal(reference.state[13], reference.state[12]);
  compiled.state.splice(13, 1);
  reference.state.splice(13, 1);
  assert.deepEqual(compiled, reference, `x87 JIT ${fault} fault changed architectural state`);
}

// Register reads must preserve operand order, TOP wrapping and empty-tag faults.
for (const empty of [false, true]) {
  const program = [0xb9, 0xa0, 0x86, 0x01, 0x00];
  const loop = program.length;
  program.push(0xd9, 0x05, 0x00, 0x20, 0, 0, 0xd9, 0x05, 0x04, 0x20, 0, 0,
    0xd9, 0xc0, 0xd8, empty ? 0xc7 : 0xc1, 0xdc, 0xc1, 0xde, 0xe9,
    0xd9, 0x1d, 0x1c, 0x20, 0, 0, 0xd9, 0x1d, 0x20, 0x20, 0, 0, 0x49, 0x75);
  program.push((loop - program.length - 1) & 0xff, 0xf4);
  for (let top = 0; top < 8; top++) {
    for (const controlWord of [0x27f, 0x37f]) {
      const args = [x87MemoryCases[0], controlWord, 250_000, null, program, top];
      assert.deepEqual(await runX87Memory(false, ...args), await runX87Memory(true, ...args),
        `x87 register read diverged: empty=${empty}, top=${top}, cw=${controlWord}`);
    }
  }
}

const forbiddenCalls = Object.keys(hostImportCalls).filter(
  (name) =>
    name.startsWith("mmap_") ||
    name.startsWith("io_port_"),
);
assert.deepEqual(forbiddenCalls, []);

// The island and its interpreter share F80 code: compare the unchanged fallback
// too, not just JIT against its own interpreter.
if (exportNames.has("vine_fpu_push_m32")) {
  const reference = new WebAssembly.Instance(
    // prepare:engine temporarily builds the island into v86-fallback.wasm.
    // Its saved normal artifact is the independent oracle throughout the build.
    new WebAssembly.Module(fs.readFileSync(path.join(path.dirname(defaultWasmPath), "v86-browser-v1.wasm"))),
    { env: { ...environment, __indirect_function_table: new WebAssembly.Table({
      element: "anyfunc", initial: wasmTableOffset + wasmTableSize,
    }) } },
  ).exports;
  assert.equal(reference.vine_execution_island_abi_version, undefined,
    "x87 oracle must be the unchanged normal engine, not the island candidate");
  reference.rust_init();
  reference.reset_cpu();
  const baselineState = new DataView(reference.memory.buffer);
  const candidateState = new DataView(wasm.memory.buffer);
  const check = (bits) => {
    for (const [cpu, view] of [[reference, baselineState], [wasm, candidateState]]) {
      view.setUint16(1040, 0x4100, true);
      cpu.f32_to_f80_jit(stateOffset.fpuSt, bits);
    }
    assert.equal(candidateState.getBigUint64(stateOffset.fpuSt, true),
      baselineState.getBigUint64(stateOffset.fpuSt, true));
    assert.equal(candidateState.getUint16(stateOffset.fpuSt + 8, true),
      baselineState.getUint16(stateOffset.fpuSt + 8, true));
    assert.equal(candidateState.getUint16(1040, true), baselineState.getUint16(1040, true));
  };
  for (let exponent = 0; exponent < 256; exponent += 1) {
    for (const fraction of [0, 1, 0x3fffff, 0x400000, 0x7fffff]) {
      check(exponent << 23 | fraction);
      check(0x80000000 | exponent << 23 | fraction);
    }
  }
  let seed = 0x13579bdf;
  for (let i = 0; i < 10000; i += 1) {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    check(seed);
  }

  const checkArithmetic = (left, right, controlWord) => {
    for (const operation of ["fpu_fadd", "fpu_fsub", "fpu_fmul"]) {
      const results = [];
      for (const [cpu, view] of [[reference, baselineState], [wasm, candidateState]]) {
        cpu.set_control_word(controlWord);
        view.setUint8(stateOffset.fpuStackEmpty, 0xff);
        view.setUint8(stateOffset.fpuStackPtr, 0);
        view.setUint16(1040, 0x4100, true);
        cpu.fpu_push(...left);
        cpu[operation](0, ...right);
        const st0 = stateOffset.fpuSt + view.getUint8(stateOffset.fpuStackPtr) * 16;
        results.push([
          view.getBigUint64(st0, true), view.getUint16(st0 + 8, true),
          view.getUint16(1040, true), view.getUint8(stateOffset.fpuStackEmpty),
          view.getUint8(stateOffset.fpuStackPtr),
        ]);
      }
      assert.deepEqual(results[1], results[0],
        `${operation} fallback divergence: cw=${controlWord.toString(16)}, ` +
        `left=${left.map(String)}, right=${right.map(String)}`);
    }
  };
  const values = [];
  // Zeros, denormals, pseudo-denormals, unsupported encodings, the binary64
  // exponent boundaries, full 64-bit mantissas, ties, infinities and both NaNs.
  for (const exponent of [0, 1, 0x3c00, 0x3c01, 0x3ffe, 0x3fff, 0x4000, 0x43fe, 0x43ff, 0x7ffe, 0x7fff]) {
    for (const mantissa of [0n, 1n, 0x8000000000000000n, 0x8000000000000400n,
      0x8000000000000800n, 0xc000000000000000n, 0xfffffffffffff800n, 0xffffffffffffffffn]) {
      values.push([mantissa, exponent], [mantissa, exponent | 0x8000]);
    }
  }
  for (const precision of [0, 2, 3]) {
    for (const rounding of [0, 1, 2, 3]) {
      const controlWord = 0x7f | precision << 8 | rounding << 10;
      for (const left of values) {
        for (const right of values) checkArithmetic(left, right, controlWord);
      }
    }
  }
  const next = () => seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  for (let i = 0; i < 10000; i += 1) {
    const number = () => {
      const exponent = 0x3c01 + next() % 2046 | next() & 0x8000;
      const mantissa = (BigInt(next()) << 32n | BigInt(next())) | 1n << 63n;
      return [mantissa & (i % 4 === 0 ? 0xffffffffffffffffn : 0xfffffffffffff800n), exponent];
    };
    const left = number();
    const right = number();
    checkArithmetic(left, right, 0x27f);
    // Cancellation and rounding carry, which independent random exponents miss.
    checkArithmetic(left, [right[0], left[1] ^ 0x8000], 0x27f);
  }

  const checkStore = (mantissa, exponent, controlWord) => {
    const results = [];
    for (const [cpu, view] of [[reference, baselineState], [wasm, candidateState]]) {
      cpu.set_control_word(controlWord);
      view.setUint16(1040, 0x4100, true);
      results.push([cpu.f80_to_f32(mantissa, exponent) >>> 0, view.getUint16(1040, true)]);
    }
    assert.deepEqual(results[1], results[0],
      `x87 binary32 store diverged: cw=${controlWord.toString(16)}, ` +
      `mantissa=${mantissa.toString(16)}, exponent=${exponent.toString(16)}`);
  };
  // Halfway-even/odd, rounding carry, noncanonical encodings, signed zeros,
  // exponent boundaries and special values; store rounding ignores x87 precision.
  const storeMantissas = [0n, 1n, 0x7fffffffffffffffn, 0x8000000000000000n,
    0x8000000000000001n, 0x8000007fffffffffn, 0x8000008000000000n,
    0x8000008000000001n, 0x8000018000000000n, 0xffffff8000000000n,
    0xffffffffffffffffn, 0xc000000000000000n];
  for (const precision of [0, 2, 3]) {
    for (const rounding of [0, 1, 2, 3]) {
      const controlWord = 0x7f | precision << 8 | rounding << 10;
      for (const exponent of [0, 1, 0x3f60, 0x3f7f, 0x3f80, 0x3f81, 0x3f82,
        0x3fff, 0x407d, 0x407e, 0x407f, 0x7fff]) {
        for (const mantissa of storeMantissas) {
          checkStore(mantissa, exponent, controlWord);
          checkStore(mantissa, exponent | 0x8000, controlWord);
        }
      }
    }
  }
  for (let i = 0; i < 10000; i++) {
    const mantissa = BigInt(next()) << 32n | BigInt(next()) | 1n << 63n;
    checkStore(mantissa, (0x3f00 + next() % 512) | next() & 0x8000,
      0x7f | (i % 3 === 0 ? 3 : 2) << 8 | (i % 4) << 10);
  }
}

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
  for (const name of ["aligned SIMD copy", "bounded byte copy", "overlapping forward copy"]) {
    const base = hotLoopCases.find(test => test.name === name);
    for (const fault of ["source", "destination", "readonly destination"]) {
      hotLoopCases.push({
        ...base,
        name: `${name} with a later ${fault} page fault`,
        pageOverrides: [[fault === "source" ? 0x3000 : 0x5000,
          fault === "readonly destination" ? 0x5005 : 0]],
        setup() {
          base.setup();
          for (let index = 0; index < 128; index++) memory[0x2ff0 + index] = index * 17 & 0xff;
          memory.fill(0xee, 0x4ff0, 0x5070);
          state.setUint32(stateOffset.controlRegisters,
            state.getUint32(stateOffset.controlRegisters, true) | 0x10000, true);
          state.setUint32(stateOffset.registers, name === "aligned SIMD copy" ? 0x2ff0 : 0, true);
          state.setUint32(stateOffset.registers + 4, name === "aligned SIMD copy" ? 4 : 64, true);
          state.setUint32(stateOffset.registers + 8, 0x2000, true);
          state.setUint32(stateOffset.registers + 5 * 4, 0x6000, true);
          state.setUint32(stateOffset.registers + 6 * 4, 0x2ff0, true);
          state.setUint32(stateOffset.registers + 7 * 4, 0x4ff0, true);
          new DataView(memory.buffer, memory.byteOffset).setUint32(0x6008, 0x4ff0, true);
        },
        output: [0x4ff0, 0x5070],
        extraOutput: [0x5cf8, 0x600c],
      });
    }
  }
  async function runHotLoop(test, useIsland, budget, warm = false) {
    const program = [...test.program];
    if (warm === true) {
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
  const nativeCopy = hotLoopCases.find(test => test.name === "aligned SIMD copy");
  const nativeContinuation = {
    ...nativeCopy,
    name: "native entry with a JIT continuation",
    program: [
      ...nativeCopy.program.slice(0, -1),
      0xb8, 0x00, 0x20, 0x00, 0x00, // reset source
      0xb9, 0x01, 0x00, 0x00, 0x00, // one copy iteration
      0x4b, 0x75, 0xdf, 0xf4, // 65,536 repetitions, then halt
    ],
    setup() {
      nativeCopy.setup();
      state.setUint32(stateOffset.registers + 4, 1, true);
      state.setUint32(stateOffset.registers + 3 * 4, 65_536, true);
    },
  };
  wasm.jit_clear_cache_js();
  wasm.set_jit_config(0, 1);
  const nativeContinuationReference = await runHotLoop(nativeContinuation, false, 100_000, "continuation");
  wasm.jit_clear_cache_js();
  wasm.set_jit_config(0, 0);
  const nativeContinuationJitBefore = wasm.vine_jit_retired_instructions_low();
  const nativeContinuationResult = await runHotLoop(nativeContinuation, true, 100_000, "continuation");
  assert.deepEqual(nativeContinuationResult, nativeContinuationReference,
    "native-loop continuation diverged from the interpreter");
  assert(((wasm.vine_jit_retired_instructions_low() - nativeContinuationJitBefore) >>> 0) > 0,
    "native-loop continuation never reached JIT execution");
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
