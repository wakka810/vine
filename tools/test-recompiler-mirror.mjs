import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { build } from "esbuild";

const { outputFiles } = await build({ entryPoints: ["browser/runtime-worker/recompiler-mirror.ts"],
  bundle: true, write: false, platform: "node", format: "esm" });
const { RecompilerMirror: BaseRecompilerMirror } = await import(`data:text/javascript;base64,${Buffer.from(outputFiles[0].contents).toString("base64")}`);
const directLinks = process.argv.includes("--direct-links");
const controlGraph = process.argv.includes("--control-graph");
const predicateIr = process.argv.includes("--predicate-ir");
const flowIr = process.argv.includes("--flow-ir");
class RecompilerMirror extends BaseRecompilerMirror {
  constructor(api, options) {
    // Existing exact-state regressions select strict explicitly. The unconfigured
    // production adapter is exercised separately below.
    super(api, {recompilerNumericMode: "strict", ...options, ...(directLinks && options.recompilerLinkedRegions ? {recompilerDirectLinks: true} : {}),
      ...(controlGraph && !(options.recompilerValueIr || options.recompilerRootedSsa ||
        options.recompilerFunctionCfg || options.recompilerLoopMemory) ? {recompilerControlGraph: true} : {}),
      ...(predicateIr && !(options.recompilerValueIr || options.recompilerRootedSsa ||
        options.recompilerFunctionCfg || options.recompilerLoopMemory || options.recompilerDirectLinks ||
        options.recompilerControlGraph || directLinks || controlGraph) ? {recompilerPredicateIr: true} : {}),
      ...(flowIr && !(predicateIr || options.recompilerPredicateIr || options.recompilerValueIr ||
        options.recompilerRootedSsa || options.recompilerFunctionCfg || options.recompilerLoopMemory ||
        options.recompilerDirectLinks || options.recompilerControlGraph || directLinks || controlGraph) ? {recompilerFlowIr: true} : {})});
  }
}
const bytes = await readFile(process.env.VINE_RECOMPILER_WASM ?? "target/wasm32-unknown-unknown/release/vine_recompiler.wasm");
const { instance } = await WebAssembly.instantiate(bytes);
const wasm = instance.exports, compiled = [], retired = [], admitted = [], linkEnds = [], fpModes = [], kernelModes = [], valueModes = [], rootedModes = [], bindingModes = [], loopModes = [], functionModes = [];
const linkedRegions = process.argv.includes("--linked-regions");
const fpIr = process.argv.includes("--fp-ir");
const typedNumeric = process.argv.includes("--typed-numeric");
const expandedCache = process.argv.includes("--expanded-cache");
const valueIr = process.argv.includes("--value-ir");
const rootedSsa = process.argv.includes("--rooted-ssa");
const bindingGlobal = process.argv.includes("--binding-global");
const functionCfg = process.argv.includes("--function-cfg");
const loopMemory = process.argv.includes("--loop-memory");
const api = { ...wasm,
  ...(typeof wasm.backend_set_function_cfg === "function" ? {
    backend_set_function_cfg(handle, enabled) {
      const result = wasm.backend_set_function_cfg(handle, enabled);
      functionModes.push({handle, enabled, result}); return result;
    },
  } : {}),
  ...(typeof wasm.backend_set_loop_memory === "function" ? {
    backend_set_loop_memory(handle, enabled) {
      const result = wasm.backend_set_loop_memory(handle, enabled);
      loopModes.push({handle, enabled, result}); return result;
    },
  } : {}),
  ...(typeof wasm.backend_set_binding_global === "function" ? {
    backend_set_binding_global(handle, enabled) {
      const result = wasm.backend_set_binding_global(handle, enabled);
      bindingModes.push({handle, enabled, result}); return result;
    },
  } : {}),
  ...(typeof wasm.backend_set_rooted_ssa === "function" ? {
    backend_set_rooted_ssa(handle, enabled) {
      const result = wasm.backend_set_rooted_ssa(handle, enabled);
      rootedModes.push({handle, enabled, result}); return result;
    },
  } : {}),
  ...(typeof wasm.backend_set_value_ir === "function" ? {
    backend_set_value_ir(handle, enabled) {
      const result = wasm.backend_set_value_ir(handle, enabled);
      valueModes.push({handle, enabled, result}); return result;
    },
  } : {}),
  ...(typeof wasm.backend_set_typed_numeric === "function" ? {
    backend_set_typed_numeric(handle, enabled) {
      const result = wasm.backend_set_typed_numeric(handle, enabled);
      kernelModes.push({handle, enabled, result}); return result;
    },
  } : {}),
  ...(typeof wasm.backend_set_fp_ir === "function" ? {
    backend_set_fp_ir(handle, enabled) {
      const result = wasm.backend_set_fp_ir(handle, enabled);
      fpModes.push({handle, enabled, result}); return result;
    },
  } : {}),
  backend_compile(...args) {
    const result = wasm.backend_compile(...args);
    if (result === 0) compiled.push({ handle: args[0], entry: args[1], id: wasm.backend_cache_selected(args[0]) });
    return result;
  },
  backend_cache_retired(handle) {
    const id = wasm.backend_cache_retired(handle) >>> 0;
    if (id) retired.push({ handle, id });
    return id;
  },
  backend_cache_validate(...args) {
    const result = wasm.backend_cache_validate(...args);
    admitted.push({ handle: args[0], id: args[1], entry: args[2], result });
    return result;
  },
  ...(typeof wasm.backend_link_begin === "function" ? {
    backend_link_begin(...args) {
      const result = wasm.backend_link_begin(...args);
      admitted.push({ handle: args[0], id: args[1], entry: args[2], result, scoped: true });
      return result;
    },
    backend_link_end(...args) {
      const result = wasm.backend_link_end(...args); linkEnds.push({ handle: args[0], result }); return result;
    },
  } : {}),
};
const effects = { flushTlb: false, invalidateInstructionFetch: false, invalidateGeneratedCode: false };
const protection = { read: true, write: false, execute: true };
const page = (address, code) => {
  const data = new Uint8Array(4096).fill(0xcc); data.set(code);
  return { address, owner: 1, protection, ownerSealed: true, dirty: false, bytes: data };
};
const original = [page(0x1000, [0xff, 0xe0]), page(0x2000, [0xff, 0xe3])];
const regions = [{ owner: 1, kind: "image", addressAllocationId: 11,
  range: { start: 0x1000, byteLength: 8192 } }];
const mirror = new RecompilerMirror(api, { recompilerFunctionCfg: functionCfg, recompilerLoopMemory: loopMemory, recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages: 4, recompilerLinkedRegions: linkedRegions,
  recompilerFpIr: fpIr, recompilerTypedNumeric: typedNumeric, recompilerExpandedCache: expandedCache, recompilerValueIr: valueIr, recompilerRootedSsa: rootedSsa });

{
  const modes = [], units = [];
  const current = new BaseRecompilerMirror({...wasm,
    backend_set_binding_global(handle, enabled) {
      modes.push(enabled); return wasm.backend_set_binding_global(handle, enabled);
    },
    backend_compile(handle, entry, limit) {
      const result = wasm.backend_compile(handle, entry, limit);
      if (result === 0) units.push({handle, id: wasm.backend_cache_selected(handle),
        bytes: Uint8Array.from(new Uint8Array(wasm.memory.buffer,
          wasm.backend_code_ptr(handle), wasm.backend_code_len(handle)))});
      return result;
    },
  }, {maximumPhysicalPages: 4});
  const original = page(0x1000, [0xb8,7,0,0,0,0x0f,0x0b]);
  current.initialize({generation:1n,pages:[original],cacheEffects:effects},
    {entryPoint:0x1000,initialStackPointer:0x8000,regions:[regions[0]]});
  const initial = current.saveCpuContext();
  assert.equal(current.executeBudget(10).context.eax,7);
  for (const [generation,value] of [[2n,8],[3n,7]]) {
    current.applyMemoryDelta({fromGeneration:generation-1n,generation,
      pages:[page(0x1000,[0xb8,value,0,0,0,0x0f,0x0b])],mappingOperations:[],cacheEffects:effects});
    current.restoreCpuContext(initial);
    assert.equal(current.executeBudget(10).context.eax,value,"changed code executes its current bytes");
  }
  assert.equal(units.length,3);
  assert.notEqual(units[0].id,units[2].id,"recreated code has a fresh Rust identity");
  assert.deepEqual(units[0].bytes,units[2].bytes,"default code bytes are independent of retired unit identity");
  assert.notEqual(wasm.backend_cache_validate(units[0].handle,units[0].id,0x1000),0,
    "identical code bytes do not revive a stale instance identity");
  current.rebuildMemory({generation:3n,pages:[original],cacheEffects:effects},[regions[0]]);
  current.restoreCpuContext(initial);
  assert.equal(current.executeBudget(10).context.eax,7);
  assert.deepEqual(modes,[1,1],"default immutable binding survives backend rebuild");
  console.log("default immutable unit binding: changed code, reusable bytes, stale identity and rebuild passed");
}

