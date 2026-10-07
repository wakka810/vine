import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { build } from "esbuild";

// Use the production adapter and real prepared CPU. Intercept only the standard
// WebAssembly instance boundary to observe allocations and inject failed exports.
// Observe cross-adapter byte copies without adding production test APIs.
const bundle = await build({
  entryPoints: ["browser/runtime-worker/v86-mirror.ts"], bundle: true, write: false,
  platform: "node", format: "esm", target: "node24",
});
const { V86Mirror } = await import(`data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].contents).toString("base64")}`);
const cpuBytes = await readFile(process.env.VINE_TEST_CPU_WASM ?? ".work/vine-v86/build/v86-fallback.wasm");
const pageSize = 4096;
const codeBase = 0x00401000;
const entry = codeBase + pageSize - 3;
const stack = 0x00900000;
const data = 0x10001000;
const rw = { read: true, write: true, execute: false };
const rx = { read: true, write: false, execute: true };
const ro = { read: true, write: false, execute: false };
const effects = { flushTlb: true, invalidateInstructionFetch: true, invalidateGeneratedCode: true };
const noEffects = { flushTlb: false, invalidateInstructionFetch: false, invalidateGeneratedCode: false };
const NativeInstance = WebAssembly.Instance;
const nativeQueueMicrotask = globalThis.queueMicrotask;
const nativeByteSet = Uint8Array.prototype.set;
const cpuBuffers = new WeakMap();
Uint8Array.prototype.set = function(source, offset) {
  const result = nativeByteSet.call(this, source, offset);
  const destination = cpuBuffers.get(this.buffer);
  const origin = cpuBuffers.get(source?.buffer);
  if (destination !== undefined && origin !== undefined && destination !== origin) {
    destination.copiedBytes += source.byteLength;
    destination.copyCalls += 1;
  }
  return result;
};
let adapters = [];
let failNextAdapter;
let heldFinalizers = null;

WebAssembly.Instance = function(module, imports) {
  let record;
  const actualImports = imports.env?.codegen_finalize ? {
    ...imports, env: { ...imports.env, codegen_finalize(...args) {
      record.compiles += 1;
      return imports.env.codegen_finalize(...args);
    } },
  } : imports;
  const instance = new NativeInstance(module, actualImports);
  const raw = instance.exports;
  if (typeof raw.vine_cpu_state_ptr !== "function") return instance;
  const failure = failNextAdapter;
  failNextAdapter = undefined;
  record = { raw, poolBytes: 0, base: 0, compiles: 0, finalizes: 0, copiedBytes: 0, copyCalls: 0,
    fullClears: 0, rearmedPages: 0, stoppedResumes: 0, forceDirtyOverflow: false };
  adapters.push(record);
  return { exports: {
    ...raw,
    allocate_memory(bytes) {
      if (failure === "allocate") return 0;
      record.poolBytes = bytes;
      record.base = raw.allocate_memory(bytes);
      cpuBuffers.set(raw.memory.buffer, record);
      return record.base;
    },
    vine_set_memory_size(bytes) {
      return failure === "memory-size" ? 1 : raw.vine_set_memory_size(bytes);
    },
    full_clear_tlb() { record.fullClears += 1; return raw.full_clear_tlb(); },
    vine_rearm_dirty_page(address) { record.rearmedPages += 1; return raw.vine_rearm_dirty_page(address); },
    vine_cpu_apply_stopped_resume(...args) { record.stoppedResumes += 1; return raw.vine_cpu_apply_stopped_resume(...args); },
    vine_dirty_pages_overflowed() { return record.forceDirtyOverflow ? 1 : raw.vine_dirty_pages_overflowed(); },
    vine_reset_dirty_pages() { record.forceDirtyOverflow = false; return raw.vine_reset_dirty_pages(); },
    vine_dirty_pages_ptr() {
      return failure === "dirty-pointer" ? raw.memory.buffer.byteLength : raw.vine_dirty_pages_ptr();
    },
    zero_memory(address, bytes) {
      if (failure === "zero") throw new Error("injected zero failure");
      return raw.zero_memory(address, bytes);
    },
    vine_set_page_executable(address, executable) {
      return failure === "execute" ? 1 : raw.vine_set_page_executable(address, executable);
    },
    vine_cpu_state_restore() {
      return failure === "restore" ? 1 : raw.vine_cpu_state_restore();
    },
    codegen_finalize_finished(...args) {
      record.finalizes += 1;
      return raw.codegen_finalize_finished(...args);
    },
  } };
};
globalThis.queueMicrotask = callback => {
  if (heldFinalizers === null) nativeQueueMicrotask(callback);
  else heldFinalizers.push(callback);
};

