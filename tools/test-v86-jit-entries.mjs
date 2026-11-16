// Exercise the production adapter with native generated Wasm and a CPU host
// stub. No guest workload or prepared CPU artifact is needed for these checks.
import assert from "node:assert/strict";
import { build } from "esbuild";

const uleb = value => {
  const bytes = [];
  do { const byte = value & 127; value >>>= 7; bytes.push(byte | (value ? 128 : 0)); } while (value);
  return bytes;
};
const text = value => [...uleb(value.length), ...new TextEncoder().encode(value)];
const section = (id, bytes) => [id, ...uleb(bytes.length), ...bytes];
const wire = sections => new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, ...sections.flat()]);
const funcType = (parameters, result = false) => [0x60, parameters, ...Array(parameters).fill(0x7f), result ? 1 : 0, ...(result ? [0x7f] : [])];
const imported = (module, name, type) => [...text(module), ...text(name), 0, type];
const exported = (name, kind, index) => [...text(name), kind, index];
const cpuBytes = wire([
  section(1, [2, ...funcType(5, true), ...funcType(1)]),
  section(2, [3, ...imported("env", "codegen_finalize", 0), ...imported("env", "jit_clear_func", 1),
    ...text("env"), ...text("__indirect_function_table"), 1, 0x70, 0, ...uleb(1924)]),
  section(5, [1, 0, 32]), section(7, [1, ...exported("memory", 2, 0)]),
]);

function generated({ paired = true, fType = 0, gType = 1, missingF = false, memoryG = false, tableImport = true, index = 7 } = {}) {
  const imports = [tableImport ? 2 : 1, ...text("e"), ...text("m"), 2, 0, 1,
    ...(tableImport ? [...text("e"), ...text("t"), 1, 0x70, 0, ...uleb(2824)] : [])];
  const bodies = [];
  const body = bytes => bodies.push(...uleb(bytes.length + 2), 0, ...bytes, 11);
  // The valid f invokes the paired g through the imported shared table.
  const f = paired && tableImport && fType === 0
    ? [0x20, 0, ...Array.from({ length: 10 }, (_, i) => [0x41, i + 1]).flat(),
      0x41, ...uleb(1924 + index), 0x11, 1, 0]
    : fType === 2 ? [0x41, 0] : [];
  body(f);
  if (paired) body(gType === 1
    ? [0x41, 0, 0x20, 0, ...Array.from({ length: 10 }, (_, i) => [0x20, i + 1, 0x6a]).flat(), 0x36, 2, 0]
    : gType === 3 ? [0x41, 0] : []);
  const exports = [...(missingF ? [] : exported("f", 0, 0)),
    ...(memoryG ? exported("g", 2, 0) : paired ? exported("g", 0, 1) : [])];
  return wire([
    section(1, [4, ...funcType(1), ...funcType(11), ...funcType(1, true), ...funcType(11, true)]),
    section(2, imports), section(3, [paired ? 2 : 1, fType, ...(paired ? [gType] : [])]),
    section(7, [(missingF ? 0 : 1) + (paired || memoryG ? 1 : 0), ...exports]),
    section(10, [paired ? 2 : 1, ...bodies]),
  ]);
}

const bundle = await build({ entryPoints: ["browser/runtime-worker/v86-mirror.ts"], bundle: true,
  write: false, platform: "node", format: "esm", target: "node24" });
const { V86Mirror } = await import(`data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].contents).toString("base64")}`);
const NativeInstance = WebAssembly.Instance;
const nativeQueueMicrotask = globalThis.queueMicrotask;
const adapters = [], queued = [];
let injectExports = null;
let checks = 0;
const check = (condition, message) => { assert(condition, message); checks++; };
const rejects = action => { assert.throws(action); checks++; };