{
  const current = new BaseRecompilerMirror(wasm, {maximumPhysicalPages: 4});
  const code = page(0x1000,[0xa1,0,0x20,0,0,0x0f,0x0b]);
  const data = value => ({address:0x2000,owner:2,ownerSealed:false,dirty:false,
    protection:{read:true,write:false,execute:false},bytes:Uint8Array.from({length:4096},(_,i)=>i===0?value:0)});
  const ranges=[{owner:1,kind:"image",addressAllocationId:11,range:{start:0x1000,byteLength:4096}},
    {owner:2,kind:"anonymous",addressAllocationId:22,range:{start:0x2000,byteLength:4096}}];
  current.initialize({generation:1n,pages:[code,data(42)],cacheEffects:effects},
    {entryPoint:0x1000,initialStackPointer:0x8000,regions:ranges});
  const initial=current.saveCpuContext();
  const load=()=>{current.restoreCpuContext(initial);return current.executeBudget(1);};
  assert.equal(load().context.eax,42);
  const operation=(operation,owner=2)=>({operation,owner,addressAllocationId:22,range:{base:0x2000,byteLength:4096}});
  assert.throws(()=>current.applyMemoryDelta({fromGeneration:1n,generation:2n,pages:[],cacheEffects:effects,
    mappingOperations:[operation("decommit",3)]}),/owner/);
  assert.equal(load().context.eax,42,"rejected authority update cannot revoke a live read");
  current.applyMemoryDelta({fromGeneration:1n,generation:2n,pages:[],cacheEffects:effects,mappingOperations:[operation("decommit")]});
  const fault=load();assert.equal(fault.stop.kind,"fault");assert.equal(fault.stop.fault,"page-fault");assert.equal(fault.stop.faultAddress,0x2000);
  assert.equal(fault.instructions,0);assert.equal(fault.context.eax,initial.eax,"decommit cannot read retired allocation");
  current.applyMemoryDelta({fromGeneration:2n,generation:3n,pages:[data(73)],cacheEffects:effects,
    mappingOperations:[{...operation("commit"),protection:{read:true,write:false,execute:false}}]});
  assert.equal(load().context.eax,73,"recommit at the same virtual address reads replacement bytes");
  current.rebuildMemory({generation:3n,pages:[code,data(55)],cacheEffects:effects},ranges);
  assert.equal(load().context.eax,55,"full mirror replacement reads the new owned backing");
  console.log("read translation: rejected owner, decommit/recommit and mirror rebuild passed");
}

if (typeof wasm.backend_set_numeric_mode === "function") {
  {
    const numeric = new BaseRecompilerMirror({...wasm,
      backend_fp_numeric() { throw Error("default execution must use fixed-operation imports"); },
    }, {maximumPhysicalPages: 4});
    const code = page(0x1000, [0xd9, 0xe8, 0xd9, 0xfe, 0x0f, 0x0b]); // FLD1; FSIN
    const reflection = {generation: 1n, pages: [code], cacheEffects: effects};
    numeric.initialize(reflection, {entryPoint: 0x1000, initialStackPointer: 0x8000, regions});
    const initial = numeric.saveCpuContext(), before = wasm.backend_numeric_count(1, 0);
    const result = numeric.executeBudget(2);
    assert.equal(result.stop.kind, "budget-yield");
    assert.equal(wasm.backend_numeric_count(1, 0) - before, 1, "unconfigured adapter executes balanced arithmetic");
    const values = result.context.snapshot.words.slice(56, 59);
    const significand = BigInt(values[0]) | BigInt(values[1]) << 32n;
    assert(Math.abs(Number(significand) * 2 ** ((values[2] & 0x7fff) - 16383 - 63) - Math.sin(1)) <= 5e-15);
    numeric.rebuildMemory(reflection, regions);
    numeric.restoreCpuContext(initial);
    assert.deepEqual(numeric.executeBudget(2).context.snapshot.words, result.context.snapshot.words);
    assert.equal(wasm.backend_numeric_count(1, 0) - before, 2, "default policy survives handle rebuild");
    console.log("default balanced arithmetic: fixed imports and handle rebuild passed");
  }
  for (const numericMode of ["strict", "balanced", "fast"]) {
    for (const typed of [false, true]) {
      const numeric = new RecompilerMirror(wasm, {maximumPhysicalPages: 4,
        recompilerFpIr: true, recompilerTypedNumeric: typed, recompilerNumericMode: numericMode});
      const code = page(0x1000, [0xd9, 0xe8, 0xd9, 0xfe, 0x0f, 0x0b]); // FLD1; FSIN
      const reflection = {generation: 1n, pages: [code], cacheEffects: effects};
      numeric.initialize(reflection, {entryPoint: 0x1000, initialStackPointer: 0x8000, regions});
      const initial = numeric.saveCpuContext();
      const result = numeric.executeBudget(2);
      assert.equal(result.stop.kind, "budget-yield");
      const values = result.context.snapshot.words.slice(56, 59);
      const significand = BigInt(values[0]) | BigInt(values[1]) << 32n;
      const value = Number(significand) * 2 ** ((values[2] & 0x7fff) - 16383 - 63);
      assert(Math.abs(value - Math.sin(1)) <= (numericMode === "fast" ? 1e-7 : 5e-15));
      numeric.rebuildMemory(reflection, regions);
      numeric.restoreCpuContext(initial);
      assert.deepEqual(numeric.executeBudget(2).context.snapshot.words, result.context.snapshot.words,
        "new Rust handle retains numerical policy and complete architectural restart");
    }
  }
  assert.throws(() => new RecompilerMirror(wasm, {maximumPhysicalPages:4, recompilerNumericMode:"invalid"}), /numerical mode/);
  assert.throws(() => new BaseRecompilerMirror(wasm, {maximumPhysicalPages:4, recompilerFpIr:false}), /floating-point IR/);
  assert.throws(() => new RecompilerMirror(wasm, {maximumPhysicalPages:4, recompilerNumericMode:"fast", recompilerFpIr:false}), /floating-point IR/);
}