function page(address, protection = rw, fill = 0, owner = 1) {
  return { address, owner, protection, ownerSealed: true, dirty: true,
    bytes: new Uint8Array(pageSize).fill(fill) };
}
function region(start, byteLength, owner = 1) {
  return { owner, kind: "anonymous", range: { start, byteLength } };
}
function delta(mappingOperations = [], pages = [], cacheEffects = effects) {
  return { fromGeneration: 1n, generation: 2n, mappingOperations, pages, cacheEffects };
}
function operation(operation, base, pages = 1, protection = rw, owner = 1) {
  return { operation, owner, range: { base, byteLength: pages * pageSize }, protection };
}
function commit(engine, base, pages, protection = rw) {
  engine.applyMemoryDelta(delta([operation("commit", base, pages, protection)]));
}
function current() { return adapters.at(-1); }
function jitRetired(record) {
  return BigInt(record.raw.vine_jit_retired_instructions_low() >>> 0) |
    (BigInt(record.raw.vine_jit_retired_instructions_high() >>> 0) << 32n);
}
function physical(record) {
  return new Uint8Array(record.raw.memory.buffer, record.base, record.poolBytes);
}
function frame(engine, record, address) {
  const words = new Uint32Array(record.raw.memory.buffer, record.base, record.poolBytes / 4);
  const directory = engine.saveCpuContext().opaqueWords[61];
  const table = words[(directory >>> 2) + (address >>> 22)] & 0xfffff000;
  assert.notEqual(table, 0, "page directory entry is present");
  const pte = words[(table >>> 2) + ((address >>> 12) & 1023)];
  assert(pte & 1, "page table entry is present");
  return pte & 0xfffff000;
}
function run(engine, budget = 4096) {
  const slice = engine.executeBudget(budget);
  assert.equal(slice.stop.kind, "budget-yield", JSON.stringify(slice.stop));
  return slice;
}
function startAt(engine, address) {
  engine.applyCpuResume({ instructionPointer: address, stackPointer: stack + 4000, eax: null });
}
async function fixture(maximumPhysicalPages = 4096, extraPages = []) {
  adapters = [];
  const engine = await V86Mirror.instantiateBytes(cpuBytes, { maximumPhysicalPages });
  const code = new Uint8Array(pageSize * 2);
  // INC DWORD PTR [data]; JMP back, with INC straddling the page boundary.
  code.set([0xff, 0x05, ...u32(data), 0xeb, 0xf8], pageSize - 3);
  // MOV DWORD PTR [data], EAX; UD2: a bounded write and then a trap.
  code.set([0xa3, ...u32(data), 0x0f, 0x0b], 0);
  const pages = [page(codeBase, rx), page(codeBase + pageSize, rx), page(stack), page(data), ...extraPages];
  pages[0].bytes.set(code.subarray(0, pageSize));
  pages[1].bytes.set(code.subarray(pageSize));
  const regions = [region(codeBase, 2 * pageSize), region(stack, pageSize), region(data, pageSize),
    ...extraPages.map(p => region(p.address, pageSize, p.owner))];
  engine.initialize({ generation: 1n, cacheEffects: effects, pages },
    { entryPoint: entry, initialStackPointer: stack + 4000, fsBase: 0x12345000, regions });
  return engine;
}
function u32(value) { return [value & 255, (value >>> 8) & 255, (value >>> 16) & 255, value >>> 24]; }
function writeOnce(engine, value) {
  engine.applyCpuResume({ instructionPointer: codeBase, stackPointer: stack + 4000, eax: value });
  const result = engine.executeBudget(16);
  assert.equal(result.stop.kind, "trap");
  assert.equal(result.stop.trap, "invalid-opcode");
}
function assertDirtyOnce(engine, addresses) {
  const changed = engine.collectChangedPages();
  assert.deepEqual(changed.map(p => p.address), [...addresses].sort((a, b) => a - b));
  assert(changed.every(p => p.owner === 1 && p.bytes.byteLength === pageSize));
  assert.deepEqual(engine.collectChangedPages(), [], "writeback is emitted exactly once");
  return changed;
}