WebAssembly.Instance = function(module, imports) {
  const instance = new NativeInstance(module, imports);
  if (!imports.env?.codegen_finalize) {
    if (imports.e?.m && injectExports) {
      const replace = injectExports; injectExports = null;
      return { exports: replace(instance.exports) };
    }
    return instance;
  }
  const memory = instance.exports.memory;
  const record = { memory, table: imports.env.__indirect_function_table, env: imports.env,
    finalized: [], synchronous: false, base: 0x40000, poolBytes: 0 };
  adapters.push(record);
  return { exports: {
    memory,
    rust_init() { new Uint32Array(memory.buffer, 0x20000, 153).set([0x56494e45, 153]); },
    vine_cpu_state_size: () => 612, vine_cpu_state_ptr: () => 0x20000,
    vine_cpu_state_save() {}, vine_cpu_state_restore: () => 0,
    get_eflags: () => new Uint32Array(memory.buffer, 0x20000, 153)[12],
    vine_set_memory_size: () => 0, vine_reset_execute_permissions() {},
    allocate_memory(bytes) { record.poolBytes = bytes; return record.base; },
    zero_memory(address, bytes) { new Uint8Array(memory.buffer, record.base + address, bytes).fill(0); },
    vine_dirty_pages_ptr: () => 0x10000, vine_dirty_page_count: () => 0,
    vine_dirty_pages_overflowed: () => 0, vine_reset_dirty_pages() {},
    vine_set_page_executable: () => 0, full_clear_tlb() {}, jit_clear_cache_js() {}, jit_dirty_cache() {},
    get_jit_config(selector) { assert.equal(selector, 6); return Number(record.synchronous); },
    codegen_finalize_finished(index, start, stateFlags) {
      record.finalized.push({ index, start, stateFlags, f: record.table.get(1024 + index), g: record.table.get(1924 + index) });
    },
  } };
};
globalThis.queueMicrotask = callback => queued.push(callback);
function compile(record, bytes = generated(), index = 7) {
  new Uint8Array(record.memory.buffer, 0x1000, bytes.length).set(bytes);
  return record.env.codegen_finalize(index, 0x401000, 15, 0x1000, bytes.length);
}
const pair = (record, index = 7) => [record.table.get(1024 + index), record.table.get(1924 + index)];
try {
  const engine = await V86Mirror.instantiateBytes(cpuBytes, { maximumPhysicalPages: 64 });
  const first = adapters[0];
  check(first.table.length === 2824, "Both entry ranges fit the CPU's own table");
  compile(first);
  const installed = pair(first);
  check(installed.every(entry => typeof entry === "function"), "Both native entries are installed synchronously");
  check(first.finalized.length === 0 && queued.length === 1, "Finalization remains asynchronous");
  installed[0](10);
  check(new DataView(first.memory.buffer).getUint32(0, true) === 65, "f reaches g through the shared table with all eleven arguments");

  for (const options of [{ fType: 1 }, { fType: 2 }, { gType: 0 }, { gType: 3 }, { missingF: true }, { memoryG: true }]) {
    rejects(() => compile(first, generated(options)));
    assert.deepEqual(pair(first), installed); checks++;
  }
  for (const field of ["f", "g"]) {
    injectExports = exports => ({ ...exports, [field]: () => {} });
    rejects(() => compile(first));
    assert.deepEqual(pair(first), installed); checks++;
  }
  for (const index of [-1, 900, 1.5, NaN]) {
    rejects(() => compile(first, generated(), index));
    rejects(() => first.env.jit_clear_func(index));
    assert.deepEqual(pair(first), installed); checks++;
  }
  check(queued.length === 1, "Rejected entries neither publish nor schedule a completion");
  compile(first, generated({ paired: false, tableImport: false }));
  check(pair(first)[0] !== installed[0] && pair(first)[1] === null, "Legacy f-only modules need no e.t import and clear stale g");
  compile(first, generated({ index: 899 }), 899);
  check(pair(first, 899).every(entry => typeof entry === "function"), "Highest paired index is in range");
  first.env.jit_clear_func(899);
  check(pair(first, 899).every(entry => entry === null), "Invalidation clears both entries");

  const rw = { read: true, write: true, execute: false };
  const commit = (address, pages, payload = []) => engine.applyMemoryDelta({
    fromGeneration: 1n, generation: 2n,
    mappingOperations: [{ operation: "commit", owner: 1, range: { base: address, byteLength: pages * 4096 }, protection: rw }],
    pages: payload, cacheEffects: { flushTlb: true, invalidateInstructionFetch: true, invalidateGeneratedCode: true },
  });
  const bytes = new Uint8Array(4096).fill(0x5a);
  commit(0x400000, 1, [{ address: 0x400000, owner: 1, protection: rw, bytes }]);
  const before = engine.saveCpuContext();
  commit(0x401000, 16);
  check(adapters.length === 2, "Actual pool growth creates a replacement CPU adapter");
  const second = adapters[1];
  assert.deepEqual(engine.saveCpuContext(), before); checks++;
  assert.deepEqual(engine.readGuestBytes(0x400000, 4096), bytes); checks++;
  compile(second);
  const replacement = pair(second);
  first.env.jit_clear_func(7);
  check(pair(first).every(entry => entry === null), "Old owner clears both old entries");
  assert.deepEqual(pair(second), replacement); checks++;
  while (queued.length) queued.shift()();
  check(first.finalized.length === 3 && second.finalized.length === 1, "Queued finalization remains bound to each original CPU instance");
  check(first.finalized[0].f === null && first.finalized[0].g === null, "Late invalidated completion observes only the old table");
  check(second.finalized[0].f === replacement[0] && second.finalized[0].g === replacement[1], "Replacement f/g are published together before its completion");
  second.synchronous = true;
  compile(second, generated({ paired: false, tableImport: false }));
  check(queued.length === 0 && pair(second)[1] === null, "Synchronous finalization preserves the legacy CPU contract");
} finally {
  WebAssembly.Instance = NativeInstance;
  globalThis.queueMicrotask = nativeQueueMicrotask;
}
console.log(JSON.stringify({ passed: true, checks, scope: "production paired JIT ABI; native modules and pool growth, no guest workload" }));