// A complete adapter rebuild must retain the original REP flags recipe, even
// though the next access faults before completing another comparison.
for (const cmps of [false, true]) {
  const scas = new RecompilerMirror(wasm, {recompilerFunctionCfg: functionCfg, recompilerLoopMemory: loopMemory, recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages:4,recompilerLinkedRegions:linkedRegions,
    recompilerFpIr:fpIr,recompilerTypedNumeric:typedNumeric,recompilerExpandedCache:expandedCache,recompilerValueIr:valueIr,recompilerRootedSsa:rootedSsa});
  const code=page(0x1000,[cmps?0xf3:0xf2,cmps?0xa6:0xae,0x0f,0x0b]);
  const data={...page(0x3000,[]),protection:{read:true,write:false,execute:false},bytes:new Uint8Array(4096).fill(1)};
  const mappings=[{owner:1,kind:'image',addressAllocationId:111,range:{start:0x1000,byteLength:0x4000}}];
  const reflection={generation:1n,pages:[code,data],cacheEffects:effects};
  scas.initialize(reflection,{entryPoint:0x1000,initialStackPointer:0x8000,regions:mappings});
  const initial=scas.saveCpuContext();
  initial.snapshot.words[0]=0;initial.snapshot.words[1]=8192;initial.snapshot.words[6]=0x3000;initial.snapshot.words[7]=0x3000;
  initial.snapshot.words[9]=cmps?0xad7:0x246;initial.snapshot.words[10]=(~0x8d5)>>>0;
  scas.restoreCpuContext(initial);
  const first=scas.executeBudget(100);
  assert.equal(first.stop.kind,'budget-yield');assert.equal(first.context.ecx,4096);assert.equal(first.context.edi,0x4000);
  assert.equal(first.context.snapshot.words[108],1,'private recipe is live at a partial yield');
  assert.notEqual(first.context.eflags,initial.snapshot.words[9]);
  scas.rebuildMemory(reflection,mappings);
  const fault=scas.executeBudget(100);
  assert.equal(fault.stop.kind,'fault');assert.equal(fault.stop.fault,'page-fault');assert.equal(fault.stop.faultAddress,0x4000);
  assert.equal(fault.context.instructionPointer,0x1000);assert.equal(fault.context.ecx,4096);assert.equal(fault.context.edi,0x4000);
  assert.equal(fault.context.snapshot.words[6],cmps?0x4000:0x3000);
  assert.equal(fault.context.eflags,initial.snapshot.words[9]);assert.equal(fault.context.snapshot.words[10],(~0x8d5)>>>0);
  assert.equal(fault.context.snapshot.words[108],0,'delivered fault ends the previous attempt');
  console.log(`${cmps?'CMPS':'SCAS'} mirror: partial yield, full rebuild and entry-flags fault restoration passed`);
}
// The production adapter must wire the pure packed-conversion import and
// preserve the whole XMM destination across synchronous #XM and mask repair.
{
  const converted = new RecompilerMirror(wasm, {recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages:4,recompilerLinkedRegions:linkedRegions,
    recompilerFpIr:fpIr,recompilerTypedNumeric:typedNumeric,recompilerExpandedCache:expandedCache,recompilerValueIr:valueIr,recompilerRootedSsa:rootedSsa});
  converted.initialize({generation:1n,pages:[page(0x1000,[0x0f,0x5b,0xc1])],cacheEffects:effects},
    {entryPoint:0x1000,initialStackPointer:0x8000,regions});
  const initial=converted.saveCpuContext();
  initial.snapshot.words.set([0xdeadbeef,0x11223344,0x55667788,0x99aabbcc],67);
  initial.snapshot.words.set([0x01000001,0xfeffffff,0x7fffffff,0x80000000],71);
  initial.snapshot.words[99]=0x0f80;
  converted.restoreCpuContext(initial);
  const fault=converted.executeBudget(1);
  assert.equal(fault.stop.kind,'fault');assert.equal(fault.stop.fault,'simd-floating-point');
  assert.equal(fault.instructions,0);assert.equal(fault.context.instructionPointer,0x1000);
  assert.deepEqual(fault.context.snapshot.words.slice(67,71),initial.snapshot.words.slice(67,71));
  assert.equal(fault.context.snapshot.words[99],0x0fa0);
  const repaired=converted.saveCpuContext();repaired.snapshot.words[99]|=0x1000;converted.restoreCpuContext(repaired);
  const result=converted.executeBudget(1);
  assert.equal(result.stop.kind,'budget-yield');assert.equal(result.instructions,1);assert.equal(result.context.instructionPointer,0x1003);
  assert.deepEqual(Array.from(result.context.snapshot.words.slice(67,71)),[0x4b800000,0xcb800000,0x4f000000,0xcf000000]);
  assert.equal(result.context.snapshot.words[99],0x1fa0);
}
{
  const multiplied = new RecompilerMirror(wasm, {recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages:4,recompilerLinkedRegions:linkedRegions,
    recompilerFpIr:fpIr,recompilerTypedNumeric:typedNumeric,recompilerExpandedCache:expandedCache,recompilerValueIr:valueIr,recompilerRootedSsa:rootedSsa});
  multiplied.initialize({generation:1n,pages:[page(0x1000,[0x0f,0x59,0xc1])],cacheEffects:effects},
    {entryPoint:0x1000,initialStackPointer:0x8000,regions});
  const initial=multiplied.saveCpuContext();
  initial.snapshot.words.set([0x7f800123,1,0x7f7fffff,0x3f800001],67);
  initial.snapshot.words.set([0x3f800000,0x3f800000,0x40000000,0x3f800001],71);
  initial.snapshot.words[99]=0;multiplied.restoreCpuContext(initial);
  for(const [masks,sticky] of [[0,3],[0x180,0x1bb]]) {
    const context=multiplied.saveCpuContext();context.snapshot.words[99]|=masks;multiplied.restoreCpuContext(context);
    const fault=multiplied.executeBudget(1);
    assert.equal(fault.stop.kind,'fault');assert.equal(fault.stop.fault,'simd-floating-point');
    assert.equal(fault.instructions,0);assert.equal(fault.context.instructionPointer,0x1000);
    assert.deepEqual(fault.context.snapshot.words.slice(67,71),initial.snapshot.words.slice(67,71));
    assert.equal(fault.context.snapshot.words[99],sticky);
  }
  const repaired=multiplied.saveCpuContext();repaired.snapshot.words[99]|=0x1f80;multiplied.restoreCpuContext(repaired);
  const result=multiplied.executeBudget(1);
  assert.equal(result.stop.kind,'budget-yield');assert.equal(result.instructions,1);assert.equal(result.context.instructionPointer,0x1003);
  assert.deepEqual(Array.from(result.context.snapshot.words.slice(67,71)),[0x7fc00123,1,0x7f800000,0x3f800002]);
  assert.equal(result.context.snapshot.words[99],0x1fbb);
}
for (const [opcode, subtract] of [[0x58, false], [0x5c, true]]) {
  const arithmetic = new RecompilerMirror(wasm, {recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages:4,recompilerLinkedRegions:linkedRegions,
    recompilerFpIr:fpIr,recompilerTypedNumeric:typedNumeric,recompilerExpandedCache:expandedCache,recompilerValueIr:valueIr,recompilerRootedSsa:rootedSsa});
  arithmetic.initialize({generation:1n,pages:[page(0x1000,[0x0f,opcode,0xc1])],cacheEffects:effects},
    {entryPoint:0x1000,initialStackPointer:0x8000,regions});
  const initial=arithmetic.saveCpuContext();
  initial.snapshot.words.set([0x7f800123,1,0x7f7fffff,0x3f800000],67);
  initial.snapshot.words.set([0x3f800000,0,subtract ? 0xff7fffff : 0x7f7fffff,subtract ? 0xb3800000 : 0x33800000],71);
  initial.snapshot.words[99]=0;arithmetic.restoreCpuContext(initial);
  for(const [masks,sticky] of [[0,3],[0x180,0x1bb]]) {
    const context=arithmetic.saveCpuContext();context.snapshot.words[99]|=masks;arithmetic.restoreCpuContext(context);
    const fault=arithmetic.executeBudget(1);
    assert.equal(fault.stop.kind,'fault');assert.equal(fault.stop.fault,'simd-floating-point');
    assert.equal(fault.instructions,0);assert.equal(fault.context.instructionPointer,0x1000);
    assert.deepEqual(fault.context.snapshot.words.slice(67,71),initial.snapshot.words.slice(67,71));
    assert.equal(fault.context.snapshot.words[99],sticky);
  }
  const repaired=arithmetic.saveCpuContext();repaired.snapshot.words[99]|=0x1f80;arithmetic.restoreCpuContext(repaired);
  const result=arithmetic.executeBudget(1);
  assert.equal(result.stop.kind,'budget-yield');assert.equal(result.instructions,1);assert.equal(result.context.instructionPointer,0x1003);
  assert.deepEqual(Array.from(result.context.snapshot.words.slice(67,71)),[0x7fc00123,1,0x7f800000,0x3f800000]);
  assert.equal(result.context.snapshot.words[99],0x1fbb);
}
for (const alias of [false, true]) {
  const truncated = new RecompilerMirror(wasm, {recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages:4,recompilerLinkedRegions:linkedRegions,
    recompilerFpIr:fpIr,recompilerTypedNumeric:typedNumeric,recompilerExpandedCache:expandedCache,recompilerValueIr:valueIr,recompilerRootedSsa:rootedSsa});
  const code=page(0x1000,[0xf3,0x0f,0x5b,alias ? 0xc0 : 0xc1]);
  truncated.initialize({generation:1n,pages:[code],cacheEffects:effects},
    {entryPoint:0x1000,initialStackPointer:0x8000,regions:[regions[0]]});
  const initial=truncated.saveCpuContext();
  initial.snapshot.words.set([0xdeadbeef,0x11223344,0x55667788,0x99aabbcc],67);
  initial.snapshot.words.set([0x7fc01234,0x3fc00000,1,0xcf000000],alias ? 67 : 71);
  initial.snapshot.words[99]=0;truncated.restoreCpuContext(initial);
  for(const [masks,sticky] of [[0,1],[0x80,0xa1]]) {
    const context=truncated.saveCpuContext();context.snapshot.words[99]|=masks;truncated.restoreCpuContext(context);
    const fault=truncated.executeBudget(1);
    assert.equal(fault.stop.kind,'fault');assert.equal(fault.stop.fault,'simd-floating-point');
    assert.equal(fault.instructions,0);assert.equal(fault.context.instructionPointer,0x1000);
    assert.deepEqual(fault.context.snapshot.words.slice(67,71),initial.snapshot.words.slice(67,71));
    assert.equal(fault.context.snapshot.words[99],sticky);
    const saved=truncated.saveCpuContext();
    truncated.rebuildMemory({generation:1n,pages:[code],cacheEffects:effects},[regions[0]]);
    assert.deepEqual(truncated.saveCpuContext(),saved,'numeric fault resumes across a fresh execution handle');
  }
  const repaired=truncated.saveCpuContext();repaired.snapshot.words[99]|=0x1f80;truncated.restoreCpuContext(repaired);
  const result=truncated.executeBudget(1);
  assert.equal(result.stop.kind,'budget-yield');assert.equal(result.instructions,1);assert.equal(result.context.instructionPointer,0x1004);
  assert.deepEqual(Array.from(result.context.snapshot.words.slice(67,71)),[0x80000000,1,0,0x80000000]);
  assert.equal(result.context.snapshot.words[99],0x1fa1);
}
{
  const packed = new RecompilerMirror(wasm, {recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages:4,recompilerLinkedRegions:linkedRegions,
    recompilerFpIr:fpIr,recompilerTypedNumeric:typedNumeric,recompilerExpandedCache:expandedCache,recompilerValueIr:valueIr,recompilerRootedSsa:rootedSsa});
  const code=page(0x1000,[0x66,0x0f,0x6b,0xc1,0x66,0x0f,0x71,0xe0,3,0x66,0x0f,0xe1,0xc2,
    0x66,0x0f,0xfd,0xc1,0x66,0x0f,0x69,0xc2]);
  packed.initialize({generation:1n,pages:[code],cacheEffects:effects},
    {entryPoint:0x1000,initialStackPointer:0x8000,regions:[regions[0]]});
  const initial=packed.saveCpuContext();
  initial.snapshot.words.set([0x80000000,0xffff7fff,0xffff8000,0x7fff],67);
  initial.snapshot.words.set([0x8000,0xffffffff,0,0x7fffffff],71);
  initial.snapshot.words.set([0,1,0xffffffff,0x7f800001],75);
  initial.snapshot.words[9]=0xad7;initial.snapshot.words[59]=0x37e;initial.snapshot.words[60]=0x80c1;initial.snapshot.words[99]=0xffff;
  packed.restoreCpuContext(initial);
  for(const [pc,words] of [[0x1004,[0x80008000,0x7fff8000,0xffff7fff,0x7fff0000]],
    [0x1009,[0xf000f000,0x0ffff000,0xffff0fff,0x0fff0000]],[0x100d,[0xffffffff,0x0000ffff,0xffff0000,0]],
    [0x1011,[0xffff7fff,0xfffffffe,0xffff0000,0x7fffffff]],[0x1015,[0xffff0000,0xffffffff,0x0001ffff,0x7f807fff]]]) {
    const result=packed.executeBudget(1);
    assert.equal(result.stop.kind,'budget-yield');assert.equal(result.instructions,1);
    for(let index=0;index<100;index++) assert.equal(result.context.snapshot.words[index],
      index===8 ? pc : index>=67&&index<71 ? words[index-67] : initial.snapshot.words[index],
      'packed integer instruction preserves every unrelated architectural word');
    const saved=packed.saveCpuContext();
    packed.rebuildMemory({generation:1n,pages:[code],cacheEffects:effects},[regions[0]]);
    assert.deepEqual(packed.saveCpuContext(),saved,'packed conversion/shift/add/unpack state survives fresh backend rebuild');
  }
}
mirror.initialize({ generation: 1n, pages: original, cacheEffects: effects },
  { entryPoint: 0x1000, initialStackPointer: 0x8000, regions });