try {
  {
    const engine = await fixture();
    const loop = page(codeBase, rx);
    loop.bytes.set([0xff, 0x05, ...u32(data), 0xeb, 0xf8]);
    engine.applyMemoryDelta(delta([], [loop]));
    startAt(engine, codeBase);
    for (let i = 0; jitRetired(current()) === 0n && i < 100; i++) {
      run(engine, 16384);
      await Promise.resolve();
    }
    assert(jitRetired(current()) > 0n, "stopped-resume controls must start with a real warmed JIT");
    const record = current();
    const fullClears = record.fullClears;
    assertDirtyOnce(engine, [data]);
    assert.equal(record.fullClears, fullClears, "ordinary dirty collection preserves code/read translations");
    assert.equal(record.rearmedPages, 1, "ordinary dirty collection rearms only its dirty virtual page");
    const before = engine.saveCpuContext().opaqueWords;
    const expected = Uint32Array.from(before);
    expected[2] = 0x12345678;
    expected[6] = stack + 4000;
    expected[10] = expected[11] = codeBase;
    expected[22] = 0;
    const staging = new Uint32Array(record.raw.memory.buffer, record.raw.vine_cpu_state_ptr(), 153);
    staging.fill(0xa5a5a5a5, 2);
    engine.applyStoppedCpuResume({ instructionPointer: codeBase, stackPointer: stack + 4000, eax: 0x12345678 });
    assert.deepEqual(engine.saveCpuContext().opaqueWords, expected, "stopped resume preserves all other CPU words despite stale staging");
    const resumes = record.stoppedResumes;
    assert.throws(() => engine.applyStoppedCpuResume({ instructionPointer: codeBase, stackPointer: stack + 4000, eax: NaN }), /unsigned/u);
    assert.equal(record.stoppedResumes, resumes, "invalid EAX is rejected before the CPU mutation");
    assert.deepEqual(engine.saveCpuContext().opaqueWords, expected);
    const retired = jitRetired(record);
    run(engine, 4096);
    assert(jitRetired(record) > retired, "the first rearmed write must use compiled code");
    assertDirtyOnce(engine, [data]);
    engine.applyStoppedCpuResume({ instructionPointer: codeBase, stackPointer: stack + 4000, eax: null });
    const value = engine.readGuestU32(data);
    run(engine, 4096);
    assert(engine.readGuestU32(data) > value);
    record.forceDirtyOverflow = true;
    const overflowClears = record.fullClears;
    assertDirtyOnce(engine, [data]);
    record.forceDirtyOverflow = false;
    assert.equal(record.fullClears, overflowClears + 1, "reported overflow retains full translation invalidation");
    engine.applyStoppedCpuResume({ instructionPointer: codeBase, stackPointer: stack + 4000, eax: null });
    run(engine, 4096);
    assert(record.raw.vine_dirty_page_count() > 0, "growth must start with uncollected guest writes");
    const beforeGrowth = engine.saveCpuContext();
    commit(engine, 0x20000000, 40);
    assert.notEqual(current(), record, "partial resume is followed by actual physical-pool growth");
    assert.deepEqual(engine.saveCpuContext(), beforeGrowth, "growth after partial resume preserves all 153 current words");
    engine.applyStoppedCpuResume({ instructionPointer: codeBase, stackPointer: stack + 4000, eax: null });
    run(engine, 4096);
    const growthClears = current().fullClears;
    assertDirtyOnce(engine, [data]);
    assert.equal(current().fullClears, growthClears + 1, "growth restoration retains its forced full dirty scan flush");
    engine.applyMemoryDelta(delta([operation("protect", data, 1, ro)]));
    engine.applyStoppedCpuResume({ instructionPointer: codeBase, stackPointer: stack + 4000, eax: 0xdeadbeef });
    const immutable = engine.readGuestU32(data);
    const rejected = engine.executeBudget(16);
    assert.equal(rejected.stop.fault, "page-fault");
    assert.equal(engine.readGuestU32(data), immutable, "protection changes still reject cached guest writes");
    assertDirtyOnce(engine, []);
    console.log("mirror stopped resume, repeated JIT dirty rearm, overflow, growth and protection passed");
  }
  {
    const engine = await fixture();
    const parked = engine.createCpuContext({ entryPoint: entry, initialStackPointer: stack + 3996, fsBase: 0x54321000 });
    const initial = engine.saveCpuContext();
    const words = Uint32Array.from(initial.opaqueWords);
    // Populate every ABI word, masking fields whose native representation is narrower.
    for (let i = 2; i < words.length; i++) words[i] = (0x10203040 + i * 0x01010101) >>> 0;
    words[17] &= 255; words[18] = 3;
    for (const i of [19, 20, 21, 22, 86]) words[i] = 1;
    for (let i = 26; i < 34; i++) words[i] &= 0xffff;
    for (let i = 50; i < 58; i++) words[i] &= 0x1ff;
    words[61] = initial.opaqueWords[61];
    words[88] &= 255; words[89] &= 7; words[90] = 0x037f; words[91] &= 0xffff;
    for (let i = 99; i < 121; i += 3) words[i] &= 0xffff;
    engine.restoreCpuContext({ ...initial, opaqueWords: words });
    const allFields = engine.saveCpuContext();
    const originalFrames = [codeBase, codeBase + pageSize, stack, data].map(a => frame(engine, current(), a));
    let priorBuffer = engine.readGuestBytes(data, 4, true).buffer;
    const growthPools = [];
    for (const [base, pages] of [[0x20000000, 20], [0x30000000, 250], [0x40000000, 500]]) {
      const beforeCount = adapters.length;
      const oldPoolBytes = current().poolBytes;
      commit(engine, base, pages);
      assert(adapters.length > beforeCount, "batch triggers a real physical pool growth");
      growthPools.push(current().poolBytes);
      if (growthPools.length === 1) {
        assert(current().copiedBytes < oldPoolBytes && current().copyCalls > 1,
          "sparse growth copies only live frames and paging structures");
      } else {
        assert.equal(current().copiedBytes, oldPoolBytes, "dense growth copies the old pool in bulk");
        assert.equal(current().copyCalls, 1);
      }
      assertDirtyOnce(engine, []);
      assert.deepEqual(engine.saveCpuContext(), allFields, "all 153 CPU state words survive growth");
      assert.deepEqual([codeBase, codeBase + pageSize, stack, data].map(a => frame(engine, current(), a)), originalFrames);
      const borrowed = engine.readGuestBytes(data, 4, true);
      assert.notEqual(borrowed.buffer, priorBuffer, "new borrowed reads use the replacement adapter");
      priorBuffer = borrowed.buffer;
      assert.deepEqual(engine.readGuestBytes(base, pageSize), new Uint8Array(pageSize), "new frames stay zeroed");
    }
    for (let a = 0x30000000; a < 0x30000000 + 250 * pageSize; a += pageSize) {
      const physicalAddress = frame(engine, current(), a);
      assert(physicalAddress < 0xa0000 || physicalAddress >= 0xc0000, "frames skip the VGA physical hole");
    }
    engine.restoreCpuContext(parked);
    assert.deepEqual(engine.saveCpuContext(), parked, "a parked thread keeps its CR3 and full context");
    const newThread = engine.createCpuContext({ entryPoint: entry, initialStackPointer: stack + 3996, fsBase: 0x54321000 });
    assert.deepEqual(newThread, parked, "future thread contexts retain the initial template across growth");
    run(engine);
    assert(engine.readGuestU32(data) > 0);
    assertDirtyOnce(engine, [data]);
    console.log(`mirror repeated growth and full CPU/thread state passed (${growthPools.join(", ")} bytes)`);
  }

  {
    const engine = await fixture();
    const allocationId = 77;
    const disposable = 0x50000000;
    engine.applyMemoryDelta(delta([
      { ...operation("reserve", disposable), kind: "anonymous", addressAllocationId: allocationId },
      { ...operation("commit", disposable), addressAllocationId: allocationId },
    ], [page(disposable, rw, 0xa5)]));
    const freed = frame(engine, current(), disposable);
    engine.applyMemoryDelta(delta([{ ...operation("decommit", disposable), addressAllocationId: allocationId }]));
    // Growth must preserve the freed page and directory frame stack, not compact it.
    commit(engine, 0x60000000, 40);
    assert.equal(frame(engine, current(), 0x60000000), freed, "freed frames are reused in the same order");
    assert.deepEqual(engine.readGuestBytes(0x60000000, pageSize), new Uint8Array(pageSize));
    assert.throws(() => engine.readGuestU32(disposable), /unmapped/u);
    assert.throws(() => engine.applyMemoryDelta(delta([
      { ...operation("commit", disposable), addressAllocationId: allocationId + 1 },
    ])), /outside its mirror reservation/u);
    engine.applyMemoryDelta(delta([{ ...operation("commit", disposable), addressAllocationId: allocationId }]));
    assert.deepEqual(engine.readGuestBytes(disposable, pageSize), new Uint8Array(pageSize));
    console.log("mirror sparse directories, reservation identity and zeroed frame reuse passed");
  }

  {
    const engine = await fixture();
    writeOnce(engine, 0x11112222);
    const borrowedPages = engine.collectChangedPageViews();
    assert.equal(borrowedPages.length, 1);
    const oldBuffer = borrowedPages[0].bytes.buffer;
    writeOnce(engine, 0x33334444);
    commit(engine, 0x20000000, 40);
    assert.equal(current().raw.vine_dirty_page_count(), 0, "new CPU has no inherited candidate queue");
    // A second growth before writeback must retain the first growth's forced scan.
    commit(engine, 0x21000000, 80);
    assert.equal(borrowedPages.length, 0, "expired borrowed writeback list is cleared on publication");
    const changed = engine.collectChangedPageViews();
    assert.equal(changed.length, 1);
    assert.notEqual(changed[0].bytes.buffer, oldBuffer);
    assert.equal(new DataView(changed[0].bytes.buffer, changed[0].bytes.byteOffset).getUint32(0, true), 0x33334444);
    assert.deepEqual(engine.collectChangedPageViews(), []);
    writeOnce(engine, 0x55556666);
    assertDirtyOnce(engine, [data]);
    engine.applyMemoryDelta(delta([operation("protect", data, 1, ro)]));
    commit(engine, 0x30000000, 80);
    startAt(engine, data);
    assert.equal(engine.executeBudget(1).stop.fault, "execute-protection", "nonexecutability survives growth");
    startAt(engine, codeBase);
    const stop = engine.executeBudget(16).stop;
    assert.equal(stop.fault, "page-fault", JSON.stringify(stop));
    assert.equal(engine.readGuestU32(data), 0x55556666, "read-only frame rejects CPU writes after growth");
    assertDirtyOnce(engine, []);
    console.log("mirror dirty writeback, borrowed view renewal and execute/write protection passed");
  }

  {
    const engine = await fixture();
    writeOnce(engine, 0xabcddcba);
    const before = engine.saveCpuContext();
    const buffer = engine.readGuestBytes(data, 4, true).buffer;
    const growth = delta([operation("commit", 0x20000000, 40)]);
    for (const failure of ["memory-size", "allocate", "dirty-pointer", "zero", "execute", "restore"]) {
      failNextAdapter = failure;
      assert.throws(() => engine.applyMemoryDelta(growth), /rejected|invalid|allocation|injected/u, failure);
      assert.deepEqual(engine.saveCpuContext(), before, `${failure} preserves CPU state`);
      assert.equal(engine.readGuestU32(data), 0xabcddcba, `${failure} preserves guest bytes`);
      assert.equal(engine.readGuestBytes(data, 4, true).buffer, buffer, `${failure} keeps the live adapter`);
      assert.throws(() => engine.readGuestU32(0x20000000), /unmapped/u);
    }
    const instancesBefore = adapters.length;
    assert.throws(() => engine.applyMemoryDelta(delta([operation("commit", 0x20000000, 40)],
      [page(data, rw, 0, 2)])), /owner does not match/u);
    assert.throws(() => engine.applyMemoryDelta(delta([operation("commit", 0x20000000, 4096)])), /physical pool exhausted/u);
    assert.equal(adapters.length, instancesBefore, "invalid complete batches fail before growth");
    assert.deepEqual(engine.saveCpuContext(), before);
    assertDirtyOnce(engine, [data]);
    engine.applyMemoryDelta(growth);
    assert.equal(engine.readGuestU32(data), 0xabcddcba, "valid retry succeeds after preparation failures");
    console.log("mirror growth failures and invalid complete batches preserve the live adapter");
  }

  {
    const engine = await fixture();
    heldFinalizers = [];
    let slices = 0;
    while (heldFinalizers.length === 0 && slices++ < 100) run(engine, 16384);
    assert(heldFinalizers.length > 0, "real cross-page JIT queues asynchronous finalization");
    const old = current();
    const beforeGrowth = engine.saveCpuContext();
    const value = engine.readGuestU32(data);
    commit(engine, 0x20000000, 40);
    const replacement = current();
    assert.deepEqual(engine.saveCpuContext(), beforeGrowth);
    assert.equal(engine.readGuestU32(data), value);
    const pending = heldFinalizers.splice(0);
    const oldFinalizes = old.finalizes;
    for (const finalize of pending) finalize();
    assert.equal(old.finalizes, oldFinalizes + pending.length);
    assert.equal(replacement.finalizes, 0, "old async completions never publish into the replacement CPU");
    for (let i = 0; i < 100; i++) {
      run(engine, 16384);
      for (const finalize of heldFinalizers.splice(0)) finalize();
    }
    assert(replacement.compiles > 0 && replacement.finalizes > 0, `replacement warms a fresh cross-page JIT (compiled=${replacement.compiles}, finalized=${replacement.finalizes})`);
    assert(jitRetired(replacement) > 0n, "replacement executes generated code before warm growth");
    assert(engine.readGuestU32(data) > value);
    assertDirtyOnce(engine, [data]);
    // Grow again after warm code actually executes.
    const warmState = engine.saveCpuContext();
    commit(engine, 0x30000000, 80);
    assert.deepEqual(engine.saveCpuContext(), warmState);
    const warmReplacement = current();
    const retiredBefore = jitRetired(warmReplacement);
    for (let i = 0; i < 100; i++) {
      run(engine, 16384);
      for (const finalize of heldFinalizers.splice(0)) finalize();
    }
    assert(jitRetired(warmReplacement) > retiredBefore, "generated code executes again after warm growth");
    assertDirtyOnce(engine, [data]);
    heldFinalizers = null;
    console.log("mirror warm cross-page JIT and instance-local async finalization passed");
  }

  {
    const engine = await fixture(34000);
    const dirtyBase = 0x20000000;
    const dirtyPages = 16385;
    commit(engine, dirtyBase, dirtyPages);
    // Write one word per page through the real CPU, overflowing its candidate queue.
    const writer = page(codeBase, rx);
    writer.bytes.set([
      0xbf, ...u32(dirtyBase), 0xb9, ...u32(dirtyPages),
      0xc7, 0x07, ...u32(0x76543210), 0x81, 0xc7, ...u32(pageSize),
      0xe2, 0xf2, 0x0f, 0x0b,
    ]);
    engine.applyMemoryDelta(delta([], [writer]));
    startAt(engine, codeBase);
    let stop;
    do { stop = engine.executeBudget(16384).stop; } while (stop.kind === "budget-yield");
    assert.equal(stop.trap, "invalid-opcode", JSON.stringify(stop));
    const old = current();
    assert.equal(old.raw.vine_dirty_page_count(), 16384);
    assert.equal(old.raw.vine_dirty_pages_overflowed(), 1);
    const oldBuffer = old.raw.memory.buffer;
    // Stay below the configured ceiling while requiring a replacement of 128 MiB.
    commit(engine, 0x40000000, 16500);
    assert.notEqual(current().raw.memory.buffer, oldBuffer);
    const changed = assertDirtyOnce(engine, Array.from({ length: dirtyPages }, (_, i) => dirtyBase + i * pageSize));
    assert(changed.every(p => new DataView(p.bytes.buffer, p.bytes.byteOffset).getUint32(0, true) === 0x76543210));
    assert(engine.readGuestBytes(0x40000000, pageSize).every(byte => byte === 0));
    console.log("mirror real dirty-candidate overflow survives growth and writes back each page exactly once");
  }

  {
    const engine = await fixture();
    const before = engine.saveCpuContext();
    engine.applyMemoryDelta(delta([
      operation("decommit", codeBase, 2), operation("decommit", stack), operation("decommit", data),
    ]));
    const oldBuffer = current().raw.memory.buffer;
    commit(engine, 0x20000000, 40);
    assert.notEqual(current().raw.memory.buffer, oldBuffer);
    assert.deepEqual(engine.saveCpuContext(), before, "initialized CPU survives growth with no live mappings");
    assertDirtyOnce(engine, []);
    console.log("mirror initialized empty-map growth preserves CPU state");
  }

  {
    const engine = await fixture(4096, Array.from({ length: 40 }, (_, i) => page(0x20000000 + i * pageSize, rw, i)));
    assert.equal(engine.readGuestBytes(0x20001000, 1)[0], 1, "initial reflection growth remains valid");
    const before = engine.saveCpuContext();
    const replacement = page(0x30000000, rw, 0x5a);
    engine.rebuildMemory({ generation: 2n, cacheEffects: effects, pages: [replacement] }, [region(replacement.address, pageSize)]);
    assert.equal(engine.readGuestU32(replacement.address), 0x5a5a5a5a);
    assert.throws(() => engine.readGuestU32(data), /unmapped/u);
    assert.deepEqual(engine.saveCpuContext(), before, "rebuild preserves CPU context");
    assertDirtyOnce(engine, []);
    console.log("mirror initial reflection and rebuild semantics passed");
  }
} finally {
  WebAssembly.Instance = NativeInstance;
  globalThis.queueMicrotask = nativeQueueMicrotask;
  Uint8Array.prototype.set = nativeByteSet;
}