const setup = mirror.saveCpuContext();
setup.snapshot.words[0] = 0x2000; setup.snapshot.words[3] = 0x1000;
mirror.restoreCpuContext(setup);
assert.equal(mirror.executeBudget(10).stop.kind, "budget-yield");
assert.equal(compiled.length, 2, "two unknown indirect destinations create two reusable units");
const [first, second] = compiled;
assert.notEqual(first.id, second.id);
const before = mirror.saveCpuContext();
const warmAdmissions = admitted.length;
assert.equal(mirror.executeBudget(10).instructions, 10);
assert.equal(compiled.length, 2, "warm alternating regions must not recompile");
assert.equal(admitted.length - warmAdmissions, linkedRegions ? 1 : 10,
  "linked warm control flow must not return through host admission at each region");
if (linkedRegions) {
  const admissions = admitted.length;
  const chained = mirror.executeBudget(200000);
  assert.equal(chained.instructions, 200000);
  assert.equal(chained.context.instructionPointer, before.instructionPointer);
  assert.equal(chained.context.eax, before.eax);
  assert.equal(chained.context.ebx, before.ebx);
  assert.equal(admitted.length - admissions, 1, "long cyclic chain has one outer admission");
  assert.equal(compiled.length, 2, "long cyclic chain reuses bound units");
}
const other = mirror.createCpuContext({ entryPoint: 0x2000, initialStackPointer: 0x9000, fsBase: 0 });
other.snapshot.words[0] = 0x2000; other.snapshot.words[3] = 0x1000;
mirror.restoreCpuContext(other);
assert.equal(mirror.executeBudget(10).instructions, 10);
mirror.restoreCpuContext(before);
assert.equal(compiled.length, 2, "thread context changes must preserve compiled region identity");

const modified = page(0x1000, [0x47, 0xff, 0xe0]); // INC EDI; JMP EAX.
mirror.applyMemoryDelta({ fromGeneration: 1n, generation: 2n, pages: [modified],
  mappingOperations: [], cacheEffects: effects });
const changed = mirror.executeBudget(12);
assert.equal(changed.context.edi, before.edi + 4);
assert.equal(compiled.length, 3, "changed code retires only its dependent region");
assert(retired.some(item => item.handle === first.handle && item.id === first.id));
assert(!retired.some(item => item.handle === second.handle && item.id === second.id));
assert.notEqual(wasm.backend_cache_validate(first.handle, first.id, first.entry), 0,
  "a stale browser instance identity cannot be admitted");
assert.equal(mirror.executeBudget(12).context.edi, before.edi + 8);
assert.equal(compiled.length, 3);

assert.throws(() => mirror.rebuildMemory({ generation: 2n, pages: [{ ...modified, owner: 99 }, original[1]],
  cacheEffects: effects }, regions), /owner/);
const afterFailure = mirror.saveCpuContext();
assert.equal(mirror.executeBudget(12).context.edi, afterFailure.edi + 4);
assert.equal(compiled.length, 3, "failed rebuild retains old handle and its Wasm instances");
const snapshot = mirror.saveCpuContext();
mirror.rebuildMemory({ generation: 2n, pages: [modified, original[1]], cacheEffects: effects }, regions);
assert.equal(mirror.executeBudget(12).context.edi, snapshot.edi + 4);
assert.equal(compiled.length, 5, "successful rebuild creates instances bound to the new backend handle");
assert.notEqual(compiled[3].handle, first.handle);
assert.equal(wasm.backend_cache_validate(first.handle, first.id, first.entry), 1);
assert(admitted.every(item => item.result === 0));
let timestampCases = 0;
if (typeof wasm.backend_begin_timestamp_counter === "function") {
  const timestamp = new RecompilerMirror(api, { recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages: 4, recompilerLinkedRegions: linkedRegions,
    recompilerFpIr: fpIr, recompilerTypedNumeric: typedNumeric, recompilerExpandedCache: expandedCache, recompilerValueIr: valueIr, recompilerRootedSsa: rootedSsa });
  const clockPage = page(0x1000, [0x0f,0x31,0x0f,0x31,0x0f,0x0b]);
  timestamp.initialize({ generation: 1n, pages: [clockPage], cacheEffects: effects },
    { entryPoint: 0x1000, initialStackPointer: 0x8000, regions: [regions[0]] });
  const firstRead = timestamp.executeBudget(8);
  assert.equal(firstRead.stop.kind, "timestamp-counter");
  assert.equal(firstRead.instructions, 0);
  const pending = timestamp.saveCpuContext();
  assert.throws(() => timestamp.completeTimestampCounter(0x1001, 1, 2), /context changed/);
  assert.deepEqual(timestamp.saveCpuContext(), pending, "rejected completion preserves the pending CPU");
  timestamp.completeTimestampCounter(0x1000, 0xffff_fffe, 7);
  const complete = timestamp.saveCpuContext();
  assert.equal(complete.eax, 0xffff_fffe); assert.equal(complete.edx, 7);
  assert.equal(complete.instructionPointer, 0x1002); assert.equal(complete.eflags, pending.eflags);
  assert.throws(() => timestamp.completeTimestampCounter(0x1000, 0, 0), /no pending/);
  const secondRead = timestamp.executeBudget(8);
  assert.equal(secondRead.stop.kind, "timestamp-counter");
  const secondPending = timestamp.saveCpuContext();
  timestamp.restoreCpuContext(secondPending);
  assert.throws(() => timestamp.completeTimestampCounter(0x1002, 9, 10), /no pending/,
    "CPU context replacement cancels the old one-use identity");
  assert.deepEqual(timestamp.saveCpuContext(), secondPending);
  assert.equal(timestamp.executeBudget(8).stop.kind, "timestamp-counter",
    "retry begins at the still-unretired RDTSC without replaying earlier work");
  timestamp.completeTimestampCounter(0x1002, 0, 8);
  timestamp.rebuildMemory({ generation: 1n, pages: [clockPage], cacheEffects: effects }, [regions[0]]);
  const resumed = timestamp.saveCpuContext();
  assert.equal(resumed.eax, 0); assert.equal(resumed.edx, 8); assert.equal(resumed.instructionPointer, 0x1004);
  assert.equal(timestamp.executeBudget(8).stop.kind, "trap");
  timestampCases = 3;
}
let fpCases = 0;
if (fpIr) {
  const fp = new RecompilerMirror(api, { recompilerFunctionCfg: functionCfg, recompilerLoopMemory: loopMemory, recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages: 4, recompilerLinkedRegions: linkedRegions,
    recompilerFpIr: true, recompilerTypedNumeric: typedNumeric, recompilerExpandedCache: expandedCache, recompilerValueIr: valueIr, recompilerRootedSsa: rootedSsa });
  const code = page(0x1000, [0xd9,0xe8,0xd9,0xe8,0xde,0xc1,0x0f,0x0b]);
  fp.initialize({ generation: 1n, pages: [code], cacheEffects: effects },
    { entryPoint: 0x1000, initialStackPointer: 0x8000, regions: [regions[0]] });
  const initial = fp.saveCpuContext();
  assert.equal(fp.executeBudget(3).stop.kind, "budget-yield");
  const first = fp.saveCpuContext(), words = first.snapshot.words;
  assert.equal(words[60] & 0x3800, 0x3800);
  assert.equal(words[61], 0x3fff);
  assert.deepEqual(Array.from(words.slice(35 + 7 * 3, 35 + 8 * 3)), [0, 0x80000000, 0x4000]);
  const beforeModes = fpModes.length;
  const beforeKernelModes = kernelModes.length;
  const beforeValueModes = valueModes.length;
  const beforeRootedModes = rootedModes.length;
  const beforeBindingModes = bindingModes.length;
  const beforeFunctionModes = functionModes.length;
  const beforeLoopModes = loopModes.length;
  fp.rebuildMemory({ generation: 1n, pages: [code], cacheEffects: effects }, [regions[0]]);
  assert.equal(fpModes.length, beforeModes + 1, "new backend restores the explicit FP lowering choice");
  if (typedNumeric) assert.equal(kernelModes.length, beforeKernelModes + 1,
    "new backend restores the typed numeric ABI choice");
  if (valueIr) assert.equal(valueModes.length, beforeValueModes + 1,
    "new backend restores the scalar value IR choice");
  if (rootedSsa) assert.equal(rootedModes.length, beforeRootedModes + 1,
    "new backend restores the rooted SSA choice");
  if (bindingGlobal) assert.equal(bindingModes.length, beforeBindingModes + 1,
    "new backend restores immutable instance binding");
  if (functionCfg) assert.equal(functionModes.length, beforeFunctionModes + 1,
    "new backend restores function CFG mode");
  if (loopMemory) assert.equal(loopModes.length, beforeLoopModes + 1,
    "new backend restores loop memory proof mode");
  fp.restoreCpuContext(initial);
  assert.equal(fp.executeBudget(3).stop.kind, "budget-yield");
  assert.deepEqual(fp.saveCpuContext(), first, "raw80 execution survives new-handle rebuild with the same semantics");
  assert(fpModes.every(mode => mode.enabled === 1 && mode.result === 0));
  if (typedNumeric) assert(kernelModes.every(mode => mode.enabled === 1 && mode.result === 0));
  if (valueIr) assert(valueModes.every(mode => mode.enabled === 1 && mode.result === 0));
  if (rootedSsa) assert(rootedModes.every(mode => mode.enabled === 1 && mode.result === 0));
  if (bindingGlobal) assert(bindingModes.every(mode => mode.enabled === 1 && mode.result === 0));
  if (functionCfg) assert(functionModes.every(mode => mode.enabled === 1 && mode.result === 0));
  if (loopMemory) assert(loopModes.every(mode => mode.enabled === 1 && mode.result === 0));
  fpCases = 2;
}
if (typeof wasm.backend_link_begin === "function" && linkedRegions) {
  assert.equal(admitted.filter(item => item.scoped).length, linkEnds.length,
    "every successful linked invocation closes its proof scope");
  const failing = new RecompilerMirror({ ...api,
    backend_link_resolve() { throw Error("intentional imported execution failure"); },
  }, { recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages: 4, recompilerLinkedRegions: true, recompilerRootedSsa: rootedSsa });
  failing.initialize({ generation: 1n, pages: original, cacheEffects: effects },
    { entryPoint: 0x1000, initialStackPointer: 0x8000, regions });
  const context = failing.saveCpuContext();
  context.snapshot.words[0] = 0x2000; failing.restoreCpuContext(context);
  const beforeTrapEnds = linkEnds.length;
  assert.throws(() => failing.executeBudget(10), /intentional imported execution failure/);
  assert.equal(linkEnds.length, beforeTrapEnds + 1, "an imported trap also closes the proof scope");
  assert.equal(linkEnds.at(-1).result, 0);
}
let retentionCases = 0;
if (typeof wasm.backend_set_cache_capacity === "function") {
  // A ring exceeds the old capacity and exercises actual high table slots.
  // It is a cache/identity correctness fixture, not a performance benchmark.
  const count = 96, firstAddress = 0x10000;
  const address = index => firstAddress + index * 4096;
  const code = (index, increments = 1) => {
    const next = address((index + 1) % count), bytes = Array(increments).fill(0x47);
    bytes.push(0xb8, next & 255, next >>> 8 & 255, next >>> 16 & 255, next >>> 24, 0xff, 0xe0);
    return bytes;
  };
  const ringPages = Array.from({length: count}, (_, index) => page(address(index), code(index)));
  const ringRegions = [{...regions[0], range: {start: firstAddress, byteLength: count * 4096}}];
  for (const expanded of [false, true]) {
    const ringCompiled = [], ringRetired = [], ringBindings = [];
    const ringApi = {...wasm,
      backend_compile(...args) { const result = wasm.backend_compile(...args);
        if (result === 0) ringCompiled.push({handle: args[0], id: wasm.backend_cache_selected(args[0]), entry: args[1]});
        return result; },
      backend_cache_retired(handle) { const id = wasm.backend_cache_retired(handle) >>> 0;
        if (id) ringRetired.push(id); return id; },
      backend_link_bind(handle, id, slot) { ringBindings.push(slot); return wasm.backend_link_bind(handle, id, slot); },
    };
    const ring = new RecompilerMirror(ringApi, {recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages: count, recompilerLinkedRegions: true,
      recompilerExpandedCache: expanded, recompilerRootedSsa: rootedSsa});
    ring.initialize({generation: 1n, pages: ringPages, cacheEffects: effects},
      {entryPoint: firstAddress, initialStackPointer: 0x8000, regions: ringRegions});
    assert.equal(ring.executeBudget(count * 3).instructions, count * 3);
    assert.equal(ring.saveCpuContext().edi, count);
    assert.equal(ring.saveCpuContext().instructionPointer, firstAddress);
    assert.equal(ringCompiled.length, count);
    const initial = ringCompiled[0];
    assert.equal(wasm.backend_cache_capacity(initial.handle), expanded ? 512 : 64);
    assert.equal(wasm.backend_cache_byte_limit(initial.handle), 16 * 1024 * 1024);
    assert.equal(ring.executeBudget(count * 3).instructions, count * 3);
    assert.equal(ring.saveCpuContext().edi, count * 2);
    assert.equal(ringCompiled.length, expanded ? count : count * 2);
    if (expanded) {
      assert(ringBindings.some(slot => slot > 64), "retained regions bind through slots beyond64");
      const changedPage = page(firstAddress, code(0, 2));
      ring.applyMemoryDelta({fromGeneration: 1n, generation: 2n, pages: [changedPage], mappingOperations: [], cacheEffects: effects});
      assert.equal(ring.executeBudget(count * 3 + 1).instructions, count * 3 + 1);
      assert.equal(ring.saveCpuContext().edi, count * 3 + 1);
      assert.equal(ringCompiled.length, count + 1, "code update retires only the affected retained unit");
      assert.equal(wasm.backend_cache_validate(initial.handle, initial.id, initial.entry), 1);
      assert(ringRetired.includes(initial.id));
      assert.equal(ringRetired.length, 1, "unrelated code retains its original bound identities");
      ring.applyCacheEffects({flushTlb: true, invalidateInstructionFetch: true, invalidateGeneratedCode: true});
      assert.equal(wasm.backend_cache_unit_count(initial.handle), 0);
      const beforeClear = ringCompiled.length;
      assert.equal(ring.executeBudget(count * 3 + 1).instructions, count * 3 + 1);
      assert.equal(ringCompiled.length, beforeClear + count, "full invalidation drains more than64 retirements");
      const beforeRebuild = ring.saveCpuContext();
      ring.rebuildMemory({generation: 2n, pages: [changedPage, ...ringPages.slice(1)], cacheEffects: effects}, ringRegions);
      assert.equal(ring.executeBudget(count * 3 + 1).instructions, count * 3 + 1);
      assert.equal(ring.saveCpuContext().edi, beforeRebuild.edi + count + 1);
      assert.notEqual(ringCompiled.at(-1).handle, initial.handle);
      assert.equal(wasm.backend_cache_capacity(ringCompiled.at(-1).handle), 512,
        "successful rebuild retains the requested Rust cache capacity");
    }
    retentionCases++;
  }
  assert.throws(() => new RecompilerMirror({...api, backend_set_cache_capacity: undefined},
    {recompilerBindingGlobal: bindingGlobal, maximumPhysicalPages: 4, recompilerExpandedCache: true}), /does not support expanded/);
}
assert.throws(() => new RecompilerMirror({...api, backend_set_binding_global: undefined},
  {maximumPhysicalPages: 4, recompilerBindingGlobal: true}), /does not support immutable unit/);
assert.throws(() => new RecompilerMirror(api,
  {maximumPhysicalPages: 4, recompilerBindingGlobal: "yes"}), /invalid immutable binding/);
assert.throws(() => new RecompilerMirror({...api, backend_set_loop_memory: undefined},
  {maximumPhysicalPages:4,recompilerLoopMemory:true}), /does not support loop memory proofs/);
assert.throws(() => new RecompilerMirror(api,
  {maximumPhysicalPages:4,recompilerLoopMemory:1}), /invalid loop memory option/);
assert.throws(() => new RecompilerMirror(api,
  {maximumPhysicalPages:4,recompilerLoopMemory:true,recompilerRootedSsa:true}), /require ordinary CFG lowering/);
assert.throws(() => new RecompilerMirror({...api, backend_set_function_cfg: undefined},
  {maximumPhysicalPages:4,recompilerFunctionCfg:true}), /does not support function CFG/);
assert.throws(() => new RecompilerMirror(api,
  {maximumPhysicalPages:4,recompilerFunctionCfg:1}), /invalid function CFG option/);
for (const incompatible of [{recompilerLoopMemory:true},{recompilerRootedSsa:true},{recompilerValueIr:true}]) {
  assert.throws(() => new RecompilerMirror(api,
    {maximumPhysicalPages:4,recompilerFunctionCfg:true,...incompatible}), /requires ordinary instruction lowering/);
}
{
  let demandCompilations = 0, bindings = 0, linkAdmissions = 0, failPreparation = false;
  const demandEntries = [];
  const preparedApi = {...wasm,
    backend_compile(...args) { demandCompilations++; demandEntries.push(args[1]); return wasm.backend_compile(...args); },
    backend_link_bind(...args) { bindings++; return wasm.backend_link_bind(...args); },
    backend_link_begin(...args) { linkAdmissions++; return wasm.backend_link_begin(...args); },
    backend_prepare_next(...args) {
      if (failPreparation) { failPreparation = false; throw Error("injected preparation failure"); }
      return wasm.backend_prepare_next(...args);
    },
  };
  const prepared = new RecompilerMirror(preparedApi, {maximumPhysicalPages:4,
    recompilerLinkedRegions:linkedRegions, recompilerFpIr:true, recompilerTypedNumeric:true});
  const demand = new RecompilerMirror(wasm, {maximumPhysicalPages:4,
    recompilerLinkedRegions:linkedRegions, recompilerFpIr:true, recompilerTypedNumeric:true});
  const straight = page(0x1000, [...Array(1200).fill(0x40), 0x0f, 0x0b]);
  const reflection = {generation:1n,pages:[straight],cacheEffects:effects};
  const bootstrap = {entryPoint:0x1000,initialStackPointer:0x8000,regions};
  prepared.initialize(reflection, bootstrap); demand.initialize(reflection, bootstrap);
  const before = prepared.saveCpuContext();
  await assert.rejects(prepared.prepareExecution([NaN]), /preparation root/);
  await assert.rejects(prepared.prepareExecution(Array(1)), /preparation root/);
  failPreparation = true;
  await assert.rejects(prepared.prepareExecution([0x1000]), /injected preparation failure/);
  const preparation = await prepared.prepareExecution([0x1000]);
  assert.equal(preparation.limited, false);
  assert(preparation.milliseconds >= 0);
  assert.deepEqual(prepared.saveCpuContext(), before, "AOT leaves all architectural state unchanged");
  assert.deepEqual(prepared.readGuestBytes(0x1000,4096), straight.bytes);
  assert.deepEqual(prepared.collectChangedPages(), []);
  const installed = bindings;
  assert(installed >= 3, "preparation follows region frontiers and instantiates before execution");
  const result = prepared.executeBudget(10000);
  const demanded = demand.executeBudget(10000);
  // AOT can link through already installed successors in one invocation;
  // demand execution may return to Chrome between those units. Word 104 is
  // the last invocation's retirement count, not an architectural register.
  for (const slice of [result,demanded]) {
    assert(slice.context.snapshot.words[104] > 0 && slice.context.snapshot.words[104] <= slice.instructions);
  }
  const comparable = slice => ({...slice,context:{...slice.context,snapshot:{...slice.context.snapshot,
    words:Array.from(slice.context.snapshot.words,(value,index) => index === 104 ? 0 : value)}}});
  assert.deepEqual(comparable(result), comparable(demanded), "AOT and JIT preserve the same precise exit and total retirement");
  assert.equal(result.context.eax,1200);
  assert.equal(demandCompilations,0, "known straight-line work needs no in-play conversion");
  assert.equal(bindings,installed, "known work needs no in-play Wasm instantiation");
  prepared.restoreCpuContext(before);
  await prepared.prepareExecution([0x1000]);
  assert.equal(bindings,installed, "repeated preparation reuses generated resources");
  const changed = page(0x1000,[0xb8,42,0,0,0,0x0f,0x0b]);
  prepared.applyMemoryDelta({fromGeneration:1n,generation:2n,pages:[changed],mappingOperations:[],cacheEffects:effects});
  assert.equal(prepared.executeBudget(10).context.eax,42, "changed prepared bytes require current code");
  assert.equal(demandCompilations,1);

  for (const prepareHead of [true,false]) for (const linked of [undefined,false,true]) {
    const shared = new RecompilerMirror(preparedApi, {maximumPhysicalPages:4,
      recompilerLinkedRegions:linked, recompilerFlowIr:flowIr});
    const head = page(0x1000,[0x40,0xe9,0xfa,0x0f,0,0]);
    const tail = page(0x2000,[0x83,0xc0,2,0x0f,0x0b]);
    shared.initialize({generation:1n,pages:[head,tail],cacheEffects:effects},bootstrap);
    const initial = shared.saveCpuContext();
    await shared.prepareExecution([0x2000]);
    if (prepareHead) await shared.prepareExecution([0x1000]);
    const compiledBefore = demandCompilations;
    const admissionsBefore = linkAdmissions;
    const first = shared.executeBudget(100);
    assert.equal(first.context.eax,3);assert.equal(first.instructions,3);
    assert.equal(first.stop.kind,"trap");
    assert.equal(demandCompilations,compiledBefore+(prepareHead?0:1),"only an unprepared head needs demand code");
    assert.equal(linkAdmissions-admissionsBefore,linked===false?0:1,
      "default native chaining keeps both prepared units in one checked invocation");
    shared.restoreCpuContext(initial);
    shared.applyMemoryDelta({fromGeneration:1n,generation:2n,
      pages:[page(0x2000,[0x83,0xc0,7,0x0f,0x0b])],mappingOperations:[],cacheEffects:effects});
    const resumed = shared.executeBudget(100);
    assert.equal(resumed.context.eax,8,"caller cannot retain stale prepared tail bytes");
    assert.equal(resumed.instructions,3);assert.equal(resumed.stop.kind,"trap");
    assert.equal(demandCompilations,compiledBefore+(prepareHead?1:2),"only the changed tail is recompiled");
    assert.equal(demandEntries.at(-1),0x2000,"changed prepared bytes do not invalidate an independently owned caller");
  }

  for (const linked of [false,true]) {
    const shared = new RecompilerMirror(preparedApi, {maximumPhysicalPages:4,
      recompilerLinkedRegions:linked, recompilerFlowIr:flowIr});
    const head = page(0x1000,[0x40,0xe9,0xfa,0x0f,0,0]);
    const tail = page(0x2000,[0x83,0xc0,2,0x89,0x07,0x0f,0x0b]);
    const data = {...page(0x3000,[]),ownerSealed:false,
      protection:{read:true,write:false,execute:false},bytes:new Uint8Array(4096)};
    shared.initialize({generation:1n,pages:[head,tail,data],cacheEffects:effects},
      {...bootstrap,regions:[{...regions[0],range:{start:0x1000,byteLength:0x3000}}]});
    const initial = shared.saveCpuContext(); initial.snapshot.words[7] = 0x3000;
    shared.restoreCpuContext(initial);
    await shared.prepareExecution([0x2000]);
    const fault = shared.executeBudget(100);
    assert.equal(fault.stop.kind,"fault");assert.equal(fault.stop.fault,"page-fault");
    assert.equal(fault.stop.faultAddress,0x3000);assert.equal(fault.context.instructionPointer,0x2003);
    assert.equal(fault.instructions,3);assert.equal(fault.context.eax,3);
    assert.deepEqual(shared.collectChangedPages(),[],"a fault in the shared tail publishes no store");
    const writable = {read:true,write:true,execute:false};
    shared.applyMemoryDelta({fromGeneration:1n,generation:2n,
      pages:[{...data,protection:writable}],cacheEffects:effects,
      mappingOperations:[{operation:"protect",owner:1,addressAllocationId:11,
        range:{base:0x3000,byteLength:4096},protection:writable}]});
    const resumed = shared.executeBudget(100);
    assert.equal(resumed.stop.kind,"trap");assert.equal(resumed.instructions,1);
    assert.equal(resumed.context.eax,3,"resumption never replays the demand head or prepared prefix");
    assert.equal(new DataView(shared.readGuestBytes(0x3000,4).buffer).getUint32(0,true),3);
  }

  // Publish these roots in separate preparation sessions so they retain
  // independent owners. Demand code can borrow unused prepared slots while
  // the prepared identities and native bindings stay resident.
  const cells = page(0x1000, []);
  for (let index = 0; index < 160; index++) {
    cells.bytes.set([0xb8,index,0,0,0,0x0f,0x0b], index * 16);
  }
  const resident = new RecompilerMirror(preparedApi, {maximumPhysicalPages:4,
    recompilerLinkedRegions:linkedRegions, recompilerFlowIr:flowIr});
  resident.initialize({generation:1n,pages:[cells],cacheEffects:effects},bootstrap);
  const initialResident = resident.saveCpuContext();
  const initialDemand = demandCompilations;
  for (let index = 0; index < 65; index++) {
    const residency = await resident.prepareExecution([0x1000 + index * 16]);
    assert.equal(residency.limited,false);
  }
  assert.deepEqual(resident.saveCpuContext(),initialResident);
  assert.deepEqual(resident.collectChangedPages(),[]);
  const runCell = index => {
    const context = structuredClone(initialResident);
    context.snapshot.words[8] = 0x1000 + index * 16;
    resident.restoreCpuContext(context);
    const result = resident.executeBudget(10);
    assert.equal(result.stop.kind,"trap");
    assert.equal(result.context.eax,index);
  };
  for(let index=65;index<145;index++) runCell(index);
  assert.equal(demandCompilations,initialDemand+80);
  const residentBindings = bindings;
  for(let index=0;index<65;index++) runCell(index);
  assert.equal(demandCompilations,initialDemand+80,"prepared identities survive demand admission");
  assert.equal(bindings,residentBindings,"prepared native instances survive demand admission");
  for(let index=65;index<145;index++) runCell(index);
  assert.equal(demandCompilations,initialDemand+80,"unused AOT slots retain demand code beyond its reserved count");
  assert.deepEqual(resident.readGuestBytes(0x1000,4096),cells.bytes);
  assert.deepEqual(resident.collectChangedPages(),[]);

  const indirect = new RecompilerMirror(preparedApi, {maximumPhysicalPages:4});
  indirect.initialize({generation:1n,pages:[page(0x1000,[0xff,0xe0]),page(0x2000,[0x0f,0x0b])],cacheEffects:effects},bootstrap);
  const context = indirect.saveCpuContext(); context.snapshot.words[0] = 0x2000;
  indirect.restoreCpuContext(context);
  await indirect.prepareExecution([0x1000]);
  const beforeIndirect = demandCompilations;
  assert.equal(indirect.executeBudget(10).stop.kind,"trap");
  assert.equal(demandCompilations,beforeIndirect+1,"unknown indirect target retains JIT supplementation");
  const transfers = new RecompilerMirror(wasm, {maximumPhysicalPages:4,recompilerLinkedRegions:false});
  transfers.initialize({generation:1n,pages:original,cacheEffects:effects},bootstrap);
  const loop = transfers.saveCpuContext(); loop.snapshot.words[0]=0x2000; loop.snapshot.words[3]=0x1000;
  transfers.restoreCpuContext(loop);
  await transfers.prepareExecution([0x1000]);
  await transfers.prepareExecution([0x2000]);
  const yielded = transfers.executeBudget(5000);
  assert.equal(yielded.stop.kind,"budget-yield");
  assert(yielded.instructions>0 && yielded.instructions<5000,"bounded host transfers yield completed work");
  const resumed = transfers.executeBudget(5000-yielded.instructions);
  assert.equal(resumed.instructions+yielded.instructions,5000,"resumption retires each indirect jump exactly once");
  assert.equal(resumed.context.instructionPointer,0x1000);
  if (!flowIr && !predicateIr) {
    const grouped = new RecompilerMirror(wasm, {maximumPhysicalPages:4,recompilerLinkedRegions:false});
    grouped.initialize({generation:1n,pages:original,cacheEffects:effects},bootstrap);
    grouped.restoreCpuContext(loop);
    await grouped.prepareExecution([0x1000,0x2000]);
    const uninterrupted = grouped.executeBudget(5000);
    assert.equal(uninterrupted.instructions,5000,"a shared batch keeps known indirect transfers inside generated code");
    assert.equal(uninterrupted.stop.kind,"budget-yield");
    assert.equal(uninterrupted.context.instructionPointer,0x1000);
    grouped.restoreCpuContext(loop);
    grouped.applyMemoryDelta({fromGeneration:1n,generation:2n,
      pages:[page(0x2000,[0x40,0x0f,0x0b])],mappingOperations:[],cacheEffects:effects});
    const replaced = grouped.executeBudget(100);
    assert.equal(replaced.instructions,2,"invalidating any shared body also prevents stale entry through its caller");
    assert.equal(replaced.context.eax,0x2001);
    assert.equal(replaced.stop.kind,"trap");
  }

  const returned = new RecompilerMirror(wasm, {maximumPhysicalPages:4,recompilerLinkedRegions:true});
  const returnCode = new Uint8Array(64).fill(0xcc);
  returnCode.set([0xe8,0x2b,0,0,0,0x0f,0x0b],0);
  returnCode.set([0xe8,0x23,0,0,0,0x0f,0x0b],8);
  returnCode.set([0x40,0x0f,0x0b],16); returnCode[48]=0xc3;
  const returnStack = {...page(0x2000,[]),ownerSealed:false,
    protection:{read:true,write:true,execute:false},bytes:new Uint8Array(4096)};
  returned.initialize({generation:1n,pages:[page(0x1000,returnCode),returnStack],cacheEffects:effects},
    {...bootstrap,initialStackPointer:0x2800});
  await returned.prepareExecution([0x1000,0x1008,0x1010]);
  const returnContext = returned.saveCpuContext();
  let returnGeneration = 1n;
  for (const actual of [0x1005,0x100d,0x1010,0x5000]) {
    const context = structuredClone(returnContext);context.snapshot.words[8]=0x1030;
    context.snapshot.words[0]=7;returned.restoreCpuContext(context);
    const stackBytes = new Uint8Array(4096);new DataView(stackBytes.buffer).setUint32(0x800,actual,true);
    returned.applyMemoryDelta({fromGeneration:returnGeneration,generation:returnGeneration+1n,
      pages:[{...returnStack,bytes:stackBytes}],mappingOperations:[],cacheEffects:effects});
    returnGeneration++;
    const boundary = returned.executeBudget(1);
    assert.equal(boundary.instructions,1,"RET retires exactly once before either a guarded or unexpected target");
    assert.equal(boundary.context.instructionPointer,actual,"the checked guest stack value decides the destination");
    assert.equal(boundary.context.snapshot.words[4],0x2804);
    assert.equal(boundary.context.eax,7,"no continuation is replayed before the budget boundary");
    const continuation = returned.executeBudget(2);
    assert.equal(continuation.instructions,actual===0x1010?1:0);
    assert.equal(continuation.context.eax,actual===0x1010?8:7);
    assert.equal(continuation.stop.kind,actual===0x5000?"fault":"trap");
  }
  assert.equal(resumed.context.eax,0x2000); assert.equal(resumed.context.ebx,0x1000);
  assert.deepEqual(transfers.collectChangedPages(),[]);
  console.log("prelaunch AOT: unchanged CPU/bytes, frontier coverage, resident execution, changed code and indirect JIT passed");
}
// Runtime cache effects summarize this delta, not an instruction to discard
// unrelated prepared code. Exercise both checked and direct native links.
for (const [linked, direct] of [[false, false], [true, false], [true, true]]) {
  const units = [], removed = [];
  let handle, demandCount = 0;
  const tracked = {...wasm,
    backend_prepare_next(h) {
      handle = h;
      const result = wasm.backend_prepare_next(h);
      if (result === 0 && wasm.backend_prepare_status(h) === 1) units.push(wasm.backend_cache_selected(h));
      return result;
    },
    backend_compile(...args) { demandCount++; return wasm.backend_compile(...args); },
    backend_cache_retired(h) { const id = wasm.backend_cache_retired(h) >>> 0; if (id) removed.push(id); return id; },
  };
  const current = new BaseRecompilerMirror(tracked, {maximumPhysicalPages: 8,
    recompilerNumericMode: "strict", recompilerLinkedRegions: linked, recompilerDirectLinks: direct});
  const code = value => [0xb8, value, 0, 0, 0, 0x0f, 0x0b];
  current.initialize({generation: 1n, pages: [page(0x1000, code(7)), page(0x2000, code(8))], cacheEffects: effects},
    {entryPoint: 0x1000, initialStackPointer: 0x8000, regions});
  const context = current.saveCpuContext();
  const run = entry => {
    const next = structuredClone(context); next.snapshot.words[8] = entry;
    current.restoreCpuContext(next); return current.executeBudget(10);
  };
  await current.prepareExecution([0x1000]);
  await current.prepareExecution([0x2000]);
  assert.equal(units.length, 2);
  const allEffects = {flushTlb: true, invalidateInstructionFetch: true, invalidateGeneratedCode: true};
  const backing = {owner: 2, addressAllocationId: 12, range: {base: 0x10000, byteLength: 8192}};
  const writable = {read: true, write: true, execute: false};
  current.applyMemoryDelta({fromGeneration: 1n, generation: 2n, cacheEffects: allEffects,
    mappingOperations: [{...backing, operation: "reserve", kind: "runtime"},
      {...backing, operation: "commit", protection: writable},
      {...backing, operation: "protect", range: {base: 0x11000, byteLength: 4096}, protection}],
    pages: [{...page(0x10000, []), owner: 2, protection: writable}, {...page(0x11000, [0x0f, 0x0b]), owner: 2}]});
  assert.equal(wasm.backend_cache_unit_count(handle), 2, "unrelated executable backing creation preserves AOT");
  assert.deepEqual(removed, []);
  assert.equal(run(0x1000).context.eax, 7); assert.equal(run(0x2000).context.eax, 8);
  assert.equal(demandCount, 0, "prepared native instances remain executable without recompilation");
  assert.equal(run(0x11000).stop.kind, "trap");
  const endpointId = wasm.backend_cache_selected(handle);
  current.applyMemoryDelta({fromGeneration: 2n, generation: 3n, pages: [], cacheEffects: allEffects,
    mappingOperations: [{...backing, operation: "release"}]});
  assert.deepEqual(removed, [endpointId], "release retires only code depending on the released backing");
  assert.equal(wasm.backend_cache_unit_count(handle), 2);
  current.applyMemoryDelta({fromGeneration: 3n, generation: 4n, mappingOperations: [], cacheEffects: allEffects,
    pages: [page(0x1000, code(9))]});
  assert.deepEqual(removed, [endpointId, units[0]], "changed AOT bytes revoke exactly their old identity");
  assert.notEqual(wasm.backend_cache_validate(handle, units[0], 0x1000), 0);
  assert.equal(run(0x1000).context.eax, 9); assert.equal(run(0x2000).context.eax, 8);
  assert.equal(demandCount, 2);
  assert.throws(() => current.applyMemoryDelta({fromGeneration: 4n, generation: 5n,
    mappingOperations: [], cacheEffects: allEffects,
    pages: [page(0x1000, code(10)), {...page(0x2000, code(10)), owner: 99}]}), /owner/);
  assert.equal(run(0x1000).context.eax, 9, "rejected delta publishes neither changed bytes nor retirement");
  assert.equal(demandCount, 2);
  current.applyMemoryDelta({fromGeneration: 4n, generation: 5n, pages: [], cacheEffects: allEffects,
    mappingOperations: [{operation: "protect", owner: 1, addressAllocationId: 11,
      range: {base: 0x2000, byteLength: 4096}, protection: {...protection, execute: false}}]});
  assert(removed.includes(units[1]));
  assert.equal(run(0x2000).stop.kind, "fault");
  const faultId = wasm.backend_cache_selected(handle);
  current.applyMemoryDelta({fromGeneration: 5n, generation: 6n, pages: [], cacheEffects: allEffects,
    mappingOperations: [{operation: "protect", owner: 1, addressAllocationId: 11,
      range: {base: 0x2000, byteLength: 4096}, protection}]});
  assert(removed.includes(faultId), "execute permission changes invalidate cached fetch faults");
  assert.equal(run(0x2000).context.eax, 8);
  assert.equal(run(0x1000).context.eax, 9);
}
console.log(JSON.stringify({ passed: true, linkedRegions, fpIr, typedNumeric, expandedCache, valueIr, predicateIr, flowIr, rootedSsa, bindingGlobal, loopMemory, functionCfg, retentionCases,
  cases: linkedRegions ? 7 : 6, compiledUnits: compiled.length,
  timestampCases, fpCases,
  checks: ["warm cache", "thread context", "code update", "stale identity", "failed rebuild", "new handle rebuild"] }));
