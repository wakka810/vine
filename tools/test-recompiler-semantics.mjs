// The same suite runs in Node and Chrome. All reference arithmetic is expressed
// with architectural integer rules; no v86 state layout or oracle is imported.
export async function runRecompilerSemantics(compilerBytes, options = {}) {
  const { instance } = await WebAssembly.instantiate(compilerBytes, {});
  const api = instance.exports;
  const hasLinks = ["backend_link_slot", "backend_link_bind", "backend_link_resolve"]
    .every((name) => typeof api[name] === "function");
  if (options.linkedRegions && !hasLinks) throw Error("Compiler does not support linked regions");
  const hasLinkInvocation = ["backend_link_begin", "backend_link_end"]
    .every((name) => typeof api[name] === "function");
  const hasTimestamp = ["backend_begin_timestamp_counter", "backend_complete_timestamp_counter", "backend_cancel_timestamp_counter"]
    .every((name) => typeof api[name] === "function");
  if (options.timestampCounter && !hasTimestamp) throw Error("Compiler does not support timestamp handoff");
  const hasFpIr = typeof api.backend_set_fp_ir === "function" && typeof api.backend_fp_numeric === "function";
  const hasValueIr = typeof api.backend_set_value_ir === "function";
  const hasBindingGlobal = typeof api.backend_set_binding_global === "function";
  if (options.bindingGlobal && !hasBindingGlobal) throw Error("Compiler does not support immutable unit binding");
  const hasRootedSsa = typeof api.backend_set_rooted_ssa === "function";
  if (options.rootedSsa && !hasRootedSsa) throw Error("Compiler does not support rooted SSA");
  if (options.valueIr && !hasValueIr) throw Error("Compiler does not support scalar value IR");
  const hasSseCvtt = ["backend_sse_cvtt_f32", "backend_sse_cvtt_f64"].every(name => typeof api[name] === "function");
  if (options.fpIr && !hasFpIr) throw Error("Compiler does not support explicit FP IR");
  const typedKernelCount = typeof api.backend_fp_kernel_25 === "function" ? 26
    : typeof api.backend_fp_kernel_23 === "function" ? 24
    : typeof api.backend_fp_kernel_22 === "function" ? 23
    : typeof api.backend_fp_kernel_21 === "function" ? 22
    : typeof api.backend_fp_kernel_20 === "function" ? 21 : 20;
  const hasTypedNumeric = ["backend_set_typed_numeric", "backend_fp_kernel_error", ...Array.from({ length: typedKernelCount }, (_, i) => `backend_fp_kernel_${i}`)]
    .every(name => typeof api[name] === "function");
  if (options.typedNumeric && (!options.fpIr || !hasTypedNumeric)) throw Error("Typed numeric kernels require supported explicit FP IR");
  const hasCacheCapacity = ["backend_set_cache_capacity", "backend_cache_capacity", "backend_cache_byte_limit"]
    .every(name => typeof api[name] === "function");
  const hasSnapshots = ["backend_snapshot_size", "backend_save_snapshot", "backend_restore_snapshot", "backend_create_snapshot"]
    .every(name => typeof api[name] === "function");
  if (options.expandedCache && !hasCacheCapacity) throw Error("Compiler does not support expanded cache configuration");
  const requestedCacheCapacity = options.expandedCache ? 512 : 64;
  const requestedCacheByteLimit = 16 * 1024 * 1024;
  const CF = 1, PF = 4, AF = 16, ZF = 64, SF = 128, OF = 2048, ARITH = 0x8d5;
  const CODE = 0x1000, DATA = 0x2000, STACK = 0x8000;
  const READ = 1, WRITE = 2, EXECUTE = 4, SEALED = 16;
  const failures = [];
  let checks = 0, cases = 0;
  const equal = (actual, expected, label) => {
    checks += 1;
    if (actual !== expected) throw Error(`${label}: got ${actual}, expected ${expected}`);
  };
  const check = (value, label) => equal(Boolean(value), true, label);
  const error = () => new TextDecoder().decode(new Uint8Array(api.memory.buffer,
    api.backend_error_ptr(), api.backend_error_len()));
  const ok = (status, label) => equal(status, 0, `${label}: ${status ? error() : ""}`);
  const newBackend = (maximumPages) => {
    const handle = api.backend_new(maximumPages);
    check(handle !== 0, "backend allocation");
    if (options.directLinks) ok(api.backend_set_direct_links(handle, 1), "select coherent direct links");
    if (options.controlGraph) ok(api.backend_set_control_graph(handle, 1), "select binary-derived control graph");
    if (hasBindingGlobal) ok(api.backend_set_binding_global(handle, Number(options.bindingGlobal ?? false)), "select immutable unit binding");
    if (hasValueIr) ok(api.backend_set_value_ir(handle, Number(options.valueIr ?? false)), "select scalar value IR");
    if (options.predicateIr) {
      if (typeof api.backend_set_predicate_ir !== "function") throw Error("Compiler does not support predicate IR");
      ok(api.backend_set_predicate_ir(handle, 1), "select predicate IR");
    }
    if (options.flowIr) {
      if (typeof api.backend_set_flow_ir !== "function") throw Error("Compiler does not support flow IR");
      ok(api.backend_set_flow_ir(handle, 1), "select flow IR");
    }
    if (hasRootedSsa) ok(api.backend_set_rooted_ssa(handle, Number(options.rootedSsa ?? false)), "select rooted SSA");
    if (options.functionCfg) {
      check(typeof api.backend_set_function_cfg === "function", "function CFG support");
      ok(api.backend_set_function_cfg(handle, 1), "select function CFG");
    }
    if (options.loopMemory) {
      check(typeof api.backend_set_loop_memory === "function", "loop memory proof support");
      ok(api.backend_set_loop_memory(handle, 1), "select loop memory proofs");
    }
    if (hasCacheCapacity) {
      ok(api.backend_set_cache_capacity(handle, Number(options.expandedCache ?? false)), "select initial cache capacity");
      equal(api.backend_cache_capacity(handle) >>> 0, requestedCacheCapacity, "new handle retains requested cache capacity");
      equal(api.backend_cache_byte_limit(handle) >>> 0, requestedCacheByteLimit, "new handle retains requested cache byte limit");
    }
    return handle;
  };
  const wordBytes = (value) => [value & 255, value >>> 8 & 255, value >>> 16 & 255, value >>> 24];
  const setWord = (bytes, offset, value) => new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).setUint32(offset, value, true);
  const page = (bytes = [], fill = 0) => {
    const result = new Uint8Array(4096);
    result.fill(fill);
    result.set(bytes);
    return result;
  };
  const testFilter = options.filter ? new RegExp(options.filter, "u") : null;
  const test = async (label, body) => {
    if (testFilter && !testFilter.test(label)) return;
    cases += 1;
    try { await body(); } catch (failure) { failures.push({ label, error: String(failure.stack ?? failure) }); }
  };
  async function fixture(code, body, codeFlags = READ | EXECUTE | SEALED, compilerOptions = {}) {
    const handle = newBackend(16);
    try {
      if (compilerOptions.functionCfg !== undefined) {
        ok(api.backend_set_function_cfg(handle, Number(compilerOptions.functionCfg)), "fixture function CFG mode");
      }
      if (compilerOptions.loopMemory !== undefined) {
        ok(api.backend_set_loop_memory(handle, Number(compilerOptions.loopMemory)), "fixture loop proof mode");
      }
      ok(api.backend_set_structured_loops(handle, options.structuredLoops ? 1 : 0), "select loop lowering");
      ok(api.backend_set_affine_memory(handle, options.affineMemory ? 1 : 0), "select affine memory lowering");
      if (hasFpIr) ok(api.backend_set_fp_ir(handle, options.fpIr ? 1 : 0), "select explicit FP IR lowering");
      if (hasTypedNumeric) ok(api.backend_set_typed_numeric(handle, options.typedNumeric ? 1 : 0), "select typed numeric kernels");
      if (compilerOptions.numericMode !== undefined) {
        ok(api.backend_set_numeric_mode(handle, compilerOptions.numericMode), "select fixture numerical policy");
      }
      const statePtr = api.backend_state(handle);
      const initialState = new Uint8Array(api.memory.buffer, statePtr, 420).slice();
      let generated;
      let generatedId = 0;
      const generatedUnits = new Map();
      const cacheCapacity = hasCacheCapacity ? api.backend_cache_capacity(handle) >>> 0 : 64;
      const links = new WebAssembly.Table({ element: "anyfunc", initial: cacheCapacity, maximum: cacheCapacity });
      const drainRetired = () => {
        if (!api.backend_cache_retired) return;
        for (let count = 0; count <= cacheCapacity; count++) {
          const id = api.backend_cache_retired(handle) >>> 0;
          if (id === 0) return;
          if (id === 0xffffffff) {
            for (let slot = 0; slot < links.length; slot++) links.set(slot, null);
            generatedUnits.clear(); generated = undefined; generatedId = 0;
          } else {
            const retired = generatedUnits.get(id);
            if (retired?.slot !== null && retired?.slot !== undefined) links.set(retired.slot, null);
            generatedUnits.delete(id);
            if (id === generatedId) { generated = undefined; generatedId = 0; }
          }
        }
        throw Error("Region retirement notification bound exceeded");
      };
      let affineRangeCount = 0, functionPartitionCount = 0, controlGraphFacts = null;
      const state = () => new Uint32Array(api.memory.buffer, statePtr, 105);
      const map = (address, flags, bytes = page()) => {
        const transfer = api.backend_alloc(4096);
        check(transfer !== 0, "transfer allocation");
        new Uint8Array(api.memory.buffer, transfer, 4096).set(bytes);
        const status = api.backend_map_page(handle, address, 1, flags, transfer);
        const message = status ? error() : "";
        ok(api.backend_free(transfer, 4096), "free transfer");
        equal(status, 0, `map ${address.toString(16)}: ${message}`);
      };
      const bytes = (address) => {
        const ptr = api.backend_page_ptr(handle, address);
        check(ptr !== 0, "mapped page pointer");
        return new Uint8Array(api.memory.buffer, ptr, 4096);
      };
      const compile = async (entry = CODE, limit = 256, roots = null) => {
        if (roots) {
          const pointer = api.backend_input_alloc(handle, roots.length * 4);
          check(pointer !== 0, "allocate owned AOT roots");
          try {
            new Uint32Array(api.memory.buffer, pointer, roots.length).set(roots);
            ok(api.backend_compile_entries(handle, pointer, roots.length, limit), "prepare shared AOT graph");
          } finally { ok(api.backend_free(pointer, roots.length * 4), "free AOT roots"); }
        } else ok(api.backend_compile(handle, entry, limit), "compile");
        drainRetired();
        const codeBytes = new Uint8Array(api.memory.buffer, api.backend_code_ptr(handle),
          api.backend_code_len(handle)).slice();
        check(WebAssembly.validate(codeBytes), "generated Wasm validates");
        const compiled = await WebAssembly.instantiate(codeBytes, { env: { memory: api.memory,
          x87: api.backend_x87, backend: handle, unit: api.backend_cache_selected(handle), links, link_resolve: api.backend_link_resolve,
          ...(options.directLinks ? {link_directory: api.backend_link_directory(handle)} : {}),
          begin_timestamp_counter: api.backend_begin_timestamp_counter, fp_numeric: api.backend_fp_numeric,
          fp_kernel_error: api.backend_fp_kernel_error,
          sse_cvtt_f32: api.backend_sse_cvtt_f32, sse_cvtt_f64: api.backend_sse_cvtt_f64,
          sse_i32_to_f32: api.backend_sse_i32_to_f32,
          sse_mul_f32: api.backend_sse_mul_f32,
          sse_add_f32: api.backend_sse_add_f32,
          sse_sub_f32: api.backend_sse_sub_f32,
          sse_cvt_f64: api.backend_sse_cvt_f64,
          sse_add_f64: api.backend_sse_add_f64,
          sse_sub_f64: api.backend_sse_sub_f64,
          sse_mul_f64: api.backend_sse_mul_f64,
          scas_begin: api.backend_scas_begin, scas_complete: api.backend_scas_complete, scas_finish: api.backend_scas_finish,
          ...Object.fromEntries(Array.from({ length: typedKernelCount }, (_, i) => [`fp_kernel_${i}`, api[`backend_fp_kernel_${i}`]])),
          ...Object.fromEntries(["balanced", "fast"].flatMap(mode => Array.from({length: 26}, (_, i) => [`fp_${mode}_${i}`, api[`backend_fp_${mode}_${i}`]]))) } });
        if (options.fpIr) check(!WebAssembly.Module.imports(compiled.module).some(({ module, name }) => module === "env" && name === "x87"),
          "explicit FP IR never imports descriptor/state-clone execution");
        if (options.typedNumeric) check(!WebAssembly.Module.imports(compiled.module).some(({ module, name }) => module === "env" && name === "fp_numeric"),
          "typed numeric kernels never import handle/opcode dispatch");
        equal(WebAssembly.Module.customSections(compiled.module, "vine.value-ir.v1").length,
          Number(options.valueIr ?? false), "compiled scalar value IR mode matches requested mode");
        equal(WebAssembly.Module.customSections(compiled.module, "vine.predicate-ir.v1").length,
          Number(options.predicateIr ?? false), "compiled predicate IR mode matches requested mode");
        equal(WebAssembly.Module.customSections(compiled.module, "vine.flow-ir.v3").length,
          Number(options.flowIr ?? false), "compiled flow IR mode matches requested mode");
        equal(WebAssembly.Module.customSections(compiled.module, "vine.rooted-ssa.v1").length,
          Number(options.rootedSsa ?? false), "compiled rooted SSA mode matches requested mode");
        const partitions = WebAssembly.Module.customSections(compiled.module, "vine.function-cfg.v1");
        functionPartitionCount = partitions.length ? JSON.parse(new TextDecoder().decode(partitions[0])).instructions.length : 0;
        const graphs = WebAssembly.Module.customSections(compiled.module, "vine.control-graph.v1");
        controlGraphFacts = graphs.length ? JSON.parse(new TextDecoder().decode(graphs[0])) : null;
        equal(partitions.length, Number(compilerOptions.functionCfg ?? options.functionCfg ?? false), "function CFG mode");
        generated = compiled.instance;
        const diagnostic = WebAssembly.Module.customSections(compiled.module, "vine.affine.v1");
        affineRangeCount = diagnostic.length ? new Uint8Array(diagnostic[0])[0] : 0;
        const id = api.backend_cache_selected?.(handle);
        let slot = null;
        if (id && hasLinks) {
          const slotPlusOne = api.backend_link_slot(handle, id) >>> 0;
          check(slotPlusOne >= 1 && slotPlusOne <= links.length, "region link slot range");
          slot = slotPlusOne - 1;
          check(links.get(slot) === null, "retirement clears old table target before reuse");
          links.set(slot, generated.exports.link);
          const status = api.backend_link_bind(handle, id, slotPlusOne);
          if (status !== 0) links.set(slot, null);
          ok(status, "publish instantiated region link");
        }
        generatedId = id ?? 0;
        if (id) generatedUnits.set(id, { instance: generated, affineRangeCount, slot });
        return id;
      };
      const select = (entry) => {
        ok(api.backend_cache_select(handle, entry), "select cached entry");
        drainRetired();
        const id = api.backend_cache_selected(handle);
        const unit = generatedUnits.get(id);
        check(unit !== undefined, "cached unit has an existing Wasm instance");
        generated = unit.instance; generatedId = id; affineRangeCount = unit.affineRangeCount;
        return id;
      };
      const reset = ({ eax = 0, ecx = 0, edx = 0, ebx = 0, esp = STACK + 0x800,
        ebp = 0, esi = 0, edi = 0, eip = CODE, flags = 0x202, fs = 0 } = {}) => {
        if (hasLinkInvocation) ok(api.backend_link_end(handle), "end links before context restore");
        if (api.backend_cancel_timestamp_counter) ok(api.backend_cancel_timestamp_counter(handle), "cancel timestamp before restore");
        if (hasSnapshots) ok(api.backend_initialize_state(handle, eip, esp, fs), "replace complete backend context before fixture reset");
        new Uint8Array(api.memory.buffer, statePtr, 420).set(initialState);
        state().set([eax, ecx, edx, ebx, esp, ebp, esi, edi, eip, flags, 0xffffffff]);
        state()[28] = fs; // FS base: segments offset 44 + index 4 * 16 + 4.
      };
      const snapshotBuffer = (body) => {
        check(hasSnapshots, "backend-private snapshot API exists");
        const length = api.backend_snapshot_size() >>> 0;
        equal(length, 512, "version2 context includes resumable instruction state");
        const ptr = api.backend_input_alloc(handle, length) >>> 0;
        check(ptr !== 0, "owned snapshot buffer allocation");
        try { return body(ptr, length); }
        finally { ok(api.backend_free(ptr, length), "free owned snapshot buffer"); }
      };
      const saveSnapshot = () => snapshotBuffer((ptr, length) => {
        ok(api.backend_save_snapshot(handle, ptr, length), "save complete backend context");
        return new Uint8Array(api.memory.buffer, ptr, length).slice();
      });
      const restoreSnapshot = (saved) => snapshotBuffer((ptr, length) => {
        equal(saved.length, length, "restore complete backend context length");
        new Uint8Array(api.memory.buffer, ptr, length).set(saved);
        ok(api.backend_restore_snapshot(handle, ptr, length), "restore complete backend context");
      });
      const run = (budget = 100, beforeFinish) => {
        if (options.linkedRegions) select(state()[8]);
        if (options.linkedRegions && hasLinkInvocation) ok(api.backend_link_begin(handle, generatedId, state()[8]), "begin linked invocation");
        else if (api.backend_cache_validate) ok(api.backend_cache_validate(handle, generatedId, state()[8]), "admit compiled entry");
        else ok(api.backend_validate(handle, state()[8]), "admit compiled entry");
        const execute = options.linkedRegions ? generated.exports.run_linked : generated.exports.run;
        check(typeof execute === "function", "selected execution export exists");
        let retired;
        try {
          retired = execute(statePtr, api.backend_table(handle), budget);
        } finally {
          if (options.linkedRegions && hasLinkInvocation) ok(api.backend_link_end(handle), "end linked invocation");
        }
        beforeFinish?.();
        ok(api.backend_finish(handle), "collect completed writes");
        equal(state()[104], retired, "exit retirement count");
        return { words: Array.from(state()), retired, kind: state()[100],
          pc: state()[8], faultPc: state()[101], address: state()[102], error: state()[103] };
      };
      map(CODE, codeFlags, page([...code, 0xcc], 0xcc));
      map(DATA, READ | WRITE);
      map(STACK, READ | WRITE);
      reset();
      await compile();
      await body({ handle, state, map, bytes, compile, select, reset, run,
        saveSnapshot, restoreSnapshot, snapshotBuffer,
        links, generatedUnits, drainRetired,
        functionPartitions: () => functionPartitionCount,
        controlGraph: () => controlGraphFacts,
        affineRanges: () => affineRangeCount,
        protect: (address, flags) => ok(api.backend_protect_page(handle, address, 1, flags), "protect"),
        alias: (address, source, flags) => ok(api.backend_map_alias(handle, address, source, 1, flags), "alias") });
    } finally {
      ok(api.backend_drop(handle), "drop backend");
    }
  }
  await test("read translation observes permission changes in a resident unit",()=>fixture(
    [0x8b,0x10,0x0f,0x0b],async({handle,bytes,protect,reset,run})=>{
      const attempt=()=>{reset({eax:DATA,edx:99});return run(1);};
      setWord(bytes(DATA),0,42);equal(attempt().words[2],42,"initial readable mapping");
      protect(DATA,WRITE);let fault=attempt();equal(fault.kind,4,"write permission never grants reads");equal(fault.error,5,"mapped protection fault");equal(fault.words[2],99,"failed read leaves destination unchanged");
      protect(DATA,READ);equal(attempt().words[2],42,"permission restoration uses resident code");
      protect(DATA,0);fault=attempt();equal(fault.kind,4,"removing all rights revokes prior translation");equal(fault.error,5,"mapping stays present");equal(fault.address,DATA,"original effective address retained");
      protect(DATA,READ);setWord(bytes(DATA),0,73);equal(attempt().words[2],73,"restored permission reads current bytes");
      api.memory.grow(1);equal(attempt().words[2],73,"translation survives Wasm growth");
      check(api.backend_protect_page(handle,DATA,2,WRITE)!==0,"wrong owner rejected");
      equal(attempt().words[2],73,"rejected protection change grants no new mapping state");
    }));
  await test("read translation aliases retain independent rights and exact crossing faults",()=>fixture(
    [0x8b,0x10,0x0f,0x0b],async({bytes,protect,alias,reset,run})=>{
      alias(0x4000,DATA,READ);alias(0x5000,DATA,WRITE);bytes(DATA).set([0x33,0x44,0x55,0x66]);bytes(DATA).set([0x11,0x22],4094);
      const attempt=address=>{reset({eax:address,edx:99});return run(1);};
      equal(attempt(0x4000).words[2],0x66554433,"read alias shares bytes");
      let fault=attempt(0x4ffe);equal(fault.kind,4,"second alias retains its own denied rights");equal(fault.address,0x5000,"crossing fault reports first byte on second page");equal(fault.error,5,"crossing mapped protection fault");equal(fault.words[2],99,"no partial split result");
      protect(0x5000,READ);equal(attempt(0x4ffe).words[2],0x44332211,"both aliases can supply a split value");
      protect(DATA,0);equal(attempt(0x4ffe).words[2],0x44332211,"aliases retain independent rights after original protection changes");
      protect(0x5000,0);fault=attempt(0x4ffe);equal(fault.error,5,"second alias cannot reuse revoked read permission");equal(fault.address,0x5000,"denied second-page address");
      protect(0x5000,READ);bytes(DATA).set([0x77,0x88]);equal(attempt(0x4ffe).words[2],0x88772211,"restored alias uses current shared bytes");
    }));
  await test("read translation does not grant RMW or store rights",()=>fixture(
    [0x8b,0x10,0x83,0x00,1,0x0f,0x0b],async({bytes,protect,reset,run})=>{
      setWord(bytes(DATA),0,42);protect(DATA,READ);reset({eax:DATA,edx:99,flags:0xad7});const fault=run(100);
      equal(fault.kind,4,"RMW still requires write rights");equal(fault.error,7,"mapped write fault");equal(fault.pc,CODE+2,"fault excludes RMW");equal(fault.retired,1,"only prior read retires");equal(fault.words[2],42,"prior read retained");equal(fault.words[9],0xad7,"RMW fault preserves flags");
      equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(0,true),42,"failed RMW cannot write through readable mapping");
    }));
  for (const offset of [0, 4094]) await test(`scalar memory reload aliased bytes and covered subspans offset${offset}`, () => fixture(
    [0x8b,0x03, 0x89,0xd9, 0x43, 0x8b,0x53,0xff, 0x89,0x3e, 0x8b,0x01, 0x0f,0xb6,0x51,3],
    async ({ map, alias, bytes, reset, run }) => {
      map(DATA + 4096, READ | WRITE); alias(0x4000, DATA, READ | WRITE); alias(0x5000, DATA + 4096, READ | WRITE);
      bytes(DATA).fill(0x77); bytes(DATA + 4096).fill(0x77);
      reset({ ebx: DATA + offset, esi: 0x4000 + offset, edi: 0xa1b2c3d4 });
      const result = run(7);
      equal(result.kind, 1, "whole scalar segment completes"); equal(result.retired, 7, "all ordered instructions retire");
      equal(result.words[0], 0xa1b2c3d4, "read observes intervening store through alias");
      equal(result.words[2], 0xa1, "contained byte observes updated high byte");
      equal(result.words[3], DATA + offset + 1, "pointer increment remains architectural");
    }));
  for (const [base, offset, kind] of [[DATA,4092,1], [DATA,4093,4], [0xfffff000,4092,1], [0xfffff000,4093,6]]) {
    await test(`scalar memory known page keeps endpoint and wrap checks ${base}:${offset}`, () => fixture(
      [0xa1,...wordBytes(base), 0x8b,0x15,...wordBytes((base + offset) >>> 0)], async ({ map, bytes, reset, run }) => {
        if (base !== DATA) map(base, READ | WRITE);
        bytes(base).fill(0x35); reset({ edx: 91 }); const result = run(2);
        equal(result.kind, kind, "known page does not cover an adjacent page or wrapped span");
        equal(result.retired, kind === 1 ? 2 : 1, "exact covered prefix");
        equal(result.words[2], kind === 1 ? 0x35353535 : 91, "no failed-access publication");
      }));
  }
  await test("scalar memory known page proof does not cross unknown FS base", () => fixture(
    [0x64,0xa1,0,0,0,0, 0x64,0x8b,0x15,0xfc,0x0f,0,0], async ({ reset, run }) => {
      reset({ fs: DATA + 4, edx: 91 }); const result = run(2);
      equal(result.kind, 4, "FS addition changes page membership"); equal(result.retired, 1, "only initial FS load retires");
      equal(result.address, DATA + 4096, "translated FS address faults"); equal(result.words[2], 91, "failed FS load preserves register");
    }));
  for (const budget of [0,1,2,3,4]) await test(`scalar memory preserve exact partial budget ${budget}`, () => fixture(
    [0x8b,0x03, 0x89,0xdf, 0x8b,0x17, 0x40], async ({ bytes, reset, run }) => {
      setWord(bytes(DATA), 0, 123); reset({ ebx: DATA, eax: 7, edx: 9, edi: 11 });
      const result = run(budget);
      equal(result.retired, budget, "exact retirement prefix"); equal(result.pc, CODE + [0,2,4,6,7][budget], "exact restart PC");
      equal(result.words[0], budget === 0 ? 7 : budget === 4 ? 124 : 123, "load and increment are ordered");
      equal(result.words[2], budget < 3 ? 9 : 123, "following load occurs only with budget");
      equal(result.words[7], budget < 2 ? 11 : DATA, "pointer copy occurs only with budget");
    }));
  for (const [label, first, second, rights, error] of [
    ["read to write", [0x8b,0x03], [0x89,0x13], READ, 7],
    ["write to read", [0x89,0x03], [0x8b,0x13], WRITE, 5],
    ["read to zero RMW", [0x8b,0x03], [0xc1,0x23,0], READ, 7],
    ["write to zero RMW", [0x89,0x03], [0xc1,0x23,0], WRITE, 7],
  ]) await test(`scalar memory cannot strengthen permissions: ${label}`, () => fixture([...first, ...second],
    async ({ map, bytes, reset, run }) => {
      map(DATA + 4096, rights, page([], 0x55)); bytes(DATA).fill(0x55);
      reset({ ebx: DATA + 4094, eax: 0x55555555, edx: 0xaabbccdd, flags: 0xad7 });
      const result = run(20);
      equal(result.kind, 4, "missing second access rights fault"); equal(result.retired, 1, "only permission-establishing prefix completes");
      equal(result.pc, CODE + first.length, "fault PC belongs to second access");
      equal(result.address, DATA + 4096, "fault is on second page"); equal(result.error, error, "exact read/write PF bits");
      equal(result.words[2], 0xaabbccdd, "fault has no destination effect"); equal(result.words[9], 0xad7, "fault retains flags");
      check(bytes(DATA).every(x => x === 0x55) && bytes(DATA + 4096).every(x => x === 0x55), "no partial second write");
    }));
  for (const first of [[0x8a,0x03], [0x8b,0x43,0xfd]]) await test(`scalar memory reject widened or uncovered spans ${first.join()}`, () => fixture(
    [...first, 0x8b,0x13], async ({ bytes, reset, run }) => {
      bytes(DATA).fill(0x33); reset({ ebx: DATA + 4095, edx: 17 });
      const result = run(20); equal(result.kind, 4, "uncovered endpoint still faults");
      equal(result.retired, 1, "only narrower or preceding span retires"); equal(result.address, DATA + 4096, "unmapped endpoint");
      equal(result.words[2], 17, "widening fault preserves destination");
    }));
  for (const change of [[0xb3,0], [0x66,0xbb,0,0x50], [0x0f,0x44,0xd9], [0x8b,0x1b]]) {
    await test(`scalar memory invalidate changed pointer ${change.join()}`, () => fixture(
      [0x8b,0x03, ...change, 0x8b,0x13], async ({ bytes, reset, run }) => {
        setWord(bytes(DATA), 0xffc, 0x5000); reset({ ebx: DATA + 4092, ecx: 0x5000, edx: 7, flags: 0x242 });
        // BL=0 remains mapped, but must load the new address, not reuse old data.
        const result = run(3);
        if (change[0] === 0xb3) { equal(result.kind, 1, "partial pointer update uses new mapped address"); equal(result.words[2], 0, "new pointer value is read"); }
        else { equal(result.kind, 4, "new unmapped pointer faults"); equal(result.retired, 2, "prefix completed"); equal(result.address, 0x5000, "new pointer fault address"); equal(result.words[2], 7, "destination preserved"); }
      }));
  }
  await test("scalar memory check implicit stack independently of previously read PUSH source", () => fixture(
    [0x8b,0x03, 0xff,0x33], async ({ bytes, reset, run }) => {
      setWord(bytes(DATA), 0, 27); reset({ ebx: DATA, esp: 0x6000 });
      const result = run(3); equal(result.kind, 4, "implicit stack store still faults"); equal(result.retired, 1, "PUSH does not partially retire");
      equal(result.address, 0x5ffc, "stack destination fault"); equal(result.error, 6, "unmapped write fault"); equal(result.words[4], 0x6000, "stack pointer unchanged");
    }));
  for (const rawEntry of [false,true]) await test(`scalar memory revalidate on ${rawEntry ? "raw entry" : "budget resume"}`, () => fixture(
    [0x8b,0x03, 0x8b,0x13], async ({ compile, protect, reset, run }) => {
      await compile(CODE, 256, [CODE, CODE + 2]); reset({ ebx: DATA, edx: 19 });
      if (!rawEntry) equal(run(1).retired, 1, "first access completed");
      protect(DATA, WRITE);
      if (rawEntry) reset({ ebx: DATA, edx: 19, eip: CODE + 2 });
      const result = run(1); equal(result.kind, 4, "new admission must check changed rights");
      equal(result.retired, 0, "failed new entry cannot use prior proof"); equal(result.words[2], 19, "new-entry destination unchanged");
    }));
  await test("scalar memory retain journal reservation after no-store RMW proof", () => fixture(
    [0xc1,0x23,0, 0x89,0x13], async ({ handle, bytes, reset, run }) => {
      setWord(bytes(DATA), 0, 0x12345678); reset({ ebx: DATA, edx: 99, flags: 0xad7 });
      const header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4), capacity = header()[2];
      header()[2] = 0;
      const result = run(2, () => { equal(header()[1], 0, "no journal publication on failure"); header()[2] = capacity; });
      equal(result.kind, 10, "previously checked writable span still needs dirty capacity"); equal(result.retired, 1, "zero-count prefix only");
      equal(result.words[9], 0xad7, "zero-count and resource failure preserve flags");
      equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true), 0x12345678, "failed store leaves bytes unchanged");
    }));
  await test("scalar memory keep SMC exit after code-alias write after zero-count RMW", () => fixture(
    [0xc1,0x23,0, 0x89,0x13, 0x40], async ({ handle, alias, bytes, reset, run }) => {
      alias(0x4000, CODE, READ | WRITE); setWord(bytes(CODE), 0x100, 7);
      reset({ ebx: 0x4100, edx: 42, eax: 9 }); const result = run(3);
      equal(result.kind, 7, "code write exits immediately"); equal(result.retired, 2, "following INC never executes");
      equal(result.words[0], 9, "post-store state is exact"); equal(result.pc, CODE + 5, "SMC restart PC");
      equal(new DataView(bytes(CODE).buffer, bytes(CODE).byteOffset).getUint32(0x100, true), 42, "code-alias store commits");
      check(api.backend_validate(handle, CODE) !== 0, "stale code dependency invalidated");
    }, READ | WRITE | EXECUTE | SEALED));
  for (const [label, code, numericMode] of [
    ["scalar and memory", [0xff, 0x03, 0x83, 0xc0, 1, 0x8b, 0x16, 0x0f, 0x0b]],
    ["mixed floating and SIMD", [0xd9, 0xe8, 0xd9, 0xfe, 0xdd, 0x1e, 0x0f, 0x57, 0xc0, 0x0f, 0x0b]],
    ["Runtime timestamp", [0x0f, 0x31, 0x0f, 0x0b]],
    ...[0,1,2].map(mode => [`floating families mode ${mode}`, [
      0xd9,0x03, 0xdd,0x06, 0xdb,0x07, 0xdf,0x07, 0xdf,0x2f,
      0xd8,0xc1, 0xd8,0xe1, 0xd8,0xc9, 0xd8,0xf1, 0xd9,0xfa, 0xd9,0xfc,
      0xd9,0xfe, 0xd9,0xff, 0xd9,0xf1, 0xd9,0xf3, 0xd9,0xf0, 0xd9,0xfd,
      0xd9,0x1b, 0xdd,0x1b, 0xdb,0x1b, 0xdf,0x1b, 0xdf,0x3b, 0xdb,0x3b,
      0xd9,0xe8, 0xdf,0xe0, 0x0f,0x0b,
    ], mode]),
  ]) {
    await test(`native preparation ${label} accesses no linear memory or host import`, () => fixture(code,
      async ({ handle, compile, reset, state, protect, links }) => {
        reset({ eax: 0x12345678, ecx: 0xabcdef01, ebx: DATA, esi: STACK, flags: 0xad7 });
        state()[60] = 0x80; // An actual waiting FP operation would observe this pending exception.
        const id = await compile();
        const module = new WebAssembly.Module(new Uint8Array(api.memory.buffer, api.backend_code_ptr(handle), api.backend_code_len(handle)));
        const env = {};
        for (const entry of WebAssembly.Module.imports(module)) {
          equal(entry.module, "env", "preparation module import namespace");
          if (entry.kind === "function") env[entry.name] = () => { throw Error(`preparation called host import ${entry.name}`); };
          else if (entry.kind === "memory") env[entry.name] = api.memory;
          else if (entry.kind === "table") env[entry.name] = links;
          else {
            check(["backend", "unit", "link_directory"].includes(entry.name), "known immutable transport binding");
            env[entry.name] = entry.name === "backend" ? handle : entry.name === "unit" ? id : api.backend_link_directory(handle);
          }
        }
        const instance = new WebAssembly.Instance(module, { env });
        const unchanged = () => {
          const before = new Uint8Array(api.memory.buffer).slice();
          for (let pass = 0; pass < 4; pass++) equal(instance.exports.prepare(), 0, "bounded preparation succeeds without guest execution");
          const after = new Uint8Array(api.memory.buffer);
          equal(after.length, before.length, "preparation cannot grow execution memory");
          let mismatch = -1;
          for (let index = 0; index < before.length; index++) if (before[index] !== after[index]) { mismatch = index; break; }
          equal(mismatch, -1, "all imported linear memory, including CPU, mappings and journals, stays byte-identical");
        };
        unchanged();
        protect(CODE, READ | WRITE); // The retained native preparation body is harmless even after code invalidation.
        unchanged();
      }, READ | EXECUTE | SEALED, { numericMode }));
  }
  for (const numericMode of [0,1,2]) {
    await test(`native preparation mode ${numericMode} retains the first floating execution`,()=>fixture([
      0xd9,0x05,...wordBytes(DATA), 0xd9,0xfa, 0xdb,0x05,...wordBytes(DATA+4), 0xde,0xc9,
      0xdd,0x1d,...wordBytes(DATA+8), 0xd9,0xe8, 0xdc,0x05,...wordBytes(DATA+8), 0xd9,0xfc,
      0xdb,0x1d,...wordBytes(DATA+16), 0x0f,0x0b,
    ],async({reset,run,state,bytes,generatedUnits,select})=>{
      reset({flags:0xad7});setWord(bytes(DATA),0,0x40100000);setWord(bytes(DATA),4,2);
      const before=Array.from(state()),dataBefore=bytes(DATA).slice();
      const unit=generatedUnits.get(select(CODE));
      equal(unit.instance.exports.prepare(),0,"generated unit prepares before first floating execution");
      equal(JSON.stringify(Array.from(state())),JSON.stringify(before),"preparation keeps complete CPU state");
      check(bytes(DATA).every((value,index)=>value===dataBefore[index]),"preparation keeps all input/output bytes");
      const actual=run(100),view=new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset);
      equal(actual.kind,8,"prepared code reaches UD");equal(actual.retired,9,"first execution retires all floating operations once");
      equal(view.getFloat64(8,true),3,"sqrt and integer multiply are executed");equal(view.getInt32(16,true),4,"load/add/round/store execute after preparation");
      equal(actual.words[9],0xad7,"preparation and floating execution preserve integer flags");equal(actual.words[61],0xffff,"balanced push/pop leaves empty x87 stack");
    },READ|EXECUTE|SEALED,{numericMode}));
  }
  await test("native preparation distinguishes zero guest address from null host pointers",()=>fixture(
    [0xd9,0x05,0,0,0,0,0xd9,0x1d,...wordBytes(DATA),0x0f,0x0b],async({reset,run,map,bytes,select,generatedUnits})=>{
      map(0,READ|WRITE);setWord(bytes(0),0,0x3fc00000);reset();
      equal(generatedUnits.get(select(CODE)).instance.exports.prepare(),0,"zero-address code prepares without access");
      const actual=run(100);equal(actual.kind,8,"real guest zero address executes");equal(actual.retired,2,"zero guest operand never selects host preparation");
      equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(0,true),0x3fc00000,"zero-address source is loaded normally");
    }));
  if (options.fpIr && typeof api.backend_set_numeric_mode === "function") {
    for (const numericMode of [0, 1, 2]) {
      await test(`numerical mode ${numericMode} sine accuracy, budget, store fault and resume`, () => fixture([
        0xdd, 0x05, ...wordBytes(DATA), 0xd9, 0xfe,
        0xdd, 0x1d, ...wordBytes(DATA + 8),
        0xff, 0x05, ...wordBytes(STACK), 0xa1, ...wordBytes(0x4000),
      ], async ({ state, bytes, reset, run, compile, protect, map, saveSnapshot, restoreSnapshot }) => {
        const data = () => new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset, 4096);
        const results = [];
        for (const x of [-3, -0.75, 0.125, 1.23456789012345, 3]) {
          reset(); bytes(STACK).fill(0); data().setFloat64(0, x, true); data().setFloat64(8, 100, true);
          await compile();
          const partial = run(2);
          equal(partial.kind, 1, "numerical instruction budget yields exactly");
          equal(partial.pc, CODE + 8, "resume at store after completed arithmetic");
          equal(data().getFloat64(8, true), 100, "budget does not perform future store");
          const saved = saveSnapshot();
          protect(DATA, READ);
          await compile(CODE + 8);
          const fault = run(10);
          equal(fault.kind, 4, "numerical store retains write protection");
          equal(fault.retired, 0, "fault does not retire or pop store");
          equal(fault.pc, CODE + 8, "precise numerical store fault PC");
          equal(fault.words[60] >>> 11 & 7, partial.words[60] >>> 11 & 7, "fault retains x87 TOP");
          protect(DATA, READ | WRITE);
          restoreSnapshot(saved); await compile(CODE + 8);
          const resumed = run(10);
          equal(resumed.kind, 4, "later independent integer load faults");
          equal(resumed.pc, CODE + 20, "resume does not replay completed arithmetic/store");
          equal(bytes(STACK)[0], 1, "completed integer write happens exactly once");
          equal(resumed.words[60] >>> 11 & 7, 0, "completed store pops exactly once");
          const actual = data().getFloat64(8, true), expected = Math.sin(x);
          check(Math.abs(actual - expected) <= (numericMode === 2 ? 3e-7 : 5e-15), "mode-specific numerical error bound");
          results.push(actual);
        }
        if (numericMode) check(api.backend_numeric_count(numericMode, 0) > 0, "approximate numerical path actually executed");
        // With precision exceptions unmasked, the same arithmetic uses the
        // strict kernel and retains the pending-exception/store suppression.
        reset(); state()[59] = 0x35f; data().setFloat64(0, 1.23456789012345, true);
        data().setFloat64(8, 100, true); await compile();
        const trapped = run(10);
        equal(trapped.kind, 14, "unmasked arithmetic retains x87 exception delivery");
        equal(data().getFloat64(8, true), 100, "pending exception suppresses memory store");
      }, READ | EXECUTE | SEALED, { numericMode }));
    }
  }
  if (options.functionCfg) {
    await test("private CFG edges preserve complete MMX XMM x87 state at budgets and faults", async () => {
      const pad = [...Array(40).fill(0x90),0xeb,0];
      const code = [0xb8,...wordBytes(0x11223344),0x0f,0x6e,0xc0,0x66,0x0f,0x6e,0xc0,...pad,
        0x0f,0x7f,0x05,...wordBytes(DATA),0x66,0x0f,0x7f,0x05,...wordBytes(DATA+16),
        0x0f,0x77,0xd9,0xe8,0xd9,0xe8,0xde,0xc1,...pad,
        0xd9,0xe8,...pad,0xff,0x05,...wordBytes(DATA+32),0x50,0xa1,...wordBytes(0x4000)];
      const expected = [];
      for (const enabled of [false,true]) await fixture(code, async ({reset,state,bytes,run,functionPartitions}) => {
        if(enabled)check(functionPartitions()>=3,"mixed banks actually cross private function boundaries");
        for(const budget of [1,44,46,90,100,130,1000]){
          reset({flags:0xad7});bytes(DATA).fill(0);bytes(STACK).fill(0);
          state()[60]=3<<11;state()[61]=0xffff;
          const result=run(budget),snapshot={result,data:Array.from(bytes(DATA)),stack:Array.from(bytes(STACK))};
          if(!enabled)expected.push(snapshot);
          else equal(JSON.stringify(snapshot),JSON.stringify(expected.shift()),"all CPU words and completed memory effects at private exit");
          if(budget===1000){equal(result.kind,4,"load faults after cross-function prefix");equal(result.address,0x4000,"fault address");
            equal(result.pc,CODE+code.length-5,"fault instruction");equal(result.words[4],STACK+0x7fc,"PUSH completed once");
            equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(32,true),1,"write completed once");}
        }
      },READ|EXECUTE|SEALED,{functionCfg:enabled});
    });
    await test("private CFG indirect cycle uses bounded host stack and preserves guest stack", async () => {
      const code=[0x40,...Array(40).fill(0x90),0xeb,0,0x49,...Array(40).fill(0x90),0x74,2,0xff,0xe2];
      await fixture(code,async({reset,run,functionPartitions})=>{
        check(functionPartitions()>=2,"cycle crosses a private function boundary");
        reset({ecx:10000,edx:CODE});const result=run(85*10000-1);
        equal(result.retired,85*10000-1,"complete real guest work per iteration");
        equal(result.words[0],10000,"all increments executed");equal(result.words[1],0,"loop terminates at exact count");
        equal(result.words[4],STACK+0x800,"no synthetic guest stack movement");equal(result.pc,CODE+code.length,"final conditional target");
        equal(result.kind,1,"exact budget wins before next instruction");
      });
    });
  }
  // MASKMOVDQU uses a full-span fault policy but commits only selected bytes.
  const maskedSource = Uint8Array.from({ length: 16 }, (_, n) => (n * 17 + 9) & 255);
  const seedMaskedState = (state, mask) => {
    for (let n = 35; n < 100; n++) state()[n] = Math.imul(n, 0x1234567) >>> 0;
    state()[59] = 0x37e; state()[60] = 0x80c1; state()[99] = 0x1f81;
    const masks = Uint8Array.from({ length: 16 }, (_, n) => (mask & (1 << n)) ? 0x80 | n : 0x7f);
    state().set(new Uint32Array(maskedSource.buffer), 67);
    state().set(new Uint32Array(masks.buffer), 71);
  };
  const maskedStateEqual = (actual, before, pc) => {
    for (let n = 0; n < 100; n++) equal(actual[n], n === 8 ? pc : before[n], `masked store full state ${n}`);
  };
  const rawPageDirty = (handle, address) => new Uint32Array(api.memory.buffer, api.backend_table(handle) + (address >>> 12) * 16, 4)[2];
  if (options.loopMemory) {
    // A conditional DAG with a data-derived table index and a conditional MMX
    // store. Source/destination recurrences cross page and budget boundaries;
    // no title address or expected native code shape participates in the oracle.
    const loopCode = [0x8b,0x06,0x85,0xc0,0x74,11,0xc1,0xe8,25,
      0x0f,0x6f,0x44,0xc5,0,0x0f,0x7e,0x07,
      0x83,0xc6,4,0x83,0xc7,4,0x49,0x75,0xe6];
    const scenarios = ["normal", "skipped", "mixed", "source-alias", "table-alias",
      "source-edge", "destination-edge", "table-edge", "protected", "unmapped",
      "skipped-unmapped", "wrap", "wrap-fault", "journal-zero", "skipped-journal-zero",
      "pending-mmx", "skipped-pending-mmx", "code-alias"];
    for (const scenario of scenarios) for (const budget of [0,1,5,10,19,20,39,40,81,160,1000]) {
      await test(`loop memory conditional effects ${scenario} budget${budget}`, async () => {
        const results = [];
        for (const enabled of [false, true]) await fixture(loopCode, async ({handle,map,alias,protect,bytes,reset,state,run}) => {
          const mapped = [CODE, DATA, STACK, 0x4000];
          map(0x4000,READ|WRITE); map(DATA+4096,READ|WRITE); mapped.push(DATA+4096);
          for(let n=0;n<128;n++) {setWord(bytes(0x4000),n*8,0x10000+n);setWord(bytes(0x4000),n*8+4,0xdead0000+n);}
          let source=DATA, destination=0x6000, table=0x4000;
          if(scenario==="source-edge")source=DATA+4080;
          if(scenario==="wrap"||scenario==="wrap-fault") {
            map(0xfffff000,READ|WRITE);map(0,READ|WRITE);mapped.push(0xfffff000,0);
            source=scenario==="wrap"?0xfffffffc:0xfffffffe;
          }
          const skipped=scenario.startsWith("skipped");
          for(let n=0;n<16;n++) {
            const value=skipped||(scenario==="mixed"&&n%3===0)?0:((n%128)*0x2000000+1)>>>0;
            for(let b=0;b<4;b++) {const address=(source+n*4+b)>>>0;bytes(address&0xfffff000)[address&4095]=value>>>(b*8);}
          }
          if(scenario==="source-alias"||scenario==="table-alias"||scenario==="code-alias") {
            alias(destination,scenario==="source-alias"?DATA:scenario==="table-alias"?0x4000:CODE,READ|WRITE);
            if(scenario==="code-alias")destination+=0x800;
            mapped.push(0x6000);
          } else if(scenario!=="unmapped"&&scenario!=="skipped-unmapped") {
            map(0x6000,READ|WRITE,page([],0x55));mapped.push(0x6000);
            if(scenario==="protected")protect(0x6000,READ);
            if(scenario==="destination-edge") {destination+=4080;map(0x7000,READ|WRITE,page([],0x55));mapped.push(0x7000);}
          }
          if(scenario==="table-edge") {table+=4088;map(0x5000,READ|WRITE,page([],0x33));mapped.push(0x5000);}
          reset({ecx:16,esi:source,edi:destination,ebp:table,eax:0x11223344,edx:73,ebx:91,flags:0xad7});
          for(let n=35;n<100;n++)state()[n]=Math.imul(n,0x1234567)>>>0;
          state()[59]=scenario.includes("pending-mmx")?0x37e:0x37f;
          state()[60]=scenario.includes("pending-mmx")?0x81:0x2800;
          state()[61]=0x5a5a;state()[99]=0x1f80;
          const journal=()=>new Uint32Array(api.memory.buffer,api.backend_table(handle)-16,4),capacity=journal()[2];
          if(scenario.includes("journal-zero"))journal()[2]=0;
          const result=run(budget,()=>{journal()[2]=capacity;});
          const memory=mapped.map(address=>({address,bytes:Array.from(bytes(address)).join(),dirty:rawPageDirty(handle,address)}));
          results.push({result,memory});
          if(budget===1000&&scenario==="normal") {
            equal(result.retired,160,"sixteen complete iterations retire exact work");equal(result.pc,CODE+26,"loop reaches its sentinel");
            equal(result.words[1],0,"loop counter terminates");equal(result.words[6],DATA+64,"source advances exactly");
            for(let n=0;n<16;n++)equal(new DataView(bytes(0x6000).buffer,bytes(0x6000).byteOffset).getUint32(n*4,true),0x10000+n,"bounded index selects exact table value");
          }
          if(budget===1000&&skipped) {
            equal(result.retired,112,"skipped arms consume no imaginary instructions");equal(result.pc,CODE+26,"skipped loop terminates without conditional faults");
            for(const address of mapped)equal(rawPageDirty(handle,address),0,"skipped writes publish no dirty pages");
          }
          if(budget===1000&&scenario==="pending-mmx") {equal(result.retired,4,"pending exception retains completed scalar prefix");equal(result.pc,CODE+9,"pending exception belongs to first reached MMX instruction");}
          if(budget===1000&&scenario==="journal-zero") {equal(result.retired,5,"journal exhaustion retains completed load");equal(result.pc,CODE+14,"journal exhaustion belongs to actual store");equal(result.kind,10,"capacity failure stays explicit");}
        },READ|EXECUTE|SEALED,{loopMemory:enabled});
        equal(JSON.stringify(results[1]),JSON.stringify(results[0]),"loop proof preserves complete CPU, fault, memory and dirty effects");
      });
    }
    await test("loop memory descending pointers recertify the preceding page",async()=>{
      const results=[];
      for(const enabled of [false,true])await fixture([0x8b,0x06,0x89,0x07,0x83,0xee,4,0x83,0xef,4,0x49,0x75,0xf3],
        async({map,bytes,reset,run})=>{
          for(const address of [0x3000,0x6000,0x7000])map(address,READ|WRITE);
          for(let n=0;n<16;n++){const address=0x3010-n*4;setWord(bytes(address&~4095),address&4095,0x12340000+n);}
          reset({esi:0x3010,edi:0x7010,ecx:16});const result=run(1000);
          equal(result.retired,96,"descending loop retires sixteen full iterations");
          for(let n=0;n<16;n++){const address=0x7010-n*4;equal(new DataView(bytes(address&~4095).buffer,bytes(address&~4095).byteOffset).getUint32(address&4095,true),0x12340000+n,"descending copy preserves exact bytes");}
          results.push({result,memory:[...bytes(0x6000),...bytes(0x7000)]});
        },READ|EXECUTE|SEALED,{loopMemory:enabled});
      equal(JSON.stringify(results[1]),JSON.stringify(results[0]),"descending page transitions retain precise state");
    });
    await test("loop memory completed iterations survive a fault and mapping repair", async()=>{
      const results=[];
      for(const enabled of [false,true])await fixture(loopCode,async({map,bytes,reset,run,compile,state})=>{
        map(0x4000,READ|WRITE);map(0x6000,READ|WRITE);
        for(let n=0;n<4;n++)setWord(bytes(DATA),4080+n*4,1);
        setWord(bytes(0x4000),0,0x12345678);
        reset({esi:DATA+4080,edi:0x6000,ebp:0x4000,ecx:8});
        const fault=run(1000);equal(fault.kind,4,"next source page faults");equal(fault.pc,CODE,"fault is at the next iteration load");equal(fault.retired,40,"four previous iterations completed once");
        equal(fault.words[1],4,"fault retains remaining iterations");equal(fault.words[7],0x6010,"fault retains committed destination progress");
        map(DATA+4096,READ|WRITE);for(let n=0;n<4;n++)setWord(bytes(DATA+4096),n*4,1);
        await compile(state()[8]);const finish=run(1000);equal(finish.retired,40,"repair runs only the remaining iterations");equal(finish.words[7],0x6020,"repair does not repeat completed stores");
        results.push({fault,finish,destination:Array.from(bytes(0x6000))});
      },READ|EXECUTE|SEALED,{loopMemory:enabled});
      equal(JSON.stringify(results[1]),JSON.stringify(results[0]),"fault/restart preserves the ordinary architectural result");
    });
    await test("loop memory budget resume and changed mappings retain exact completed effects", async()=>{
      const results=[];
      for(const enabled of [false,true])await fixture(loopCode,async({map,bytes,reset,state,run,compile,saveSnapshot,restoreSnapshot})=>{
        map(0x4000,READ|WRITE);map(0x6000,READ|WRITE);
        for(let n=0;n<16;n++)setWord(bytes(DATA),n*4,1);setWord(bytes(0x4000),0,0x11111111);
        reset({esi:DATA,edi:0x6000,ebp:0x4000,ecx:16});const first=run(43),saved=saveSnapshot();
        equal(first.pc,CODE+6,"partial iteration budget resumes at SHR");
        reset({eax:7,ecx:3,flags:0x202});restoreSnapshot(saved);
        // Replacement and allocation growth happen at the Runtime handoff.
        map(0x4000,READ|WRITE,page(wordBytes(0x22222222)));map(0x9000,READ|WRITE);map(0xa000,READ|WRITE);
        await compile(state()[8]);let remaining=117,total=0,result;
        for(let attempts=0;attempts<20&&remaining>0;attempts++) {
          result=run(remaining);total+=result.retired;remaining-=result.retired;
          if(remaining>0)await compile(result.pc);
        }
        equal(total,117,"resume completes exactly the remaining guest work");equal(result.words[1],0,"resumed loop terminates");
        for(let n=0;n<16;n++)equal(new DataView(bytes(0x6000).buffer,bytes(0x6000).byteOffset).getUint32(n*4,true),n<4?0x11111111:0x22222222,"only future iterations observe remapped table");
        results.push({words:result.words.slice(0,100),memory:Array.from(bytes(0x6000))});
      },READ|EXECUTE|SEALED,{loopMemory:enabled});
      equal(JSON.stringify(results[1]),JSON.stringify(results[0]),"context restore and mapping change invalidate old loop proofs");
    });
  }
  for (const offset of [3, 4081, 4088, 4095]) for (const mask of [0, 1, 0x8000, 0x00ff, 0xff00, 0x5555, 0xffff]) {
    await test(`MASKMOVDQU unaligned selected bytes offset${offset} mask${mask}`, () => fixture(
      [0x66, 0x0f, 0xf7, 0xc1], async ({ handle, map, alias, protect, bytes, reset, state, run }) => {
        map(DATA + 4096, WRITE, page([], 0x55)); alias(0x4000, DATA, READ); protect(DATA, WRITE); bytes(DATA).fill(0x55);
        reset({ edi: DATA + offset, eax: 13, flags: 0xed7 }); seedMaskedState(state, mask);
        const before = Array.from(state()), result = run(1);
        equal(result.kind, 1, "unaligned write-only destination succeeds"); equal(result.retired, 1, "one complete masked store");
        maskedStateEqual(result.words, before, CODE + 4);
        for (let p = 0; p < 2; p++) {
          let touched = false;
          for (let n = 0; n < 4096; n++) {
            const lane = p * 4096 + n - offset, selected = lane >= 0 && lane < 16 && (mask & (1 << lane));
            equal(bytes(DATA + p * 4096)[n], selected ? maskedSource[lane] : 0x55, "only selected destination bytes change");
            touched ||= Boolean(selected);
          }
          equal(rawPageDirty(handle, DATA + p * 4096), touched ? 2 : 0, "only written pages become pending dirty");
          if (p === 0) {
            equal(rawPageDirty(handle, 0x4000), touched ? 2 : 0, "written allocation propagates dirty to read-only alias");
            equal(Array.from(bytes(0x4000)).join(), Array.from(bytes(DATA)).join(), "alias observes exact selected bytes");
          }
        }
      }));
  }
  for (const mask of [0, 0x00ff, 0xff00, 0xffff]) for (const secondFlags of [null, READ]) {
    await test(`MASKMOVDQU full-span fault and same-PC repair mask${mask} second${secondFlags}`, () => fixture(
      [0xff, 0x06, 0x66, 0x0f, 0xf7, 0xc1], async ({ map, protect, bytes, reset, state, compile, run }) => {
        bytes(DATA).fill(0x55); if (secondFlags !== null) map(DATA + 4096, secondFlags, page([], 0x55));
        reset({ edi: DATA + 4088, esi: STACK, flags: 0x203 }); seedMaskedState(state, mask);
        const before = Array.from(state()), fault = run(100);
        before[9] = 0x203; // INC dword [ESI] changes 0 to 1, retaining CF.
        equal(fault.kind, 4, "masked-off bytes still participate in write permission checks"); equal(fault.retired, 1, "only prior INC retires");
        equal(fault.address, DATA + 4096, "faulting second-page address"); equal(fault.error, secondFlags === null ? 6 : 7, "user write PF bits");
        equal(fault.faultPc, CODE + 2, "fault is at MASKMOVDQU"); maskedStateEqual(fault.words, before, CODE + 2);
        check(bytes(DATA).every(x => x === 0x55), "no first-page partial stores"); equal(bytes(STACK)[0], 1, "preceding INC completed once");
        if (secondFlags === null) map(DATA + 4096, WRITE, page([], 0x55)); else protect(DATA + 4096, WRITE);
        await compile(CODE + 2); const repaired = run(1); equal(repaired.retired, 1, "only repaired instruction retires");
        maskedStateEqual(repaired.words, before, CODE + 6); equal(bytes(STACK)[0], 1, "repair does not repeat prior effect");
        for (let lane = 0; lane < 16; lane++) equal(bytes(DATA + (lane < 8 ? 0 : 4096))[(4088 + lane) % 4096], mask & (1 << lane) ? maskedSource[lane] : 0x55, "repaired selected byte");
      }));
  }
  for (const [mask, capacity, needed] of [[0, 0, 0], [1, 0, 1], [0x00ff, 1, 1], [0xff00, 1, 1], [0xffff, 1, 2]]) {
    await test(`MASKMOVDQU reserves selected dirty pages only mask${mask} slots${capacity}`, () => fixture(
      [0x66, 0x0f, 0xf7, 0xc1], async ({ handle, map, bytes, reset, state, run }) => {
        map(DATA + 4096, WRITE, page([], 0x55)); bytes(DATA).fill(0x55);
        reset({ edi: DATA + 4088, flags: 0xad7 }); seedMaskedState(state, mask); const before = Array.from(state());
        const header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4), originalCapacity = header()[2];
        header()[2] = capacity;
        const failed = needed > capacity, result = run(1, () => {
          equal(header()[1], failed ? 0 : needed, "journal is atomic and records only selected pages"); header()[2] = originalCapacity;
        });
        equal(result.kind, failed ? 10 : 1, "resource failure is checked before any store"); equal(result.retired, failed ? 0 : 1, "retirement follows whole-store success");
        maskedStateEqual(result.words, before, failed ? CODE : CODE + 4);
        for (let lane = 0; lane < 16; lane++) equal(bytes(DATA + (lane < 8 ? 0 : 4096))[(4088 + lane) % 4096], !failed && (mask & (1 << lane)) ? maskedSource[lane] : 0x55, "resource failure preserves all destination bytes");
      }));
  }
  for (const mask of [0, 0x00ff, 0xff00]) {
    await test(`MASKMOVDQU SMC applies only to selected executable alias mask${mask}`, () => fixture(
      [0x66, 0x0f, 0xf7, 0xc1, 0x40], async ({ handle, alias, map, bytes, reset, state, compile, run }) => {
        alias(0x4000, CODE, READ | WRITE); map(0x5000, WRITE, page([], 0x55));
        reset({ edi: 0x4ff8, eax: 9, flags: 0xad7 }); seedMaskedState(state, mask);
        const before = Array.from(state()), result = run(2), codeWritten = Boolean(mask & 0xff);
        equal(result.kind, codeWritten ? 7 : 1, "unselected executable bytes do not trigger SMC"); equal(result.retired, codeWritten ? 1 : 2, "SMC stops before following INC");
        if (codeWritten) {
          maskedStateEqual(result.words, before, CODE + 4); check(api.backend_validate(handle, CODE) !== 0, "written code dependency invalidated");
          const output = bytes(CODE).slice(); await compile(CODE + 4); seedMaskedState(state, 0);
          const resumed = run(1); equal(resumed.words[0], 10, "following INC executes once"); equal(Array.from(bytes(CODE)).join(), Array.from(output).join(), "resume does not repeat code store");
        } else {
          equal(result.words[0], 10, "following instruction continues"); equal(rawPageDirty(handle, CODE), 0, "unselected code allocation stays clean");
        }
      }, READ | WRITE | EXECUTE | SEALED));
  }
  for (const [prefix, address, fs, kind] of [[[], 0xfffffff8, 0, 6], [[0x36], 0xfffffff8, 0, 9], [[0x64], 0xfffffff8, 0x2010, 6], [[0x64], 3, DATA, 1]]) {
    await test(`MASKMOVDQU full span segment policy ${prefix} ${address}`, () => fixture(
      [...prefix, 0x66, 0x0f, 0xf7, 0xc1], async ({ bytes, reset, state, run }) => {
        bytes(DATA).fill(0x55); reset({ edi: address, fs, flags: 0xad7 }); seedMaskedState(state, 0xffff);
        const before = Array.from(state()), result = run(1); equal(result.kind, kind, "segment/wrap or valid FS outcome");
        equal(result.retired, kind === 1 ? 1 : 0, "full span fault does not retire"); maskedStateEqual(result.words, before, kind === 1 ? CODE + prefix.length + 4 : CODE);
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], kind === 1 && n >= 3 && n < 19 ? maskedSource[n - 3] : 0x55, "invalid span never wraps stores to low memory");
      }));
  }
  if (typeof api.backend_set_diagnostic_stop === "function") {
    const arm = (handle, pc) => ok(api.backend_set_diagnostic_stop(handle, 1, pc), "arm diagnostic PC");
    const disarm = handle => ok(api.backend_set_diagnostic_stop(handle, 0, 0), "clear diagnostic PC");
    const sameArchitecture = (actual, expected, label) => {
      for (let n = 0; n < 100; n++) equal(actual[n], expected[n], `${label}: architectural word ${n}`);
    };
    if (options.flowIr || options.predicateIr || options.valueIr || options.rootedSsa || options.structuredLoops || options.affineMemory || options.loopMemory) {
      await test("diagnostic stop rejects incompatible experimental lowering without CPU changes", () => fixture(
        [0x90], async ({ handle, state }) => {
          const before = Array.from(state());
          check(api.backend_set_diagnostic_stop(handle, 1, CODE) !== 0, "unsupported diagnostic mode is rejected");
          sameArchitecture(state(), before, "rejected diagnostic mode");
        }));
    } else {
      await test("diagnostic interior stop preserves prior effects and retires stale cached bodies", () => fixture(
        [0xff, 0x07, 0x83, 0xc0, 1, 0x8b, 0x1e, 0x83, 0xea, 1], async ({ handle, reset, state, bytes, map, compile, run }) => {
          reset({ eax: 0x7fffffff, ebx: 9, edx: 7, edi: DATA, esi: 0x5000, flags: 0x203 });
          const before = Array.from(state()), code = bytes(CODE).slice(), old = api.backend_cache_selected(handle);
          arm(handle, CODE + 5);
          check(api.backend_cache_validate(handle, old, CODE) !== 0, "arming rejects the old ordinary unit");
          await compile();
          const hit = run(100), expected = before.slice(); expected[0] = 0x80000000; expected[8] = CODE + 5; expected[9] = 0xa96;
          equal(hit.kind, 17, "host diagnostic exit precedes missing operand access"); equal(hit.retired, 2, "stop never retires");
          equal(hit.faultPc, CODE + 5, "exact diagnostic PC"); equal(hit.address, 0, "no invented fault address"); equal(hit.error, 0, "no invented guest error");
          sameArchitecture(hit.words, expected, "completed prefix at diagnostic stop"); equal(bytes(DATA)[0], 1, "prior INC stored once");
          disarm(handle); await compile(CODE + 5);
          const fault = run(100); equal(fault.kind, 4, "cleared stop exposes the real operand fault"); equal(fault.retired, 0, "fault does not replay prefix");
          sameArchitecture(fault.words, expected, "failed next instruction"); equal(bytes(DATA)[0], 1, "prefix remains once");
          map(0x5000, READ, page(wordBytes(0x76543210))); arm(handle, CODE + 7); await compile(CODE + 5);
          const moved = run(100); expected[3] = 0x76543210; expected[8] = CODE + 7;
          equal(moved.kind, 17, "moved stop catches the following instruction"); equal(moved.retired, 1, "only repaired MOV retires");
          sameArchitecture(moved.words, expected, "same-PC resume"); equal(bytes(DATA)[0], 1, "moving stop does not replay write");
          equal(Array.from(bytes(CODE)).join(), Array.from(code).join(), "arming/clearing never changes guest code bytes");
        }));
      await test("diagnostic return target preserves CALL RET stack effects across compiled units", () => fixture(
        [0xe8, 3, 0, 0, 0, 0x43, 0xeb, 6, 0xb8, 0x44, 0x33, 0x22, 0x11, 0xc3, 0x90], async ({ handle, reset, state, bytes, compile, select, run }) => {
          reset({ eax: 9, ebx: 12, flags: 0xad7 }); const before = Array.from(state());
          arm(handle, CODE + 5); await compile(CODE + 5, 1); await compile(CODE + 8, 2); await compile(CODE, 1);
          let result, retired = 0;
          for (let attempt = 0; attempt < 3; attempt++) {
            result = run(100); retired += result.retired;
            if (result.kind !== 2) break;
            select(result.pc);
          }
          equal(result.kind, 17, "actual guest return address reaches diagnostic target"); equal(retired, 3, "CALL MOV RET retire once");
          const expected = before.slice(); expected[0] = 0x11223344; expected[8] = CODE + 5;
          sameArchitecture(result.words, expected, "completed CALL RET");
          equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0x7fc, true), CODE + 5, "guest return address was written");
          disarm(handle); await compile(CODE + 5); equal(run(1).words[3], 13, "same return PC resumes without executing CALL again");
        }));
      await test("diagnostic direct branch target stops before its instruction", () => fixture(
        [0xeb, 1, 0x40, 0x43], async ({ handle, reset, state, compile, run }) => {
          reset({ eax: 5, ebx: 6 }); const before = Array.from(state()); arm(handle, CODE + 3); await compile();
          const result = run(100), expected = before.slice(); expected[8] = CODE + 3;
          equal(result.kind, 17, "internal branch reaches stop"); equal(result.retired, 1, "only JMP retires");
          sameArchitecture(result.words, expected, "branch boundary");
        }));
      for (const [name, code] of [["MMX", [0x0f, 0x6e, 0xc0]], ["SSE", [0x0f, 0x59, 0xc1]], ["x87", [0xd9, 0xe8]]]) {
        await test(`diagnostic before ${name} leaves pending numeric environment untouched`, () => fixture(
          code, async ({ handle, reset, state, compile, run }) => {
            reset({ eax: 0x11223344, flags: 0xad7 });
            for (let n = 35; n < 100; n++) state()[n] = Math.imul(n, 0x1234567) >>> 0;
            state()[59] = 0x37e; state()[60] = 0x80c1; state()[99] = 0;
            const before = Array.from(state()); arm(handle, CODE); await compile();
            equal(run(0).kind, 1, "zero fuel yields before observing stop");
            const result = run(100); equal(result.kind, 17, "pending numeric exception is not attempted"); equal(result.retired, 0, "stop does not retire");
            sameArchitecture(result.words, before, "preinstruction numeric stop");
          }));
      }
      await test("diagnostic after MMX SSE and x87 publishes the complete raw register banks", () => fixture(
        [0x0f, 0x6e, 0xc0, 0x66, 0x0f, 0xfd, 0xca, 0x0f, 0x77, 0xd9, 0xe8], async ({ handle, reset, state, compile, run }) => {
          const setup = () => { reset({ eax: 0x11223344, flags: 0xad7 }); state().set([0x00010001, 0x00010001, 0x00010001, 0x00010001], 71); state().set([0x00020002, 0x00020002, 0x00020002, 0x00020002], 75); };
          setup(); const reference = run(4); equal(reference.kind, 1, "unarmed instruction-boundary reference");
          equal(reference.words[35], 0x11223344, "MMX raw low32"); equal(reference.words[37], 0xffff, "MMX raw exponent");
          for (let n = 71; n < 75; n++) equal(reference.words[n], 0x00030003, "SSE word sums");
          equal(reference.words[56], 0, "FLD1 raw significand low"); equal(reference.words[57], 0x80000000, "FLD1 raw significand high"); equal(reference.words[58], 0x3fff, "FLD1 raw exponent");
          setup(); arm(handle, CODE + 11); await compile(); const hit = run(100);
          equal(hit.kind, 17, "diagnostic follows completed numeric work"); equal(hit.retired, 4, "numeric operations retire once");
          sameArchitecture(hit.words, reference.words, "all banks at diagnostic exit");
        }));
      await test("diagnostic SCAS stop preserves suspended recipe through snapshot and later fault", () => fixture(
        [0xf3, 0xae], async ({ handle, reset, state, bytes, compile, run, saveSnapshot, restoreSnapshot }) => {
          bytes(DATA).fill(0x42); reset({ eax: 0x42, ecx: 4097, edi: DATA, flags: 0xa97 }); state()[10] = 0xfffff72a;
          const original = Array.from(state()), partial = run(1);
          equal(partial.kind, 12, "SCAS yields after a bounded prefix"); equal(partial.error, 4096, "4096 completed comparisons"); equal(partial.retired, 0, "REP still unretired");
          const suspended = saveSnapshot(); arm(handle, CODE); await compile(); const hit = run(1);
          equal(hit.kind, 17, "suspended SCAS can be observed before resumption"); equal(hit.retired, 0, "diagnostic adds no element");
          sameArchitecture(hit.words, partial.words, "completed REP progress");
          const saved = saveSnapshot(); equal(Array.from(saved.slice(420)).join(), Array.from(suspended.slice(420)).join(), "original flags recipe is unchanged");
          restoreSnapshot(saved); equal(Array.from(saveSnapshot()).join(), Array.from(saved).join(), "full512 snapshot roundtrip at diagnostic stop");
          disarm(handle); await compile(); const fault = run(1), expected = original.slice(); expected[1] = 1; expected[7] = DATA + 4096;
          equal(fault.kind, 4, "next SCAS element raises actual missing-page fault"); equal(fault.address, DATA + 4096, "next unread element"); equal(fault.retired, 0, "faulting REP still unretired");
          sameArchitecture(fault.words, expected, "fault restores original flags with completed progress");
          equal(new DataView(saveSnapshot().buffer).getUint32(432, true), 0, "fault ends the suspended flags recipe");
        }));
      await test("diagnostic unmapped PC observes before fetch and clearing exposes actual fetch fault", () => fixture(
        [0x90], async ({ handle, reset, state, compile, run }) => {
          arm(handle, 0x5000); reset({ eip: 0x5000, eax: 7, flags: 0xad7 }); const before = Array.from(state()); await compile(0x5000);
          equal(run(0).kind, 1, "zero budget precedes diagnostic fetch boundary"); const hit = run(1);
          equal(hit.kind, 17, "unmapped target is only a host observation"); equal(hit.retired, 0, "no fetch retired"); sameArchitecture(hit.words, before, "unmapped diagnostic");
          disarm(handle); await compile(0x5000); const fault = run(1);
          equal(fault.kind, 4, "real instruction fetch faults after clearing"); equal(fault.error, 20, "user nonpresent execute error"); equal(fault.address, 0x5000, "real fetch address");
          sameArchitecture(fault.words, before, "unmapped fault");
        }));
    }
  }
  const widthMask = (width) => (1n << BigInt(width)) - 1n;
  const signed = (value, width) => BigInt.asIntN(width, BigInt(value));
  const parity = (value) => {
    let bits = Number(value & 255n), count = 0;
    for (; bits; bits >>>= 1) count += bits & 1;
    return count % 2 === 0;
  };
  const common = (result, width) => (result === 0n ? ZF : 0)
    | (result & (1n << BigInt(width - 1)) ? SF : 0) | (parity(result) ? PF : 0);
  function arithmetic(operation, width, av, bv, carry) {
    const mask = widthMask(width), a = BigInt(av) & mask, b = BigInt(bv) & mask;
    const subtract = operation === "sub" || operation === "sbb" || operation === "cmp";
    const incoming = operation === "adc" || operation === "sbb" ? BigInt(carry) : 0n;
    const full = subtract ? a - b - incoming : a + b + incoming;
    const result = full & mask;
    const fullSigned = subtract ? signed(a, width) - signed(b, width) - incoming
      : signed(a, width) + signed(b, width) + incoming;
    const lower = -(1n << BigInt(width - 1)), upper = (1n << BigInt(width - 1)) - 1n;
    return { result: Number(result), flags: common(result, width)
      | ((subtract ? full < 0n : full > mask) ? CF : 0)
      | (((a ^ b ^ result) & 16n) ? AF : 0)
      | (fullSigned < lower || fullSigned > upper ? OF : 0) };
  }
  const retainedResult = (initial, result, width) =>
    Number((BigInt(initial >>> 0) & ~widthMask(width)) | BigInt(result)) >>> 0;
  let random = 0x7e57c0de;
  const next = () => { random ^= random << 13; random ^= random >>> 17; random ^= random << 5; return random >>> 0; };
  for (const [operation, opcode] of [["add", 0x00], ["adc", 0x10], ["sub", 0x28], ["sbb", 0x18], ["cmp", 0x38]]) {
    for (const width of [8, 16, 32]) {
      await test(`${operation} ${width}-bit architectural arithmetic`, () => fixture(
        [...(width === 16 ? [0x66] : []), opcode + (width === 8 ? 0 : 1), 0xd8], async ({ reset, run }) => {
          const mask = Number(widthMask(width));
          const pairs = [[0, 0], [mask, 0], [mask, 1], [mask, mask], [mask >>> 1, 1],
            [2 ** (width - 1), 1], [0, mask], [0x10, 1], [0x0f, 1]];
          for (let i = 0; i < 36; i += 1) pairs.push([next() & mask, next() & mask]);
          for (const [a, b] of pairs) for (const carry of [0, 1]) {
            const eax = retainedResult(0xa55a5a00, Number(BigInt(a >>> 0) & widthMask(width)), width);
            const ebx = retainedResult(0x5aa5a500, Number(BigInt(b >>> 0) & widthMask(width)), width);
            const reference = arithmetic(operation, width, eax, ebx, carry);
            reset({ eax, ebx, flags: 0x202 | (carry ? CF : 0) });
            const result = run(1);
            equal(result.retired, 1, "one complete arithmetic instruction");
            equal(result.words[0], operation === "cmp" ? eax : retainedResult(eax, reference.result, width), "GPR result");
            equal(result.words[9] & ARITH, reference.flags, `${operation}/${width} flags a=${eax} b=${ebx} carry=${carry}`);
            equal(result.words[10] & ARITH, ARITH, "defined arithmetic flags");
            equal(result.words[3], ebx, "source register unchanged");
          }
        }));
    }
  }
  for (const [operation, opcode] of [["add", 0x00], ["adc", 0x10], ["sub", 0x28], ["sbb", 0x18],
    ["cmp", 0x38], ["and", 0x20], ["or", 0x08], ["xor", 0x30], ["test", 0x84],
    ["inc", null], ["dec", null], ["neg", null]]) {
    for (const [width, high8] of [[8, false], [16, false], [32, false], [8, true]]) {
      const unary = opcode === null, logical = ["and", "or", "xor", "test"].includes(operation);
      const producer = [...(width === 16 ? [0x66] : []),
        ...(unary ? [operation === "neg" ? (width === 8 ? 0xf6 : 0xf7) : (width === 8 ? 0xfe : 0xff),
          ({ inc: 0xc0, dec: 0xc8, neg: 0xd8 })[operation] + (high8 ? 4 : 0)]
          : [opcode + Number(width !== 8), high8 ? 0xfc : 0xd8])];
      await test(`cold flag recipe ${operation} ${width}${high8 ? " high byte" : ""} exact fault and budget state`,
        () => fixture([...producer, 0x8b, 0x16, 0x83, 0xc1, 1, 0x0f, 0x0b], async ({ reset, run, bytes }) => {
          setWord(bytes(DATA), 0, 0x55667788);
          const mask = Number(widthMask(width)), pairs = [[0, 0], [mask, 1], [mask, mask],
            [mask >>> 1, 1], [2 ** (width - 1), 1], [0, mask], [0x10, 1], [0x0f, 1]];
          const pack = (initial, value) => high8 ? ((initial & 0xffff00ff) | value << 8) >>> 0
            : retainedResult(initial, value, width);
          for (const [a, b] of pairs) for (const carry of [0, 1]) {
            const initialFlags = 0xad6 | carry, eax = pack(0xa55a5a96, a), ebx = pack(0x5aa5a569, b);
            let reference;
            if (logical) {
              const result = Number((operation === "or" ? BigInt(a) | BigInt(b)
                : operation === "xor" ? BigInt(a) ^ BigInt(b) : BigInt(a) & BigInt(b)) & widthMask(width));
              reference = { result, flags: common(BigInt(result), width) | (initialFlags & AF) };
            } else if (operation === "neg") reference = arithmetic("sub", width, 0, a, 0);
            else if (operation === "inc" || operation === "dec") {
              reference = arithmetic(operation === "inc" ? "add" : "sub", width, a, 1, 0);
              reference.flags = (reference.flags & ~CF) | carry;
            } else reference = arithmetic(operation, width, a, b, carry);
            const producedFlags = ((initialFlags & ~ARITH) | reference.flags) >>> 0;
            const expectedEax = operation === "cmp" || operation === "test" ? eax : pack(eax, reference.result);
            for (const fault of [false, true]) for (const budget of [0, 1, 2, 3, 100]) {
              reset({ eax, ebx, ecx: 0x7fffffff, edx: 0x11223344, esi: fault ? DATA + 4096 : DATA, flags: initialFlags });
              const retired = Math.min(budget, fault ? 1 : 3), result = run(budget);
              const kind = fault && budget > 1 ? 4 : budget > 3 ? 8 : 1;
              equal(result.kind, kind, "cold recipe exact exit"); equal(result.retired, retired, "cold recipe completed prefix");
              equal(result.pc, CODE + [0, producer.length, producer.length + 2, producer.length + 5][retired], "cold recipe exact PC");
              equal(result.words[0], retired ? expectedEax : eax, "partial-register result is published once");
              equal(result.words[3], ebx, "recipe preserves source register");
              equal(result.words[2], retired >= 2 ? 0x55667788 : 0x11223344, "fault cannot publish load destination");
              equal(result.words[1], retired === 3 ? 0x80000000 : 0x7fffffff, "replacement producer only after successful load");
              equal(result.words[9], retired === 0 ? initialFlags : retired === 3
                ? ((initialFlags & ~ARITH) | arithmetic("add", 32, 0x7fffffff, 1, 0).flags) >>> 0 : producedFlags,
              `all flags at ${operation}/${width}/${high8} a=${a} b=${b} carry=${carry} budget=${budget} fault=${fault}`);
              equal(result.words[10], logical && retired > 0 && retired < 3 ? (~AF) >>> 0 : 0xffffffff, "precise defined flags");
              if (kind === 4) { equal(result.address, DATA + 4096, "exact inaccessible operand"); equal(result.faultPc, CODE + producer.length, "faulting instruction does not retire"); }
            }
          }
        }));
    }
  }
  await test("cold flag recipe retains a loaded comparison across an aliasing store", () => fixture([
    0x83, 0x3e, 1, 0xc7, 0x06, ...wordBytes(0x2468ace0), 0x8b, 0x17, 0x83, 0xc1, 1, 0x0f, 0x0b,
  ], async ({ reset, run, bytes }) => {
    for (const value of [0, 1, 0x80000000, 0xffffffff]) for (const fault of [false, true])
      for (const budget of [0, 1, 2, 3, 4, 100]) {
        setWord(bytes(DATA), 0, value);
        reset({ esi: DATA, edi: fault ? DATA + 4096 : DATA, ecx: 0x7fffffff, edx: 0x11223344, flags: 0xad7 });
        const result = run(budget), retired = Math.min(budget, fault ? 2 : 4);
        equal(result.kind, fault && budget > 2 ? 4 : budget > 4 ? 8 : 1, "aliasing store exact stop");
        equal(result.retired, retired, "aliasing store exact prefix");
        equal(result.pc, CODE + [0, 3, 9, 11, 14][retired], "aliasing store exact continuation");
        equal(result.words[9], retired === 0 ? 0xad7 : (0xad7 & ~ARITH) | (retired === 4
          ? arithmetic("add", 32, 0x7fffffff, 1, 0).flags : arithmetic("cmp", 32, value, 1, 0).flags),
        "recovery uses the original loaded value even after an aliasing store");
        equal(result.words[2], retired >= 3 ? 0x2468ace0 : 0x11223344, "later load observes the changed memory only on success");
        equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true),
          retired >= 2 ? 0x2468ace0 : value, "comparison recovery never replays or undoes the store");
      }
  }));
  for (const failure of ["none", "read", "write", "journal", "later-load"]) {
    await test(`cold flag recipe replacement retains the prior producer at ${failure} failure`, () => fixture([
      0x83, 0xc0, 1, 0x83, 0x06, 1, 0x8b, 0x17, 0x83, 0xc1, 1, 0x0f, 0x0b,
    ], async ({ handle, reset, run, bytes, protect }) => {
      const header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4);
      for (const budget of [0, 1, 2, 3, 4, 100]) {
        protect(DATA, READ | WRITE); setWord(bytes(DATA), 0, 0xffffffff);
        if (failure === "read") protect(DATA, 0);
        if (failure === "write") protect(DATA, READ);
        reset({ eax: 0x7fffffff, esi: DATA, edi: failure === "later-load" ? DATA + 4096 : DATA,
          ecx: 0x0f, edx: 0x11223344, flags: 0xad7 });
        const stop = failure === "none" ? 4 : failure === "later-load" ? 2 : 1;
        const capacity = header()[2]; if (failure === "journal") header()[2] = 0;
        const result = run(budget, () => { header()[2] = capacity; }), retired = Math.min(budget, stop);
        equal(result.kind, budget <= stop ? 1 : failure === "none" ? 8 : failure === "journal" ? 10 : 4,
          "replacement checks all fallible effects before replacing the flag producer");
        equal(result.retired, retired, "only completed producers retire");
        equal(result.pc, CODE + [0, 3, 6, 8, 11][retired], "replacement exact PC");
        const reference = retired === 1 ? arithmetic("add", 32, 0x7fffffff, 1, 0)
          : retired === 4 ? arithmetic("add", 32, 0x0f, 1, 0) : arithmetic("add", 32, 0xffffffff, 1, 0);
        equal(result.words[9], retired === 0 ? 0xad7 : (0xad7 & ~ARITH) | reference.flags, "exact live producer flags");
        equal(result.words[0], retired ? 0x80000000 : 0x7fffffff, "first register write completes once");
        equal(result.words[2], retired >= 3 ? 0 : 0x11223344, "later load cannot cross a failure");
        equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true), retired >= 2 ? 0 : 0xffffffff,
          "failed memory producer does not publish bytes");
      }
    }));
  }
  await test("cold flag recipe publishes the new memory producer before executable alias invalidation", () => fixture([
    0x83, 0xc0, 1, 0x83, 0x06, 1, 0x8b, 0x17, 0x83, 0xc1, 1, 0x0f, 0x0b,
  ], async ({ alias, compile, reset, run, bytes }) => {
    alias(0x4000, CODE, READ | WRITE); setWord(bytes(CODE), 0x100, 0xffffffff); await compile();
    reset({ eax: 0x7fffffff, esi: 0x4100, edi: DATA + 4096, ecx: 0x0f, edx: 0x11223344, flags: 0xad7 });
    const result = run(100);
    equal(result.kind, 7, "completed store invalidates executable alias"); equal(result.retired, 2, "both completed producers retire");
    equal(result.pc, CODE + 6, "SMC names the following instruction");
    equal(result.words[9], (0xad7 & ~ARITH) | arithmetic("add", 32, 0xffffffff, 1, 0).flags, "SMC recovers the completed new producer");
    equal(result.words[0], 0x80000000, "earlier register effect remains"); equal(result.words[2], 0x11223344, "later load never executes");
    equal(new DataView(bytes(CODE).buffer, bytes(CODE).byteOffset).getUint32(0x100, true), 0, "memory producer is committed exactly once");
  }));
  for (const [operation, replacement] of [["adc", [0x83, 0xd1, 0]], ["sbb", [0x83, 0xd9, 0]], ["inc", [0x41]]]) {
    await test(`cold flag recipe preserves carry across ${operation} consumption and replacement`, () => fixture([
      0x83, 0xc0, 1, 0x8b, 0x16, ...replacement, 0x8b, 0x17, 0x83, 0xc3, 1, 0x0f, 0x0b,
    ], async ({ reset, run, bytes }) => {
      setWord(bytes(DATA), 0, 0x55667788);
      for (const eax of [0x7fffffff, 0xffffffff]) for (const ecx of [0, 0x7fffffff, 0x80000000, 0xffffffff])
        for (const failedLoad of [0, 1, 2]) for (const budget of [0, 1, 2, 3, 4, 5, 100]) {
          reset({ eax, ecx, ebx: 0x0f, edx: 0x11223344, esi: failedLoad === 1 ? DATA + 4096 : DATA,
            edi: failedLoad === 2 ? DATA + 4096 : DATA, flags: 0xad7 });
          const stop = failedLoad === 1 ? 1 : failedLoad === 2 ? 3 : 5, retired = Math.min(stop, budget);
          const first = arithmetic("add", 32, eax, 1, 0), second = arithmetic(operation === "inc" ? "add" : operation,
            32, ecx, operation === "inc" ? 1 : 0, first.flags & CF);
          if (operation === "inc") second.flags = (second.flags & ~CF) | (first.flags & CF);
          const result = run(budget);
          equal(result.kind, budget <= stop ? 1 : failedLoad ? 4 : 8, "carry consumer exact exit");
          equal(result.retired, retired, "carry consumer exact prefix");
          equal(result.pc, CODE + [0, 3, 5, 5 + replacement.length, 7 + replacement.length, 10 + replacement.length][retired],
            "carry consumer precise continuation");
          equal(result.words[1], retired >= 3 ? second.result : ecx, "consumer uses the completed producer carry");
          equal(result.words[9], retired === 0 ? 0xad7 : (0xad7 & ~ARITH) | (retired < 3 ? first.flags
            : retired < 5 ? second.flags : arithmetic("add", 32, 0x0f, 1, 0).flags), "carry and other flags recover from their proper producer");
        }
    }));
  }
  for (const [operation, modrm] of [["add", 0xc0], ["adc", 0xd0], ["sub", 0xe8], ["sbb", 0xd8]]) {
    await test(`${operation} sign-extended immediate into AX`, () => fixture([0x66, 0x83, modrm, 0xff],
      async ({ reset, run }) => {
        for (const a of [0, 1, 0x7fff, 0x8000, 0xffff]) for (const carry of [0, 1]) {
          const reference = arithmetic(operation, 16, a, 0xffff, carry);
          reset({ eax: 0xabcd0000 | a, flags: 0x202 | carry });
          const result = run(1);
          equal(result.words[0], (0xabcd0000 | reference.result) >>> 0, "AX result preserves high half");
          equal(result.words[9] & ARITH, reference.flags, "sign-extended immediate flags");
        }
      }));
  }
  for (const width of [8, 16, 32]) {
    await test(`MUL ${width}-bit complete unsigned double-width product`, () => fixture(
      [...(width === 16 ? [0x66] : []), width === 8 ? 0xf6 : 0xf7, 0xe1], async ({ reset, run }) => {
        const values = [0, 1, 2, 2 ** (width - 1), Number(widthMask(width))];
        for (const a of values) for (const b of values) {
          const eax = retainedResult(0xa55a0000, a, width), edx = 0x5aa51234;
          const ecx = retainedResult(0xcc330000, b, width);
          const product = BigInt(a) * BigInt(b), low = Number(product & widthMask(width));
          const high = Number(product >> BigInt(width));
          reset({ eax, edx, ecx, flags: 0xad7 });
          const result = run(1);
          equal(result.retired, 1, "MUL retires exactly once");
          equal(result.words[0], width === 8 ? retainedResult(eax, Number(product), 16) : retainedResult(eax, low, width), "full low product destination");
          equal(result.words[2], width === 8 ? edx : retainedResult(edx, high, width), "high product preserves unaffected EDX bits");
          equal(result.words[1], ecx, "source register remains unchanged");
          equal(result.words[9] & (CF | OF), high ? CF | OF : 0, "unsigned high half determines CF and OF");
          equal(result.words[9] & ~ARITH, 0xad7 & ~ARITH, "nonarithmetic flags retained");
          equal(result.words[10] & ARITH, CF | OF, "only CF and OF remain defined");
        }
      }));
  }
  for (const [label, code, width] of [["AH aliases AX destination", [0xf6, 0xe4], 8], ["EDX aliases high destination", [0xf7, 0xe2], 32]]) {
    await test(`MUL ${label} reads the original source before either write`, () => fixture(code, async ({ reset, run }) => {
      for (const eax of [0, 0x1234ffff, 0xffffffff, 0x87658002]) for (const edx of [0, 1, 116, 0xffffffff]) {
        const a = BigInt(eax) & widthMask(width), b = BigInt(width === 8 ? eax >>> 8 & 255 : edx);
        const product = a * b;
        reset({ eax, edx, flags: 0x202 });
        const result = run(1);
        equal(result.retired, 1, "aliased MUL completes");
        equal(result.words[0], width === 8 ? retainedResult(eax, Number(product), 16) : Number(product & widthMask(width)), "aliased product low");
        equal(result.words[2], width === 8 ? edx : Number(product >> 32n), "aliased product high");
        equal(result.words[9] & (CF | OF), product >> BigInt(width) ? CF | OF : 0, "alias overflow flags");
      }
    }));
  }
  for (const width of [8, 16, 32]) {
    await test(`MUL ${width}-bit memory fault retains the completed store and flags`, () => fixture(
      [0x83, 0x07, 1, ...(width === 16 ? [0x66] : []), width === 8 ? 0xf6 : 0xf7, 0xa3, 0, 0, 0, 0],
      async ({ reset, run, map, bytes, compile }) => {
        const prefix = page(wordBytes(0x7fffffff)); map(DATA, READ | WRITE, prefix);
        const source = width === 8 ? 0x4000 : DATA + 4095;
        reset({ eax: 0x8765ffff, edx: 0xa55a1234, ebx: source, edi: DATA, flags: 0x203 });
        const first = run(100);
        equal(first.kind, 4, "unmapped multiplier is a precise page fault");
        equal(first.pc, CODE + 3, "MUL remains the fault instruction");
        equal(first.address, width === 8 ? 0x4000 : DATA + 4096, "first inaccessible multiplier byte");
        equal(first.retired, 1, "only prior memory ADD retired");
        equal(first.words[0], 0x8765ffff, "fault preserves entire EAX");
        equal(first.words[2], 0xa55a1234, "fault preserves entire EDX");
        equal(first.words[9] & ARITH, arithmetic("add", 32, 0x7fffffff, 1, 0).flags, "prefix flags survive fault");
        equal(first.words[10] & ARITH, ARITH, "failed MUL does not undefine prior flags");
        equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true), 0x80000000, "completed prefix store remains visible");
        map(source & ~4095, READ | WRITE, width === 8 ? page([3]) : bytes(DATA).slice());
        if (width !== 8) { bytes(DATA)[4095] = 3; map(DATA + 4096, READ | WRITE); }
        await compile(CODE + 3);
        const resumed = run(1), product = (0x8765ffffn & widthMask(width)) * 3n;
        equal(resumed.retired, 1, "repair resumes only MUL");
        equal(resumed.words[0], width === 8 ? retainedResult(0x8765ffff, Number(product), 16) : retainedResult(0x8765ffff, Number(product & widthMask(width)), width), "repaired low product");
        equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true), 0x80000000, "resume never duplicates prefix store");
      }));
  }
  await test("LOCK MUL is invalid before register or flag effects", () => fixture([0xf0, 0xf7, 0xe2], async ({ reset, run }) => {
    reset({ eax: 7, edx: 116, flags: 0xad7 }); const result = run(1);
    equal(result.kind, 8, "LOCK MUL raises UD"); equal(result.retired, 0, "invalid operation unretired");
    equal(result.words[0], 7, "invalid operation preserves EAX"); equal(result.words[2], 116, "invalid operation preserves EDX");
    equal(result.words[9], 0xad7, "invalid operation preserves flags");
  }));
  const wideSignedReference = (eax, edx, source, width) => {
    const product = signed(eax, width) * signed(source, width);
    const full = BigInt.asUintN(width * 2, product), low = Number(full & widthMask(width));
    return { eax: width === 8 ? retainedResult(eax, Number(full), 16) : retainedResult(eax, low, width),
      edx: width === 8 ? edx >>> 0 : retainedResult(edx, Number(full >> BigInt(width)), width),
      flags: product !== BigInt.asIntN(width, product) ? CF | OF : 0 };
  };
  const checkWideSigned = (result, reference, originalFlags, label) => {
    equal(result.words[0], reference.eax, `${label}: complete low destination`);
    equal(result.words[2], reference.edx, `${label}: complete high destination`);
    equal(result.words[9] & (CF | OF), reference.flags, `${label}: signed overflow flags`);
    equal(result.words[9] & ~ARITH, originalFlags & ~ARITH, `${label}: nonarithmetic flags`);
    equal(result.words[10] & ARITH, CF | OF, `${label}: other arithmetic flags are undefined`);
  };
  for (const width of [8, 16, 32]) {
    await test(`one-operand IMUL ${width}-bit produces an exact signed double-width product`, () => fixture(
      [...(width === 16 ? [0x66] : []), width === 8 ? 0xf6 : 0xf7, 0xe9], async ({ reset, state, run }) => {
        const high = 2 ** (width - 1), mask = Number(widthMask(width));
        const values = [0, 1, 2, 3, high - 2, high - 1, high, high + 1, mask - 1, mask];
        const pairs = values.flatMap(a => values.map(b => [a, b]));
        for (let n = 0; n < 48; n++) pairs.push([next() & mask, next() & mask]);
        for (const [a, b] of pairs) {
          const eax = retainedResult(0xa55a0000, a >>> 0, width), edx = 0x5aa51234;
          const ecx = retainedResult(0xcc330000, b >>> 0, width);
          reset({ eax, edx, ecx, flags: 0xad7 });
          for (let n = 35; n < 100; n++) state()[n] = n === 99 ? 0x1f80 : (n * 0x1234567) >>> 0;
          const initial = Array.from(state()), zero = run(0);
          equal(zero.retired, 0, "zero budget does not start multiply");
          for (let n = 0; n < 100; n++) equal(zero.words[n], initial[n], "zero budget preserves architectural state");
          const result = run(1); equal(result.retired, 1, "signed wide multiply retires once");
          checkWideSigned(result, wideSignedReference(eax, edx, ecx, width), 0xad7, `signed ${width}`);
          equal(result.words[1], ecx, "explicit source register unchanged");
          for (let n = 35; n < 100; n++) equal(result.words[n], initial[n], "integer multiply preserves all FP/vector state");
        }
      }));
  }
  for (const [name, width, modrm, source] of [
    ["AL", 8, 0xe8, (eax) => eax & 255], ["AH", 8, 0xec, (eax) => eax >>> 8 & 255],
    ["AX", 16, 0xe8, (eax) => eax & 65535], ["DX", 16, 0xea, (_eax, edx) => edx & 65535],
    ["EAX", 32, 0xe8, (eax) => eax], ["EDX", 32, 0xea, (_eax, edx) => edx],
  ]) await test(`one-operand IMUL ${name} snapshots aliased inputs before implicit writes`, () => fixture(
    [...(width === 16 ? [0x66] : []), width === 8 ? 0xf6 : 0xf7, modrm], async ({ reset, run }) => {
      for (const eax of [0, 1, 0x123480ff, 0x7654ff80, 0x80008000, 0xffffffff]) for (const edx of [0, 1, 0x7fffffff, 0x80000000, 0xffff8001, 0xffffffff]) {
        reset({ eax, edx, flags: 0xad7 }); const result = run(1);
        equal(result.retired, 1, "aliased signed multiply completes");
        checkWideSigned(result, wideSignedReference(eax, edx, source(eax, edx), width), 0xad7, name);
      }
    }));
  for (const width of [8, 16, 32]) {
    await test(`one-operand IMUL ${width}-bit reads EAX/EDX-based source address before mutation`, async () => {
      for (const base of [0, 2]) await fixture(
        [...(width === 16 ? [0x66] : []), width === 8 ? 0xf6 : 0xf7, 0x28 | base],
        async ({ bytes, reset, run }) => {
          bytes(DATA).fill(0x6b); bytes(DATA).set([0xfd, 0xff, 0xff, 0xff], 3); const initial = bytes(DATA).slice();
          const eax = base === 0 ? DATA + 3 : 0x8765fffb, edx = base === 2 ? DATA + 3 : 0x5aa51234;
          reset({ eax, edx, flags: 0xad7 }); const result = run(1);
          equal(result.retired, 1, "unaligned addressed multiplier completes");
          checkWideSigned(result, wideSignedReference(eax, edx, -3, width), 0xad7, "address alias");
          for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], initial[n], "source memory remains unchanged");
        });
    });
    await test(`one-operand IMUL ${width}-bit source fault preserves earlier flags/store and repairs once`, () => fixture(
      [0x83, 0x07, 1, ...(width === 16 ? [0x66] : []), width === 8 ? 0xf6 : 0xf7, 0x2b],
      async ({ bytes, map, reset, run, compile }) => {
        setWord(bytes(DATA), 0, 0x7fffffff); bytes(DATA)[4095] = 0xfd;
        const source = width === 8 ? 0x4000 : DATA + 4095, eax = 0x8765fffb, edx = 0x5aa51234;
        reset({ eax, edx, ebx: source, edi: DATA, flags: 0x203 }); const first = run(100);
        equal(first.kind, 4, "missing source byte faults before implicit destinations");
        equal(first.pc, CODE + 3, "fault points at signed multiply"); equal(first.retired, 1, "only earlier ADD completed");
        equal(first.address, width === 8 ? 0x4000 : DATA + 4096, "precise first missing byte");
        equal(first.words[0], eax, "failed multiply retains EAX"); equal(first.words[2], edx, "failed multiply retains EDX");
        equal(first.words[9] & ARITH, arithmetic("add", 32, 0x7fffffff, 1, 0).flags, "earlier ADD flags survive failure");
        equal(first.words[10] & ARITH, ARITH, "failed multiply does not undefine flags");
        equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true), 0x80000000, "earlier store is committed once");
        if (width === 8) map(source, READ, page([0xfd]));
        else map(DATA + 4096, READ, page(Array(width / 8 - 1).fill(0xff)));
        await compile(CODE + 3); const resumed = run(1); equal(resumed.retired, 1, "only repaired multiply retires");
        checkWideSigned(resumed, wideSignedReference(eax, edx, -3, width), first.words[9], "repaired signed multiply");
        equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true), 0x80000000, "repair cannot replay preceding ADD");
      }));
  }
  await test("consecutive signed wide multiplies invalidate both implicit GPR facts", () => fixture(
    [0xb8, 2, 0, 0, 0, 0xba, 3, 0, 0, 0, 0xf7, 0xe9, 0x01, 0xd3, 0xf7, 0xea, 0x01, 0xd3, 0xf7, 0xe8, 0x89, 0x07, 0x89, 0x57, 4],
    async ({ reset, run, bytes, compile }) => {
      reset({ ecx: 0xfffffffe, ebx: 10, edi: DATA }); const whole = run(9);
      equal(whole.retired, 9, "whole multiplication chain completes"); equal(whole.words[0], 16, "later multiply reads prior low result");
      equal(whole.words[2], 0, "later multiply reads prior high result"); equal(whole.words[3], 9, "uses each updated EDX rather than original constant");
      equal(bytes(DATA)[0], 16, "stored final low result"); equal(bytes(DATA)[4], 0, "stored final high result");
      reset({ ecx: 0xfffffffe, ebx: 10, edi: DATA }); bytes(DATA).fill(0);
      let stepped;
      for (const offset of [0, 5, 10, 12, 14, 16, 18, 20, 22]) { await compile(CODE + offset); stepped = run(1); equal(stepped.retired, 1, "each exact budget boundary resumes once"); }
      for (let n = 0; n < 100; n++) if (n !== 9) equal(stepped.words[n], whole.words[n], "budget-partitioned chain equals uninterrupted execution");
      equal(stepped.words[9] & ~(ARITH & ~(CF | OF)), whole.words[9] & ~(ARITH & ~(CF | OF)), "partitioned chain preserves defined and nonarithmetic flags");
    }));
  await test("signed wide overflow feeds adjacent CF/OF consumers", () => fixture(
    [0xf7, 0xe9, 0x0f, 0x92, 0xc3, 0x0f, 0x90, 0xc1], async ({ reset, run }) => {
      for (const [a, b] of [[0xffffffff, 2], [0x80000000, 1], [0x80000000, 0xffffffff], [0x7fffffff, 2], [0, 0xffffffff]]) {
        reset({ eax: a, ecx: b, ebx: 0x12345600, edx: 0x87654321 }); const result = run(3);
        const reference = wideSignedReference(a, 0x87654321, b, 32), overflow = reference.flags ? 1 : 0;
        equal(result.retired, 3, "multiply and both flag consumers complete"); equal(result.words[0], reference.eax, "flag users preserve low product"); equal(result.words[2], reference.edx, "flag users preserve high product");
        equal(result.words[3] & 255, overflow, "SETC sees signed product overflow"); equal(result.words[1] & 255, overflow, "SETO sees same signed overflow");
      }
    }));
  await test("signed wide multiply shares a generated unit with vector, divide and FP arithmetic", () => fixture(
    [0x0f, 0x57, 0xc0, 0x0f, 0x6e, 0xc0, 0xf7, 0xe9, 0xf7, 0xf3, 0x0f, 0x77, 0xd9, 0xe8],
    async ({ reset, state, run }) => {
      reset({ eax: 7, ecx: 3, ebx: 5, edx: 0xdeadbeef });
      state()[59] = 0x37f; state()[60] = 0; state()[61] = 0xffff;
      state().fill(0x5a5aa5a5, 67, 99); const result = run(6);
      equal(result.retired, 6, "mixed local kinds execute within one generated unit");
      equal(result.words[0], 4, "DIV sees signed multiply low product21"); equal(result.words[2], 1, "DIV sees signed multiply high product0");
      for (let n = 67; n < 71; n++) equal(result.words[n], 0, "XMM0 retains XORPS result");
      for (let n = 71; n < 99; n++) equal(result.words[n], 0x5a5aa5a5, "other vector registers retained");
      equal(result.words[35], 7, "MMX alias retained original EAX before multiply"); equal(result.words[36], 0, "MOVD zero-extended MMX payload"); equal(result.words[37], 0xffff, "MMX exponent alias retained after EMMS");
      equal(result.words[60], 7 << 11, "FLD1 after EMMS pushes one value"); equal(result.words[61], 0x3fff, "precise empty-to-one stack tags");
      equal(result.words[56], 0, "new ST0 low significand"); equal(result.words[57], 0x80000000, "new ST0 high significand"); equal(result.words[58], 0x3fff, "new ST0 exponent");
    }));
  await test("LOCK one-operand IMUL rejects every width without source access or effects", async () => {
    for (const width of [8, 16, 32]) for (const memory of [false, true]) await fixture(
      [0xf0, ...(width === 16 ? [0x66] : []), width === 8 ? 0xf6 : 0xf7, memory ? 0x2b : 0xe9], async ({ reset, run }) => {
        reset({ eax: 7, edx: 0x12345678, ebx: 0x6000, ecx: 0xffffffff, flags: 0xad7 }); const result = run(1);
        equal(result.kind, 8, "LOCK is UD before a missing memory read"); equal(result.retired, 0, "invalid multiply does not retire");
        equal(result.words[0], 7, "UD preserves EAX"); equal(result.words[2], 0x12345678, "UD preserves EDX"); equal(result.words[9], 0xad7, "UD preserves flags");
      });
  });
  for (const width of [8, 16, 32]) for (const operation of ["cmp", "test"]) {
    await test(`forwarded comparison ${operation}${width} preserves all conditions and partial budgets`, async () => {
      const conditions = [f => f & OF, f => !(f & OF), f => f & CF, f => !(f & CF),
        f => f & ZF, f => !(f & ZF), f => f & (CF | ZF), f => !(f & (CF | ZF)),
        f => f & SF, f => !(f & SF), f => f & PF, f => !(f & PF),
        f => Boolean(f & SF) !== Boolean(f & OF), f => Boolean(f & SF) === Boolean(f & OF),
        f => Boolean(f & ZF) || Boolean(f & SF) !== Boolean(f & OF),
        f => !(f & ZF) && Boolean(f & SF) === Boolean(f & OF)];
      const values = [0,1,2,0x7f,0x80,0xff,0x7fff,0x8000,0xffff,0x7fffffff,0x80000000,0xffffffff];
      const compare = [...(width === 16 ? [0x66] : []), operation === "cmp" ? (width === 8 ? 0x38 : 0x39) : (width === 8 ? 0x84 : 0x85), 0xd8];
      for (let condition = 0; condition < 16; condition++) {
        const branch = compare.length + 5, fall = branch + 2, taken = fall + 5;
        await fixture([...compare,0xb8,0x34,0x12,0,0,0x70+condition,5,0x83,0xc1,1,0xeb,3,0x83,0xc1,2,0x0f,0x0b],
          async ({ reset, run }) => {
            for (const a of values) for (const b of values) for (let budget = 0; budget <= 4; budget++) {
              const seed = 0xad7;
              const flags = operation === "cmp" ? (seed & ~ARITH) | arithmetic("sub",width,a,b,0).flags
                : (seed & ~(ARITH & ~AF)) | common(BigInt((a & b) >>> 0) & widthMask(width),width);
              const goes = Boolean(conditions[condition](flags)), delta = goes ? 2 : 1;
              reset({eax:a,ebx:b,ecx:0x123,flags:seed});
              const result = run(budget);
              equal(result.kind,1,"comparison sequence stops at exact budget");
              equal(result.retired,budget,"comparison, copy, branch and arithmetic retire individually");
              equal(result.words[0],budget>=2?0x1234:a,"later overwrite does not change captured comparison");
              equal(result.words[3],b,"comparison source remains unchanged");
              equal(result.words[1],budget>=4?0x123+delta:0x123,"numeric branch chooses original operands");
              const expectedPc=[0,compare.length,branch,goes?taken:fall,(goes?taken:fall)+3][budget];
              equal(result.pc,CODE+expectedPc,"exact branch and partial-budget continuation");
              const expectedFlags=budget===0?seed:budget===4?(seed&~ARITH)|arithmetic("add",32,0x123,delta,0).flags:flags;
              equal(result.words[9],expectedFlags,"raw EFLAGS match at every observable boundary");
              equal(result.words[10],operation==="test"&&budget>0&&budget<4?(0xffffffff&~AF)>>>0:0xffffffff,"defined mask remains architectural");
            }
          });
      }
    });
  }
  await test("forwarded comparison retains loaded values through writes, faults and raw entries", () => fixture(
    [0x39,0x1e,0xc7,0x06,0,0,0,0,0x8b,0x0f,0x7c,5,0x83,0xc2,1,0xeb,3,0x83,0xc2,2,0x0f,0x0b],
    async ({handle,bytes,reset,run,map,compile,select,alias}) => {
      const beforeFlags=0xad7, comparison=(beforeFlags&~ARITH)|arithmetic("sub",32,0xffffffff,1,0).flags;
      setWord(bytes(DATA),0,0xffffffff);
      reset({esi:DATA,edi:0x5000,ebx:1,edx:9,flags:beforeFlags});
      const fault=run(100);
      equal(fault.kind,4,"later memory read faults before branch");equal(fault.pc,CODE+8,"fault at original read");
      equal(fault.retired,2,"comparison and preceding memory write are complete");equal(fault.words[9],comparison,"comparison flags remain observable at fault");
      equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(0,true),0,"comparison input was overwritten once");
      map(0x5000,READ|WRITE,page([7]));
      await compile(CODE+8);
      const resumed=run(3);
      equal(resumed.words[2],11,"resumed raw entry uses architectural flags without reloading old comparison data");
      // The original full block captures its comparison before changing the
      // memory input. A raw branch root below is a different admission case.
      setWord(bytes(DATA),0,0xffffffff);
      reset({esi:DATA,edi:0x5000,ebx:1,edx:9,flags:beforeFlags});select(CODE);
      equal(run(5).words[2],11,"completed comparison survives later source memory mutation");
      // A direct branch entry in the same prepared graph must use incoming flags.
      await compile(CODE,256,[CODE,CODE+10]);
      reset({eip:CODE+10,flags:0x202,edx:9});select(CODE+10);
      equal(run(2).words[2],10,"raw branch entry never inherits a predecessor predicate");
      alias(0x6000,CODE,READ|WRITE);
      reset({esi:0x6100,edi:0x5000,ebx:1,edx:9,flags:beforeFlags});
      const codeId=select(CODE);
      const smc=run(100);
      equal(smc.kind,7,"code alias store exits before the pending branch");equal(smc.retired,2,"SMC preserves only completed prefix");
      equal(smc.pc,CODE+8,"SMC continuation excludes later read/branch");equal(smc.words[2],9,"SMC cannot execute captured branch early");
      check(api.backend_cache_validate(handle,codeId,CODE)!==0,"modified code requires fresh admission");
    }));
  for (const width of [16, 32]) {
    await test(`IMUL ${width}-bit signed product and overflow`, () => fixture(
      [...(width === 16 ? [0x66] : []), 0x0f, 0xaf, 0xc3], async ({ reset, run }) => {
        const values = [0, 1, 2, 2 ** (width - 1) - 1, 2 ** (width - 1), Number(widthMask(width))];
        for (const a of values) for (const b of values) {
          const product = signed(a, width) * signed(b, width);
          const low = BigInt.asUintN(width, product);
          const overflow = product !== BigInt.asIntN(width, low);
          reset({ eax: a, ebx: b, flags: 0x202 | ARITH });
          const result = run(1);
          equal(result.words[0], Number(low), "IMUL low product");
          equal(result.words[9] & (CF | OF), overflow ? CF | OF : 0, "IMUL signed overflow flags");
          equal(result.words[10] & ARITH, CF | OF, "only IMUL CF/OF are defined");
        }
      }));
  }
  const shldReference = (destination, source, count) => {
    const shift = count & 31, dest = BigInt(destination >>> 0), src = BigInt(source >>> 0);
    if (shift === 0) return { result: destination >>> 0, flags: 0, defined: 0 };
    const result = ((dest << BigInt(shift)) | (src >> BigInt(32 - shift))) & 0xffffffffn;
    return { result: Number(result), flags: common(result, 32) | (Number(dest >> BigInt(32 - shift) & 1n) ? CF : 0)
      | (shift === 1 && ((dest ^ result) & 0x80000000n) ? OF : 0),
    defined: CF | PF | SF | ZF | (shift === 1 ? OF : 0) };
  };
  for (const count of [0, 1, 10, 31, 32, 33, 255]) {
    await test(`SHLD EDX,ESI immediate ${count} uses count mask and exact double-word bits`, () => fixture(
      [0x0f, 0xa4, 0xf2, count], async ({ reset, state, run }) => {
        for (const dest of [0, 1, 0x80000000, 0x87654321, 0xffffffff]) for (const source of [0, 0x12345678, 0xffffffff]) {
          reset({ edx: dest, esi: source, flags: 0xad7 }); state()[10] = 0x800008d5;
          const result = run(1), reference = shldReference(dest, source, count);
          equal(result.retired, 1, "SHLD completes once"); equal(result.words[2], reference.result, "exact SHLD result");
          equal(result.words[6], source, "source retained");
          if ((count & 31) === 0) {
            equal(result.words[9], 0xad7, "zero count retains every flag value");
            equal(result.words[10], 0x800008d5, "zero count retains exact defined mask");
          } else {
            equal(result.words[9] & reference.defined, reference.flags, "all defined SHLD flags");
            equal(result.words[10] & ARITH, reference.defined, "AF undefined and OF only defined for one-bit count");
            equal(result.words[9] & ~ARITH, 0xad7 & ~ARITH, "nonarithmetic flags retained");
          }
        }
      }));
  }
  for (const destinationAliasesCl of [false, true]) {
    await test(`SHLD CL count ${destinationAliasesCl ? "destination" : "source"} alias reads original ECX`, () => fixture(
      destinationAliasesCl ? [0x0f, 0xa5, 0xc1] : [0x0f, 0xa5, 0xca], async ({ reset, run }) => {
        for (const count of [0, 1, 10, 31, 32, 33, 255]) {
          const ecx = (0x87654300 | count) >>> 0, eax = 0x12345678, edx = 0xabcdef01;
          const reference = shldReference(destinationAliasesCl ? ecx : edx, destinationAliasesCl ? eax : ecx, count);
          reset({ eax, ecx, edx, flags: 0xad7 }); const result = run(1);
          equal(result.retired, 1, "CL alias SHLD completes");
          equal(result.words[destinationAliasesCl ? 1 : 2], reference.result, "CL is sampled before destination write");
          equal(result.words[destinationAliasesCl ? 0 : 1], destinationAliasesCl ? eax : ecx, "non-destination source unchanged");
          if (count & 31) equal(result.words[9] & reference.defined, reference.flags, "alias flags");
          else equal(result.words[9], 0xad7, "zero CL count leaves flags");
        }
      }));
  }
  await test("SHLD same source and destination observes original word twice", () => fixture(
    [0x0f, 0xa5, 0xd2], async ({ reset, run }) => {
      for (const count of [0, 1, 10, 31, 32, 33, 255]) for (const edx of [0x80000001, 0x87654321]) {
        reset({ ecx: count, edx }); const result = run(1), reference = shldReference(edx, edx, count);
        equal(result.words[2], reference.result, "same-source result equals exact rotate bit pattern");
        if (count & 31) equal(result.words[9] & reference.defined, reference.flags, "same-source flags");
      }
    }));
  await test("SHLD one-bit CF and OF feed following SETcc before state publication", () => fixture(
    [0x0f, 0xa4, 0xf2, 1, 0x0f, 0x90, 0xc0, 0x0f, 0x92, 0xc3], async ({ reset, run }) => {
      for (const edx of [0, 0x40000000, 0x80000000, 0xc0000000]) {
        reset({ eax: 0xaabbccdd, ebx: 0x11223344, edx, esi: 0xffffffff });
        const result = run(3), reference = shldReference(edx, 0xffffffff, 1);
        equal(result.retired, 3, "SHLD and both flag consumers retire");
        equal(result.words[0], (0xaabbcc00 | Number(Boolean(reference.flags & OF))) >>> 0, "SETO observes one-bit overflow");
        equal(result.words[3], (0x11223300 | Number(Boolean(reference.flags & CF))) >>> 0, "SETB observes original top bit");
      }
    }));
  const shrdReference = (destination, source, count) => {
    const shift = count & 31, dest = BigInt(destination >>> 0), src = BigInt(source >>> 0);
    if (!shift) return { result: destination >>> 0, flags: 0, defined: 0 };
    const result = ((dest >> BigInt(shift)) | (src << BigInt(32 - shift))) & 0xffffffffn;
    return { result: Number(result), flags: common(result, 32) | Number(dest >> BigInt(shift - 1) & 1n)
      | (shift === 1 && ((dest ^ result) & 0x80000000n) ? OF : 0),
    defined: CF | PF | ZF | SF | (shift === 1 ? OF : 0) };
  };
  const seedShrdState = (words) => {
    for (let n = 35; n < 100; n++) words[n] = (0xa1b2c3d4 ^ Math.imul(n, 0x1020304)) >>> 0;
    words[59] = 0x37e; words[60] = 0x8081; // Integer instructions ignore pending x87 exceptions.
  };
  const checkShrdState = (result, before, destination, reference, nextPc, retired = 1) => {
    equal(result.retired, retired, "SHRD precise retirement"); equal(result.pc, nextPc, "SHRD precise continuation");
    for (let n = 0; n < 100; n++) if (![8, 9, 10].includes(n))
      equal(result.words[n], n === destination && reference ? reference.result : before[n], `SHRD architectural word ${n}`);
    if (!reference?.defined) {
      equal(result.words[9], before[9], "zero count or fault preserves every flag value");
      equal(result.words[10], before[10], "zero count or fault preserves exact flag validity");
    } else {
      equal(result.words[9] & reference.defined, reference.flags, "all architecturally defined SHRD flags");
      equal(result.words[9] & ~ARITH, before[9] & ~ARITH, "nonarithmetic flags unchanged");
      equal(result.words[10], ((before[10] & ~ARITH) | reference.defined) >>> 0, "AF undefined; OF defined only for count one");
    }
  };
  await test("SHRD all 256 immediate encodings preserve exact bits and flag validity", async () => {
    for (let count = 0; count < 256; count++) {
      const destination = count & 7, source = count >>> 3 & 7;
      await fixture([0x0f, 0xac, 0xc0 | source << 3 | destination, count], async ({ reset, state, run }) => {
        for (const high of [false, true]) {
          reset({ flags: 0xad7 }); seedShrdState(state()); state()[10] = 0x800008d5;
          state().set([0, 1, 0x80000000, 0xffffffff, 0x87654321, 0x12345678, 0x55555555, 0xaaaaaaaa]);
          if (high) for (let n = 0; n < 8; n++) state()[n] = ~state()[n];
          const before = Array.from(state()), reference = shrdReference(before[destination], before[source], count);
          checkShrdState(run(1), before, destination, reference, CODE + 4);
        }
      });
    }
  });
  for (const [destination, source] of [[0, 2], [1, 0], [2, 1], [1, 1], [2, 2]]) {
    await test(`SHRD CL all 256 counts with destination${destination}/source${source} aliases`, () => fixture(
      [0x0f, 0xad, 0xc0 | source << 3 | destination], async ({ reset, state, run }) => {
        for (let count = 0; count < 256; count++) {
          reset({ eax: 0x80000001, edx: 0x12345678, ecx: (0x87654300 | count) >>> 0, flags: 0xad7 });
          seedShrdState(state()); state()[10] = 0x800008d5;
          const before = Array.from(state()), reference = shrdReference(before[destination], before[source], before[1]);
          checkShrdState(run(1), before, destination, reference, CODE + 3);
        }
      }));
  }
  await test("SHRD one-bit CF and OF are visible to following SETcc without a host exit", () => fixture(
    [0x0f, 0xac, 0xf2, 1, 0x0f, 0x90, 0xc0, 0x0f, 0x92, 0xc3], async ({ reset, run }) => {
      for (const destination of [0, 1, 0x80000000, 0x80000001]) for (const source of [0, 1]) {
        reset({ eax: 0xaabbccdd, ebx: 0x11223344, edx: destination, esi: source });
        const result = run(3), reference = shrdReference(destination, source, 1);
        equal(result.retired, 3, "shift and both flag consumers complete");
        equal(result.words[0], (0xaabbcc00 | Number(Boolean(reference.flags & OF))) >>> 0, "SETO observes sign change");
        equal(result.words[3], (0x11223300 | Number(Boolean(reference.flags & CF))) >>> 0, "SETB observes shifted-out low bit");
      }
    }));
  for (const source of [1, 2]) {
    await test(`SHRD CL unaligned memory source${source} aliases preserve address/count inputs`, () => fixture(
      [0x0f, 0xad, source << 3 | 2], async ({ reset, state, bytes, run }) => {
        for (let count = 0; count < 256; count++) {
          setWord(bytes(DATA), 3, 0x80000001); reset({ edx: DATA + 3, ecx: (0x87654300 | count) >>> 0, flags: 0xad7 });
          seedShrdState(state()); const before = Array.from(state()), reference = shrdReference(0x80000001, before[source], count);
          checkShrdState(run(1), before, -1, reference, CODE + 3);
          equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(3, true), reference.result, "exact unaligned memory result");
        }
      }));
  }
  for (const count of [0, 1, 31, 32]) for (const protection of [null, 0, READ, WRITE]) {
    await test(`SHRD count${count} complete RMW permissions${protection} precede flags and partial writes`, () => fixture(
      [0x83, 0x07, 1, 0x0f, 0xac, 0x13, count], async ({ reset, state, map, protect, bytes, compile, run }) => {
        setWord(bytes(STACK), 0, 0x7fffffff); bytes(DATA).fill(0x55); bytes(DATA).set([0xaa, 0xbb], 4094);
        if (protection !== null) map(DATA + 4096, protection, page([0x42, 0x31], 0x66));
        reset({ ebx: DATA + 4094, edx: 0x87654321, edi: STACK, flags: 0x203 }); seedShrdState(state());
        const before = Array.from(state()), prefix = arithmetic("add", 32, 0x7fffffff, 1, 0);
        before[9] = ((before[9] & ~ARITH) | prefix.flags) >>> 0;
        const fault = run(100); equal(fault.kind, 4, "whole RMW span faults even for zero count");
        checkShrdState(fault, before, -1, null, CODE + 3, 1);
        equal(fault.address, DATA + 4096, "first inaccessible destination byte");
        equal(fault.error, protection === null ? 6 : 7, "RMW reports a user write fault");
        equal(bytes(DATA)[4094], 0xaa, "first destination byte not partially written"); equal(bytes(DATA)[4095], 0xbb, "second destination byte not partially written");
        equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "prior ADD committed exactly once");
        if (protection === null) map(DATA + 4096, READ | WRITE, page([0x42, 0x31], 0x66)); else protect(DATA + 4096, READ | WRITE);
        await compile(CODE + 3); const repairedBefore = Array.from(state()), reference = shrdReference(0x3142bbaa, before[2], count);
        checkShrdState(run(1), repairedBefore, -1, reference, CODE + 7);
        const output = new Uint8Array([bytes(DATA)[4094], bytes(DATA)[4095], bytes(DATA + 4096)[0], bytes(DATA + 4096)[1]]);
        equal(new DataView(output.buffer).getUint32(0, true), reference.result, "repaired cross-page result");
        equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "repair never replays prior ADD");
      }));
  }
  for (const count of [0, 1, 32]) {
    await test(`SHRD count${count} checks dirty capacity only for an actual write`, () => fixture(
      [0x0f, 0xac, 0x13, count], async ({ handle, reset, state, bytes, run }) => {
        setWord(bytes(DATA), 0, 0x80000001); reset({ ebx: DATA, edx: 0x12345678, flags: 0xad7 }); seedShrdState(state());
        const before = Array.from(state()), header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4), capacity = header()[2];
        header()[2] = 0;
        const result = run(1, () => { equal(header()[1], 0, "no partial dirty journal publication"); header()[2] = capacity; });
        if (count & 31) { equal(result.kind, 10, "actual store rejects exhausted journal before mutation"); checkShrdState(result, before, -1, null, CODE, 0); }
        else checkShrdState(result, before, -1, shrdReference(0x80000001, before[2], count), CODE + 4);
        equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true), 0x80000001, "suppressed or failed write leaves bytes");
      }));
  }
  await test("SHRD FS uses the effective offset and validates its entire segment span", () => fixture(
    [0x64, 0x0f, 0xac, 0x13, 1], async ({ reset, state, bytes, run }) => {
      setWord(bytes(DATA), 3, 0x80000001); reset({ ebx: 3, edx: 1, fs: DATA, flags: 0xad7 }); seedShrdState(state());
      const before = Array.from(state()); checkShrdState(run(1), before, -1, shrdReference(0x80000001, 1, 1), CODE + 5);
      equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(3, true), 0xc0000000, "FS-based unaligned RMW");
      reset({ ebx: 0xfffffffe, edx: 1, fs: DATA + 2, flags: 0xad7 }); seedShrdState(state());
      const invalidBefore = Array.from(state()), dataBefore = bytes(DATA).slice(), fault = run(1);
      equal(fault.kind, 6, "FS offset span cannot wrap behind a valid linear address"); checkShrdState(fault, invalidBefore, -1, null, CODE, 0);
      for (let i = 0; i < 4096; i++) equal(bytes(DATA)[i], dataBefore[i], "FS fault preserves full page");
    }));
  await test("SHRD stack-segment wrap faults before memory and flags change", () => fixture(
    [0x0f, 0xac, 0x55, 0, 1], async ({ reset, state, map, bytes, run }) => {
      map(0xfffff000, READ | WRITE, page([], 0x77)); map(0, READ | WRITE, page([], 0x88));
      reset({ ebp: 0xfffffffe, edx: 1, flags: 0xad7 }); seedShrdState(state()); const before = Array.from(state()), fault = run(1);
      equal(fault.kind, 9, "SS operand full-span wrap is stack fault"); checkShrdState(fault, before, -1, null, CODE, 0);
      equal(bytes(0xfffff000)[4095], 0x77, "last byte remains unchanged"); equal(bytes(0)[0], 0x88, "first byte remains unchanged");
    }));
  for (const count of [0, 1]) await test(`SHRD code alias count${count} publishes SMC only after a real store`, () => fixture(
    [0x0f, 0xac, 0x13, count, 0x40], async ({ handle, alias, reset, bytes, run }) => {
      alias(0x4000, CODE, READ | WRITE); setWord(bytes(CODE), 0x100, 0x80000001);
      reset({ ebx: 0x4100, edx: 1, eax: 17, flags: 0xad7 }); const result = run(count ? 2 : 1);
      equal(result.retired, 1, "only SHRD completes before budget or SMC"); equal(result.words[0], 17, "following INC has not executed");
      equal(result.pc, CODE + 4, "next instruction remains restart point");
      equal(new DataView(bytes(CODE).buffer, bytes(CODE).byteOffset).getUint32(0x100, true), count ? 0xc0000000 : 0x80000001, "code alias receives exact result");
      if (count) { equal(result.kind, 7, "real code write exits before stale continuation"); check(api.backend_validate(handle, CODE) !== 0, "old executable stamp is invalidated"); }
      else { check(result.kind !== 7, "zero count does not manufacture SMC"); ok(api.backend_validate(handle, CODE), "no-store keeps old compiled stamp valid"); }
    }, READ | WRITE | EXECUTE | SEALED));
  for (const [name, code, kind] of [["LOCK register", [0xf0, 0x0f, 0xac, 0xd0, 1], 8], ["LOCK memory", [0xf0, 0x0f, 0xac, 0x13, 0], 8],
    ["address16", [0x67, 0x0f, 0xac, 0x10, 1], 3], ["operand16", [0x66, 0x0f, 0xac, 0xd0, 1], 3]]) {
    await test(`SHRD ${name} remains an explicit unsupported/invalid boundary`, () => fixture(code, async ({ reset, state, run }) => {
      reset({ ebx: 0x5000, flags: 0xad7 }); seedShrdState(state()); const before = Array.from(state()), result = run(1);
      equal(result.kind, kind, "decode rejection precedes any memory or flag effect"); checkShrdState(result, before, -1, null, CODE, 0);
    }));
  }
  const scalarRmwOperations = ["add", "adc", "sub", "sbb", "and", "or", "xor", "inc", "dec", "neg", "not", "shl", "shr", "sar"];
  const scalarRmwCode = (operation, width, displacement = null) => {
    const prefix = width === 16 ? [0x66] : [], wide = Number(width !== 8), mode = displacement === null ? 0 : 0x40;
    const binary = { add: 0x00, adc: 0x10, sub: 0x28, sbb: 0x18, and: 0x20, or: 0x08, xor: 0x30, cmp: 0x38, test: 0x84 };
    const group = { inc: [0xfe + wide, 0], dec: [0xfe + wide, 1], neg: [0xf6 + wide, 3], not: [0xf6 + wide, 2],
      shl: [0xd2 + wide, 4], shr: [0xd2 + wide, 5], sar: [0xd2 + wide, 7] };
    const [opcode, extension] = operation in binary ? [binary[operation] + wide, 2] : group[operation];
    return [...prefix, opcode, mode | extension << 3 | 3, ...(displacement === null ? [] : [displacement])];
  };
  const scalarRmwReference = (operation, width, destination, source, count, initialFlags) => {
    const mask = widthMask(width), a = BigInt(destination) & mask, b = BigInt(source) & mask;
    if (["shl", "shr", "sar"].includes(operation)) {
      const n = count & 31;
      if (!n) return { result: Number(a), flags: initialFlags, preserve: true };
      const result = operation === "shl" ? a << BigInt(n) & mask : operation === "shr" ? a >> BigInt(n) : signed(a, width) >> BigInt(n) & mask;
      let defined = PF | ZF | SF, flags = common(result, width), carry = 0;
      if (operation === "sar" || n < width) {
        defined |= CF;
        carry = Number(operation === "shl" ? a >> BigInt(width - n) & 1n
          : operation === "sar" ? signed(a, width) >> BigInt(n - 1) & 1n : a >> BigInt(n - 1) & 1n);
        flags |= carry;
      }
      if (n === 1) {
        defined |= OF;
        if (operation === "shr" && (a >> BigInt(width - 1))) flags |= OF;
        if (operation === "shl" && (Number(result >> BigInt(width - 1)) ^ carry)) flags |= OF;
      }
      return { result: Number(result), flags, defined };
    }
    if (operation === "not") return { result: Number(a ^ mask), flags: initialFlags, preserve: true };
    if (["and", "or", "xor", "test"].includes(operation)) {
      const result = operation === "or" ? a | b : operation === "xor" ? a ^ b : a & b;
      return { result: Number(result), flags: common(result, width), defined: ARITH & ~AF };
    }
    const reference = arithmetic(operation === "inc" ? "add" : operation === "dec" || operation === "neg" ? "sub" : operation,
      width, operation === "neg" ? 0 : Number(a), operation === "inc" || operation === "dec" ? 1 : operation === "neg" ? Number(a) : source, initialFlags & CF);
    if (operation === "inc" || operation === "dec") reference.flags = reference.flags & ~CF | initialFlags & CF;
    return { ...reference, defined: ARITH };
  };
  const checkScalarRmwState = (result, before, reference, nextPc, retired) => {
    equal(result.retired, retired, "scalar RMW precise retirement"); equal(result.pc, nextPc, "scalar RMW restart PC");
    for (let n = 0; n < 100; n++) if (![8, 9, 10].includes(n)) equal(result.words[n], before[n], `scalar RMW unchanged architectural word ${n}`);
    if (!reference || reference.preserve) {
      equal(result.words[9], before[9], "fault, NOT or zero shift retains all flags"); equal(result.words[10], before[10], "unchanged flag validity");
    } else {
      equal(result.words[9] & reference.defined, reference.flags, "defined scalar RMW flags");
      equal(result.words[9] & ~ARITH, before[9] & ~ARITH, "nonarithmetic flags retained");
      equal(result.words[10], ((before[10] & ~ARITH) | reference.defined) >>> 0, "scalar RMW defined flags");
    }
  };
  for (const operation of scalarRmwOperations) for (const width of [8, 16, 32]) {
    for (const count of (["shl", "shr", "sar"].includes(operation) ? [0, 1, 32] : [1])) {
      const code = scalarRmwCode(operation, width), size = width / 8;
      await test(`${operation} m${width} count${count} rejects write-only span before effects and resumes exactly`, () => fixture(
        [0x83, 0x07, 1, ...code], async ({ reset, state, map, protect, bytes, compile, run }) => {
          const address = DATA + (size === 1 ? 4096 : 4095), value = Number(0x7f815aa5n & widthMask(width));
          const first = page([], 0x55), second = page([], 0x66), input = new Uint8Array([...first, ...second]);
          for (let n = 0; n < size; n++) input[address - DATA + n] = value >>> (n * 8) & 255;
          map(DATA, READ | WRITE, input.subarray(0, 4096)); map(DATA + 4096, WRITE, input.subarray(4096));
          setWord(bytes(STACK), 0, 0xffffffff); reset({ ebx: address, edx: 0x87654321, ecx: count, edi: STACK, flags: 0xad7 });
          seedShrdState(state()); const before = Array.from(state()), prefix = arithmetic("add", 32, 0xffffffff, 1, 0);
          before[9] = ((before[9] & ~ARITH) | prefix.flags) >>> 0;
          const fault = run(100); equal(fault.kind, 4, "RMW needs read and write even for zero shift");
          checkScalarRmwState(fault, before, null, CODE + 3, 1); equal(fault.address, DATA + 4096, "first inaccessible RMW byte");
          equal(fault.error, 7, "missing read permission on RMW still reports write PF");
          equal(Array.from(bytes(DATA)).join(), Array.from(input.subarray(0, 4096)).join(), "no partial first-page write");
          equal(Array.from(bytes(DATA + 4096)).join(), Array.from(input.subarray(4096)).join(), "no partial second-page write");
          equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0, "prior ADD committed once");
          protect(DATA + 4096, READ | WRITE); await compile(CODE + 3);
          const resumeBefore = Array.from(state()), reference = scalarRmwReference(operation, width, value, before[2], count, before[9]);
          checkScalarRmwState(run(1), resumeBefore, reference, CODE + 3 + code.length, 1);
          for (let n = 0; n < size; n++) equal(bytes((address + n) & ~4095)[(address + n) & 4095], reference.result >>> (n * 8) & 255, "repaired scalar result byte");
          equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0, "repair never replays the prefix ADD");
        }));
    }
  }
  for (const operation of ["cmp", "test"]) for (const width of [8, 16, 32]) {
    await test(`${operation} m${width} remains a read access and needs no dirty capacity`, () => fixture(
      scalarRmwCode(operation, width), async ({ handle, reset, state, map, protect, bytes, run }) => {
        map(DATA, READ | WRITE, page(wordBytes(0x7f815aa5))); const image = bytes(DATA).slice();
        for (const permission of [READ, 0, WRITE]) {
          protect(DATA, permission); reset({ ebx: DATA, edx: 0x87654321, flags: 0xad7 }); seedShrdState(state());
          const before = Array.from(state()), header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4), capacity = header()[2]; header()[2] = 0;
          const result = run(1, () => { equal(header()[1], 0, "comparison publishes no dirty entries"); header()[2] = capacity; });
          if (permission === READ) checkScalarRmwState(result, before, scalarRmwReference(operation, width, 0x7f815aa5, before[2], 0, before[9]), CODE + scalarRmwCode(operation, width).length, 1);
          else { equal(result.kind, 4, "comparison source faults"); equal(result.error, 5, "comparison PF retains read access"); checkScalarRmwState(result, before, null, CODE, 0); }
          equal(Array.from(bytes(DATA)).join(), Array.from(image).join(), "comparison leaves every source byte unchanged");
        }
      }));
  }
  for (const width of [8, 16, 32]) {
    const code = [...(width === 16 ? [0x66] : []), width === 8 ? 0x02 : 0x03, 0x03];
    await test(`ADD register${width} with a memory source retains read-only permission semantics`, () => fixture(code,
      async ({ handle, reset, state, map, protect, bytes, run }) => {
        map(DATA, READ | WRITE, page(wordBytes(0x7f815aa5))); const image = bytes(DATA).slice();
        for (const permission of [READ, 0, WRITE]) {
          protect(DATA, permission); reset({ eax: 0xa5a5ffff, ebx: DATA, flags: 0xad7 }); seedShrdState(state()); const before = Array.from(state());
          const header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4), capacity = header()[2]; header()[2] = 0;
          const result = run(1, () => { equal(header()[1], 0, "read source publishes no dirty entries"); header()[2] = capacity; });
          if (permission === READ) {
            const reference = { ...arithmetic("add", width, before[0], 0x7f815aa5, 0), defined: ARITH };
            before[0] = retainedResult(before[0], reference.result, width); checkScalarRmwState(result, before, reference, CODE + code.length, 1);
          } else { equal(result.kind, 4, "register destination source faults"); equal(result.error, 5, "source PF is read"); checkScalarRmwState(result, before, null, CODE, 0); }
          equal(Array.from(bytes(DATA)).join(), Array.from(image).join(), "register arithmetic does not change source bytes");
        }
      }));
  }
  for (const operation of ["shl", "shr", "sar"]) for (const width of [8, 16, 32]) {
    await test(`${operation} m${width} zero counts check permission without dirty capacity or SMC`, () => fixture(
      scalarRmwCode(operation, width), async ({ handle, reset, state, alias, bytes, protect, run }) => {
        alias(0x4000, CODE, READ | WRITE); setWord(bytes(CODE), 0x100, 0x7f815aa5);
        for (const count of [0, 32, 1]) {
          reset({ ebx: 0x4100, ecx: count, flags: 0xad7 }); seedShrdState(state()); const before = Array.from(state());
          const header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4), capacity = header()[2]; header()[2] = 0;
          const result = run(1, () => { equal(header()[1], 0, "suppressed or rejected store does not publish dirty journal"); header()[2] = capacity; });
          if (count & 31) { equal(result.kind, 10, "actual shift fails capacity before flags/store"); checkScalarRmwState(result, before, null, CODE, 0); }
          else { check(result.kind !== 7, "zero shift does not invalidate code alias"); checkScalarRmwState(result, before, { preserve: true }, CODE + scalarRmwCode(operation, width).length, 1); }
          equal(new DataView(bytes(CODE).buffer, bytes(CODE).byteOffset).getUint32(0x100, true), 0x7f815aa5, "no partial alias store"); ok(api.backend_validate(handle, CODE), "code stamp remains current");
        }
        protect(0x4000, READ); reset({ ebx: 0x4100, ecx: 0, flags: 0xad7 }); seedShrdState(state()); const before = Array.from(state()), fault = run(1);
        equal(fault.kind, 4, "zero shift still checks write protection"); equal(fault.error, 7, "zero shift write PF"); checkScalarRmwState(fault, before, null, CODE, 0);
      }, READ | WRITE | EXECUTE | SEALED));
  }
  for (const count of [0, 1]) await test(`affine read proof cannot satisfy later RMW count${count} write permission`, () => fixture(
    [0x8b, 0x03, 0xd3, 0x63, 4], async ({ reset, state, bytes, protect, run, affineRanges }) => {
      if (options.affineMemory) check(affineRanges() > 0, "fixture exercises a shared memory proof");
      setWord(bytes(DATA), 0, 77); setWord(bytes(DATA), 4, 0x80000001); protect(DATA, READ);
      reset({ ebx: DATA, ecx: count, flags: 0xad7 }); seedShrdState(state()); const before = Array.from(state()); before[0] = 77;
      const fault = run(2); equal(fault.kind, 4, "read proof miss falls back to exact RMW fault"); equal(fault.error, 7, "RMW reports write PF after successful read");
      equal(fault.address, DATA + 4, "precise second operand"); checkScalarRmwState(fault, before, null, CODE + 2, 1);
      equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(4, true), 0x80000001, "RMW destination unchanged");
      protect(DATA, READ | WRITE); reset({ ebx: DATA, ecx: count, flags: 0xad7 }); seedShrdState(state()); const admitted = Array.from(state()); admitted[0] = 77;
      const reference = scalarRmwReference("shl", 32, 0x80000001, 0, count, admitted[9]);
      checkScalarRmwState(run(2), admitted, reference, CODE + 5, 2);
      equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(4, true), reference.result, "certified RMW writes the exact result");
    }));
  for (const [operation, modrm] of [["shl", 0xe0], ["shr", 0xe8], ["sar", 0xf8]]) {
    for (const width of [8, 16, 32]) {
      await test(`${operation} ${width}-bit counts and defined flags`, () => fixture(
        [...(width === 16 ? [0x66] : []), width === 8 ? 0xd2 : 0xd3, modrm], async ({ reset, run }) => {
          const mask = widthMask(width);
          for (const av of [0n, 1n, mask, 1n << BigInt(width - 1), 0x51n]) {
            for (const rawCount of [0, 1, width - 1, width, width + 1, 31, 32, 33]) {
              const count = rawCount & 31, a = av & mask;
              const expected = count === 0 ? a : operation === "shl" ? a << BigInt(count) & mask
                : operation === "shr" ? a >> BigInt(count) : signed(a, width) >> BigInt(count) & mask;
              const initialFlags = 0x202 | ARITH;
              reset({ eax: Number(a), ecx: rawCount, flags: initialFlags });
              const result = run(1);
              equal(result.words[0], Number(expected), "shift result");
              if (count === 0) {
                equal(result.words[9], initialFlags, "count zero preserves all flags");
                equal(result.words[10], 0xffffffff, "count zero preserves flag validity");
              } else {
                let valid = PF | ZF | SF, flags = common(expected, width), carry = 0;
                if (operation === "sar" || count < width) {
                  valid |= CF;
                  carry = Number(operation === "shl" ? a >> BigInt(width - count) & 1n
                    : operation === "sar" ? signed(a, width) >> BigInt(count - 1) & 1n : a >> BigInt(count - 1) & 1n);
                  flags |= carry;
                }
                if (count === 1) {
                  valid |= OF;
                  if (operation === "shr" && (a >> BigInt(width - 1))) flags |= OF;
                  if (operation === "shl" && (Number(expected >> BigInt(width - 1)) ^ carry)) flags |= OF;
                }
                equal(result.words[9] & valid, flags, "defined shift flags");
                equal(result.words[10] & valid, valid, "architecturally defined shift mask");
              }
            }
          }
        }));
    }
  }
  await test("high byte destination, carry preservation, and full-budget flag optimization", () => fixture(
    [0x00, 0xdc, 0x40, 0x83, 0xc0, 1, 0x83, 0xe8, 2], async ({ reset, run }) => {
      reset({ eax: 0x1234ff56, ebx: 1, flags: 0x202 });
      const first = run(1);
      equal(first.words[0], 0x12340056, "AH writes preserve AL and upper half");
      equal(first.words[9] & CF, CF, "AH carry");
      reset({ eax: 0x1234ff56, ebx: 1, flags: 0x202 });
      const all = run(100);
      equal(all.kind, 3, "unsupported sentinel remains explicit");
      equal(all.retired, 4, "all arithmetic retired");
      equal(all.words[0], 0x12340056, "full-block optimized result");
      equal(all.words[9] & ARITH, arithmetic("sub", 32, 0x12340058, 2, 0).flags, "last flags are materialized");
    }));
  for (const row of [
    { code: [0x94], width: 32, left: 0, right: 4, label: "short EAX ESP" },
    { code: [0x66, 0x94], width: 16, left: 0, right: 4, label: "short AX SP" },
    { code: [0x87, 0xcb], width: 32, left: 3, right: 1, label: "ModRM EBX ECX" },
    { code: [0x66, 0x87, 0xcb], width: 16, left: 3, right: 1, label: "ModRM BX CX" },
    { code: [0x86, 0xf8], width: 8, left: 0, right: 3, rightHigh: true, label: "AL BH" },
    { code: [0x86, 0xe0], width: 8, left: 0, right: 0, rightHigh: true, label: "overlapping AL AH" },
    { code: [0x86, 0xc4], width: 8, left: 0, right: 0, leftHigh: true, label: "overlapping AH AL" },
    { code: [0x86, 0xe4], width: 8, left: 0, right: 0, leftHigh: true, rightHigh: true, label: "same AH" },
    { code: [0x87, 0xc9], width: 32, left: 1, right: 1, label: "same ECX" },
  ]) {
    await test(`XCHG register ${row.label} preserves flags and exact partial bits`, () => fixture(row.code,
      async ({ reset, run, state }) => {
        const initial = [0xa1b2c3d4, 0x10203040, 0x55667788, 0x9988aabb, 0x88776655, 0x12345678, 0x89abcdef, 0xfedcba98];
        const flags = 0xad7, validity = 0xfffffffb;
        const resetCase = () => { reset({ flags }); state().set(initial); state()[10] = validity; };
        resetCase();
        const idle = run(0);
        equal(idle.retired, 0, "zero budget does not exchange"); equal(idle.pc, CODE, "zero budget PC");
        for (let n = 0; n < 8; n++) equal(idle.words[n], initial[n], "zero budget register unchanged");
        for (const budget of [1, 100]) {
          resetCase();
          const expected = initial.slice(), mask = row.width === 32 ? 0xffffffff : (1 << row.width) - 1;
          const leftShift = row.leftHigh ? 8 : 0, rightShift = row.rightHigh ? 8 : 0;
          const left = (initial[row.left] >>> leftShift) & mask, right = (initial[row.right] >>> rightShift) & mask;
          const put = (index, shift, value) => { expected[index] = ((expected[index] & ~(mask << shift)) | ((value & mask) << shift)) >>> 0; };
          put(row.left, leftShift, right); put(row.right, rightShift, left);
          const result = run(budget);
          equal(result.retired, 1, "exchange retires once"); equal(result.pc, CODE + row.code.length, "exchange continuation");
          for (let n = 0; n < 8; n++) equal(result.words[n], expected[n], "architectural register exchange");
          equal(result.words[9], flags, "all EFLAGS preserved"); equal(result.words[10], validity, "flag validity preserved");
        }
      }));
  }
  for (const operation of ["push", "ret"]) {
    await test(`XCHG ESP commits before following ${operation.toUpperCase()} fault and resumes without replay`, () => fixture(
      [0xff, 0x05, ...wordBytes(DATA), 0x94, operation === "push" ? 0x50 : 0xc3],
      async ({ reset, run, map, bytes, compile }) => {
        reset({ eax: 0x5000, esp: STACK + 0x800 });
        const first = run(100);
        equal(first.kind, 4, "following stack access faults"); equal(first.retired, 2, "prior store and exchange completed");
        equal(first.pc, CODE + 7, "fault points to stack operation"); equal(first.faultPc, CODE + 7, "precise stack fault PC");
        equal(first.words[0], STACK + 0x800, "exchange old ESP remains in EAX"); equal(first.words[4], 0x5000, "new ESP survives later fault");
        equal(first.address, operation === "push" ? 0x4ffc : 0x5000, "new stack address used");
        equal(first.words[9] & (ARITH & ~CF), arithmetic("add", 32, 0, 1, 0).flags & ~CF, "earlier flags survive exchange and fault");
        equal(bytes(DATA)[0], 1, "prefix store completed once");
        map(operation === "push" ? 0x4000 : 0x5000, READ | WRITE);
        if (operation === "ret") setWord(bytes(0x5000), 0, CODE + 0x80);
        await compile(first.pc);
        const resumed = run(1);
        equal(resumed.retired, 1, "resume completes only stack operation");
        equal(resumed.words[0], STACK + 0x800, "exchange was not replayed");
        equal(resumed.words[4], operation === "push" ? 0x4ffc : 0x5004, "stack operation commits once");
        if (operation === "push") equal(new DataView(bytes(0x4000).buffer, bytes(0x4000).byteOffset).getUint32(4092, true), STACK + 0x800, "push uses swapped EAX");
        else equal(resumed.pc, CODE + 0x80, "return reads guest return address");
        equal(bytes(DATA)[0], 1, "prefix store is not replayed on resume");
      }));
  }
  for (const exchange of [[0x86, 0x03], [0x66, 0x87, 0x03], [0x87, 0x03], [0xf0, 0x87, 0x03]]) {
    await test(`memory XCHG ${exchange.join("-")} stops explicitly after prior committed effects`, () => fixture(
      [0xff, 0x05, ...wordBytes(DATA), ...exchange], async ({ reset, run, bytes, compile }) => {
        setWord(bytes(DATA), 4, 0x11223344); reset({ eax: 0xaabbccdd, ebx: DATA + 4, flags: 0x203 });
        const first = run(100);
        equal(first.kind, 3, "implicit/explicit locked memory form is unsupported"); equal(first.retired, 1, "only prefix store retired");
        equal(first.pc, CODE + 6, "memory exchange exact PC"); equal(first.words[0], 0xaabbccdd, "memory exchange did not write register");
        equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(4, true), 0x11223344, "memory exchange did not write bytes");
        equal(first.words[9] & ARITH, (arithmetic("add", 32, 0, 1, 0).flags & ~CF) | CF, "prefix flags preserved");
        await compile(first.pc);
        const retry = run(100); equal(retry.retired, 0, "unsupported retry has no retirement");
        equal(bytes(DATA)[0], 1, "unsupported retry did not replay earlier store");
      }));
  }
  await test("XCHG invalidates certificates for both register destinations", () => fixture(
    [0x8b, 0x10, 0x8b, 0x50, 4, 0x8b, 0x33, 0x8b, 0x73, 4, 0x93, 0x8b, 0x08, 0x8b, 0x7b, 4],
    async ({ reset, run, bytes }) => {
      setWord(bytes(DATA), 0, 11); setWord(bytes(DATA), 4, 22);
      setWord(bytes(STACK), 0, 33); setWord(bytes(STACK), 4, 44);
      reset({ eax: DATA, ebx: STACK }); const result = run(7);
      equal(result.retired, 7, "both base certificates exercised around exchange");
      equal(result.words[0], STACK, "first base exchanged"); equal(result.words[3], DATA, "second base exchanged");
      equal(result.words[1], 33, "first exchanged base rechecked"); equal(result.words[7], 22, "second exchanged base rechecked");
    }));
  await test("LOCK register XCHG raises invalid opcode without exchange", () => fixture(
    [0x40, 0xf0, 0x87, 0xc3], async ({ reset, run }) => {
      reset({ eax: 7, ebx: 11 }); const result = run(100);
      equal(result.kind, 8, "LOCK with register destination is #UD"); equal(result.pc, CODE + 1, "invalid instruction PC");
      equal(result.retired, 1, "only previous INC completed");
      equal(result.words[0], 8, "prior INC retained and EAX not exchanged"); equal(result.words[3], 11, "EBX not exchanged");
    }));
  await test("XCHG high-byte base mutation invalidates its earlier certificate", () => fixture(
    [0x8b, 0x10, 0x8b, 0x50, 4, 0x86, 0xcc, 0x8b, 0x18], async ({ reset, run, bytes }) => {
      setWord(bytes(DATA), 0, 11); setWord(bytes(DATA), 4, 22); setWord(bytes(STACK), 0, 33);
      reset({ eax: DATA, ecx: 0x80 }); const result = run(4);
      equal(result.words[0], STACK, "AH receives original CL"); equal(result.words[1], 0x20, "CL receives original AH");
      equal(result.words[3], 33, "partial base write cannot reuse old translation");
    }));
  // Compatibility results recorded from the immutable product CPU pin. These
  // deliberately include its unknown-leaf/max-leaf quirks. Backend instruction
  // coverage must not change feature bits or guest dispatch decisions.
  const cpuidCases = [
    [0, 0, [0x16, 0x756e6547, 0x6c65746e, 0x49656e69]],
    [0, 0xffffffff, [0x16, 0x756e6547, 0x6c65746e, 0x49656e69]],
    [1, 0, [0x673, 0x10800, 0xc0800001, 0x0780a979]],
    [1, 0xffffffff, [0x673, 0x10800, 0xc0800001, 0x0780a979]],
    [2, 0, [0x665b5001, 0, 0, 0x007a7000]],
    [3, 0, [0, 0, 0, 0]],
    [4, 0, [0x121, 0x01c0003f, 0x3f, 1]],
    [4, 1, [0x122, 0x01c0003f, 0x3f, 1]],
    [4, 2, [0x143, 0x05c0003f, 0xfff, 1]],
    [4, 3, [0, 0, 0, 0]],
    [4, 0xffffffff, [0, 0, 0, 0]],
    [5, 0xffffffff, [0x40, 0x40, 3, 0x00142120]],
    [7, 0, [0, 0x200, 0, 0]],
    [7, 1, [0, 0, 0, 0]],
    [0xb, 0, [0, 0, 0, 0]],
    [0x15, 0xffffffff, [1, 1, 1_000_000_000, 0]],
    [0x16, 0, [1000, 1000, 10, 0]],
    [0x17, 0, [0, 0, 0, 0]],
    [0x40000000, 0, [0, 0x61774d56, 0x4d566572, 0x65726177]],
    [0x80000000, 0, [5, 0, 0, 0]],
    [0x80000001, 0, [0, 0, 0, 0]],
    [0xffffffff, 0xffffffff, [0, 0, 0, 0]],
  ];
  for (const prefix of [[], [0x66]]) {
    await test(`CPUID ${prefix.length ? "operand-size prefix" : "default"} fixed profile and exact architectural preservation`,
      () => fixture([...prefix, 0x0f, 0xa2], async ({ reset, run, state, bytes }) => {
        bytes(DATA).fill(0x5a);
        for (const [leaf, subleaf, expected] of cpuidCases) for (const budget of [0, 1, 100]) {
          reset({ eax: leaf, ecx: subleaf, edx: 0x89abcdef, ebx: 0x76543210,
            esp: STACK + 0x10, ebp: 0x11223344, esi: 0x55667788, edi: 0x99aabbcc, flags: 0xed7 });
          state()[10] = 0xfffffffb;
          const before = Array.from(state());
          const result = run(budget);
          equal(result.retired, budget ? 1 : 0, "CPUID retirement obeys guest budget");
          equal(result.pc, CODE + (budget ? prefix.length + 2 : 0), "CPUID exact continuation");
          for (const [index, reg] of [0, 3, 1, 2].entries()) {
            equal(result.words[reg], budget ? expected[index] : before[reg], "CPUID output and subleaf snapshot");
          }
          for (let n = 4; n < 100; n++) {
            if (n !== 8) equal(result.words[n], before[n], "CPUID preserves other CPU state including all flags");
          }
        }
        check(bytes(DATA).every(value => value === 0x5a), "CPUID does not mutate guest bytes");
      }));
  }
  for (const base of [0, 1, 2, 3]) {
    await test(`CPUID invalidates affine memory certificate for GPR ${base}`, () => fixture(
      [0x8b, 0x30 | base, 0x8b, 0x78 | base, 4, 0x0f, 0xa2, 0x8b, 0x28 | base],
      async ({ reset, run, bytes, affineRanges }) => {
        setWord(bytes(DATA), 0, 11); setWord(bytes(DATA), 4, 22);
        reset({ eax: base === 0 ? DATA : 0xdead, ecx: base === 1 ? DATA : 7,
          edx: base === 2 ? DATA : 0x33333333, ebx: base === 3 ? DATA : 0x44444444, ebp: 0x55555555 });
        if (options.affineMemory) equal(affineRanges(), 1, "candidate certificate really emitted");
        const result = run(100);
        equal(result.kind, 4, "unknown leaf zero output yields unmapped pointer fault");
        equal(result.retired, 3, "both reads and CPUID completed before fault");
        equal(result.pc, CODE + 7, "fault after CPUID has exact location"); equal(result.address, 0, "new base rechecked");
        for (let n = 0; n < 4; n++) equal(result.words[n], 0, "all four CPUID destinations defined");
        equal(result.words[5], 0x55555555, "fault does not overwrite destination");
        equal(result.words[6], 11, "first read preserved"); equal(result.words[7], 22, "second read preserved");
      }));
  }
  await test("CPUID preserves prior effects and precise following fault across restart", () => fixture(
    [0xff, 0x05, ...wordBytes(DATA), 0x0f, 0xa2, 0x8b, 0x7d, 0],
    async ({ reset, run, map, bytes, compile }) => {
      reset({ eax: 0, ecx: 0xdeadbeef, ebp: 0x5000, flags: 0x203 });
      const result = run(100);
      equal(result.kind, 4, "load after CPUID faults"); equal(result.pc, CODE + 8, "precise following fault");
      equal(result.retired, 2, "store and CPUID completed"); equal(bytes(DATA)[0], 1, "earlier store visible");
      equal(result.words[0], 0x16, "CPUID committed before later fault");
      equal(result.words[9] & ARITH, (arithmetic("add", 32, 0, 1, 0).flags & ~CF) | CF, "CPUID preserves prior flags at fault");
      map(0x5000, READ | WRITE); setWord(bytes(0x5000), 0, 0xabcdef01);
      await compile(result.pc); const resumed = run(1);
      equal(resumed.retired, 1, "only faulting load resumes"); equal(resumed.words[7], 0xabcdef01, "resumed load result");
      equal(resumed.words[0], 0x16, "CPUID was not rerun using its own output leaf");
      equal(bytes(DATA)[0], 1, "prefix write not duplicated");
    }));
  await test("LOCK CPUID is invalid and does not publish outputs after prior write", () => fixture(
    [0xff, 0x05, ...wordBytes(DATA), 0xf0, 0x0f, 0xa2], async ({ reset, run, bytes }) => {
      reset({ eax: 0, ecx: 2, edx: 3, ebx: 4 }); const result = run(100);
      equal(result.kind, 8, "LOCK CPUID raises invalid opcode"); equal(result.pc, CODE + 6, "invalid CPUID exact PC");
      equal(result.retired, 1, "only earlier store completed"); equal(bytes(DATA)[0], 1, "earlier bytes retained");
      for (const [reg, expected] of [0, 2, 3, 4].entries()) equal(result.words[reg], expected, "invalid CPUID outputs unchanged");
    }));
  await test("memory fault observes earlier arithmetic flags and exact continuation", () => fixture(
    [0x83, 0xc0, 1, 0x8b, 0x03], async ({ reset, run }) => {
      reset({ eax: 0x7fffffff, ebx: 0x5000, flags: 0x202 });
      const result = run(100);
      equal(result.kind, 4, "guest page fault");
      equal(result.pc, CODE + 3, "fault instruction PC");
      equal(result.faultPc, CODE + 3, "published fault PC");
      equal(result.address, 0x5000, "fault address");
      equal(result.error, 4, "user read of unmapped page");
      equal(result.retired, 1, "only prior arithmetic completed");
      equal(result.words[0], 0x80000000, "prior register side effect retained");
      equal(result.words[9] & ARITH, arithmetic("add", 32, 0x7fffffff, 1, 0).flags, "prior flags visible at fault");
    }));
  await test("cross-page write validates whole access and resumes without replay", () => fixture(
    [0xff, 0x05, ...wordBytes(DATA), 0xa3, ...wordBytes(DATA + 4094)], async ({ map, bytes, reset, run, protect, compile }) => {
      map(DATA, READ | WRITE, page([], 0x11));
      map(DATA + 4096, READ, page([], 0x22));
      const initial = new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset, 4096).getUint32(0, true);
      reset({ eax: 0xaabbccdd });
      const result = run(100);
      equal(result.kind, 4, "write protection fault");
      equal(result.pc, CODE + 6, "fault after first completed store");
      equal(result.address, DATA + 4096, "first forbidden byte is second page");
      equal(result.error, 7, "present user write protection code");
      equal(result.retired, 1, "first store retired exactly once");
      equal(bytes(DATA)[4094], 0x11, "first part of failed store unchanged");
      equal(bytes(DATA + 4096)[0], 0x22, "second part of failed store unchanged");
      equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset, 4096).getUint32(0, true), initial + 1, "prior store committed");
      protect(DATA + 4096, READ | WRITE);
      await compile(result.pc);
      const resumed = run(1);
      equal(resumed.retired, 1, "resume completes only failed instruction");
      equal(bytes(DATA)[4094], 0xdd, "cross-page low byte");
      equal(bytes(DATA + 4096)[1], 0xaa, "cross-page high byte");
      equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset, 4096).getUint32(0, true), initial + 1, "resume did not replay prefix store");
    }));
  for (const signedDivision of [false, true]) for (const width of [8, 16, 32]) {
    await test(`${signedDivision ? "IDIV" : "DIV"} ${width} quotient, remainder and precise #DE`, () => fixture(
      [...(width === 16 ? [0x66] : []), width === 8 ? 0xf6 : 0xf7, signedDivision ? 0xf9 : 0xf1],
      async ({ reset, run }) => {
        const bits = BigInt(width), wide = bits * 2n;
        const dividends = [0n, 1n, -1n, (1n << bits) - 1n, 1n << bits,
          -(1n << (bits - 1n)), (1n << (wide - 1n)) - 1n, -(1n << (wide - 1n))];
        const divisors = [0n, 1n, -1n, 2n, -3n, 1n << (bits - 1n), (1n << bits) - 1n];
        for (const dividendInput of dividends) for (const divisorInput of divisors) {
          const raw = BigInt.asUintN(width * 2, dividendInput);
          const divisorBits = BigInt.asUintN(width, divisorInput);
          const dividend = signedDivision ? BigInt.asIntN(width * 2, raw) : raw;
          const divisor = signedDivision ? BigInt.asIntN(width, divisorBits) : divisorBits;
          const eax = width === 8 ? retainedResult(0xa5a50000, Number(raw), 16)
            : retainedResult(0xa5a50000, Number(raw & widthMask(width)), width);
          const edx = width === 8 ? 0x5a5a5678
            : retainedResult(0x5a5a0000, Number(raw >> bits), width);
          reset({ eax, edx, ecx: Number(divisorBits), flags: 0xa93 });
          const result = run(1);
          const quotient = divisor === 0n ? null : dividend / divisor;
          const lower = signedDivision ? -(1n << (bits - 1n)) : 0n;
          const upper = signedDivision ? (1n << (bits - 1n)) - 1n : widthMask(width);
          if (quotient === null || quotient < lower || quotient > upper) {
            equal(result.kind, 11, "guest divide error, never Wasm trap");
            equal(result.retired, 0, "division did not retire");
            equal(result.pc, CODE, "exact #DE PC");
            equal(result.words[0], eax, "fault retains EAX");
            equal(result.words[2], edx, "fault retains EDX");
            equal(result.words[9], 0xa93, "fault retains flags");
          } else {
            const q = Number(BigInt.asUintN(width, quotient));
            const r = Number(BigInt.asUintN(width, dividend % divisor));
            equal(result.retired, 1, "successful division retires");
            equal(result.words[0], width === 8 ? retainedResult(eax, q | r << 8, 16)
              : retainedResult(eax, q, width), "quotient register");
            equal(result.words[2], width === 8 ? edx : retainedResult(edx, r, width), "remainder register");
            equal(result.words[10] & ARITH, 0, "arithmetic flags undefined only after success");
          }
        }
      }));
  }
  await test("DIV reads memory before #DE and preserves prior completed flags", () => fixture(
    [0x83, 0xc0, 1, 0xf7, 0x33], async ({ reset, run, map, compile, bytes }) => {
      reset({ eax:0x7fffffff, ebx:0x5000 });
      const first=run(100);
      equal(first.kind,4,"missing divisor faults on read");
      equal(first.pc,CODE+3,"division fault PC");
      const flags=arithmetic("add",32,0x7fffffff,1,0).flags;
      equal(first.words[9]&ARITH,flags,"prior arithmetic visible at memory fault");
      map(0x5000,READ,page()); await compile(first.pc);
      const zero=run(1);
      equal(zero.kind,11,"mapped zero divisor produces #DE");
      equal(zero.words[0],0x80000000,"failed divide preserves completed ADD");
      equal(zero.words[9]&ARITH,flags,"prior flags visible at #DE");
      equal(bytes(DATA)[0],0,"unrelated memory unchanged");
    }));
  await test("CALL/RET retains guest stack bytes and returns to guest address", () => fixture(
    [0xe8, 1, 0, 0, 0, 0xcc, 0xb8, 0x42, 0, 0, 0, 0xc3], async ({ bytes, reset, run, compile }) => {
      reset();
      let result = run(100);
      if (options.rootedSsa) {
        equal(result.kind, 2, "RET requests its unrestricted continuation root");
        equal(result.retired, 3, "CALL/body/RET complete before root handoff");
        const completed = result.retired;
        await compile(result.pc); result = run(100 - completed);
        result.retired += completed;
      }
      equal(result.kind, 3, "return reaches sentinel");
      equal(result.pc, CODE + 5, "return continuation");
      equal(result.retired, 3, "CALL body RET all completed");
      equal(result.words[0], 0x42, "callee result");
      equal(result.words[4], STACK + 0x800, "guest stack restored");
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset, 4096).getUint32(0x7fc, true), CODE + 5, "guest return address was actually stored");
    }));
  await test("rooted SSA mixed flag producers fault after one completed push and resume precisely", async () => {
    for (const ecx of [0, 1]) await fixture(
      [0x85,0xc9,0x74,4,0x01,0xd8,0xeb,2,0x29,0xd8,0x50,0x8b,0x12,0x0f,0x0b],
      async ({ reset, run, map, compile, bytes }) => {
        reset({ eax: 0x7fffffff, ebx: 1, ecx, edx: 0x5000, flags: 0x602 });
        const expected = arithmetic(ecx ? "add" : "sub", 32, 0x7fffffff, 1, 0);
        const fault = run(100);
        equal(fault.kind, 4, "joined load faults"); equal(fault.pc, CODE+11, "joined precise fault PC");
        equal(fault.retired, ecx ? 5 : 4, "all preceding control and push effects completed once");
        equal(fault.words[0], expected.result, "producer-dependent GPR survives");
        equal(fault.words[9], 0x602 | expected.flags, "all arithmetic and raw nonarithmetic flags survive");
        equal(fault.words[4], STACK+0x7fc, "one completed push retained");
        equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset,4096).getUint32(0x7fc,true),expected.result,"pushed value retained");
        map(0x5000, READ, page(wordBytes(0xdecafbad))); await compile(fault.pc);
        const resumed = run(100);
        equal(resumed.kind, 8, "repair resumes to UD2"); equal(resumed.retired, 1, "only failed load now completes");
        equal(resumed.words[2], 0xdecafbad, "repaired load result");
        equal(resumed.words[4], STACK+0x7fc, "cold recovery never repeats the push");
        equal(resumed.words[9], fault.words[9], "repair preserves joined flags");
      });
  });
  await test("rooted SSA loop keeps prior load and current value live at a later fault", () => fixture(
    [0x8b,0x06,0x89,0xc3,0x40,0x81,0xc6,0x00,0x10,0,0,0x47,0x8b,0x06,0x49,0x75,0xf1,0x0f,0x0b],
    async ({ reset, run, map, bytes, compile }) => {
      setWord(bytes(DATA),0,0x11111111); map(DATA+4096,READ|WRITE,page(wordBytes(0x22222222)));
      reset({ esi: DATA, ecx: 2 });
      const fault = run(100);
      equal(fault.kind,4,"second-iteration load faults"); equal(fault.pc,CODE+12,"faulting loop load PC");
      equal(fault.words[0],0x22222223,"current incremented register retained");
      equal(fault.words[3],0x22222222,"previous completed load retained separately");
      equal(fault.words[7],2,"two completed loop increments"); equal(fault.words[1],1,"faulting iteration has not decremented count");
      setWord(bytes(DATA+4096),0,0x99999999); map(DATA+8192,READ,page(wordBytes(0x33333333)));
      await compile(fault.pc); const resumed=run(100);
      equal(resumed.kind,8,"loop repair reaches UD2"); equal(resumed.words[0],0x33333333,"repaired load succeeds");
      equal(resumed.words[3],0x22222222,"old load was not replayed from changed memory");
      equal(resumed.words[7],2,"loop prefix not replayed"); equal(resumed.words[1],0,"remaining counter decremented once");
    }));
  await test("rooted SSA parallel loop phis rotate three registers before a precise fault", async () => {
    for (const count of [1,2,3,4]) await fixture(
      [0x93,0x87,0xd3,0x49,0x75,0xfa,0x8b,0x06,0x0f,0x0b],
      async ({reset,run,map,compile}) => {
        reset({eax:0x11111111,ebx:0x22222222,edx:0x33333333,ecx:count,esi:0x5000,flags:0x203});
        let expected=[0x11111111,0x22222222,0x33333333];
        for(let i=0;i<count;i++) expected=[expected[1],expected[2],expected[0]];
        const fault=run(100);
        equal(fault.kind,4,"post-rotation load faults"); equal(fault.pc,CODE+6,"rotation fault position");
        equal(fault.retired,count*4,"all rotation instructions retired once");
        equal(fault.words[0],expected[0],"parallel phi EAX"); equal(fault.words[3],expected[1],"parallel phi EBX");
        equal(fault.words[2],expected[2],"parallel phi EDX"); equal(fault.words[1],0,"loop exhausted count");
        equal(fault.words[9]&CF,CF,"DEC retains incoming carry across rotations");
        map(0x5000,READ,page(wordBytes(0x98765432))); await compile(fault.pc); const resumed=run(100);
        equal(resumed.kind,8,"rotation repair reaches UD2"); equal(resumed.words[3],expected[1],"repair never repeats rotation");
        equal(resumed.words[2],expected[2],"repair preserves other rotation output");
      });
  });
  await test("rooted SSA irreducible two-entry SCC preserves route and fault state", async () => {
    for(const entry of [0,1]) for(const count of [1,2,3,4,5,6]) await fixture(
      [0x85,0xc0,0x74,6,0x40,0x49,0x74,10,0xeb,0,0x83,0xc3,2,0x49,0x74,2,0xeb,0xf2,0x8b,0x16,0x0f,0x0b],
      async ({reset,run}) => {
        reset({eax:entry,ebx:100,edx:0x55555555,ecx:count,esi:0x5000});
        let a=entry,b=100,path=entry ? 0 : 1;
        for(let i=0;i<count;i++){if(path===0)a++;else b+=2;path^=1;}
        const fault=run(100);
        equal(fault.kind,4,"two-entry SCC reaches precise load fault"); equal(fault.pc,CODE+18,"irreducible fault position");
        equal(fault.retired,4*count+1,"both route patterns retire exact work");
        equal(fault.words[0],a,"A path executes expected times"); equal(fault.words[3],b,"B path executes expected times");
        equal(fault.words[1],0,"both entries exhaust counter"); equal(fault.words[2],0x55555555,"fault does not overwrite destination");
      });
  });
  if (options.rootedSsa) await test("rooted SSA external interior PC creates a fresh architectural root", () => fixture(
    [0x40,0xeb,0,0x01,0xd8,0x89,0x02,0x0f,0x0b],
    async ({ handle, reset, run, compile, bytes }) => {
      reset({eax:1,ebx:4,edx:DATA}); const first=run(100);
      equal(first.kind,8,"normal path reaches UD2"); equal(first.words[0],6,"normal root result");
      ok(api.backend_cache_select(handle,CODE+3),"look up independent interior root");
      equal(api.backend_cache_selected(handle),0,"normal interior is not an externally admitted SSA entry");
      await compile(CODE+3); reset({eax:7,ebx:9,edx:DATA,eip:CODE+3});
      const second=run(100); equal(second.kind,8,"fresh root reaches UD2"); equal(second.words[0],16,"fresh architectural inputs used");
      equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset,4096).getUint32(0,true),16,"fresh root stores its own result");
    }));
  for (const width of [16, 32]) await test(`LEAVE ${width} uses full frame pointer and preserves flags`, () => fixture(
    [...(width === 16 ? [0x66] : []), 0xc9], async ({ map, bytes, reset, run }) => {
      const frame = 0x12348004;
      map(frame & ~4095, READ | WRITE, page([0, 0, 0, 0, ...wordBytes(0xdeadbeef)]));
      reset({ esp: STACK + 0x800, ebp: frame, flags: 0xa93 });
      const result = run(1);
      equal(result.retired, 1, "LEAVE completed");
      equal(result.words[4], frame + width / 8, "ESP uses full 32-bit EBP and operand width");
      equal(result.words[5], width === 16 ? 0x1234beef : 0xdeadbeef, "EBP width semantics");
      equal(result.words[9], 0xa93, "LEAVE preserves flags");
      equal(bytes(frame & ~4095)[4], 0xef, "LEAVE does not change stack bytes");
    }));
  await test("LEAVE fault preserves ESP/EBP and resume does not replay preceding store", () => fixture(
    [0xff, 0x03, 0xc9], async ({ bytes, map, reset, run, compile }) => {
      reset({ ebx: DATA, ebp: 0x9ffe });
      const first = run(100);
      equal(first.kind, 4, "LEAVE unmapped stack fault");
      equal(first.pc, CODE + 2, "LEAVE fault location");
      equal(first.retired, 1, "only earlier store completed");
      equal(first.words[4], STACK + 0x800, "fault ESP remains before LEAVE");
      equal(first.words[5], 0x9ffe, "fault EBP remains before LEAVE");
      equal(bytes(DATA)[0], 1, "earlier store retained");
      map(0x9000, READ | WRITE); bytes(0x9000).set([0x78, 0x56], 4094);
      await compile(first.pc);
      const crossing = run(1);
      equal(crossing.kind, 4, "second-page LEAVE fault");
      equal(crossing.address, 0xa000, "second-page fault address");
      equal(crossing.words[4], STACK + 0x800, "crossing fault ESP unchanged");
      map(0xa000, READ | WRITE, page([0x34, 0x12]));
      const resumed = run(1);
      equal(resumed.words[4], 0xa002, "successful ESP after resume");
      equal(resumed.words[5], 0x12345678, "successful EBP after resume");
      equal(bytes(DATA)[0], 1, "prefix store not replayed");
    }));
  await test("CALL code-alias stack store exits at callee after completed push", () => fixture(
    [0xe8, 1, 0, 0, 0, 0xcc, 0x40, 0xc3], async ({ protect, reset, run, compile }) => {
      protect(STACK, READ | WRITE | EXECUTE | SEALED);
      reset();
      const result = run(100);
      equal(result.kind, 7, "stack write triggers code invalidation exit");
      equal(result.retired, 1, "call completed before invalidation exit");
      equal(result.pc, CODE + 6, "completed call continues at callee");
      equal(result.words[4], STACK + 0x7fc, "completed push retained");
      await compile(result.pc);
      const resumed = run(100);
      equal(resumed.pc, CODE + 5, "callee return after code-write exit");
      equal(resumed.words[0], 1, "callee executed once");
      equal(resumed.words[4], STACK + 0x800, "return pops single call frame");
    }));
  await test("failed CALL push publishes no stack or target side effect", () => fixture(
    [0xe8, 1, 0, 0, 0, 0xcc, 0xc3], async ({ reset, run }) => {
      reset({ esp: 0x7000, flags: 0xa93 });
      const result = run(100);
      equal(result.kind, 4, "unmapped call stack faults");
      equal(result.pc, CODE, "failed CALL remains current");
      equal(result.retired, 0, "CALL has not completed");
      equal(result.address, 0x6ffc, "failed return-address store");
      equal(result.words[4], 0x7000, "ESP unchanged on failed push");
      equal(result.words[9], 0xa93, "flags unchanged on failed push");
    }));
  await test("indirect callee and return reuse cached regions with exact guest CALL and RET effects", () => fixture(
    [0xff, 0xd0, 0xcc, 0xb8, 0x44, 0, 0, 0, 0xc3], async ({ handle, reset, run, compile, select }) => {
      const callerId = select(CODE);
      reset({ eax: CODE + 3 });
      const first = run(100);
      equal(first.kind, 2, "unknown indirect callee exits explicitly");
      equal(first.pc, CODE + 3, "unknown callee address");
      equal(first.retired, 1, "indirect CALL completed its push");
      equal(first.words[4], STACK + 0x7fc, "indirect CALL pushed exactly one return address");
      const calleeId = await compile(first.pc);
      const resumed = run(100);
      equal(resumed.pc, CODE + 2, "RET follows the guest return continuation");
      equal(resumed.retired, 2, "resume executes body and RET");
      equal(resumed.words[4], STACK + 0x800, "RET pops exactly one guest return address");
      const continuationId = options.rootedSsa ? await compile(resumed.pc) : callerId;
      reset({ eax: CODE + 3 });
      equal(select(CODE), callerId, "warm caller keeps its compiled unit");
      const warmCall = run(100);
      let warmReturn = { retired: 0 }, warmStop = warmCall;
      if (!options.linkedRegions) {
        equal(warmCall.kind, 2, "warm CALL exits at the cached callee boundary");
        equal(select(warmCall.pc), calleeId, "warm callee keeps its compiled unit");
        warmReturn = run(100);
        equal(select(warmReturn.pc), continuationId, "return continuation reuses its admitted unit");
        warmStop = run(100);
      }
      equal(warmStop.kind, 3, "warm execution reaches the same sentinel");
      equal(options.linkedRegions ? warmStop.retired : warmCall.retired + warmReturn.retired + warmStop.retired,
        3, "warm total guest work is unchanged");
      equal(warmStop.words[0], 0x44, "indirect callee result");
      equal(warmStop.words[4], STACK + 0x800, "warm guest stack balances across units");
      equal(api.backend_cache_unit_count(handle), options.rootedSsa ? 3 : 2, "warm execution did not compile another region");
    }));
  await test("RET follows modified guest return address rather than a host call stack", () => {
    const code = [0xe8, 1, 0, 0, 0, 0xcc, 0xc7, 0x04, 0x24, ...wordBytes(CODE + 20), 0xc3];
    while (code.length < 20) code.push(0xcc);
    code.push(0x40);
    return fixture(code, async ({ reset, run, compile }) => {
      reset();
      const first = run(100);
      equal(first.kind, 2, "altered return address is an external edge");
      equal(first.pc, CODE + 20, "RET loaded actual guest stack value");
      equal(first.words[4], STACK + 0x800, "RET popped guest stack");
      await compile(first.pc);
      const resumed = run(100);
      equal(resumed.words[0], 1, "altered continuation executed");
    });
  });
  if (!options.rootedSsa && !options.valueIr && !options.structuredLoops && !options.affineMemory) {
    await test("budget continuation compiles only its suffix and rejoins the ordinary region", () => fixture(
      [0xff,0x05,...wordBytes(DATA),0x50,0x83,0xc0,7,0x5b,0xeb,2,0xcc,0xcc,0x41],
      async ({handle,reset,run,compile,select,bytes})=>{
        const parent=select(CODE);reset({eax:0x12345678,ecx:7});
        const prefix=run(2);equal(prefix.pc,CODE+7,"budget publishes exact interior PC");
        const continuation=await compile(prefix.pc);
        equal(api.backend_instruction_count(handle),3,"only ADD POP JMP suffix is transformed");
        equal(api.backend_block_count(handle),1,"existing successor blocks are reused");
        equal(api.backend_cache_unit_count(handle),2,"one bounded continuation resource is added");
        const suffix=run(3);equal(suffix.pc,CODE+15,"suffix reaches original successor head");
        equal(suffix.words[0],0x1234567f,"ADD executes once");equal(suffix.words[3],0x12345678,"POP consumes original PUSH");
        equal(suffix.words[4],STACK+0x800,"stack operation is not replayed");
        equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(0,true),1,"earlier store remains single");
        equal(select(suffix.pc),parent,"cached successor keeps the normal region");
        equal(run(1).words[1],8,"normal region continues after the suffix");
        reset({eax:0x12345678,ecx:7});select(CODE);run(2);
        equal(select(CODE+7),continuation,"later same budget boundary reuses the suffix");
        const warm=run(options.linkedRegions?4:3);
        equal(warm.words[4],STACK+0x800,"warm continuation keeps exact stack effects");
        if(options.linkedRegions)equal(warm.words[1],8,"tail link continues within the same budget");
      }));
    await test("budget continuation fault keeps completed prefix and mapping repair", () => fixture(
      [0xff,0x05,...wordBytes(DATA),0x50,0xa1,...wordBytes(0x4000),0x43],
      async ({reset,run,compile,bytes,map,select})=>{
        reset({eax:0x76543210});const prefix=run(2);await compile(prefix.pc);
        const fault=run(10);equal(fault.kind,4,"interior load faults precisely");
        equal(fault.retired,0,"fault repeats no prefix");equal(fault.pc,CODE+7,"fault EIP");equal(fault.address,0x4000,"fault address");
        equal(fault.words[4],STACK+0x7fc,"previous PUSH remains completed");
        equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(0,true),1,"previous write remains single");
        map(0x4000,READ|WRITE,page(wordBytes(0xdeadbeef)));select(fault.pc);
        const resumed=run(2);equal(resumed.words[0],0xdeadbeef,"repaired mapping supplies load");equal(resumed.words[3],1,"successor executes");
        equal(resumed.words[4],STACK+0x7fc,"repaired fault does not replay PUSH");
      }));
    await test("budget continuation restores raw80 MMX aliases and complete context", () => fixture(
      [0xd9,0xe8,0xd9,0xee,0xde,0xc1,0xdd,0x1d,...wordBytes(DATA),
        0x0f,0x6f,0x05,...wordBytes(DATA+16),0x0f,0x77,0xd9,0xe8,0xdf,0xe0],
      async ({reset,state,run,compile,select,bytes,saveSnapshot,restoreSnapshot})=>{
        const count=8;
        for(let top=0;top<8;top++)for(let split=1;split<count;split++){
          const initial=()=>{reset({eax:0x12345678,flags:0xad7});bytes(DATA).fill(0);setWord(bytes(DATA),16,0x11223344);setWord(bytes(DATA),20,0x55667788);state()[60]=top<<11;state()[61]=0xffff;};
          initial();select(CODE);const whole=run(count),expected=whole.words.slice(0,100),memory=bytes(DATA).slice();
          initial();select(CODE);const prefix=run(split),saved=saveSnapshot();
          reset({eax:7,esp:STACK+0x900});restoreSnapshot(saved);await compile(prefix.pc);
          const continued=run(count-split);equal(continued.retired,count-split,"suffix retires exactly remaining work");
          for(let n=0;n<100;n++)equal(continued.words[n],expected[n],"complete FP and scalar state after context restoration");
          for(let n=0;n<4096;n++)equal(bytes(DATA)[n],memory[n],"complete memory after FP/MMX continuation");
        }
      }));
  }
  if (options.linkedRegions) {
    await test("linked regions preserve every shared budget boundary through a long cycle", () => fixture(
      [0x40, 0xff, 0xe2], async ({ map, compile, reset, run }) => {
        const target = 0x4000;
        map(target, READ | EXECUTE | SEALED, page([0x43, 0xff, 0xe6], 0xcc));
        await compile(target);
        for (const budget of [0, 1, 2, 3, 4, 5, 100001]) {
          reset({ edx: target, esi: CODE });
          const result = run(budget);
          // Budget completion is independent of compiled ownership. The
          // next target is not fetched merely to distinguish its owner.
          equal(result.kind, 1,
            `linked cycle exits at exact exhausted boundary ${budget}`);
          equal(result.retired, budget, "each guest instruction retires exactly once");
          equal(result.words[0], Math.floor((budget + 3) / 4), "live EAX crosses each link");
          equal(result.words[3], Math.floor((budget + 1) / 4), "live EBX crosses each link");
          equal(result.pc, [CODE, CODE + 1, target, target + 1][budget % 4], "exact budget EIP");
        }
      }));
    await test("linked regions forward arithmetic carry and defined flags without state reload", () => fixture(
      [0x83, 0xc0, 1, 0xff, 0xe2], async ({ map, compile, reset, run, bytes }) => {
        const target = 0x4000;
        map(target, READ | EXECUTE | SEALED, page([0x83, 0xd3, 0, 0x89, 0x19, 0x0f, 0x0b], 0xcc));
        await compile(target);
        reset({ eax: 0xffffffff, ebx: 7, ecx: DATA, edx: target });
        const result = run(100);
        equal(result.kind, 8, "precise UD2 exit after linked arithmetic");
        equal(result.retired, 4, "ADD, branch, ADC and store completed");
        equal(result.pc, target + 5, "target fault position");
        equal(result.words[0], 0, "source ADD result retained");
        equal(result.words[3], 8, "target ADC consumed live carry");
        equal(bytes(DATA)[0], 8, "target store used live destination");
        equal(result.words[10] & ARITH, ARITH, "defined arithmetic flags survive link");
      }));
    await test("linked fault keeps prior stores and resumes only the failed target instruction", () => fixture(
      [0xff, 0x03, 0xff, 0xe2], async ({ map, compile, reset, run, bytes }) => {
        const target = 0x4000, missing = 0x6000;
        map(target, READ | EXECUTE | SEALED, page([0x8b, 0x01, 0x47, 0xcc], 0xcc));
        await compile(target);
        reset({ ebx: DATA, ecx: missing, edx: target, edi: 10 });
        const first = run(100);
        equal(first.kind, 4, "target read faults");
        equal(first.retired, 2, "source write and branch completed");
        equal(first.pc, target, "faulting target remains current");
        equal(first.faultPc, target, "fault record identifies target instruction");
        equal(first.address, missing, "exact missing address");
        equal(bytes(DATA)[0], 1, "source write is visible once");
        map(missing, READ | WRITE, page([42]));
        bytes(DATA)[0] = 77;
        const resumed = run(2);
        equal(resumed.retired, 2, "read and following increment resume");
        equal(resumed.words[0], 42, "resumed target read");
        equal(resumed.words[7], 11, "following target increment occurs once");
        equal(bytes(DATA)[0], 77, "source write was not replayed");
      }));
    for (const [firstCount, nextCount] of [[4096, 1], [3000, 2000]]) {
      await test(`linked REP shares element fuel across units ${firstCount}+${nextCount}`, () => fixture(
        [0xf3, 0xaa, 0xb9, ...wordBytes(nextCount), 0xff, 0xe2], async ({ map, compile, reset, run, bytes }) => {
          const target = 0x4000;
          map(target, READ | EXECUTE | SEALED, page([0xf3, 0xaa, 0xcc], 0xcc));
          map(STACK + 4096, READ | WRITE);
          await compile(target);
          reset({ edi: STACK, ecx: firstCount, eax: 7, edx: target });
          const first = run(100);
          const partial = firstCount < 4096;
          equal(first.kind, partial ? 12 : 1, "shared element quota yields at target");
          equal(first.retired, 3, "source REP, MOV and branch retired only");
          equal(first.pc, target, "target REP remains resumable");
          equal(first.error, partial ? 4096 - firstCount : 0, "target partial progress is explicit");
          equal(first.words[1], firstCount + nextCount - 4096, "remaining target element count");
          equal(first.words[7], STACK + 4096, "global fuel capped completed stores");
          equal(bytes(STACK + 4096)[0], 0, "no store beyond shared fuel");
          bytes(STACK)[0] = 99;
          const resumed = run(1);
          equal(resumed.retired, 1, "target REP completes on next outer invocation");
          equal(resumed.words[1], 0, "target remainder consumed");
          equal(bytes(STACK)[0], 99, "completed source prefix not replayed");
          equal(bytes(STACK + 4096)[0], 7, "remaining destination written");
        }));
    }
    await test("linked code alias writes retire stale target bindings before resuming", () => fixture(
      [0xc6, 0x03, 0x43, 0xff, 0xe2], async ({ handle, map, alias, compile, reset, run, drainRetired, links, generatedUnits }) => {
        const target = 0x4000;
        map(target, READ | EXECUTE | SEALED, page([0x40, 0xcc], 0xcc));
        alias(0x5000, target, READ | WRITE);
        const oldId = await compile(target), oldSlot = generatedUnits.get(oldId).slot;
        reset({ eax: 7, ebx: 0x5000, edx: target });
        const first = run(100);
        equal(first.kind, 7, "code write exits before linked target");
        equal(first.retired, 1, "code write completes once");
        equal(first.pc, CODE + 3, "resume is following branch");
        equal(first.words[0], 7, "old target was not called");
        equal(api.backend_link_resolve(handle, target), 0n, "stale target is not resolvable");
        drainRetired();
        equal(links.get(oldSlot), null, "retired table target cleared");
        check(!generatedUnits.has(oldId), "retired instance removed");
        await compile(target);
        await compile(CODE + 3);
        const resumed = run(2);
        equal(resumed.words[0], 7, "old INC EAX never executes");
        equal(resumed.words[3], 0x5001, "new INC EBX executes once");
      }));
    await test("unbound linked targets exit at exact PC until an instance is published", () => fixture(
      [0xff, 0xe2], async ({ handle, map, compile, reset, run }) => {
        const target = 0x4000;
        map(target, READ | EXECUTE | SEALED, page([0x40, 0xcc], 0xcc));
        ok(api.backend_compile(handle, target, 2), "compile without publishing an instance");
        equal(api.backend_link_resolve(handle, target), 0n, "compiled but unbound target misses");
        reset({ edx: target });
        const first = run(100);
        equal(first.kind, 2, "unbound target takes the explicit host exit");
        equal(first.retired, 1, "branch retired once");
        equal(first.pc, target, "host receives exact unbound target");
        equal(first.words[0], 0, "unbound target did not execute");
        await compile(target);
        reset({ edx: target });
        const second = run(2);
        equal(second.kind, 1, "published target executes within shared budget");
        equal(second.retired, 2, "branch and target instruction retire");
        equal(second.words[0], 1, "published target result");
      }));
    await test("retirement overflow clears all links and cannot resolve an unbound survivor", () => fixture(
      [0x40], async ({ handle, compile, reset, run, drainRetired, links, generatedUnits }) => {
        const oldId = api.backend_cache_selected(handle), oldSlot = api.backend_link_slot(handle, oldId);
        for (let n = 0; n < links.length * 2 + 2; n++) ok(api.backend_compile(handle, CODE, 1), "bounded unbound cache churn");
        equal(api.backend_link_resolve(handle, CODE), 0n, "unbound surviving region cannot link");
        drainRetired();
        equal(generatedUnits.size, 0, "all-retired sentinel clears instance ownership");
        for (let n = 0; n < links.length; n++) equal(links.get(n), null, "all-retired sentinel clears table slot");
        check(api.backend_link_bind(handle, oldId, oldSlot) !== 0, "retired identity cannot be rebound");
        await compile(CODE, 1);
        reset();
        const result = run(1);
        equal(result.words[0], 1, "freshly bound survivor executes");
        equal(result.retired, 1, "fresh invocation accounting");
      }));
  }
  if (options.linkedRegions && hasLinkInvocation) {
    await test("linked warmed source preserves alternating indirect targets and partial budgets", () => fixture(
      [0xff,0xe2], async ({map,compile,reset,run}) => {
        const a=0x4000,b=0x6000;
        map(a,READ|EXECUTE|SEALED,page([0x40,0xba,...wordBytes(b),0xff,0xe7],0xcc));
        map(b,READ|EXECUTE|SEALED,page([0x83,0xc3,2,0xba,...wordBytes(a),0xff,0xe7],0xcc));
        await compile(a); await compile(b);
        for (let extra=0;extra<8;extra++) {
          reset({edx:a,edi:CODE});
          const budget=80+extra,result=run(budget),whole=Math.floor(budget/8),tail=budget%8;
          equal(result.retired,budget,"all alternating transfers count exactly once");
          equal(result.kind,1,"exact budget yield");
          equal(result.words[0],whole+Number(tail>=2),"only A increments EAX");
          equal(result.words[3],2*(whole+Number(tail>=6)),"only B adds to EBX");
          equal(result.words[2],tail>=3&&tail<7?b:a,"indirect target changes at the MOV boundary");
          equal(result.pc,[CODE,a,a+1,a+6,CODE,b,b+3,b+8][tail],"precise continuation after warm links");
        }
      }));
    await test("linked warmed target retains fault prefix and resumes after permission repair", () => fixture(
      [0xff,0xe2], async ({map,compile,reset,run,protect,bytes,state}) => {
        const target=0x4000;
        map(target,READ|EXECUTE|SEALED,page([0x40,0x8b,0x1e,0xff,0xe7],0xcc));
        await compile(target); setWord(bytes(DATA),0,71);
        reset({edx:target,edi:CODE,esi:DATA});
        let result=run(80); equal(result.words[0],20,"target warmed through twenty cycles");
        equal(result.words[3],71,"warm load result");
        protect(DATA,WRITE); result=run(80);
        equal(result.kind,4,"warm target still faults on denied read");
        equal(result.retired,2,"source jump and target increment precede fault");
        equal(result.pc,target+1,"load instruction is restartable");
        equal(result.words[0],21,"completed increment retained"); equal(result.words[3],71,"failed load cannot change EBX");
        protect(DATA,READ); setWord(bytes(DATA),0,93); await compile(target+1,2);
        result=run(2); equal(result.retired,2,"repaired load and return complete");
        equal(result.words[0],21,"resumption never repeats target prefix");
        equal(result.words[3],93,"resumed load observes new data"); equal(state()[8],CODE,"back at source after repair");
      }));
    for (const outerValidation of [false, true]) {
      await test(`linked shared entries reject pending code alias at ${outerValidation ? "outer admission" : "a new invocation"}`, () => fixture(
        [0xff, 0xe2], async ({ handle, map, alias, compile, reset, run, state }) => {
          const target = 0x4000, writer = 0x6000;
          map(target, READ | EXECUTE | SEALED, page([0x40, 0xeb, 0, 0x43, 0xff, 0xe6], 0xcc));
          alias(0x5000, target, READ | WRITE);
          map(writer, READ | EXECUTE | SEALED, page([0xc6, 0x06, 0x48, 0xff, 0xe2], 0xcc));
          const targetId = await compile(target, 256, [target, target + 3]);
          const writerId = await compile(writer);
          reset({ eip: target });
          equal(run(1).words[0], 1, "first target entry executes once");
          reset({ eip: target + 3 });
          equal(run(1).words[3], 1, "second entry executes its own suffix");
          reset({ eip: writer, esi: 0x5000, edx: target + 3, eax: 7, ebx: 9 });
          const result = run(20, () => {
            const before = Array.from(state());
            if (outerValidation) {
              check(api.backend_cache_validate(handle, targetId, target + 3) !== 0,
                "outer admission rejects unfinished write against a previously admitted shared entry");
            } else {
              // Begin from clean code while another unit's code store has not
              // reached finish yet. Its old invocation cannot authorize it.
              ok(api.backend_link_begin(handle, writerId, writer), "new invocation begins with clean code");
              equal(api.backend_link_resolve(handle, target + 3), 0n, "old shared entry cannot reuse an earlier invocation");
              ok(api.backend_link_end(handle), "end explicit clean invocation");
            }
            for (let n = 0; n < before.length; n++) equal(state()[n], before[n], "rejection preserves completed CPU effects");
          });
          equal(result.kind, 7, "code-alias write exits for invalidation");
          equal(result.retired, 1, "store retires once before the next branch");
          equal(result.pc, writer + 3, "exact continuation after code write");
          equal(result.words[0], 7, "old target prefix never executes");
          equal(result.words[3], 9, "old target suffix never executes");
          await compile(target, 256, [target, target + 3]);
          reset({ eip: target, eax: 7 });
          equal(run(1).words[0], 6, "fresh code executes changed decrement");
        }));
    }
    for (const aliasWrite of [false, true]) {
      await test(`linked NX dependency ${aliasWrite ? "alias" : "direct"} write forces fresh admission`, () => fixture(
        [0xff, 0xe2], async ({ map, alias, compile, reset, run, bytes }) => {
          const target = 0x4ff7, writer = 0x6000;
          const targetPage = page([], 0xcc);
          targetPage.set([0x40, 0x85, 0xc9, 0x74, 4, 0xff, 0xe6], target & 4095);
          map(0x4000, READ | EXECUTE | SEALED, targetPage);
          map(0x5000, READ | WRITE);
          if (aliasWrite) alias(0x7000, 0x5000, READ | WRITE);
          map(writer, READ | EXECUTE | SEALED, page([0xff, 0x03, 0xff, 0xe2], 0xcc));
          await compile(target); await compile(writer);
          reset({ ecx: 1, edx: target, esi: writer, ebx: aliasWrite ? 0x7000 : 0x5000 });
          const first = run(100);
          equal(first.kind, 2, "changed NX dependency requires host code admission");
          equal(first.pc, target, "stopped at precise next target");
          equal(first.retired, 7, "initial branch, target and writer completed once");
          equal(first.words[0], 1, "stale target was not reentered");
          equal(bytes(0x5000)[0], 1, "NX dependency write completed");
          await compile(target);
          const resumed = run(1);
          equal(resumed.retired, 1, "freshly admitted target resumes");
          equal(resumed.words[0], 2, "target increment occurs only after readmission");
        }));
    }
    await test("linked memo preserves the exact arbitrary entry EIP", () => fixture(
      [0xff, 0xe2], async ({ map, compile, reset, run }) => {
        const target = 0x4000, writer = 0x6000, stop = 0x7000;
        map(target, READ | EXECUTE | SEALED, page([0x40, 0xeb, 0, 0x43, 0xff, 0xe6], 0xcc));
        map(writer, READ | EXECUTE | SEALED,
          page([0xba, ...wordBytes(target + 3), 0xbe, ...wordBytes(stop), 0xff, 0xe2], 0xcc));
        map(stop, READ | EXECUTE | SEALED, page([0x0f, 0x0b], 0xcc));
        await compile(target); await compile(writer); await compile(stop);
        if (options.rootedSsa) await compile(target + 3);
        reset({ edx: target, esi: writer });
        const result = run(100);
        equal(result.kind, 8, "exact final UD2 reached");
        equal(result.pc, stop, "final target PC retained");
        equal(result.retired, 10, "interior reentry does not execute skipped prefix");
        equal(result.words[0], 1, "first entry prefix runs once");
        equal(result.words[3], 2, "shared interior executes for both entries");
      }));
    await test("ended linked invocation revalidates restored CPU state before resolving", () => fixture(
      [0xff, 0xe2], async ({ handle, map, compile, select, reset, state }) => {
        const target = 0x4000;
        map(target, READ | EXECUTE | SEALED, page([0x40], 0xcc));
        await compile(target);
        reset({ edx: target }); const id = select(CODE);
        ok(api.backend_link_begin(handle, id, CODE), "begin explicit invocation");
        check(api.backend_link_resolve(handle, target) !== 0n, "first target is admitted");
        equal(api.backend_link_memo_misses(handle), 1, "first target requires ordinary validation");
        equal(api.backend_link_memo_hits(handle), 0, "first target is not an unvalidated hit");
        check(api.backend_link_resolve(handle, target) !== 0n, "target remains resolvable in same invocation");
        equal(api.backend_link_memo_hits(handle), 1, "same target reuses admitted proof");
        equal(api.backend_link_memo_misses(handle), 1, "reuse does not add another slow-path validation");
        ok(api.backend_link_end(handle), "host return clears invocation proof");
        state()[9] |= 0x100;
        equal(api.backend_link_resolve(handle, target), 0n, "unsupported restored TF cannot reuse ended proof");
        state()[9] &= ~0x100;
        check(api.backend_link_resolve(handle, target) !== 0n, "valid restored CPU uses ordinary checked resolver");
      }));
    await test("linked Wasm trap clears invocation proof through finally", () => fixture(
      [0xff, 0xe2], async ({ handle, map, compile, reset, run, state, links, generatedUnits }) => {
        const target = 0x4000;
        map(target, READ | EXECUTE | SEALED, page([0x40], 0xcc));
        const id = await compile(target), unit = generatedUnits.get(id);
        links.set(unit.slot, null);
        reset({ edx: target });
        let trapped = false;
        try { run(2); } catch (failure) { trapped = failure instanceof WebAssembly.RuntimeError; }
        check(trapped, "missing host resource gives an explicit Wasm trap");
        state()[9] |= 0x100;
        equal(api.backend_link_resolve(handle, target), 0n, "trap cleanup prevents stale proof after host control resumes");
        links.set(unit.slot, unit.instance.exports.link);
      }));
  }
  if (options.controlGraph) await test("AOT switch graph preserves arbitrary entries, mutable aliases, budget and precise faults", async () => {
    const code = new Array(0x81).fill(0xcc);
    code.splice(0, 12, 0x83,0xf8,1,0x77,0x2b,0xff,0x24,0x85,...wordBytes(DATA));
    code.splice(0x30,3,0x42,0xeb,0x4d);
    code.splice(0x40,3,0x41,0xeb,0x3d);
    code.splice(0x50,5,0x83,0xc1,2,0xeb,0x2b);
    await fixture(code, async ({handle,bytes,compile,controlGraph,reset,run,state,alias,map,select,protect}) => {
      new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset,4096).setUint32(0,CODE+0x40,true);
      new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset,4096).setUint32(4,CODE+0x50,true);
      const before = Array.from(state());
      const id = await compile(CODE,256,[CODE,CODE+5]);
      equal(JSON.stringify(Array.from(state())),JSON.stringify(before),"AOT preparation does not execute or publish guest state");
      equal(controlGraph()?.cases,2,"both binary-derived cases lowered");
      for (const eax of [0,1]) {
        reset({eax});
        const stopped=run(3);
        equal(stopped.retired,3,"cmp, branch, table jump retire exactly once");
        equal(stopped.pc,CODE+(eax?0x50:0x40),"budget stops before selected case");
        equal(stopped.words[1],0,"no speculative case side effect");
        reset({eax});
        const completed=run(5);
        equal(completed.pc,CODE+0x80,"case reaches common exit");
        equal(completed.words[1],eax?2:1,"case arithmetic completed");
      }
      reset({eip:CODE+5,eax:0x40000000});
      const wrapped=run(3);
      equal(wrapped.words[1],1,"arbitrary entry bypasses compare, wrapped memory address still loads real target");
      alias(0x5000,DATA,READ|WRITE);
      map(0x6000,READ|EXECUTE|SEALED,page([0xc7,0x06,...wordBytes(CODE+0x50)],0xcc));
      await compile(0x6000);
      reset({eip:0x6000,esi:0x5000});
      equal(run(1).retired,1,"guest alias write completes once");
      select(CODE);
      equal(api.backend_cache_selected(handle),id,"data-only table mutation retains code unit");
      reset({eax:0});
      equal(run(5).words[1],2,"changed table target takes dynamic guard miss without stale case");
      protect(DATA,WRITE);
      reset({eax:0,ecx:77});
      const fault=run(20);
      equal(fault.kind,4,"table read protection fault");
      equal(fault.pc,CODE+5,"fault belongs to original jump load");
      equal(fault.retired,2,"prior compare/branch preserved, faulting jump unretired");
      equal(fault.words[1],77,"fault does not execute a case");
      protect(DATA,READ|WRITE);
      reset({eip:CODE+5,eax:0,ecx:77});
      equal(run(3).words[1],79,"repair resumes at faulting load without replaying prior effects");
    });
  });
  if (options.directLinks) await test("coherent direct targets survive growth and retire before SMC and NX repair", () => fixture(
    [0xff, 0xe2], async ({handle, map, alias, bytes, compile, reset, run, protect}) => {
      const target = 0x4000, writer = 0x6000;
      map(target, READ | EXECUTE | SEALED, page([0x40, 0xff, 0xe3], 0xcc));
      alias(0x5000, target, READ | WRITE);
      await compile(target);
      const directory = api.backend_link_directory(handle);
      api.memory.grow(1);
      equal(api.backend_link_directory(handle), directory, "directory offset survives growth");
      reset({edx: target, ebx: CODE});
      const warmed = run(300);
      equal(warmed.words[0], 100, "direct loop completes the same arithmetic work");
      equal(warmed.pc, CODE, "complete direct loop backedge");
      check(api.backend_link_directory_count(handle, 0) >= 100, "generated links actually used the directory");
      equal(new Uint32Array(api.memory.buffer, directory, 1)[0], 0, "outer finally ends direct admission");
      map(writer, READ | EXECUTE | SEALED, page([0xc6, 0x06, 0x48, 0xff, 0xe2], 0xcc));
      await compile(writer);
      reset({eip: writer, esi: 0x5000, edx: target, ebx: CODE, eax: 7});
      const smc = run(20);
      equal(smc.kind, 7, "code-alias store stops the admitted chain");
      equal(smc.retired, 1, "store retires once before any target instruction");
      equal(smc.pc, writer + 3, "resume after completed store");
      equal(smc.words[0], 7, "stale increment never executed");
      equal(bytes(target)[0], 0x48, "completed alias store is visible");
      await compile(target);
      reset({edx: target, ebx: CODE, eax: 7});
      equal(run(3).words[0], 6, "new target decrements rather than stale increment");
      protect(target, READ);
      await compile(target);
      reset({edx: target, ebx: CODE, eax: 7});
      const fault = run(20);
      equal(fault.kind, 4, "NX target remains a precise fetch fault");
      equal(fault.pc, target, "fault names inaccessible target");
      equal(fault.retired, 1, "completed incoming jump retained");
      equal(fault.words[0], 7, "faulting target has no arithmetic side effect");
      protect(target, READ | EXECUTE | SEALED);
      await compile(target);
      reset({eip: target, edx: target, ebx: CODE, eax: 7});
      equal(run(3).words[0], 6, "permission repair resumes without replaying store or incoming jump");
    }));
  await test("x87 physical payload, MMX aliases, XMM and MXCSR survive integer code", () => fixture(
    [0x40, 0x83, 0xc0, 2], async ({ reset, state, run }) => {
      reset();
      // x87 begins at byte 140; all representation fields remain opaque to
      // the integer emitter, including 80-bit values that cannot fit in f64.
      for (let index = 35; index < 100; index += 1) state()[index] = next();
      const before = Array.from(state().slice(35, 100));
      run(100);
      before.forEach((value, index) => equal(state()[index + 35], value, `extended state word ${index}`));
    }));
  await test("32-bit wrap faults before any cross-boundary store", () => fixture(
    [0x66, 0xa3, 0xff, 0xff, 0xff, 0xff], async ({ map, bytes, reset, run }) => {
      map(0xfffff000, READ | WRITE, page([], 0x77));
      map(0, READ | WRITE, page([], 0x88));
      reset({ eax: 0x1234 });
      const result = run(100);
      equal(result.kind, 6, "linear address wrap is a general protection fault");
      equal(result.pc, CODE, "faulting wrapped store PC");
      equal(result.retired, 0, "wrapped store did not complete");
      equal(bytes(0xfffff000)[4095], 0x77, "last guest byte unchanged");
      equal(bytes(0)[0], 0x88, "first guest byte unchanged");
    }));
  await test("FS-relative read, LEA offset, and backward branch", () => fixture(
    [0x64, 0xa1, 0x10, 0, 0, 0, 0x64, 0x8d, 0x58, 4, 0x40, 0x83, 0xf8, 3, 0x7c, 0xfa], async ({ map, reset, run }) => {
      const data = page(); data[16] = 1; map(DATA, READ | WRITE, data);
      reset({ fs: DATA });
      const result = run(100);
      equal(result.kind, 3, "loop terminates at sentinel");
      equal(result.words[0], 3, "loop result");
      equal(result.words[3], 5, "LEA ignores FS segment base");
      equal(result.retired, 8, "loop ran both actual iterations");
    }));
  for (const aliasWrite of [false, true]) {
    await test(`${aliasWrite ? "alias" : "direct"} self-modifying code retires and invalidates`, () => fixture(
      [0xc6, 0x05, ...wordBytes(aliasWrite ? 0x4000 : CODE), 0x90, 0x40], async ({ handle, alias, reset, run, bytes, compile }) => {
        if (aliasWrite) alias(0x4000, CODE, READ | WRITE);
        reset();
        const result = run(100);
        equal(result.kind, 7, "code write yields before stale next instruction");
        equal(result.pc, CODE + 7, "SMC continuation follows completed store");
        equal(result.retired, 1, "only code store completed");
        equal(result.words[0], 0, "following instruction has not executed");
        equal(bytes(CODE)[0], 0x90, "code bytes changed through selected address");
        check(api.backend_validate(handle, CODE) !== 0, "old compiled code is rejected");
        await compile(result.pc);
        const resumed = run(100);
        equal(resumed.words[0], 1, "recompiled continuation runs once");
      }, READ | WRITE | EXECUTE | SEALED));
  }
  await test("mapping change and non-flat segment reject old admission", () => fixture([0x40], async ({ handle, protect, state, reset }) => {
    reset();
    state()[16] = 0x1000; // CS base, intentionally not a supported flat environment.
    check(api.backend_validate(handle, CODE) !== 0, "non-flat CS is explicit unsupported admission");
    reset();
    protect(CODE, READ | SEALED);
    check(api.backend_validate(handle, CODE) !== 0, "execute protection change invalidates compiled code");
  }));
  await test("unimplemented single-step, VM86, and alignment modes reject admission", () => fixture(
    [0x40], async ({ handle, reset }) => {
      for (const [name, flag] of [["TF", 0x100], ["VM", 0x20000], ["AC", 0x40000]]) {
        reset({ flags: 0x202 | flag });
        check(api.backend_validate(handle, CODE) !== 0, `${name} cannot silently disable its architectural behavior`);
      }
      reset();
      ok(api.backend_validate(handle, CODE), "ordinary flat user-mode flags remain admitted");
    }));
  for (const opcode of [0xa1, 0xa3]) {
    await test(`FS ${opcode === 0xa1 ? "load" : "store"} checks offset limit before base wrap`, () => fixture(
      [0x64, 0x66, opcode, 0xff, 0xff, 0xff, 0xff], async ({ map, bytes, reset, run }) => {
        map(DATA, READ | WRITE, page([], 0x77));
        reset({ eax: 0x11223344, fs: DATA + 1 });
        const result = run(100);
        equal(result.kind, 6, "FS operand extends beyond the segment offset limit");
        equal(result.pc, CODE, "FS segment fault instruction");
        equal(result.retired, 0, "out-of-limit FS instruction did not complete");
        equal(result.words[0], 0x11223344, "FS failed load leaves EAX unchanged");
        equal(bytes(DATA)[0], 0x77, "FS failed store leaves first byte unchanged");
        equal(bytes(DATA)[1], 0x77, "FS failed store leaves second byte unchanged");
      }));
  }
  await test("FS base addition can wrap when the operand offset span is valid", () => fixture(
    [0x64, 0xa0, 0, 0x20, 0, 0], async ({ reset, run }) => {
      reset({ eax: 0x11223300, fs: 0xfffff000 });
      const result = run(1);
      equal(result.words[0], 0x11223364, "FS wrapped linear address reads first code byte");
      equal(result.retired, 1, "valid FS offset retired");
    }));
  for (const opcode of [0xa1, 0xa3]) {
    await test(`FS ${opcode === 0xa1 ? "load" : "store"} with linear-only span wrap exits unsupported`, () => fixture(
      [0x64, 0x66, opcode, 0, 0x10, 0, 0], async ({ map, bytes, reset, run }) => {
        map(0xfffff000, READ | WRITE, page([], 0x77));
        map(0, READ | WRITE, page([], 0x88));
        reset({ eax: 0x11223344, fs: 0xffffefff });
        const result = run(100);
        equal(result.kind, 3, "unimplemented linear-span wrap is not a manufactured guest fault");
        equal(result.pc, CODE, "unsupported FS instruction continuation");
        equal(result.retired, 0, "unsupported linear span did not execute");
        equal(result.words[0], 0x11223344, "unsupported load preserves register");
        equal(bytes(0xfffff000)[4095], 0x77, "unsupported store preserves final guest byte");
        equal(bytes(0)[0], 0x88, "unsupported store preserves first guest byte");
      }));
  }
  await test("unmapped instruction fetch faults and newly mapped code invalidates stub", () => fixture(
    [0x40], async ({ handle, map, compile, reset, run }) => {
      reset({ eip: 0x5000, eax: 0x1234, flags: 0xa93 });
      await compile(0x5000);
      const fault = run(100);
      equal(fault.kind, 4, "unmapped instruction fetch is a guest page fault");
      equal(fault.pc, 0x5000, "unmapped instruction PC");
      equal(fault.address, 0x5000, "unmapped fetch address");
      equal(fault.error, 20, "instruction/user/nonpresent error code");
      equal(fault.retired, 0, "unfetched instruction not completed");
      equal(fault.words[0], 0x1234, "failed instruction fetch preserves registers");
      equal(fault.words[9], 0xa93, "failed instruction fetch preserves flags");
      map(0x5000, READ | EXECUTE | SEALED, page([0x40, 0xcc], 0xcc));
      check(api.backend_validate(handle, 0x5000) !== 0, "mapping code invalidates previously absent fetch stub");
      await compile(0x5000);
      const resumed = run(1);
      equal(resumed.words[0], 0x1235, "mapped instruction executes after recompile");
    }));
  await test("NX instruction fetch reports present execute protection fault", () => fixture(
    [0x40], async ({ map, compile, reset, run }) => {
      map(DATA, READ | WRITE, page([0x40]));
      reset({ eip: DATA });
      await compile(DATA);
      const result = run(100);
      equal(result.kind, 4, "NX execute fault");
      equal(result.pc, DATA, "NX instruction PC");
      equal(result.address, DATA, "NX fetch address");
      equal(result.error, 21, "instruction/user/present error code");
      equal(result.retired, 0, "NX instruction not completed");
    }));
  await test("instruction split across NX page faults at first unavailable byte", () => fixture(
    [0x40], async ({ map, compile, reset, run }) => {
      const code = page([], 0xcc); code.set([0xb8, 0x11], 4094);
      map(CODE, READ | EXECUTE | SEALED, code);
      map(DATA, READ | WRITE, page([0x22, 0x33, 0x44]));
      reset({ eip: CODE + 4094, eax: 0x11223344 });
      await compile(CODE + 4094);
      const result = run(100);
      equal(result.kind, 4, "split instruction execute fault");
      equal(result.pc, CODE + 4094, "split instruction start remains fault PC");
      equal(result.address, DATA, "first missing execute byte");
      equal(result.error, 21, "split execute protection error code");
      equal(result.retired, 0, "partial instruction fetch never retires");
      equal(result.words[0], 0x11223344, "partial immediate did not modify EAX");
    }));
  await test("invalid opcode raises explicit invalid-instruction exit", () => fixture(
    [0x0f, 0x04], async ({ reset, run }) => {
      reset({ eax: 0x11223344 });
      const result = run(100);
      equal(result.kind, 8, "invalid instruction exception kind");
      equal(result.pc, CODE, "invalid opcode PC");
      equal(result.retired, 0, "invalid instruction not completed");
      equal(result.words[0], 0x11223344, "invalid instruction preserves registers");
    }));
  await test("UD2 traps at the exact Runtime sentinel after completed side effects", () => fixture(
    [0x83, 0x03, 1, 0x83, 0xc0, 1, 0x0f, 0x0b, 0x40], async ({ reset, run, bytes, compile }) => {
      reset({ eax: 0x7fffffff, ebx: DATA, flags: 0x202 });
      const result = run(100);
      equal(result.kind, 8, "UD2 is an architectural invalid-opcode trap");
      equal(result.pc, CODE + 6, "UD2 stays at the trapping instruction");
      equal(result.faultPc, CODE + 6, "UD2 publishes the exact fault PC");
      equal(result.retired, 2, "UD2 and following code never retire");
      equal(result.words[0], 0x80000000, "prior arithmetic is retained");
      equal(result.words[9] & ARITH, arithmetic("add", 32, 0x7fffffff, 1, 0).flags, "prior flags are materialized at trap");
      equal(bytes(DATA)[0], 1, "prior store completes once");
      // A trap can be inside an original block. Admit its exact new entry as
      // Runtime does on a code-cache miss before retrying the instruction.
      await compile(result.pc);
      const again = run(100);
      equal(again.kind, 8, "retry remains a trap until Runtime handles it");
      equal(again.retired, 0, "retry does not replay prior work");
      equal(bytes(DATA)[0], 1, "retry does not repeat completed store");
    }));
  await test("CS-prefixed store is an explicit unsupported access with no mutation", () => fixture(
    [0x2e, 0xa3, ...wordBytes(DATA)], async ({ bytes, reset, run }) => {
      reset({ eax: 0x11223344 });
      const result = run(100);
      equal(result.kind, 3, "CS memory semantics have not been flattened into writable DS");
      equal(result.pc, CODE, "unsupported CS store stays at its instruction");
      equal(result.retired, 0, "CS store did not execute");
      equal(bytes(DATA)[0], 0, "CS store preserves data");
    }));
  for (const prefixes of [4, 6, 7]) {
    const code = [...Array(prefixes).fill(0x66), 0x2e, 0x0f, 0x1f, 0x84, 0, 0, 0, 0, 0];
    await test(`CS-prefixed ${code.length}-byte NOP preserves no-access and length semantics`, () => fixture(
      code, async ({ state, reset, run }) => {
        reset({ eax: 0x70000000, ecx: 0x12345678, flags: 0xa93 });
        const prior = Array.from(state().slice(0, 8));
        const result = run(1);
        const valid = code.length <= 15;
        equal(result.kind, valid ? 1 : 3, "NOP or explicit overlength exit");
        equal(result.retired, valid ? 1 : 0, "NOP is exactly one instruction within length bound");
        equal(result.pc, CODE + (valid ? code.length : 0), "NOP exact continuation");
        equal(result.words[9], 0xa93, "NOP preserves flags");
        for (let n = 0; n < 8; n += 1) equal(result.words[n], prior[n], "NOP preserves GPRs without memory access");
      }));
  }
  for (const [label, code] of [["default SS through ESP", [0x8b, 0x04, 0x24]],
    ["explicit SS override", [0x36, 0x66, 0xa3, 0xff, 0xff, 0xff, 0xff]]]) {
    await test(`${label} preserves stack-segment fault identity`, () => fixture(code,
      async ({ map, bytes, reset, run }) => {
        map(0xfffff000, READ | WRITE, page([], 0x77));
        map(0, READ | WRITE, page([], 0x88));
        reset({ eax: 0x11223344, esp: 0xfffffffe });
        const result = run(100);
        equal(result.kind, 9, "SS limit violation is a stack fault, not general protection");
        equal(result.error, 0, "stack limit error code");
        equal(result.pc, CODE, "SS fault instruction");
        equal(result.retired, 0, "SS faulting instruction did not complete");
        equal(result.words[0], 0x11223344, "SS failed load preserves register");
        equal(result.words[4], 0xfffffffe, "SS memory fault preserves ESP");
        equal(bytes(0xfffff000)[4095], 0x77, "SS failed store preserves high byte");
        equal(bytes(0)[0], 0x88, "SS failed store preserves low byte");
      }));
  }
  for (const [label, opcode, esp] of [["PUSH", 0x50, 1], ["POP", 0x58, 0xfffffffe]]) {
    await test(`${label} implicit stack limit fault is precise`, () => fixture([opcode],
      async ({ map, bytes, reset, run }) => {
        map(0xfffff000, READ | WRITE, page([], 0x77));
        map(0, READ | WRITE, page([], 0x88));
        reset({ eax: 0x11223344, esp });
        const result = run(100);
        equal(result.kind, 9, "implicit stack limit exception kind");
        equal(result.pc, CODE, "implicit stack fault PC");
        equal(result.retired, 0, "implicit stack instruction did not complete");
        equal(result.words[0], 0x11223344, "failed stack instruction preserves EAX");
        equal(result.words[4], esp, "failed stack instruction preserves ESP");
        equal(bytes(0xfffff000)[4095], 0x77, "failed stack instruction preserves top memory byte");
        equal(bytes(0)[0], 0x88, "failed stack instruction preserves wrapped memory byte");
      }));
  }
  await test("overlength decode is unsupported rather than a manufactured invalid-opcode fault", () => fixture(
    [...Array(15).fill(0x66), 0x90], async ({ reset, run }) => {
      reset({ eax: 0x11223344 });
      const result = run(100);
      equal(result.kind, 3, "decoder length ambiguity remains explicit unsupported behavior");
      equal(result.pc, CODE, "overlength instruction PC");
      equal(result.retired, 0, "overlength instruction not completed");
      equal(result.words[0], 0x11223344, "overlength instruction preserves register");
    }));
  const mmValue = (words, index) => BigInt(words[35 + index * 3]) | BigInt(words[36 + index * 3]) << 32n;
  const setMm = (words, index, value, exponent = 0x3fff) => {
    words[35 + index * 3] = Number(value & 0xffffffffn);
    words[36 + index * 3] = Number(value >> 32n & 0xffffffffn);
    words[37 + index * 3] = exponent;
  };
  const lane = (value, index, width) => value >> BigInt(index * width) & widthMask(width);
  function packedReference(operation, a, b, count = 0) {
    let value = 0n;
    for (let index = 0; index < (operation === "unpack" || operation === "pack" ? 8 : 4); index += 1) {
      let result, width = 16;
      if (operation === "unpack") {
        width = 8; result = lane(index % 2 ? b : a, index >> 1, 8);
      } else if (operation === "pack") {
        width = 8; result = BigInt.asIntN(16, lane(index < 4 ? a : b, index & 3, 16));
        result = result < 0n ? 0n : result > 255n ? 255n : result;
      } else {
        const av = lane(a, index, 16), bv = lane(b, index, 16);
        result = operation === "add" ? av + bv : operation === "sub" ? av - bv
          : operation === "mul" ? av * bv : operation === "xor" ? av ^ bv : BigInt.asIntN(16, av) >> BigInt(Math.min(count, 15));
      }
      value |= (result & widthMask(width)) << BigInt(index * width);
    }
    return value;
  }
  for (const [operation, opcode] of [["unpack", 0x60], ["sub", 0xf9], ["mul", 0xd5], ["add", 0xfd], ["pack", 0x67], ["xor", 0xef]]) {
    await test(`MMX ${operation} direct SIMD lane semantics`, () => fixture([0x0f, opcode, 0xc1], async ({ reset, state, run }) => {
      const pairs = [[0x8000ffff00ff0001n, 0x7fff010000000002n], [0xffffffffffffffffn, 0xffffffffffffffffn], [0n, 0n]];
      for (let n = 0; n < 32; n += 1) pairs.push([BigInt(next()) << 32n | BigInt(next()), BigInt(next()) << 32n | BigInt(next())]);
      for (const [a, b] of pairs) {
        reset({ flags: 0xa93 }); setMm(state(), 0, a); setMm(state(), 1, b, 0x4000);
        state()[60] = 0x7f00; state()[61] = 0xffff;
        const result = run(1);
        equal(mmValue(result.words, 0), packedReference(operation, a, b), "packed integer result");
        equal(mmValue(result.words, 1), b, "source physical register unchanged");
        equal(result.words[37], 0xffff, "written MMX exponent becomes all ones");
        equal(result.words[40], 0x4000, "unwritten source exponent preserved");
        equal(result.words[60], 0x4700, "MMX clears TOP and preserves other status bits");
        equal(result.words[61], 0, "MMX marks every tag valid");
        equal(result.words[9], 0xa93, "MMX does not change EFLAGS");
      }
    }));
  }
  // Intel SDM Vol.2B PUNPCKHBW/HWD/HDQ: interleave the high32 of each
  // raw64 operand. This scalar lane oracle does not use Wasm shuffle indices.
  const unpackHighReference = (destination, source, width) => {
    let value = 0n;
    for (let index = 0; index < 32 / width; index++) {
      const sourceBit = BigInt(32 + index * width), resultBit = BigInt(index * width * 2);
      value |= ((destination >> sourceBit) & widthMask(width)) << resultBit;
      value |= ((source >> sourceBit) & widthMask(width)) << (resultBit + BigInt(width));
    }
    return value;
  };
  const seedUnpackHigh = words => {
    for (let index = 35; index < 100; index++) words[index] = (index * 0x45d9f3b) >>> 0;
    for (let index = 0; index < 8; index++) setMm(words, index, 0x0123456789abcdefn + BigInt(index), 0x3ff0 + index);
    words[59] = 0x37f; words[60] = 0x7f20; words[61] = 0xffff; words[99] = 0x7f81;
  };
  const checkUnpackHigh = (result, before, destination, value, pc) => {
    equal(result.retired, 1, "one high unpack retires");
    const at = 35 + destination * 3;
    for (let index = 0; index < 100; index++) {
      const expected = index === 8 ? pc : index === at ? Number(value & 0xffffffffn)
        : index === at + 1 ? Number(value >> 32n) : index === at + 2 ? 0xffff
          : index === 60 ? before[60] & ~0x3800 : index === 61 ? 0 : before[index];
      equal(result.words[index], expected, "high unpack changes only destination raw64/exponent, TOP and tags");
    }
  };
  const unpackLowMmxWords = (destination, source) => (destination & 0xffffn) | (source & 0xffffn) << 16n
    | (destination >> 16n & 0xffffn) << 32n | (source >> 16n & 0xffffn) << 48n;
  await test("MMX PUNPCKLWD all64 register pairs interleave old low32 and preserve full FP state", async () => {
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
      [0x0f, 0x61, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        for (const [a, b] of [[0xffff000012345678n, 0x0000ffff89abcdefn], [0x01234567ffffffffn, 0x89abcdef00000000n], [0n, 0xffffffffffffffffn]]) {
          reset({ flags: 0xad7 }); seedUnpackHigh(state()); setMm(state(), destination, a, 0x4010 + destination); setMm(state(), source, b, 0x4020 + source);
          const before = Array.from(state()), zero = run(0); for (let n = 0; n < 100; n++) equal(zero.words[n], before[n], "zero budget leaves physical80/tag/TOP untouched");
          checkUnpackHigh(run(1), before, destination, unpackLowMmxWords(mmValue(before, destination), mmValue(before, source)), CODE + 3);
        }
      });
  });
  await test("MMX PUNPCKLWD m32 reads exact four bytes including page last4 through aliases", () => fixture(
    [0x0f, 0x61, 0x3b], async ({ alias, bytes, reset, state, run }) => {
      alias(0x4000, DATA, READ); bytes(DATA).fill(0x5a);
      for (const offset of [0, 1, 7, 4092]) {
        setWord(bytes(DATA), offset, 0xabcd1234); reset({ ebx: 0x4000 + offset, flags: 0xad7 }); seedUnpackHigh(state());
        const before = Array.from(state()), image = bytes(DATA).slice(); checkUnpackHigh(run(1), before, 7, unpackLowMmxWords(mmValue(before, 7), 0xabcd1234n), CODE + 3);
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "narrow unpack only reads memory");
      }
    }));
  for (const present of [false, true]) await test(`MMX PUNPCKLWD page last3 ${present ? "protection" : "absent"} fault and repair preserve completed ADD`, () => fixture(
    [0x83, 0x07, 1, 0x0f, 0x61, 0x0b], async ({ map, protect, bytes, reset, state, run, compile }) => {
      bytes(DATA).set([0x11, 0x22, 0x33], 4093); if (present) map(DATA + 4096, WRITE); setWord(bytes(STACK), 0, 0x7fffffff);
      reset({ ebx: DATA + 4093, edi: STACK, flags: 0x203 }); seedUnpackHigh(state()); const before = Array.from(state()), fault = run(100), flags = before[9] & ~ARITH | arithmetic("add", 32, 0x7fffffff, 1, 0).flags;
      equal(fault.kind, 4, "four-byte source crosses into unavailable page"); equal(fault.address, DATA + 4096, "first inaccessible byte"); equal(fault.error, present ? 5 : 4, "source read fault"); equal(fault.retired, 1, "prior ADD only completed");
      for (let n = 0; n < 100; n++) equal(fault.words[n], n === 8 ? CODE + 3 : n === 9 ? flags : before[n], "fault has no MMX entry or destination effects");
      if (present) protect(DATA + 4096, READ | WRITE); else map(DATA + 4096, READ | WRITE); bytes(DATA + 4096)[0] = 0x44;
      await compile(CODE + 3); const prior = Array.from(state()); checkUnpackHigh(run(1), prior, 1, unpackLowMmxWords(mmValue(prior, 1), 0x44332211n), CODE + 6);
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "repair never replays completed ADD");
    }));
  await test("MMX PUNPCKLWD pending MF precedes an unmapped m32 and exact repaired restart", () => fixture(
    [0x0f, 0x61, 0x0b], async ({ reset, state, run, map, bytes }) => {
      reset({ ebx: 0x6001, flags: 0xad7 }); seedUnpackHigh(state()); state()[59] = 0x37e; state()[60] = 0xb881; const before = Array.from(state()), pending = run(1);
      equal(pending.kind, 14, "pending MF before any memory translation"); equal(pending.retired, 0, "no retired unpack"); for (let n = 0; n < 100; n++) equal(pending.words[n], before[n], "MF preserves complete CPU state");
      state()[59] = 0x37f; const fault = run(1); equal(fault.kind, 4, "masked prior MF exposes missing m32"); equal(fault.address, 0x6001, "unaligned input");
      for (let n = 0; n < 100; n++) equal(fault.words[n], n === 59 ? 0x37f : before[n], "read fault leaves MMX state unchanged");
      map(0x6000, READ); bytes(0x6000).set([1, 2, 3, 4], 1); const prior = Array.from(state()); checkUnpackHigh(run(1), prior, 1, unpackLowMmxWords(mmValue(prior, 1), 0x04030201n), CODE + 3);
    }));
  await test("MMX PUNPCKLWD validates its exact FS offset span before linear base wrap", () => fixture(
    [0x64, 0x0f, 0x61, 0x13], async ({ reset, state, run, bytes }) => {
      setWord(bytes(DATA), 1, 0x44332211);
      for (const [offset, base, kind] of [[DATA + 1, 0, 1], [DATA + 4097, 0xfffff000, 1], [0xfffffffd, DATA + 4, 6]]) {
        reset({ ebx: offset, fs: base, flags: 0xad7 }); seedUnpackHigh(state()); const before = Array.from(state()), result = run(1); equal(result.kind, kind, "m32 FS span contract");
        if (kind === 1) checkUnpackHigh(result, before, 2, unpackLowMmxWords(mmValue(before, 2), 0x44332211n), CODE + 4);
        else for (let n = 0; n < 100; n++) equal(result.words[n], before[n], "invalid offset has no MMX effects");
      }
    }));
  await test("MMX PUNPCKLWD shares raw80 with x87 across EMMS and precise budgets", () => fixture(
    [0xd9, 0xe8, 0x0f, 0x61, 0xf8, 0x0f, 0x77, 0xd9, 0xe8, 0xd9, 0x1b], async ({ reset, state, run, bytes }) => {
      const one = 0x8000000000000000n, pcs = [CODE, CODE + 2, CODE + 5, CODE + 7, CODE + 9, CODE + 11];
      for (let budget = 0; budget <= 5; budget++) {
        reset({ ebx: DATA, flags: 0xad7 }); seedUnpackHigh(state()); state()[60] = 0; state()[61] = 0xffff; setWord(bytes(DATA), 0, 0xdeadbeef);
        const before = Array.from(state()), unpacked = unpackLowMmxWords(one, mmValue(before, 0)), result = run(budget), expected = before.slice(); equal(result.retired, budget, "exact mixed FP/MMX budget");
        const payload = budget >= 4 ? one : budget >= 2 ? unpacked : budget === 1 ? one : mmValue(before, 7);
        expected[8] = pcs[budget]; expected[56] = Number(payload & 0xffffffffn); expected[57] = Number(payload >> 32n); expected[58] = budget >= 4 ? 0x3fff : budget >= 2 ? 0xffff : budget === 1 ? 0x3fff : before[58];
        if (budget) { expected[60] = budget === 1 || budget === 4 ? 0x3800 : 0; expected[61] = budget === 1 || budget === 4 ? 0x3fff : budget === 2 ? 0 : 0xffff;
          expected[62] = budget === 5 ? 0x11b : 0x1e8; expected[63] = budget === 5 ? CODE + 9 : budget >= 4 ? CODE + 7 : CODE; expected[64] = before[15]; if (budget === 5) { expected[65] = DATA; expected[66] = before[23]; } }
        for (let n = 0; n < 100; n++) equal(result.words[n], expected[n], "raw80 and FP metadata reflect each boundary");
        equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true), budget === 5 ? 0x3f800000 : 0xdeadbeef, "new x87 value replaces stale MMX payload");
      }
    }));
  await test("MMX PUNPCKLWD output survives executable-alias store and no-replay rebuild", () => fixture(
    [0x0f, 0x61, 0xc1, 0x0f, 0x7f, 0x03, 0x40], async ({ handle, reset, state, run, bytes, alias, compile }) => {
      alias(0x4000, CODE, READ | WRITE); reset({ ebx: 0x4101, eax: 9, flags: 0xad7 }); seedUnpackHigh(state()); const before = Array.from(state()), packed = unpackLowMmxWords(mmValue(before, 0), mmValue(before, 1)), result = run(100);
      equal(result.kind, 7, "completed MMX store invalidates executable alias"); equal(result.retired, 2, "unpack/store complete once"); equal(result.pc, CODE + 6, "INC not yet executed"); equal(mmValue(result.words, 0), packed, "correct low words saved");
      for (let n = 0; n < 8; n++) equal(bytes(CODE)[0x101 + n], Number(packed >> BigInt(n * 8) & 255n), "all packed result bytes stored");
      check(api.backend_validate(handle, CODE) !== 0, "stale compiled code rejected"); await compile(CODE + 6); setMm(state(), 0, 0n); const resumed = run(1); equal(resumed.words[0], 10, "only next INC runs");
      for (let n = 0; n < 8; n++) equal(bytes(CODE)[0x101 + n], Number(packed >> BigInt(n * 8) & 255n), "resume never repeats unpack/store");
    }, READ | WRITE | EXECUTE | SEALED));
  for (const [code, kind] of [[[0xf0, 0x0f, 0x61, 0x03], 8], [[0x67, 0x0f, 0x61, 0x03], 3], [[0x65, 0x0f, 0x61, 0x03], 3]]) await test("MMX PUNPCKLWD rejected encoding " + code.join("-"), () => fixture(
    code, async ({ reset, state, run }) => { reset({ ebx: 0x5001, flags: 0xad7 }); seedUnpackHigh(state()); const before = Array.from(state()), result = run(1); equal(result.kind, kind, "invalid or explicit unsupported precedes effects"); for (let n = 0; n < 100; n++) equal(result.words[n], before[n], "rejected unpack preserves state"); }));
  for (const [name, opcode, width] of [["HBW", 0x68, 8], ["HWD", 0x69, 16], ["HDQ", 0x6a, 32]]) {
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) {
      await test(`PUNPCK${name} MM${destination},MM${source} high lanes and physical aliases`, () => fixture(
        [0x0f, opcode, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
          const pairs = [[0n, 0n], [0xffffffffffffffffn, 0n], [0x88776655aabbccddn, 0x44332211eeff0012n],
            [0x00000000ffffffffn, 0x0000000012345678n], [0xffffffff00000000n, 0x8000000100000000n]];
          for (let index = 0; index < 4; index++) pairs.push([BigInt(next()) << 32n | BigInt(next()), BigInt(next()) << 32n | BigInt(next())]);
          for (const [a, b] of pairs) {
            reset({ eax: 0x12345678, edx: 0xfedcba98, flags: 0xad7 }); seedUnpackHigh(state());
            setMm(state(), destination, a, 0x4010 + destination); setMm(state(), source, b, 0x4020 + source);
            const before = Array.from(state()), zero = run(0);
            equal(zero.retired, 0, "zero budget does not enter MMX");
            for (let index = 0; index < 100; index++) equal(zero.words[index], before[index], "zero budget preserves full state");
            checkUnpackHigh(run(1), before, destination, unpackHighReference(mmValue(before, destination), mmValue(before, source), width), CODE + 3);
          }
        }));
    }
    await test(`PUNPCK${name} reads unaligned and cross-page m64 through aliases without writing memory`, () => fixture(
      [0x0f, opcode, 0x3b], async ({ map, alias, bytes, reset, state, run }) => {
        map(DATA + 4096, READ); alias(0x4000, DATA, READ); alias(0x5000, DATA + 4096, READ);
        for (const offset of [0, 1, 7, 4088, 4089, 4095]) {
          for (let index = 0; index < 4096; index++) { bytes(DATA)[index] = index * 37 + 19; bytes(DATA + 4096)[index] = index * 17 + 83; }
          const first = bytes(DATA).slice(), second = bytes(DATA + 4096).slice();
          let source = 0n;
          for (let index = 0; index < 8; index++) { const at = offset + index; source |= BigInt(at < 4096 ? first[at] : second[at - 4096]) << BigInt(index * 8); }
          reset({ ebx: 0x4000 + offset, flags: 0xad7 }); seedUnpackHigh(state()); setMm(state(), 7, 0x88776655aabbccddn, 0x4007);
          const before = Array.from(state()); checkUnpackHigh(run(1), before, 7, unpackHighReference(mmValue(before, 7), source, width), CODE + 3);
          for (let index = 0; index < 4096; index++) { equal(bytes(DATA)[index], first[index], "first input page unchanged"); equal(bytes(DATA + 4096)[index], second[index], "second input page unchanged"); }
        }
      }));
    await test(`PUNPCK${name} FS permits unaligned linear wrap but checks the full offset span`, () => fixture(
      [0x64, 0x0f, opcode, 0x13], async ({ reset, state, run, bytes }) => {
        bytes(DATA).set([0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88], 1);
        for (const [offset, base, kind] of [[DATA + 1, 0, 1], [DATA + 4097, 0xfffff000, 1], [0xfffffffc, DATA + 5, 6]]) {
          reset({ ebx: offset, fs: base, flags: 0xad7 }); seedUnpackHigh(state());
          const before = Array.from(state()), result = run(1);
          equal(result.kind, kind, "valid m64 linear wrap or precise segment GP");
          if (kind === 1) checkUnpackHigh(result, before, 2, unpackHighReference(mmValue(before, 2), 0x8877665544332211n, width), CODE + 4);
          else {
            equal(result.error, 0, "offset-span GP has no selector error"); equal(result.retired, 0, "segment fault precedes MMX entry");
            for (let index = 0; index < 100; index++) equal(result.words[index], before[index], "segment fault preserves full state");
          }
        }
      }));
    await test(`PUNPCK${name} crosses x87 and EMMS boundaries without stale physical80 writeback`, () => fixture(
      [0xd9, 0xe8, 0x0f, opcode, 0xf8, 0x0f, 0x77, 0xd9, 0xe8, 0xd9, 0x1b], async ({ reset, state, run, bytes }) => {
        const one = 0x8000000000000000n, pcs = [CODE, CODE + 2, CODE + 5, CODE + 7, CODE + 9, CODE + 11];
        for (let budget = 0; budget <= 5; budget++) {
          reset({ ebx: DATA, flags: 0xad7 }); seedUnpackHigh(state()); state()[60] = 0; state()[61] = 0xffff; setWord(bytes(DATA), 0, 0xdeadbeef);
          const before = Array.from(state()), unpacked = unpackHighReference(one, mmValue(before, 0), width), result = run(budget), expected = before.slice();
          equal(result.retired, budget, "each mixed FP/MMX boundary respects budget");
          const payload = budget >= 4 ? one : budget >= 2 ? unpacked : budget === 1 ? one : mmValue(before, 7);
          expected[8] = pcs[budget]; expected[56] = Number(payload & 0xffffffffn); expected[57] = Number(payload >> 32n);
          expected[58] = budget >= 4 ? 0x3fff : budget >= 2 ? 0xffff : budget === 1 ? 0x3fff : before[58];
          if (budget) {
            expected[60] = budget === 1 || budget === 4 ? 0x3800 : 0; expected[61] = budget === 1 || budget === 4 ? 0x3fff : budget === 2 ? 0 : 0xffff;
            expected[62] = budget === 5 ? 0x11b : 0x1e8; expected[63] = budget === 5 ? CODE + 9 : budget >= 4 ? CODE + 7 : CODE; expected[64] = before[15];
            if (budget === 5) { expected[65] = DATA; expected[66] = before[23]; }
          }
          for (let index = 0; index < 100; index++) equal(result.words[index], expected[index], "mixed x87/unpack state matches exact boundary");
          equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true), budget === 5 ? 0x3f800000 : 0xdeadbeef, "following x87 store sees fresh1.0 rather than stale packed data");
        }
      }));
    for (const [prefix, kind, label] of [[0xf0, 8, "LOCK"], ...(opcode === 0x69 ? [] : [[0x66, 3, "unimplemented XMM"]])]) await test(`PUNPCK${name} ${label} cannot enter the MMX bank`, () => fixture(
      [prefix, 0x0f, opcode, 0x03], async ({ reset, state, run }) => {
        reset({ ebx: 0x6000, flags: 0xad7 }); seedUnpackHigh(state()); state()[59] = 0x37e; state()[60] = 0xb881;
        const before = Array.from(state()), result = run(1);
        equal(result.kind, kind, "unsupported/invalid encoding precedes MF or absent data operand"); equal(result.retired, 0, "rejected instruction has no effects");
        for (let index = 0; index < 100; index++) equal(result.words[index], before[index], "rejected form preserves full architectural state");
      }));
    await test(`PUNPCK${name} pending MF precedes absent operand and resumes after repair`, () => fixture(
      [0x0f, opcode, 0x0b], async ({ reset, state, run, map, bytes }) => {
        reset({ ebx: 0x6001, flags: 0xad7 }); seedUnpackHigh(state()); state()[59] = 0x37e; state()[60] = 0xb881;
        const before = Array.from(state()), pending = run(1);
        equal(pending.kind, 14, "pending MF precedes memory read"); equal(pending.error, 1, "pending invalid cause"); equal(pending.retired, 0, "no high unpack completed");
        for (let index = 0; index < 100; index++) equal(pending.words[index], before[index], "pending MF leaves full state intact");
        state()[59] = 0x37f; const fault = run(1); equal(fault.kind, 4, "masking MF exposes absent m64"); equal(fault.address, 0x6001, "missing operand address");
        for (let index = 0; index < 100; index++) equal(fault.words[index], index === 59 ? 0x37f : before[index], "memory fault has no MMX entry effects");
        map(0x6000, READ); bytes(0x6000).set([1, 2, 3, 4, 5, 6, 7, 8], 1);
        const repaired = Array.from(state()); checkUnpackHigh(run(1), repaired, 1, unpackHighReference(mmValue(repaired, 1), 0x0807060504030201n, width), CODE + 3);
      }));
    for (const present of [false, true]) await test(`PUNPCK${name} ${present ? "protection" : "unmapped"} page fault preserves completed prefix and resumes once`, () => fixture(
      [0x83, 0x07, 1, 0x0f, opcode, 0x0b], async ({ map, protect, bytes, reset, state, run, compile }) => {
        bytes(DATA).set([0x11, 0x22, 0x33], 4093); if (present) map(DATA + 4096, WRITE);
        setWord(bytes(STACK), 0, 0x7fffffff); reset({ ebx: DATA + 4093, edi: STACK, flags: 0x203 }); seedUnpackHigh(state());
        const before = Array.from(state()), input = bytes(DATA).slice(), fault = run(100);
        const flags = (before[9] & ~ARITH) | arithmetic("add", 32, 0x7fffffff, 1, 0).flags;
        equal(fault.kind, 4, "m64 read faults precisely"); equal(fault.address, DATA + 4096, "first denied input byte"); equal(fault.error, present ? 5 : 4, "user-read page fault cause"); equal(fault.retired, 1, "only prefix ADD retired");
        for (let index = 0; index < 100; index++) equal(fault.words[index], index === 8 ? CODE + 3 : index === 9 ? flags : index === 10 ? (before[10] | ARITH) >>> 0 : before[index], "failed high unpack preserves all state and prior ADD effects");
        for (let index = 0; index < 4096; index++) equal(bytes(DATA)[index], input[index], "failed read cannot dirty source bytes");
        if (present) protect(DATA + 4096, READ); else map(DATA + 4096, READ);
        bytes(DATA + 4096).set([0x44, 0x55, 0x66, 0x77, 0x88]); await compile(CODE + 3);
        const repaired = Array.from(state()); checkUnpackHigh(run(1), repaired, 1, unpackHighReference(mmValue(repaired, 1), 0x8877665544332211n, width), CODE + 6);
        equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "completed dirty prefix never repeats");
      }));
    await test(`PUNPCK${name} result commits through executable alias store before exact SMC resume`, () => fixture(
      [0x0f, opcode, 0xc1, 0x0f, 0xe7, 0x03, 0x40], async ({ alias, reset, state, run, bytes, compile }) => {
        alias(0x4000, CODE, READ | WRITE); reset({ eax: 9, ebx: 0x4103, flags: 0xad7 }); seedUnpackHigh(state());
        setMm(state(), 0, 0x88776655aabbccddn); setMm(state(), 1, 0x44332211eeff0012n, 0x4001);
        const expected = unpackHighReference(mmValue(state(), 0), mmValue(state(), 1), width), result = run(100);
        equal(result.kind, 7, "complete executable-alias write exits for SMC"); equal(result.retired, 2, "high unpack and full store complete"); equal(result.pc, CODE + 6, "INC remains unexecuted"); equal(result.words[0], 9, "SMC stops before INC"); equal(result.words[9], 0xad7, "unpack/store preserve flags");
        equal(mmValue(result.words, 0), expected, "unpack result survives publication"); equal(result.words[37], 0xffff, "destination MMX exponent"); equal(result.words[40], 0x4001, "source raw80 exponent unchanged");
        for (let index = 0; index < 8; index++) equal(bytes(CODE)[0x103 + index], Number(expected >> BigInt(index * 8) & 255n), "exact committed unpack bytes");
        await compile(CODE + 6); setMm(state(), 0, 0n); const resumed = run(1);
        equal(resumed.words[0], 10, "following INC executes once"); equal(mmValue(resumed.words, 0), 0n, "unpack is never replayed");
        for (let index = 0; index < 8; index++) equal(bytes(CODE)[0x103 + index], Number(expected >> BigInt(index * 8) & 255n), "committed store is never replayed");
      }, READ | WRITE | EXECUTE | SEALED));
  }
  for(let destination=0;destination<8;destination++)for(const source of [destination,(destination+1)&7])await test(`PXOR MM${destination},MM${source} preserves physical aliases and enters MMX even for zero`,()=>fixture(
    [0x0f,0xef,0xc0|(destination<<3)|source],async({reset,state,run})=>{
      for(const value of [0n,0xffffffffffffffffn,0x7ff80001fedcba98n,BigInt(next())<<32n|BigInt(next())]){
        reset({eax:0x12345678,edx:0xfedcba98,flags:0xad7});
        for(let n=35;n<100;n++)state()[n]=(0x45d9f3b*n)>>>0;for(let r=0;r<8;r++)setMm(state(),r,0x0123456789abcdefn+BigInt(r),0x3ff0+r);
        setMm(state(),destination,value,0x4010+destination);state()[59]=0x37f;state()[60]=0x7f20;state()[61]=0xffff;state()[99]=0x7f81;
        const before=Array.from(state()),expected=mmValue(before,destination)^mmValue(before,source),zero=run(0);
        equal(zero.retired,0,"zero budget preserves the MMX instruction");for(let n=0;n<100;n++)equal(zero.words[n],before[n],"zero budget leaves all architectural state unchanged");
        const result=run(1);equal(result.retired,1,"raw64 XOR retires once");equal(result.pc,CODE+3,"XOR continuation");
        for(let n=0;n<100;n++){
          const at=35+destination*3,expectedWord=n===8?CODE+3:n===at?Number(expected&0xffffffffn):n===at+1?Number(expected>>32n):n===at+2?0xffff:n===60?0x4720:n===61?0:before[n];
          equal(result.words[n],expectedWord,"only destination raw64/exponent and MMX TOP/tags change");
        }
      }
    }));
  for(const memory of [false,true])await test(`PXOR MMX ${memory?"memory":"self-zero"} cannot erase a pending x87 exception`,()=>fixture(
    memory?[0x0f,0xef,0x03]:[0x0f,0xef,0xc0],async({reset,state,run,map,bytes})=>{
      reset({ebx:0x6001,flags:0xad7});setMm(state(),0,0x8877665544332211n,0x4000);state()[59]=0x37e;state()[60]=0xb881;state()[61]=0xffff;
      const before=Array.from(state()),pending=run(1);equal(pending.kind,14,"pending MF precedes zero idiom or absent operand");equal(pending.error,1,"pending invalid cause");equal(pending.retired,0,"fault has no completed XOR");for(let n=0;n<100;n++)equal(pending.words[n],before[n],"pending MF preserves full CPU state");
      state()[59]=0x37f;if(memory){const fault=run(1);equal(fault.kind,4,"masking pending MF reveals memory fault");equal(fault.address,0x6001,"unaligned missing operand address");equal(fault.words[60],0xb881,"read fault retains TOP");equal(fault.words[61],0xffff,"read fault retains tags");map(0x6000,READ|WRITE);bytes(0x6000).set([1,2,3,4,5,6,7,8],1);}
      const done=run(1);equal(done.retired,1,"repair executes XOR once");equal(mmValue(done.words,0),memory?0x8877665544332211n^0x0807060504030201n:0n,"repaired exact raw64 result");equal(done.words[37],0xffff,"even self-zero writes MMX exponent");equal(done.words[60],0x8081,"successful MMX only clears TOP");equal(done.words[61],0,"successful MMX marks all tags valid");
    }));
  await test("PXOR MMX reads exactly8bytes at unaligned and page-crossing addresses",()=>fixture(
    [0x0f,0xef,0x3b],async({reset,state,run,map,bytes})=>{
      map(DATA+4096,READ|WRITE);for(let n=0;n<8192;n++)bytes(n<4096?DATA:DATA+4096)[n&4095]=(n*29+7)&255;
      for(const offset of [0,1,7,4088,4089,4095]){
        reset({ebx:DATA+offset,flags:0xad7});setMm(state(),7,0x8877665544332211n,0x4007);state()[60]=0x3800;state()[61]=0xffff;const before=Array.from(state());let operand=0n;for(let n=0;n<8;n++){const at=offset+n;operand|=BigInt(bytes(at<4096?DATA:DATA+4096)[at&4095])<<BigInt(n*8);}
        const result=run(1),expected=0x8877665544332211n^operand;equal(result.retired,1,"unaligned m64 XOR succeeds");equal(mmValue(result.words,7),expected,"full raw8byte operand");equal(result.words[58],0xffff,"destination MM7 physical exponent");equal(result.words[60],0,"clears TOP independently of physical MM7");equal(result.words[61],0,"enters MMX state");for(let n=0;n<100;n++)if(![8,56,57,58,60,61].includes(n))equal(result.words[n],before[n],"read XOR preserves all unrelated GPR/FP/vector/flags state");
      }
    }));
  for(const present of [false,true])await test(`PXOR MMX complete read ${present?"protection":"missing-page"} fault preserves prior scalar effects and resumes exactly`,()=>fixture(
    [0x83,0x07,1,0x0f,0xef,0x0b],async({map,protect,bytes,reset,state,run,compile})=>{
      bytes(DATA).set([0x11,0x22,0x33],4093);if(present)map(DATA+4096,WRITE,page([],0x44));setWord(bytes(STACK),0,0x7fffffff);
      reset({ebx:DATA+4093,edi:STACK,flags:0x203});setMm(state(),1,0xffeeddccbbaa9988n,0x4001);state()[60]=0x3800;state()[61]=0xffff;const before=Array.from(state()),fault=run(100);
      equal(fault.kind,4,"second page read fault");equal(fault.address,DATA+4096,"first denied read byte");equal(fault.error,present?5:4,"precise read protection/presence code");equal(fault.retired,1,"prefix ADD alone completes");equal(fault.pc,CODE+3,"precise XOR restart");for(let n=35;n<100;n++)equal(fault.words[n],before[n],"failed read has no MMX state entry or destination mutation");equal(fault.words[9]&ARITH,arithmetic("add",32,0x7fffffff,1,0).flags,"prefix flags remain");
      if(present)protect(DATA+4096,READ|WRITE);else map(DATA+4096,READ|WRITE);bytes(DATA+4096).set([0x44,0x55,0x66,0x77,0x88]);await compile(CODE+3);const done=run(1);
      equal(done.retired,1,"only repaired XOR retires");equal(mmValue(done.words,1),0xffeeddccbbaa9988n^0x8877665544332211n,"exact completed XOR");equal(done.words[40],0xffff,"repaired exponent");equal(done.words[60],0,"repaired TOP");equal(done.words[61],0,"repaired tag");equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset).getUint32(0,true),0x80000000,"earlier dirty store never repeats");equal(done.words[9],fault.words[9],"repair leaves prefix flags");
    }));
  await test("PXOR MMX result survives subsequent executable-alias store and exact SMC continuation",()=>fixture(
    [0x0f,0xef,0xc1,0x0f,0xe7,0x03,0x40],async({alias,reset,state,run,bytes,compile})=>{
      alias(0x4000,CODE,READ|WRITE);reset({eax:9,ebx:0x4103,flags:0xad7});setMm(state(),0,0x8877665544332211n);setMm(state(),1,0x1122334455667788n,0x4001);state()[60]=0x3800;state()[61]=0xffff;const expected=0x9955551111555599n,result=run(100);
      equal(result.kind,7,"following store ends at SMC boundary");equal(result.retired,2,"XOR and full store completed");equal(result.pc,CODE+6,"INC remains unexecuted");equal(result.words[0],9,"no operation after SMC");equal(mmValue(result.words,0),expected,"XOR result survives store publication");equal(result.words[37],0xffff,"XOR exponent retained");equal(result.words[40],0x4001,"source exponent unchanged");for(let n=0;n<8;n++)equal(bytes(CODE)[0x103+n],Number(expected>>BigInt(n*8)&255n),"stored exact XOR bytes");
      await compile(CODE+6);setMm(state(),0,0n);const resumed=run(1);equal(resumed.words[0],10,"INC executes once after SMC");equal(mmValue(resumed.words,0),0n,"neither XOR nor store replayed");for(let n=0;n<8;n++)equal(bytes(CODE)[0x103+n],Number(expected>>BigInt(n*8)&255n),"old committed bytes remain after changed MM source");
    },READ|WRITE|EXECUTE|SEALED));
  const psrlwReference = (value, count) => {
    const shift = BigInt(count);
    if (shift > 15n) return 0n;
    let result = 0n;
    for (let index = 0; index < 4; index++) result |= ((value >> BigInt(index * 16) & 0xffffn) >> shift) << BigInt(index * 16);
    return result;
  };
  const seedPsrlw = words => {
    for(let n=35;n<100;n++)words[n]=(n*0x45d9f3b)>>>0;
    for(let index=0;index<8;index++)setMm(words,index,0x0123456789abcdefn+BigInt(index),0x3ff0+index);
    words[59]=0x37f;words[60]=0x7f20;words[61]=0xffff;words[62]=0x321;words[63]=0x12345678;words[64]=0x23;words[65]=0x87654321;words[66]=0x2b;words[99]=0x7f81;words[10]=0xfffff72a;
  };
  const checkPsrlw = (result,before,destination,expected,pc) => {
    const at=35+destination*3;
    equal(result.retired,1,"one packed logical shift completes");
    for(let n=0;n<100;n++)equal(result.words[n],n===8?pc:n===at?Number(expected&0xffffffffn):n===at+1?Number(expected>>32n):n===at+2?0xffff:n===60?before[60]&~0x3800:n===61?0:before[n],"PSRLW changes only destination physical64/exponent and MMX TOP/tag state");
  };
  const immediateShiftCases = new Set(Array.from({length:256},(_,count)=>count*8+(count&7)));
  for(const count of [0,15,16,32,255])for(let destination=0;destination<8;destination++)immediateShiftCases.add(count*8+destination);
  for(const key of immediateShiftCases){const count=key>>>3,destination=key&7;
    await test(`PSRLW MM${destination} immediate${count} uses unsigned16 lanes without modulo count`,()=>fixture(
      [0x0f,0x71,0xd0|destination,count],async({reset,state,run})=>{
        for(const value of [0n,0xffffffffffffffffn,0x8000ffff7fff0001n,BigInt(next())<<32n|BigInt(next())]){
          reset({eax:0x12345678,edx:0xfedcba98,flags:0xad7});seedPsrlw(state());setMm(state(),destination,value,0x4010+destination);const before=Array.from(state()),zero=run(0);
          equal(zero.retired,0,"zero budget leaves packed instruction pending");for(let n=0;n<100;n++)equal(zero.words[n],before[n],"zero budget has no MMX entry effects");
          checkPsrlw(run(1),before,destination,psrlwReference(value,count),CODE+4);
        }
      }));
  }
  const variableShiftCounts=[0n,1n,7n,15n,16n,31n,32n,255n,256n,65535n,0x10000n,0xffffffffn,0x100000000n,0x100000001n,0x8000000000000000n,0xffffffffffffffffn];
  for(let destination=0;destination<8;destination++)for(const source of [destination,(destination+1)&7])await test(`PSRLW MM${destination},MM${source} consumes the complete unsigned64 count before writing aliases`,()=>fixture(
    [0x0f,0xd1,0xc0|(destination<<3)|source],async({reset,state,run})=>{
      for(const count of variableShiftCounts){reset({eax:0x12345678,ecx:0x87654321,flags:0xad7});seedPsrlw(state());setMm(state(),destination,0x8000ffff7fff0001n,0x4010+destination);setMm(state(),source,count,0x4020+source);const before=Array.from(state());checkPsrlw(run(1),before,destination,psrlwReference(mmValue(before,destination),mmValue(before,source)),CODE+3);}
    }));
  for(const [code,memory,label]of [
    [[0x0f,0x71,0xd0,0],false,"immediate zero"],[[0x0f,0x71,0xd0,255],false,"immediate oversized"],
    [[0x0f,0xd1,0xc1],false,"register oversized"],[[0x0f,0xd1,0x03],true,"missing memory count"]])await test(`PSRLW ${label} cannot bypass pending x87 MF`,()=>fixture(
    code,async({reset,state,run,map,bytes})=>{
      reset({ebx:0x6001,flags:0xad7});seedPsrlw(state());setMm(state(),0,0x8000ffff7fff0001n,0x4000);setMm(state(),1,0x100000000n,0x4001);state()[59]=0x37e;state()[60]=0xb881;const before=Array.from(state()),pending=run(1);
      equal(pending.kind,14,"pending MF is checked before zero/result shortcut or operand access");equal(pending.error,1,"pending invalid cause retained");equal(pending.retired,0,"pending exception suppresses packed operation");for(let n=0;n<100;n++)equal(pending.words[n],before[n],"pending MF leaves complete architectural state intact");
      state()[59]=0x37f;
      if(memory){const fault=run(1);equal(fault.kind,4,"masking x87 exception reveals missing count operand");equal(fault.address,0x6001,"unaligned operand fault location");for(let n=0;n<100;n++)equal(fault.words[n],n===59?0x37f:before[n],"count fault has no MMX entry effects");map(0x6000,READ|WRITE);bytes(0x6000)[1]=1;}
      const repairedBefore=Array.from(state()),count=memory?1n:code[1]===0x71?BigInt(code[3]):0x100000000n;checkPsrlw(run(1),repairedBefore,0,psrlwReference(mmValue(repairedBefore,0),count),CODE+code.length);
    }));
  await test("PSRLW memory count reads full8bytes through unaligned, cross-page and alias mappings",()=>fixture(
    [0x0f,0xd1,0x3b],async({map,alias,bytes,reset,state,run})=>{
      map(DATA+4096,READ);alias(0x4000,DATA,READ);alias(0x5000,DATA+4096,READ);
      for(const offset of [1,4088,4089,4095])for(const count of [0n,1n,15n,16n,32n,0x100000000n,0x100000001n,0xffffffffffffffffn]){
        bytes(DATA).fill(0x55);bytes(DATA+4096).fill(0x66);for(let n=0;n<8;n++){const address=offset+n;bytes(address<4096?DATA:DATA+4096)[address&4095]=Number(count>>BigInt(n*8)&255n);}
        reset({ebx:0x4000+offset,flags:0xad7});seedPsrlw(state());setMm(state(),7,0x8000ffff7fff0001n,0x4007);const before=Array.from(state()),first=bytes(DATA).slice(),second=bytes(DATA+4096).slice();checkPsrlw(run(1),before,7,psrlwReference(mmValue(before,7),count),CODE+3);
        for(let n=0;n<4096;n++){equal(bytes(DATA)[n],first[n],"first count page unchanged");equal(bytes(DATA+4096)[n],second[n],"second count page unchanged");}
      }
    }));
  for(const present of [false,true])await test(`PSRLW whole memory count ${present?'protection':'absent'} fault precedes even a known oversized low count`,()=>fixture(
    [0x83,0x07,1,0x0f,0xd1,0x0b],async({map,protect,bytes,reset,state,run,compile})=>{
      bytes(DATA).set([16,0,0],4093);if(present)map(DATA+4096,WRITE);setWord(bytes(STACK),0,0x7fffffff);reset({ebx:DATA+4093,edi:STACK,flags:0x203});seedPsrlw(state());setMm(state(),1,0x8000ffff7fff0001n,0x4001);const before=Array.from(state()),data=bytes(DATA).slice(),fault=run(100),flags=(before[9]&~ARITH)|arithmetic('add',32,0x7fffffff,1,0).flags;
      equal(fault.kind,4,"whole8byte operand must be readable before zero result");equal(fault.address,DATA+4096,"first denied count byte");equal(fault.error,present?5:4,"precise user-read page fault");equal(fault.retired,1,"only prefix store retires");for(let n=0;n<100;n++)equal(fault.words[n],n===8?CODE+3:n===9?flags:n===10?(before[10]|ARITH)>>>0:before[n],"count fault preserves packed state and preceding scalar effects");
      for(let n=0;n<4096;n++)equal(bytes(DATA)[n],data[n],"failed count read never writes guest bytes");
      if(present)protect(DATA+4096,READ);else map(DATA+4096,READ);await compile(CODE+3);const repairedBefore=Array.from(state());checkPsrlw(run(1),repairedBefore,1,0n,CODE+6);equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset).getUint32(0,true),0x80000000,"preceding ADD store is never replayed");
    }));
  await test("PSRLW crosses precise x87 and EMMS boundaries without stale physical-register writeback",()=>fixture(
    [0xd9,0xe8,0x0f,0x71,0xd7,15,0x0f,0x77,0xd9,0xe8,0xd9,0x1b],async({reset,state,run,bytes})=>{
      const one=0x8000000000000000n,shifted=psrlwReference(one,15),pcs=[CODE,CODE+2,CODE+6,CODE+8,CODE+10,CODE+12];
      for(let budget=0;budget<=5;budget++){
        reset({ebx:DATA,flags:0xad7});seedPsrlw(state());state()[60]=0;state()[61]=0xffff;setWord(bytes(DATA),0,0xdeadbeef);const before=Array.from(state()),result=run(budget);equal(result.retired,budget,"each mixed x87/MMX boundary respects exact budget");
        const payload=budget>=4?one:budget>=2?shifted:budget===1?one:mmValue(before,7),exponent=budget>=4?0x3fff:budget>=2?0xffff:budget===1?0x3fff:before[58];
        const expected=before.slice();expected[8]=pcs[budget];expected[56]=Number(payload&0xffffffffn);expected[57]=Number(payload>>32n);expected[58]=exponent;
        if(budget){expected[60]=budget===1||budget===4?0x3800:0;expected[61]=budget===1||budget===4?0x3fff:budget===2?0:0xffff;expected[62]=budget===5?0x11b:0x1e8;expected[63]=budget===5?CODE+10:budget>=4?CODE+8:CODE;expected[64]=before[15];if(budget===5){expected[65]=DATA;expected[66]=before[23];}}
        for(let n=0;n<100;n++)equal(result.words[n],expected[n],"mixed FP/MMX state and environment match exact operation boundary");equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(0,true),budget===5?0x3f800000:0xdeadbeef,"following x87 store uses new1.0 value, never stale shifted bits");
      }
    }));
  for(const [code,kind,label]of [[[0xf0,0x0f,0x71,0xd0,0],8,"LOCK immediate"],[[0xf0,0x0f,0xd1,0x03],8,"LOCK memory"]])await test(`PSRLW ${label} does not silently enter wrong register bank`,()=>fixture(
    code,async({reset,state,run})=>{reset({ebx:0x6000,flags:0xad7});seedPsrlw(state());const before=Array.from(state()),result=run(1);equal(result.kind,kind,"exact unsupported or invalid instruction contract");equal(result.retired,0,"rejected instruction has no completed effects");for(let n=0;n<100;n++)equal(result.words[n],before[n],"rejected form preserves all architectural state");}));
  for (const count of [0, 1, 7, 15, 16, 31, 255]) {
    await test(`MMX PSRAW immediate ${count} saturates shift count`, () => fixture([0x0f, 0x71, 0xe0, count], async ({ reset, state, run }) => {
      const a = 0x8000ffff7fff0001n;
      reset(); setMm(state(), 0, a);
      equal(mmValue(run(1).words, 0), packedReference("sar", a, 0n, count), "arithmetic packed shift");
    }));
  }
  await test("MOVD and EMMS preserve physical x87 aliases and environment", () => fixture(
    [0x0f, 0x6e, 0xc0, 0x0f, 0x7e, 0xc3, 0x0f, 0x77], async ({ reset, state, run }) => {
      reset({ eax: 0xfedcba98 });
      for (let n = 0; n < 8; n += 1) setMm(state(), n, 0x1234567890abcdefn + BigInt(n), 0x3fff + n);
      state()[60] = 0x7f00;
      for (let n = 62; n < 67; n += 1) state()[n] = 0x12340000 + n;
      const result = run(100);
      equal(result.retired, 3, "MOVD input/output and EMMS retire");
      equal(result.words[3], 0xfedcba98, "MOVD reads low 32 bits into GPR");
      equal(mmValue(result.words, 0), 0xfedcba98n, "MOVD input clears upper 32 MMX bits");
      equal(result.words[37], 0xffff, "MOVD sets written exponent");
      for (let n = 1; n < 8; n += 1) {
        equal(mmValue(result.words, n), 0x1234567890abcdefn + BigInt(n), "EMMS preserves payload");
        equal(result.words[37 + n * 3], 0x3fff + n, "EMMS preserves unwritten exponent");
      }
      equal(result.words[60], 0x4700, "EMMS TOP and non-TOP status");
      equal(result.words[61], 0xffff, "EMMS empties tags");
      for (let n = 62; n < 67; n += 1) equal(result.words[n], 0x12340000 + n, "MMX preserves x87 environment");
    }));
  for (const write of [false, true]) {
    await test(`MMX 64-bit ${write ? "store" : "load"} fault preserves full prior state`, () => fixture(
      [0x0f, write ? 0x7f : 0x6f, 0x03], async ({ map, bytes, state, reset, run, protect }) => {
        map(DATA, READ | WRITE, page([], 0x55));
        if (write) map(DATA + 4096, READ, page([], 0x66));
        reset({ ebx: DATA + 4092, flags: 0xa93 });
        setMm(state(), 0, 0x8877665544332211n); state()[60] = 0x3800; state()[61] = 0xffff;
        const fault = run(100);
        equal(fault.kind, 4, "cross-page MMX page fault");
        equal(fault.address, DATA + 4096, "MMX fault at first unavailable byte");
        equal(fault.retired, 0, "MMX fault has no retirement");
        equal(mmValue(fault.words, 0), 0x8877665544332211n, "MMX fault preserves register payload");
        equal(fault.words[37], 0x3fff, "MMX fault preserves exponent");
        equal(fault.words[60], 0x3800, "MMX fault preserves TOP");
        equal(fault.words[61], 0xffff, "MMX fault preserves tags");
        equal(bytes(DATA)[4092], 0x55, "failed 64-bit store cannot partially write");
        if (write) {
          protect(DATA + 4096, READ | WRITE);
          const success = run(1);
          equal(success.retired, 1, "resumed MMX store completes once");
          equal(bytes(DATA)[4092], 0x11, "resumed low byte");
          equal(bytes(DATA + 4096)[3], 0x88, "resumed high byte");
          equal(success.words[37], 0x3fff, "MMX store does not write source exponent");
          equal(success.words[61], 0, "completed MMX store marks tags valid");
        }
      }));
  }
  for(let source=0;source<8;source++)await test(`MOVNTQ MM${source} stores raw64 at aligned, unaligned and cross-page destinations`,()=>fixture(
    [0x0f,0xe7,0x03|(source<<3)],async({map,bytes,reset,state,run})=>{
      map(DATA+4096,READ|WRITE);const expected=0x7ff80001fedcba98n+BigInt(source),output=Array.from({length:8},(_,n)=>Number(expected>>BigInt(n*8)&255n));
      for(const offset of [0,1,7,4089,4095]){
        bytes(DATA).fill(0x55);bytes(DATA+4096).fill(0x66);reset({ebx:DATA+offset,flags:0xad7});
        for(let r=0;r<8;r++)setMm(state(),r,0x0123456789abcdefn+BigInt(r),0x3fff+r);setMm(state(),source,expected,0x4000+source);
        state()[59]=0x37f;state()[60]=0x7f20;state()[61]=0xffff;for(let n=62;n<100;n++)state()[n]=n===99?0x7f81:(n*0x1234567)>>>0;
        const before=Array.from(state()),zero=run(0);equal(zero.retired,0,"zero budget cannot enter MMX state");for(let n=35;n<100;n++)equal(zero.words[n],before[n],"zero budget preserves raw FP/vector state");
        const result=run(1);equal(result.retired,1,"unaligned non-temporal store completes");equal(result.pc,CODE+3,"completed store continuation");equal(result.words[9],0xad7,"MOVNTQ preserves integer flags");
        for(let n=35;n<100;n++)equal(result.words[n],n===60?0x4720:n===61?0:before[n],"store enters MMX without overwriting any physical80 payload, exponent or environment");
        for(let n=0;n<8192;n++)equal(bytes(n<4096?DATA:DATA+4096)[n&4095],n>=offset&&n<offset+8?output[n-offset]:n<4096?0x55:0x66,"exact8bytes with no alignment rounding or partial overwrite");
      }
    }));
  await test("MOVNTQ cross-page protection fault preserves MMX entry and earlier scalar effects",()=>fixture(
    [0x83,0x07,1,0x0f,0xe7,0x0b],async({map,protect,bytes,reset,state,run,compile})=>{
      bytes(DATA).fill(0x55);map(DATA+4096,READ,page([],0x66));setWord(bytes(STACK),0,0x7fffffff);
      reset({ebx:DATA+4093,edi:STACK,flags:0x203});setMm(state(),1,0x8877665544332211n,0x4001);state()[60]=0x3800;state()[61]=0xffff;const before=Array.from(state()),fault=run(100);
      equal(fault.kind,4,"second page denies write");equal(fault.error,7,"present user-write protection fault");equal(fault.address,DATA+4096,"first denied byte");equal(fault.retired,1,"only earlier ADD completes");equal(fault.pc,CODE+3,"MMX store remains faulting instruction");
      for(let n=35;n<100;n++)equal(fault.words[n],before[n],"failed MOVNTQ does not enter MMX or mutate state");equal(fault.words[9]&ARITH,arithmetic("add",32,0x7fffffff,1,0).flags,"earlier flags retained");equal(fault.words[10]&ARITH,ARITH,"failed store does not alter defined flags");
      for(let n=0;n<8;n++){const at=4093+n;equal(bytes(at<4096?DATA:DATA+4096)[at&4095],at<4096?0x55:0x66,"no partial8byte store");}
      protect(DATA+4096,READ|WRITE);await compile(CODE+3);const repaired=run(1);equal(repaired.retired,1,"only repaired store retires");equal(repaired.words[60],0,"completed store clears TOP");equal(repaired.words[61],0,"completed store marks MMX tags valid");equal(repaired.words[40],0x4001,"store preserves source exponent");
      for(let n=0;n<8;n++){const at=4093+n;equal(bytes(at<4096?DATA:DATA+4096)[at&4095],0x11*(n+1),"repaired full raw bytes");}equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset).getUint32(0,true),0x80000000,"earlier store never repeats");
    }));
  await test("MOVNTQ pending x87 MF precedes memory fault and does not erase TOP or tags",()=>fixture(
    [0xff,0x07,0x0f,0xe7,0x03],async({reset,state,run,bytes,map,compile})=>{
      reset({edi:STACK,ebx:0x6003});setMm(state(),0,0x8877665544332211n,0x3fff);state()[59]=0x37e;state()[60]=0xb881;state()[61]=0xffff;
      const before=Array.from(state().slice(35,100)),pending=run(100);equal(pending.kind,14,"pending MF precedes absent unaligned destination");equal(pending.error,1,"pending invalid cause retained");equal(pending.retired,1,"only prior scalar increment completes");equal(pending.pc,CODE+2,"pending instruction exactPC");equal(bytes(STACK)[0],1,"prefix increment retained");for(let n=0;n<before.length;n++)equal(pending.words[35+n],before[n],"pending MF leaves complete FP state untouched");
      state()[59]=0x37f;await compile(CODE+2);const memory=run(1);equal(memory.kind,4,"masking pending exception reveals absent destination");equal(memory.address,0x6003,"exact unaligned missing address");equal(memory.retired,0,"failed store unretired");equal(memory.words[60],0xb881,"memory fault retains TOP/status");equal(memory.words[61],0xffff,"memory fault retains tags");
      map(0x6000,READ|WRITE);const repaired=run(1);equal(repaired.retired,1,"repair executes store once");equal(repaired.words[60],0x8081,"successful MMX entry only clears TOP");equal(repaired.words[61],0,"successful MMX entry marks tags valid");equal(bytes(STACK)[0],1,"prefix increment not replayed");for(let n=0;n<8;n++)equal(bytes(0x6000)[3+n],0x11*(n+1),"exact final store bytes");
    }));
  await test("MOVNTQ checks complete two-page dirty capacity before write and MMX state entry",()=>fixture(
    [0x0f,0xe7,0x03],async({handle,map,bytes,reset,state,run})=>{
      map(DATA+4096,READ|WRITE);reset({ebx:DATA+4093,flags:0xad7});setMm(state(),0,0x8877665544332211n);state()[60]=0x3800;state()[61]=0xffff;const before=Array.from(state());
      const header=()=>new Uint32Array(api.memory.buffer,api.backend_table(handle)-16,4),capacity=header()[2];header()[2]=1;
      const failed=run(1,()=>{header()[2]=capacity;});equal(failed.kind,10,"insufficient whole-store journal capacity is explicit");equal(failed.retired,0,"resource failure does not retire");for(let n=35;n<100;n++)equal(failed.words[n],before[n],"resource failure cannot enter MMX");equal(failed.words[9],0xad7,"resource failure preserves flags");
      for(let n=0;n<8;n++){const at=4093+n;equal(bytes(at<4096?DATA:DATA+4096)[at&4095],0,"resource failure changes no bytes");}
      const repaired=run(1);equal(repaired.retired,1,"sufficient capacity allows exactly one store");equal(repaired.words[61],0,"successful retry enters MMX");
    }));
  await test("MOVNTQ wrapped base-plus-index EA writes8rawbytes without alignment assumptions",()=>fixture(
    [0x0f,0xe7,0x04,0x02],async({reset,state,run,bytes})=>{
      reset({eax:0xfffffff0,edx:0x2013,flags:0xad7});setMm(state(),0,0x8877665544332211n,0x4000);state()[60]=0x3800;state()[61]=0xffff;const result=run(1);
      equal(result.retired,1,"EAwrap to mapped unaligned address is valid");equal(result.words[0],0xfffffff0,"index GPR unchanged");equal(result.words[2],0x2013,"base GPR unchanged");equal(result.words[9],0xad7,"non-temporal copy preserves flags");for(let n=0;n<8;n++)equal(bytes(DATA)[3+n],0x11*(n+1),"32bit sum wraps before width8access");equal(bytes(DATA)[2],0,"leading byte untouched");equal(bytes(DATA)[11],0,"trailing byte untouched");
    }));
  await test("MOVNTQ unaligned executable alias commits once then stops for SMC",()=>fixture(
    [0x0f,0xe7,0x03,0x40],async({handle,alias,reset,state,run,bytes,compile})=>{
      alias(0x4000,CODE,READ|WRITE);reset({eax:9,ebx:0x4103,flags:0xad7});setMm(state(),0,0x8877665544332211n,0x4000);state()[60]=0x3800;state()[61]=0xffff;const result=run(100);
      equal(result.kind,7,"MOVNTQ codealias store exits for SMC");equal(result.retired,1,"only complete8byte store retires");equal(result.pc,CODE+3,"next instruction is unexecuted");equal(result.words[0],9,"following INC has not executed");equal(result.words[61],0,"completed MMX state entry retained");equal(result.words[60],0,"completed store clearsTOP");equal(result.words[37],0x4000,"store keeps physical exponent");
      for(let n=0;n<8;n++)equal(bytes(CODE)[0x103+n],0x11*(n+1),"allunalignedcodealias bytes committed");check(api.backend_validate(handle,CODE)!==0,"oldcodeunit rejected");
      await compile(CODE+3);setMm(state(),0,0n);const resumed=run(1);equal(resumed.retired,1,"resume executes INC only");equal(resumed.words[0],10,"increment executes once");for(let n=0;n<8;n++)equal(bytes(CODE)[0x103+n],0x11*(n+1),"store not replayed using changed source");
    },READ|WRITE|EXECUTE|SEALED));
  await test("MOVNTQ two virtual aliases share committed data without a second MMX state",()=>fixture(
    [0x0f,0xe7,0x3b],async({alias,bytes,reset,state,run})=>{
      alias(0x4000,DATA,READ|WRITE);reset({ebx:0x4005});setMm(state(),7,0x8877665544332211n,0x4007);state()[60]=0x3800;state()[61]=0xffff;const result=run(1);
      equal(result.retired,1,"aliasstorecomplete");for(let n=0;n<8;n++){equal(bytes(DATA)[5+n],0x11*(n+1),"canonicalmirrorpage sees allaliasbytes");equal(bytes(0x4000)[5+n],0x11*(n+1),"aliasmirrorpage sees samebytes");}equal(result.words[58],0x4007,"physical MM7 exponentunchanged despite initialTOP7");equal(result.words[61],0,"singlearchitecturalMMXtag state");
    }));
  for(const code of[[0xf0,0x0f,0xe7,0x03],[0x0f,0xe7,0xc1]])await test(`${code[0]===0xf0?"LOCK":"register destination"} MOVNTQ is invalid before effects`,()=>fixture(
    code,async({reset,state,run})=>{reset({ebx:0x5003,flags:0xad7});setMm(state(),0,0x8877665544332211n);state()[60]=0x3800;state()[61]=0xffff;const before=Array.from(state()),result=run(1);equal(result.kind,8,"invalid store form is UD before memory checks");equal(result.retired,0,"invalid instruction unretired");for(let n=0;n<100;n++)equal(result.words[n],before[n],"invalid instruction retains all architecturalstate");}));
  await test("pending unmasked x87 exception cannot be erased by MMX", () => fixture([0x0f, 0x6e, 0xc0], async ({ reset, state, run }) => {
    reset({ eax: 0x1234 }); setMm(state(), 0, 0x0123456789abcdefn);
    state()[59] &= ~1; state()[60] = 0x3881; state()[61] = 0xffff;
    const result = run(100);
    equal(result.kind, 14, "pending exception uses architectural floating-point exit");
    equal(result.error, 1, "pending invalid exception mask is retained");
    equal(result.retired, 0, "MMX has not executed before pending exception");
    equal(mmValue(result.words, 0), 0x0123456789abcdefn, "pending exception payload preserved");
    equal(result.words[60], 0x3881, "pending exception status preserved");
    equal(result.words[61], 0xffff, "pending exception tag preserved");
  }));
  await test("PREFETCHNTA remains a non-faulting optional cache hint", () => fixture([0x0f, 0x18, 0x03], async ({ reset, run }) => {
    reset({ ebx: 0x7000, flags: 0xa93 }); const result = run(1);
    equal(result.retired, 1, "cache hint retires without mapped target");
    equal(result.words[9], 0xa93, "cache hint preserves flags");
  }));
  for (const mmx of [false, true]) {
    await test(`${mmx ? "MMX" : "scalar RMW"} journal exhaustion precedes all side effects`, () => fixture(
      mmx ? [0x0f, 0x7f, 0x03] : [0xff, 0x03], async ({ handle, bytes, state, reset, run }) => {
        reset({ ebx: DATA, flags: 0xa93 }); setMm(state(), 0, 0x8877665544332211n);
        state()[60] = 0x3800; state()[61] = 0xffff;
        const header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4);
        const capacity = header()[2]; header()[2] = 0;
        const result = run(100, () => { header()[2] = capacity; });
        equal(result.kind, 10, "journal exhaustion is explicit resource exit");
        equal(result.retired, 0, "exhausted store did not complete");
        equal(bytes(DATA)[0], 0, "exhausted store preserves bytes");
        equal(result.words[9], 0xa93, "exhausted RMW preserves flags");
        equal(result.words[60], 0x3800, "exhausted MMX store preserves TOP");
        equal(result.words[61], 0xffff, "exhausted MMX store preserves tag");
      }));
  }
  await test("structured loop preserves every budget boundary and interior entry", () => fixture(
    [0x85, 0xc0, 0x74, 1, 0x43, 0x49, 0x75, 0xf8, 0x0f, 0x0b], async ({ reset, run, compile, select }) => {
      if (options.rootedSsa) { await compile(CODE + 4); await compile(CODE + 5); }
      for (const entry of [0, 4, 5]) for (const eax of [0, 1]) for (let budget = 0; budget <= 23; budget += 1) {
        let pc = entry, ebx = 10, ecx = 3, flags = 0x202, defined = 0xffffffff, retired = 0, kind = 1;
        while (retired < budget) {
          if (pc === 8) { kind = 8; break; }
          if (pc === 0) {
            flags = (flags & ~(ARITH & ~AF)) | common(BigInt(eax), 32); defined &= ~AF; pc = 2;
          } else if (pc === 2) pc = flags & ZF ? 5 : 4;
          else if (pc === 4 || pc === 5) {
            const increment = pc === 4;
            const result = arithmetic(increment ? "add" : "sub", 32, increment ? ebx : ecx, 1, 0);
            flags = (flags & ~(ARITH & ~CF)) | (result.flags & ~CF); defined |= ARITH & ~CF;
            if (increment) { ebx = result.result; pc = 5; } else { ecx = result.result; pc = 6; }
          } else if (pc === 6) pc = flags & ZF ? 8 : 0;
          else throw Error("invalid reference loop PC");
          retired += 1;
        }
        reset({ eax, ebx: 10, ecx: 3, eip: CODE + entry });
        if (options.rootedSsa) select(CODE + entry);
        const result = run(budget);
        equal(result.kind, kind, "loop budget/trap exit");
        equal(result.retired, retired, "loop completed instructions");
        equal(result.pc, CODE + pc, "loop exact continuation");
        equal(result.words[3], ebx, "conditional loop increment");
        equal(result.words[1], ecx, "loop counter");
        equal(result.words[9] & defined, flags & defined, "loop defined flags at each budget");
      }
    }));
  for (const width of [16, 32]) {
    await test(`CMOVcc ${width}-bit conditions preserve untouched register bits and flags`, async () => {
      const conditions = [f => f & OF, f => !(f & OF), f => f & CF, f => !(f & CF),
        f => f & ZF, f => !(f & ZF), f => f & (CF | ZF), f => !(f & (CF | ZF)),
        f => f & SF, f => !(f & SF), f => f & PF, f => !(f & PF),
        f => Boolean(f & SF) !== Boolean(f & OF), f => Boolean(f & SF) === Boolean(f & OF),
        f => Boolean(f & ZF) || Boolean(f & SF) !== Boolean(f & OF),
        f => !(f & ZF) && Boolean(f & SF) === Boolean(f & OF)];
      for (let condition = 0; condition < 16; condition += 1) {
        await fixture([...(width === 16 ? [0x66] : []), 0x0f, 0x40 + condition, 0xc3], async ({ reset, run }) => {
          for (let bits = 0; bits < 32; bits += 1) {
            const flags = 0x202 | (bits & 1 ? CF : 0) | (bits & 2 ? PF : 0)
              | (bits & 4 ? ZF : 0) | (bits & 8 ? SF : 0) | (bits & 16 ? OF : 0);
            reset({ eax: 0x98765432, ebx: 0x1234abcd, flags });
            const result = run(1);
            const expected = conditions[condition](flags) ? retainedResult(0x98765432, 0x1234abcd & Number(widthMask(width)), width) : 0x98765432;
            equal(result.retired, 1, "CMOV retires once");
            equal(result.words[0], expected, "CMOV predicate and register width");
            equal(result.words[9], flags, "CMOV preserves all flags");
          }
        });
      }
    });
  }
  await test("false CMOV memory source still faults before destination or flags change", () => fixture(
    [0x0f, 0x44, 0x03], async ({ reset, run, map, compile }) => {
      reset({ eax: 0x98765432, ebx: 0x5000, flags: 0x202 });
      const fault = run(1);
      equal(fault.kind, 4, "false CMOVE still reads its source");
      equal(fault.pc, CODE, "conditional move exact fault PC");
      equal(fault.address, 0x5000, "conditional move source address");
      equal(fault.retired, 0, "faulted conditional move does not retire");
      equal(fault.words[0], 0x98765432, "fault leaves destination unchanged");
      equal(fault.words[9], 0x202, "fault leaves flags unchanged");
      map(0x5000, READ, page(wordBytes(0x1234abcd)));
      await compile();
      const falseResult = run(1);
      equal(falseResult.words[0], 0x98765432, "mapped false condition preserves destination");
      reset({ eax: 0x98765432, ebx: 0x5000, flags: 0x202 | ZF });
      const trueResult = run(1);
      equal(trueResult.words[0], 0x1234abcd, "mapped true condition reads source");
      equal(trueResult.words[9], 0x202 | ZF, "true condition preserves flags");
    }));
  await test("CMOV consumes live arithmetic carry before a later flag overwrite", () => fixture(
    [0x66, 0x83, 0xc0, 1, 0x66, 0x0f, 0x43, 0xcb, 0x83, 0xfa, 0, 0x0f, 0x0b], async ({ reset, run }) => {
      for (const initial of [1, 0xffff]) {
        reset({ eax: initial, ecx: 0xaaaa7777, ebx: 0x1234, edx: 0 });
        const result = run(100);
        equal(result.kind, 8, "full-budget CMOV sequence reaches UD2");
        equal(result.retired, 3, "arithmetic, CMOV and comparison complete");
        equal(result.words[1], initial === 1 ? 0xaaaa1234 : 0xaaaa7777, "CMOV observes the preceding defined carry");
        equal(result.words[9] & ARITH, arithmetic("cmp", 32, 0, 0, 0).flags, "later comparison flags remain materialized");
      }
    }));
  await test("indirect branch into a structured loop interior retains normal guest control flow", () => fixture(
    [0xff, 0xe7, 0x85, 0xc0, 0x74, 1, 0x43, 0x49, 0x75, 0xf8, 0x0f, 0x0b], async ({ compile, select, reset, run }) => {
      await compile(CODE + 6);
      reset({ eax: 1, ebx: 10, ecx: 3, edi: CODE + 6 });
      select(CODE);
      const branch = run(100);
      let result = branch;
      if (!options.linkedRegions) {
        equal(branch.kind, 2, "unlinked branch exposes the loop interior target");
        equal(branch.pc, CODE + 6, "indirect branch keeps exact target");
        select(branch.pc);
        result = run(100);
      }
      equal(result.kind, 8, "indirect loop reaches UD2");
      equal(result.pc, CODE + 10, "indirect loop final PC");
      equal(result.words[3], 13, "interior entry executes each increment once");
      equal(result.words[1], 0, "interior entry loop counter");
      equal(options.linkedRegions ? result.retired : branch.retired + result.retired,
        14, "indirect entry preserves exact retirement across cached regions");
    }));
  await test("loop fault resumes at failed iteration without replaying prior stores", () => fixture(
    [0xff, 0x03, 0x83, 0xc3, 4, 0x49, 0x75, 0xf8, 0x0f, 0x0b], async ({ map, bytes, reset, run }) => {
      reset({ ebx: DATA + 4092, ecx: 3 });
      const fault = run(100);
      equal(fault.kind, 4, "second iteration faults on unmapped page");
      equal(fault.retired, 4, "one iteration completed before fault");
      equal(fault.pc, CODE, "fault remains at failed store");
      equal(fault.words[3], DATA + 4096, "prior pointer increment retained");
      equal(fault.words[1], 2, "prior loop decrement retained");
      equal(bytes(DATA)[4092], 1, "first iteration store committed once");
      map(DATA + 4096, READ | WRITE);
      const resumed = run(100);
      equal(resumed.kind, 8, "resumed loop reaches UD2");
      equal(resumed.retired, 8, "only remaining two iterations retire");
      equal(bytes(DATA)[4092], 1, "resume did not replay prior page write");
      equal(bytes(DATA + 4096)[0], 1, "resumed second iteration");
      equal(bytes(DATA + 4096)[4], 1, "resumed third iteration");
    }));
  const xmmWords = (words, index) => Array.from(words.slice(67 + index * 4, 71 + index * 4));
  const putXmm = (words, index, values) => words.set(values, 67 + index * 4);
  const cvttReference = (bits, width, mxcsr) => {
    const fractionBits=width===32?23n:52n, exponentBits=width===32?8n:11n;
    const exponentMask=(1n<<exponentBits)-1n, exponent=bits>>fractionBits&exponentMask;
    const fraction=bits&((1n<<fractionBits)-1n), negative=Boolean(bits>>BigInt(width-1));
    if(exponent===exponentMask)return{value:0x80000000,flags:1};
    if(exponent===0n&&mxcsr&0x40)return{value:0,flags:0};
    const significand=exponent===0n?fraction:fraction|(1n<<fractionBits);
    const shift=(exponent===0n?1n:exponent)-(width===32?127n:1023n)-fractionBits;
    const magnitude=shift>=0n?significand<<shift:significand>>-shift;
    const discarded=shift<0n&&(significand&((1n<<-shift)-1n))!==0n;
    const integer=negative?-magnitude:magnitude;
    if(integer<-(1n<<31n)||integer>=(1n<<31n))return{value:0x80000000,flags:1};
    return{value:Number(BigInt.asUintN(32,integer)),flags:discarded?32:0};
  };
  if (hasSseCvtt) {
    const conversionBits = {
      32:[0n,0x80000000n,0x3f800000n,0xbf800000n,0x3fc00000n,0xbfc00000n,0x3f7fffffn,0x3f800001n,
        0x4effffffn,0x4f000000n,0xcf000000n,0xcf000001n,0x7f7fffffn,0xff7fffffn,0x7f800000n,0xff800000n,
        0x7fc01234n,0x7f801234n,1n,0x80000001n,0x007fffffn,0x00800000n,0x80800000n],
      64:[0n,0x8000000000000000n,0x3ff0000000000000n,0xbff0000000000000n,0x3ff8000000000000n,0xbff8000000000000n,
        0x3fefffffffffffffn,0x3ff0000000000001n,0x41dfffffffc00000n,0x41dfffffffffffffn,0x41e0000000000000n,
        0xc1e0000000000000n,0xc1e0000000000001n,0xc1e0000000200000n,0x7fefffffffffffffn,0xffefffffffffffffn,
        0x7ff0000000000000n,0xfff0000000000000n,0x7ff8000012345678n,0x7ff0000012345678n,1n,0x8000000000000001n,
        0x000fffffffffffffn,0x0010000000000000n,0x8010000000000000n],
    };
    const seedSseConversion = (words,mxcsr) => {
      for(let n=35;n<99;n++)words[n]=(n*0x1234567)>>>0;
      for(let n=37;n<59;n+=3)words[n]&=0xffff;
      words[59]=0x37e;words[60]=0x80c1;words[61]=0x3fff;words[62]=0x321;words[63]=0x11223344;words[65]=0x55667788;words[99]=mxcsr;
    };
    await test("pure SSE conversion ABI returns fresh flags without CPU, guest or FP scratch mutation",()=>fixture(
      [0x90],async({reset,state,bytes})=>{
        reset({eax:0x12345678});seedSseConversion(state(),0x1f80);bytes(DATA).fill(0x73);
        const before=Array.from(state()),image=bytes(DATA).slice();
        const fpPointer=hasTypedNumeric?api.backend_fp_kernel_0(0x37f,0,0x80000000,0x3fff,0,0x80000000,0x3fff):0;
        const fpOutput=fpPointer?Array.from(new Uint32Array(api.memory.buffer,fpPointer,7)):null;
        for(const width of [32,64])for(const bits of conversionBits[width])for(const mxcsr of [0,0x1f80,0x8040,0xffff]){
          const low=Number(bits&0xffffffffn),high=width===32?0xffffffff:Number(bits>>32n);
          const result=api[width===32?"backend_sse_cvtt_f32":"backend_sse_cvtt_f64"](mxcsr,low,high),reference=cvttReference(bits,width,mxcsr);
          equal(Number(result&0xffffffffn),reference.value,"pure result bits match integer oracle");equal(Number(result>>32n),reference.flags,"pure helper reports fresh IE/PE only, independent of masks");
        }
        for(const name of ["backend_sse_cvtt_f32","backend_sse_cvtt_f64"])check(Boolean(api[name](0x10000,0,0)&(1n<<63n)),"reserved MXCSR input returns explicit invalid-input sentinel");
        for(let n=0;n<105;n++)equal(state()[n],before[n],"pure SSE helpers never touch CPU state");for(let n=0;n<4096;n++)equal(bytes(DATA)[n],image[n],"pure SSE helpers never touch guest bytes");
        if(fpOutput)for(let n=0;n<7;n++)equal(new Uint32Array(api.memory.buffer,fpPointer,7)[n],fpOutput[n],"SSE return-value ABI cannot overwrite x87 scratch");
      }));
    for(const width of [32,64]) {
      const prefix=width===32?0xf3:0xf2,name=width===32?"CVTTSS2SI":"CVTTSD2SI";
      await test(`${name} uses exact signed truncation across MXCSR RC/DAZ/FTZ and masks`,()=>fixture(
        [prefix,0x0f,0x2c,0xc1],async({reset,state,run})=>{
          for(const bits of conversionBits[width])for(const rc of [0,1,2,3])for(const daz of [0,0x40])for(const ftz of [0,0x8000])for(const unmask of [0,0x80,0x1000,0x1080]) {
            const mxcsr=(0x1f80&~unmask)|(rc<<13)|daz|ftz|0x15;
            reset({eax:0x76543210,ecx:0xaabbccdd,edx:0x11223344,flags:0xad7});seedSseConversion(state(),mxcsr);
            putXmm(state(),1,[Number(bits&0xffffffffn),Number(bits>>32n),0x7f800001,0xdeadbeef]);
            const before=Array.from(state()),reference=cvttReference(bits,width,mxcsr),trap=Boolean(reference.flags&~(mxcsr>>>7));
            const result=run(1);
            equal(result.retired,trap?0:1,"unmasked new exception suppresses retirement only");equal(result.kind,trap?16:1,"precise SIMD exception or budget completion");
            equal(result.pc,CODE+(trap?0:4),"conversion EIP");equal(result.words[0],trap?before[0]:reference.value,"full destination GPR result or suppression");
            equal(result.words[99],mxcsr|reference.flags,"only newly raised IE/PE added to sticky MXCSR");
            if(trap){equal(result.error,0,"architectural XM has no error code");equal(result.address,0,"XM has no guest memory address");equal(result.faultPc,CODE,"precise SIMD fault instruction");}
            for(let n=1;n<8;n++)equal(result.words[n],before[n],"unrelated GPR retained");equal(result.words[9],0xad7,"conversion retains EFLAGS");equal(result.words[10],before[10],"conversion retains defined flags");
            for(let n=35;n<99;n++)equal(result.words[n],before[n],"conversion retains every x87/MMX/XMM word despite pending MF");
          }
        }));
      await test(`${name} preexisting unmasked sticky flags alone do not retrap`,()=>fixture(
        [prefix,0x0f,0x2c,0xd7],async({reset,state,run})=>{
          reset({edx:0xabcdef01});seedSseConversion(state(),0x1f80&~0x1080|0x21);
          const bits=width===32?0x40000000n:0x4000000000000000n;
          putXmm(state(),7,[Number(bits&0xffffffffn),Number(bits>>32n),0x11111111,0x22222222]);
          const result=run(1);equal(result.retired,1,"old IE/PE without new exception cannot raise XM");equal(result.words[2],2,"alternate destination/source register decoded");equal(result.words[99],0x1f80&~0x1080|0x21,"existing status retained");
        }));
      await test(`${name} unaligned and cross-page memory source is read before GPR overwrite`,()=>fixture(
        [prefix,0x0f,0x2c,0x00],async({map,reset,state,run,bytes})=>{
          map(DATA+4096,READ|WRITE);
          const bits=width===32?0xbfc00000n:0xbff8000000000000n;
          for(const offset of [1,4093,4095]){
            for(let n=0;n<width/8;n++){const address=DATA+offset+n;bytes(address&~4095)[address&4095]=Number(bits>>BigInt(n*8)&255n);}
            reset({eax:DATA+offset,flags:0xad7});seedSseConversion(state(),0x1f80);
            const before=Array.from(state()),result=run(1);equal(result.retired,1,"unaligned scalar memory conversion completes");equal(result.words[0],0xffffffff,"source address used before same GPR is replaced");equal(result.words[99],0x1fa0,"inexact truncation sets PE");
            for(let n=35;n<99;n++)equal(result.words[n],before[n],"memory conversion preserves all FP/vector words");
          }
        }));
      await test(`${name} complete memory fault precedes invalid conversion and survives two repairs`,()=>fixture(
        [0xff,0x03,prefix,0x0f,0x2c,0x02],async({map,bytes,reset,state,run,compile})=>{
          const bits=width===32?0x7fc01234n:0x7ff8000012345678n,offset=4094;
          for(let n=0;n<2;n++)bytes(DATA)[offset+n]=Number(bits>>BigInt(n*8)&255n);
          reset({eax:0x12345678,ebx:STACK,edx:DATA+offset});seedSseConversion(state(),0x1f00);
          const before=Array.from(state()),fault=run(100);equal(fault.kind,4,"missing second page wins over numeric invalid");equal(fault.pc,CODE+2,"conversion exact fault PC");equal(fault.address,DATA+4096,"first unavailable source page");equal(fault.retired,1,"only prior store completed");equal(fault.words[99],before[99],"failed read never changes MXCSR");equal(fault.words[0],before[0],"failed read never writes destination");equal(bytes(STACK)[0],1,"prior store retained");
          map(DATA+4096,READ|WRITE);for(let n=2;n<width/8;n++)bytes(DATA+4096)[n-2]=Number(bits>>BigInt(n*8)&255n);
          await compile(CODE+2);const numeric=run(1);equal(numeric.kind,16,"repaired read now raises newly unmasked IE");equal(numeric.retired,0,"numeric exception suppresses conversion retirement");equal(numeric.words[99],0x1f01,"IE becomes sticky after complete read");equal(numeric.words[0],before[0],"numeric exception leaves destination unchanged");equal(bytes(STACK)[0],1,"numeric retry did not replay earlier store");
          state()[99]|=0x80;const masked=run(1);equal(masked.retired,1,"mask repair executes only conversion");equal(masked.words[0],0x80000000,"masked invalid returns integer indefinite");equal(bytes(STACK)[0],1,"second repair does not repeat prior store");
          for(let n=35;n<99;n++)equal(masked.words[n],before[n],"read/fault/repair cycle preserves x87 and vectors");
        }));
      await test(`${name} unreadable source preserves conversion state`,()=>fixture(
        [prefix,0x0f,0x2c,0x03],async({map,reset,state,run})=>{
          map(0x6000,WRITE);reset({eax:0x11223344,ebx:0x6001});seedSseConversion(state(),0x1f00);
          const before=Array.from(state()),result=run(1);equal(result.kind,4,"write-only source cannot be read");equal(result.retired,0,"protected read unretired");equal(result.words[0],before[0],"protected destination retained");equal(result.words[99],before[99],"protected source never reaches conversion");
        }));
      await test(`LOCK ${name} is UD before source access`,()=>fixture(
        [0xf0,prefix,0x0f,0x2c,0x03],async({reset,state,run})=>{
          reset({eax:0x12345678,ebx:0x6000});state()[99]=0x1f00;const result=run(1);
          equal(result.kind,8,"LOCK is invalid independent of missing operand");equal(result.retired,0,"invalid conversion unretired");equal(result.words[0],0x12345678,"invalid conversion GPR retained");equal(result.words[99],0x1f00,"invalid opcode never changes MXCSR");
        }));
    }
    await test("CVTTSD2SI FS alias read sees completed scalar store without owning guest bytes",()=>fixture(
      [0xc7,0x03,0,0,0,0,0x64,0xf2,0x0f,0x2c,0x02],async({alias,bytes,reset,state,run})=>{
        alias(0x6000,DATA,READ|WRITE);setWord(bytes(DATA),4,0x3ff80000);
        reset({eax:0xabcdef01,ebx:DATA,edx:0,fs:0x6000,flags:0xad7});seedSseConversion(state(),0x1f80);
        const before=Array.from(state()),result=run(2);equal(result.retired,2,"store then alias conversion both complete");equal(result.words[0],1,"FS alias sees new low word and existing high word");equal(result.words[99],0x1fa0,"fractional conversion sets PE");
        for(let n=35;n<99;n++)equal(result.words[n],before[n],"alias read preserves FP/vector state");
      }));
    if(hasTimestamp)await test("SSE conversion and RDTSC mixed imports preserve numeric and timestamp continuations",()=>fixture(
      [0xf2,0x0f,0x2c,0xd1,0x0f,0x31],async({handle,reset,state,run})=>{
        reset();seedSseConversion(state(),0x1f80);putXmm(state(),1,[0,0x3ff80000,0,0]);
        const result=run(100);equal(result.kind,15,"timestamp import follows SSE helper correctly");equal(result.retired,1,"conversion completed before timestamp handoff");equal(result.words[2],1,"conversion output survives precise handoff");equal(result.words[99],0x1fa0,"conversion sticky PE survives handoff");
        ok(api.backend_complete_timestamp_counter(handle,CODE+4,123,456),"complete timestamp after scalar SIMD");equal(state()[0],123,"timestamp low word");equal(state()[2],456,"timestamp high word");equal(state()[99],0x1fa0,"timestamp leaves MXCSR intact");
      }));
    if(hasTimestamp)await test("typed x87, both SSE conversion formats and RDTSC share correct import indices",()=>fixture(
      [0xd8,0xc0,0xf3,0x0f,0x2c,0xd9,0xf2,0x0f,0x2c,0xf2,0x0f,0x31],async({handle,reset,state,run})=>{
        reset();state()[61]=0xfffc;state().set([0,0x80000000,0x3fff],35);putXmm(state(),1,[0xbfc00000,0,0,0]);putXmm(state(),2,[0,0x40040000,0,0]);
        const result=run(100);equal(result.kind,15,"all optional function imports resolve correctly");equal(result.retired,3,"x87 and both SIMD conversions complete before RDTSC");
        equal(result.words[3],0xffffffff,"single conversion destination");equal(result.words[6],2,"double conversion destination");equal(result.words[37],0x4000,"x87 doubled value remains");equal(result.words[99],0x1fa0,"both SIMD conversions share sticky PE");
        ok(api.backend_complete_timestamp_counter(handle,CODE+10,1,2),"complete timestamp after mixed FP helpers");
      }));
  }
  await test("XORPS and MOVUPS registers preserve all raw bits and unrelated FP state", () => fixture(
    [0x0f, 0x57, 0xc1, 0x0f, 0x10, 0xd0], async ({ state, reset, run }) => {
      for (let sample = 0; sample < 16; sample += 1) {
        reset({ flags: 0xa93 });
        const a = [0x7f800001, 0xff800000, next(), next()];
        const b = [0x80000000, 0x7fc01234, next(), next()];
        for (let n = 35; n < 67; n += 1) state()[n] = next();
        state()[99] = 0x7f81;
        putXmm(state(), 0, a); putXmm(state(), 1, b);
        for (let n = 2; n < 8; n += 1) putXmm(state(), n, [next(), next(), next(), next()]);
        const prior = Array.from(state());
        const result = run(2);
        equal(result.retired, 2, "two SIMD instructions complete");
        for (let n = 0; n < 4; n += 1) {
          equal(result.words[67 + n], (a[n] ^ b[n]) >>> 0, "bitwise XOR includes NaN payload bits");
          equal(result.words[75 + n], (a[n] ^ b[n]) >>> 0, "register move preserves every bit");
          equal(result.words[71 + n], b[n], "source register unchanged");
        }
        for (let n = 35; n < 67; n += 1) equal(result.words[n], prior[n], "SSE does not modify shared x87/MMX state");
        for (let n = 79; n < 99; n += 1) equal(result.words[n], prior[n], "unwritten XMM register preserved");
        equal(result.words[99], 0x7f81, "SSE bitwise/move preserves MXCSR");
        equal(result.words[9], 0xa93, "SSE bitwise/move preserves EFLAGS");
      }
    }));
  await test("reached XORPS zero and MOVUPS stack store keep generic vector semantics", () => fixture(
    [0x0f, 0x57, 0xc0, 0x0f, 0x11, 0x44, 0x24, 8], async ({ state, bytes, reset, run }) => {
      reset(); putXmm(state(), 0, [0xffffffff, 0x7f800001, 0x87654321, 0x12345678]);
      bytes(STACK).fill(0x55);
      const result = run(2);
      equal(result.retired, 2, "reached stack initialization completes");
      for (let n = 0; n < 16; n += 1) equal(bytes(STACK)[0x808 + n], 0, "full sixteen-byte stack output");
      equal(bytes(STACK)[0x807], 0x55, "byte before store preserved");
      equal(bytes(STACK)[0x818], 0x55, "byte after store preserved");
    }));
  await test("MOVUPS unaligned and cross-page transfers preserve all sixteen bytes", () => fixture(
    [0x0f, 0x10, 0x03, 0x0f, 0x11, 0x02], async ({ map, bytes, reset, run }) => {
      map(DATA + 4096, READ | WRITE); map(STACK + 4096, READ | WRITE);
      for (const offset of [7, 4090, 4095]) {
        const values = Array.from({ length: 16 }, (_, n) => (n * 19 + offset) & 255);
        for (let n = 0; n < 16; n += 1) {
          const at = DATA + offset + n; bytes(at & ~4095)[at & 4095] = values[n];
        }
        reset({ ebx: DATA + offset, edx: STACK + offset });
        const result = run(2);
        equal(result.retired, 2, "unaligned load and store complete");
        for (let n = 0; n < 16; n += 1) {
          const at = STACK + offset + n;
          equal(bytes(at & ~4095)[at & 4095], values[n], "cross-page vector bytes");
        }
      }
    }));
  for (const write of [false, true]) {
    await test(`MOVUPS cross-page ${write ? "store" : "load"} faults before partial effects and resumes once`, () => fixture(
      [0x40, 0x0f, write ? 0x11 : 0x10, 0x03], async ({ map, bytes, state, reset, run, protect, compile }) => {
        bytes(DATA).fill(0x55);
        if (write) map(DATA + 4096, READ, page([], 0x66));
        reset({ ebx: DATA + 4090 }); putXmm(state(), 0, [0x03020100, 0x07060504, 0x0b0a0908, 0x0f0e0d0c]);
        const prior = xmmWords(state(), 0);
        const fault = run(100);
        equal(fault.kind, 4, "vector page fault");
        equal(fault.retired, 1, "only prior arithmetic completed");
        equal(fault.pc, CODE + 1, "vector fault exact continuation");
        equal(fault.address, DATA + 4096, "first unavailable vector byte page");
        for (let n = 0; n < 4; n += 1) equal(xmmWords(fault.words, 0)[n], prior[n], "failed vector instruction preserves destination/source");
        for (let n = 4090; n < 4096; n += 1) equal(bytes(DATA)[n], 0x55, "failed store cannot partially change first page");
        if (write) protect(DATA + 4096, READ | WRITE); else map(DATA + 4096, READ, page([], 0x66));
        await compile(fault.pc);
        const resumed = run(1);
        equal(resumed.retired, 1, "resumed vector instruction completes once");
        equal(resumed.words[0], 1, "resume does not replay earlier increment");
        if (write) for (let n = 0; n < 16; n += 1) {
          const at = DATA + 4090 + n; equal(bytes(at & ~4095)[at & 4095], n, "resumed full vector store");
        } else {
          equal(resumed.words[67], 0x55555555, "cross-page load lowword");
          equal(resumed.words[68], 0x66665555, "cross-page load mixedword");
          equal(resumed.words[70], 0x66666666, "cross-page load finalword");
        }
      }));
  }
  await test("legacy XORPS memory enforces alignment and preserves state on faults", () => fixture(
    [0x0f, 0x57, 0x03], async ({ state, reset, run, bytes }) => {
      for (const [address, kind] of [[0x5000, 4], [0x5001, 6]]) {
        reset({ ebx: address, flags: 0xa93 }); putXmm(state(), 0, [1, 2, 3, 4]);
        const result = run(1);
        equal(result.kind, kind, "legacy XORPS alignment precedes memory access");
        equal(result.retired, 0, "failed XORPS does not retire");
        for (let n = 0; n < 4; n += 1) equal(result.words[67 + n], n + 1, "failed XORPS preserves destination");
        equal(result.words[9], 0xa93, "failed XORPS preserves flags");
      }
      bytes(DATA).set(new Uint8Array(new Uint32Array([0x12345678, 0x87654321, 0xffffffff, 0x7f800001]).buffer));
      reset({ ebx: DATA }); putXmm(state(), 0, [1, 2, 3, 4]);
      const result = run(1);
      for (const [n, value] of [0x12345678, 0x87654321, 0xffffffff, 0x7f800001].entries())
        equal(result.words[67 + n], (value ^ (n + 1)) >>> 0, "aligned memory XOR is bitwise");
    }));
  for (const sameRegister of [true, false]) {
    await test(`PXOR XMM ${sameRegister ? "self zero" : "unequal source"} preserves pending FP and full vector state`, () => fixture(
      [0x66, 0x0f, 0xef, sameRegister ? 0xc0 : 0xc1], async ({ reset, run, state }) => {
        reset({ flags: 0xad7 });
        for (let n = 35; n < 100; n++) state()[n] = (0x9e3779b9 * n) >>> 0;
        state()[59] = 0x37e; state()[60] = 0x80c1; state()[61] = 0x3fff; state()[99] = 0x1f81;
        const before = Array.from(state());
        const result = run(1);
        equal(result.retired, 1, "nonfloating PXOR completes despite pending x87 exception");
        for (let n = 35; n < 100; n++) {
          const expected = n >= 67 && n < 71
            ? (before[n] ^ before[sameRegister ? n : n + 4]) >>> 0 : before[n];
          equal(result.words[n], expected, "exact vector result and unrelated raw FP/vector preservation");
        }
        equal(result.words[9], 0xad7, "PXOR leaves EFLAGS unchanged");
      }));
  }
  await test("PXOR aligned memory and atomic alignment/page faults retain FP environment", () => fixture(
    [0x66, 0x0f, 0xef, 0x03], async ({ reset, run, state, bytes }) => {
      const source = [0x12345678, 0x87654321, 0xffffffff, 0x7f800001];
      bytes(DATA).set(new Uint8Array(new Uint32Array(source).buffer));
      for (const [address, kind] of [[DATA, 1], [DATA + 1, 6], [0x5000, 4], [0x5001, 6]]) {
        reset({ ebx: address, flags: 0xad7 });
        for (let n = 35; n < 100; n++) state()[n] = (0x7f4a7c15 * n) >>> 0;
        state()[59] = 0x37e; state()[60] = 0x80c1; state()[61] = 0x3fff; state()[99] = 0x1f81;
        const before = Array.from(state()), result = run(1);
        equal(result.kind, kind, "legacy alignment check precedes page check");
        equal(result.retired, kind === 1 ? 1 : 0, "only complete XOR retires");
        equal(result.pc, kind === 1 ? CODE + 4 : CODE, "precise completion or fault PC");
        for (let n = 35; n < 100; n++) {
          const expected = kind === 1 && n >= 67 && n < 71 ? (before[n] ^ source[n - 67]) >>> 0 : before[n];
          equal(result.words[n], expected, "fault atomicity and full raw state preservation");
        }
        equal(result.words[9], 0xad7, "memory PXOR preserves integer flags");
      }
    }));
  for (const [code, kind, label] of [[[0xf0,0x0f,0xef,0xc0],8,"LOCK MMX self-zero is invalid"],
    [[0xf0,0x0f,0xef,0x03],8,"LOCK MMX memory is invalid"],[[0xf0, 0x66, 0x0f, 0xef, 0xc1], 8, "LOCK XMM is invalid"]]) {
    await test(`PXOR encoding distinction: ${label}`, () => fixture(code, async ({ reset, run, state }) => {
      reset({ flags: 0xad7 });
      for (let n = 35; n < 100; n++) state()[n] = (0x45d9f3b * n) >>> 0;
      const before = Array.from(state()), result = run(1);
      equal(result.kind, kind, "LOCK cannot route through either supported lowering");
      equal(result.retired, 0, "rejected encoding has no completed instruction");
      equal(result.pc, CODE, "exact rejected instruction PC");
      for (let n = 35; n < 100; n++) equal(result.words[n], before[n], "rejected operation leaves all FP/vector state untouched");
      equal(result.words[9], 0xad7, "rejected operation preserves flags");
    }));
  }
  for(const xmm of [false,true])await test(`PXOR ${xmm?"66-prefixed XMM":"MMX"} keeps the correct width and architectural register bank`,()=>fixture(
    [...(xmm?[0x66]:[]),0x0f,0xef,0x03],async({reset,state,run,bytes})=>{
      bytes(DATA).set([1,2,3,4,5,6,7,8],4088);reset({ebx:DATA+4088,flags:0xad7});setMm(state(),0,0x8877665544332211n,0x4000);putXmm(state(),0,[1,2,3,4]);state()[60]=0x3800;state()[61]=0xffff;const before=Array.from(state()),result=run(1);
      equal(result.kind,xmm?6:1,"MMX last8bytes succeed; XMM unaligned16 is GP before page lookup");equal(result.retired,xmm?0:1,"only the correct-width MMX operation completes");
      for(let n=0;n<100;n++){const value=0x8877665544332211n^0x0807060504030201n;const expected=xmm?before[n]:n===8?CODE+3:n===35?Number(value&0xffffffffn):n===36?Number(value>>32n):n===37?0xffff:n===60?0:n===61?0:before[n];equal(result.words[n],expected,"prefix selects precise width, bank and MMX effects");}
    }));
  await test("LDMXCSR unaligned full m32 preserves non-SSE state", () => fixture(
    [0x0f,0xae,0x13], async ({ reset,state,bytes,run }) => {
      for(const value of [0,0x3f,0x40,0x1f80,0xffff]) {
        reset({ ebx:DATA+1,flags:0xad7 }); state()[59]=0x37e;state()[60]=0x81;
        for(let n=35;n<59;n++)state()[n]=n*31337;for(let n=67;n<99;n++)state()[n]=n*0x12345;
        setWord(bytes(DATA),1,value);const before=Array.from(state()),result=run(1);
        equal(result.retired,1,"control load completes despite pending x87 exception/unmasked SSE sticky bits");equal(result.words[99],value,"all supported MXCSR bits loaded");
        for(let n=35;n<99;n++)equal(result.words[n],before[n],"control load retains full raw80/environment/XMM");equal(result.words[9],0xad7,"control load preserves EFLAGS");
      }
    }));
  await test("LDMXCSR completes cross-page read before reserved-bit GP and retries atomically", () => fixture(
    [0x0f,0xae,0x13], async ({ reset,state,bytes,run,map }) => {
      bytes(DATA).set([0,0,1],4093);reset({ebx:DATA+4093,flags:0xad7});state()[99]=0x1234;
      const before=Array.from(state()),missing=run(1);
      equal(missing.kind,4,"missing final byte faults before interpreting reserved bits");equal(missing.address,DATA+4096,"precise cross-page address");equal(missing.retired,0,"failed read unretired");
      for(let n=35;n<100;n++)equal(missing.words[n],before[n],"partial read changes no floating state");
      map(DATA+4096,READ|WRITE);const reserved=run(1);
      equal(reserved.kind,6,"complete value with bit16 is GP");equal(reserved.error,0,"reserved-bit GP error zero");equal(reserved.address,0,"reserved-bit GP address zero");equal(reserved.pc,CODE,"same exact instruction remains pending");equal(reserved.retired,0,"reserved value unretired");
      for(let n=35;n<100;n++)equal(reserved.words[n],before[n],"GP preserves floating and vector state");
      bytes(DATA)[4095]=0;const repaired=run(1);equal(repaired.retired,1,"fixed data resumes once");equal(repaired.words[99],0,"repaired valid MXCSR committed");
    }));
  await test("STMXCSR cross-page fault preserves prior store and writes all four bytes on repair", () => fixture(
    [0xff,0x07,0x0f,0xae,0x1b], async ({ reset,state,bytes,run,map,compile }) => {
      bytes(DATA).fill(0x55,4093);reset({ebx:DATA+4093,edi:STACK,flags:0xad7});state()[99]=0xffff;
      state()[59]=0x37e;state()[60]=0x81;const before=Array.from(state().slice(35,100)),fault=run(100);
      equal(fault.kind,4,"missing store tail faults");equal(fault.retired,1,"only prior scalar store retired");equal(fault.pc,CODE+2,"precise STMXCSR PC");equal(bytes(STACK)[0],1,"earlier write retained");
      for(let n=4093;n<4096;n++)equal(bytes(DATA)[n],0x55,"failed STMXCSR stores no prefix bytes");
      for(let n=0;n<before.length;n++)equal(fault.words[35+n],before[n],"fault preserves all floating state");
      map(DATA+4096,READ|WRITE);await compile(CODE+2);const repaired=run(1);equal(repaired.retired,1,"repaired STMXCSR completed once");equal(bytes(STACK)[0],1,"earlier scalar store was not repeated");
      for(let n=0;n<4;n++){const address=DATA+4093+n;equal(bytes(address&~4095)[address&4095],wordBytes(0xffff)[n],"all stored MXCSR bytes");}
    }));
  await test("MXCSR FS addressing and mixed XMM operations preserve independent state", () => fixture(
    [0x64,0x0f,0xae,0x10,0x66,0x0f,0xef,0xc1,0x64,0x0f,0xae,0x18], async ({ reset,state,bytes,run }) => {
      reset({eax:4,fs:DATA,flags:0xad7});setWord(bytes(DATA),4,0x3f);
      const original=[];for(let n=67;n<99;n++){state()[n]=(0xabcdef00+n)>>>0;original.push(state()[n]);}
      const result=run(3);equal(result.retired,3,"load, independent vector XOR and control store complete");equal(result.words[99],0x3f,"unmasked sticky flags remain a control value");
      for(let n=0;n<4;n++)equal(result.words[67+n],(original[n]^original[n+4])>>>0,"XMM XOR uses original source vectors");
      for(let n=4;n<32;n++)equal(result.words[67+n],original[n],"other vectors retain state");equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(4,true),0x3f,"FS store uses offset plus base");equal(result.words[9],0xad7,"mixed control/vector flags unchanged");
    }));
  await test("STMXCSR code-alias write commits once and invalidates before next instruction", () => fixture(
    [0x0f,0xae,0x1b,0x40], async ({handle,alias,reset,state,run,bytes}) => {
      alias(0x4000,CODE,READ|WRITE);reset({ebx:0x4101,eax:9});state()[99]=0xffff;
      const result=run(100);equal(result.kind,7,"code alias write terminates current region");equal(result.retired,1,"STMXCSR retires once");equal(result.pc,CODE+3,"SMC continuation is after store");equal(result.words[0],9,"following INC was not executed");
      equal(new DataView(bytes(CODE).buffer,bytes(CODE).byteOffset).getUint32(0x101,true),0xffff,"unaligned alias bytes updated exactly");check(api.backend_validate(handle,CODE)!==0,"old code admission rejected");
    }));
  for(const prefix of [0xf0,0x66])for(const modrm of [0x13,0x1b])await test(`${prefix===0xf0?"LOCK":"66"} MXCSR control opcode ${modrm.toString(16)} faults before memory access`,()=>fixture(
    [prefix,0x0f,0xae,modrm],async({reset,run})=>{reset({ebx:0x6000,flags:0xad7});const result=run(1);equal(result.kind,8,"invalid control prefix is UD even when operand absent");equal(result.retired,0,"invalid instruction unretired");equal(result.words[99],0x1f80,"invalid instruction retains MXCSR");equal(result.words[9],0xad7,"invalid instruction retains flags");}));
  const seedScalarSseMove = (words) => {
    for (let n = 35; n < 100; n++) words[n] = Math.imul(n, 0x1234567) >>> 0;
    words[59] = 0x37e; words[60] = 0xb8c1; words[61] = 0x0fff; words[99] = 0x7f81;
    for (let r = 0; r < 8; r++) putXmm(words, r, [0x7f800001 + r, 0xfff00000, 0x80000000 + r, 0x7ff01234 + r]);
  };
  const checkScalarSseMove = (result, before, changes, nextPc, retired = 1) => {
    equal(result.retired, retired, "scalar SSE transfer retires only completed instructions");
    const expected = before.slice(); expected[8] = nextPc;
    for (const [index, value] of Object.entries(changes)) expected[Number(index)] = value >>> 0;
    for (let n = 0; n < 100; n++) equal(result.words[n], expected[n], `scalar SSE exact architectural word ${n}`);
  };
  const cvttpsChanges = (before, destination, inputs, mxcsr) => {
    const lanes = inputs.map(raw => cvttReference(BigInt(raw >>> 0), 32, mxcsr));
    const all = lanes.reduce((flags, lane) => flags | lane.flags, 0), masks = mxcsr >>> 7;
    const fresh = all & 1 & ~masks ? all & 1 : all, trap = (fresh & ~masks) !== 0;
    const changes = { 99: mxcsr | fresh };
    if (!trap) lanes.forEach((lane, n) => { changes[67 + destination * 4 + n] = lane.value; });
    return { changes, trap, fresh };
  };
  const cvttpsInputs = [0, 0x80000000, 0x3f800000, 0xbf800000, 0x3fc00000, 0xbfc00000, 0x3f7fffff, 0x3f800001,
    0x4effffff, 0x4f000000, 0xcf000000, 0xcf000001, 0x7f7fffff, 0xff7fffff, 0x7f800000, 0xff800000,
    0x7fc01234, 0x7f801234, 1, 0x80000001, 0x007fffff, 0x00800000, 0x80800000];
  await test("CVTTPS2DQ all XMM pairs and aliases truncate every lane atomically", async () => {
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
      [0xf3, 0x0f, 0x5b, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        for (let rc = 0; rc < 4; rc++) for (const masks of [0, 0x80, 0x1000, 0x1f80]) {
          reset({ flags: 0xad7 }); seedScalarSseMove(state()); const mxcsr = masks | rc << 13; state()[99] = mxcsr;
          putXmm(state(), source, [0x7fc01234, 0x3fc00000, 1, 0xcf000000]);
          const before = Array.from(state()), { changes, trap } = cvttpsChanges(before, destination, xmmWords(before, source), mxcsr);
          checkScalarSseMove(run(0), before, {}, CODE, 0); const result = run(1); equal(result.kind, trap ? 16 : 1, "packed conversion completion or XM");
          checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 4), trap ? 0 : 1);
        }
      });
  });
  await test("CVTTPS2DQ raw classes boundaries RC DAZ FTZ and masks match integer oracle", () => fixture(
    [0xf3, 0x0f, 0x5b, 0xc1], async ({ reset, state, run }) => {
      for (let at = 0; at < cvttpsInputs.length; at++) for (let rc = 0; rc < 4; rc++) for (const extra of [0, 0x40, 0x8000, 0x8040]) for (const unmask of [0, 0x80, 0x1000, 0x1080]) {
        const mxcsr = (0x1f80 & ~unmask) | rc << 13 | extra | (at & 1 ? 0x3f : 0), inputs = Array.from({ length: 4 }, (_, lane) => cvttpsInputs[(at + lane) % cvttpsInputs.length]);
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = mxcsr; putXmm(state(), 1, inputs);
        const before = Array.from(state()), { changes, trap } = cvttpsChanges(before, 0, inputs, mxcsr), result = run(1);
        equal(result.kind, trap ? 16 : 1, "only freshly unmasked exceptions trap"); checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 4), trap ? 0 : 1);
        if (trap) { equal(result.faultPc, CODE, "precise CVTTPS2DQ PC"); equal(result.address, 0, "XM address"); equal(result.error, 0, "XM error"); }
      }
    }));
  await test("CVTTPS2DQ only fresh unmasked IE suppresses other-lane PE, never old sticky", () => fixture(
    [0xf3, 0x0f, 0x5b, 0xc1], async ({ reset, state, run }) => {
      const inputs = [0x7f800123, 0x3fc00000, 1, 0xcf000000];
      for (let mask = 0; mask < 64; mask++) for (const sticky of [0, 0x20, 0x3f]) {
        const mxcsr = mask << 7 | sticky; reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = mxcsr; putXmm(state(), 1, inputs);
        const before = Array.from(state()), { changes, trap, fresh } = cvttpsChanges(before, 0, inputs, mxcsr), result = run(1);
        equal(fresh, mask & 1 ? 0x21 : 1, "packed IE precedes PE"); equal(result.kind, trap ? 16 : 1, "whole vector suppression");
        checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 4), trap ? 0 : 1);
      }
      reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x3f; putXmm(state(), 1, [0, 0x80000000, 0x40000000, 0xcf000000]);
      const before = Array.from(state()), { changes } = cvttpsChanges(before, 0, xmmWords(before, 1), 0x3f); checkScalarSseMove(run(1), before, changes, CODE + 4);
    }));
  await test("CVTTPS2DQ denormal conversion reports PE only or DAZ zero even when DE unmasked", () => fixture(
    [0xf3, 0x0f, 0x5b, 0xc1], async ({ reset, state, run }) => {
      for (const mxcsr of [0x1e80, 0x1ec0, 0x9e80, 0x9ec0]) {
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = mxcsr; putXmm(state(), 1, [1, 0x80000001, 0x007fffff, 0x807fffff]);
        const before = Array.from(state()), { changes } = cvttpsChanges(before, 0, xmmWords(before, 1), mxcsr), result = run(1);
        equal(result.kind, 1, "no denormal exception"); equal(result.words[99], mxcsr | (mxcsr & 0x40 ? 0 : 0x20), "DAZ alone controls new precision"); checkScalarSseMove(result, before, changes, CODE + 4);
      }
    }));
  await test("CVTTPS2DQ full aligned source access precedes numeric flags and alias reads see current bytes", () => fixture(
    [0x64, 0xf3, 0x0f, 0x5b, 0x13], async ({ reset, state, run, bytes, map, protect, alias }) => {
      map(DATA + 4096, WRITE); alias(0x6000, DATA, READ); const inputs = [0x7fc01234, 0x3fc00000, 1, 0xcf000000];
      for (const [offset, base, rights, kind, error] of [[DATA, 0, READ, 16, 0], [DATA + 4080, 0, READ, 16, 0], [DATA + 4088, 0, READ, 6, 0], [DATA + 1, 0, WRITE, 6, 0], [0x5001, 0, READ, 6, 0], [DATA, 0, WRITE, 4, 5], [0x5000, 0, READ, 4, 4], [DATA + 8, 8, READ, 16, 0], [0xfffffff8, DATA + 24, READ, 6, 0], [0x6000, 0, READ, 16, 0]]) {
        protect(DATA, READ | WRITE); bytes(DATA).fill(0x55); for (const at of [0, 16, 4080]) inputs.forEach((word, lane) => setWord(bytes(DATA), at + lane * 4, word));
        reset({ ebx: offset, fs: base, flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0; const before = Array.from(state()), image = bytes(DATA).slice(); protect(DATA, rights);
        const result = run(1); equal(result.kind, kind, "memory check first"); equal(result.error, error, "memory error"); checkScalarSseMove(result, before, kind === 16 ? { 99: 1 } : {}, CODE, 0);
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "source is read-only");
      }
    }));
  await test("CVTTPS2DQ PF then IE then PE repair retains completed memory ADD and thread snapshot", () => fixture(
    [0x83, 0x07, 1, 0xf3, 0x0f, 0x5b, 0x13], async ({ reset, state, run, bytes, map, compile, saveSnapshot, restoreSnapshot }) => {
      const inputs = [0x7fc01234, 0x3fc00000, 1, 0xcf000000];
      setWord(bytes(STACK), 0, 0x7fffffff); reset({ ebx: 0x5000, edi: STACK, flags: 0x203 }); seedScalarSseMove(state()); state()[99] = 0;
      const before = Array.from(state()), flags = before[9] & ~ARITH | arithmetic("add", 32, 0x7fffffff, 1, 0).flags, fault = run(100);
      equal(fault.kind, 4, "missing source before conversion"); checkScalarSseMove(fault, before, { 9: flags }, CODE + 3, 1);
      const input = page(); inputs.forEach((word, lane) => setWord(input, lane * 4, word)); map(0x5000, READ, input); await compile(CODE + 3);
      let prior = Array.from(state()), result = run(1); equal(result.kind, 16, "IE after page repair"); checkScalarSseMove(result, prior, { 99: 1 }, CODE + 3, 0);
      const saved = saveSnapshot(); reset({ eax: 0xdeadbeef }); restoreSnapshot(saved);
      state()[99] |= 0x80; prior = Array.from(state()); result = run(1); equal(result.kind, 16, "PE after IE mask repair"); checkScalarSseMove(result, prior, { 99: 0xa1 }, CODE + 3, 0);
      state()[99] |= 0x1f80; prior = Array.from(state()); const { changes } = cvttpsChanges(prior, 2, inputs, prior[99]); checkScalarSseMove(run(1), prior, changes, CODE + 7);
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "all repairs/restarts retain exactly one completed ADD");
    }));
  await test("CVTTPS2DQ result and flags survive executable alias store SMC and exact resume", () => fixture(
    [0xf3, 0x0f, 0x5b, 0xc1, 0x0f, 0x11, 0x03, 0x40], async ({ handle, reset, state, run, bytes, alias, compile }) => {
      alias(0x4000, CODE, READ | WRITE); reset({ ebx: 0x4101, eax: 9, flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x1f80;
      const inputs = [0x7fc01234, 0x3fc00000, 1, 0xcf000000]; putXmm(state(), 1, inputs); const before = Array.from(state()), { changes } = cvttpsChanges(before, 0, inputs, before[99]), result = run(100);
      equal(result.kind, 7, "completed store forces SMC"); checkScalarSseMove(result, before, changes, CODE + 7, 2);
      const output = new Uint8Array(new Uint32Array(inputs.map(raw => cvttReference(BigInt(raw), 32, before[99]).value)).buffer);
      for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "integer vector stored");
      check(api.backend_validate(handle, CODE) !== 0, "old code invalidated"); await compile(CODE + 7); putXmm(state(), 0, [0, 0, 0, 0]);
      const resumed = Array.from(state()); checkScalarSseMove(run(1), resumed, { 0: 10, 9: arithmetic("add", 32, 9, 1, 0).flags | resumed[9] & ~ARITH | resumed[9] & CF }, CODE + 8);
      for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "resume cannot repeat conversion or store");
    }, READ | WRITE | EXECUTE | SEALED));
  for (const [code, kind] of [[[0x66, 0xf3, 0x0f, 0x5b, 0xc1], 3], [[0xf3, 0x66, 0x0f, 0x5b, 0xc1], 3], [[0x67, 0xf3, 0x0f, 0x5b, 0x03], 3], [[0x65, 0xf3, 0x0f, 0x5b, 0x03], 3], [[0xc5, 0xfa, 0x5b, 0xc1], 3], [[0xf0, 0xf3, 0x0f, 0x5b, 0x03], 8]]) await test("CVTTPS2DQ unsupported versus invalid " + code.join("-"), () => fixture(
    code, async ({ reset, state, run }) => { reset({ ebx: 0x5001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1); equal(result.kind, kind, "encoding boundary"); checkScalarSseMove(result, before, {}, CODE, 0); }));
  await test("CVTTPS2DQ alone and sparse SUBPS import share existing f32 CVTT ABI without new imports", async () => {
    for (const withSub of [false, true]) await fixture([0xf3, 0x0f, 0x5b, 0xd9, ...(withSub ? [0x0f, 0x5c, 0xc1] : [])], async ({ handle, reset, state, run }) => {
      const module = new WebAssembly.Module(new Uint8Array(api.memory.buffer, api.backend_code_ptr(handle), api.backend_code_len(handle)).slice());
      const imports = WebAssembly.Module.imports(module).filter(item => item.name.startsWith("sse_")).map(item => item.name);
      equal(imports.join(","), withSub ? "sse_cvtt_f32,sse_sub_f32" : "sse_cvtt_f32", "only shared old helper and selected sparse operation imported");
      reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x1f80; putXmm(state(), 0, [0x40800000, 0x40800000, 0x40800000, 0x40800000]); putXmm(state(), 1, [0x3fc00000, 0x3fc00000, 0x3fc00000, 0x3fc00000]);
      const before = Array.from(state()), { changes } = cvttpsChanges(before, 3, xmmWords(before, 1), before[99]);
      if (withSub) for (let lane = 0; lane < 4; lane++) changes[67 + lane] = 0x40200000;
      checkScalarSseMove(run(withSub ? 2 : 1), before, changes, CODE + (withSub ? 7 : 4), withSub ? 2 : 1);
    });
  });
  if (hasTimestamp && hasSseCvtt) await test("CVTTPS2DQ and scalar CVTT share one import beside x87 SCAS double and timestamp", () => fixture(
    [0xf2, 0xae, 0xd9, 0xee, 0xd9, 0x1b, 0xf3, 0x0f, 0x5b, 0xd9, 0xf3, 0x0f, 0x2c, 0xc1, 0xf2, 0x0f, 0x2c, 0xd2, 0x0f, 0x31], async ({ handle, reset, state, run, bytes }) => {
      const module = new WebAssembly.Module(new Uint8Array(api.memory.buffer, api.backend_code_ptr(handle), api.backend_code_len(handle)).slice());
      equal(WebAssembly.Module.imports(module).filter(item => item.name === "sse_cvtt_f32").length, 1, "scalar and packed share one function import");
      bytes(DATA)[0] = 3; reset({ eax: 3, ecx: 1, edi: DATA, ebx: STACK, flags: 0xad7 }); putXmm(state(), 1, [0x3fc00000, 0xbfc00000, 0x4effffff, 0xcf000000]); putXmm(state(), 2, [0, 0x40180000, 0, 0]);
      const result = run(100); equal(result.kind, 15, "shared numeric indices reach timestamp"); equal(result.pc, CODE + 18, "timestamp PC"); equal(result.retired, 6, "six completed operations");
      equal(result.words[0], 1, "scalar CVTT reuses helper"); equal(result.words[2], 6, "double CVTT distinct helper"); equal(result.words[99], 0x1fa0, "packed/scalar precision sticky");
      for (let lane = 0; lane < 4; lane++) equal(result.words[79 + lane], [1, 0xffffffff, 0x7fffff80, 0x80000000][lane], "all packed integer lanes retained");
      equal(result.words[9], (0xad7 & ~ARITH) | 0x44, "SCAS flags retained"); equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0, "x87 store retained");
      ok(api.backend_complete_timestamp_counter(handle, CODE + 18, 0x12345678, 9), "complete timestamp"); equal(state()[8], CODE + 20, "timestamp resume");
    }));
  // Independent oracle: search adjacent positive IEEE binary32 encodings in
  // exact integer units of 2^-23, then select by MXCSR.RC and distance/tie parity.
  const i32ToF32Cache = new Map();
  const i32ToF32Reference = (raw, mxcsr) => {
    raw >>>= 0; const rc = mxcsr >>> 13 & 3, key = raw + ":" + rc;
    if (i32ToF32Cache.has(key)) return i32ToF32Cache.get(key);
    const negative = raw >= 0x80000000, magnitude = BigInt(negative ? 0x100000000 - raw : raw), target = magnitude << 23n;
    if (magnitude === 0n) return { bits: 0, flags: 0 };
    const scaled = bits => BigInt((bits & 0x7fffff) | 0x800000) << BigInt((bits >>> 23) - 127);
    let low = 0x3f800000, high = 0x4f000000;
    while (low < high) { const mid = Math.ceil((low + high) / 2); if (scaled(mid) <= target) low = mid; else high = mid - 1; }
    const lower = scaled(low), inexact = lower !== target;
    let rounded = low;
    if (inexact) {
      const up = rc === 0 ? target - lower > scaled(low + 1) - target || (target - lower === scaled(low + 1) - target && (low & 1) !== 0)
        : rc === 1 ? negative : rc === 2 ? !negative : false;
      if (up) rounded++;
    }
    const result = { bits: (rounded | (negative ? 0x80000000 : 0)) >>> 0, flags: inexact ? 32 : 0 };
    i32ToF32Cache.set(key, result); return result;
  };
  const i32ToF32Values = [...new Set([0, 1, 2, 3, 0xffffffff, 0xfffffffe, 0x80000000, 0x80000001, 0x7fffffff,
    0x7f800000, 0x7fc00001, 0xff800000, 0xffc00001,
    ...Array.from({ length: 8 }, (_, i) => 23 + i).flatMap(bit => [-3, -2, -1, 0, 1, 2, 3, 63, 64, 65, 127].flatMap(delta =>
      [(2 ** bit + delta) >>> 0, (-(2 ** bit + delta)) >>> 0]))])];
  const cvtdqChanges = (before, destination, source, mxcsr) => {
    const converted = source.map(raw => i32ToF32Reference(raw, mxcsr)), flags = converted.reduce((all, value) => all | value.flags, 0);
    const trap = Boolean(flags & ~(mxcsr >>> 7)), changes = { 99: mxcsr | flags };
    if (!trap) converted.forEach((value, lane) => { changes[67 + destination * 4 + lane] = value.bits; });
    return { changes, trap };
  };
  await test("CVTDQ2PS pure raw numeric ABI observes RC and fresh PE without guest or x87 scratch mutation", () => fixture(
    [0x90], async ({ reset, state, bytes }) => {
      check(typeof api.backend_sse_i32_to_f32 === "function", "new pure conversion export exists");
      reset({ flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), image = bytes(DATA).slice();
      const pointer = hasTypedNumeric ? api.backend_fp_kernel_0(0x37f, 0, 0x80000000, 0x3fff, 0, 0x80000000, 0x3fff) : 0;
      const scratch = pointer ? Array.from(new Uint32Array(api.memory.buffer, pointer, 7)) : null;
      for (const raw of i32ToF32Values) for (let rc = 0; rc < 4; rc++) for (const extra of [0, 0x40, 0x8000, 0x8040]) for (const status of [0, 0x1fbf]) {
        const mxcsr = rc << 13 | extra | status, expected = i32ToF32Reference(raw, mxcsr), actual = api.backend_sse_i32_to_f32(mxcsr, raw, 0xffffffff);
        equal(Number(actual & 0xffffffffn), expected.bits, "raw binary32 agrees with adjacent-encoding oracle");
        equal(Number(actual >> 32n), expected.flags, "new PE ignores preexisting sticky, masks, DAZ, FTZ and unused high input");
      }
      equal(BigInt.asUintN(64, api.backend_sse_i32_to_f32(0x10000, 0, 0)), 1n << 63n, "invalid control has the exact ABI sentinel");
      for (let n = 0; n < 105; n++) equal(state()[n], before[n], "pure conversion never mutates architectural state");
      for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "pure conversion never touches guest bytes");
      if (scratch) for (let n = 0; n < 7; n++) equal(new Uint32Array(api.memory.buffer, pointer, 7)[n], scratch[n], "pure conversion preserves x87 kernel output");
    }));
  await test("CVTDQ2PS covers every XMM pair, all RC modes and signed integer bits including FP-looking patterns", async () => {
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
      [0x0f, 0x5b, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        for (let rc = 0; rc < 4; rc++) for (let at = 0; at < i32ToF32Values.length; at += 4) {
          const values = Array.from({ length: 4 }, (_, lane) => i32ToF32Values[(at + lane) % i32ToF32Values.length]), mxcsr = 0x9fdf | rc << 13;
          reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = mxcsr; putXmm(state(), source, values);
          const before = Array.from(state()), { changes } = cvtdqChanges(before, destination, values, mxcsr);
          checkScalarSseMove(run(1), before, changes, CODE + 3);
        }
      });
  });
  await test("CVTDQ2PS commits all four lanes together and traps only on newly unmasked precision", () => fixture(
    [0x0f, 0x5b, 0xc1], async ({ reset, state, run }) => {
      for (let rc = 0; rc < 4; rc++) for (const extra of [0, 0x40, 0x8000, 0x8040]) for (const masks of [0x1f80, 0x0f80, 0]) for (const sticky of [0, 0x20, 0x3f]) for (let lane = -1; lane < 4; lane++) {
        const values = [0, 1, 0x80000000, 0xffffffff]; if (lane >= 0) values[lane] = lane & 1 ? 0xfeffffff : 0x01000001;
        const mxcsr = rc << 13 | extra | masks | sticky;
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = mxcsr; putXmm(state(), 1, values);
        const before = Array.from(state()), { changes, trap } = cvtdqChanges(before, 0, values, mxcsr);
        checkScalarSseMove(run(0), before, {}, CODE, 0); const result = run(1);
        equal(result.kind, trap ? 16 : 1, "new precision versus old sticky contract"); checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 3), trap ? 0 : 1);
        if (trap) { equal(result.faultPc, CODE, "XM fault points to whole conversion"); equal(result.error, 0, "XM has no error code"); equal(result.address, 0, "XM has no memory address"); }
      }
    }));
  await test("CVTDQ2PS aligned full128 access precedes numeric state across FS wrap, missing and protection faults", () => fixture(
    [0x64, 0x0f, 0x5b, 0x13], async ({ reset, state, run, bytes, map, protect }) => {
      map(DATA + 4096, WRITE); const values = [0x01000001, 0xfeffffff, 0x7fffffff, 0x80000001];
      for (const [offset, base, rights, kind, error] of [[DATA, 0, READ, 16, 0], [DATA + 4080, 0, READ, 16, 0], [DATA + 4088, 0, READ, 6, 0],
        [DATA + 1, 0, WRITE, 6, 0], [0x5001, 0, READ, 6, 0], [DATA, 0, WRITE, 4, 5], [0x5000, 0, READ, 4, 4], [DATA + 8, 8, READ, 16, 0], [0xfffffff8, DATA + 24, READ, 6, 0]]) {
        protect(DATA, READ | WRITE); bytes(DATA).fill(0x55); for (const at of [0, 16, 4080]) values.forEach((word, lane) => setWord(bytes(DATA), at + lane * 4, word));
        reset({ ebx: offset, fs: base, flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x0f80; const before = Array.from(state()), image = bytes(DATA).slice(); protect(DATA, rights);
        const result = run(1); equal(result.kind, kind, "alignment/protection comes before numeric PE"); equal(result.error, error, "precise exception error");
        checkScalarSseMove(result, before, kind === 16 ? { 99: 0x0fa0 } : {}, CODE, 0);
        if (kind === 4) equal(result.address, offset, "unavailable aligned page");
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "conversion never writes source bytes");
      }
    }));
  await test("CVTDQ2PS missing-page repair then XM mask repair preserves prior ADD exactly once", () => fixture(
    [0x83, 0x07, 1, 0x0f, 0x5b, 0x13], async ({ reset, state, run, bytes, map, compile }) => {
      setWord(bytes(STACK), 0, 0x7fffffff); reset({ ebx: 0x5000, edi: STACK, flags: 0x203 }); seedScalarSseMove(state()); state()[99] = 0x0f80;
      const before = Array.from(state()), flags = (before[9] & ~ARITH) | arithmetic("add", 32, 0x7fffffff, 1, 0).flags;
      const fault = run(100); equal(fault.kind, 4, "missing source faults before conversion"); checkScalarSseMove(fault, before, { 9: flags }, CODE + 3, 1);
      const values = [0x01000001, 2, 0xfeffffff, 0x80000000], input = page(); values.forEach((word, lane) => setWord(input, lane * 4, word)); map(0x5000, READ, input); await compile(CODE + 3);
      const numericBefore = Array.from(state()), numeric = run(1); equal(numeric.kind, 16, "fully readable source now raises precision XM"); checkScalarSseMove(numeric, numericBefore, { 99: 0x0fa0 }, CODE + 3, 0);
      state()[99] |= 0x1000; const repairedBefore = Array.from(state()), { changes } = cvtdqChanges(repairedBefore, 2, values, repairedBefore[99]);
      checkScalarSseMove(run(1), repairedBefore, changes, CODE + 6);
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "prior ADD not replayed after either repair");
    }));
  await test("CVTDQ2PS result and sticky state survive executable alias store and exact SMC resume", () => fixture(
    [0x0f, 0x5b, 0xc1, 0x0f, 0x11, 0x03, 0x40], async ({ handle, reset, state, run, bytes, alias, compile }) => {
      alias(0x4000, CODE, READ | WRITE); reset({ ebx: 0x4101, eax: 9, flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x1f80;
      const values = [0x01000001, 0xfeffffff, 0x7fffffff, 0x80000000]; putXmm(state(), 1, values);
      const before = Array.from(state()), { changes } = cvtdqChanges(before, 0, values, before[99]), result = run(100);
      equal(result.kind, 7, "completed vector store forces SMC boundary"); checkScalarSseMove(result, before, changes, CODE + 6, 2);
      const output = new Uint8Array(new Uint32Array(values.map(raw => i32ToF32Reference(raw, before[99]).bits)).buffer);
      for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "stored rounded result");
      check(api.backend_validate(handle, CODE) !== 0, "old generated version invalidated"); await compile(CODE + 6); putXmm(state(), 0, [0, 0, 0, 0]);
      const resumedBefore = Array.from(state()); checkScalarSseMove(run(1), resumedBefore, { 0: 10, 9: arithmetic("add", 32, 9, 1, 0).flags | resumedBefore[9] & ~ARITH | resumedBefore[9] & CF }, CODE + 7);
      for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "resume cannot repeat conversion or store");
    }, READ | WRITE | EXECUTE | SEALED));
  for (const [code, kind] of [[[0x66, 0x0f, 0x5b, 0xc1], 3], [[0x67, 0x0f, 0x5b, 0x03], 3],
    [[0x65, 0x0f, 0x5b, 0x03], 3], [[0xc5, 0xf8, 0x5b, 0xc1], 3], [[0xf0, 0x0f, 0x5b, 0x03], 8]]) await test("CVTDQ2PS distinct encoding " + code.join("-") + " stops before effects", () => fixture(
      code, async ({ reset, state, run }) => { reset({ ebx: 0x5001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1); equal(result.kind, kind, "unsupported and invalid encodings stay distinct"); checkScalarSseMove(result, before, {}, CODE, 0); }));
  if (hasTimestamp && hasSseCvtt) await test("CVTDQ2PS shares numeric import numbering with both CVTTs, SCAS, x87 and timestamp", () => fixture(
    [0xf2, 0xae, 0xd9, 0xee, 0xd9, 0x1b, 0x0f, 0x5b, 0xf7, 0xf3, 0x0f, 0x2c, 0xc6, 0xf2, 0x0f, 0x2c, 0xd5, 0x0f, 0x31], async ({ handle, reset, state, run, bytes }) => {
      bytes(DATA)[0] = 3; reset({ eax: 3, ecx: 1, edi: DATA, ebx: STACK, flags: 0xad7 });
      putXmm(state(), 7, [3, 0x01000001, 0xfeffffff, 0x80000000]); putXmm(state(), 5, [0, 0x40180000, 0x7f800001, 0xffffffff]);
      const result = run(100); equal(result.kind, 15, "all imports reach RDTSC handoff"); equal(result.pc, CODE + 17, "mixed module timestamp PC"); equal(result.retired, 6, "six completed operations before RDTSC");
      equal(result.words[0], 3, "CVTT f32 consumes newly converted lane"); equal(result.words[2], 6, "CVTT f64 uses separate raw input"); equal(result.words[99], 0x1fa0, "packed precision remains sticky");
      equal(result.words[9], (0xad7 & ~ARITH) | 0x44, "noninteger operations preserve SCAS flags"); equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0, "x87 pure zero store");
      for (let lane = 0; lane < 4; lane++) equal(result.words[91 + lane], i32ToF32Reference([3, 0x01000001, 0xfeffffff, 0x80000000][lane], 0x1f80).bits, "full packed result persists");
      ok(api.backend_complete_timestamp_counter(handle, CODE + 17, 0x12345678, 9), "complete timestamp after shifted imports"); equal(state()[8], CODE + 19, "timestamp continuation");
    }));
  // Deliberately small exact-product cases with written binary32 outcomes.
  // Broader finite-value validation uses the independent encoding-search/native
  // oracle outside this generated-state suite; these cases test publication.
  const mulFiniteCases = new Map([
    ["1:1", { bits: [0, 0, 1, 0], tiny: true, boundedInexact: true }],
    ["1:3f000000", { bits: [0, 0, 1, 0], tiny: true, boundedInexact: true }],
    ["1:3f800000", { bits: [1, 1, 1, 1], tiny: true }],
    ["800000:3f000000", { bits: [0x00400000, 0x00400000, 0x00400000, 0x00400000], tiny: true }],
    ["3f800000:3f800000", { bits: [0x3f800000, 0x3f800000, 0x3f800000, 0x3f800000] }],
    ["3f800000:3f800001", { bits: [0x3f800001, 0x3f800001, 0x3f800001, 0x3f800001] }],
    ["3f800001:3f800001", { bits: [0x3f800002, 0x3f800002, 0x3f800003, 0x3f800002], unboundedInexact: true }],
    ["3f800001:3fc00000", { bits: [0x3fc00002, 0x3fc00001, 0x3fc00002, 0x3fc00001], unboundedInexact: true }],
    ["3fc00000:40000000", { bits: [0x40400000, 0x40400000, 0x40400000, 0x40400000] }],
    ["40000000:7f7fffff", { bits: [0x7f800000, 0x7f7fffff, 0x7f800000, 0x7f7fffff], overflow: true }],
    ["3f800001:7f7fffff", { bits: [0x7f800000, 0x7f7fffff, 0x7f800000, 0x7f7fffff], overflow: true, unboundedInexact: true }],
    ["7f7fffff:7f7fffff", { bits: [0x7f800000, 0x7f7fffff, 0x7f800000, 0x7f7fffff], overflow: true, unboundedInexact: true }],
  ]);
  const mulF32Reference = (a, b, mxcsr) => {
    a >>>= 0; b >>>= 0;
    const nan = value => (value & 0x7fffffff) > 0x7f800000;
    const snan = value => nan(value) && (value & 0x400000) === 0;
    if (nan(a) || nan(b)) return { bits: ((nan(a) ? a : b) | 0x400000) >>> 0, flags: snan(a) || snan(b) ? 1 : 0 };
    const sub = value => (value & 0x7fffffff) !== 0 && (value & 0x7f800000) === 0;
    const denormal = sub(a) || sub(b);
    if (mxcsr & 0x40) { if (sub(a)) a = (a & 0x80000000) >>> 0; if (sub(b)) b = (b & 0x80000000) >>> 0; }
    const sign = ((a ^ b) & 0x80000000) >>> 0, aa = a & 0x7fffffff, bb = b & 0x7fffffff;
    let flags = denormal && !(mxcsr & 0x40) ? 2 : 0;
    if ((aa === 0x7f800000 && bb === 0) || (bb === 0x7f800000 && aa === 0)) return { bits: 0xffc00000, flags: 1 };
    if (aa === 0x7f800000 || bb === 0x7f800000) return { bits: (sign | 0x7f800000) >>> 0, flags };
    if (aa === 0 || bb === 0) return { bits: sign, flags };
    const known = mulFiniteCases.get(Math.min(aa, bb).toString(16) + ":" + Math.max(aa, bb).toString(16));
    if (!known) throw Error("Missing written MULPS reference pair " + a.toString(16) + ":" + b.toString(16));
    const rc = mxcsr >>> 13 & 3, magnitudeRc = sign ? [0, 2, 1, 3][rc] : rc;
    let bits = known.bits[magnitudeRc];
    if (known.overflow) flags |= 8 | ((mxcsr & 0x400) || known.unboundedInexact ? 32 : 0);
    else if (known.tiny) {
      const masked = Boolean(mxcsr & 0x800), flush = masked && Boolean(mxcsr & 0x8000);
      if (!masked || known.boundedInexact || flush) flags |= 16;
      if (masked ? known.boundedInexact || flush : known.unboundedInexact) flags |= 32;
      if (flush) bits = 0;
    } else if (known.unboundedInexact) flags |= 32;
    return { bits: (bits | sign) >>> 0, flags };
  };
  const mulPairs = [
    [0, 0x3f800000], [0x80000000, 0x3f800000], [0x80000000, 0xbf800000], [0, 0x7f800000], [0xff800000, 0],
    [0x7f800000, 0xbf800000], [0x7f800000, 1], [0, 1], [1, 0x3f800000], [1, 0x3f000000], [0x80000001, 0x3f000000],
    [0x00800000, 0x3f000000], [0x80800000, 0x3f000000], [0x3f800001, 0x3f800001], [0xbf800001, 0x3f800001],
    [0x3f800001, 0x3fc00000], [0xbf800001, 0x3fc00000], [0x7f7fffff, 0x40000000], [0xff7fffff, 0x40000000],
    [0x7f7fffff, 0x3f800001], [0xff7fffff, 0x3f800001], [0x3fc00000, 0x40000000],
    [0x7fc00123, 1], [1, 0xffc00456], [0x7f800123, 0x3f800000], [0x3f800000, 0xff800456],
    [0x7fc00123, 0xff800456], [0x7f800123, 0xffc00456], [0x7f800123, 0xff800456], [0xffc00123, 0x7fc00456],
  ];
  const mulVectorChanges = (before, destination, left, right, mxcsr) => {
    const lanes = left.map((value, lane) => mulF32Reference(value, right[lane], mxcsr));
    const all = lanes.reduce((flags, lane) => flags | lane.flags, 0), unmasked = ~(mxcsr >>> 7);
    const fresh = all & 7 & unmasked ? all & 7 : all, trap = Boolean(fresh & unmasked), changes = { 99: mxcsr | fresh };
    if (!trap) lanes.forEach((lane, n) => { changes[67 + destination * 4 + n] = lane.bits; });
    return { changes, trap, fresh };
  };
  await test("MULPS pure numeric ABI keeps phase flags, rounding, signed zero and first-source NaN payload", () => fixture(
    [0x90], async ({ reset, state, bytes }) => {
      check(typeof api.backend_sse_mul_f32 === "function", "MULPS pure export exists"); reset({ flags: 0xad7 }); seedScalarSseMove(state());
      const before = Array.from(state()), image = bytes(DATA).slice();
      const pointer = hasTypedNumeric ? api.backend_fp_kernel_0(0x37f, 0, 0x80000000, 0x3fff, 0, 0x80000000, 0x3fff) : 0;
      const scratch = pointer ? Array.from(new Uint32Array(api.memory.buffer, pointer, 7)) : null;
      for (const [a, b] of mulPairs) for (let rc = 0; rc < 4; rc++) for (const extra of [0, 0x40, 0x8000, 0x8040]) for (const masks of [0, 0x400, 0x800, 0xc00, 0x1f80]) for (const sticky of [0, 0x3f]) {
        const mxcsr = rc << 13 | extra | masks | sticky, expected = mulF32Reference(a, b, mxcsr);
        const actual = BigInt.asUintN(64, api.backend_sse_mul_f32(mxcsr, a, b));
        equal(Number(actual >> 32n), expected.flags, "per-lane helper retains pre and post candidates and reports fresh flags");
        if ((expected.flags & ~(mxcsr >>> 7)) === 0) equal(Number(actual & 0xffffffffn), expected.bits, "committable raw bits match written exact-product/NaN oracle");
      }
      equal(BigInt.asUintN(64, api.backend_sse_mul_f32(0x10000, 0, 0)), 1n << 63n, "reserved MXCSR produces only invalid sentinel");
      for (let n = 0; n < 105; n++) equal(state()[n], before[n], "pure multiply has no CPU authority");
      for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "pure multiply has no memory effects");
      if (scratch) for (let n = 0; n < 7; n++) equal(new Uint32Array(api.memory.buffer, pointer, 7)[n], scratch[n], "pure multiply preserves x87 scratch");
    }));
  await test("MULPS all XMM pairs retain old destination inputs and alias lane order", async () => {
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
      [0x0f, 0x59, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        for (let rc = 0; rc < 4; rc++) for (const masks of [0, 0x1f80]) {
          reset({ flags: 0xad7 }); seedScalarSseMove(state()); const mxcsr = masks | rc << 13; state()[99] = mxcsr;
          const left = [0x7fc00123, 0x3f800001, 0x7f7fffff, 1], right = destination === source ? left : [0xff800456, 0x3fc00000, 0x40000000, 0x3f000000];
          putXmm(state(), destination, left); putXmm(state(), source, right); const before = Array.from(state());
          const { changes, trap } = mulVectorChanges(before, destination, xmmWords(before, destination), xmmWords(before, source), mxcsr);
          checkScalarSseMove(run(0), before, {}, CODE, 0); const result = run(1); equal(result.kind, trap ? 16 : 1, "whole vector commits or faults");
          checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 3), trap ? 0 : 1);
        }
      });
  });
  await test("MULPS full control and input classes use one whole-vector exception phase", () => fixture(
    [0x0f, 0x59, 0xc1], async ({ reset, state, run }) => {
      for (let at = 0; at < mulPairs.length; at += 4) for (let rc = 0; rc < 4; rc++) for (const extra of [0, 0x40, 0x8000, 0x8040]) for (let mask = 0; mask < 32; mask++) {
        const maskBits = (mask & 1 ? 0x80 : 0) | (mask & 2 ? 0x100 : 0) | (mask & 4 ? 0x400 : 0) | (mask & 8 ? 0x800 : 0) | (mask & 16 ? 0x1000 : 0);
        const mxcsr = maskBits | 0x200 | rc << 13 | extra | (at & 4 ? 0x3f : 0), pairs = Array.from({ length: 4 }, (_, lane) => mulPairs[(at + lane) % mulPairs.length]);
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = mxcsr; putXmm(state(), 0, pairs.map(pair => pair[0])); putXmm(state(), 1, pairs.map(pair => pair[1]));
        const before = Array.from(state()), { changes, trap } = mulVectorChanges(before, 0, xmmWords(before, 0), xmmWords(before, 1), mxcsr), result = run(1);
        equal(result.kind, trap ? 16 : 1, "only freshly unmasked exceptions raise XM"); checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 3), trap ? 0 : 1);
        if (trap) { equal(result.faultPc, CODE, "precise multiplication PC"); equal(result.error, 0, "XM error code"); equal(result.address, 0, "XM address"); }
      }
    }));
  await test("MULPS cross-lane unmasked IE or DE suppresses new OE UE PE but preserves old sticky", () => fixture(
    [0x0f, 0x59, 0xc1], async ({ reset, state, run }) => {
      const left = [0x7f800123, 1, 0x7f7fffff, 0x3f800001], right = [0x3f800000, 0x3f800000, 0x40000000, 0x3f800001];
      for (let mask = 0; mask < 64; mask++) for (const sticky of [0, 0x38, 0x3f]) {
        const mxcsr = mask << 7 | sticky; reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = mxcsr; putXmm(state(), 0, left); putXmm(state(), 1, right);
        const before = Array.from(state()), { changes, trap, fresh } = mulVectorChanges(before, 0, left, right, mxcsr), result = run(1);
        equal(fresh, (mask & 3) === 3 ? ((mask & 16) === 0 ? 0x3b : 0x2b) : 3, "known mixed-stage aggregate, including unmasked exact tiny UE");
        equal(result.kind, trap ? 16 : 1, "global stage gate"); checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 3), trap ? 0 : 1);
      }
    }));
  await test("MULPS full aligned memory access precedes operand exceptions and preserves memory", () => fixture(
    [0x64, 0x0f, 0x59, 0x13], async ({ reset, state, run, bytes, map, protect }) => {
      map(DATA + 4096, WRITE); const right = [0x3f800000, 0x3f800000, 0x40000000, 0x3f800001];
      for (const [offset, base, rights, kind, error] of [[DATA, 0, READ, 16, 0], [DATA + 4080, 0, READ, 16, 0], [DATA + 4088, 0, READ, 6, 0],
        [DATA + 1, 0, WRITE, 6, 0], [0x5001, 0, READ, 6, 0], [DATA, 0, WRITE, 4, 5], [0x5000, 0, READ, 4, 4], [DATA + 8, 8, READ, 16, 0], [0xfffffff8, DATA + 24, READ, 6, 0]]) {
        protect(DATA, READ | WRITE); bytes(DATA).fill(0x55); for (const at of [0, 16, 4080]) right.forEach((word, lane) => setWord(bytes(DATA), at + lane * 4, word));
        reset({ ebx: offset, fs: base, flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0; putXmm(state(), 2, [0x7f800123, 1, 0x7f7fffff, 0x3f800001]);
        const before = Array.from(state()), image = bytes(DATA).slice(); protect(DATA, rights); const result = run(1);
        equal(result.kind, kind, "alignment/protection before unmasked numeric input"); equal(result.error, error, "memory fault error code");
        checkScalarSseMove(result, before, kind === 16 ? { 99: 3 } : {}, CODE, 0); if (kind === 4) equal(result.address, offset, "unavailable aligned source");
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "MULPS only reads source bytes");
      }
    }));
  await test("MULPS repairs PF then pre-XM then post-XM without replaying a completed ADD", () => fixture(
    [0x83, 0x07, 1, 0x0f, 0x59, 0x13], async ({ reset, state, run, bytes, map, compile }) => {
      const left = [0x7f800123, 1, 0x7f7fffff, 0x3f800001], right = [0x3f800000, 0x3f800000, 0x40000000, 0x3f800001];
      setWord(bytes(STACK), 0, 0x7fffffff); reset({ ebx: 0x5000, edi: STACK, flags: 0x203 }); seedScalarSseMove(state()); state()[99] = 0; putXmm(state(), 2, left);
      const before = Array.from(state()), flags = before[9] & ~ARITH | arithmetic("add", 32, 0x7fffffff, 1, 0).flags, fault = run(100);
      equal(fault.kind, 4, "source missing before any numeric call"); checkScalarSseMove(fault, before, { 9: flags }, CODE + 3, 1);
      const input = page(); right.forEach((word, lane) => setWord(input, lane * 4, word)); map(0x5000, READ, input); await compile(CODE + 3);
      let prior = Array.from(state()), result = run(1); equal(result.kind, 16, "read repair reaches pre-computation exception"); checkScalarSseMove(result, prior, { 99: 3 }, CODE + 3, 0);
      state()[99] |= 0x180; prior = Array.from(state()); result = run(1); equal(result.kind, 16, "pre mask repair reaches post-computation exception"); checkScalarSseMove(result, prior, { 99: 0x1bb }, CODE + 3, 0);
      state()[99] |= 0x1f80; prior = Array.from(state()); const { changes } = mulVectorChanges(prior, 2, left, right, prior[99]); checkScalarSseMove(run(1), prior, changes, CODE + 6);
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "every repair starts at MULPS, never prior ADD");
    }));
  await test("MULPS exact operation does not retrigger old unmasked sticky exceptions", () => fixture(
    [0x0f, 0x59, 0xc1], async ({ reset, state, run }) => {
      reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x3f; putXmm(state(), 0, [0, 0x80000000, 0x3f800000, 0x3f800001]); putXmm(state(), 1, [0x3f800000, 0x3f800000, 0x3f800000, 0x3f800000]);
      const before = Array.from(state()), { changes } = mulVectorChanges(before, 0, xmmWords(before, 0), xmmWords(before, 1), before[99]); checkScalarSseMove(run(1), before, changes, CODE + 3);
    }));
  await test("MULPS masked results and sticky flags survive executable-alias SMC and exact resume", () => fixture(
    [0x0f, 0x59, 0xc1, 0x0f, 0x11, 0x03, 0x40], async ({ handle, reset, state, run, bytes, alias, compile }) => {
      alias(0x4000, CODE, READ | WRITE); reset({ ebx: 0x4101, eax: 9, flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x1f80;
      const left = [0x7f800123, 1, 0x7f7fffff, 0x3f800001], right = [0x3f800000, 0x3f800000, 0x40000000, 0x3f800001]; putXmm(state(), 0, left); putXmm(state(), 1, right);
      const before = Array.from(state()), { changes } = mulVectorChanges(before, 0, left, right, before[99]), result = run(100);
      equal(result.kind, 7, "vector store completes before SMC exit"); checkScalarSseMove(result, before, changes, CODE + 6, 2);
      const output = new Uint8Array(new Uint32Array(left.map((raw, lane) => mulF32Reference(raw, right[lane], before[99]).bits)).buffer);
      for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "full rounded/nan vector stored");
      check(api.backend_validate(handle, CODE) !== 0, "old generated code invalidated"); await compile(CODE + 6); putXmm(state(), 0, [0, 0, 0, 0]);
      const resumedBefore = Array.from(state()); checkScalarSseMove(run(1), resumedBefore, { 0: 10, 9: arithmetic("add", 32, 9, 1, 0).flags | resumedBefore[9] & ~ARITH | resumedBefore[9] & CF }, CODE + 7);
      for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "SMC resume never repeats multiply/store");
    }, READ | WRITE | EXECUTE | SEALED));
  for (const [code, kind] of [[[0xf3, 0x0f, 0x59, 0xc1], 3],
    [[0x67, 0x0f, 0x59, 0x03], 3], [[0x65, 0x0f, 0x59, 0x03], 3], [[0xc5, 0xf8, 0x59, 0xc1], 3], [[0xf0, 0x0f, 0x59, 0x03], 8]]) await test("MULPS distinct encoding " + code.join("-") + " stops before effects", () => fixture(
      code, async ({ reset, state, run }) => { reset({ ebx: 0x5001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1); equal(result.kind, kind, "unsupported versus invalid encoding"); checkScalarSseMove(result, before, {}, CODE, 0); }));
  if (hasTimestamp && hasSseCvtt) await test("MULPS coexists with all SSE numeric imports, x87, SCAS and timestamp", () => fixture(
    [0xf2, 0xae, 0xd9, 0xee, 0xd9, 0x1b, 0x0f, 0x5b, 0xf7, 0x0f, 0x59, 0xf4, 0xf3, 0x0f, 0x2c, 0xc6, 0xf2, 0x0f, 0x2c, 0xd5, 0x0f, 0x31], async ({ handle, reset, state, run, bytes }) => {
      bytes(DATA)[0] = 3; reset({ eax: 3, ecx: 1, edi: DATA, ebx: STACK, flags: 0xad7 }); putXmm(state(), 7, [1, 1, 1, 1]);
      putXmm(state(), 4, [0x3f800001, 0x3f800001, 0x3f800001, 0x3f800001]); putXmm(state(), 5, [0, 0x40180000, 0, 0]);
      const result = run(100); equal(result.kind, 15, "four numeric imports and SCAS/x87 reach timestamp"); equal(result.pc, CODE + 20, "timestamp PC"); equal(result.retired, 7, "seven completed operations");
      equal(result.words[0], 1, "CVTT f32 reads multiplied lane"); equal(result.words[2], 6, "CVTT f64 uses its own import"); equal(result.words[99], 0x1fa0, "CVTT inexact remains sticky");
      for (let lane = 0; lane < 4; lane++) equal(result.words[91 + lane], 0x3f800001, "CVTDQ then MULPS result");
      equal(result.words[9], (0xad7 & ~ARITH) | 0x44, "numeric ops preserve SCAS flags"); equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0, "x87 store unaffected by imports");
      ok(api.backend_complete_timestamp_counter(handle, CODE + 20, 0x12345678, 9), "complete mixed module timestamp"); equal(state()[8], CODE + 22, "timestamp continuation");
    }));
  // Independent exact-sum oracle: binary32 values are integer multiples of
  // 2^-149. Search ordered positive encodings for adjacent rounding candidates;
  // this does not duplicate the numeric kernel's limb arithmetic/normalization.
  const addSubOracleCache = new Map();
  const addSubF32Reference = (a, b, mxcsr, subtract) => {
    a >>>= 0; b >>>= 0;
    const key = [a, b, mxcsr & 0xec40, subtract].join(":");
    if (addSubOracleCache.has(key)) return addSubOracleCache.get(key);
    const result = (() => {
      const nan = x => (x & 0x7fffffff) > 0x7f800000, snan = x => nan(x) && !(x & 0x400000);
      if (nan(a) || nan(b)) return { bits: ((nan(a) ? a : b) | 0x400000) >>> 0, flags: snan(a) || snan(b) ? 1 : 0 };
      const tiny = x => (x & 0x7fffffff) !== 0 && (x & 0x7f800000) === 0;
      let flags = !(mxcsr & 0x40) && (tiny(a) || tiny(b)) ? 2 : 0;
      if (mxcsr & 0x40) { if (tiny(a)) a = (a & 0x80000000) >>> 0; if (tiny(b)) b = (b & 0x80000000) >>> 0; }
      if (subtract) b = (b ^ 0x80000000) >>> 0;
      const aa = a & 0x7fffffff, bb = b & 0x7fffffff, rc = mxcsr >>> 13 & 3;
      if (aa === 0x7f800000 || bb === 0x7f800000) {
        if (aa === bb && (a >>> 31) !== (b >>> 31)) return { bits: 0xffc00000, flags: 1 };
        return { bits: aa === 0x7f800000 ? a : b, flags };
      }
      const units = raw => { const exponent = Math.floor(raw / 0x800000), fraction = BigInt(raw % 0x800000); return exponent === 0 ? fraction : (0x800000n + fraction) << BigInt(exponent - 1); };
      const exact = units(aa) * (a >>> 31 ? -1n : 1n) + units(bb) * (b >>> 31 ? -1n : 1n);
      if (exact === 0n) return { bits: aa === 0 && bb === 0 && a >>> 31 === b >>> 31 ? a : rc === 1 ? 0x80000000 : 0, flags };
      const negative = exact < 0n, magnitude = negative ? -exact : exact;
      const mode = negative && (rc === 1 || rc === 2) ? 3 - rc : rc;
      let lo = 0, hi = 0x80000000;
      while (lo < hi) { const mid = Math.floor((lo + hi + 1) / 2); if (units(mid) <= magnitude) lo = mid; else hi = mid - 1; }
      const lower = units(lo), inexact = lower !== magnitude;
      let rounded = lo;
      if (inexact && (mode === 2 || (mode === 0 && (magnitude - lower > units(lo + 1) - magnitude || (magnitude - lower === units(lo + 1) - magnitude && (lo & 1)))))) rounded++;
      const sign = negative ? 0x80000000 : 0;
      if (rounded >= 0x7f800000) {
        flags |= 8 | (mxcsr & 0x400 || inexact ? 32 : 0);
        return { bits: (sign | (mode === 0 || mode === 2 ? 0x7f800000 : 0x7f7fffff)) >>> 0, flags };
      }
      if (inexact) flags |= 32;
      if (rounded < 0x800000) {
        if (!(mxcsr & 0x800)) flags |= 16;
        else if (mxcsr & 0x8000) return { bits: sign, flags: flags | 48 };
      }
      return { bits: (sign | rounded) >>> 0, flags };
    })();
    addSubOracleCache.set(key, result); return result;
  };
  const addSubVectorChanges = (before, destination, left, right, mxcsr, subtract) => {
    const lanes = left.map((raw, lane) => addSubF32Reference(raw, right[lane], mxcsr, subtract));
    const all = lanes.reduce((flags, lane) => flags | lane.flags, 0), masks = mxcsr >>> 7;
    const fresh = all & 7 & ~masks ? all & 7 : all, trap = (fresh & ~masks) !== 0;
    const changes = { 99: mxcsr | fresh };
    if (!trap) lanes.forEach((lane, n) => { changes[67 + destination * 4 + n] = lane.bits; });
    return { changes, trap, fresh };
  };
  const addSubPairs = [
    [0, 0], [0x80000000, 0], [0, 0x80000000], [0x80000000, 0x80000000],
    [0x3f800000, 0x3f800000], [0xbf800000, 0x3f800000], [0x3f800000, 0xbf800000],
    [0x3f800000, 0x33800000], [0x3f800001, 0x33800000], [0xbf800000, 0xb3800000],
    [0x7f7fffff, 0x7f7fffff], [0x7f7fffff, 0x7f000000], [0xff7fffff, 0xff7fffff],
    [0x00800000, 0x807fffff], [1, 0], [1, 1], [0x007fffff, 1], [0x00800000, 0x807ffffe],
    [0x7f800000, 0x7f800000], [0x7f800000, 0xff800000], [0xff800000, 0], [1, 0x7f800000],
    [0x7f800123, 0xffc00456], [0x7fc00123, 0xff800456], [0x3f800000, 0xff800456], [1, 0xffc00456],
    [0x4b800000, 0x3f800000], [0xcb800000, 0xbf800000], [0x7f7fffff, 1], [0x00800000, 1],
  ];
  for (const [name, byte, subtract, exportName] of [["ADDPS", 0x58, false, "backend_sse_add_f32"], ["SUBPS", 0x5c, true, "backend_sse_sub_f32"]]) {
    await test(name + " finite packed sums retain exact flags across exponent gaps and domain boundaries", () => fixture(
      [0x0f, byte, 0xc1], async ({ reset, state, run }) => {
        const pairs = [[0x0c800001,0x8c800000],[0x0c800000,0x8c7fffff],[0x737fffff,0x737fffff],
          [0x73800000,0xf37fffff],[0,0x80000000],[0x80000000,0x80000000],[0x3f800000,0],
          ...[0,1,23,24,29,30,52,53,54,100,205].flatMap(gap => [0,0x80000000].flatMap(sign => [
            [(230<<23)|0x7fffff,((230-gap)<<23)|sign|0x7fffff],
            [((230-gap)<<23)|sign,230<<23],
          ]))];
        for (const pair of pairs) for (let rc=0;rc<4;rc++) for (const mx of [0x1f80,0x0f80,0x9fc0,0x8fff]) {
          const mxcsr=mx|rc<<13,left=[pair[0]>>>0,pair[1]>>>0,0x80000000,0x0c800001],right=[pair[1]>>>0,pair[0]>>>0,0,0x8c800000];
          reset({flags:0xad7});seedScalarSseMove(state());state()[99]=mxcsr;putXmm(state(),0,left);putXmm(state(),1,right);
          const before=Array.from(state()),{changes,trap}=addSubVectorChanges(before,0,left,right,mxcsr,subtract),result=run(1);
          equal(result.kind,trap?16:1,"exact result or freshly unmasked PE");checkScalarSseMove(result,before,changes,CODE+(trap?0:3),trap?0:1);
        }
      }));
    await test(name + " finite register alias uses original four lanes and preserves MMX raw80", () => fixture(
      [0x0f,byte,0xc0],async({reset,state,run})=>{
        for(const mxcsr of [0x1f80,0x3f,0x9fc0,0x5f80]){
          reset({flags:0xad7});seedScalarSseMove(state());state()[99]=mxcsr;const input=[0x80000000,0x3f800001,0x0c800001,0x737fffff];putXmm(state(),0,input);
          const before=Array.from(state()),{changes,trap}=addSubVectorChanges(before,0,input,input,mxcsr,subtract),result=run(1);
          equal(result.kind,trap?16:1,"aliased vector completion");checkScalarSseMove(result,before,changes,CODE+(trap?0:3),trap?0:1);
        }
      }));
    await test(name + " finite PF then precision XM restart never repeats an earlier store",()=>fixture(
      [0x83,0x07,1,0x0f,byte,0x03],async({reset,state,run,bytes,map,compile,saveSnapshot,restoreSnapshot})=>{
        const left=[0x3f800000,0x0c800001,0,0x737fffff],right=[0x33800000,0x8c800000,0x80000000,0x0c800000];
        reset({edi:STACK,ebx:0x6000,flags:0x203});seedScalarSseMove(state());state()[99]=0x0f80;putXmm(state(),0,left);setWord(bytes(STACK),0,0x7fffffff);
        const first=run(100);equal(first.kind,4,"source fault precedes arithmetic");equal(first.retired,1,"only preceding store completed");equal(first.pc,CODE+3,"faulting SIMD instruction remains current");
        const input=page();right.forEach((v,i)=>setWord(input,4*i,v));map(0x6000,READ,input);await compile(CODE+3);
        let before=Array.from(state()),expected=addSubVectorChanges(before,0,left,right,before[99],subtract),result=run(1);
        check(expected.trap,"test requires a newly unmasked PE");equal(result.kind,16,"precision before destination commit");checkScalarSseMove(result,before,expected.changes,CODE+3,0);
        const saved=saveSnapshot();reset({eax:0xdeadbeef});restoreSnapshot(saved);state()[99]|=0x1000;
        before=Array.from(state());expected=addSubVectorChanges(before,0,left,right,before[99],subtract);result=run(1);checkScalarSseMove(result,before,expected.changes,CODE+6);
        equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset).getUint32(0,true),0x80000000,"completed store happens once across both repairs and context restore");
      }));
    await test(name + " pure ABI matches exact-sum encoding-search oracle and has no CPU/memory effects", () => fixture(
      [0x0f, byte, 0xc1], async ({ reset, state, bytes }) => {
        check(typeof api[exportName] === "function", name + " export exists"); reset({ flags: 0xad7 }); seedScalarSseMove(state());
        const before = Array.from(state()), image = bytes(DATA).slice();
        for (const [a, b] of addSubPairs) for (let rc = 0; rc < 4; rc++) for (const extra of [0, 0x40, 0x8000, 0x8040]) for (const masks of [0, 0x400, 0x800, 0x1f80]) {
          const mxcsr = rc << 13 | extra | masks | 0x3f, expected = addSubF32Reference(a, b, mxcsr, subtract);
          const actual = BigInt.asUintN(64, api[exportName](mxcsr, a, b));
          equal(Number(actual >> 32n), expected.flags, name + " fresh per-lane flags");
          if (!(expected.flags & ~(mxcsr >>> 7))) equal(Number(actual & 0xffffffffn), expected.bits, name + " committable raw result");
        }
        equal(BigInt.asUintN(64, api[exportName](0x10000, 0, 0)), 1n << 63n, "invalid MXCSR sentinel");
        for (let n = 0; n < 105; n++) equal(state()[n], before[n], "pure numeric has no CPU authority");
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "pure numeric has no guest memory effects");
      }));
    await test(name + " all register pairs and aliases preserve old inputs and all unrelated state", async () => {
      for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
        [0x0f, byte, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
          for (let rc = 0; rc < 4; rc++) for (const masks of [0, 0x1f80]) {
            reset({ flags: 0xad7 }); seedScalarSseMove(state()); const mxcsr = masks | rc << 13; state()[99] = mxcsr;
            putXmm(state(), destination, [0x7fc00123, 0x3f800001, 0x7f7fffff, 1]);
            if (source !== destination) putXmm(state(), source, [0xff800456, 0x33800000, subtract ? 0xff7fffff : 0x7f7fffff, 0]);
            const before = Array.from(state()), { changes, trap } = addSubVectorChanges(before, destination, xmmWords(before, destination), xmmWords(before, source), mxcsr, subtract);
            checkScalarSseMove(run(0), before, {}, CODE, 0); const result = run(1); equal(result.kind, trap ? 16 : 1, "atomic vector or XM");
            checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 3), trap ? 0 : 1);
          }
        });
    });
    await test(name + " input classes RC DAZ FTZ masks and old sticky use global exception priority", () => fixture(
      [0x0f, byte, 0xc1], async ({ reset, state, run }) => {
        for (let at = 0; at < addSubPairs.length; at += 4) for (let rc = 0; rc < 4; rc++) for (const extra of [0, 0x40, 0x8000, 0x8040]) for (let mask = 0; mask < 32; mask++) {
          const mxcsr = (mask & 1 ? 0x80 : 0) | (mask & 2 ? 0x100 : 0) | 0x200 | (mask & 4 ? 0x400 : 0) | (mask & 8 ? 0x800 : 0) | (mask & 16 ? 0x1000 : 0) | rc << 13 | extra | (at & 4 ? 0x3f : 0);
          const pairs = Array.from({ length: 4 }, (_, lane) => addSubPairs[(at + lane) % addSubPairs.length]);
          reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = mxcsr; putXmm(state(), 0, pairs.map(pair => pair[0])); putXmm(state(), 1, pairs.map(pair => pair[1]));
          const before = Array.from(state()), { changes, trap } = addSubVectorChanges(before, 0, xmmWords(before, 0), xmmWords(before, 1), mxcsr, subtract), result = run(1);
          equal(result.kind, trap ? 16 : 1, "fresh exceptions determine XM"); checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 3), trap ? 0 : 1);
          if (trap) { equal(result.faultPc, CODE, "faulting arithmetic PC"); equal(result.error, 0, "XM error"); equal(result.address, 0, "XM address"); }
        }
      }));
    const mixedLeft = [0x7f800123, 1, 0x7f7fffff, 0x3f800000], mixedRight = [0x3f800000, 0, subtract ? 0xff7fffff : 0x7f7fffff, subtract ? 0xb3800000 : 0x33800000];
    await test(name + " cross-lane IE DE suppress new post flags and preserve old sticky", () => fixture(
      [0x0f, byte, 0xc1], async ({ reset, state, run }) => {
        for (let mask = 0; mask < 64; mask++) for (const sticky of [0, 0x38, 0x3f]) {
          const mxcsr = mask << 7 | sticky; reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = mxcsr; putXmm(state(), 0, mixedLeft); putXmm(state(), 1, mixedRight);
          const before = Array.from(state()), { changes, trap, fresh } = addSubVectorChanges(before, 0, mixedLeft, mixedRight, mxcsr, subtract), result = run(1);
          equal(fresh, (mask & 3) === 3 ? ((mask & 16) === 0 ? 0x3b : 0x2b) : 3, "independent mixed-stage known flags");
          equal(result.kind, trap ? 16 : 1, "global gate"); checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 3), trap ? 0 : 1);
        }
      }));
    await test(name + " full aligned memory preflight precedes numeric flags including segment wrap", () => fixture(
      [0x64, 0x0f, byte, 0x13], async ({ reset, state, run, bytes, map, protect }) => {
        map(DATA + 4096, WRITE);
        for (const [offset, base, rights, kind, error] of [[DATA, 0, READ, 16, 0], [DATA + 4080, 0, READ, 16, 0], [DATA + 4088, 0, READ, 6, 0], [DATA + 1, 0, WRITE, 6, 0], [0x5001, 0, READ, 6, 0], [DATA, 0, WRITE, 4, 5], [0x5000, 0, READ, 4, 4], [DATA + 8, 8, READ, 16, 0], [0xfffffff8, DATA + 24, READ, 6, 0]]) {
          protect(DATA, READ | WRITE); bytes(DATA).fill(0x55); for (const at of [0, 16, 4080]) mixedRight.forEach((word, lane) => setWord(bytes(DATA), at + lane * 4, word));
          reset({ ebx: offset, fs: base, flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0; putXmm(state(), 2, mixedLeft); const before = Array.from(state()), image = bytes(DATA).slice(); protect(DATA, rights);
          const result = run(1); equal(result.kind, kind, "full source access before arithmetic"); equal(result.error, error, "fault error"); checkScalarSseMove(result, before, kind === 16 ? { 99: 3 } : {}, CODE, 0);
          for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "arithmetic reads only");
        }
      }));
    await test(name + " repairs PF pre-XM post-XM without replay of completed memory ADD", () => fixture(
      [0x83, 0x07, 1, 0x0f, byte, 0x13], async ({ reset, state, run, bytes, map, compile }) => {
        setWord(bytes(STACK), 0, 0x7fffffff); reset({ ebx: 0x5000, edi: STACK, flags: 0x203 }); seedScalarSseMove(state()); state()[99] = 0; putXmm(state(), 2, mixedLeft);
        const before = Array.from(state()), flags = before[9] & ~ARITH | arithmetic("add", 32, 0x7fffffff, 1, 0).flags, fault = run(100);
        equal(fault.kind, 4, "source PF first"); checkScalarSseMove(fault, before, { 9: flags }, CODE + 3, 1);
        const input = page(); mixedRight.forEach((word, lane) => setWord(input, lane * 4, word)); map(0x5000, READ, input); await compile(CODE + 3);
        let prior = Array.from(state()), result = run(1); equal(result.kind, 16, "pre exception"); checkScalarSseMove(result, prior, { 99: 3 }, CODE + 3, 0);
        state()[99] |= 0x180; prior = Array.from(state()); result = run(1); equal(result.kind, 16, "post exception"); checkScalarSseMove(result, prior, { 99: 0x1bb }, CODE + 3, 0);
        state()[99] |= 0x1f80; prior = Array.from(state()); const { changes } = addSubVectorChanges(prior, 2, mixedLeft, mixedRight, prior[99], subtract); checkScalarSseMove(run(1), prior, changes, CODE + 6);
        equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "completed ADD happened once");
      }));
    await test(name + " exact operation ignores old unmasked sticky and keeps pending x87", () => fixture(
      [0x0f, byte, 0xc1], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x3f; putXmm(state(), 0, [0, 0x80000000, 0x3f800000, 0xbf800000]); putXmm(state(), 1, [0, 0, 0, 0]);
        const before = Array.from(state()), { changes } = addSubVectorChanges(before, 0, xmmWords(before, 0), xmmWords(before, 1), before[99], subtract); checkScalarSseMove(run(1), before, changes, CODE + 3);
      }));
    await test(name + " result survives executable alias store and SMC resume without replay", () => fixture(
      [0x0f, byte, 0xc1, 0x0f, 0x11, 0x03, 0x40], async ({ handle, reset, state, run, bytes, alias, compile }) => {
        alias(0x4000, CODE, READ | WRITE); reset({ ebx: 0x4101, eax: 9, flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x1f80; putXmm(state(), 0, mixedLeft); putXmm(state(), 1, mixedRight);
        const before = Array.from(state()), { changes } = addSubVectorChanges(before, 0, mixedLeft, mixedRight, before[99], subtract), result = run(100);
        equal(result.kind, 7, "store SMC exit"); checkScalarSseMove(result, before, changes, CODE + 6, 2);
        const output = new Uint8Array(new Uint32Array(mixedLeft.map((raw, lane) => addSubF32Reference(raw, mixedRight[lane], before[99], subtract).bits)).buffer);
        for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "rounded vector stored");
        check(api.backend_validate(handle, CODE) !== 0, "old unit invalidated"); await compile(CODE + 6); putXmm(state(), 0, [0, 0, 0, 0]);
        const resumed = Array.from(state()); checkScalarSseMove(run(1), resumed, { 0: 10, 9: arithmetic("add", 32, 9, 1, 0).flags | resumed[9] & ~ARITH | resumed[9] & CF }, CODE + 7);
        for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "resume never replays arithmetic/store");
      }, READ | WRITE | EXECUTE | SEALED));
    for (const [code, kind] of [[[0xf3, 0x0f, byte, 0xc1], 3], [[0x67, 0x0f, byte, 0x03], 3], [[0x65, 0x0f, byte, 0x03], 3], [[0xc5, 0xf8, byte, 0xc1], 3], [[0xf0, 0x0f, byte, 0x03], 8]]) await test(name + " unsupported encoding " + code.join("-"), () => fixture(
      code, async ({ reset, state, run }) => { reset({ ebx: 0x5001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1); equal(result.kind, kind, "unsupported versus invalid"); checkScalarSseMove(result, before, {}, CODE, 0); }));
  }
  await test("Packed conversions compare complete lane results near exact-integer and truncation boundaries",async()=>{
    for(const convert of [true,false])await fixture(convert?[0x0f,0x5b,0xc1]:[0xf3,0x0f,0x5b,0xc1],async({reset,state,run})=>{
      const vectors=convert?[[0,1,0xffffffff,0x01000000],[0xff000000,0x00ffffff,0xff000001,9],[0x01000001,1,2,3],[0x80000000,0x7fffffff,0,1]]:
        [[0,0x80000000,0x00800000,0x80800000],[0x4effffff,0xcf000000,0x3f800001,0xbf800001],[0x4f000000,0xcf000001,0x3fc00000,0],
          [1,0x80000001,0x7f800123,0x3fc00000],[0x4b800000,0xcb800000,0x3f7fffff,0xbf7fffff]];
      for(const input of vectors)for(let rc=0;rc<4;rc++)for(const flags of [0x1f80,0x0f80,0x8fc0,0x3f]){
        const mxcsr=flags|rc<<13;reset({flags:0xad7});seedScalarSseMove(state());state()[99]=mxcsr;putXmm(state(),1,input);
        const before=Array.from(state()),{changes,trap}=convert?cvtdqChanges(before,0,input,mxcsr):cvttpsChanges(before,0,input,mxcsr),result=run(1);
        equal(result.kind,trap?16:1,"all lanes aggregate before a conversion commit");checkScalarSseMove(result,before,changes,CODE+(trap?0:convert?3:4),trap?0:1);
      }
    });
  });
  await test("Packed numeric qualified inputs still reject malformed MXCSR without architectural mutation",async()=>{
    for(const code of [[0x0f,0x5b,0xc1],[0xf3,0x0f,0x5b,0xc1],[0x0f,0x58,0xc1],[0x0f,0x5c,0xc1]])await fixture(code,async({reset,state,run})=>{
      reset({flags:0xad7});seedScalarSseMove(state());state()[99]=0x11f80;putXmm(state(),0,[0,0,0,0]);putXmm(state(),1,[0,0,0,0]);
      const before=Array.from(state()),result=run(1);equal(result.kind,3,"malformed control rejected by precise boundary");checkScalarSseMove(result,before,{},CODE,0);
    });
  });
  if (hasTimestamp && hasSseCvtt) await test("ADDPS SUBPS coexist with six SSE imports x87 SCAS and timestamp", () => fixture(
    [0xf2, 0xae, 0xd9, 0xee, 0xd9, 0x1b, 0x0f, 0x5b, 0xf7, 0x0f, 0x59, 0xf4, 0x0f, 0x58, 0xf4, 0x0f, 0x5c, 0xf4, 0xf3, 0x0f, 0x2c, 0xc6, 0xf2, 0x0f, 0x2c, 0xd5, 0x0f, 0x31], async ({ handle, reset, state, run, bytes }) => {
      bytes(DATA)[0] = 3; reset({ eax: 3, ecx: 1, edi: DATA, ebx: STACK, flags: 0xad7 }); putXmm(state(), 7, [1, 1, 1, 1]);
      putXmm(state(), 4, [0x3f800001, 0x3f800001, 0x3f800001, 0x3f800001]); putXmm(state(), 5, [0, 0x40180000, 0, 0]);
      const result = run(100); equal(result.kind, 15, "six numeric imports reach timestamp"); equal(result.pc, CODE + 26, "timestamp PC"); equal(result.retired, 9, "nine completed operations");
      equal(result.words[0], 1, "CVTT f32 after arithmetic"); equal(result.words[2], 6, "CVTT f64 import"); equal(result.words[99], 0x1fa0, "precision sticky");
      for (let lane = 0; lane < 4; lane++) equal(result.words[91 + lane], 0x3f800001, "full arithmetic pipeline result");
      equal(result.words[9], (0xad7 & ~ARITH) | 0x44, "SCAS flags preserved"); equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0, "x87 store");
      ok(api.backend_complete_timestamp_counter(handle, CODE + 26, 0x12345678, 9), "timestamp after six numeric imports"); equal(state()[8], CODE + 28, "timestamp continuation");
    }));
  for (const sample of [
    { name: "CVTDQ2PS precision", code: [0x0f, 0x5b, 0xc1], mxcsr: 0x0f90, fresh: 32,
      left: [0, 0, 0, 0], right: [0x01000001, 1, 2, 3] },
    { name: "CVTTPS2DQ invalid before other-lane precision", code: [0xf3, 0x0f, 0x5b, 0xc1], mxcsr: 0x0f10, fresh: 1,
      left: [0, 0, 0, 0], right: [0x7f800001, 0x3fc00000, 0x3f800000, 0] },
    { name: "MULPS invalid before other-lane precision", code: [0x0f, 0x59, 0xc1], mxcsr: 0x0f10, fresh: 1,
      left: [0x7f800001, 0x3f800001, 0x3f800000, 0x3f800000], right: [0x3f800000, 0x3f800001, 0x3f800000, 0x3f800000] },
    { name: "ADDPS precision", code: [0x0f, 0x58, 0xc1], mxcsr: 0x0f90, fresh: 32,
      left: [0x3f800000, 0, 0x3f800000, 0x40000000], right: [0x33800000, 0, 0, 0] },
    { name: "SUBPS invalid before other-lane precision", code: [0x0f, 0x5c, 0xc1], mxcsr: 0x0f10, fresh: 1,
      left: [0x7f800000, 0x3f800000, 0x3f800000, 0x40000000], right: [0x7f800000, 0x33000000, 0, 0] },
  ]) await test(`${sample.name} register XM keeps preceding ADD flags in the optimized complete block`, () => fixture(
    // The later CMP kills the SUB flags, retaining a real complete-block
    // optimized path even after the numeric instruction becomes a barrier.
    [0x83, 0xc0, 1, ...sample.code, 0x83, 0xeb, 1, 0x83, 0xfa, 0], async ({ reset, state, run }) => {
      reset({ eax: 0x7fffffff, ebx: 0x12345678, edx: 9, flags: 0x203 }); seedScalarSseMove(state());
      state()[99] = sample.mxcsr; putXmm(state(), 0, sample.left); putXmm(state(), 1, sample.right);
      const before = Array.from(state()), flags = (before[9] & ~ARITH) | arithmetic("add", 32, 0x7fffffff, 1, 0).flags;
      const result = run(100);
      equal(result.kind, 16, "numeric exception occurs before subsequent SUB/CMP");
      equal(result.faultPc, CODE + 3, "exact unretired numeric instruction");
      equal(result.address, 0, "XM address"); equal(result.error, 0, "XM error");
      checkScalarSseMove(result, before, { 0: 0x80000000, 9: flags, 99: sample.mxcsr | sample.fresh }, CODE + 3, 1);
    }));
  for (const toGpr of [false, true]) await test(`MOVD SSE ${toGpr ? "XMM to GPR" : "GPR to XMM"} transfers raw32 and preserves the other register bank`, async () => {
    for (let index = 0; index < 8; index++) {
      const xmm = toGpr ? (index + 3) & 7 : index, gpr = toGpr ? index : (index + 2) & 7;
      await fixture([0x66, 0x0f, toGpr ? 0x7e : 0x6e, 0xc0 | xmm << 3 | gpr], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[gpr] = 0x89abcdef;
        const before = Array.from(state());
        checkScalarSseMove(run(0), before, {}, CODE, 0);
        const changes = toGpr ? { [gpr]: before[67 + xmm * 4] }
          : { [67 + xmm * 4]: before[gpr], [68 + xmm * 4]: 0, [69 + xmm * 4]: 0, [70 + xmm * 4]: 0 };
        checkScalarSseMove(run(1), before, changes, CODE + 4);
      });
    }
  });
  for (const operation of [{ name: "MOVSD scalar", prefix: 0xf2, words: 2 }, { name: "MOVSS", prefix: 0xf3, words: 1 }])
    for (const storeEncoding of [false, true]) await test(`${operation.name} register ${storeEncoding ? "store" : "load"} encoding preserves untouched destination lanes`, async () => {
    for (let destination = 0; destination < 8; destination++) for (const source of [destination, (destination + 1) & 7]) {
      await fixture([operation.prefix, 0x0f, storeEncoding ? 0x11 : 0x10,
        0xc0 | (storeEncoding ? source : destination) << 3 | (storeEncoding ? destination : source)], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state());
        const changes = {};
        for (let n = 0; n < operation.words; n++) changes[67 + destination * 4 + n] = before[67 + source * 4 + n];
        checkScalarSseMove(run(1), before, changes, CODE + 4);
      });
    }
  });
  for (const storeEncoding of [false, true]) await test(`MOVQ XMM register ${storeEncoding ? "store" : "load"} encoding clears upper64 including source/destination alias`, async () => {
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) {
      await fixture([storeEncoding ? 0x66 : 0xf3, 0x0f, storeEncoding ? 0xd6 : 0x7e,
        0xc0 | (storeEncoding ? source : destination) << 3 | (storeEncoding ? destination : source)], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state());
        checkScalarSseMove(run(0), before, {}, CODE, 0);
        checkScalarSseMove(run(1), before, { [67 + destination * 4]: before[67 + source * 4],
          [68 + destination * 4]: before[68 + source * 4], [69 + destination * 4]: 0, [70 + destination * 4]: 0 }, CODE + 4);
      });
    }
  });
  await test("MOVQ displacement load reads the exact final eight bytes and retains x87/MXCSR/flags", () => fixture(
    [0xf3, 0x0f, 0x7e, 0x4e, 0x10], async ({ reset, state, run, bytes }) => {
      reset({ esi: DATA + 4096 - 24, flags: 0xad7 }); seedScalarSseMove(state());
      setWord(bytes(DATA), 4088, 0x7f800001); setWord(bytes(DATA), 4092, 0xfff00000);
      const before = Array.from(state());
      checkScalarSseMove(run(1), before, { 71: 0x7f800001, 72: 0xfff00000, 73: 0, 74: 0 }, CODE + 5);
    }));
  for (const encoding of [[0x67, 0xf3, 0x0f, 0x7e, 0], [0x67, 0x66, 0x0f, 0xd6, 0],
    [0x65, 0xf3, 0x0f, 0x7e, 0], [0x66, 0xf3, 0x0f, 0x7e, 0xc1], [0xf3, 0x66, 0x0f, 0x7e, 0xc1],
    [0xc5, 0xfa, 0x7e, 0xc1], [0xc5, 0xf9, 0xd6, 0xc1]]) await test(`MOVQ unimplemented encoding ${encoding.map(n => n.toString(16)).join(" ")} stops explicitly before state mutation`, () => fixture(
      encoding, async ({ reset, state, run }) => {
        reset({ eax: 0x6001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1);
        equal(result.kind, 3, "valid but unimplemented form remains unsupported, not UD");
        checkScalarSseMove(result, before, {}, CODE, 0);
      }));
  // Independent full-integer binary64 oracle. Unlike the production jammed
  // addition/106-bit product, every finite value uses exact units of 2^-2148.
  const D_SIGN = 1n << 63n, D_INF = 0x7ff0000000000000n, D_FRAC = (1n << 52n) - 1n, D_QUIET = 1n << 51n;
  const doubleUnits = raw => {
    const exponent = Number(raw >> 52n), fraction = raw & D_FRAC;
    return exponent ? ((1n << 52n) | fraction) << BigInt(exponent - 1) : fraction;
  };
  const doubleRound = (magnitude, shift, negative, rc) => {
    if (shift <= 0) return { value: magnitude << BigInt(-shift), inexact: false };
    const divisor = 1n << BigInt(shift), quotient = magnitude / divisor, remainder = magnitude % divisor;
    const increment = remainder !== 0n && (rc === 1 ? negative : rc === 2 ? !negative : rc === 3 ? false
      : remainder * 2n > divisor || (remainder * 2n === divisor && (quotient & 1n) !== 0n));
    return { value: quotient + BigInt(increment), inexact: remainder !== 0n };
  };
  const doubleReference = (a, b, mxcsr, operation) => {
    const nan = n => (n & ~D_SIGN) > D_INF, tiny = n => (n & ~D_SIGN) !== 0n && (n & D_INF) === 0n;
    if (nan(a) || nan(b)) return { bits: (nan(a) ? a : b) | D_QUIET, flags: (nan(a) && !(a & D_QUIET)) || (nan(b) && !(b & D_QUIET)) ? 1 : 0 };
    if (mxcsr & 0x40) { if (tiny(a)) a &= D_SIGN; if (tiny(b)) b &= D_SIGN; }
    if (operation === 'sub') b ^= D_SIGN;
    const am = a & ~D_SIGN, bm = b & ~D_SIGN, pre = tiny(a) || tiny(b) ? 2 : 0;
    const rc = mxcsr >>> 13 & 3, mul = operation === 'mul';
    if (mul && ((am === D_INF && bm === 0n) || (bm === D_INF && am === 0n))) return { bits: D_SIGN | D_INF | D_QUIET, flags: 1 };
    if (!mul && am === D_INF && bm === D_INF && (a & D_SIGN) !== (b & D_SIGN)) return { bits: D_SIGN | D_INF | D_QUIET, flags: 1 };
    if (am === D_INF || bm === D_INF) return { bits: mul ? ((a ^ b) & D_SIGN) | D_INF : am === D_INF ? a : b, flags: pre };
    const signed = n => doubleUnits(n & ~D_SIGN) * (n & D_SIGN ? -1n : 1n);
    const exact = mul ? signed(a) * signed(b) : (signed(a) + signed(b)) << 1074n;
    if (exact === 0n) return { bits: mul ? (a ^ b) & D_SIGN : ((a & D_SIGN) === (b & D_SIGN) ? a & D_SIGN : rc === 1 ? D_SIGN : 0n), flags: pre };
    const negative = exact < 0n, magnitude = negative ? -exact : exact, sign = negative ? D_SIGN : 0n;
    const shift = Math.max(0, magnitude.toString(2).length - 53), unbounded = doubleRound(magnitude, shift, negative, rc);
    const top = unbounded.value.toString(2).length - 1, exponent = top + shift - 2148;
    if (exponent > 1023) return { bits: sign | (rc === 0 || (negative ? rc === 1 : rc === 2) ? D_INF : D_INF - 1n), flags: pre | 8 | ((mxcsr & 0x400) || unbounded.inexact ? 32 : 0) };
    if (exponent < -1022) {
      if ((mxcsr & 0x800) && (mxcsr & 0x8000)) return { bits: sign, flags: pre | 48 };
      const bounded = doubleRound(magnitude, 1074, negative, rc);
      return { bits: sign | bounded.value, flags: pre | (mxcsr & 0x800 ? bounded.inexact ? 48 : 0 : 16 | (unbounded.inexact ? 32 : 0)) };
    }
    return { bits: sign | BigInt(exponent + 1023) << 52n | ((unbounded.value >> BigInt(top - 52)) & D_FRAC), flags: pre | (unbounded.inexact ? 32 : 0) };
  };
  const doubleWords = values => values.flatMap(n => [Number(n & 0xffffffffn), Number(n >> 32n)]);
  const doubleChanges = (destination, left, right, mxcsr, operation, scalar) => {
    const results = left.slice(0, scalar ? 1 : 2).map((n, lane) => doubleReference(n, right[lane], mxcsr, operation));
    let flags = results.reduce((flags, n) => flags | n.flags, 0);
    if (flags & 7 & ~(mxcsr >>> 7)) flags &= 7;
    const trap = Boolean(flags & ~(mxcsr >>> 7)), changes = { 99: mxcsr | flags };
    if (!trap) doubleWords(results.map(n => n.bits)).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
    return { changes, trap };
  };
  const doublePairs = [
    [0n, D_SIGN], [D_SIGN, D_SIGN], [1n, 0n], [D_FRAC, 1n], [1n << 52n, D_SIGN | D_FRAC],
    [0x3ff0000000000000n, 0x3ca0000000000000n], [0x3ff0000000000001n, 0x3ff0000000000001n],
    [0x0010000000000001n, 0x3fe0000000000000n], [0x7fe0000000000000n, 0x4000000000000000n],
    [D_INF - 1n, D_INF - 1n], [D_INF, D_SIGN | D_INF], [0n, D_INF],
    [D_INF | 123n, D_SIGN | D_INF | D_QUIET | 456n], [1n, D_INF | D_QUIET | 123n],
    [0x3ff0000000000000n, D_SIGN | 1n], [0x0010000000000001n, 0x0010000000000000n],
  ];
  for (const [operation, byte] of [['add', 0x58], ['sub', 0x5c], ['mul', 0x59]]) for (const scalar of [false, true]) {
    const name = operation.toUpperCase() + (scalar ? 'SD' : 'PD'), prefix = scalar ? 0xf2 : 0x66;
    await test(`SSE2 ${name} exact oracle RC DAZ FTZ masks old flags and upper lane`, () => fixture([prefix, 0x0f, byte, 0xc1], async ({ reset, state, run }) => {
      for (let at = 0; at < doublePairs.length; at++) for (let rc = 0; rc < 4; rc++) for (const extra of [0, 0x40, 0x8000, 0x8040]) for (const masks of [0, 1, 3, 11, 19, 31, 63]) {
        const pair = doublePairs[at], other = doublePairs[(at + 7) % doublePairs.length], mxcsr = masks << 7 | rc << 13 | extra | (at & 1 ? 63 : 0);
        const left = [pair[0], other[0]], right = [pair[1], other[1]];
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = mxcsr; putXmm(state(), 0, doubleWords(left)); putXmm(state(), 1, doubleWords(right));
        const before = Array.from(state()), { changes, trap } = doubleChanges(0, left, right, mxcsr, operation, scalar), result = run(1);
        equal(result.kind, trap ? 16 : 1, 'synchronous XM or exact completion'); checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 4), trap ? 0 : 1);
      }
    }));
    await test(`SSE2 ${name} all XMM aliases read the old sources`, async () => {
      for (let dst = 0; dst < 8; dst++) for (const src of [dst, (dst + 1) & 7]) await fixture([prefix, 0x0f, byte, 0xc0 | dst << 3 | src], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x1f80;
        const left = [0x3ff0000000000001n, D_SIGN], right = src === dst ? left : [0x3ff0000000000001n, D_INF | 1n];
        putXmm(state(), dst, doubleWords(left)); if (src !== dst) putXmm(state(), src, doubleWords(right));
        const before = Array.from(state()), { changes } = doubleChanges(dst, left, right, 0x1f80, operation, scalar);
        checkScalarSseMove(run(0), before, {}, CODE, 0); checkScalarSseMove(run(1), before, changes, CODE + 4);
      });
    });
    await test(`SSE2 ${name} whole memory fault then XM repair retains one preceding store`, () => fixture([0x83, 0x07, 1, prefix, 0x0f, byte, 0x03], async ({ reset, state, run, map, bytes, compile, saveSnapshot, restoreSnapshot }) => {
      const left = [0x3ff0000000000001n, D_INF | 1n], right = [0x3ff0000000000001n, 1n], address = scalar ? 0x5ffc : 0x6000;
      map(0x5000, READ); if (scalar) doubleWords(right.slice(0, 1)).slice(0, 1).forEach((v, n) => setWord(bytes(0x5000), 4092 + n * 4, v));
      reset({ ebx: address, edi: STACK, flags: 0x203 }); seedScalarSseMove(state()); state()[99] = 0; putXmm(state(), 0, doubleWords(left)); setWord(bytes(STACK), 0, 0x7fffffff);
      const first = run(100); equal(first.kind, 4, 'whole source preflight'); equal(first.pc, CODE + 3, 'precise SSE fault'); equal(first.retired, 1, 'only earlier store');
      const input = page(); doubleWords(right).slice(scalar ? 1 : 0, scalar ? 2 : 4).forEach((v, n) => setWord(input, n * 4, v)); map(0x6000, READ, input); await compile(CODE + 3);
      let before = Array.from(state()), expected = doubleChanges(0, left, right, before[99], operation, scalar), result = run(1);
      // SUB of identical low lanes is exact; packed upper SNaN still traps.
      equal(result.kind, expected.trap ? 16 : 1, 'numeric result after repair'); checkScalarSseMove(result, before, expected.changes, CODE + (expected.trap ? 3 : 7), expected.trap ? 0 : 1);
      if (expected.trap) { const snapshot = saveSnapshot(); reset(); restoreSnapshot(snapshot); state()[99] |= 0x1f80; before = Array.from(state()); expected = doubleChanges(0, left, right, before[99], operation, scalar); checkScalarSseMove(run(1), before, expected.changes, CODE + 7); }
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, 'completed store never replayed');
    }));
    await test(`SSE2 ${name} register XM preserves prior flags before later overwrite`, () => fixture([0x83, 0xc0, 1, prefix, 0x0f, byte, 0xc1, 0x83, 0xf8, 9], async ({ reset, state, run }) => {
      reset({ eax: 0x7fffffff, flags: 0x203 }); seedScalarSseMove(state()); state()[99] = 0; putXmm(state(), 0, doubleWords([D_INF | 1n, 0n])); putXmm(state(), 1, doubleWords([0n, 0n]));
      const before = Array.from(state()), result = run(100), flags = before[9] & ~ARITH | arithmetic('add', 32, 0x7fffffff, 1, 0).flags;
      equal(result.kind, 16, 'unmasked IE'); checkScalarSseMove(result, before, { 0: 0x80000000, 9: flags, 99: 1 }, CODE + 3, 1);
    }));
  }
  for (let dst = 0; dst < 8; dst++) await test(`SSE2 CVTSD2SI r${dst} uses RC and retains every other CPU field`, () => fixture([0xf2, 0x0f, 0x2d, 0xc0 | dst << 3 | 1], async ({ reset, state, run }) => {
    const inputs = [0n, D_SIGN, 1n, D_SIGN | 1n, 0x3ff8000000000000n, 0xbff8000000000000n, 0x4004000000000000n, 0x41dfffffffe00000n, 0xc1e0000000080000n, D_INF, D_INF | 1n];
    for (const input of inputs) for (let rc = 0; rc < 4; rc++) for (const control of [0, 0x1f80, 0x9fc0, 0x803f]) {
      const mxcsr = control | rc << 13, magnitude = input & ~D_SIGN;
      let value = 0, fresh = 0;
      if (magnitude >= D_INF) { value = 0x80000000; fresh = 1; }
      else {
        const rounded = doubleRound(mxcsr & 0x40 && magnitude < (1n << 52n) ? 0n : doubleUnits(magnitude), 1074, Boolean(input & D_SIGN), rc);
        const integer = input & D_SIGN ? -rounded.value : rounded.value;
        if (integer < -2147483648n || integer > 2147483647n) { value = 0x80000000; fresh = 1; }
        else { value = Number(BigInt.asUintN(32, integer)); fresh = rounded.inexact ? 32 : 0; }
      }
      reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = mxcsr; putXmm(state(), 1, doubleWords([input, D_INF | 1n]));
      const before = Array.from(state()), trap = Boolean(fresh & ~(mxcsr >>> 7)), result = run(1), changes = { 99: mxcsr | fresh };
      if (!trap) changes[dst] = value;
      equal(result.kind, trap ? 16 : 1, 'rounded integer or XM'); checkScalarSseMove(result, before, changes, CODE + (trap ? 0 : 4), trap ? 0 : 1);
    }
  }));
  await test('SSE2 numeric64 scratch is value-only and invalid input preserves the prior output', () => fixture([0x90], async ({ reset, state, bytes }) => {
    reset(); seedScalarSseMove(state()); const before = Array.from(state()), memory = bytes(DATA).slice();
    for (const operation of ['add', 'sub', 'mul']) {
      const fn = api['backend_sse_' + operation + '_f64']; check(typeof fn === 'function', 'fixed pure export');
      const ptr = fn(0x1f80, 0x3ff0000000000001n, 0x3ff0000000000001n), view = new DataView(api.memory.buffer, ptr, 12);
      const expected = doubleReference(0x3ff0000000000001n, 0x3ff0000000000001n, 0x1f80, operation);
      equal(view.getBigUint64(0, true), expected.bits, 'complete numeric bits'); equal(view.getUint32(8, true), expected.flags, 'fresh exceptions');
      const prior = new Uint8Array(api.memory.buffer, ptr, 12).slice(); equal(fn(0x10000, 0n, 0n), 0, 'invalid MXCSR explicit null');
      for (let n = 0; n < 12; n++) equal(new Uint8Array(api.memory.buffer, ptr, 12)[n], prior[n], 'failed numeric call leaves scratch');
    }
    for (let n = 0; n < 105; n++) equal(state()[n], before[n], 'no CPU authority');
    for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], memory[n], 'no memory authority');
  }));
  await test('SSE2 mixed binary64 binary32 x87 comparison and timestamp sparse import types coexist', () => fixture(
    [0xd9,0xee,0xdd,0x1f,0xf3,0xae,0x0f,0x59,0xc1,0x66,0x0f,0x58,0xc1,0xf2,0x0f,0x2d,0xc0,0x0f,0x31], async ({ handle, reset, state, run, bytes }) => {
      reset({ ecx: 1, edi: STACK }); putXmm(state(), 0, [0,0,0,0]); putXmm(state(), 1, [0,0,0,0]);
      const result = run(100); equal(result.kind, 15, 'all mixed imports reach Runtime timestamp'); equal(result.pc, CODE + 17, 'exact RDTSC boundary'); equal(result.retired, 6, 'only completed instructions');
      equal(state()[0], 0, 'rounded integer'); equal(state()[1], 0, 'SCAS count'); equal(state()[7], STACK + 1, 'SCAS destination'); equal(state()[9], 0x246, 'numeric operations retain comparison flags');
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getBigUint64(0, true), 0n, 'x87 zero store');
      ok(api.backend_complete_timestamp_counter(handle, CODE + 17, 0x12345678, 9), 'complete timestamp'); equal(state()[8], CODE + 19, 'resume after timestamp');
    }));
  const rawSse2Operations = [
    ['ANDPD', 0x54, (a, b) => a.map((x, n) => (x & b[n]) >>> 0)],
    ['ORPD', 0x56, (a, b) => a.map((x, n) => (x | b[n]) >>> 0)],
    ['XORPD', 0x57, (a, b) => a.map((x, n) => (x ^ b[n]) >>> 0)],
    ['MOVAPD', 0x28, (_a, b) => b],
    ['PSUBD', 0xfa, (a, b) => a.map((x, n) => (x - b[n]) >>> 0)],
    ['PSUBQ', 0xfb, (a, b) => [0, 2].flatMap(n => { const x = BigInt.asUintN(64, (BigInt(a[n]) | BigInt(a[n + 1]) << 32n) - (BigInt(b[n]) | BigInt(b[n + 1]) << 32n)); return [Number(x & 0xffffffffn), Number(x >> 32n)]; })],
    ['PCMPEQD', 0x76, (a, b) => a.map((x, n) => x === b[n] ? 0xffffffff : 0)],
    ['PMAXSW', 0xee, (a, b) => a.map((x, n) => ((Math.max(x << 16 >> 16, b[n] << 16 >> 16) & 0xffff) | Math.max(x >> 16, b[n] >> 16) << 16) >>> 0)],
    ['UNPCKLPD', 0x14, (a, b) => [a[0], a[1], b[0], b[1]]],
  ];
  for (const [name, opcode, reference] of rawSse2Operations) for (const form of ['register', 'self', 'alias']) await test(`SSE2 ${name} raw128 ${form} preserves all FP and scalar state`, () => fixture(
    [0x66, 0x0f, opcode, form === 'alias' ? 0x03 : form === 'self' ? 0xc0 : 0xc1], async ({ alias, bytes, reset, state, run }) => {
      alias(0x5000, DATA, READ); reset({ ebx: 0x5010, flags: 0xad7 }); seedScalarSseMove(state());
      const left = [0xffffffff, 0x80000000, 0x7f800001, 0x80017fff], right = [1, 0xffffffff, 0x7f800001, 0x7fff8000];
      putXmm(state(), 0, left); putXmm(state(), 1, right); for (let n = 0; n < 4; n++) setWord(bytes(DATA), 16 + n * 4, right[n]);
      const before = Array.from(state()), expected = reference(left, form === 'self' ? left : right), changes = Object.fromEntries(expected.map((x, n) => [67 + n, x]));
      checkScalarSseMove(run(0), before, {}, CODE, 0); checkScalarSseMove(run(1), before, changes, CODE + 4);
    }));
  for (const [name, opcode] of rawSse2Operations) for (const misaligned of [false, true]) await test(`SSE2 ${name} memory ${misaligned ? 'alignment' : 'missing page'} fault precedes XMM change`, () => fixture(
    [0x66, 0x0f, opcode, 0x03], async ({ reset, state, run }) => { reset({ ebx: 0x6000 + Number(misaligned), flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1); equal(result.kind, misaligned ? 6 : 4, "legacy full-width operand fault"); checkScalarSseMove(result, before, {}, CODE, 0); }));
  for (const left of [false, true]) for (const memory of [false, true]) await test(`SSE2 PS${left ? 'LL' : 'RL'}Q ${memory ? 'memory' : 'register'} count uses all unsigned64 bits`, () => fixture(
    [0x66, 0x0f, left ? 0xf3 : 0xd3, memory ? 0x03 : 0xc1], async ({ bytes, reset, state, run }) => {
      const values = [0xfedcba9876543210n, 0x8000000000000001n], mask = (1n << 64n) - 1n;
      for (const count of [0n, 1n, 31n, 32n, 63n, 64n, 255n, 0x100000000n, 0xffffffffffffffffn]) {
        reset({ ebx: DATA + 16, flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), 0, values.flatMap(x => [Number(x & 0xffffffffn), Number(x >> 32n)]));
        const source = [Number(count & 0xffffffffn), Number(count >> 32n), 0xdeadbeef, 0xffffffff]; putXmm(state(), 1, source); source.forEach((x, n) => setWord(bytes(DATA), 16 + n * 4, x));
        const before = Array.from(state()), expected = values.flatMap(x => { const y = count >= 64n ? 0n : left ? x << count & mask : x >> count; return [Number(y & 0xffffffffn), Number(y >> 32n)]; });
        checkScalarSseMove(run(1), before, Object.fromEntries(expected.map((x, n) => [67 + n, x])), CODE + 4);
      }
    }));
  for (const left of [false, true]) await test(`SSE2 PS${left ? 'LL' : 'RL'}Q immediate every encoded count is not modulo64`, async () => {
    for (let count = 0; count < 256; count++) await fixture([0x66, 0x0f, 0x73, left ? 0xf0 : 0xd0, count], async ({ reset, state, run }) => {
      reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), 0, [0xffffffff, 0x80000000, 1, 0x7fffffff]); const before = Array.from(state());
      const expected = [0x80000000ffffffffn, 0x7fffffff00000001n].flatMap(x => { const y = count >= 64 ? 0n : BigInt.asUintN(64, left ? x << BigInt(count) : x >> BigInt(count)); return [Number(y & 0xffffffffn), Number(y >> 32n)]; });
      checkScalarSseMove(run(1), before, Object.fromEntries(expected.map((x, n) => [67 + n, x])), CODE + 5);
    });
  });
  for (const selector of [0, 1, 7, 8, 255]) await test(`SSE2 PEXTRW selector${selector} zero extends and preserves the XMM source`, async () => {
    for (let destination = 0; destination < 8; destination++) await fixture([0x66, 0x0f, 0xc5, 0xc0 | destination << 3 | 1, selector], async ({ reset, state, run }) => {
      reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), 1, [0xffff8000, 0x1234abcd, 0x80017fff, 0xfedc9876]); const before = Array.from(state()), lane = selector & 7;
      checkScalarSseMove(run(1), before, { [destination]: before[71 + (lane >> 1)] >>> ((lane & 1) * 16) & 0xffff }, CODE + 5);
    });
  });
  await test("SSE2 PMOVMSKB collects all sixteen sign bits without MMX alias effects", async () => {
    for (let destination = 0; destination < 8; destination++) await fixture([0x66, 0x0f, 0xd7, 0xc0 | destination << 3 | 1], async ({ reset, state, run }) => {
      reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), 1, [0x7f80ff00, 0x80ff007f, 0x01234567, 0x89abcdef]); const before = Array.from(state()), bytes = new Uint8Array(new Uint32Array(before.slice(71, 75)).buffer);
      const mask = bytes.reduce((value, byte, n) => value | (byte >>> 7) << n, 0); checkScalarSseMove(run(1), before, { [destination]: mask }, CODE + 4);
    });
  });
  for (const memory of [false, true]) await test(`SSE2 CVTDQ2PD exact signed pair ${memory ? 'split memory' : 'aliased register'} ignores RC DAZ FTZ`, () => fixture(
    [0xf3, 0x0f, 0xe6, memory ? 0x03 : 0xc0], async ({ map, bytes, reset, state, run }) => {
      map(DATA + 4096, READ | WRITE);
      for (const values of [[0x80000000, 0x7fffffff], [0, 0xffffffff], [0x7f800001, 0x80000001]]) for (const rc of [0, 1, 2, 3]) {
        reset({ ebx: DATA + 4093, flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = rc << 13 | 0x8041;
        putXmm(state(), 0, [...values, 0x7ff00001, 0xdeadbeef]); const input = new Uint8Array(new Uint32Array(values).buffer); for (let n = 0; n < 8; n++) bytes(DATA + ((4093 + n) & ~4095))[(4093 + n) & 4095] = input[n];
        const out = new DataView(new ArrayBuffer(16)); values.forEach((x, n) => out.setFloat64(n * 8, x | 0, true)); const before = Array.from(state());
        checkScalarSseMove(run(1), before, Object.fromEntries(Array.from({ length: 4 }, (_, n) => [67 + n, out.getUint32(n * 4, true)])), CODE + 4);
      }
    }));
  await test("SSE2 CVTDQ2PD complete m64 fault suppresses both result lanes", () => fixture([0xf3, 0x0f, 0xe6, 0x03], async ({ reset, state, run }) => {
    reset({ ebx: DATA + 4093, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1); equal(result.kind, 4, "missing second page"); equal(result.address, DATA + 4096, "exact failed byte"); checkScalarSseMove(result, before, {}, CODE, 0);
  }));
  const scalarSseMoves = [
    { name: "MOVD", prefix: 0x66, load: 0x6e, store: 0x7e, length: 4 },
    { name: "MOVSD scalar", prefix: 0xf2, load: 0x10, store: 0x11, length: 8 },
    { name: "MOVSS", prefix: 0xf3, load: 0x10, store: 0x11, length: 4 },
    { name: "MOVQ XMM", prefix: 0xf3, storePrefix: 0x66, load: 0x7e, store: 0xd6, length: 8 },
    { name: "MOVLPD", prefix: 0x66, load: 0x12, store: 0x13, length: 8, preserveUpper: true },
  ];
  for (const operation of scalarSseMoves) for (const write of [false, true]) {
    const prefix = write ? operation.storePrefix ?? operation.prefix : operation.prefix;
    await test(`${operation.name} ${write ? "store" : "load"} permits unaligned and cross-page alias access without FP exceptions`, () => fixture(
      [prefix, 0x0f, write ? operation.store : operation.load, 0x1b], async ({ reset, state, run, bytes, map, alias }) => {
        map(DATA + 4096, READ | WRITE); alias(0x5000, DATA, READ | WRITE); alias(0x6000, DATA + 4096, READ | WRITE);
        for (const offset of [1, 4096 - operation.length, 4093]) {
          bytes(DATA).fill(0x55); bytes(DATA + 4096).fill(0x66);
          reset({ ebx: 0x5000 + offset, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state());
          const source = write ? new Uint8Array(new Uint32Array(before.slice(79, 83)).buffer).slice(0, operation.length)
            : Uint8Array.from({ length: operation.length }, (_, n) => offset + n < 4096 ? 0x55 : 0x66);
          const changes = {};
          if (!write) for (let n = 0; n < 4; n++) changes[79 + n] = n * 4 < operation.length ? new DataView(source.buffer).getUint32(n * 4, true) : operation.preserveUpper ? before[79 + n] : 0;
          checkScalarSseMove(run(1), before, changes, CODE + 4);
          for (let n = 0; n < 8192; n++) {
            const expected = write && n >= offset && n < offset + operation.length ? source[n - offset] : n < 4096 ? 0x55 : 0x66;
            equal(bytes(DATA + (n & ~4095))[n & 4095], expected, "only exact transfer-width bytes change through aliases");
          }
        }
      }));
    for (const present of [false, true]) await test(`${operation.name} ${write ? "store" : "load"} full-span ${present ? "protection" : "absent"} fault preserves prior effects and resumes once`, () => fixture(
      [0x83, 0x07, 1, prefix, 0x0f, write ? operation.store : operation.load, 0x1b], async ({ reset, state, run, bytes, map, protect, compile }) => {
        bytes(DATA).fill(0x55); if (present) map(DATA + 4096, write ? READ : WRITE, page([], 0x66));
        setWord(bytes(STACK), 0, 0x7fffffff); reset({ ebx: DATA + 4093, edi: STACK, flags: 0x203 }); seedScalarSseMove(state());
        const before = Array.from(state()), flags = (before[9] & ~ARITH) | arithmetic("add", 32, 0x7fffffff, 1, 0).flags;
        const fault = run(100); equal(fault.kind, 4, "scalar SSE raises page fault despite pending x87 state");
        equal(fault.address, DATA + 4096, "first failing page address"); equal(fault.error, (write ? 6 : 4) | Number(present), "exact access and protection fault bits");
        checkScalarSseMove(fault, before, { 9: flags }, CODE + 3);
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], 0x55, "failed cross-page store cannot partially mutate first page");
        if (present) protect(DATA + 4096, READ | WRITE); else map(DATA + 4096, READ | WRITE, page([], 0x66));
        await compile(CODE + 3); const repairedBefore = Array.from(state()), changes = {};
        if (!write) for (let n = 0; n < 4; n++) changes[79 + n] = n * 4 < operation.length ? n === 0 ? 0x66555555 : 0x66666666 : operation.preserveUpper ? before[79 + n] : 0;
        checkScalarSseMove(run(1), repairedBefore, changes, CODE + 7);
        equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "prior ADD store never replays on repair");
        if (write) {
          const source = new Uint8Array(new Uint32Array(before.slice(79, 83)).buffer);
          for (let n = 0; n < 8192; n++) equal(bytes(DATA + (n & ~4095))[n & 4095], n >= 4093 && n < 4093 + operation.length ? source[n - 4093] : n < 4096 ? 0x55 : 0x66, "repaired transfer changes exactly its own bytes");
        }
      }));
    await test(`${operation.name} ${write ? "store" : "load"} FS-base and wrapped effective addresses retain checked spans`, () => fixture(
      [0x64, prefix, 0x0f, write ? operation.store : operation.load, 0x1c, 0x02], async ({ reset, state, run, bytes }) => {
        bytes(DATA).fill(0x5a); reset({ eax: 0xfffffff0, edx: 0x2010, fs: 1, flags: 0xad7 }); seedScalarSseMove(state());
        const before = Array.from(state()), changes = {};
        if (!write) for (let n = 0; n < 4; n++) changes[79 + n] = n * 4 < operation.length ? 0x5a5a5a5a : operation.preserveUpper ? before[79 + n] : 0;
        checkScalarSseMove(run(1), before, changes, CODE + 6);
        const source = new Uint8Array(new Uint32Array(before.slice(79, 83)).buffer);
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], write && n >= 1 && n < 1 + operation.length ? source[n - 1] : 0x5a, "wrapped EA plus FS base has exact byte range");
        reset({ eax: 0xffffffff, edx: 0, fs: DATA + 1, flags: 0xad7 }); seedScalarSseMove(state()); const invalidBefore = Array.from(state()), invalid = run(1);
        equal(invalid.kind, 6, "FS offset span is checked before base addition can hide wrap"); checkScalarSseMove(invalid, invalidBefore, {}, CODE, 0);
      }));
    if (write) await test(`${operation.name} executable alias store retires once and invalidates before following code`, () => fixture(
      [prefix, 0x0f, operation.store, 0x1b, 0x40], async ({ handle, alias, reset, state, run, bytes, compile }) => {
        alias(0x4000, CODE, READ | WRITE); reset({ eax: 9, ebx: 0x4101, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state());
        const result = run(100); equal(result.kind, 7, "code alias store ends current execution"); checkScalarSseMove(result, before, {}, CODE + 4);
        const source = new Uint8Array(new Uint32Array(before.slice(79, 83)).buffer);
        for (let n = 0; n < operation.length; n++) equal(bytes(CODE)[0x101 + n], source[n], "all raw code-alias bytes committed");
        check(api.backend_validate(handle, CODE) !== 0, "stale compiled dependency rejected"); await compile(CODE + 4); putXmm(state(), 3, [0, 0, 0, 0]);
        const resumed = run(1); equal(resumed.words[0], 10, "next instruction resumes once");
        for (let n = 0; n < operation.length; n++) equal(bytes(CODE)[0x101 + n], source[n], "SMC continuation does not replay transfer");
      }, READ | WRITE | EXECUTE | SEALED));
    await test(`LOCK ${operation.name} ${write ? "store" : "load"} is invalid before memory or FP access`, () => fixture(
      [0xf0, prefix, 0x0f, write ? operation.store : operation.load, 0x1b], async ({ reset, state, run }) => {
        reset({ ebx: 0x6001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1);
        equal(result.kind, 8, "LOCK raw SSE transfer is invalid"); checkScalarSseMove(result, before, {}, CODE, 0);
      }));
  }
  await test("MOVSS absolute load and scalar register transfers preserve the exact lanes at every budget boundary", () => fixture(
    [0xf3, 0x0f, 0x10, 0x15, ...wordBytes(DATA), 0xf2, 0x0f, 0x10, 0xda,
      0xf3, 0x0f, 0x10, 0xd9, 0x66, 0x0f, 0x7e, 0xda, 0xf2, 0x0f, 0x11, 0x1d, ...wordBytes(STACK)],
    async ({ reset, state, run, bytes }) => {
      setWord(bytes(DATA), 0, 0x7f800001);
      for (let budget = 0; budget <= 5; budget++) {
        bytes(STACK).fill(0x55); reset({ flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), changes = {};
        if (budget >= 1) Object.assign(changes, { 75: 0x7f800001, 76: 0, 77: 0, 78: 0 });
        if (budget >= 2) Object.assign(changes, { 79: 0x7f800001, 80: 0 });
        if (budget >= 3) changes[79] = before[71];
        if (budget >= 4) changes[2] = before[71];
        checkScalarSseMove(run(budget), before, changes, CODE + [0, 8, 12, 16, 20, 28][budget], budget);
        const output = new Uint8Array(new Uint32Array([before[71], 0]).buffer);
        for (let n = 0; n < 4096; n++) equal(bytes(STACK)[n], budget === 5 && n < 8 ? output[n] : 0x55, "only final scalar store publishes eight bytes");
      }
    }));
  const unpackLowWords = (a, b) => Array.from({ length: 4 }, (_, n) => {
    const shift = (n & 1) * 16;
    return ((a[n >>> 1] >>> shift & 0xffff) | (b[n >>> 1] >>> shift & 0xffff) << 16) >>> 0;
  });
  await test("SSE2 PUNPCKLWD interleaves low words for every destination and overlapping source", async () => {
    for (let destination = 0; destination < 8; destination++) for (const source of [destination, (destination + 1) & 7]) {
      await fixture([0x66, 0x0f, 0x61, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state());
        for (let r = 0; r < 8; r++) putXmm(state(), r, [(0x8000ffff ^ r * 0x01010101) >>> 0, (0x7fff0001 ^ r * 0x01000100) >>> 0, 0xdeadbeef, 0x01234567]);
        const before = Array.from(state()), words = unpackLowWords(xmmWords(before, destination), xmmWords(before, source)), changes = {};
        words.forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
        checkScalarSseMove(run(0), before, {}, CODE, 0);
        checkScalarSseMove(run(1), before, changes, CODE + 4);
      });
    }
  });
  await test("SSE2 PUNPCKLWD memory observes aligned operand protection and reads the final sixteen bytes of a page", () => fixture(
    [0x66, 0x0f, 0x61, 0x13], async ({ reset, state, run, bytes, map, protect }) => {
      map(DATA + 4096, WRITE); const memory = [0x01234567, 0x89abcdef, 0xdeadbeef, 0x5555aaaa];
      for (const [address, rights, kind, errorCode] of [[DATA, READ, 1, 0], [DATA + 4080, READ, 1, 0],
        [DATA + 4088, READ, 6, 0], [0x5001, READ, 6, 0], [DATA + 1, WRITE, 6, 0],
        [DATA, WRITE, 4, 5], [0x5000, READ, 4, 4]]) {
        protect(DATA, READ | WRITE); bytes(DATA).fill(0x55);
        for (const offset of [0, 4080]) memory.forEach((word, n) => setWord(bytes(DATA), offset + n * 4, word));
        reset({ ebx: address, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), image = bytes(DATA).slice(); protect(DATA, rights);
        const result = run(1), changes = {}; equal(result.kind, kind, "linear alignment precedes page lookup");
        if (kind === 1) unpackLowWords(xmmWords(before, 2), memory).forEach((word, n) => { changes[75 + n] = word; });
        else { equal(result.error, errorCode, "exact page or GP error code"); if (kind === 4) equal(result.address, address, "aligned fault address"); }
        checkScalarSseMove(result, before, changes, kind === 1 ? CODE + 4 : CODE, kind === 1 ? 1 : 0);
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "unpack never mutates source bytes");
      }
    }));
  await test("SSE2 PUNPCKLWD memory fault repairs without replaying prior scalar flags or stores", () => fixture(
    [0x83, 0x07, 1, 0x66, 0x0f, 0x61, 0x13], async ({ reset, state, run, bytes, map, compile }) => {
      setWord(bytes(STACK), 0, 0x7fffffff); reset({ ebx: 0x5000, edi: STACK, flags: 0x203 }); seedScalarSseMove(state()); const before = Array.from(state());
      const fault = run(100); equal(fault.kind, 4, "unpack source page fault"); equal(fault.address, 0x5000, "exact source page address"); equal(fault.error, 4, "missing user read fault");
      checkScalarSseMove(fault, before, { 9: (before[9] & ~ARITH) | arithmetic("add", 32, 0x7fffffff, 1, 0).flags }, CODE + 3);
      const memory = [0x01234567, 0x89abcdef, 0xdeadbeef, 0x5555aaaa], input = page(); memory.forEach((word, n) => setWord(input, n * 4, word)); map(0x5000, READ, input);
      await compile(CODE + 3); const repairedBefore = Array.from(state()), changes = {};
      unpackLowWords(xmmWords(repairedBefore, 2), memory).forEach((word, n) => { changes[75 + n] = word; });
      checkScalarSseMove(run(1), repairedBefore, changes, CODE + 7);
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "preceding memory ADD committed once");
    }));
  await test("SSE2 PUNPCKLWD FS checks the full operand offset span before linear alignment", () => fixture(
    [0x64, 0x66, 0x0f, 0x61, 0x13], async ({ reset, state, run, bytes }) => {
      const memory = [0x01234567, 0x89abcdef, 0xdeadbeef, 0x5555aaaa]; memory.forEach((word, n) => setWord(bytes(DATA), 16 + n * 4, word));
      for (const [offset, base, kind] of [[DATA + 8, 8, 1], [DATA + 7, 8, 6], [0xfffffff8, DATA + 24, 6]]) {
        reset({ ebx: offset, fs: base, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), changes = {}, result = run(1);
        equal(result.kind, kind, "full128 segment proof precedes aligned source access");
        if (kind === 1) unpackLowWords(xmmWords(before, 2), memory).forEach((word, n) => { changes[75 + n] = word; });
        else equal(result.error, 0, "segment/alignment GP has zero error code");
        checkScalarSseMove(result, before, changes, kind === 1 ? CODE + 5 : CODE, kind === 1 ? 1 : 0);
      }
    }));
  await test("MOVD inputs feed PUNPCKLWD and scalar output with no stale vector state", () => fixture(
    [0x66, 0x0f, 0x6e, 0xc8, 0x66, 0x0f, 0x6e, 0xd2, 0x66, 0x0f, 0x61, 0xca, 0xf2, 0x0f, 0x11, 0x0b],
    async ({ reset, state, run, bytes }) => {
      for (let budget = 1; budget <= 4; budget++) {
        bytes(DATA).fill(0x55); reset({ eax: 0x8000ffff, edx: 0x7fff0001, ebx: DATA, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), changes = {};
        Object.assign(changes, { 71: before[0], 72: 0, 73: 0, 74: 0 });
        if (budget >= 2) Object.assign(changes, { 75: before[2], 76: 0, 77: 0, 78: 0 });
        if (budget >= 3) Object.assign(changes, { 71: 0x0001ffff, 72: 0x7fff8000 });
        checkScalarSseMove(run(budget), before, changes, CODE + budget * 4, budget);
        const output = new Uint8Array(new Uint32Array([0x0001ffff, 0x7fff8000]).buffer);
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], budget === 4 && n < 8 ? output[n] : 0x55, "unpack publishes through exact low64 store only");
      }
    }));
  await test("LOCK SSE2 PUNPCKLWD rejects before vector or pending FP effects", () => fixture(
    [0xf0, 0x66, 0x0f, 0x61, 0xc1], async ({ reset, state, run }) => {
      reset({ flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1);
      equal(result.kind, 8, "LOCK unpack is invalid"); checkScalarSseMove(result, before, {}, CODE, 0);
    }));
  const shufpsReference = (left, right, selectors) => [left[selectors % 4], left[Math.floor(selectors / 4) % 4],
    right[Math.floor(selectors / 16) % 4], right[Math.floor(selectors / 64) % 4]];
  const pshufdReference = (source, selectors) => [0, 1, 2, 3].map(lane => source[Math.floor(selectors / 4 ** lane) % 4]);
  await test("PSHUFD all256 selectors and every XMM pair copy only old source raw bits", async () => {
    for (let selectors = 0; selectors < 256; selectors++) {
      const destination = selectors & 7, source = selectors >> 3 & 7;
      await fixture([0x66, 0x0f, 0x70, 0xc0 | destination << 3 | source, selectors], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x3f;
        for (let register = 0; register < 8; register++) putXmm(state(), register, [0x7f800001 + register, 0xff800020 + register, 0x80000000 + register, 0x12345600 + register]);
        const before = Array.from(state()), changes = {}; pshufdReference(xmmWords(before, source), selectors).forEach((word, lane) => { changes[67 + destination * 4 + lane] = word; });
        checkScalarSseMove(run(0), before, {}, CODE, 0); const result = run(1); equal(result.kind, 1, "raw shuffle ignores pending x87 and sticky unmasked SIMD"); checkScalarSseMove(result, before, changes, CODE + 5);
      });
    }
  });
  for (const selectors of [0, 0x1b, 0xe4, 0xff]) await test("PSHUFD memory selector " + selectors + " checks all aligned128 bits before destination mutation", () => fixture(
    [0x64, 0x66, 0x0f, 0x70, 0x13, selectors], async ({ reset, state, run, bytes, map, protect }) => {
      map(DATA + 4096, WRITE); const memory = [0x7f800001, 0xff800123, 0x80000000, 0x01234567];
      for (const [offset, base, rights, kind, error] of [[DATA, 0, READ, 1, 0], [DATA + 4080, 0, READ, 1, 0], [DATA + 4088, 0, READ, 6, 0],
        [DATA + 1, 0, WRITE, 6, 0], [0x5001, 0, READ, 6, 0], [DATA, 0, WRITE, 4, 5], [0x5000, 0, READ, 4, 4], [DATA + 8, 8, READ, 1, 0], [0xfffffff8, DATA + 24, READ, 6, 0]]) {
        protect(DATA, READ | WRITE); bytes(DATA).fill(0x55); for (const at of [0, 16, 4080]) memory.forEach((word, lane) => setWord(bytes(DATA), at + lane * 4, word));
        reset({ ebx: offset, fs: base, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), image = bytes(DATA).slice(), changes = {}; protect(DATA, rights);
        const result = run(1); equal(result.kind, kind, "full source alignment/protection precede even broadcast of lane0"); equal(result.error, error, "memory exception error");
        if (kind === 1) pshufdReference(memory, selectors).forEach((word, lane) => { changes[75 + lane] = word; }); else if (kind === 4) equal(result.address, offset, "precise missing source");
        checkScalarSseMove(result, before, changes, CODE + (kind === 1 ? 6 : 0), kind === 1 ? 1 : 0);
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "raw source is read only");
      }
    }));
  await test("PSHUFD reads the final aligned128 source through an alias without reading beyond it", () => fixture(
    [0x66, 0x0f, 0x70, 0x42, 0x20, 0x1b], async ({ reset, state, run, bytes, alias }) => {
      alias(0x4000, DATA, READ); const source = [0x7f800001, 0xff800123, 0x80000000, 1]; source.forEach((word, lane) => setWord(bytes(DATA), 4080 + lane * 4, word));
      reset({ edx: 0x4fd0, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), changes = {}; pshufdReference(source, 0x1b).forEach((word, lane) => { changes[67 + lane] = word; });
      checkScalarSseMove(run(1), before, changes, CODE + 6);
    }));
  await test("PSHUFD and prior ADD retain completed state across protected-source repair", () => fixture(
    [0x66, 0x0f, 0x70, 0xc1, 0x1b, 0x83, 0x07, 1, 0x66, 0x0f, 0x70, 0x13, 0xe4], async ({ reset, state, run, bytes, map, protect, compile }) => {
      const source = [0x7f800001, 0xff800123, 0x80000000, 0x76543210], input = page(); source.forEach((word, lane) => setWord(input, lane * 4, word)); map(0x5000, WRITE, input);
      setWord(bytes(STACK), 0, 0x7fffffff); reset({ ebx: 0x5000, edi: STACK, flags: 0x203 }); seedScalarSseMove(state());
      const before = Array.from(state()), changes = { 9: before[9] & ~ARITH | arithmetic("add", 32, 0x7fffffff, 1, 0).flags }; pshufdReference(xmmWords(before, 1), 0x1b).forEach((word, lane) => { changes[67 + lane] = word; });
      const fault = run(100); equal(fault.kind, 4, "read permission fault follows completed shuffle and ADD"); equal(fault.error, 5, "read protection"); equal(fault.address, 0x5000, "source address"); checkScalarSseMove(fault, before, changes, CODE + 8, 2);
      protect(0x5000, READ); putXmm(state(), 1, [0, 0, 0, 0]); await compile(CODE + 8); const prior = Array.from(state()), repaired = {}; pshufdReference(source, 0xe4).forEach((word, lane) => { repaired[75 + lane] = word; });
      checkScalarSseMove(run(1), prior, repaired, CODE + 13); equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "prior ADD executes only once");
    }));
  await test("PSHUFD output survives executable-alias SMC and precise rebuild", () => fixture(
    [0x66, 0x0f, 0x70, 0xc1, 0x1b, 0x0f, 0x11, 0x03, 0x40], async ({ handle, reset, state, run, bytes, alias, compile }) => {
      alias(0x4000, CODE, READ | WRITE); reset({ ebx: 0x4101, eax: 9, flags: 0xad7 }); seedScalarSseMove(state());
      const before = Array.from(state()), changes = {}, shuffled = pshufdReference(xmmWords(before, 1), 0x1b); shuffled.forEach((word, lane) => { changes[67 + lane] = word; });
      const result = run(100); equal(result.kind, 7, "completed store causes SMC before INC"); checkScalarSseMove(result, before, changes, CODE + 8, 2);
      const output = new Uint8Array(new Uint32Array(shuffled).buffer); for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "raw shuffled bits stored");
      check(api.backend_validate(handle, CODE) !== 0, "old module invalidated"); await compile(CODE + 8); putXmm(state(), 1, [0, 0, 0, 0]); const prior = Array.from(state());
      checkScalarSseMove(run(1), prior, { 0: 10, 9: arithmetic("add", 32, 9, 1, 0).flags | prior[9] & ~ARITH | prior[9] & CF }, CODE + 9);
      for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "rebuild never repeats shuffle or store");
    }, READ | WRITE | EXECUTE | SEALED));
  for (const [code, kind] of [[[0x0f, 0x70, 0xc1, 0], 3], [[0xf2, 0x0f, 0x70, 0xc1, 0], 3], [[0xf3, 0x0f, 0x70, 0xc1, 0], 3],
    [[0x67, 0x66, 0x0f, 0x70, 0x03, 0], 3], [[0x65, 0x66, 0x0f, 0x70, 0x03, 0], 3], [[0xc5, 0xf9, 0x70, 0xc1, 0], 3], [[0xf0, 0x66, 0x0f, 0x70, 0x03, 0], 8]]) await test("PSHUFD distinct encoding " + code.join("-") + " stops before effects", () => fixture(
      code, async ({ reset, state, run }) => { reset({ ebx: 0x5001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1); equal(result.kind, kind, "valid unsupported versus invalid LOCK"); checkScalarSseMove(result, before, {}, CODE, 0); }));
  await test("SHUFPS exhausts all256 selectors with distinct and aliased sources without interpreting FP bits", async () => {
    for (let selectors = 0; selectors <= 255; selectors++) for (const same of [false, true]) await fixture(
      [0x0f, 0xc6, same ? 0xc0 : 0xc1, selectors], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[99] = 0x3f;
        putXmm(state(), 0, [0x7f800001, 0xffc01234, 0x80000000, 0x01234567]); putXmm(state(), 1, [0xff800123, 0x7fc04567, 1, 0x89abcdef]);
        const before = Array.from(state()), changes = {};
        shufpsReference(xmmWords(before, 0), xmmWords(before, same ? 0 : 1), selectors).forEach((word, lane) => { changes[67 + lane] = word; });
        checkScalarSseMove(run(0), before, {}, CODE, 0); const result = run(1); equal(result.kind, 1, "raw shuffle ignores pending x87 and unmasked SIMD sticky"); checkScalarSseMove(result, before, changes, CODE + 4);
      });
  });
  await test("SHUFPS every XMM pair preserves source order, repeated lanes and all unselected state", async () => {
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) for (const selectors of [0, 0x1b, 0x44, 0x4e, 0xe4, 0xff]) await fixture(
      [0x0f, 0xc6, 0xc0 | destination << 3 | source, selectors], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state());
        for (let register = 0; register < 8; register++) putXmm(state(), register, [0x7f800001 + register, 0xff800020 + register, 0x80000000 + register, 0x12345600 + register]);
        const before = Array.from(state()), changes = {};
        shufpsReference(xmmWords(before, destination), xmmWords(before, source), selectors).forEach((word, lane) => { changes[67 + destination * 4 + lane] = word; });
        checkScalarSseMove(run(1), before, changes, CODE + 4);
      });
  });
  for (const selectors of [0, 0x44, 0xff]) await test("SHUFPS memory selector " + selectors + " validates the full aligned128 operand before any lane mutation", () => fixture(
    [0x64, 0x0f, 0xc6, 0x13, selectors], async ({ reset, state, run, bytes, map, protect }) => {
      map(DATA + 4096, WRITE); const memory = [0x7f800001, 0xff800123, 0x80000000, 0x01234567];
      for (const [offset, base, rights, kind, error] of [[DATA, 0, READ, 1, 0], [DATA + 4080, 0, READ, 1, 0], [DATA + 4088, 0, READ, 6, 0],
        [DATA + 1, 0, WRITE, 6, 0], [0x5001, 0, READ, 6, 0], [DATA, 0, WRITE, 4, 5], [0x5000, 0, READ, 4, 4], [DATA + 8, 8, READ, 1, 0], [0xfffffff8, DATA + 24, READ, 6, 0]]) {
        protect(DATA, READ | WRITE); bytes(DATA).fill(0x55); for (const at of [0, 16, 4080]) memory.forEach((word, lane) => setWord(bytes(DATA), at + lane * 4, word));
        reset({ ebx: offset, fs: base, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), image = bytes(DATA).slice(), changes = {}; protect(DATA, rights);
        const result = run(1); equal(result.kind, kind, "raw memory shuffle has precise GP/PF contract"); equal(result.error, error, "memory exception error code");
        if (kind === 1) shufpsReference(xmmWords(before, 2), memory, selectors).forEach((word, lane) => { changes[75 + lane] = word; });
        else if (kind === 4) equal(result.address, offset, "first unavailable aligned source page");
        checkScalarSseMove(result, before, changes, CODE + (kind === 1 ? 5 : 0), kind === 1 ? 1 : 0);
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "shuffle read never writes guest bytes");
      }
    }));
  await test("SHUFPS generic displacement reads exactly the final aligned16 bytes through an alias mapping", () => fixture(
    [0x0f, 0xc6, 0x42, 0x20, 0x44], async ({ reset, state, run, bytes, alias }) => {
      alias(0x4000, DATA, READ); const memory = [0x7f800001, 0xff800123, 0x80000000, 1]; memory.forEach((word, lane) => setWord(bytes(DATA), 4080 + lane * 4, word));
      reset({ edx: 0x4fd0, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), changes = {};
      shufpsReference(xmmWords(before, 0), memory, 0x44).forEach((word, lane) => { changes[67 + lane] = word; });
      checkScalarSseMove(run(1), before, changes, CODE + 5);
    }));
  await test("SHUFPS prior shuffle and ADD survive protected-source repair without replay", () => fixture(
    [0x0f, 0xc6, 0xc1, 0x1b, 0x83, 0x07, 1, 0x0f, 0xc6, 0x13, 0xe4], async ({ reset, state, run, bytes, map, protect, compile }) => {
      const memory = [0x7f800001, 0xff800123, 0x80000000, 0x76543210], input = page(); memory.forEach((word, lane) => setWord(input, lane * 4, word)); map(0x5000, WRITE, input);
      setWord(bytes(STACK), 0, 0x7fffffff); reset({ ebx: 0x5000, edi: STACK, flags: 0x203 }); seedScalarSseMove(state());
      const before = Array.from(state()), changes = { 9: before[9] & ~ARITH | arithmetic("add", 32, 0x7fffffff, 1, 0).flags };
      shufpsReference(xmmWords(before, 0), xmmWords(before, 1), 0x1b).forEach((word, lane) => { changes[67 + lane] = word; });
      const fault = run(100); equal(fault.kind, 4, "unreadable source faults after completed vector and scalar effects"); equal(fault.error, 5, "read protection fault"); equal(fault.address, 0x5000, "source fault address"); checkScalarSseMove(fault, before, changes, CODE + 7, 2);
      protect(0x5000, READ); putXmm(state(), 1, [0, 0, 0, 0]); await compile(CODE + 7);
      const prior = Array.from(state()), repairedChanges = {}; shufpsReference(xmmWords(prior, 2), memory, 0xe4).forEach((word, lane) => { repairedChanges[75 + lane] = word; });
      checkScalarSseMove(run(1), prior, repairedChanges, CODE + 11);
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "prior ADD store completes once");
    }));
  await test("SHUFPS raw result survives executable-alias SMC and exact resume", () => fixture(
    [0x0f, 0xc6, 0xc1, 0x1b, 0x0f, 0x11, 0x03, 0x40], async ({ handle, reset, state, run, bytes, alias, compile }) => {
      alias(0x4000, CODE, READ | WRITE); reset({ ebx: 0x4101, eax: 9, flags: 0xad7 }); seedScalarSseMove(state());
      const before = Array.from(state()), changes = {}, shuffled = shufpsReference(xmmWords(before, 0), xmmWords(before, 1), 0x1b); shuffled.forEach((word, lane) => { changes[67 + lane] = word; });
      const result = run(100); equal(result.kind, 7, "completed store causes SMC before INC"); checkScalarSseMove(result, before, changes, CODE + 7, 2);
      const output = new Uint8Array(new Uint32Array(shuffled).buffer); for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "all shuffled raw bits stored");
      check(api.backend_validate(handle, CODE) !== 0, "prior module invalidated"); await compile(CODE + 7); putXmm(state(), 0, [0, 0, 0, 0]);
      const prior = Array.from(state()); checkScalarSseMove(run(1), prior, { 0: 10, 9: arithmetic("add", 32, 9, 1, 0).flags | prior[9] & ~ARITH | prior[9] & CF }, CODE + 8);
      for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "SMC resume never repeats shuffle or store");
    }, READ | WRITE | EXECUTE | SEALED));
  for (const [code, kind] of [[[0x66, 0x0f, 0xc6, 0xc1, 0x44], 3], [[0x67, 0x0f, 0xc6, 0x03, 0x44], 3],
    [[0x65, 0x0f, 0xc6, 0x03, 0x44], 3], [[0xc5, 0xf8, 0xc6, 0xc1, 0x44], 3], [[0xf0, 0x0f, 0xc6, 0x03, 0x44], 8]]) await test("SHUFPS distinct encoding " + code.join("-") + " stops before effects", () => fixture(
      code, async ({ reset, state, run }) => { reset({ ebx: 0x5001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1); equal(result.kind, kind, "valid unsupported versus invalid LOCK"); checkScalarSseMove(result, before, {}, CODE, 0); }));
  const packedSseReference = (operation, a, b, immediate) => {
    if (operation === "or") return a.map((word, n) => (word | b[n]) >>> 0);
    if (operation === "left-dword-shift") {
      const count = immediate === undefined ? BigInt(b[0]) | BigInt(b[1]) << 32n : BigInt(immediate);
      return a.map(word => count >= 32n ? 0 : Number(BigInt(word) * (2n ** count) % 0x100000000n));
    }
    if (operation === "signed-shift") {
      const count = immediate === undefined ? BigInt(b[0]) | BigInt(b[1]) << 32n : BigInt(immediate);
      // Mathematical division models signed arithmetic shift independently of
      // Wasm/JavaScript masked shift counts, including every count >= 32.
      return a.map(word => {
        const signed = word >= 0x80000000 ? word - 0x100000000 : word;
        return (count >= 32n ? (signed < 0 ? -1 : 0) : Math.floor(signed / 2 ** Number(count))) >>> 0;
      });
    }
    const av = new DataView(new Uint32Array(a).buffer), bv = new DataView(new Uint32Array(b).buffer), output = new Uint8Array(16), view = new DataView(output.buffer);
    if (operation === "unpack") for (let n = 0; n < 8; n++) { output[n * 2] = av.getUint8(n); output[n * 2 + 1] = bv.getUint8(n); }
    else if (operation === "unpack-high-words") for (let n = 0; n < 4; n++) {
      view.setUint16(n * 4, av.getUint16(8 + n * 2, true), true);
      view.setUint16(n * 4 + 2, bv.getUint16(8 + n * 2, true), true);
    }
    else if (operation === "add-words") for (let n = 0; n < 8; n++) view.setUint16(n * 2,
      (av.getUint16(n * 2, true) + bv.getUint16(n * 2, true)) % 65536, true);
    else if (operation === "subtract-words") for (let n = 0; n < 8; n++) view.setUint16(n * 2,
      (av.getUint16(n * 2, true) - bv.getUint16(n * 2, true) + 65536) % 65536, true);
    else if (operation === "pack") for (let n = 0; n < 16; n++) output[n] = Math.max(0, Math.min(255, (n < 8 ? av : bv).getInt16((n & 7) * 2, true)));
    else if (operation === "signed-pack") for (let n = 0; n < 16; n++) view.setInt8(n, Math.max(-128, Math.min(127, (n < 8 ? av : bv).getInt16((n & 7) * 2, true))));
    else if (operation === "signed-pack-dwords") for (let n = 0; n < 8; n++) view.setInt16(n * 2, Math.max(-32768, Math.min(32767, (n < 4 ? av : bv).getInt32((n & 3) * 4, true))), true);
    else if (operation === "mul") for (let n = 0; n < 8; n++) view.setUint16(n * 2, Math.imul(av.getUint16(n * 2, true), bv.getUint16(n * 2, true)) & 0xffff, true);
    else if (operation === "shift") {
      const count = immediate === undefined ? BigInt(b[0]) | BigInt(b[1]) << 32n : BigInt(immediate);
      for (let n = 0; n < 8; n++) view.setUint16(n * 2, count >= 16n ? 0 : av.getUint16(n * 2, true) >>> Number(count), true);
    } else if (operation === "signed-word-shift") {
      const count = immediate === undefined ? BigInt(b[0]) | BigInt(b[1]) << 32n : BigInt(immediate);
      for (let n = 0; n < 8; n++) {
        const value = av.getInt16(n * 2, true);
        view.setInt16(n * 2, count >= 16n ? (value < 0 ? -1 : 0) : Math.floor(value / 2 ** Number(count)), true);
      }
    } else throw Error("Unknown packed integer reference operation");
    return Array.from(new Uint32Array(output.buffer));
  };
  const packedSseOperations = [{ name: "PUNPCKLBW", opcode: 0x60, operation: "unpack" },
    { name: "PMULLW", opcode: 0xd5, operation: "mul" }, { name: "PACKUSWB", opcode: 0x67, operation: "pack" },
    { name: "PSRLW", opcode: 0xd1, operation: "shift" }, { name: "POR", opcode: 0xeb, operation: "or" },
    { name: "PACKSSWB", opcode: 0x63, operation: "signed-pack" }, { name: "PSRAD", opcode: 0xe2, operation: "signed-shift" },
    { name: "PACKSSDW", opcode: 0x6b, operation: "signed-pack-dwords" }, { name: "PSRAW", opcode: 0xe1, operation: "signed-word-shift" },
    { name: "PSLLD", opcode: 0xf2, operation: "left-dword-shift" },
    { name: "PADDW", opcode: 0xfd, operation: "add-words" }, { name: "PUNPCKHWD", opcode: 0x69, operation: "unpack-high-words" }];
  packedSseOperations.push({ name: "PSUBW", opcode: 0xf9, operation: "subtract-words" });
  const signedWordVector = values => Array.from(new Uint32Array(Int16Array.from(values).buffer));
  for (const operation of packedSseOperations.filter(op => ["add-words", "subtract-words", "unpack-high-words"].includes(op.operation))) {
    await test(`SSE2 ${operation.name} all64 pairs preserve raw lane order and modulo16 arithmetic`, async () => {
      const patterns = [[0xffff, 0x7fff, 0x8000, 0, 1, 0xfffe, 0x1234, 0xabcd],
        [1, 1, 0x8000, 0xffff, 0xffff, 2, 0xedcb, 0x5433],
        [0x1111, 0x2222, 0x3333, 0x4444, 0x5555, 0x6666, 0x7777, 0x8888],
        [0x0001, 0x7f80, 0xbeef, 0xffc1, 0x0000, 0x8000, 0xffff, 0xffff]];
      for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
        [0x66, 0x0f, operation.opcode, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
          for (let pattern = 0; pattern < patterns.length; pattern++) {
            reset({ flags: 0xad7 }); seedScalarSseMove(state());
            putXmm(state(), destination, signedWordVector(patterns[pattern]));
            if (source !== destination) putXmm(state(), source, signedWordVector(patterns[(pattern + 1) % patterns.length]));
            const before = Array.from(state()), changes = {};
            packedSseReference(operation.operation, xmmWords(before, destination), xmmWords(before, source))
              .forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
            checkScalarSseMove(run(0), before, {}, CODE, 0);
            checkScalarSseMove(run(1), before, changes, CODE + 4);
          }
        });
    });
  }
  await test("SSE2 PACKSSDW covers all64 pairs, signed32 saturation boundaries, order and alias", async () => {
    const edges = [-2147483648, -2147483647, -65536, -32769, -32768, -32767, -1, 0, 1, 32766, 32767, 32768, 32769, 65535, 2147483646, 2147483647];
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
      [0x66, 0x0f, 0x6b, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        for (let rotation = 0; rotation < edges.length; rotation++) {
          reset({ flags: 0xad7 }); seedScalarSseMove(state());
          putXmm(state(), destination, Array.from({ length: 4 }, (_, n) => edges[(rotation + n) % edges.length] >>> 0));
          if (source !== destination) putXmm(state(), source, Array.from({ length: 4 }, (_, n) => edges[(rotation + n + 4) % edges.length] >>> 0));
          const before = Array.from(state()), changes = {};
          packedSseReference("signed-pack-dwords", xmmWords(before, destination), xmmWords(before, source)).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
          checkScalarSseMove(run(0), before, {}, CODE, 0); checkScalarSseMove(run(1), before, changes, CODE + 4);
        }
      });
  });
  await test("SSE2 PACKSSDW exact small integers and deterministic full32 samples match scalar clamp", () => fixture(
    [0x66, 0x0f, 0x6b, 0xc1], async ({ reset, state, run }) => {
      let random = 0x3b92a17d;
      const sample = () => { random ^= random << 13; random ^= random >>> 17; random ^= random << 5; return random | 0; };
      for (let iteration = 0; iteration < 2048; iteration++) {
        const input = Array.from({ length: 8 }, (_, n) => iteration < 1024 ? -32772 + ((iteration * 67 + n) % 65544) : sample());
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), 0, input.slice(0, 4)); putXmm(state(), 1, input.slice(4));
        const before = Array.from(state()), changes = {}, expected = signedWordVector(input.map(value => Math.max(-32768, Math.min(32767, value))));
        expected.forEach((word, n) => { changes[67 + n] = word; }); checkScalarSseMove(run(1), before, changes, CODE + 4);
      }
    }));
  await test("SSE2 PSRAW all256 immediates preserve signed16 lane semantics beyond count15", async () => {
    const edges = [-32768, -32767, -16385, -3, -2, -1, 0, 1, 2, 3, 16385, 32766, 32767];
    for (let count = 0; count <= 255; count++) await fixture(
      [0x66, 0x0f, 0x71, 0xe0 | count % 8, count], async ({ reset, state, run }) => {
        const destination = count % 8;
        for (let rotation = 0; rotation < edges.length; rotation++) {
          reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), destination, signedWordVector(Array.from({ length: 8 }, (_, n) => edges[(rotation + n) % edges.length])));
          const before = Array.from(state()), changes = {};
          packedSseReference("signed-word-shift", xmmWords(before, destination), [], count).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
          checkScalarSseMove(run(0), before, {}, CODE, 0); checkScalarSseMove(run(1), before, changes, CODE + 5);
        }
      });
  });
  await test("SSE2 PSRAW variable count covers all64 pairs aliases full low64 and ignored upper64", async () => {
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
      [0x66, 0x0f, 0xe1, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        for (const count of [0n, 1n, 3n, 7n, 14n, 15n, 16n, 17n, 31n, 32n, 63n, 64n, 255n, 0xffffffffn, 0x100000000n, 0x100000001n, 0x8000000000000000n, 0xffffffffffffffffn]) for (const upper of [[0, 0], [0xffffffff, 0x80000000]]) {
          reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), destination, signedWordVector([-32768, 32767, -1, 1, -3, 3, -16385, 16385]));
          putXmm(state(), source, [Number(count & 0xffffffffn), Number(count >> 32n), ...upper]);
          const before = Array.from(state()), changes = {};
          packedSseReference("signed-word-shift", xmmWords(before, destination), xmmWords(before, source)).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
          checkScalarSseMove(run(1), before, changes, CODE + 4);
        }
      });
  });
  await test("SSE2 PSRAW aligned alias memory reads all128 but uses the entire unsigned low64 count", () => fixture(
    [0x66, 0x0f, 0xe1, 0x13], async ({ reset, state, run, bytes, alias }) => {
      alias(0x4000, DATA, READ);
      for (const count of [0n, 1n, 15n, 16n, 17n, 0x100000000n, 0x100000001n, 0xffffffffffffffffn]) {
        const memory = [Number(count & 0xffffffffn), Number(count >> 32n), 0xffffffff, 0x80000000]; memory.forEach((word, n) => setWord(bytes(DATA), 4080 + n * 4, word));
        reset({ ebx: 0x4ff0, flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), 2, signedWordVector([-32768, 32767, -1, 1, -3, 3, -16385, 16385]));
        const before = Array.from(state()), image = bytes(DATA).slice(), changes = {};
        packedSseReference("signed-word-shift", xmmWords(before, 2), memory).forEach((word, n) => { changes[75 + n] = word; }); checkScalarSseMove(run(1), before, changes, CODE + 4);
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "PSRAW only reads memory");
      }
    }));
  for (const [code, kind] of [[[0xc5, 0xf9, 0x71, 0xe0, 3], 3], [[0xf0, 0x66, 0x0f, 0x71, 0xe0, 3], 8], [[0x66, 0x0f, 0x71, 0x20, 3], 8], [[0xf2, 0x66, 0x0f, 0x71, 0xe0, 3], 8], [[0xf3, 0x66, 0x0f, 0x71, 0xe0, 3], 8]]) await test("SSE2 PSRAW unsupported or invalid immediate " + code.join("-"), () => fixture(
    code, async ({ reset, state, run }) => { reset({ ebx: 0x5001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1); equal(result.kind, kind, "no accidental MMX/AVX execution or invented UD"); checkScalarSseMove(result, before, {}, CODE, 0); }));
  await test("SSE2 PACKSSWB covers every register pair, signed saturation boundary, lane order and source alias", async () => {
    const edges = [-32768, -129, -128, -127, -1, 0, 1, 126, 127, 128, 129, 255, 256, 32767, -256, -255];
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
      [0x66, 0x0f, 0x63, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        for (let rotation = 0; rotation < edges.length; rotation++) {
          reset({ flags: 0xad7 }); seedScalarSseMove(state());
          putXmm(state(), destination, signedWordVector(Array.from({ length: 8 }, (_, n) => edges[(rotation + n) % edges.length])));
          if (destination !== source) putXmm(state(), source, signedWordVector(Array.from({ length: 8 }, (_, n) => edges[(rotation + n + 8) % edges.length])));
          const before = Array.from(state()), changes = {};
          packedSseReference("signed-pack", xmmWords(before, destination), xmmWords(before, source)).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
          checkScalarSseMove(run(0), before, {}, CODE, 0); checkScalarSseMove(run(1), before, changes, CODE + 4);
        }
      });
  });
  await test("SSE2 PACKSSWB compares all65536 signed input words against independent scalar saturation", () => fixture(
    [0x66, 0x0f, 0x63, 0xc1], async ({ reset, state, run }) => {
      for (let first = -32768; first < 32768; first += 16) {
        reset({ flags: 0xad7 }); seedScalarSseMove(state());
        putXmm(state(), 0, signedWordVector(Array.from({ length: 8 }, (_, n) => first + n)));
        putXmm(state(), 1, signedWordVector(Array.from({ length: 8 }, (_, n) => first + n + 8)));
        const before = Array.from(state()), output = Int8Array.from({ length: 16 }, (_, n) => Math.max(-128, Math.min(127, first + n))), changes = {};
        Array.from(new Uint32Array(output.buffer)).forEach((word, n) => { changes[67 + n] = word; });
        checkScalarSseMove(run(1), before, changes, CODE + 4);
      }
    }));
  await test("SSE2 POR covers every XMM pair and every result lane without MMX or FP state effects", async () => {
    const patterns = [[0, 0, 0, 0], [0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff],
      [0x01234567, 0x89abcdef, 0xaaaaaaaa, 0x55555555], [0x7f800001, 0x80000000, 0xdeadbeef, 0x01020408]];
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
      [0x66, 0x0f, 0xeb, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        for (let pattern = 0; pattern < patterns.length; pattern++) {
          reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), destination, patterns[pattern]);
          putXmm(state(), source, patterns[(pattern + 2) % patterns.length]); const before = Array.from(state()), changes = {};
          packedSseReference("or", xmmWords(before, destination), xmmWords(before, source)).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
          checkScalarSseMove(run(0), before, {}, CODE, 0);
          checkScalarSseMove(run(1), before, changes, CODE + 4);
        }
      });
  });
  for (const operation of [{ name: "POR", opcode: 0xeb, reference: "or" }, { name: "PACKSSWB", opcode: 0x63, reference: "signed-pack" },
    { name: "PSRAD", opcode: 0xe2, reference: "signed-shift" }, { name: "PACKSSDW", opcode: 0x6b, reference: "signed-pack-dwords" },
    { name: "PSRAW", opcode: 0xe1, reference: "signed-word-shift" },
    { name: "PSLLD", opcode: 0xf2, reference: "left-dword-shift" },
    { name: "PADDW", opcode: 0xfd, reference: "add-words", mmxSupported: true },
    { name: "PSUBW", opcode: 0xf9, reference: "subtract-words", mmxSupported: true },
    { name: "PUNPCKHWD", opcode: 0x69, reference: "unpack-high-words", mmxSupported: true }]) {
  for (const code of [...(operation.mmxSupported ? [] : [[0x0f, operation.opcode, 0xc1], [0x0f, operation.opcode, 0x03]]), [0x67, 0x66, 0x0f, operation.opcode, 0x03],
    [0x65, 0x66, 0x0f, operation.opcode, 0x03], [0xc5, 0xf9, operation.opcode, 0xc1]]) await test(`unimplemented ${operation.name} encoding ${code.join("-")} does not enter the XMM path or invent UD`, () => fixture(
      code, async ({ reset, state, run }) => {
        reset({ ebx: 0x5001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1);
        equal(result.kind, 3, "unimplemented MMX, address16, GS or AVX form stops explicitly"); checkScalarSseMove(result, before, {}, CODE, 0);
      }));
  await test(`SSE2 ${operation.name} prior vector and scalar effects survive fault repair without replay`, () => fixture(
    [0x66, 0x0f, operation.opcode, 0xc1, 0x83, 0x07, 1, 0x66, 0x0f, operation.opcode, 0x13], async ({ reset, state, run, bytes, map, compile }) => {
      setWord(bytes(STACK), 0, 0x7fffffff); reset({ ebx: 0x5000, edi: STACK, flags: 0x203 }); seedScalarSseMove(state());
      const before = Array.from(state()), changes = { 9: (before[9] & ~ARITH) | arithmetic("add", 32, 0x7fffffff, 1, 0).flags };
      packedSseReference(operation.reference, xmmWords(before, 0), xmmWords(before, 1)).forEach((word, n) => { changes[67 + n] = word; });
      const fault = run(100); equal(fault.kind, 4, "later packed operation raises precise missing-source fault"); equal(fault.address, 0x5000, "fault address");
      checkScalarSseMove(fault, before, changes, CODE + 7, 2);
      const memory = [0x80000001, 0x10204080, 0x7f800001, 0x01010101], input = page(); memory.forEach((word, n) => setWord(input, n * 4, word)); map(0x5000, READ, input);
      putXmm(state(), 1, [0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff]); await compile(CODE + 7);
      const repairedBefore = Array.from(state()), repairedChanges = {};
      packedSseReference(operation.reference, xmmWords(repairedBefore, 2), memory).forEach((word, n) => { repairedChanges[75 + n] = word; });
      checkScalarSseMove(run(1), repairedBefore, repairedChanges, CODE + 11);
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "prior memory ADD remains completed once");
    }));
  await test(`SSE2 ${operation.name} result survives a following executable-alias store and exact SMC resume`, () => fixture(
    [0x66, 0x0f, operation.opcode, 0xc1, 0x0f, 0x11, 0x03, 0x40], async ({ handle, reset, state, run, bytes, alias, compile }) => {
      alias(0x4000, CODE, READ | WRITE); reset({ ebx: 0x4101, eax: 9, flags: 0xad7 }); seedScalarSseMove(state());
      const before = Array.from(state()), changes = {}, merged = packedSseReference(operation.reference, xmmWords(before, 0), xmmWords(before, 1));
      merged.forEach((word, n) => { changes[67 + n] = word; });
      const result = run(100); equal(result.kind, 7, "completed vector store invalidates code before INC"); checkScalarSseMove(result, before, changes, CODE + 7, 2);
      const output = new Uint8Array(new Uint32Array(merged).buffer); for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "full128 result written through alias");
      check(api.backend_validate(handle, CODE) !== 0, "old compiled code rejected"); await compile(CODE + 7); putXmm(state(), 0, [0, 0, 0, 0]);
      const resumedBefore = Array.from(state()); checkScalarSseMove(run(1), resumedBefore, { 0: 10, 9: arithmetic("add", 32, 9, 1, 0).flags | (resumedBefore[9] & ~ARITH) | (resumedBefore[9] & CF) }, CODE + 8);
      for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], output[n], "resume does not replay prior packed operation or store");
    }, READ | WRITE | EXECUTE | SEALED));
  }
  for (const toMmx of [false, true]) await test(`${toMmx ? "MOVDQ2Q" : "MOVQ2DQ"} crosses raw register banks with exact MMX entry state`, async () => {
    for (let destination = 0; destination < 8; destination++) for (const source of [destination, (destination + 1) & 7]) await fixture(
      [toMmx ? 0xf2 : 0xf3, 0x0f, 0xd6, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[59] = 0x37f; state()[60] = 0x3a20;
        for (let r = 0; r < 8; r++) setMm(state(), r, 0x8877665544332211n ^ BigInt(r), 0x4000 + r);
        const before = Array.from(state()), changes = { 60: before[60] & ~0x3800, 61: 0 };
        if (toMmx) Object.assign(changes, { [35 + destination * 3]: before[67 + source * 4], [36 + destination * 3]: before[68 + source * 4], [37 + destination * 3]: 0xffff });
        else Object.assign(changes, { [67 + destination * 4]: before[35 + source * 3], [68 + destination * 4]: before[36 + source * 3], [69 + destination * 4]: 0, [70 + destination * 4]: 0 });
        checkScalarSseMove(run(0), before, {}, CODE, 0); checkScalarSseMove(run(1), before, changes, CODE + 4);
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); const pending = Array.from(state()), fault = run(1);
        equal(fault.kind, 14, "crossbank instruction delivers pending x87 exception before transfer"); checkScalarSseMove(fault, pending, {}, CODE, 0);
      });
  });
  for (const operation of packedSseOperations) await test(`SSE2 ${operation.name} keeps all128 result bits and its register bank`, async () => {
    for (let destination = 0; destination < 8; destination++) for (const source of [destination, (destination + 1) & 7]) await fixture(
      [0x66, 0x0f, operation.opcode, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        const values = ["shift", "signed-shift", "signed-word-shift", "left-dword-shift"].includes(operation.operation) ? [0n, 1n, 15n, 16n, 31n, 32n, 33n, 63n, 64n, 255n, 0x100000000n, 0x100000001n, 0x8000000000000000n, 0xffffffffffffffffn] : [null];
        for (const count of values) {
          reset({ flags: 0xad7 }); seedScalarSseMove(state());
          for (let r = 0; r < 8; r++) putXmm(state(), r, [(0x8000ffff ^ r * 0x01010101) >>> 0, 0x010000ff, 0x7fff0000, 0xff000001]);
          if (count !== null) putXmm(state(), source, [Number(count & 0xffffffffn), Number(count >> 32n), 0xffffffff, 0x80000000]);
          const before = Array.from(state()), changes = {};
          packedSseReference(operation.operation, xmmWords(before, destination), xmmWords(before, source)).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
          checkScalarSseMove(run(1), before, changes, CODE + 4);
        }
      });
  });
  for (const count of [0, 1, 7, 8, 15, 16, 31, 32, 63, 64, 127, 255]) await test(`SSE2 PSRLW immediate${count} compares the count before any Wasm shift masking`, () => fixture(
    [0x66, 0x0f, 0x71, 0xd0 | count % 8, count], async ({ reset, state, run }) => {
      const destination = count % 8; reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), destination, [0x8000ffff, 0x7fff0001, 0xff0000ff, 0x0000aaaa]); const before = Array.from(state()), changes = {};
      packedSseReference("shift", xmmWords(before, destination), [], count).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
      checkScalarSseMove(run(1), before, changes, CODE + 5);
    }));
  await test("SSE2 PSRAD immediate exhausts all256 count bytes across every XMM destination and signed32 boundaries", async () => {
    const edges = [0, 1, 0x7fffffff, 0x80000000, 0xffffffff, 0x80000001, 0x40000001, 0xbfffffff];
    for (let count = 0; count <= 255; count++) await fixture(
      [0x66, 0x0f, 0x72, 0xe0 | count % 8, count], async ({ reset, state, run }) => {
        const destination = count % 8;
        for (let rotation = 0; rotation < edges.length; rotation++) {
          reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), destination, Array.from({ length: 4 }, (_, n) => edges[(rotation + n) % edges.length]));
          const before = Array.from(state()), changes = {};
          packedSseReference("signed-shift", xmmWords(before, destination), [], count).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
          checkScalarSseMove(run(0), before, {}, CODE, 0); checkScalarSseMove(run(1), before, changes, CODE + 5);
        }
      });
  });
  await test("SSE2 PSLLD all256 immediates and raw dword boundaries never use a modulo32 count", async () => {
    for (let count = 0; count < 256; count++) {
      const destination = count & 7;
      await fixture([0x66, 0x0f, 0x72, 0xf0 | destination, count], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), destination, [0x80000001, 0x7fffffff, 0xffffffff, 0x01234567]);
        const before = Array.from(state()), changes = {};
        packedSseReference("left-dword-shift", xmmWords(before, destination), [], count).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
        checkScalarSseMove(run(0), before, {}, CODE, 0); checkScalarSseMove(run(1), before, changes, CODE + 5);
      });
    }
  });
  await test("SSE2 PSLLD all64 register pairs read the old full low64 count before alias writes", async () => {
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
      [0x66, 0x0f, 0xf2, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        for (const count of [0n, 1n, 16n, 31n, 32n, 33n, 255n, 0x100000000n, 0x8000000000000000n, 0xffffffffffffffffn]) {
          reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), destination, [0x80000001, 0x7fffffff, 0xffffffff, 0x01234567]);
          putXmm(state(), source, [Number(count & 0xffffffffn), Number(count >> 32n), 0xffffffff, 0x80000000]);
          const before = Array.from(state()), changes = {};
          packedSseReference("left-dword-shift", xmmWords(before, destination), xmmWords(before, source)).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
          checkScalarSseMove(run(1), before, changes, CODE + 4);
        }
      });
  });
  const pinsrwReference = (words, value, immediate) => {
    const bytes = new Uint8Array(new Uint32Array(words).buffer).slice();
    new DataView(bytes.buffer).setUint16((immediate & 7) * 2, value & 0xffff, true);
    return Array.from(new Uint32Array(bytes.buffer));
  };
  await test("PINSRW all256 selector bytes and all64 XMM/GPR pairs preserve seven untouched words", async () => {
    for (let immediate = 0; immediate < 256; immediate++) {
      const destination = immediate & 7, source = immediate >> 3 & 7;
      await fixture([0x66, 0x0f, 0xc4, 0xc0 | destination << 3 | source, immediate], async ({ reset, state, run }) => {
        for (const value of [0x12340000, 0x5678ffff, 0x80007fff, 0xffff8000]) {
          reset({ flags: 0xad7 }); seedScalarSseMove(state()); state()[source] = value;
          const before = Array.from(state()), changes = {};
          pinsrwReference(xmmWords(before, destination), value, immediate).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
          checkScalarSseMove(run(0), before, {}, CODE, 0); checkScalarSseMove(run(1), before, changes, CODE + 5);
        }
      });
    }
  });
  await test("PINSRW m16 permits unaligned and last-two-byte reads and respects FS before alias access", () => fixture(
    [0x64, 0x66, 0x0f, 0xc4, 0x13, 0xff], async ({ reset, state, run, bytes, alias, map }) => {
      alias(0x4000, DATA, READ); map(DATA + 4096, READ | WRITE); bytes(DATA).fill(0xa5); bytes(DATA + 4096).fill(0x5a);
      for (const [address, fs, expected] of [[0x4001, 0, 0xa5a5], [0x4ffe, 0, 0xa5a5], [0x4001, DATA - 0x4000, 0xa5a5], [DATA + 4095, 0, 0x5aa5]]) {
        reset({ ebx: address, fs, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), changes = {};
        pinsrwReference(xmmWords(before, 2), expected, 0xff).forEach((word, n) => { changes[75 + n] = word; });
        checkScalarSseMove(run(1), before, changes, CODE + 6);
      }
    }));
  for (const present of [false, true]) await test(`PINSRW full two-byte ${present ? "protection" : "missing"} fault preserves prior effects and resumes once`, () => fixture(
    [0x66, 0x0f, 0xc4, 0xc0, 2, 0x83, 0x07, 1, 0x66, 0x0f, 0xc4, 0x13, 0xfe], async ({ reset, state, run, bytes, map, protect, compile }) => {
      bytes(DATA)[4095] = 0x34; if (present) map(DATA + 4096, WRITE); setWord(bytes(STACK), 0, 0x7fffffff);
      reset({ eax: 0x8765abcd, ebx: DATA + 4095, edi: STACK, flags: 0x203 }); seedScalarSseMove(state());
      const before = Array.from(state()), changes = { 9: before[9] & ~ARITH | arithmetic("add", 32, 0x7fffffff, 1, 0).flags };
      pinsrwReference(xmmWords(before, 0), before[0], 2).forEach((word, n) => { changes[67 + n] = word; });
      const fault = run(100); equal(fault.kind, 4, "precise m16 fault"); equal(fault.address, DATA + 4096, "first unavailable byte"); equal(fault.error, present ? 5 : 4, "source read fault permissions");
      checkScalarSseMove(fault, before, changes, CODE + 8, 2);
      if (present) protect(DATA + 4096, READ | WRITE); else map(DATA + 4096, READ | WRITE); bytes(DATA + 4096)[0] = 0x12;
      await compile(CODE + 8); const prior = Array.from(state()), repaired = {}; pinsrwReference(xmmWords(prior, 2), 0x1234, 0xfe).forEach((word, n) => { repaired[75 + n] = word; });
      checkScalarSseMove(run(1), prior, repaired, CODE + 13); equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "earlier ADD is not replayed");
    }));
  await test("PINSRW FS offset wrap raises GP before any word replacement", () => fixture(
    [0x64, 0x66, 0x0f, 0xc4, 0x13, 1], async ({ reset, state, run }) => {
      reset({ ebx: 0xffffffff, fs: DATA + 1, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), fault = run(1);
      equal(fault.kind, 6, "full segment offset span wraps"); checkScalarSseMove(fault, before, {}, CODE, 0);
    }));
  await test("PINSRW result survives executable-alias SMC and rebuild without repeating insertion", () => fixture(
    [0x66, 0x0f, 0xc4, 0xc0, 7, 0x0f, 0x11, 0x03, 0x40], async ({ handle, reset, state, run, bytes, alias, compile }) => {
      alias(0x4000, CODE, READ | WRITE); reset({ ebx: 0x4101, eax: 9, flags: 0xad7 }); seedScalarSseMove(state());
      const before = Array.from(state()), changes = {}, output = pinsrwReference(xmmWords(before, 0), before[0], 7); output.forEach((word, n) => { changes[67 + n] = word; });
      const result = run(100); equal(result.kind, 7, "completed store causes SMC"); checkScalarSseMove(result, before, changes, CODE + 8, 2);
      const stored = new Uint8Array(new Uint32Array(output).buffer); for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], stored[n], "raw insertion result stored");
      check(api.backend_validate(handle, CODE) !== 0, "old code invalidated"); await compile(CODE + 8); putXmm(state(), 0, [0, 0, 0, 0]);
      const prior = Array.from(state()); checkScalarSseMove(run(1), prior, { 0: 10, 9: arithmetic("add", 32, 9, 1, 0).flags | prior[9] & ~ARITH | prior[9] & CF }, CODE + 9);
      for (let n = 0; n < 16; n++) equal(bytes(CODE)[0x101 + n], stored[n], "resume does not repeat insertion or store");
    }, READ | WRITE | EXECUTE | SEALED));
  for (const [code, kind] of [[[0x0f, 0xc4, 0xc1, 0], 3], [[0xc5, 0xf9, 0xc4, 0xc1, 0], 3], [[0x67, 0x66, 0x0f, 0xc4, 0x03, 0], 3], [[0x65, 0x66, 0x0f, 0xc4, 0x03, 0], 3],
    [[0x0f, 0x72, 0xf0, 0], 3], [[0xc5, 0xf9, 0x72, 0xf0, 0], 3], [[0xf0, 0x66, 0x0f, 0x72, 0xf0, 0], 8], [[0x66, 0x0f, 0x72, 0x30, 0], 8], [[0xf0, 0x66, 0x0f, 0xc4, 0x03, 0], 8]]) await test("PSLLD/PINSRW rejected encoding " + code.join("-"), () => fixture(
      code, async ({ reset, state, run }) => { reset({ ebx: 0x5001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1); equal(result.kind, kind, "unsupported form versus invalid encoding"); checkScalarSseMove(result, before, {}, CODE, 0); }));
  await test("SSE2 PSRAD register count covers all64 pairs, aliases, full low64 and ignored upper64", async () => {
    for (let destination = 0; destination < 8; destination++) for (let source = 0; source < 8; source++) await fixture(
      [0x66, 0x0f, 0xe2, 0xc0 | destination << 3 | source], async ({ reset, state, run }) => {
        for (const count of [0n, 1n, 16n, 30n, 31n, 32n, 33n, 63n, 64n, 255n, 0xffffffffn, 0x100000000n, 0x100000001n, 0x8000000000000000n, 0xffffffffffffffffn]) {
          for (const upper of [[0, 0], [0xffffffff, 0x80000000]]) {
            reset({ flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), destination, [0x80000000, 0x7fffffff, 0xffffffff, 0x40000001]);
            putXmm(state(), source, [Number(count & 0xffffffffn), Number(count >> 32n), ...upper]);
            const before = Array.from(state()), changes = {};
            packedSseReference("signed-shift", xmmWords(before, destination), xmmWords(before, source)).forEach((word, n) => { changes[67 + destination * 4 + n] = word; });
            checkScalarSseMove(run(1), before, changes, CODE + 4);
          }
        }
      });
  });
  await test("SSE2 PSRAD memory low64 saturates before truncation and ignores upper64 after the full read", () => fixture(
    [0x66, 0x0f, 0xe2, 0x13], async ({ reset, state, run, bytes, alias }) => {
      alias(0x4000, DATA, READ);
      for (const count of [0n, 1n, 31n, 32n, 33n, 0x100000000n, 0x100000001n, 0xffffffffffffffffn]) {
        const source = [Number(count & 0xffffffffn), Number(count >> 32n), 0xffffffff, 0x80000000]; source.forEach((word, n) => setWord(bytes(DATA), 4080 + n * 4, word));
        reset({ ebx: 0x4ff0, flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), 2, [0x80000000, 0x7fffffff, 0xffffffff, 0x40000001]);
        const before = Array.from(state()), changes = {}, image = bytes(DATA).slice();
        packedSseReference("signed-shift", xmmWords(before, 2), source).forEach((word, n) => { changes[75 + n] = word; });
        checkScalarSseMove(run(1), before, changes, CODE + 4);
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "PSRAD alias read preserves memory");
      }
    }));
  for (const [code, kind] of [[[0x0f, 0x72, 0xe0, 16], 3], [[0xc5, 0xf9, 0x72, 0xe0, 16], 3],
    [[0xf0, 0x66, 0x0f, 0x72, 0xe0, 16], 8]]) await test(`PSRAD immediate rejected form ${code.join("-")} preserves all state`, () => fixture(
      code, async ({ reset, state, run }) => {
        reset({ flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1);
        equal(result.kind, kind, "unsupported bank and invalid LOCK are distinct"); checkScalarSseMove(result, before, {}, CODE, 0);
      }));
  for (const operation of packedSseOperations) {
    await test(`SSE2 ${operation.name} memory alignment/protection and full segment span precede vector mutation`, () => fixture(
      [0x64, 0x66, 0x0f, operation.opcode, 0x13], async ({ reset, state, run, bytes, map, protect }) => {
        map(DATA + 4096, WRITE); const memory = ["shift", "signed-shift", "signed-word-shift", "left-dword-shift"].includes(operation.operation) ? [1, 0, 0xffffffff, 0xffffffff] : [0x8000ffff, 0x010000ff, 0x7fff0000, 0xff000001];
        for (const [offset, base, rights, kind, errorCode] of [[DATA, 0, READ, 1, 0], [DATA + 4080, 0, READ, 1, 0],
          [DATA + 4088, 0, READ, 6, 0], [0x5001, 0, READ, 6, 0], [DATA + 1, 0, WRITE, 6, 0], [DATA, 0, WRITE, 4, 5], [0x5000, 0, READ, 4, 4],
          [DATA + 8, 8, READ, 1, 0], [0xfffffff8, DATA + 24, READ, 6, 0]]) {
          protect(DATA, READ | WRITE); bytes(DATA).fill(0x55); for (const at of [0, 16, 4080]) memory.forEach((word, n) => setWord(bytes(DATA), at + n * 4, word));
          reset({ ebx: offset, fs: base, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), image = bytes(DATA).slice(), changes = {}; protect(DATA, rights);
          const result = run(1); equal(result.kind, kind, "SSE packed memory has precise GP/PF contract despite pending x87");
          if (kind === 1) packedSseReference(operation.operation, xmmWords(before, 2), memory).forEach((word, n) => { changes[75 + n] = word; });
          else { equal(result.error, errorCode, "packed operand fault error"); if (kind === 4) equal(result.address, offset, "first unavailable aligned page"); }
          checkScalarSseMove(result, before, changes, kind === 1 ? CODE + 5 : CODE, kind === 1 ? 1 : 0);
          for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "packed read never mutates guest bytes");
        }
      }));
    await test(`SSE2 ${operation.name} repairs missing source without repeating prior memory ADD`, () => fixture(
      [0x83, 0x07, 1, 0x66, 0x0f, operation.opcode, 0x13], async ({ reset, state, run, bytes, map, compile }) => {
        setWord(bytes(STACK), 0, 0x7fffffff); reset({ ebx: 0x5000, edi: STACK, flags: 0x203 }); seedScalarSseMove(state()); const before = Array.from(state()), fault = run(100);
        equal(fault.kind, 4, "packed source fault"); equal(fault.address, 0x5000, "precise missing operand");
        checkScalarSseMove(fault, before, { 9: (before[9] & ~ARITH) | arithmetic("add", 32, 0x7fffffff, 1, 0).flags }, CODE + 3);
        const memory = ["shift", "signed-shift", "signed-word-shift", "left-dword-shift"].includes(operation.operation) ? [1, 0, 0xffffffff, 0xffffffff] : [0x8000ffff, 0x010000ff, 0x7fff0000, 0xff000001], input = page(); memory.forEach((word, n) => setWord(input, n * 4, word)); map(0x5000, READ, input);
        await compile(CODE + 3); const prior = Array.from(state()), changes = {}; packedSseReference(operation.operation, xmmWords(prior, 2), memory).forEach((word, n) => { changes[75 + n] = word; });
        checkScalarSseMove(run(1), prior, changes, CODE + 7); equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x80000000, "earlier memory write completed once");
      }));
    await test(`LOCK SSE2 ${operation.name} is rejected before missing operand access`, () => fixture(
      [0xf0, 0x66, 0x0f, operation.opcode, 0x13], async ({ reset, state, run }) => {
        reset({ ebx: 0x5001, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state()), result = run(1); equal(result.kind, 8, "invalid LOCK wins over memory fault"); checkScalarSseMove(result, before, {}, CODE, 0);
      }));
  }
  await test("MOVQ2DQ and MOVDQ2Q preserve exact x87 values around EMMS and later precise arithmetic", () => fixture(
    [0xd9, 0xe8, 0xf3, 0x0f, 0xd6, 0xc7, 0xf2, 0x0f, 0xd6, 0xf8, 0x0f, 0x77, 0xd9, 0xe8, 0xd9, 0x1b], async ({ reset, state, run, bytes }) => {
      const pcs = [0, 2, 6, 10, 12, 14, 16];
      for (let budget = 0; budget <= 6; budget++) {
        reset({ ebx: DATA, flags: 0xad7 }); seedScalarSseMove(state()); state()[59] = 0x37f; state()[60] = 0; state()[61] = 0xffff; setWord(bytes(DATA), 0, 0xdeadbeef); const before = Array.from(state()), changes = {};
        if (budget) {
          Object.assign(changes, { 56: 0, 57: 0x80000000, 58: budget === 3 || budget === 4 ? 0xffff : 0x3fff, 60: budget === 1 || budget === 5 ? 0x3800 : 0,
            61: budget === 1 || budget === 5 ? 0x3fff : budget === 2 || budget === 3 ? 0 : 0xffff,
            62: budget === 6 ? 0x11b : 0x1e8, 63: CODE + (budget === 6 ? 14 : budget >= 5 ? 12 : 0), 64: before[15] });
          if (budget >= 2) Object.assign(changes, { 67: 0, 68: 0x80000000, 69: 0, 70: 0 });
          if (budget === 6) Object.assign(changes, { 65: DATA, 66: before[23] });
        }
        checkScalarSseMove(run(budget), before, changes, CODE + pcs[budget], budget);
        equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true), budget === 6 ? 0x3f800000 : 0xdeadbeef, "post-EMMS x87 store sees fresh exact1.0");
      }
    }));
  // A relocated real packed-pixel loop. Addresses, pixels and scale factors are
  // fixture inputs; the backend sees only ordinary guest instructions.
  const pixelLoopCode = [0x0f, 0x6f, 0x04, 0x02, 0xf3, 0x0f, 0xd6, 0xd0,
    0x66, 0x0f, 0x60, 0xd1, 0x66, 0x0f, 0xd5, 0xd0, 0x66, 0x0f, 0x71, 0xd2, 8,
    0x66, 0x0f, 0x67, 0xd1, 0xf2, 0x0f, 0xd6, 0xc2, 0x0f, 0x7f, 0,
    0x83, 0xc0, 8, 0x4e, 0x75, 0xda];
  const pixelLoopReference = (initial, originalPages, budget) => {
    const words = Uint32Array.from(initial), images = new Map([...originalPages].map(([address, image]) => [address, image.slice()]));
    const read = address => { const image = images.get(address & ~4095); if (!image) throw Error("Reference read outside explicit fixture pages"); return image[address & 4095]; };
    const write = (address, value) => { const image = images.get(address & ~4095); if (!image) throw Error("Reference write outside explicit fixture pages"); image[address & 4095] = value; };
    const mmxEntry = () => { words[60] &= ~0x3800; words[61] = 0; };
    for (let n = 0; n < budget; n++) {
      const offset = words[8] - CODE;
      switch (offset) {
        case 0: { const input = Uint8Array.from({ length: 8 }, (_, index) => read((words[0] + words[2] + index) >>> 0)); const data = new DataView(input.buffer);
          words[35] = data.getUint32(0, true); words[36] = data.getUint32(4, true); words[37] = 0xffff; mmxEntry(); words[8] += 4; break; }
        case 4: putXmm(words, 2, [words[35], words[36], 0, 0]); mmxEntry(); words[8] += 4; break;
        case 8: putXmm(words, 2, packedSseReference("unpack", xmmWords(words, 2), xmmWords(words, 1))); words[8] += 4; break;
        case 12: putXmm(words, 2, packedSseReference("mul", xmmWords(words, 2), xmmWords(words, 0))); words[8] += 4; break;
        case 16: putXmm(words, 2, packedSseReference("shift", xmmWords(words, 2), [], 8)); words[8] += 5; break;
        case 21: putXmm(words, 2, packedSseReference("pack", xmmWords(words, 2), xmmWords(words, 1))); words[8] += 4; break;
        case 25: words[35] = words[75]; words[36] = words[76]; words[37] = 0xffff; mmxEntry(); words[8] += 4; break;
        case 29: { const output = new Uint8Array(new Uint32Array([words[35], words[36]]).buffer); output.forEach((value, index) => write((words[0] + index) >>> 0, value)); mmxEntry(); words[8] += 3; break; }
        case 32: { const result = arithmetic("add", 32, words[0], 8, 0); words[0] = result.result; words[9] = (words[9] & ~ARITH) | result.flags; words[10] |= ARITH; words[8] += 3; break; }
        case 35: { const result = arithmetic("sub", 32, words[6], 1, 0); words[6] = result.result; words[9] = (words[9] & ~(ARITH & ~CF)) | (result.flags & ~CF); words[10] |= ARITH & ~CF; words[8]++; break; }
        case 36: words[8] = words[9] & ZF ? CODE + 38 : CODE; break;
        default: throw Error(`Reference budget executes beyond the loop endpoint at ${offset}`);
      }
    }
    return { words, images };
  };
  const seedPixelLoop = (words) => {
    seedScalarSseMove(words); words[59] = 0x37f; words[60] = 0x3a20;
    putXmm(words, 0, [0x00010000, 0x010000ff, 0x01ff0101, 0xffff7fff]); putXmm(words, 1, [0, 0, 0, 0]);
  };
  const checkPixelLoop = (result, expected, bytes, retired) => {
    equal(result.retired, retired, "whole-loop exact completed work");
    for (let n = 0; n < 100; n++) equal(result.words[n], expected.words[n], `whole-loop architectural word ${n}`);
    for (const [address, image] of expected.images) for (let n = 0; n < image.length; n++) equal(bytes(address)[n], image[n], "whole-loop compares every mapped input/output byte");
  };
  await test("real packed-pixel loop executes ordinary decoded code with exact results at every operation boundary", () => fixture(
    pixelLoopCode, async ({ reset, state, run, bytes }) => {
      for (const count of [1, 3, 9]) for (const budget of [...Array.from({ length: 12 }, (_, n) => n), ...(count > 1 ? [count * 11] : [])]) {
        for (let n = 0; n < 4096; n++) { bytes(DATA)[n] = n * 37 + 19 & 255; bytes(STACK)[n] = 0x55; }
        reset({ eax: STACK + 7, edx: (DATA + 5 - (STACK + 7)) >>> 0, esi: count, ecx: 0x87654321, flags: 0xad7 }); seedPixelLoop(state());
        const expected = pixelLoopReference(state(), new Map([[DATA, bytes(DATA)], [STACK, bytes(STACK)]]), budget);
        checkPixelLoop(run(budget), expected, bytes, budget);
      }
    }));
  await test("pixel-loop next-iteration page fault preserves completed output and resumes from the faulting load", () => fixture(
    pixelLoopCode, async ({ reset, state, run, bytes, map, compile }) => {
      for (let n = 0; n < 4096; n++) bytes(DATA)[n] = n * 37 + 19 & 255; bytes(STACK).fill(0x55);
      reset({ eax: STACK + 8, edx: (DATA + 4088 - (STACK + 8)) >>> 0, esi: 2, flags: 0xad7 }); seedPixelLoop(state());
      const firstExpected = pixelLoopReference(state(), new Map([[DATA, bytes(DATA)], [STACK, bytes(STACK)]]), 11), fault = run(100);
      equal(fault.kind, 4, "second iteration missing input faults"); equal(fault.address, DATA + 4096, "next input page"); equal(fault.error, 4, "user read fault"); checkPixelLoop(fault, firstExpected, bytes, 11);
      const nextPage = page(Array.from({ length: 8 }, (_, n) => n * 29 + 7)); map(DATA + 4096, READ, nextPage); await compile(CODE);
      const repairedExpected = pixelLoopReference(state(), new Map([[DATA, bytes(DATA)], [DATA + 4096, bytes(DATA + 4096)], [STACK, bytes(STACK)]]), 11);
      checkPixelLoop(run(11), repairedExpected, bytes, 11);
    }));
  await test("pixel-loop executable-alias output commits once before SMC and resumes at its following ADD", () => fixture(
    pixelLoopCode, async ({ handle, alias, reset, state, run, bytes, compile }) => {
      alias(0x4000, CODE, READ | WRITE); for (let n = 0; n < 4096; n++) bytes(DATA)[n] = n * 37 + 19 & 255;
      reset({ eax: 0x4100, edx: (DATA - 0x4100) >>> 0, esi: 1, flags: 0xad7 }); seedPixelLoop(state());
      const before = new Map([[DATA, bytes(DATA)], [0x4000, bytes(0x4000)]]), expected = pixelLoopReference(state(), before, 8), stopped = run(100);
      equal(stopped.kind, 7, "whole-loop MMX store ends execution on executable alias"); checkPixelLoop(stopped, expected, bytes, 8); check(api.backend_validate(handle, CODE) !== 0, "modified code page invalidates unit");
      await compile(CODE + 32); const continuation = pixelLoopReference(state(), new Map([[DATA, bytes(DATA)], [0x4000, bytes(0x4000)]]), 3);
      checkPixelLoop(run(3), continuation, bytes, 3);
    }, READ | WRITE | EXECUTE | SEALED));
  if (options.linkedRegions) await test("pixel-loop links split units while preserving crossbank state and one shared budget", () => fixture(
    pixelLoopCode, async ({ handle, compile, select, reset, state, run, bytes }) => {
      ok(api.backend_cache_invalidate(handle), "discard initial single-unit loop for split-unit test"); await compile(CODE, 4); await compile(CODE + 16, 7); select(CODE);
      for (let n = 0; n < 4096; n++) bytes(DATA)[n] = n * 37 + 19 & 255; bytes(STACK).fill(0x55);
      reset({ eax: STACK + 8, edx: (DATA - (STACK + 8)) >>> 0, esi: 9, flags: 0xad7 }); seedPixelLoop(state());
      const expected = pixelLoopReference(state(), new Map([[DATA, bytes(DATA)], [STACK, bytes(STACK)]]), 99); checkPixelLoop(run(99), expected, bytes, 99);
    }));
  await test("MOVD GPR output invalidates any preceding affine address fact before later reads", () => fixture(
    [0x8b, 0x03, 0x8b, 0x53, 4, 0x66, 0x0f, 0x7e, 0xcb, 0x8b, 0x3b, 0x8b, 0x73, 4], async ({ reset, state, run, bytes, affineRanges }) => {
      setWord(bytes(DATA), 0, 0x11111111); setWord(bytes(DATA), 4, 0x22222222); setWord(bytes(STACK), 0, 0x33333333); setWord(bytes(STACK), 4, 0x44444444);
      reset({ ebx: DATA, flags: 0xad7 }); seedScalarSseMove(state()); putXmm(state(), 1, [STACK, 0xfff00000, 0x7f800001, 0xffffffff]); const before = Array.from(state());
      if (options.affineMemory) check(affineRanges() > 0, "case applies an affine certificate before GPR replacement");
      checkScalarSseMove(run(5), before, { 0: 0x11111111, 2: 0x22222222, 3: STACK, 7: 0x33333333, 6: 0x44444444 }, CODE + 14, 5);
    }));
  await test("scalar MOVSD and REP MOVSD keep distinct raw-SSE and string execution semantics", () => fixture(
    [0xf2, 0x0f, 0x10, 0x03, 0xf3, 0xa5, 0xf2, 0x0f, 0x11, 0x02], async ({ reset, state, run, bytes }) => {
      const raw = [0x7f800001, 0xfff00000]; raw.forEach((value, n) => setWord(bytes(DATA), n * 4, value));
      reset({ ebx: DATA, esi: DATA, edi: STACK, edx: STACK + 16, ecx: 2, flags: 0xad7 }); seedScalarSseMove(state()); const before = Array.from(state());
      checkScalarSseMove(run(3), before, { 1: 0, 6: DATA + 8, 7: STACK + 8, 67: raw[0], 68: raw[1], 69: 0, 70: 0 }, CODE + 10, 3);
      for (const offset of [0, 16]) for (let n = 0; n < 2; n++) equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(offset + n * 4, true), raw[n], "string and scalar copy retain exact raw words");
    }));
  for (const storeEncoding of [false,true]) await test(`MOVAPS register ${storeEncoding?"store":"load"} encoding copies raw NaNs and aliases`,async()=>{
    for(const destination of [0,1,7])for(const source of [0,1,7])await fixture(
      [0x0f,storeEncoding?0x29:0x28,0xc0|((storeEncoding?source:destination)<<3)|(storeEncoding?destination:source)],async({reset,state,run})=>{
        reset({flags:0xad7});for(let n=35;n<100;n++)state()[n]=(n*0x1234567)>>>0;
        state()[59]=0x37e;state()[60]=0x80c1;state()[99]=0x7f81;
        for(let r=0;r<8;r++)putXmm(state(),r,[0x7f800001+r,0xff800000,0x80000000,(0x7fc01234+r)>>>0]);
        const before=Array.from(state()),result=run(1);equal(result.retired,1,"raw register move completes despite pending x87 state");
        for(let n=35;n<100;n++){const at=67+destination*4,expected=n>=at&&n<at+4?before[67+source*4+n-at]:before[n];equal(result.words[n],expected,"all raw vector and unrelated FP words preserved");}
        equal(result.words[9],0xad7,"move preserves integer flags");
      });
  });
  const alignedSseMoves=[{name:"MOVAPS load",opcode:0x28,write:false},{name:"MOVAPS store",opcode:0x29,write:true},{name:"MOVNTPS store",opcode:0x2b,write:true}];
  for(const operation of alignedSseMoves){
    await test(`${operation.name} linear alignment and full-width protection precede any mutation`,()=>fixture(
      [0x0f,operation.opcode,0x03],async({reset,state,run,bytes,protect})=>{
        const vector=[0x7f800001,0xff800000,0x80000000,0x7fc01234],vectorBytes=new Uint8Array(new Uint32Array(vector).buffer);
        for(const[address,permissions,kind,errorCode]of[
          [DATA+4080,READ|WRITE,1,0],[DATA+1,READ|WRITE,6,0],[DATA+4095,READ|WRITE,6,0],
          [0x5001,READ|WRITE,6,0],[0x5000,READ|WRITE,4,operation.write?6:4],[DATA,operation.write?READ:WRITE,4,operation.write?7:5],
        ]){
          protect(DATA,READ|WRITE);bytes(DATA).fill(0x55);reset({ebx:address,flags:0xad7});putXmm(state(),0,vector);
          state()[59]=0x37e;state()[60]=0x80c1;state()[99]=0x7f81;const before=Array.from(state());protect(DATA,permissions);
          const result=run(1);equal(result.kind,kind,"aligned access or exact GP/PF");equal(result.retired,kind===1?1:0,"only completed operation retires");equal(result.pc,kind===1?CODE+3:CODE,"precise vector PC");
          if(kind===4){equal(result.error,errorCode,"user access/protection error bits");equal(result.address,address,"aligned failing address");}else if(kind===6){equal(result.error,0,"alignment GP has zero error code");}
          for(let n=35;n<100;n++)equal(result.words[n],!operation.write&&kind===1&&n>=67&&n<71?0x55555555:before[n],"destination and complete FP/vector state atomic");equal(result.words[9],0xad7,"move retains flags");
          for(let n=0;n<4096;n++)equal(bytes(DATA)[n],operation.write&&kind===1&&n>=4080?vectorBytes[n-4080]:0x55,"all sixteen bytes or no output");
        }
      }));
    await test(`${operation.name} repairs protected or absent memory without replaying prior scalar effects`,()=>fixture(
      [0x83,0x07,1,0x0f,operation.opcode,0x03],async({map,protect,bytes,reset,state,run,compile})=>{
        setWord(bytes(STACK),0,0x7fffffff);if(operation.write)map(0x5000,READ,page([],0x77));
        const vector=[0x7f800001,0xff800000,0x80000000,0x7fc01234];reset({ebx:0x5000,edi:STACK,flags:0x203});putXmm(state(),0,vector);
        state()[59]=0x37e;state()[60]=0x80c1;state()[99]=0x7f81;const before=Array.from(state()),fault=run(100);
        equal(fault.kind,4,"aligned operand produces precise PF");equal(fault.pc,CODE+3,"vector instruction remains unretired");equal(fault.retired,1,"prior memory ADD alone completes");equal(fault.address,0x5000,"missing/protected vector address");equal(fault.error,operation.write?7:4,"fault error distinguishes protection and absence");
        equal(fault.words[9]&ARITH,arithmetic("add",32,0x7fffffff,1,0).flags,"earlier flags survive vector fault");equal(fault.words[10]&ARITH,ARITH,"fault preserves defined flags");for(let n=35;n<100;n++)equal(fault.words[n],before[n],"fault cannot mutate x87/XMM/MXCSR");
        if(operation.write){for(let n=0;n<16;n++)equal(bytes(0x5000)[n],0x77,"failed store is atomic");protect(0x5000,READ|WRITE);}else map(0x5000,READ,page(Array.from(new Uint8Array(new Uint32Array(vector).buffer))));
        await compile(CODE+3);const repaired=run(1);equal(repaired.retired,1,"only repaired vector retires");equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset).getUint32(0,true),0x80000000,"prior store never repeats");
        for(let n=0;n<4;n++)equal(operation.write?new DataView(bytes(0x5000).buffer,bytes(0x5000).byteOffset).getUint32(n*4,true):repaired.words[67+n],vector[n],"repaired full raw transfer");
      }));
    await test(`${operation.name} uses FS linear alignment while preserving pre-base segment limits`,()=>fixture(
      [0x64,0x0f,operation.opcode,0x03],async({reset,state,run,bytes})=>{
        const vector=[0x7f800001,0xff800000,0x80000000,0x7fc01234];
        for(const[base,offset,kind]of[[1,DATA+15,1],[15,DATA+1,1],[1,DATA,6],[1,0xffffffff,6]]){
          bytes(DATA).fill(0x5a);reset({ebx:offset,flags:0xad7});state()[28]=base;putXmm(state(),0,vector);state()[59]=0x37e;state()[60]=0x80c1;state()[99]=0x7f81;const before=Array.from(state()),result=run(1);
          equal(result.kind,kind,"FS offset proof then linear alignment");equal(result.retired,kind===1?1:0,"FS vector retirement");equal(result.pc,kind===1?CODE+4:CODE,"FS exact continuation");
          for(let n=35;n<100;n++)equal(result.words[n],kind===1&&!operation.write&&n>=67&&n<71?0x5a5a5a5a:before[n],"FS FP/vector state unchanged except successful load");
          const expectedBytes=new Uint8Array(new Uint32Array(vector).buffer);for(let n=0;n<4096;n++)equal(bytes(DATA)[n],kind===1&&operation.write&&n>=16&&n<32?expectedBytes[n-16]:0x5a,"FS exact bytes or atomic GP");
        }
      }));
  }
  for(const operation of alignedSseMoves.filter(operation=>operation.write))await test(`${operation.name} executable alias commits once and invalidates before next instruction`,()=>fixture(
    [0x0f,operation.opcode,0x03,0x40],async({handle,alias,reset,state,run,bytes,compile})=>{
      alias(0x4000,CODE,READ|WRITE);const vector=[0x7f800001,0xff800000,0x80000000,0x7fc01234];reset({eax:9,ebx:0x4100,flags:0xad7});putXmm(state(),0,vector);
      const result=run(100);equal(result.kind,7,"complete store exits for SMC");equal(result.retired,1,"only full vector store retired");equal(result.pc,CODE+3,"continuation is after store");equal(result.words[0],9,"following INC not executed yet");equal(result.words[9],0xad7,"store retains flags");
      for(let n=0;n<4;n++)equal(new DataView(bytes(CODE).buffer,bytes(CODE).byteOffset).getUint32(0x100+n*4,true),vector[n],"executable alias receives all raw bytes");check(api.backend_validate(handle,CODE)!==0,"stale unit rejected after code change");
      await compile(CODE+3);putXmm(state(),0,[0,0,0,0]);const resumed=run(1);equal(resumed.retired,1,"only following INC resumes");equal(resumed.words[0],10,"integer executes once after SMC");
      for(let n=0;n<4;n++)equal(new DataView(bytes(CODE).buffer,bytes(CODE).byteOffset).getUint32(0x100+n*4,true),vector[n],"store was not replayed with new source vector");
    },READ|WRITE|EXECUTE|SEALED));
  await test("MOVAPS to MOVNTPS copy uses wrapped base-plus-index address and preserves raw FP bytes",()=>fixture(
    [0x0f,0x28,0x00,0x0f,0x2b,0x04,0x02],async({map,bytes,reset,state,run})=>{
      const vector=[0x7f800001,0xff800000,0x80000000,0x7fc01234],input=page([],0x55);input.set(new Uint8Array(new Uint32Array(vector).buffer),4080);map(0xfffff000,READ,input);
      reset({eax:0xfffffff0,edx:0x2010,flags:0xad7});state()[59]=0x37e;state()[60]=0x80c1;state()[99]=0x7f81;const before=Array.from(state()),result=run(2);
      equal(result.retired,2,"aligned source and wrapped destination copy complete");equal(result.words[0],0xfffffff0,"source GPR retained");equal(result.words[2],0x2010,"destination displacement retained");
      for(let n=0;n<4;n++){equal(result.words[67+n],vector[n],"full raw load");equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(n*4,true),vector[n],"EA wraps to valid low guest address");}
      for(let n=35;n<100;n++)if(n<67||n>=71)equal(result.words[n],before[n],"copy preserves all other FP/vector state");equal(result.words[9],0xad7,"copy preserves flags");
    }));
  await test("aligned vector copy preserves shared execution-mirror bytes across virtual aliases",()=>fixture(
    [0x0f,0x28,0x03,0x0f,0x2b,0x02],async({alias,bytes,reset,state,run})=>{
      alias(0x4000,DATA,READ|WRITE);const vector=[0x7f800001,0xff800000,0x80000000,0x7fc01234];bytes(DATA).set(new Uint8Array(new Uint32Array(vector).buffer),16);
      reset({ebx:DATA+16,edx:0x4020,flags:0xad7});const result=run(2);equal(result.retired,2,"ordinary alias copy completes");
      for(let n=0;n<4;n++){equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(32+n*4,true),vector[n],"canonical virtual page sees alias write");equal(new DataView(bytes(0x4000).buffer,bytes(0x4000).byteOffset).getUint32(32+n*4,true),vector[n],"alias sees same committed bytes");}
      equal(result.words[9],0xad7,"alias copy keeps flags");
    }));
  for(const operation of alignedSseMoves)await test(`LOCK ${operation.name} faults before operand checks`,()=>fixture(
    [0xf0,0x0f,operation.opcode,0x03],async({reset,state,run})=>{
      reset({ebx:0x5001,flags:0xad7});putXmm(state(),0,[0x7f800001,0xffffffff,0x80000000,0x7fc01234]);const before=Array.from(state()),result=run(1);
      equal(result.kind,8,"LOCK vector instruction is UD before GP/PF");equal(result.retired,0,"invalid move does not retire");for(let n=0;n<100;n++)equal(result.words[n],before[n],"invalid move preserves all architectural state");
    }));
  await test("MOVNTPS register destination is invalid rather than a vector-register copy",()=>fixture(
    [0x0f,0x2b,0xc1],async({reset,state,run})=>{reset();const before=Array.from(state()),result=run(1);equal(result.kind,8,"memory-only store rejects ModRM register mode");equal(result.retired,0,"invalid destination is unretired");for(let n=0;n<100;n++)equal(result.words[n],before[n],"invalid store has no CPU effects");}));
  for (const storeEncoding of [false, true]) {
    await test(`MOVDQA register ${storeEncoding ? "store" : "load"} encoding copies full XMM only`, () => fixture(
      [0x66, 0x0f, storeEncoding ? 0x7f : 0x6f, 0xc1], async ({ reset, state, run }) => {
        reset({ flags: 0xad7 });
        for (let n = 35; n < 100; n++) state()[n] = (0x9e3779b9 * n) >>> 0;
        state()[59] = 0x37e; state()[60] = 0x80c1; state()[99] = 0x1f81;
        const before = Array.from(state()), result = run(1), destination = storeEncoding ? 71 : 67;
        equal(result.retired, 1, "full vector register copy completes");
        for (let n = 35; n < 100; n++) {
          const expected = n >= destination && n < destination + 4 ? before[n + (storeEncoding ? -4 : 4)] : before[n];
          equal(result.words[n], expected, "raw vector copy and unrelated FP/vector preservation");
        }
        equal(result.words[9], 0xad7, "MOVDQA preserves integer flags");
      }));
  }
  for (const write of [false, true]) {
    await test(`MOVDQA ${write ? "store" : "load"} checks alignment and permission before effects`, () => fixture(
      [0x66, 0x0f, write ? 0x7f : 0x6f, 0x03], async ({ reset, state, run, bytes, protect }) => {
        const vector = [0x12345678, 0x87654321, 0x7f800001, 0xffffffff];
        for (const [address, flags, kind, errorCode] of [
          [DATA + 4080, READ | WRITE, 1, 0],
          [DATA + 1, READ | WRITE, 6, 0],
          [0x5001, READ | WRITE, 6, 0],
          [0x5000, READ | WRITE, 4, write ? 6 : 4],
          [DATA, write ? READ : WRITE, 4, write ? 7 : 5],
        ]) {
          protect(DATA, READ | WRITE); bytes(DATA).fill(0x55);
          reset({ ebx: address, flags: 0xad7 }); putXmm(state(), 0, vector);
          state()[59] = 0x37e; state()[60] = 0x80c1; state()[99] = 0x1f81;
          const before = Array.from(state()); protect(DATA, flags);
          const result = run(1);
          equal(result.kind, kind, "aligned vector access or precise fault");
          equal(result.retired, kind === 1 ? 1 : 0, "only completed memory operation retires");
          equal(result.pc, kind === 1 ? CODE + 4 : CODE, "exact vector resume position");
          if (kind === 4) equal(result.error, errorCode, "user page fault access bits");
          for (let n = 35; n < 100; n++) {
            const expected = !write && kind === 1 && n >= 67 && n < 71 ? 0x55555555 : before[n];
            equal(result.words[n], expected, "destination and full FP/vector state are atomic");
          }
          equal(result.words[9], 0xad7, "vector memory access preserves EFLAGS");
          const expectedBytes = new Uint8Array(new Uint32Array(vector).buffer);
          const actualBytes = bytes(DATA);
          for (let n = 0; n < 4096; n++) {
            const expected = write && kind === 1 && n >= 4080 ? expectedBytes[n - 4080] : 0x55;
            equal(actualBytes[n], expected, "exact memory output or no partial mutation");
          }
        }
      }));
  }
  await test("MOVDQA code-alias store completes once and exits before the next instruction", () => fixture(
    [0x66, 0x0f, 0x7f, 0x03, 0x40], async ({ handle, alias, reset, state, run, bytes }) => {
      alias(0x4000, CODE, READ | WRITE);
      const vector = [0x12345678, 0x87654321, 0x7f800001, 0xffffffff];
      reset({ eax: 9, ebx: 0x4100, flags: 0xad7 }); putXmm(state(), 0, vector);
      const result = run(100);
      equal(result.kind, 7, "aligned executable alias write requests SMC exit");
      equal(result.retired, 1, "only the complete store retires");
      equal(result.pc, CODE + 4, "resume follows store"); equal(result.words[0], 9, "following INC did not execute");
      equal(result.words[9], 0xad7, "store preserved flags");
      const actual = new DataView(bytes(CODE).buffer, bytes(CODE).byteOffset);
      for (let n = 0; n < 4; n++) equal(actual.getUint32(0x100 + n * 4, true), vector[n], "all sixteen alias bytes committed");
      check(api.backend_validate(handle, CODE) !== 0, "old code admission fails after aligned alias store");
    }, READ | WRITE | EXECUTE | SEALED));
  await test("MOVUPS alias code store completes sixteen bytes then invalidates before next instruction", () => fixture(
    [0x0f, 0x11, 0x03, 0x40], async ({ handle, alias, state, reset, run, bytes }) => {
      alias(0x4000, CODE, READ | WRITE);
      reset({ ebx: 0x4100 }); putXmm(state(), 0, [0x12345678, 0x87654321, 0xffffffff, 0x7f800001]);
      const result = run(100);
      equal(result.kind, 7, "vector alias write exits for SMC");
      equal(result.retired, 1, "vector code store retires once");
      equal(result.pc, CODE + 3, "continuation follows whole vector store");
      equal(result.words[0], 0, "next instruction has not executed");
      const written = new DataView(bytes(CODE).buffer, bytes(CODE).byteOffset + 0x100, 16);
      for (const [n, value] of [0x12345678, 0x87654321, 0xffffffff, 0x7f800001].entries())
        equal(written.getUint32(n * 4, true), value, "all alias output words committed");
      check(api.backend_validate(handle, CODE) !== 0, "alias code dependency is invalidated");
    }, READ | WRITE | EXECUTE | SEALED));
  await test("MOVUPS two-page dirty capacity is checked before any output", () => fixture(
    [0x0f, 0x11, 0x03], async ({ handle, map, state, reset, run, bytes }) => {
      map(DATA + 4096, READ | WRITE);
      reset({ ebx: DATA + 4090, flags: 0xa93 }); putXmm(state(), 0, [0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff]);
      const header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4);
      const capacity = header()[2]; header()[2] = 1;
      const result = run(1, () => { header()[2] = capacity; });
      equal(result.kind, 10, "two dirty pages require capacity before store");
      equal(result.retired, 0, "resource-failed vector store does not retire");
      equal(result.words[9], 0xa93, "resource failure preserves flags");
      for (let n = 0; n < 16; n += 1) {
        const at = DATA + 4090 + n; equal(bytes(at & ~4095)[at & 4095], 0, "resource failure leaves both pages unchanged");
      }
    }));
  const requireAffine = (affineRanges) => {
    if (options.affineMemory) check(affineRanges() > 0, "case exercises selected affine range");
  };
  await test("affine range survives direct backedges with identical guest work", () => fixture(
    [0x8b, 0x03, 0x8b, 0x53, 4, 0x49, 0x75, 0xf8], async ({ reset, run, bytes, affineRanges }) => {
      requireAffine(affineRanges);
      setWord(bytes(DATA), 64, 0x12345678); setWord(bytes(DATA), 68, 0xabcdef01);
      reset({ ebx: DATA + 64, ecx: 3 });
      const result = run(12);
      equal(result.retired, 12, "three complete iterations"); equal(result.words[0], 0x12345678, "first load");
      equal(result.words[2], 0xabcdef01, "second load"); equal(result.words[1], 0, "loop completed");
    }));
  await test("affine partial register and conditional writes invalidate old base", () => fixture(
    [0x8b, 0x03, 0x66, 0xbb, 0x80, 0x20, 0x8b, 0x4b, 4, 0x0f, 0x45, 0xda, 0x8b, 0x73, 4],
    async ({ reset, run, bytes, affineRanges }) => {
      requireAffine(affineRanges);
      setWord(bytes(DATA), 64, 11); setWord(bytes(DATA), 132, 22); setWord(bytes(DATA), 196, 33);
      for (const taken of [false, true]) {
        reset({ ebx: DATA + 64, edx: DATA + 192, flags: taken ? 0x202 : 0x242 });
        const result = run(5);
        equal(result.words[0], 11, "old base read"); equal(result.words[1], 22, "partial BX write changes address");
        equal(result.words[6], taken ? 33 : 22, "CMOV follows actual selected base");
      }
    }));
  await test("affine division result invalidates implicit EAX base", () => fixture(
    [0x8b, 0x30, 0xf7, 0xf1, 0x8b, 0x78, 4], async ({ reset, run, bytes, affineRanges }) => {
      requireAffine(affineRanges); setWord(bytes(DATA), 128, 11);
      reset({ eax: DATA + 128, ecx: 2 }); const result = run(3);
      equal(result.words[6], 11, "original EAX memory"); equal(result.words[0], (DATA + 128) / 2, "quotient");
      equal(result.words[7], 0xcccccccc, "new quotient addresses code-page data");
    }));
  await test("affine implicit stack mutation invalidates ESP base", () => fixture(
    [0x8b, 0x44, 0x24, 8, 0x6a, 0, 0x8b, 0x4c, 0x24, 12, 0x8b, 0x54, 0x24, 8],
    async ({ reset, run, bytes, affineRanges }) => {
      requireAffine(affineRanges); setWord(bytes(STACK), 0x804, 21); setWord(bytes(STACK), 0x808, 22);
      reset(); const result = run(4);
      equal(result.words[0], 22, "before PUSH"); equal(result.words[1], 22, "advanced stack plus12");
      equal(result.words[2], 21, "advanced stack plus8"); equal(result.words[4], STACK + 0x7fc, "guest stack effect retained");
    }));
  await test("affine broad range miss never pre-faults an unreached address", () => fixture(
    [0x8b, 0x03, 0x8b, 0x4b, 4], async ({ reset, run, bytes, affineRanges }) => {
      requireAffine(affineRanges); setWord(bytes(DATA), 4092, 77); reset({ ebx: DATA + 4092 });
      const result = run(2); equal(result.kind, 4, "second access faults"); equal(result.retired, 1, "prefix read retired once");
      equal(result.pc, CODE + 2, "exact failing instruction"); equal(result.words[0], 77, "prefix result retained");
      equal(result.address, DATA + 4096, "unmapped exact address");
    }));
  await test("affine combined permissions do not pre-fault valid read", () => fixture(
    [0x8b, 0x03, 0x89, 0x4b, 4], async ({ reset, run, bytes, protect, affineRanges }) => {
      requireAffine(affineRanges); setWord(bytes(DATA), 0, 77); protect(DATA, READ); reset({ ebx: DATA, ecx: 99 });
      const result = run(2); equal(result.kind, 4, "write protection fault"); equal(result.retired, 1, "read completed before write fault");
      equal(result.words[0], 77, "read-only value observed"); equal(result.address, DATA + 4, "exact write fault");
      equal(bytes(DATA)[4], 0, "failed store has no side effect");
    }));
  await test("affine proof discarded after exit and mapping protection change", () => fixture(
    [0x8b, 0x03, 0x8b, 0x4b, 4], async ({ reset, run, protect, compile, affineRanges }) => {
      requireAffine(affineRanges); reset({ ebx: DATA }); equal(run(1).retired, 1, "first invocation read");
      protect(DATA, 0); await compile(CODE + 2);
      const result = run(1); equal(result.kind, 4, "new invocation checks new protection");
      equal(result.retired, 0, "faulting resume has no retirement"); equal(result.address, DATA + 4, "resumed access address");
    }));
  await test("affine certified store retains alias code invalidation", () => fixture(
    [0x8b, 0x03, 0x89, 0x4b, 4, 0x42], async ({ reset, run, bytes, alias, affineRanges }) => {
      requireAffine(affineRanges); alias(0x4000, CODE, READ | WRITE); reset({ ebx: 0x4100, ecx: 0x12345678 });
      const result = run(3); equal(result.kind, 7, "alias-backed executable write exits"); equal(result.retired, 2, "load and store retire");
      equal(result.words[2], 0, "following INC is not executed");
      equal(new DataView(bytes(CODE).buffer, bytes(CODE).byteOffset).getUint32(0x104, true), 0x12345678, "whole store committed");
    }, READ | WRITE | EXECUTE | SEALED));
  await test("affine certified store keeps dirty capacity preflight", () => fixture(
    [0x8b, 0x03, 0x89, 0x4b, 4], async ({ handle, reset, run, bytes, affineRanges }) => {
      requireAffine(affineRanges); reset({ ebx: DATA, ecx: 0xffffffff });
      const header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4);
      const capacity = header()[2]; header()[2] = 0;
      const result = run(2, () => { header()[2] = capacity; });
      equal(result.kind, 10, "capacity fault survives proof"); equal(result.retired, 1, "only prefix read completed");
      equal(result.address, DATA + 4, "capacity fault precise address"); equal(bytes(DATA)[4], 0, "store not published");
    }));
  await test("affine broad span wrap falls back to legal wrapping address arithmetic", () => fixture(
    [0x8b, 0x03, 0x8b, 0x4b, 4], async ({ reset, run, map, bytes, affineRanges }) => {
      requireAffine(affineRanges); map(0xfffff000, READ | WRITE); map(0, READ | WRITE);
      setWord(bytes(0xfffff000), 4092, 55); setWord(bytes(0), 0, 66); reset({ ebx: 0xfffffffc });
      const result = run(2); equal(result.retired, 2, "both individual spans legal");
      equal(result.words[0], 55, "last-page value"); equal(result.words[1], 66, "wrapped effective address value");
    }));
  const scasCode = (width, repeat = 0, prefix = []) => [...prefix, ...(repeat ? [repeat] : []), ...(width === 2 ? [0x66] : []), width === 1 ? 0xae : 0xaf];
  const seedScasState = words => {
    for(let n=35;n<100;n++)words[n]=(n*0x45d9f3b)>>>0;
    for(let n=0;n<8;n++)words[37+n*3]&=0xffff;
    words[59]=0x37e;words[60]=0xb881;words[61]=0x3fff;words[62]&=0x7ff;words[64]&=0xffff;words[66]&=0xffff;words[99]=0x7f81;
    words[10]=0xfffff72a;
  };
  const checkScasWords = (actual,before,{pc,eax=before[0],ecx=before[1],esi=before[6],edi=before[7],flags=before[9],defined=before[10]}) => {
    for(let n=0;n<100;n++)equal(actual[n],n===0?eax:n===1?ecx:n===6?esi:n===7?edi:n===8?pc:n===9?flags:n===10?defined:before[n],"comparison preserves every unrelated architectural word including pending raw80/MMX/XMM/MXCSR");
  };
  const writeScasElement = (data,offset,width,value) => {for(let n=0;n<width;n++)data[offset+n]=Number(BigInt(value)>>BigInt(8*n)&255n);};
  const cmpsCode = (width, repeat = 0, prefix = []) => [...prefix, ...(repeat ? [repeat] : []), ...(width === 2 ? [0x66] : []), width === 1 ? 0xa6 : 0xa7];
  for (const width of [1, 2, 4]) for (const backward of [false, true]) await test(`CMPS${width} single comparison arithmetic and DF=${Number(backward)}`, () => fixture(
    cmpsCode(width), async ({ bytes, reset, state, run }) => {
      const bits = width * 8, mask = Number(widthMask(bits)), sign = 2 ** (bits - 1);
      for (const [left, right] of [[0, 0], [0, 1], [mask, 1], [sign, 1], [sign - 1, mask], [15, 16]]) for (const ecx of [0, 9]) {
        writeScasElement(bytes(DATA), 129, width, left); writeScasElement(bytes(STACK), 65, width, right);
        reset({ eax: 0xdeadbeef, ecx, esi: DATA + 129, edi: STACK + 65, flags: 0xad7 | (backward ? 0x400 : 0) }); seedScasState(state());
        const before = Array.from(state()), zero = run(0); equal(zero.retired, 0, "zero budget does not read or compare"); checkScasWords(zero.words, before, { pc: CODE });
        const result = run(1), ref = arithmetic('sub', bits, left, right, 0), step = backward ? -width : width;
        equal(result.retired, 1, "single compare ignores ECX and pending x87 state");
        checkScasWords(result.words, before, { pc: CODE + cmpsCode(width).length, esi: DATA + 129 + step, edi: STACK + 65 + step, flags: (before[9] & ~ARITH) | ref.flags, defined: (before[10] | ARITH) >>> 0 });
      }
    }));
  for (const width of [1, 2, 4]) for (const repeat of [0xf2, 0xf3]) for (const backward of [false, true]) await test(`CMPS${width} repeat${repeat.toString(16)} compares before testing ZF DF=${Number(backward)}`, () => fixture(
    cmpsCode(width, repeat), async ({ bytes, reset, state, run }) => {
      for (const initialZf of [0, ZF]) {
        const step = backward ? -width : width;
        for (let n = 0; n < 5; n++) {
          writeScasElement(bytes(DATA), 128 + n * step, width, 0x25);
          writeScasElement(bytes(STACK), 128 + n * step, width, (n === 2) === (repeat === 0xf2) ? 0x25 : 0x14);
        }
        reset({ eax: 0xfedcba98, ecx: 5, esi: DATA + 128, edi: STACK + 128, flags: (0xad7 & ~ZF) | initialZf | (backward ? 0x400 : 0) }); seedScasState(state());
        const before = Array.from(state()), source = bytes(DATA).slice(), destination = bytes(STACK).slice(), result = run(1);
        equal(result.retired, 1, "comparison condition ends after exactly three elements");
        const ref = arithmetic('sub', width * 8, 0x25, repeat === 0xf2 ? 0x25 : 0x14, 0);
        checkScasWords(result.words, before, { pc: CODE + cmpsCode(width, repeat).length, ecx: 2, esi: DATA + 128 + 3 * step, edi: STACK + 128 + 3 * step, flags: (before[9] & ~ARITH) | ref.flags, defined: (before[10] | ARITH) >>> 0 });
        for (let n = 0; n < 4096; n++) { equal(bytes(DATA)[n], source[n], "source unchanged"); equal(bytes(STACK)[n], destination[n], "destination unchanged"); }
        reset({ ecx: 0, esi: 0xffffffff, edi: 0xffffffff, flags: 0xed7 }); seedScasState(state());
        const empty = Array.from(state()), zero = run(1); equal(zero.retired, 1, "zero count never accesses either invalid span"); checkScasWords(zero.words, empty, { pc: CODE + cmpsCode(width, repeat).length });
      }
    }));
  for (const width of [1, 2, 4]) for (const repeat of [0xf2, 0xf3]) for (const side of ['source', 'destination']) for (const protectedPage of [false, true]) await test(`CMPS${width} repeat${repeat.toString(16)} ${side} ${protectedPage ? 'protected' : 'absent'} split fault and repair`, () => fixture(
    [0x83, 0x03, 1, ...cmpsCode(width, repeat)], async ({ bytes, reset, state, run, map, protect, compile }) => {
      const counter = 0x6000, start = 4096 - 2 * width + (width === 1 ? 0 : 1), completed = width === 1 ? 2 : 1;
      map(counter, READ | WRITE); setWord(bytes(counter), 0, 7);
      const source = DATA + (side === 'source' ? start : 128), destination = STACK + (side === 'destination' ? start : 128), missing = (side === 'source' ? DATA : STACK) + 4096;
      if (protectedPage) map(missing, WRITE);
      for (let n = 0; n < (side === 'source' ? completed : 3); n++) writeScasElement(bytes(DATA), source - DATA + n * width, width, 0x25);
      for (let n = 0; n < (side === 'destination' ? completed : 3); n++) writeScasElement(bytes(STACK), destination - STACK + n * width, width, repeat === 0xf3 ? 0x25 : 0x14);
      reset({ eax: 0x12345678, ecx: 3, ebx: counter, esi: source, edi: destination, flags: 0xad7 }); seedScasState(state());
      const before = Array.from(state()), fault = run(100), entryFlags = (before[9] & ~ARITH) | arithmetic('add', 32, 7, 1, 0).flags;
      equal(fault.kind, 4, "inaccessible full element read faults"); equal(fault.error, protectedPage ? 5 : 4, "user read permission code"); equal(fault.address, missing, "first inaccessible byte"); equal(fault.retired, 1, "only preceding store retires");
      checkScasWords(fault.words, before, { pc: CODE + 3, ecx: 3 - completed, esi: source + completed * width, edi: destination + completed * width, flags: entryFlags, defined: (before[10] | ARITH) >>> 0 });
      equal(new DataView(bytes(counter).buffer, bytes(counter).byteOffset).getUint32(0, true), 8, "prior write completed once");
      state()[9] = 0xa17; state()[10] = 0xfffff72a; await compile(CODE + 3);
      const again = run(1); equal(again.kind, 4, "unrepaired span faults without progress"); equal(again.words[9], 0xa17, "new attempt retains handler-provided flags"); equal(again.words[10], 0xfffff72a, "new attempt validity retained");
      if (protectedPage) protect(missing, READ | WRITE); else map(missing, READ | WRITE);
      for (let n = completed; n < 3; n++) for (let b = 0; b < width; b++) {
        const address = (side === 'source' ? source : destination) + n * width + b, value = side === 'source' || repeat === 0xf3 ? 0x25 : 0x14;
        bytes(address & 0xfffff000)[address & 4095] = value >>> (8 * b) & 255;
      }
      const repaired = run(1); equal(repaired.retired, 1, "remaining compare completes"); equal(repaired.words[1], 0, "only remaining elements consumed");
      equal(repaired.words[6], source + 3 * width, "source advances once per complete pair"); equal(repaired.words[7], destination + 3 * width, "destination advances once per complete pair");
      equal(new DataView(bytes(counter).buffer, bytes(counter).byteOffset).getUint32(0, true), 8, "pre-fault store never replays");
    }));
  for (const width of [1, 2, 4]) await test(`CMPS${width} FS applies only to source and both read aliases stay clean`, () => fixture(
    cmpsCode(width, 0xf3, [0x64]), async ({ handle, alias, bytes, reset, state, run }) => {
      alias(0x4000, DATA, READ); writeScasElement(bytes(DATA), 1, width, 0x25);
      reset({ ecx: 2, esi: 1, edi: 0x4001, fs: DATA, flags: 0xad7 }); seedScasState(state());
      const before = Array.from(state()), result = run(1, () => { equal(rawPageDirty(handle, DATA), 0, "canonical page has no writes"); equal(rawPageDirty(handle, 0x4000), 0, "alias has no writes"); });
      equal(result.retired, 1, "same underlying bytes compare twice"); checkScasWords(result.words, before, { pc: CODE + cmpsCode(width, 0xf3, [0x64]).length, ecx: 0, esi: 1 + 2 * width, edi: 0x4001 + 2 * width, flags: (before[9] & ~ARITH) | 0x44, defined: (before[10] | ARITH) >>> 0 });
    }));
  for (const width of [1, 2, 4]) for (const side of ['source', 'destination']) await test(`CMPS${width} backward ${side} fault retains complete pairs only`, () => fixture(
    cmpsCode(width, 0xf3), async ({ map, bytes, reset, state, run }) => {
      const sourcePage = 0x6000, destinationPage = 0xa000, start = 2 * width - 1;
      map(sourcePage, READ); map(destinationPage, READ);
      const source = sourcePage + (side === 'source' ? start : 128), destination = destinationPage + (side === 'destination' ? start : 128);
      for (let n = 0; n < 2; n++) { writeScasElement(bytes(sourcePage), source - sourcePage - n * width, width, 0x25); writeScasElement(bytes(destinationPage), destination - destinationPage - n * width, width, 0x25); }
      reset({ ecx: 5, esi: source, edi: destination, flags: 0xed7 }); seedScasState(state());
      const before = Array.from(state()), result = run(1); equal(result.kind, 4, "preceding page is absent"); equal(result.address, (side === 'source' ? sourcePage : destinationPage) - 1, "fault names first unavailable byte");
      checkScasWords(result.words, before, { pc: CODE, ecx: 3, esi: source - 2 * width, edi: destination - 2 * width });
    }));
  if (options.linkedRegions) await test("CMPS linked target shares REP fuel and preserves source progress through a later fault", () => fixture(
    [0xf3, 0xaa, 0xb9, ...wordBytes(2000), 0xbe, ...wordBytes(DATA + 3000), 0xbf, ...wordBytes(0x6000 + 3000), 0xff, 0xe2],
    async ({ map, bytes, reset, state, run, compile, saveSnapshot, restoreSnapshot }) => {
      const target = 0x5000; map(target, READ | EXECUTE | SEALED, page([0xf3, 0xa6, 0xcc], 0xcc)); map(0x6000, READ, page([], 2)); await compile(target); bytes(DATA).fill(2);
      reset({ eax: 2, ecx: 3000, edi: STACK, edx: target, flags: 0xad7 }); seedScasState(state()); const before = Array.from(state()), first = run(100);
      equal(first.kind, 12, "target consumes remaining shared REP quota"); equal(first.error, 1096, "only actual target elements consume its progress quota"); equal(first.retired, 5, "only source instructions retire"); equal(first.pc, target, "incomplete target is current");
      const saved = saveSnapshot(); restoreSnapshot(saved); await compile(target); const fault = run(1);
      equal(fault.kind, 4, "linked comparison resumes at absent source page"); equal(fault.retired, 0, "incomplete comparison remains unretired"); checkScasWords(fault.words, before, { pc: target, ecx: 904, esi: DATA + 4096, edi: 0x7000 });
      for (let n = 0; n < 4096; n++) equal(bytes(STACK)[n], n < 3000 ? 2 : 0, "source REP stores remain exactly once");
    }));
  for (const width of [2, 4]) for (const side of ['source', 'destination']) for (const ss of [false, true]) await test(`CMPS${width} ${side} span wrap ${ss ? 'SS' : 'DS'} faults before progress`, () => fixture(
    cmpsCode(width, 0xf3, ss ? [0x36] : []), async ({ reset, state, run }) => {
      reset({ ecx: 3, esi: side === 'source' ? 0xffffffff : DATA, edi: side === 'destination' ? 0xffffffff : STACK, flags: 0xad7 }); seedScasState(state());
      const before = Array.from(state()), result = run(1); equal(result.kind, side === 'source' && ss ? 9 : 6, "fault uses operand segment"); equal(result.retired, 0, "wrapping element does not complete"); checkScasWords(result.words, before, { pc: CODE });
    }));
  await test("CMPS byte indices wrap after a complete last-address comparison", () => fixture([0xa6], async ({ map, bytes, reset, state, run }) => {
    map(0xfffff000, READ); bytes(0xfffff000)[4095] = 0x25; reset({ ecx: 7, esi: 0xffffffff, edi: 0xffffffff, flags: 0xad7 }); seedScasState(state());
    const before = Array.from(state()), result = run(1); equal(result.retired, 1, "last byte is a valid whole operand"); checkScasWords(result.words, before, { pc: CODE + 1, esi: 0, edi: 0, flags: (before[9] & ~ARITH) | 0x44, defined: (before[10] | ARITH) >>> 0 });
  }));
  await test("CMPS suspended source and destination progress survive snapshots and a later fault", () => fixture([0xf3, 0xa6], async ({ handle, map, bytes, reset, state, run, compile, saveSnapshot, restoreSnapshot, snapshotBuffer }) => {
    map(DATA + 4096, READ); map(STACK + 4096, READ); for (const address of [DATA, DATA + 4096, STACK, STACK + 4096]) bytes(address).fill(1);
    reset({ eax: 0xdeadbeef, ecx: 9000, esi: DATA, edi: STACK, flags: 0xad7 }); seedScasState(state());
    const before = Array.from(state()), first = run(1); equal(first.kind, 12, "first bounded chunk yields"); equal(first.retired, 0, "partial instruction never retires"); equal(first.words[6], DATA + 4096, "source progress retained"); equal(first.words[7], STACK + 4096, "destination progress retained");
    const saved = saveSnapshot(), changed = saved.slice(); changed[24] ^= 1;
    snapshotBuffer((ptr, length) => { new Uint8Array(api.memory.buffer, ptr, length).set(changed); check(api.backend_restore_snapshot(handle, ptr, length) !== 0, "altered source progress is rejected"); });
    equal(Array.from(saveSnapshot()).join(), Array.from(saved).join(), "failed restore does not change live continuation");
    reset({ ecx: 1, esi: DATA, edi: STACK, flags: 0x202 }); equal(run(1).retired, 1, "another context completes independently"); restoreSnapshot(saved);
    equal(run(0).retired, 0, "zero-budget resume keeps original recipe"); restoreSnapshot(saveSnapshot()); await compile(CODE);
    const second = run(1); equal(second.kind, 12, "second chunk yields"); equal(second.words[1], 808, "both chunks count exactly once"); restoreSnapshot(saveSnapshot());
    const fault = run(1); equal(fault.kind, 4, "next source page is absent"); equal(fault.address, DATA + 8192, "exact next source byte"); checkScasWords(fault.words, before, { pc: CODE, ecx: 808, esi: DATA + 8192, edi: STACK + 8192 });
  }));
  for(const set of [false,true])for(const prefix of [[],[0x66],[0x64]])await test(`${set?'STD':'CLD'} defines only DF and ignores prefix${prefix.join('-')}`,()=>fixture(
    [...prefix,set?0xfd:0xfc],async({reset,state,run})=>{
      for(const flags of [0x202,0x602,0xad7,0xed7])for(const defined of [0,0xffffffff,0xfffff32a]){
        reset({eax:0x12345678,ecx:9,edx:0xdeadbeef,ebx:DATA,ebp:STACK+64,esi:DATA+17,edi:DATA+33,flags});seedScasState(state());state()[10]=defined;
        const before=Array.from(state()),zero=run(0);equal(zero.retired,0,"zero budget leaves direction unchanged");checkScasWords(zero.words,before,{pc:CODE});
        const result=run(1);equal(result.retired,1,"direction instruction completes once");checkScasWords(result.words,before,{pc:CODE+prefix.length+1,flags:set?flags|0x400:flags&~0x400,defined:(defined|0x400)>>>0});
      }
    }));
  for(const opcode of [0xfc,0xfd])await test(`LOCK direction flag opcode${opcode.toString(16)} is invalid without mutation`,()=>fixture(
    [0xf0,opcode],async({reset,state,run})=>{reset({flags:0xed7});seedScasState(state());state()[10]&=~0x400;const before=Array.from(state()),result=run(1);equal(result.kind,8,"LOCK CLD/STD is invalid opcode");equal(result.retired,0,"invalid direction operation does not retire");checkScasWords(result.words,before,{pc:CODE});}));
  for(const backward of [false,true])await test(`${backward?'STD':'CLD'} controls following REPNE SCAS direction without changing arithmetic inputs`,()=>fixture(
    [backward?0xfd:0xfc,0xf2,0xae],async({reset,state,run,bytes})=>{
      bytes(DATA).set([3,1,1,1,3]);reset({eax:3,ecx:5,edi:DATA+2,flags:0xad7|(backward?0:0x400)});seedScasState(state());state()[10]&=~0x400;const before=Array.from(state()),data=bytes(DATA).slice(),result=run(2);
      equal(result.retired,2,"direction and complete repeated comparison both retire");checkScasWords(result.words,before,{pc:CODE+3,ecx:2,edi:DATA+(backward?-1:5),flags:((before[9]&~(ARITH|0x400))|(backward?0x400:0)|0x44)>>>0,defined:(before[10]|ARITH|0x400)>>>0});for(let n=0;n<4096;n++)equal(bytes(DATA)[n],data[n],"direction change and scan do not write guest data");
    }));
  await test("STD direction and original entry flags survive repeated SCAS snapshot yields",()=>fixture(
    [0xfd,0xf2,0xae],async({map,bytes,reset,state,run,compile,saveSnapshot,restoreSnapshot})=>{
      map(0x6000,READ,page([],1));map(0x7000,READ,page([],1));reset({eax:2,ecx:9000,edi:0x7fff,flags:0xad7});seedScasState(state());state()[10]&=~0x400;const before=Array.from(state()),first=run(2);
      equal(first.kind,12,"backward repeated scan yields after first quota");equal(first.retired,1,"preceding STD retires once");equal(first.pc,CODE+1,"incomplete scan remains current");equal(first.words[7],0x6fff,"first4096 backwards comparisons completed");const saved=saveSnapshot();await compile(CODE+1);restoreSnapshot(saved);
      const second=run(1);equal(second.kind,12,"second backward quota yields");equal(second.words[7],0x5fff,"backward index includes both quotas");restoreSnapshot(saveSnapshot());const fault=run(1);equal(fault.kind,4,"next backward operand faults");equal(fault.address,0x5fff,"exact absent preceding byte");checkScasWords(fault.words,before,{pc:CODE+1,ecx:808,edi:0x5fff,flags:before[9]|0x400,defined:(before[10]|0x400)>>>0});
      for(let n=0;n<4096;n++){equal(bytes(0x6000)[n],1,"low scan page unchanged");equal(bytes(0x7000)[n],1,"high scan page unchanged");}
    }));
  for(const width of [1,2,4])for(const backward of [false,true])await test(`SCAS${width} exact subtraction flags and single-element DF=${Number(backward)}`,()=>fixture(
    scasCode(width),async({reset,state,run,bytes})=>{
      const bits=width*8,mask=Number(widthMask(bits)),sign=2**(bits-1);
      const pairs=[[0,0],[0,1],[1,0],[mask,1],[sign,1],[sign-1,mask],[16,1],[15,16],[mask,mask],[0,sign],[sign,sign-1]];
      for(const [accumulator,operand]of pairs)for(const ecx of [0,0x12345678]){
        bytes(DATA).fill(0x55);writeScasElement(bytes(DATA),129,width,operand);
        const eax=(0xa5a50000&~mask)|(accumulator&mask);reset({eax:eax>>>0,ecx,edi:DATA+129,esi:0x76543210,flags:0xad7|(backward?0x400:0)});seedScasState(state());
        const before=Array.from(state()),beforeMemory=bytes(DATA).slice(),zero=run(0);equal(zero.retired,0,"zero budget never performs a comparison");checkScasWords(zero.words,before,{pc:CODE});
        const result=run(1),ref=arithmetic('sub',bits,accumulator,operand,0);
        equal(result.retired,1,"single SCAS completes despite ECX zero or pending x87 exception");checkScasWords(result.words,before,{pc:CODE+scasCode(width).length,edi:(DATA+129+(backward?-width:width))>>>0,flags:(before[9]&~ARITH)|ref.flags,defined:(before[10]|ARITH)>>>0});
        for(let n=0;n<4096;n++)equal(bytes(DATA)[n],beforeMemory[n],"SCAS is read-only at every byte");
      }
    }));
  for(const width of [1,2,4])for(const repeat of [0xf2,0xf3])for(const backward of [false,true])await test(`${repeat===0xf2?'REPNE':'REPE'} SCAS${width} checks new comparison ZF and advances only completed elements DF=${Number(backward)}`,()=>fixture(
    scasCode(width,repeat),async({reset,state,run,bytes})=>{
      const equalValue=0x25,other=0x14,continueValue=repeat===0xf2?other:equalValue,stopValue=repeat===0xf2?equalValue:other;
      for(const initialZero of [false,true])for(const stopAfter of [1,3,7]){
        bytes(DATA).fill(0x55);const start=256+(backward?6*width:0),step=backward?-width:width;
        for(let n=0;n<7;n++)writeScasElement(bytes(DATA),start+n*step,width,n===stopAfter-1?stopValue:continueValue);
        reset({eax:0xa5a50000|equalValue,ecx:7,edi:DATA+start,flags:(0xa93&~ZF)|(initialZero?ZF:0)|(backward?0x400:0)});
        if(width===4)state()[0]=equalValue;seedScasState(state());const before=Array.from(state()),data=bytes(DATA).slice(),result=run(1),ref=arithmetic('sub',width*8,equalValue,stopValue,0);
        equal(result.retired,1,"one repeated instruction retires at condition exit");checkScasWords(result.words,before,{pc:CODE+scasCode(width,repeat).length,ecx:7-stopAfter,edi:(DATA+start+stopAfter*step)>>>0,flags:(before[9]&~ARITH)|ref.flags,defined:(before[10]|ARITH)>>>0});for(let n=0;n<4096;n++)equal(bytes(DATA)[n],data[n],"scanning never writes input");
      }
      bytes(DATA).fill(continueValue);for(let n=0;n<3;n++)writeScasElement(bytes(DATA),128+n*width,width,continueValue);reset({eax:equalValue,ecx:3,edi:DATA+(backward?128+2*width:128),flags:0x202|(backward?0x400:0)});seedScasState(state());const before=Array.from(state()),result=run(1),ref=arithmetic('sub',width*8,equalValue,continueValue,0);
      equal(result.retired,1,"count exhaustion completes an otherwise continuing scan");checkScasWords(result.words,before,{pc:CODE+scasCode(width,repeat).length,ecx:0,edi:DATA+(backward?128-width:128+3*width),flags:(before[9]&~ARITH)|ref.flags,defined:(before[10]|ARITH)>>>0});
    }));
  for(const width of [1,2,4])for(const repeat of [0xf2,0xf3])await test(`SCAS${width} repeat${repeat.toString(16)} zero ECX skips invalid spans and preserves undefined flags`,()=>fixture(
    scasCode(width,repeat),async({reset,state,run})=>{
      for(const edi of [0x5000,0xffffffff]){reset({eax:0x12345678,ecx:0,edi,flags:0xed7});seedScasState(state());const before=Array.from(state()),result=run(1);equal(result.retired,1,"empty repeated instruction completes without memory or pending-MF check");checkScasWords(result.words,before,{pc:CODE+scasCode(width,repeat).length});}
    }));
  for(const prefix of [0x64,0x2e,0x65])for(const width of [1,2,4])await test(`SCAS${width} ignores segment override ${prefix.toString(16)} for implicit ES`,()=>fixture(
    scasCode(width,0xf2,[prefix]),async({reset,state,run,bytes})=>{
      writeScasElement(bytes(DATA),1,width,0x17);reset({eax:0x17,ecx:2,edi:DATA+1,fs:0xffff0000,flags:0xad7});seedScasState(state());const before=Array.from(state()),result=run(1);equal(result.retired,1,"first ES value matches despite irrelevant FS base");checkScasWords(result.words,before,{pc:CODE+scasCode(width,0xf2,[prefix]).length,ecx:1,edi:DATA+1+width,flags:(before[9]&~ARITH)|0x44,defined:(before[10]|ARITH)>>>0});
    }));
  for(const width of [1,2,4])for(const repeat of [0,0xf2,0xf3])await test(`SCAS${width} address-size16 remains explicit unsupported repeat${repeat.toString(16)}`,()=>fixture(
    scasCode(width,repeat,[0x67]),async({reset,state,run})=>{reset({eax:0x12345678,ecx:0,edi:0xdeadffff,flags:0xad7});seedScasState(state());const before=Array.from(state()),result=run(1);equal(result.kind,3,"unimplemented16-bit addressing is explicitly unsupported");equal(result.retired,0,"unsupported instruction never completes");checkScasWords(result.words,before,{pc:CODE});}));
  for(const width of [1,2,4])for(const repeat of [0xf2,0xf3])for(const protectedPage of [false,true])await test(`SCAS${width} repeat${repeat.toString(16)} Intel fault restores entry flags and preserves preceding writes (${protectedPage?'protection':'absent'})`,()=>fixture(
    [0x83,0x03,1,...scasCode(width,repeat)],async({reset,state,run,bytes,map,protect,compile})=>{
      const start=4096-width*2+(width===1?0:1),accumulator=0x25,continueValue=repeat===0xf2?0x14:accumulator;
      bytes(DATA).fill(0x55);for(let n=0;n<(width===1?2:1);n++)writeScasElement(bytes(DATA),start+n*width,width,continueValue);if(protectedPage)map(DATA+4096,WRITE,page([],0x55));
      setWord(bytes(STACK),0,0x7fffffff);reset({eax:accumulator,ecx:3,ebx:STACK,edi:DATA+start,flags:0x203});seedScasState(state());const before=Array.from(state()),data=bytes(DATA).slice(),fault=run(100),entryFlags=(before[9]&~ARITH)|arithmetic('add',32,0x7fffffff,1,0).flags;
      const completed=width===1?2:1;equal(fault.kind,4,"next whole operand is inaccessible");equal(fault.error,protectedPage?5:4,"exact user-read fault code");equal(fault.address,DATA+4096,"first unavailable byte");equal(fault.pc,CODE+3,"repeat prefix is fault EIP");equal(fault.retired,1,"only preceding ADD retires");
      checkScasWords(fault.words,before,{pc:CODE+3,ecx:3-completed,edi:DATA+start+completed*width,flags:entryFlags,defined:(before[10]|ARITH)>>>0});equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset).getUint32(0,true),0x80000000,"earlier dirty store committed once");for(let n=0;n<4096;n++)equal(bytes(DATA)[n],data[n],"failed read and completed comparisons never write data");
      // A delivered fault ends this attempt. Guest repair may change flags;
      // an immediately repeated fault must restore the new attempt's flags.
      state()[9]=0xa17;state()[10]=0xfffff72a;await compile(CODE+3);const again=run(1);equal(again.kind,4,"unrepaired span faults again");equal(again.retired,0,"faulting instruction still unretired");equal(again.words[9],0xa17,"handler-provided entry flags are not replaced by stale prior attempt");equal(again.words[10],0xfffff72a,"new attempt defined-mask retained");
      if(protectedPage)protect(DATA+4096,READ|WRITE);else map(DATA+4096,READ|WRITE);
      const at=again.words[7]-DATA;for(let n=0;n<width;n++){const location=at+n;bytes(location<4096?DATA:DATA+4096)[location&4095]=(accumulator>>>(8*n))&255;}
      const repaired=run(1);equal(repaired.retired,1,"repair completes remaining instruction only");equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset).getUint32(0,true),0x80000000,"preceding store never replays");
    }));
  for(const width of [1,2,4])for(const repeat of [0xf2,0xf3])await test(`backward SCAS${width} repeat${repeat.toString(16)} fault preserves only whole comparisons`,()=>fixture(
    scasCode(width,repeat),async({map,bytes,reset,state,run})=>{
      const scanPage=0x6000,start=2*width-1,operand=repeat===0xf2?1:2;map(scanPage,READ,page([],0x55));
      writeScasElement(bytes(scanPage),start,width,operand);writeScasElement(bytes(scanPage),start-width,width,operand);
      reset({eax:2,ecx:5,edi:scanPage+start,flags:0xed7});seedScasState(state());const before=Array.from(state()),data=bytes(scanPage).slice(),fault=run(1);
      equal(fault.kind,4,"backward scan reaches absent preceding page");equal(fault.error,4,"first byte is unmapped user read");equal(fault.address,scanPage-1,"fault identifies first inaccessible byte of failed operand");equal(fault.retired,0,"faulted repeat remains unretired");checkScasWords(fault.words,before,{pc:CODE,ecx:3,edi:scanPage-1});
      for(let n=0;n<4096;n++)equal(bytes(scanPage)[n],data[n],"backward comparisons never modify either half of a failed span");
    }));
  await test("SCAS read aliases observe data and never create dirty or SMC writes",()=>fixture(
    [0xf2,0xae],async({handle,alias,bytes,reset,state,run})=>{
      alias(0x4000,DATA,READ);bytes(DATA).set([1,2,3,4]);reset({eax:3,ecx:4,edi:0x4000,flags:0xad7});seedScasState(state());const before=Array.from(state()),data=bytes(DATA).slice(),result=run(1,()=>equal(new Uint32Array(api.memory.buffer,api.backend_table(handle)-16,4)[1],0,"no dirty journal entry from reads before collection"));
      equal(result.retired,1,"read-only alias scan succeeds");checkScasWords(result.words,before,{pc:CODE+2,ecx:1,edi:0x4003,flags:(before[9]&~ARITH)|0x44,defined:(before[10]|ARITH)>>>0});for(let n=0;n<4096;n++){equal(bytes(DATA)[n],data[n],"canonical page unchanged");equal(bytes(0x4000)[n],data[n],"alias page unchanged");}
    }));
  for(const width of [1,2,4])for(const repeat of [0,0xf2,0xf3])await test(`LOCK SCAS${width} repeat${repeat.toString(16)} is invalid without memory access`,()=>fixture(
    scasCode(width,repeat,[0xf0]),async({reset,state,run})=>{
      reset({eax:7,ecx:0,edi:0xffffffff,flags:0xad7});seedScasState(state());const before=Array.from(state()),result=run(1);
      equal(result.kind,8,"LOCK SCAS decodes to architectural invalid opcode");equal(result.retired,0,"invalid instruction does not retire");checkScasWords(result.words,before,{pc:CODE});
    }));
  for(const width of [1,2,4])for(const backward of [false,true])await test(`REPNE SCAS${width} wraps EDI between complete elements DF=${Number(backward)}`,()=>fixture(
    scasCode(width,0xf2),async({map,bytes,reset,state,run})=>{
      map(0xfffff000,READ);map(0,READ);
      const start=backward?0:0x100000000-width,next=backward?0x100000000-width:0;
      writeScasElement(bytes(start>=0xfffff000?0xfffff000:0),start&4095,width,1);
      writeScasElement(bytes(next>=0xfffff000?0xfffff000:0),next&4095,width,2);
      reset({eax:2,ecx:3,edi:start,flags:0xad7|(backward?0x400:0)});seedScasState(state());const before=Array.from(state()),lastPage=bytes(0xfffff000).slice(),zeroPage=bytes(0).slice(),result=run(1);
      equal(result.retired,1,"two individually valid spans across index wrap complete");checkScasWords(result.words,before,{pc:CODE+scasCode(width,0xf2).length,ecx:1,edi:(start+(backward?-2*width:2*width))>>>0,flags:(before[9]&~ARITH)|0x44,defined:(before[10]|ARITH)>>>0});
      for(let n=0;n<4096;n++){equal(bytes(0xfffff000)[n],lastPage[n],"last virtual page unchanged");equal(bytes(0)[n],zeroPage[n],"zero virtual page unchanged");}
    }));
  for(const repeat of [0xf2,0xf3])await test(`${repeat===0xf2?'REPNE':'REPE'} SCAS snapshots carry original flags across two quota yields and a later fault`,()=>fixture(
    [repeat,0xae],async({handle,reset,state,run,map,bytes,compile,saveSnapshot,restoreSnapshot,snapshotBuffer})=>{
      const operand=repeat===0xf2?0x14:0x25;map(DATA+4096,READ|WRITE);bytes(DATA).fill(operand);bytes(DATA+4096).fill(operand);
      reset({eax:0x25,ecx:9000,edi:DATA,flags:0xad7});seedScasState(state());const before=Array.from(state()),data0=bytes(DATA).slice(),data1=bytes(DATA+4096).slice(),flags=(before[9]&~ARITH)|arithmetic('sub',8,0x25,operand,0).flags;
      const first=run(1);equal(first.kind,12,"first quota yields an incomplete instruction");equal(first.error,4096,"first exact element progress");equal(first.retired,0,"partial SCAS does not retire");checkScasWords(first.words,before,{pc:CODE,ecx:4904,edi:DATA+4096,flags,defined:(before[10]|ARITH)>>>0});
      const saved=saveSnapshot();for(let n=0;n<420;n++)equal(saved[n],new Uint8Array(Uint32Array.from(first.words).buffer)[n],"snapshot preserves architectural projection");
      // Constructing another thread's initial snapshot cannot replace this
      // suspended thread. Its later restore must carry its own continuation.
      snapshotBuffer((ptr,length)=>{
        ok(api.backend_create_snapshot(handle,CODE,STACK+512,0,ptr,length),"create independent thread context");
        const created=new Uint8Array(api.memory.buffer,ptr,length).slice();equal(new Uint32Array(created.buffer)[8],CODE,"new thread entry");equal(new Uint32Array(created.buffer)[4],STACK+512,"new thread stack");
      });
      const still=saveSnapshot();for(let n=0;n<saved.length;n++)equal(still[n],saved[n],"creating another context leaves parked continuation unchanged");
      reset({eax:operand,ecx:1,edi:DATA,flags:0x202});equal(run(1).retired,1,"unrelated thread attempt completes");restoreSnapshot(saved);
      ok(api.backend_cache_invalidate(handle),"discard compiled regions while thread remains parked");await compile(CODE);
      const zero=run(0);equal(zero.kind,1,"zero guest budget does not restart pending scan");equal(zero.retired,0,"zero budget does not retire");
      restoreSnapshot(saveSnapshot());const second=run(1);equal(second.kind,12,"second quota resumes original attempt");equal(second.error,4096,"second exact progress");checkScasWords(second.words,before,{pc:CODE,ecx:808,edi:DATA+8192,flags,defined:(before[10]|ARITH)>>>0});
      const afterSecond=saveSnapshot();restoreSnapshot(afterSecond);const fault=run(1);equal(fault.kind,4,"later unmapped page faults");equal(fault.address,DATA+8192,"fault names first unavailable byte");equal(fault.retired,0,"full REP remains unretired");checkScasWords(fault.words,before,{pc:CODE,ecx:808,edi:DATA+8192});
      for(let n=0;n<4096;n++){equal(bytes(DATA)[n],data0[n],"completed first page unchanged");equal(bytes(DATA+4096)[n],data1[n],"completed second page unchanged");}
      map(DATA+8192,READ|WRITE,page([],repeat===0xf2?0x25:0x14));const repaired=run(1);equal(repaired.retired,1,"fault repair only compares remaining element");equal(repaired.words[1],807,"completed prefix was not scanned twice");equal(repaired.words[7],DATA+8193,"repair preserves prior index progress");
    }));
  await test("SCAS snapshot restore validates complete progress atomically and remains independent of backend handle",()=>fixture(
    [0xf2,0xae],async({handle,reset,state,run,map,bytes,alias,saveSnapshot,restoreSnapshot,snapshotBuffer})=>{
      bytes(DATA).fill(1);map(DATA+4096,READ|WRITE,page([],1));reset({eax:2,ecx:5000,edi:DATA,flags:0xad7});seedScasState(state());const before=Array.from(state()),partial=run(1);equal(partial.kind,12,"source continuation suspended");const saved=saveSnapshot();
      snapshotBuffer((ptr,length)=>{
        new Uint8Array(api.memory.buffer,ptr,length).set(saved);check(api.backend_restore_snapshot(handle,ptr,length-1)!==0,"truncated snapshot rejected");
        new Uint8Array(api.memory.buffer,ptr,length)[4]^=1;check(api.backend_restore_snapshot(handle,ptr,length)!==0,"ECX inconsistent with private progress rejected");
      });
      const preserved=saveSnapshot();for(let n=0;n<saved.length;n++)equal(preserved[n],saved[n],"failed snapshot restore leaves architecture and continuation intact");
      const other=newBackend(1);try{const ptr=api.backend_input_alloc(other,saved.length);check(ptr!==0,"other handle owns buffer");new Uint8Array(api.memory.buffer,ptr,saved.length).set(saved);check(api.backend_restore_snapshot(handle,ptr,saved.length)!==0,"foreign owned snapshot buffer rejected");}finally{ok(api.backend_drop(other),"drop foreign buffer owner");}
      await fixture([0xf2,0xae],async(rebuilt)=>{
        rebuilt.bytes(DATA).fill(1);rebuilt.map(DATA+4096,READ|WRITE,page([],1));rebuilt.restoreSnapshot(saved);
        const again=rebuilt.saveSnapshot();for(let n=0;n<saved.length;n++)equal(again[n],saved[n],"rebuilt equivalent backend preserves complete thread snapshot");
        rebuilt.protect(DATA+4096,WRITE);const fault=rebuilt.run(1);equal(fault.kind,4,"new backend observes current permissions");equal(fault.error,5,"rebuild permission fault is precise");checkScasWords(fault.words,before,{pc:CODE,ecx:904,edi:DATA+4096});
      });
      restoreSnapshot(saved);const completed=run(1);equal(completed.retired,1,"original backend resumes separately");equal(completed.words[1],0,"original scan completes remaining count");equal(completed.words[7],DATA+5000,"original progress is not shared with other backend");
      alias(0x5000,DATA+4096,READ|WRITE);bytes(0x5000)[0]=2;restoreSnapshot(saved);const changedData=run(1);equal(changedData.retired,1,"restored scan observes current shared backing bytes");equal(changedData.words[1],903,"new match through data alias stops at first remaining element");equal(changedData.words[7],DATA+4097,"completed prefix is not re-read after alias update");
      for(let n=0;n<4096;n++){equal(bytes(DATA)[n],1,"completed prefix remains unchanged");equal(bytes(DATA+4096)[n],n===0?2:1,"only host alias update changes backing data");equal(bytes(0x5000)[n],n===0?2:1,"alias and canonical page remain identical");}
    }));
  await test("SCAS continuation rejects changed instruction encoding after a yield",()=>fixture(
    [0xf2,0xae],async({handle,reset,state,run,map,bytes,compile,saveSnapshot,restoreSnapshot})=>{
      bytes(DATA).fill(1);map(DATA+4096,READ|WRITE,page([],1));reset({eax:2,ecx:5000,edi:DATA,flags:0xad7});seedScasState(state());equal(run(1).kind,12,"original repeated scan yields");const saved=saveSnapshot(),before=Array.from(state());
      map(CODE,READ|EXECUTE|SEALED,page([0xf3,0xae,0xcc],0xcc));const changedId=await compile(CODE);restoreSnapshot(saved);
      const rejected=options.linkedRegions&&hasLinkInvocation?api.backend_link_begin(handle,changedId,CODE):api.backend_cache_validate(handle,changedId,CODE);
      check(rejected!==0,"different repeat condition is rejected before replacement instruction executes");for(let n=0;n<105;n++)equal(state()[n],before[n],"failed code admission preserves complete prior partial state");
      // Returning the original bytes and rebuilding is safe: the snapshot
      // describes architectural progress and encoding, never a cache unit ID.
      map(CODE,READ|EXECUTE|SEALED,page([0xf2,0xae,0xcc],0xcc));await compile(CODE);restoreSnapshot(saved);equal(run(1).retired,1,"equivalent code recompile resumes existing progress");
    }));
  await test("SCAS snapshot preserves original flags through instruction-fetch protection fault",()=>fixture(
    [0xf2,0xae],async({reset,state,run,map,protect,bytes,compile,saveSnapshot,restoreSnapshot})=>{
      bytes(DATA).fill(1);map(DATA+4096,READ|WRITE,page([],1));reset({eax:2,ecx:5000,edi:DATA,flags:0xad7});seedScasState(state());const before=Array.from(state());equal(run(1).kind,12,"scan parks after quota");const saved=saveSnapshot();
      protect(CODE,READ);await compile(CODE);restoreSnapshot(saved);const fault=run(1);equal(fault.kind,4,"current non-executable mapping produces fetch fault");equal(fault.error,21,"present user execute fault");equal(fault.address,CODE,"faulted instruction address");equal(fault.faultPc,CODE,"original repeat instruction remains fault PC");equal(fault.retired,0,"no element executes after failed fetch");checkScasWords(fault.words,before,{pc:CODE,ecx:904,edi:DATA+4096});
      protect(CODE,READ|EXECUTE|SEALED);await compile(CODE);restoreSnapshot(saved);equal(run(1).retired,1,"same instruction resumes after executable permission repair");equal(state()[1],0,"only remaining scan elements complete");
    }));
  for(const [firstCount,nextCount]of [[3000,2000],[4096,1],[4096,0]])await test(`SCAS shares REP fuel with preceding stores ${firstCount}+${nextCount}`,()=>fixture(
    [0xf3,0xaa,0xb9,...wordBytes(nextCount),0xbf,...wordBytes(DATA+3000),0xf2,0xae],async({reset,state,run,bytes,compile,saveSnapshot,restoreSnapshot})=>{
      bytes(DATA).fill(1);reset({eax:2,ecx:firstCount,edi:STACK,flags:0xad7});seedScasState(state());const before=Array.from(state()),data=bytes(DATA).slice(),first=run(4),partial=firstCount===3000;
      equal(first.retired,nextCount===0?4:3,"only complete instructions retire at shared quota");equal(first.kind,nextCount===0?1:partial?12:1,"zero-count comparison completes even with exhausted element fuel");
      equal(first.pc,CODE+(nextCount===0?14:12),"shared quota retains exact scan boundary");equal(first.words[1],nextCount-(partial?1096:0),"only actual comparisons reduce remaining count");equal(first.words[7],DATA+3000+(partial?1096:0),"only actual comparisons advance EDI");
      for(let n=0;n<4096;n++){equal(bytes(STACK)[n],n<firstCount?2:0,"preceding stores consume exactly shared fuel");equal(bytes(DATA)[n],data[n],"comparison data stays read-only");}
      for(let n=35;n<100;n++)equal(first.words[n],before[n],"shared REP work leaves all FP state untouched");
      if(nextCount){const saved=saveSnapshot();restoreSnapshot(saved);await compile(CODE+12);
        const fault=run(1);equal(fault.kind,partial?4:1,"resumed partial reaches fault while newly starting single scan completes");
        if(partial){equal(fault.pc,CODE+12,"partial fault exact repeat prefix");equal(fault.address,DATA+4096,"missing next page");equal(fault.words[9],before[9],"fault restores SCAS entry flags across different preceding REP");equal(fault.words[10],before[10],"fault restores entry defined mask");}
      }
    }));
  if(options.linkedRegions)await test("linked SCAS target preserves instruction-entry flags through shared-fuel snapshot restart",()=>fixture(
    [0xf3,0xaa,0xb9,...wordBytes(2000),0xff,0xe2],async({map,compile,reset,state,run,bytes,saveSnapshot,restoreSnapshot})=>{
      const target=0x5000;map(target,READ|EXECUTE|SEALED,page([0xbf,...wordBytes(DATA+3000),0xf2,0xae,0xcc],0xcc));await compile(target);
      bytes(DATA).fill(1);reset({eax:2,ecx:3000,edi:STACK,edx:target,flags:0xad7});seedScasState(state());const before=Array.from(state()),first=run(100);
      equal(first.kind,12,"linked target receives remaining element fuel");equal(first.error,1096,"target consumes remaining quota only");equal(first.retired,4,"source REP, count, branch and target address setup retire");equal(first.pc,target+5,"SCAS target exact partial PC");equal(first.words[1],904,"remaining linked comparison count");equal(first.words[7],DATA+4096,"linked scan stops at quota");
      const saved=saveSnapshot();restoreSnapshot(saved);await compile(target+5);const fault=run(1);equal(fault.kind,4,"linked resume observes missing page");equal(fault.retired,0,"incomplete target scan remains unretired");equal(fault.words[9],before[9],"original target-entry flags restored");equal(fault.words[10],before[10],"original target-entry defined mask restored");
      for(let n=0;n<4096;n++)equal(bytes(STACK)[n],n<3000?2:0,"linked source stores never replay");for(let n=35;n<100;n++)equal(fault.words[n],before[n],"all FP banks survive linked snapshot/fault");
    }));
  await test("code-alias store exits before SCAS starts and is not replayed after a scan fault",()=>fixture(
    [0xc6,0x03,0x90,0xf2,0xae],async({alias,reset,state,run,bytes,compile,saveSnapshot,restoreSnapshot})=>{
      alias(0x4000,CODE,READ|WRITE);bytes(DATA).fill(1);reset({eax:2,ecx:2,edi:DATA+4095,ebx:0x4100,flags:0xad7});seedScasState(state());const before=Array.from(state()),first=run(100);
      equal(first.kind,7,"preceding code-alias write requests SMC before scan");equal(first.retired,1,"only whole store completes");checkScasWords(first.words,before,{pc:CODE+3});equal(bytes(CODE)[0x100],0x90,"exact prior store is visible");
      const saved=saveSnapshot();restoreSnapshot(saved);await compile(CODE+3);const fault=run(1);equal(fault.kind,4,"scan resumes after code store and faults after one element");checkScasWords(fault.words,before,{pc:CODE+3,ecx:1,edi:DATA+4096});equal(bytes(CODE)[0x100],0x90,"old store is not repeated");
    },READ|WRITE|EXECUTE|SEALED));
  if(hasTimestamp&&hasSseCvtt)await test("SCAS imports coexist with x87, typed kernels, SSE conversion and timestamp completion",()=>fixture(
    [0xf2,0xae,0xd9,0xee,0xd9,0x1b,0xf3,0x0f,0x2c,0xd0,0x0f,0x31],async({handle,reset,state,run,bytes})=>{
      bytes(DATA)[0]=3;setWord(bytes(STACK),0,0xffffffff);reset({eax:3,ecx:1,edi:DATA,ebx:STACK,flags:0xad7});putXmm(state(),0,[0x3fc00000,0x7f800001,0x80000000,0xffffffff]);
      const result=run(10);equal(result.kind,15,"mixed module reaches Runtime timestamp request");equal(result.retired,4,"SCAS, FLDZ, FSTP and SSE conversion complete before RDTSC");equal(result.pc,CODE+10,"exact mixed-module timestamp PC");equal(result.words[1],0,"SCAS count consumed");equal(result.words[7],DATA+1,"SCAS index advanced");equal(result.words[2],1,"SSE helper returns truncated integer");equal(result.words[9],(0xad7&~ARITH)|0x44,"later FP/SSE/timestamp operations preserve SCAS flags");equal(result.words[99],0x1fa0,"SSE conversion records precision sticky flag");equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset).getUint32(0,true),0,"x87 store result is exact zero");
      for(const [n,value]of [0x3fc00000,0x7f800001,0x80000000,0xffffffff].entries())equal(result.words[67+n],value,"conversion source XMM remains raw and unchanged");
      ok(api.backend_complete_timestamp_counter(handle,CODE+10,0x12345678,9),"complete mixed module timestamp");equal(state()[8],CODE+12,"timestamp continuation follows whole instruction");equal(state()[0],0x12345678,"timestamp low word");equal(state()[2],9,"timestamp high word");equal(state()[104],5,"timestamp completion adds exactly one retirement");
    }));
  for (const width of [1, 2, 4]) for (const backward of [false, true]) {
    await test(`REP MOVS${width} ordered elements DF=${Number(backward)}`, () => fixture(
      [...(width === 2 ? [0x66] : []), 0xf3, width === 1 ? 0xa4 : 0xa5],
      async ({ reset, run, bytes }) => {
        const start = 128, length = 7 * width;
        for (let n = 0; n < length; n++) bytes(DATA)[start + n] = n * 7 + 3;
        const adjustment = backward ? 6 * width : 0;
        const flags = 0xa93 | (backward ? 0x400 : 0);
        reset({ esi: DATA + start + adjustment, edi: STACK + start + adjustment, ecx: 7, flags });
        const result = run(1); equal(result.retired, 1, "whole REP retires once");
        equal(result.words[1], 0, "all elements completed"); equal(result.words[9], flags, "all flags preserved");
        equal(result.words[6], DATA + start + (backward ? -width : length), "source next element");
        equal(result.words[7], STACK + start + (backward ? -width : length), "destination next element");
        for (let n = 0; n < length; n++) equal(bytes(STACK)[start + n], n * 7 + 3, "copied element bytes");
      }));
  }
  for (const opcode of [0xa4, 0xab]) await test(`REP zero ECX skips every data check ${opcode}`, () => fixture(
    [0xf3, opcode], async ({ reset, run }) => {
      reset({ esi: 0xffffffff, edi: 0xffffffff, ecx: 0, flags: 0xe93 });
      const result = run(1); equal(result.retired, 1, "empty REP completed"); equal(result.pc, CODE + 2, "empty REP continuation");
      equal(result.words[6], 0xffffffff, "empty source unchanged"); equal(result.words[7], 0xffffffff, "empty destination unchanged");
      equal(result.words[9], 0xe93, "empty flags unchanged");
    }));
  for (const opcode of [0xa5, 0xab]) await test(`single string operation ignores ECX ${opcode}`, () => fixture(
    [opcode], async ({ reset, run, bytes }) => {
      setWord(bytes(DATA), 0, 0x12345678); reset({ esi: DATA, edi: STACK, ecx: 0, eax: 0x87654321 });
      const result = run(1); equal(result.retired, 1, "single string completed"); equal(result.words[1], 0, "single ECX unchanged");
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), opcode === 0xa5 ? 0x12345678 : 0x87654321, "single data value");
      equal(result.words[6], opcode === 0xa5 ? DATA + 4 : DATA, "STOS does not advance source");
      equal(result.words[7], STACK + 4, "single destination advanced");
    }));
  await test("REP MOVS overlapping buffers observe each prior element", () => fixture(
    [0xf3, 0xa4], async ({ reset, run, bytes }) => {
      for (const backward of [false, true]) {
        bytes(DATA).set([1, 2, 3, 4, 5, 6, 7, 8, 9]);
        reset({ esi: DATA + (backward ? 8 : 0), edi: DATA + (backward ? 7 : 1), ecx: 8,
          flags: 0x202 | (backward ? 0x400 : 0) });
        equal(run(1).retired, 1, "overlapping REP completed");
        for (let n = 0; n < 9; n++) equal(bytes(DATA)[n], backward ? 9 : 1, "sequential alias propagation");
      }
    }));
  await test("REP FS source and fixed ES destination remain distinct", () => fixture(
    [0x64, 0xf3, 0xa5, 0x64, 0xf3, 0xab], async ({ reset, run, bytes, state, compile }) => {
      setWord(bytes(DATA), 128, 0x12345678); reset({ fs: DATA + 128, esi: 0, edi: STACK, ecx: 1 });
      const first = run(1); equal(first.words[6], 4, "FS offset advances without folding base");
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x12345678, "FS source bytes");
      state()[1] = 1; state()[0] = 0x87654321; await compile(CODE + 3);
      const second = run(1); equal(second.retired, 1, "FS prefix is ignored by STOS destination");
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(4, true), 0x87654321, "ES destination stays flat");
    }));
  await test("REP partial fuel yield resumes at remaining elements without retirement inflation", () => fixture(
    [0xf3, 0xa4], async ({ reset, run, bytes, map }) => {
      map(DATA + 4096, READ | WRITE); map(STACK + 4096, READ | WRITE);
      for (let n = 0; n < 5000; n++) bytes(DATA + (n & ~4095))[n & 4095] = (n * 13 + 7) & 255;
      reset({ esi: DATA, edi: STACK, ecx: 5000 });
      const first = run(1); equal(first.kind, 12, "bounded partial exit"); equal(first.error, 4096, "explicit element progress");
      equal(first.retired, 0, "incomplete REP is not a retired instruction"); equal(first.pc, CODE, "REP restart PC");
      equal(first.words[1], 904, "remaining count"); equal(first.words[6], DATA + 4096, "remaining source");
      bytes(DATA)[0] = 99;
      const second = run(1); equal(second.retired, 1, "completed resumed REP retires once"); equal(second.words[1], 0, "resume finishes remainder");
      equal(bytes(STACK)[0], 7, "completed prefix was not copied twice");
      for (let n = 4096; n < 5000; n++) equal(bytes(STACK + 4096)[n & 4095], (n * 13 + 7) & 255, "remaining bytes copied");
    }));
  await test("REP global fuel exhaustion before next instruction yields ordinary budget", () => fixture(
    [0xf3, 0xaa, 0xb9, 1, 0, 0, 0, 0xf3, 0xaa], async ({ reset, run, map, bytes, compile }) => {
      map(STACK + 4096, READ | WRITE); reset({ edi: STACK, ecx: 4096, eax: 7 });
      const first = run(3); equal(first.kind, 1, "ordinary yield before next REP starts");
      equal(first.retired, 2, "first REP plus MOV only"); equal(first.error, 0, "no false partial0 record");
      equal(first.pc, CODE + 7, "next REP retained"); equal(first.words[1], 1, "new count not consumed");
      if (options.rootedSsa) await compile(first.pc);
      equal(run(1).retired, 1, "next invocation gets fresh element quota"); equal(bytes(STACK + 4096)[0], 7, "next element stored");
    }));
  await test("REP destination cross-page fault preserves completed prefix and precise element", () => fixture(
    [0xf3, 0xa5], async ({ reset, run, bytes, map }) => {
      for (let n = 0; n < 12; n++) bytes(DATA)[n] = n + 1;
      reset({ esi: DATA, edi: STACK + 4090, ecx: 3, flags: 0xa93 });
      const first = run(1); equal(first.kind, 4, "second element page fault"); equal(first.retired, 0, "REP remains incomplete");
      equal(first.words[1], 2, "one complete element consumed"); equal(first.words[6], DATA + 4, "source does not pass failed element");
      equal(first.words[7], STACK + 4094, "destination points at failed element"); equal(first.words[9], 0xa93, "fault flags preserved");
      equal(first.address, STACK + 4096, "second page fault address");
      equal(bytes(STACK)[4094], 0, "failed split element leaves first page untouched");
      bytes(DATA)[0] = 99; map(STACK + 4096, READ | WRITE);
      const second = run(1); equal(second.retired, 1, "fault resolution resumes complete instruction");
      equal(bytes(STACK)[4090], 1, "prior complete element was not replayed");
      for (let n = 4; n < 12; n++) { const address = STACK + 4090 + n; equal(bytes(address & ~4095)[address & 4095], n + 1, "remaining elements"); }
    }));
  await test("REP source fault exposes prior stores without advancing failed read", () => fixture(
    [0xf3, 0xa5], async ({ reset, run, bytes, map }) => {
      setWord(bytes(DATA), 4092, 0x12345678); reset({ esi: DATA + 4092, edi: STACK, ecx: 2 });
      const first = run(1); equal(first.kind, 4, "second source read faults"); equal(first.words[1], 1, "remaining source count");
      equal(first.words[7], STACK + 4, "only committed destination advanced"); equal(first.address, DATA + 4096, "source fault address");
      setWord(bytes(DATA), 4092, 0xffffffff); map(DATA + 4096, READ | WRITE); setWord(bytes(DATA + 4096), 0, 0x87654321);
      equal(run(1).retired, 1, "source fault restart completes");
      equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0, true), 0x12345678, "prior store not replayed");
    }));
  await test("REP partial code-alias write exits before another element", () => fixture(
    [0xf3, 0xaa], async ({ reset, run, bytes, alias, compile }) => {
      alias(0x4000, CODE, READ | WRITE); reset({ edi: 0x4100, ecx: 3, eax: 0x90 });
      for (let n = 0; n < 3; n++) {
        const result = run(1);
        equal(result.kind, n === 2 ? 7 : 13, "partial versus completed code modification");
        equal(result.retired, n === 2 ? 1 : 0, "only last element retires REP");
        equal(result.words[1], 2 - n, "code exit preserves exact remaining count");
        if (n !== 2) { equal(result.error, 1, "one explicit partial element"); equal(result.pc, CODE, "partial restart PC"); }
        equal(bytes(CODE)[0x100 + n], 0x90, "one committed alias store");
        equal(bytes(CODE)[0x101 + n], 0xcc, "next element not written before exit");
        if (n !== 2) await compile(CODE);
      }
    }, READ | WRITE | EXECUTE | SEALED));
  await test("REP dirty capacity fault keeps prior element and no partial next store", () => fixture(
    [0xf3, 0xaa], async ({ handle, reset, run, map, bytes }) => {
      map(STACK + 4096, READ | WRITE); reset({ edi: STACK + 4095, ecx: 2, eax: 0x7f });
      const header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4);
      const capacity = header()[2]; header()[2] = 1;
      const first = run(1, () => { header()[2] = capacity; });
      equal(first.kind, 10, "second dirty page exhausts capacity"); equal(first.retired, 0, "incomplete REP retirement remains zero");
      equal(first.words[1], 1, "only first element consumed"); equal(bytes(STACK)[4095], 0x7f, "first element retained");
      equal(bytes(STACK + 4096)[0], 0, "failed element not written"); equal(run(1).retired, 1, "capacity retry resumes remainder");
    }));
  const extBytes = (significand, exponent) => {
    const data = new Uint8Array(10), view = new DataView(data.buffer);
    view.setBigUint64(0, significand, true); view.setUint16(8, exponent, true);
    return data;
  };
  const extRegister = (words, index) => ({
    significand: BigInt(words[35 + index * 3]) | BigInt(words[36 + index * 3]) << 32n,
    exponent: words[37 + index * 3],
  });
  await test("x87 logical stack crosses unequal-height branches with exact TOP at every budget", () => fixture(
    [0xd9,0xe8,0xdf,0xe0,0xeb,2,0x90,0x90,0xd9,0xee,0x75,4,0xdd,0xd8,0xeb,2,
      0xde,0xc1,0xd9,0xe8,0xde,0xc1,0xdf,0xe0], async ({ reset, state, run }) => {
      for (const top of [0,1,2,3,4,5,6,7]) for (const taken of [false,true]) {
        const path = taken ? [0,2,4,8,10,16,18,20,22] : [0,2,4,8,10,12,14,18,20,22];
        for (let budget=0;budget<=path.length;budget++) {
          reset({eax:0x76543210,flags:0x202|(taken?0:ZF)});
          const w=state(); w[59]=0x37f; w[60]=top<<11|0x4500; w[61]=0;
          for(let p=0;p<8;p++) {
            w.set([0x11223300+p,0x80000000+p,0x3ffe+p],35+p*3);
            w[61]|=(p%4)<<(p*2);
          }
          for(const p of [(top+7)&7,(top+6)&7])w[61]|=3<<(p*2);
          const expected=Array.from(w);let t=top;
          const tag=(p,value)=>{expected[61]=(expected[61]&~(3<<(p*2)))|value<<(p*2);};
          const write=(p,exponent)=>{expected.splice(35+p*3,3,0,exponent?0x80000000:0,exponent);tag(p,exponent?0:1);};
          for(const offset of path.slice(0,budget)) {
            if(offset===2||offset===22)expected[0]=((expected[0]&0xffff0000)|expected[60])>>>0;
            if([0,8,12,16,18,20].includes(offset)) {
              if(offset===0||offset===8||offset===18){t=(t+7)&7;write(t,offset===8?0:0x3fff);}
              else {if(offset!==12)write((t+1)&7,offset===20?0x4000:0x3fff);tag(t,3);t=(t+1)&7;}
              expected[60]=(expected[60]&~0xba80)|(t<<11);
              expected[62]=offset===8?0x1ee:offset===12?0x5d8:offset===16||offset===20?0x6c1:0x1e8;
              expected[63]=CODE+offset;expected[64]=expected[15]&0xffff;
            }
          }
          expected[8]=CODE+(path[budget]??24);
          const result=run(budget);
          equal(result.kind,1,"budget boundary at exact continuation");equal(result.retired,budget,"same completed instructions across branch");
          for(let n=0;n<100;n++)equal(result.words[n],expected[n],`architectural word${n} TOP${top} path${Number(taken)} budget${budget}`);
        }
      }
    }));
  for (const width of [16, 32]) await test(`CWD/CDQ ${width}-bit pair sign extension`, () => fixture(
    [...(width === 16 ? [0x66] : []), 0x99], async ({ reset, run }) => {
      for (const eax of [0, 1, 0x7fff, 0x8000, 0xffff, 0x7fffffff, 0x80000000, 0xffffffff]) {
        reset({ eax, edx: 0x12345678, flags: 0xa93 });
        const result = run(1), negative = Boolean(eax & (width === 16 ? 0x8000 : 0x80000000));
        equal(result.retired, 1, "pair sign extension completed"); equal(result.words[0], eax, "source accumulator retained");
        equal(result.words[2], width === 16 ? 0x12340000 | (negative ? 0xffff : 0) : negative ? 0xffffffff : 0,
          "destination sign extension and upper-half preservation");
        equal(result.words[9], 0xa93, "CWD/CDQ flags unchanged");
      }
    }));
  if (hasTypedNumeric) {
    await test("all typed numeric kernels preserve the legacy exact result over PC and RC", () => fixture(
      [0x90], async ({ handle, reset, state, bytes }) => {
        reset({ eax: 0x76543210, flags: 0xad7 }); bytes(DATA).fill(0x39);
        const before = Array.from(state()), image = bytes(DATA).slice();
        const raw = [[0,0x80000000,0x3fff],[1,0x80000000,0x3fff],[1,0,0],[0,0xc0000000,0x7fff],[0,0x80000001,0x7fff],[0xffffffff,0xffffffff,0x7ffe]];
        let stablePointer = 0;
        for (let op = 0; op < typedKernelCount; op++) for (const pc of [0,2,3]) for (const rc of [0,1,2,3]) for (let index = 0; index < raw.length; index++) {
          const control = 0x7f | pc << 8 | rc << 10;
          const a = op >= 6 && op <= 11 ? [[0x3f800001,0,0],[1,0,0],[0x7f800001,0,0],[0xffffffff,0x3ff00000,0],[0x8000,0,0],[0xffffffff,0x7fffffff,0]][index] : raw[index];
          const b = raw[(index + 1) % raw.length];
          const referencePointer = api.backend_fp_numeric(handle, op, control, ...a, ...b);
          const expected = referencePointer ? Array.from(new Uint32Array(api.memory.buffer, referencePointer, 7)) : null;
          const previous = stablePointer ? Array.from(new Uint32Array(api.memory.buffer, stablePointer, 7)) : null;
          const pointer = api[`backend_fp_kernel_${op}`](control, ...a, ...b);
          equal(pointer === 0, referencePointer === 0, `kernel${op} success/failure matches legacy numerical contract`);
          if (pointer) {
            if (!stablePointer) stablePointer = pointer;
            equal(pointer, stablePointer, "every typed operation shares the instance-owned result cell");
            const actual = new Uint32Array(api.memory.buffer, pointer, 7);
            for (let n = 0; n < 7; n++) equal(actual[n], expected[n], `kernel${op} exact value/bits/exceptions/round-up word${n}`);
            equal(api.backend_fp_kernel_error(), 0, "successful kernel clears prior diagnostic error");
          } else {
            check(api.backend_fp_kernel_error() > 0, "failed kernel supplies a typed error");
            if (previous) for (let n = 0; n < 7; n++) equal(new Uint32Array(api.memory.buffer, stablePointer, 7)[n], previous[n], "failed numerical operation cannot replace prior output");
          }
        }
        for (let n = 0; n < 105; n++) equal(state()[n], before[n], "pure kernels and oracle do not mutate CPU state");
        for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], image[n], "pure kernels and oracle do not access guest memory");
      }));
    await test("typed numeric result lifetime is per compiler instance rather than per guest backend", () => fixture(
      [0x90], async ({ handle, state, bytes, reset }) => {
        reset({ eax: 0xabcdef01 }); bytes(DATA).fill(0x53);
        const snapshot = Array.from(state()), image = bytes(DATA).slice();
        const add = (exports, exponent) => exports.backend_fp_kernel_0(0x37f,0,0x80000000,exponent,0,0x80000000,exponent);
        const pointer = add(api,0x3fff); check(pointer !== 0, "initial exact numeric output");
        const consumed = Array.from(new Uint32Array(api.memory.buffer,pointer,7));
        const temporary = newBackend(1); check(temporary !== 0, "temporary guest backend exists");
        equal(add(api,0x4000),pointer,"another caller reuses same instance cell");
        equal(new Uint32Array(api.memory.buffer,pointer,7)[2],0x4001,"next success overwrites scratch as documented");
        equal(consumed[2],0x4000,"caller consumed prior result before next call");
        ok(api.backend_drop(temporary),"temporary backend destroyed");
        equal(add(api,0x3fff),pointer,"destroying backend cannot invalidate global pure result cell");
        const separate = (await WebAssembly.instantiate(compilerBytes,{})).instance.exports;
        const separatePointer = add(separate,0x4001); check(separatePointer !== 0,"separate compiler can call numeric export without any backend");
        equal(new Uint32Array(separate.memory.buffer,separatePointer,7)[2],0x4002,"second compiler result");
        equal(new Uint32Array(api.memory.buffer,pointer,7)[2],0x4000,"separate compiler does not overwrite first instance scratch");
        const output = Array.from(new Uint32Array(api.memory.buffer,pointer,7));
        for (const [control,a2,b2,errorCode] of [[0x17f,0x3fff,0x3fff,1],[0x1037f,0x3fff,0x3fff,4],[0x37f,0x10000,0x3fff,4],[0x37f,0x3fff,0x10000,4]]) {
          equal(api.backend_fp_kernel_0(control,0,0x80000000,a2,0,0x80000000,b2),0,"invalid pure-kernel input rejected");
          equal(api.backend_fp_kernel_error(),errorCode,"failure error code is stable before next operation");
          equal(api.backend_fp_kernel_error(),errorCode,"reading error does not consume it");
          for(let n=0;n<7;n++) equal(new Uint32Array(api.memory.buffer,pointer,7)[n],output[n],"failure preserves every prior output word");
        }
        equal(add(api,0x3fff),pointer,"next valid call retains stable cell"); equal(api.backend_fp_kernel_error(),0,"next valid call clears diagnostic");
        for(let n=0;n<105;n++) equal(state()[n],snapshot[n],"interleaved backend lifetimes retain CPU state");
        for(let n=0;n<4096;n++) equal(bytes(DATA)[n],image[n],"interleaved backend lifetimes retain guest bytes");
        check(api.backend_state(handle)!==0,"original guest owner remains live");
      }));
    if (options.typedNumeric) await test("typed numeric failure preserves FP state and earlier store before exact retry", () => fixture(
      [0xff,0x03,0xd8,0xc1], async ({ reset,state,run,bytes,compile }) => {
        reset({ ebx:DATA,flags:0xad7 }); state()[59]=0x17f;state()[61]=0xfff0;
        for(let r=0;r<2;r++){state()[35+r*3]=0;state()[36+r*3]=0x80000000;state()[37+r*3]=0x3fff;}
        state()[62]=0x321;state()[63]=0x11223344;state()[65]=0x55667788;
        const before=Array.from(state().slice(35,100)), fault=run(100);
        equal(fault.kind,3,"reserved PC causes explicit unsupported exit");equal(fault.error,1,"kernel ReservedPrecision copied immediately to exit");
        equal(fault.retired,1,"only prior store completed");equal(fault.pc,CODE+2,"failed numeric instruction exact PC");equal(bytes(DATA)[0],1,"prior scalar write remains");
        for(let n=0;n<before.length;n++)equal(fault.words[35+n],before[n],"failed kernel cannot partially commit FP environment/payload");
        state()[59]=0x37f;await compile(CODE+2);const repaired=run(1);
        equal(repaired.retired,1,"fixed precision resumes only the FADD");equal(bytes(DATA)[0],1,"repair never replays earlier store");
        equal(extRegister(repaired.words,0).significand,0x8000000000000000n,"repaired exact sum");equal(extRegister(repaired.words,0).exponent,0x4000,"repaired sum exponent");
      }));
  }
  if (hasFpIr) {
    await test("exact numeric Wasm ABI owns isolated stable scratch and preserves guest state on every failure", () => fixture(
      [0x90], async ({ handle, state, bytes, reset }) => {
        const other = newBackend(1); check(other !== 0, "second numeric owner allocated");
        const readOutput = (pointer) => Array.from(new Uint32Array(api.memory.buffer, pointer, 7));
        const add = (owner, exponent) => api.backend_fp_numeric(owner, 0, 0x37f, 0, 0x80000000, exponent, 0, 0x80000000, exponent);
        try {
          reset({ eax: 0x12345678, flags: 0xad7 });
          state()[59] = 0x27f; state()[60] = 0x80c1; state()[61] = 0xfffc;
          for (let n = 0; n < 4096; n++) bytes(DATA)[n] = n * 37 + 19;
          const beforeState = Array.from(state()), beforeBytes = bytes(DATA).slice();
          const pointer = add(handle, 0x3fff), second = add(other, 0x4000);
          check(pointer !== 0 && second !== 0 && pointer !== second, "each owner has separate numeric output");
          const expected = [0, 0x80000000, 0x4000, 0, 0, 0, 0], otherExpected = [0, 0x80000000, 0x4001, 0, 0, 0, 0];
          const unchanged = (label) => {
            for (let n = 0; n < 105; n++) equal(state()[n], beforeState[n], `${label}: CPU word ${n}`);
            for (let n = 0; n < 4096; n++) equal(bytes(DATA)[n], beforeBytes[n], `${label}: guest byte ${n}`);
            for (let n = 0; n < 7; n++) {
              equal(readOutput(pointer)[n], expected[n], `${label}: first scratch word ${n}`);
              equal(readOutput(second)[n], otherExpected[n], `${label}: second scratch word ${n}`);
            }
          };
          unchanged("successful arithmetic has no architectural effects");
          for (const [owner, operation, control, a2, b2] of [
            [0, 0, 0x37f, 0x3fff, 0x3fff], [0xffffffff, 0, 0x37f, 0x3fff, 0x3fff],
            [handle, typedKernelCount, 0x37f, 0x3fff, 0x3fff], [handle, 0xffffffff, 0x37f, 0x3fff, 0x3fff],
            [handle, 0, 0x1037f, 0x3fff, 0x3fff], [handle, 0, 0x17f, 0x3fff, 0x3fff],
            [handle, 0, 0x37f, 0x10000, 0x3fff], [handle, 0, 0x37f, 0x3fff, 0x10000],
          ]) {
            equal(api.backend_fp_numeric(owner, operation, control, 0, 0x80000000, a2, 0, 0x80000000, b2), 0, "invalid helper input rejected");
            unchanged("rejected helper preserves owners and architectural state");
          }
          if (options.typedNumeric) ok(api.backend_set_typed_numeric(handle, 0), "disable typed kernels before changing FP IR mode");
          ok(api.backend_set_fp_ir(handle, options.fpIr ? 0 : 1), "change numeric caller lowering");
          ok(api.backend_cache_invalidate(handle), "invalidate numeric caller cache");
          reset();
          equal(add(handle, 0x3fff), pointer, "numeric output pointer survives options, cache and CPU context reset");
          for (let n = 0; n < 7; n++) equal(readOutput(second)[n], otherExpected[n], "context reset of first owner does not affect second scratch");
        } finally { ok(api.backend_drop(other), "drop second numeric owner"); }
      }));
    if (hasLinkInvocation && hasLinks) await test("pure numeric success retains invocation proof but a failed helper clears it", () => fixture(
      [0x90], async ({ handle, select, reset }) => {
        reset(); const id = select(CODE);
        ok(api.backend_link_begin(handle, id, CODE), "start numeric caller invocation");
        check(api.backend_fp_numeric(handle, 0, 0x37f, 0, 0x80000000, 0x3fff, 0, 0x80000000, 0x3fff) !== 0, "exact add succeeds");
        check(api.backend_link_resolve(handle, CODE) !== 0n, "memo target remains admitted after pure helper");
        equal(api.backend_link_memo_hits(handle), 1, "success preserves first-entry memo");
        equal(api.backend_fp_numeric(handle, typedKernelCount, 0x37f, 0, 0, 0, 0, 0, 0), 0, "bad numeric protocol rejected");
        check(api.backend_link_resolve(handle, CODE) !== 0n, "target is still independently admissible");
        equal(api.backend_link_memo_hits(handle), 1, "failure cleared memo proof rather than adding a stale hit");
        ok(api.backend_link_end(handle), "end numeric caller invocation");
      }));
  }
  // Reverse arithmetic keeps its mathematical operand order separate from its
  // physical destination, including ST0 aliases and a subsequently popped slot.
  {
    const reverseForms = [
      {name:"FDIVR ST0,STi",first:0xd8,second:0xf8,op:23,destination:false,pop:false,divide:true},
      {name:"FDIVR STi,ST0",first:0xdc,second:0xf0,op:23,destination:true,pop:false,divide:true},
      {name:"FDIVRP STi,ST0",first:0xde,second:0xf0,op:23,destination:true,pop:true,divide:true},
      {name:"FSUBR ST0,STi",first:0xd8,second:0xe8,op:22,destination:false,pop:false,divide:false},
      {name:"FSUBR STi,ST0",first:0xdc,second:0xe0,op:22,destination:true,pop:false,divide:false},
      {name:"FSUBRP STi,ST0",first:0xde,second:0xe0,op:22,destination:true,pop:true,divide:false},
    ];
    const reverseSeed=(words,top,index,a,b,control)=>{
      for(let r=0;r<8;r++)words.set([r+19,0x80000000,0x3fff],35+r*3);
      const put=(r,[sig,exp])=>words.set([Number(sig&0xffffffffn),Number(sig>>32n),exp],35+r*3);
      put(top,a);if(index)put((top+index)&7,b);
      words[59]=control;words[60]=(top<<11)|0x4700;words[61]=0;
      words[62]=0x321;words[63]=0x11223344;words[64]=0x1b;words[65]=0x55667788;words[66]=0x23;words[15]=0x1b;
      for(let n=67;n<99;n++)words[n]=(n*0x1234567)>>>0;words[99]=0x1f80;
    };
    const reversePairs=[
      [[0x8000000000000000n,0x4000],[0x8000000000000000n,0x4002]], // 2 and 8
      [[0xc000000000000000n,0x4000],[0xe000000000000000n,0x4001]], // 3 and 7
      [[0x8000000000000000n,0x3fbe],[0x8000000000000000n,0x3fff]], // 2^-65 and 1
    ];
    for(const form of reverseForms)await test(`${form.name} preserves operand order, aliases, precision and pop semantics`,async()=>{
      for(const index of [0,1,6,7])await fixture([form.first,form.second+index],async({handle,reset,state,run})=>{
        const descriptor=form.op|(index<<8)|(form.destination?1<<14:0)|(form.pop?1<<15:0),fop=((form.first&7)<<8)|(form.second+index);
        for(const pc of [0,2,3])for(const rc of [0,1,2,3])for(const top of [0,6,7])for(let pair=0;pair<reversePairs.length;pair++){
          const [a,b]=reversePairs[pair];reset({eax:0x12345678,flags:0xad7});reverseSeed(state(),top,index,a,b,0x7f|(pc<<8)|(rc<<10));
          const initial=Array.from(state()),zero=run(0);equal(zero.retired,0,"zero budget cannot modify an arithmetic destination or pop");for(let n=35;n<100;n++)equal(zero.words[n],initial[n],"zero budget preserves complete FP/vector state");
          const result=run(1);equal(result.retired,1,"reverse arithmetic retires once");
          if(pair===0&&index){
            const target=(top+(form.destination?index:0))&7,value=extRegister(result.words,target);
            equal(value.significand,form.divide?0x8000000000000000n:0xc000000000000000n,"independent noncommutative exact significand");
            equal(value.exponent,form.divide?(form.destination?0x3ffd:0x4001):(form.destination?0xc001:0x4001),"independent 8/2 versus2/8 or8−2 versus2−8");
          }
          new Uint32Array(api.memory.buffer,api.backend_state(handle),105).set(initial);
          equal(api.backend_x87(handle,descriptor,0xffffffff,0,CODE,fop),0,"independent state transition accepts descriptor");state()[8]=CODE+2;
          for(let n=0;n<100;n++)equal(result.words[n],state()[n],`reverse typed lowering vs state transition word${n}`);
          equal(result.words[9],0xad7,"reverse arithmetic retains integer flags");
        }
      });
    });
    const reverseMemory = [];
    for(const divide of [false,true])for(const format of [
      {name:"f32",first:0xd8,kind:1,bytes:4,source:[0,0,0xe0,0x40]},
      {name:"f64",first:0xdc,kind:2,bytes:8,source:[0,0,0,0,0,0,0x1c,0x40]},
      {name:"i16",first:0xde,kind:4,bytes:2,source:[7,0]},
      {name:"i32",first:0xda,kind:5,bytes:4,source:[7,0,0,0]},
    ])reverseMemory.push({...format,divide,op:divide?23:22,second:divide?0x3b:0x2b});
    for(const form of reverseMemory){
      const name=`${form.divide?"reverse divide":"reverse subtract"} ${form.name}`;
      await test(`${name} reads memory as left operand across every PC/RC`,()=>fixture(
        [form.first,form.second],async({handle,bytes,reset,state,run})=>{
          bytes(DATA).fill(0x61);bytes(DATA).set(form.source,3);const image=bytes(DATA).slice(),fop=((form.first&7)<<8)|form.second;
          for(const pc of [0,2,3])for(const rc of [0,1,2,3])for(const negative of [false,true]){
            reset({ebx:DATA+3,flags:0xad7});reverseSeed(state(),6,1,[0xc000000000000000n,negative?0xc000:0x4000],[0x8000000000000000n,0x3fff],0x7f|(pc<<8)|(rc<<10));
            const initial=Array.from(state()),result=run(1);equal(result.retired,1,"unaligned reverse memory arithmetic completes");
            if(!form.divide){equal(extRegister(result.words,6).significand,negative?0xa000000000000000n:0x8000000000000000n,"memory minus ST0 exact significand");equal(extRegister(result.words,6).exponent,negative?0x4002:0x4001,"7−(−3)=10 or7−3=4");}
            state().set(initial);equal(api.backend_x87(handle,form.op|(form.kind<<11),3,DATA+3,CODE,fop),0,"reference reverse memory operation");state()[8]=CODE+2;
            for(let n=0;n<100;n++)equal(result.words[n],state()[n],"memory typed lowering matches state transition");
            for(let n=0;n<4096;n++)equal(bytes(DATA)[n],image[n],"reverse memory arithmetic is read-only");
          }
        }));
      await test(`${name} pending MF and cross-page source faults precede arithmetic effects`,()=>fixture(
        [0xff,0x02,form.first,form.second],async({map,bytes,reset,state,run,compile})=>{
          const offset=4095;bytes(DATA)[offset]=form.source[0];
          reset({edx:STACK,ebx:DATA+offset,flags:0xad7});reverseSeed(state(),0,1,[0xc000000000000000n,0x4000],[0x8000000000000000n,0x3fff],0x37e);state()[60]=0x80c1;
          const initial=Array.from(state().slice(35,100)),pending=run(100);equal(pending.kind,14,"pending MF precedes missing memory tail");equal(pending.retired,1,"prior scalar store completed once");equal(pending.pc,CODE+2,"pending fault at reverse operation");equal(bytes(STACK)[0],1,"prior store retained");
          for(let n=0;n<initial.length;n++)equal(pending.words[35+n],initial[n],"pending MF preserves all FP/vector state");
          state()[59]=0x37f;state()[60]=0;await compile(CODE+2);const before=Array.from(state().slice(35,100)),memory=run(1);
          equal(memory.kind,4,"cleared pending exception reveals exact source page fault");equal(memory.address,DATA+4096,"first missing source byte page");equal(memory.retired,0,"failed source read does not retire arithmetic");for(let n=0;n<before.length;n++)equal(memory.words[35+n],before[n],"source fault has no FP partial effects");
          map(DATA+4096,READ|WRITE);bytes(DATA+4096).set(form.source.slice(1));const result=run(1);equal(result.retired,1,"repaired source executes only failed arithmetic");equal(bytes(STACK)[0],1,"repair cannot replay preceding scalar store");
          equal(result.words[65],DATA+offset,"completed memory arithmetic publishes precise FDP");
        }));
    }
    for(const pop of [false,true])await test(`FDIVR${pop?"P":""} unmasked zero divide preserves destination and pop`,()=>fixture(
      [pop?0xde:0xdc,0xf1,0x40,0xd9,0xc0],async({reset,state,run})=>{
        for(const masked of [false,true]){
          reset({eax:10,flags:0xad7});reverseSeed(state(),6,1,[0x8000000000000000n,0x4000],[0n,0],masked?0x37f:0x37b);state()[60]=6<<11;state()[61]=1<<14;
          const initial=Array.from(state()),result=run(masked?1:100);
          equal(result.retired,masked?1:2,"zero divide retires; integer may run before next waiting MF");equal(result.words[60],masked?(pop?7<<11:6<<11)|4:(6<<11)|0x8084,"masked/unmasked ZE and TOP");
          if(masked){equal(extRegister(result.words,7).significand,0x8000000000000000n,"positive infinity significand");equal(extRegister(result.words,7).exponent,0x7fff,"ST0/ST1 uses zero denominator");equal(result.words[61],(2<<14)|(pop?3<<12:0),"masked result tag and optional pop");}
          else{equal(result.kind,14,"next waiting instruction delivers deferred ZE");equal(result.pc,CODE+3,"precise next waiting instruction");equal(result.words[0],11,"intervening INC completed once");equal(result.words[61],initial[61],"unmasked zero divide suppresses pop");for(let n=35;n<59;n++)equal(result.words[n],initial[n],"unmasked zero divide writes no FP payload");}
        }
      }));
    for(const form of reverseForms)await test(`${form.name} empty source or destination cannot manufacture a result or pop`,()=>fixture(
      [form.first,form.second+1],async({reset,state,run})=>{
        for(const missing of [0,1])for(const masked of [false,true]){
          reset();reverseSeed(state(),0,1,[0x8000000000000000n,0x4000],[0x8000000000000000n,0x4002],masked?0x37f:0x37e);state()[60]=0x200;state()[61]=3<<(missing*2);
          const initial=Array.from(state()),result=run(1);equal(result.retired,1,"new stack invalid is deferred");
          equal(result.words[60],masked?(form.pop?0x841:0x41):0x80c1,"stack underflow clears C1 and pops only masked pop forms");
          const destination=form.destination?1:0,shift=destination*2;
          let tag=(initial[61]&~(3<<shift))|(2<<shift);if(form.pop)tag|=3;
          equal(result.words[61],masked?tag:initial[61],"masked indefinite destination/pop or unmasked retained tags");
          if(masked){equal(extRegister(result.words,destination).significand,0xc000000000000000n,"masked indefinite payload");equal(extRegister(result.words,destination).exponent,0xffff,"masked indefinite exponent");}
          else for(let n=35;n<59;n++)equal(result.words[n],initial[n],"unmasked invalid preserves all payloads");
        }
      }));
    for(const form of reverseMemory.filter(form=>form.kind===1||form.kind===2))await test(
      `reverse ${form.divide?"divide":"subtract"} ${form.name} denormal input precedes rounding and suppresses unmasked results`,()=>fixture(
        [form.first,form.second,0x40,0xd9,0xc0],async({handle,bytes,reset,state,run,compile})=>{
          bytes(DATA).fill(0);bytes(DATA)[3]=1;
          for(const masked of [false,true]){
            await compile(CODE);
            reset({eax:10,ebx:DATA+3,flags:0xad7});reverseSeed(state(),6,1,[0x8000000000000000n,0x4000],[0x8000000000000000n,0x3fff],masked?0x37f:0x37d);state()[60]=6<<11;
            const initial=Array.from(state()),result=run(1);equal(result.retired,1,"new denormal exception is deferred");
            equal(result.words[60]&0x3f,masked?(form.divide?2:0x22):2,"unmasked DE suppresses subsequent precision exception");
            equal(result.words[61],initial[61],"non-pop denormal operation preserves valid stack topology");
            if(masked){const value=extRegister(result.words,6);equal(value.significand,0x8000000000000000n,"tiny/2 or tiny−2 raw significand");equal(value.exponent,form.divide?(form.kind===1?0x3f69:0x3bcc):0xc000,"exact finite result or correctly rounded negative two");}
            else for(let n=35;n<59;n++)equal(result.words[n],initial[n],"unmasked DE writes no raw FP payload");
            state().set(initial);equal(api.backend_x87(handle,form.op|(form.kind<<11),3,DATA+3,CODE,((form.first&7)<<8)|form.second),0,"denormal reference transition");state()[8]=CODE+2;
            for(let n=0;n<100;n++)equal(result.words[n],state()[n],"denormal lowering matches complete state transition");
            state().set(result.words);
            if(!masked){await compile(CODE+2);const pending=run(100);equal(pending.kind,14,"next waiting FP instruction delivers DE");equal(pending.pc,CODE+3,"deferred DE preserves exact resume point");equal(pending.retired,1,"only intervening integer instruction completes");equal(pending.words[0],11,"integer executes once before pending MF");}
          }
        }));
    await test("FDIVRP unmasked denormal denominator suppresses overflow and pop",()=>fixture(
      [0xde,0xf1,0x40,0xd9,0xc0],async({reset,state,run})=>{
        reset({eax:10});reverseSeed(state(),7,1,[0x8000000000000000n,0x4000],[1n,0],0x37d);state()[60]=7<<11;state()[61]=2;
        const initial=Array.from(state()),result=run(100);equal(result.kind,14,"next waiting instruction reports unmasked DE");equal(result.retired,2,"arithmetic and intervening integer retire");equal(result.pc,CODE+3,"exact pending fault PC");equal(result.words[60],(7<<11)|0x8082,"DE suppresses range arithmetic and preserves TOP");equal(result.words[61],initial[61],"DE suppresses pop at TOP wrap");
        for(let n=35;n<59;n++)equal(result.words[n],initial[n],"DE leaves physical stack values unchanged");
      }));
  }
  if (typedKernelCount >= 21) {
    // These finite results are certified by an independent rational log series.
    // Architectural PE is separate: FYL2X sets it even for ordinary exact powers.
    const logarithms = [
      { name: "one below unity", x: [0xffffffffffffffffn,0x3ffe], y: [0x8000000000000000n,0x3fff],
        exponent: 0xbfbf, values: [0xb8aa3b295c17f0bcn,0xb8aa3b295c17f0bdn,0xb8aa3b295c17f0bcn,0xb8aa3b295c17f0bcn], precision: true },
      { name: "one above unity", x: [0x8000000000000001n,0x3fff], y: [0x8000000000000000n,0x3fff],
        exponent: 0x3fc0, values: [0xb8aa3b295c17f0bbn,0xb8aa3b295c17f0bbn,0xb8aa3b295c17f0bcn,0xb8aa3b295c17f0bbn], precision: true },
      { name: "negative scaled near power", x: [0x8000000000000001n,0x4000], y: [0x8000000000000000n,0xbfff],
        exponent: 0xbfff, values: [0x8000000000000001n,0x8000000000000002n,0x8000000000000001n,0x8000000000000001n], precision: true },
      { name: "exact dyadic power", x: [0x8000000000000000n,0x4000], y: [0xc000000000000000n,0x3fff],
        exponent: 0x3fff, values: Array(4).fill(0xc000000000000000n), precision: true },
      { name: "power with full significand multiplier", x: [0x8000000000000000n,0x4000], y: [0xb17217f7d1cf79acn,0x3ffe],
        exponent: 0x3ffe, values: Array(4).fill(0xb17217f7d1cf79acn), precision: true },
      { name: "unity times negative", x: [0x8000000000000000n,0x3fff], y: [0x8000000000000000n,0xbfff],
        exponent: 0x8000, values: Array(4).fill(0n), precision: false },
    ];
    const seedLogarithm = (words, x, y, control, top = 0) => {
      for (let r = 0; r < 8; r++) words.set([r+11,0x80000000,0x3fff],35+r*3);
      const put = (r, [sig,exp]) => words.set([Number(sig & 0xffffffffn),Number(sig >> 32n),exp],35+r*3);
      put(top,x); put((top+1)&7,y);
      words[59]=control;words[60]=(top<<11)|0x4700;
      words[61]=0xffff & ~(3 << (top*2)) & ~(3 << (((top+1)&7)*2));
      words[62]=0x321;words[63]=0x11223344;words[64]=0x1b;words[65]=0x55667788;words[66]=0x23;
      for(let n=67;n<100;n++)words[n]=n===99?0x1f80:(n*0x1234567)>>>0;
      words[15]=0x1b;
    };
    for (const sample of logarithms) await test(`FYL2X ${sample.name}: full ext80, all PC/RC and budget resume`, () => fixture(
      [0xd9,0xf1,0xdb,0x3b], async ({reset,state,run,compile,select,bytes}) => {
        await compile(CODE+2);
        for(const pc of [0,2,3])for(const rc of [0,1,2,3])for(const top of [0,6,7]) {
          reset({ebx:DATA,flags:0xad7});seedLogarithm(state(),sample.x,sample.y,0x7f|(pc<<8)|(rc<<10),top);
          const before=Array.from(state());select(CODE);
          const zero=run(0);equal(zero.retired,0,"zero budget cannot execute FYL2X");
          for(let n=35;n<100;n++)equal(zero.words[n],before[n],"budget zero preserves all FP/vector state");
          const result=run(1),destination=(top+1)&7,value=sample.values[rc];
          equal(result.retired,1,"FYL2X retires once");equal(result.pc,CODE+2,"budget ends before store");
          equal(extRegister(result.words,destination).significand,value,"certified result significand");
          equal(extRegister(result.words,destination).exponent,sample.exponent,"certified result sign/exponent");
          for(let r=0;r<8;r++)if(r!==destination)for(let n=0;n<3;n++)equal(result.words[35+r*3+n],before[35+r*3+n],"unwritten physical raw80 remains");
          const increment=value!==sample.values[3];
          equal(result.words[60],(destination<<11)|0x4500|(sample.precision?0x20:0)|(increment?0x200:0),"TOP/C1/PE separated from undefined condition bits");
          equal(result.words[61],(0xffff & ~(3<<(destination*2))) | (value===0n?1<<(destination*2):0),"one pop and classified destination tag");
          equal(result.words[59],before[59],"control word unchanged");equal(result.words[62],0x1f1,"FYL2X FOP");equal(result.words[63],CODE,"FYL2X FIP");
          equal(result.words[64],0x1b,"FYL2X FCS");equal(result.words[65],before[65],"register operation retains FDP");equal(result.words[66],before[66],"register operation retains FDS");
          equal(result.words[9],0xad7,"EFLAGS retained");for(let n=67;n<100;n++)equal(result.words[n],before[n],"XMM/MXCSR retained");
          select(CODE+2);const resumed=run(1);equal(resumed.retired,1,"resume stores result without replaying logarithm");
          const expected=extBytes(value,sample.exponent);for(let n=0;n<10;n++)equal(bytes(DATA)[n],expected[n],"resumed m80 store bytes");
          equal(resumed.words[61],0xffff,"resumed store empties the remaining stack value");
        }
      }));
    await test("FYL2X exact power sets architectural PE; unmask commits result then faults at next waiting instruction", () => fixture(
      [0xd9,0xf1,0x40,0xd9,0x02],async({reset,state,run})=>{
        reset({eax:7,edx:0xffffffff});seedLogarithm(state(),[0x8000000000000000n,0x4000],[0x8000000000000000n,0x3fff],0x35f);
        const result=run(100);equal(result.kind,14,"next waiting instruction reports unmasked PE");equal(result.error,32,"pending cause is precision");
        equal(result.retired,2,"FYL2X and intervening integer operation completed");equal(result.pc,CODE+3,"MF precedes next memory access");equal(result.words[0],8,"integer operation happened once");
        equal(result.words[60],0xcda0,"PE/ES/B with popped TOP and retained undefined condition bits");equal(result.words[61],0xfff3,"unmasked PE does not suppress result/pop");
        equal(extRegister(result.words,1).significand,0x8000000000000000n,"exact value retained despite architectural PE");
        equal(result.words[63],CODE,"saved FIP remains causing FYL2X");
      }));
    await test("FYL2X pending MF preserves prior store and all FP state without double execution",()=>fixture(
      [0xff,0x03,0xd9,0xf1],async({reset,state,run,bytes,compile})=>{
        reset({ebx:DATA,flags:0xad7});seedLogarithm(state(),[0x8000000000000000n,0x4000],[0x8000000000000000n,0x3fff],0x35f);
        state()[60]=0x80a0;const before=Array.from(state().slice(35,100)),fault=run(100);
        equal(fault.kind,14,"preexisting unmasked precision faults");equal(fault.retired,1,"only earlier store retires");equal(fault.pc,CODE+2,"precise waiting instruction");equal(bytes(DATA)[0],1,"earlier store retained");
        for(let n=0;n<before.length;n++)equal(fault.words[35+n],before[n],"pending MF has no FP partial effects");
        state()[59]=0x37f;state()[60]=0;await compile(CODE+2);const repaired=run(1);
        equal(repaired.retired,1,"repaired operation alone retires");equal(bytes(DATA)[0],1,"repair never replays earlier store");equal(repaired.words[61],0xfff3,"repair pops once");
      }));
    for(const empty of [1,2,3])await test(`FYL2X empty input mask${empty} suppresses or writes indefinite atomically`,()=>fixture(
      [0xd9,0xf1],async({reset,state,run})=>{
        for(const masked of [false,true]){
          reset();seedLogarithm(state(),[0x8000000000000000n,0x4000],[0x8000000000000000n,0x3fff],masked?0x37f:0x37e);
          state()[60]=0x200;state()[61]|=(empty&1?3:0)|(empty&2?12:0);const before=Array.from(state()),result=run(1);
          equal(result.retired,1,"new stack invalid is deferred");equal(result.words[60],masked?0x841:0x80c1,"native-confirmed stack underflow clears C1");
          equal(result.words[61],masked?0xfffb:before[61],"masked pop/indefinite or unmasked retained tags");
          if(masked){equal(extRegister(result.words,1).significand,0xc000000000000000n,"masked indefinite");equal(extRegister(result.words,1).exponent,0xffff,"masked indefinite sign/exponent");}
          for(let r=0;r<8;r++)if(!masked||r!==1)for(let n=0;n<3;n++)equal(result.words[35+r*3+n],before[35+r*3+n],"suppressed or unwritten raw registers retained");
          equal(result.words[62],0x1f1,"faulting arithmetic records FOP");equal(result.words[63],CODE,"faulting arithmetic records FIP");
        }
      }));
    const logSpecials = [
      {name:"negative domain precedes denormal",x:[0x8000000000000000n,0xbfff],y:[1n,0],value:[0xc000000000000000n,0xffff],flags:1},
      {name:"zero divide precedes denormal",x:[0n,0],y:[1n,0],value:[0x8000000000000000n,0xffff],flags:4},
      {name:"zero with infinity has no zero-divide",x:[0n,0],y:[0x8000000000000000n,0x7fff],value:[0x8000000000000000n,0xffff],flags:0},
      {name:"unity still observes denormal",x:[0x8000000000000000n,0x3fff],y:[1n,0],value:[0n,0],flags:2},
      {name:"infinity still observes denormal",x:[0x8000000000000000n,0x7fff],y:[1n,0],value:[0x8000000000000000n,0x7fff],flags:2},
      {name:"quiet NaN precedes domain",x:[0x8000000000000000n,0xbfff],y:[0xc000000000000123n,0x7fff],value:[0xc000000000000123n,0x7fff],flags:0},
      {name:"signaling NaN quiets with invalid",x:[0x8000000000000123n,0x7fff],y:[0x8000000000000000n,0x3fff],value:[0xc000000000000123n,0x7fff],flags:1},
      {name:"unsupported raw encoding is invalid",x:[0x4000000000000000n,0x3fff],y:[0x8000000000000000n,0x3fff],value:[0xc000000000000000n,0xffff],flags:1},
    ];
    const logTag=([sig,exp])=>!(exp&0x7fff)&&sig===0n?1:!(exp&0x7fff)||(exp&0x7fff)===0x7fff||sig<0x8000000000000000n?2:0;
    for(const sample of logSpecials)await test(`FYL2X ${sample.name}: masked value and unmasked suppression`,()=>fixture(
      [0xd9,0xf1],async({reset,state,run})=>{
        for(const pc of [0,2,3])for(const rc of [0,1,2,3])for(const unmask of [0,sample.flags||0x3f]){
          reset({flags:0xad7});seedLogarithm(state(),sample.x,sample.y,(0x7f|(pc<<8)|(rc<<10))&~unmask);
          state()[61]=0xfff0|logTag(sample.x)|(logTag(sample.y)<<2);
          const before=Array.from(state()),result=run(1),suppressed=Boolean(sample.flags&unmask);
          equal(result.retired,1,"new architectural exception is deferred");
          equal(result.words[60],0x4500|sample.flags|(suppressed?0x8080:0x800),"exception precedence and TOP");
          equal(result.words[61],suppressed?before[61]:0xfff3|(logTag(sample.value)<<2),"result classification or suppressed tags");
          if(!suppressed){equal(extRegister(result.words,1).significand,sample.value[0],"special result payload");equal(extRegister(result.words,1).exponent,sample.value[1],"special result sign/exponent");}
          for(let r=0;r<8;r++)if(suppressed||r!==1)for(let n=0;n<3;n++)equal(result.words[35+r*3+n],before[35+r*3+n],"unwritten physical raw80 retained");
          equal(result.words[62],0x1f1,"special records FOP");equal(result.words[63],CODE,"special records FIP");
          equal(result.words[65],before[65],"special register operation retains FDP");equal(result.words[9],0xad7,"special retains integer flags");
          for(let n=67;n<100;n++)equal(result.words[n],before[n],"special retains vectors/MXCSR");
        }
      }));
    await test("FYL2X raw pseudo-denormal observes DE without losing the extended exponent",()=>fixture(
      [0xd9,0xf1],async({reset,state,run})=>{
        reset();seedLogarithm(state(),[0x8000000000000000n,0],[0x8000000000000000n,0x3fff],0x37f);
        const result=run(1);equal(result.retired,1,"pseudo-denormal arithmetic completes");equal(result.words[60]&0x3f,0x22,"raw exponent zero reports DE plus ordinary PE");
        equal(extRegister(result.words,1).significand,0xfff8000000000000n,"log2 minimum-normal magnitude 16382");equal(extRegister(result.words,1).exponent,0xc00c,"log2 minimum-normal sign/exponent");
      }));
    await test("FYL2X exact subnormal reports range flags and unmasked range exits atomically",()=>fixture(
      [0xff,0x03,0xd9,0xf1],async({reset,state,run,bytes})=>{
        for(const unmask of [0,16]){
          reset({ebx:DATA});bytes(DATA).fill(0);seedLogarithm(state(),[0x8000000000000000n,0x4000],[1n,0],0x37f&~unmask);
          const before=Array.from(state().slice(35,100)),result=run(100);
          equal(bytes(DATA)[0],1,"preceding store retained once");
          if(unmask){equal(result.kind,3,"unmasked range is explicit unsupported");equal(result.error,0,"architectural range exit retains the existing neutral diagnostic");equal(result.retired,1,"unsupported FP instruction never retires");equal(result.pc,CODE+2,"exact unsupported range PC");
            for(let n=0;n<before.length;n++)equal(result.words[35+n],before[n],"unsupported range cannot mutate FP state");
          }else{equal(result.retired,2,"masked tiny result completes");equal(result.words[60]&0x3f,0x32,"exact tiny transcendental still sets DE/UE/PE");
            equal(extRegister(result.words,1).significand,1n,"smallest subnormal retained");equal(extRegister(result.words,1).exponent,0,"subnormal exponent retained");}
        }
      }));
  }
  if (!(options.functionCfg || options.loopMemory || options.structuredLoops || options.affineMemory
    || options.valueIr || options.predicateIr || options.flowIr || options.rootedSsa)) {
    await test("maximum admitted region preserves large completed prefixes and cold exits", () => fixture(
      [0x90], async ({ map, compile, reset, state, run }) => {
        map(0x10000, READ | EXECUTE | SEALED, page([], 0x40));
        map(0x11000, READ | EXECUTE | SEALED, page([], 0x40));
        await compile(0x10000, 8192);
        for (const budget of [0, 1, 127, 128, 4095, 4096, 8191, 8192, 8193]) {
          reset({ eip: 0x10000, eax: 0x7ffff000, flags: 0x203 }); state()[10] = 0;
          const result = run(budget), completed = Math.min(budget, 8192);
          equal(result.retired, completed, "bounded large region retires its exact prefix");
          equal(result.kind, budget <= 8192 ? 1 : 2, "budget or external undecoded edge");
          equal(result.pc, 0x10000 + completed, "large selector retains the exact PC");
          equal(result.words[0], (0x7ffff000 + completed) >>> 0, "no skipped or repeated increments");
          const flags = completed ? 0x203 & ~(ARITH & ~CF)
            | arithmetic("add", 32, (0x7ffff000 + completed - 1) >>> 0, 1, 0).flags & (ARITH & ~CF) : 0x203;
          equal(result.words[9], flags, "full and interruptible paths agree on final flags");
          equal(result.words[10], completed ? ARITH & ~CF : 0, "large prefix defined mask");
        }
      }));
    await test("cold flag recipe maximum region retains bounded saved inputs and exact large prefixes", () => fixture(
      [0x90], async ({ map, compile, reset, state, run, bytes }) => {
        const code = new Uint8Array(4096 * 5);
        for (let offset = 0; offset < code.length; offset += 5) code.set([0x83, 0xc0, 1, 0x8b, 0x16], offset);
        for (let offset = 0; offset < code.length; offset += 4096) map(0x10000 + offset, READ | EXECUTE | SEALED, code.slice(offset, offset + 4096));
        setWord(bytes(DATA), 0, 0x55667788); await compile(0x10000, 8192);
        for (const budget of [0, 1, 2, 3, 127, 128, 4095, 4096, 8190, 8191, 8192, 8193]) {
          reset({ eip: 0x10000, eax: 0x7ffff800, edx: 0x11223344, esi: DATA, flags: 0x203 }); state()[10] = 0;
          const result = run(budget), completed = Math.min(budget, 8192), additions = Math.ceil(completed / 2);
          equal(result.retired, completed, "large recipe table retires exact prefix");
          equal(result.kind, budget <= 8192 ? 1 : 2, "large recipe budget or undecoded edge");
          equal(result.pc, 0x10000 + Math.floor(completed / 2) * 5 + (completed % 2 ? 3 : 0), "large recipe exact continuation");
          equal(result.words[0], (0x7ffff800 + additions) >>> 0, "large recipe never replays its producer");
          equal(result.words[2], completed >= 2 ? 0x55667788 : 0x11223344, "memory load can overwrite scratch without losing recipe input");
          equal(result.words[9], additions ? (0x203 & ~ARITH)
            | arithmetic("add", 32, (0x7ffff800 + additions - 1) >>> 0, 1, 0).flags : 0x203, "large recipe full flag recovery");
          equal(result.words[10], additions ? ARITH : 0, "large recipe defined mask");
        }
      }));
  }
  for (const failure of ["none", "load", "store"]) for (const directEntry of [false, true]) {
    await test(`entry flag convergence preserves ${directEntry ? "raw" : "internal"} prefix and ${failure} recovery`, () => fixture([
      0x39, 0xd8, 0xeb, 12, ...Array(12).fill(0xcc),
      0x41, 0x8b, 0x16, 0x01, 0xf8, 0x89, 0x45, 0, 0x83, 0xd3, 1, 0x0f, 0x0b,
    ], async ({ map, bytes, reset, run }) => {
      const destination = 0x4000, source = failure === "load" ? 0x6000 : DATA;
      map(destination, failure === "store" ? READ : READ | WRITE, page());
      setWord(bytes(DATA), 0, 0x12345678);
      const offsets = directEntry ? [16, 17, 19, 21, 24, 27] : [0, 2, 16, 17, 19, 21, 24, 27];
      for (const initialEax of [0x7ffffffe, 0xfffffffe]) for (let budget = 0; budget <= 9; budget++) {
        bytes(destination).fill(0x5a);
        reset({ eip: CODE + offsets[0], eax: initialEax, ecx: 0x7fffffff, edx: 0x44556677,
          ebx: 2, ebp: destination, esi: source, edi: 5, flags: 0xad7 });
        const expected = [initialEax, 0x7fffffff, 0x44556677, 2, STACK + 0x800, destination, source, 5];
        let retired = 0, flags = 0xad7, kind = 1, stored = 0x5a5a5a5a;
        while (retired < budget) {
          const pc = offsets[retired];
          if (pc === 27) { kind = 8; break; }
          if (pc === 17 && failure === "load" || pc === 21 && failure === "store") { kind = 4; break; }
          if ([0, 16, 19, 24].includes(pc)) {
            const index = pc === 16 ? 1 : pc === 24 ? 3 : 0;
            const reference = arithmetic(pc === 0 ? "sub" : pc === 24 ? "adc" : "add", 32, expected[index],
              pc === 0 ? expected[3] : pc === 19 ? expected[7] : 1, pc === 24 ? flags & CF : 0);
            const mask = pc === 16 ? ARITH & ~CF : ARITH;
            flags = ((flags & ~mask) | (reference.flags & mask)) >>> 0;
            if (pc !== 0) expected[index] = reference.result;
          } else if (pc === 17) expected[2] = 0x12345678;
          else if (pc === 21) stored = expected[0];
          retired++;
        }
        const result = run(budget);
        equal(result.retired, retired, "convergent entries retire each completed instruction once");
        equal(result.kind, kind, "prefix/suffix exit reason");
        equal(result.pc, CODE + offsets[retired], "exact raw/internal continuation");
        for (let n = 0; n < 8; n++) equal(result.words[n], expected[n], `convergent entry GPR ${n}`);
        equal(result.words[9], flags, "CF survives INC/fault until the full producer completes");
        equal(new DataView(bytes(destination).buffer, bytes(destination).byteOffset, 4).getUint32(0, true), stored,
          "faults and partial budgets publish only completed stores");
      }
    }));
  }
  for (const failure of ["none", "load", "store", "journal"]) {
    await test(`completed instruction prefixes retain exact budget and ${failure} exit state`, () => fixture([
      0xb8, ...wordBytes(0x7fffffff), 0x83, 0xc0, 1, 0x83, 0xc0, 1,
      0x89, 0x03, 0x41, 0x8d, 0x54, 0x48, 4, 0x8b, 0x3e, 0x01, 0xcf, 0x0f, 0x0b,
    ], async ({ handle, map, protect, bytes, reset, run }) => {
      if (failure !== "load") map(0x4000, READ, page(wordBytes(0x12345678)));
      if (failure === "store") protect(DATA, READ);
      const offsets = [0, 5, 8, 11, 13, 14, 18, 20, 22];
      const header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4);
      for (let budget = 0; budget <= 10; budget++) {
        bytes(DATA).fill(0x5a);
        reset({ ecx: 7, ebx: DATA, esi: 0x4000, edi: 0xabcdef01, flags: 0xad7 });
        const expected = [0, 7, 0, DATA, STACK + 0x800, 0, 0x4000, 0xabcdef01];
        let retired = 0, flags = 0xad7, kind = 1, stored = 0x5a5a5a5a;
        while (retired < budget) {
          if (retired === 8) { kind = 8; break; }
          if (retired === 3 && ["store", "journal"].includes(failure)) { kind = failure === "store" ? 4 : 10; break; }
          if (retired === 6 && failure === "load") { kind = 4; break; }
          if (retired === 0) expected[0] = 0x7fffffff;
          else if ([1, 2, 4, 7].includes(retired)) {
            const index = retired === 4 ? 1 : retired === 7 ? 7 : 0;
            const reference = arithmetic("add", 32, expected[index], retired === 7 ? expected[1] : 1, 0);
            const mask = retired === 4 ? ARITH & ~CF : ARITH;
            expected[index] = reference.result;
            flags = ((flags & ~mask) | (reference.flags & mask)) >>> 0;
          } else if (retired === 3) stored = expected[0];
          else if (retired === 5) expected[2] = (expected[0] + 2 * expected[1] + 4) >>> 0;
          else if (retired === 6) expected[7] = 0x12345678;
          retired++;
        }
        const capacity = header()[2];
        if (failure === "journal") header()[2] = 0;
        const result = run(budget, () => { header()[2] = capacity; });
        equal(result.kind, kind, "completed prefix exit reason");
        equal(result.retired, retired, "only completed instructions consume budget");
        equal(result.pc, CODE + offsets[retired], "exact next or faulting PC");
        equal(result.faultPc, result.pc, "published position agrees with CPU state");
        for (let n = 0; n < 8; n++) equal(result.words[n], expected[n], `completed prefix GPR ${n}`);
        equal(result.words[9], flags, "all flags remain observable at every boundary");
        equal(result.words[10], 0xffffffff, "defined flag mask remains architectural");
        equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset, 4096).getUint32(0, true), stored, "store publication follows completion");
        if (kind === 4) {
          equal(result.address, failure === "load" ? 0x4000 : DATA, "fault address");
          equal(result.error, failure === "load" ? 4 : 7, "read/nonpresent or write/protection error");
        }
      }
    }));
  }
  for (const indirect of [false, true]) {
    const target = 0x6000;
    const call = indirect ? [0xff, 0xd5] : [0xe8, ...wordBytes(target - (CODE + 8))];
    await test(`completed prefix and ${indirect ? "indirect" : "direct"} CALL survive executable stack alias`, () => fixture(
      [0x40, 0x43, 0x49, ...call], async ({ map, alias, compile, reset, run, bytes }) => {
        map(target, READ | EXECUTE | SEALED, page([0x0f, 0x0b]));
        alias(0x5000, CODE, READ | WRITE);
        await compile();
        reset({ eax: 4, ebx: 7, ecx: 3, ebp: target, esp: 0x5004, flags: 0x203 });
        const result = run(100);
        equal(result.kind, 7, "CALL stack store exits before stale target execution");
        equal(result.retired, 4, "prefix and completed CALL retire exactly once");
        equal(result.pc, target, "SMC reports CALL target, not its sequential continuation");
        equal(result.words[0], 5, "prefix EAX retained"); equal(result.words[3], 8, "prefix EBX retained");
        equal(result.words[1], 2, "prefix ECX retained"); equal(result.words[4], 0x5000, "completed push advances ESP");
        equal(result.words[9], 0x203, "DEC preserves preceding carry at the call exit");
        equal(new DataView(bytes(CODE).buffer, bytes(CODE).byteOffset, 4096).getUint32(0, true), CODE + 3 + call.length, "committed return address is visible through the executable alias");
      }));
  }
  await test("constant and dynamic flag validity compose at every prefix and fault", () => fixture([
    0x31, 0xc0, 0x0f, 0xaf, 0xd9, 0xba, ...wordBytes(0x7fffffff),
    0x83, 0xc2, 1, 0xd3, 0xeb, 0x9e, 0x8b, 0x3e,
  ], async ({ reset, state, run }) => {
    const offsets = [0, 2, 5, 10, 13, 15, 16];
    for (const count of [0, 1, 2, 31, 32]) for (const initialMask of [0, 1, 0x400, 0xffffffff, 0x5555aaaa]) {
      const masks = [initialMask];
      masks.push(((masks.at(-1) & ~AF) | (ARITH & ~AF)) >>> 0);
      masks.push(((masks.at(-1) & ~(PF | AF | ZF | SF)) | CF | OF) >>> 0);
      masks.push(masks.at(-1)); // MOV does not define or undefine flags.
      masks.push((masks.at(-1) | ARITH) >>> 0);
      masks.push(count % 32 === 0 ? masks.at(-1) : ((masks.at(-1) & ~ARITH) | CF | PF | ZF | SF | (count === 1 ? OF : 0)) >>> 0);
      masks.push((masks.at(-1) | 0xd5) >>> 0);
      for (let budget = 0; budget <= 8; budget++) {
        reset({ ecx: count, ebx: 0x80000001, esi: 0x4000, edi: 0x12345678 });
        state()[10] = initialMask;
        const result = run(budget), completed = Math.min(budget, 6);
        equal(result.retired, completed, "flag metadata never adds retired work");
        equal(result.pc, CODE + offsets[completed], "exact mixed flag prefix");
        equal(result.kind, budget <= 6 ? 1 : 4, "budget takes precedence over the missing load");
        equal(result.words[10], masks[completed], "ordered defined and undefined flag effects");
        equal(result.words[7], 0x12345678, "future or faulting load never changes destination");
      }
    }
  }));
  await test("loop backedges preserve carry through INC/DEC until a precise later fault", () => fixture(
    [0xb9, 3, 0, 0, 0, 0x40, 0x49, 0x75, 0xfc, 0x8b, 0x13], async ({ reset, run, state, compile, map, bytes }) => {
      for (const carry of [0, 1]) {
        reset({ eax: 0x7ffffffd, ebx: DATA + 4096, edx: 0x12345678, flags: 0x202 | carry });
        const fault = run(100);
        equal(fault.kind, 4, "fault after all loop iterations"); equal(fault.pc, CODE + 9, "fault location after backedges");
        equal(fault.retired, 10, "MOV and three complete INC/DEC/JNZ iterations");
        equal(fault.words[0], 0x80000000, "loop carried EAX result"); equal(fault.words[1], 0, "loop counter reaches zero");
        equal(fault.words[2], 0x12345678, "faulting destination unchanged"); equal(fault.words[9], 0x246 | carry, "CF crosses every INC/DEC and loop edge");
      }
      // Every decoded block is an admissible entry, including after host resume.
      reset({ eax: 10, ecx: 2, ebx: DATA + 4096, flags: 0x203 }); state()[8] = CODE + 5;
      await compile(CODE + 5);
      const arbitrary = run(100); equal(arbitrary.words[0], 12, "arbitrary loop entry uses supplied GPRs");
      equal(arbitrary.retired, 6, "entry skips the initial MOV"); equal(arbitrary.words[9], 0x247, "supplied CF survives arbitrary entry");
      map(DATA + 4096, READ | WRITE, page([0x78, 0x56, 0x34, 0x12])); await compile(CODE + 9);
      const resumed = run(1); equal(resumed.words[0], 12, "fault repair never repeats the loop"); equal(resumed.words[2], 0x12345678, "repair completes only the load");
      equal(bytes(DATA)[0], 0, "unrelated mapped bytes retained");
    }));
  await test("diamond merges preserve the selected carry through INC into ADC", () => fixture(
    [0x83,0xf8,0,0x74,5,0x83,0xc0,1,0xeb,3,0x83,0xe8,1,0x41,0x83,0xd2,0,0x8b,0x3b], async ({ reset, run }) => {
      for (const input of [0, 1, 0xffffffff, 0x7fffffff]) {
        reset({ eax: input, ecx: 0xffffffff, edx: 0x7fffffff, edi: 0x76543210, ebx: DATA + 4096, flags: 0x202 });
        const chosen = arithmetic(input === 0 ? "sub" : "add", 32, input, 1, 0);
        const expected = arithmetic("adc", 32, 0x7fffffff, 0, chosen.flags & CF);
        const result = run(100);
        equal(result.kind, 4, "join is followed by a precise read fault"); equal(result.pc, CODE + 17, "fault belongs to final load");
        equal(result.retired, input === 0 ? 5 : 6, "only selected branch retires"); equal(result.words[0], chosen.result, "selected branch value retained");
        equal(result.words[1], 0, "INC completes at join"); equal(result.words[2], expected.result, "ADC consumes selected CF despite intervening INC");
        equal(result.words[7], 0x76543210, "faulting destination remains old value"); equal(result.words[9], 0x202 | expected.flags, "ADC materializes the selected producer's carry");
      }
    }));
  for (const prefix of [[], [0x66]]) await test(`SAHF ${prefix.length ? "operand prefix" : "ordinary"} transfers every AH pattern without changing other state`, () => fixture(
    [...prefix, 0x9e], async ({ reset, state, run }) => {
      for (let ah = 0; ah < 256; ah++) for (const flags of [0x202, 0xad7, 0x602, 0xed7]) for (const defined of [0, 0xffffffff]) {
        reset({ eax: (0x76540032 | ah << 8) >>> 0, ecx: 0x12345678, edx: 0x87654321, flags }); seedShrdState(state()); state()[10] = defined;
        const before = Array.from(state()), result = run(1);
        equal(result.retired, 1, "SAHF retires once"); equal(result.pc, CODE + prefix.length + 1, "prefix contributes to length only");
        equal(result.words[9], ((flags & ~0xd5) | (ah & 0xd5)) >>> 0, "only SF/ZF/AF/PF/CF receive AH bits");
        equal(result.words[10], (defined | 0xd5) >>> 0, "five written flags become defined");
        for (let n = 0; n < 100; n++) if (![8, 9, 10].includes(n)) equal(result.words[n], before[n], `SAHF retains architectural word ${n}`);
      }
    }));
  await test("SAHF preserves the preceding overflow producer for SETO and later faults", () => fixture(
    [0x05, 0x01, 0, 0, 0, 0x9e, 0x0f, 0x90, 0xc2, 0x8b, 0x0b], async ({ reset, run }) => {
      reset({ eax: 0x7fffffff, edx: 0x12345600, ebx: DATA + 4096, flags: 0x202 });
      const result = run(100), add = arithmetic("add", 32, 0x7fffffff, 1, 0);
      equal(result.kind, 4, "following read faults"); equal(result.pc, CODE + 9, "fault PC follows completed status operations"); equal(result.retired, 3, "ADD, SAHF and SETO complete");
      equal(result.words[0], 0x80000000, "ADD result retained"); equal(result.words[2], 0x12345601, "SETO consumes OF from before SAHF");
      // ADD leaves EAX=0x80000000, whose AH is zero (the sign bit is in bit 31).
      equal(result.words[9], ((0x202 & ~ARITH) | add.flags) & ~0xd5, "SAHF keeps OF while replacing AH-derived flags");
    }));
  await test("SAHF flags and preceding stores survive precise fault repair", () => fixture(
    [0xff, 0x07, 0x9e, 0x8b, 0x13], async ({ reset, state, run, bytes, map, compile }) => {
      reset({ eax: 0x1234d532, edi: STACK, ebx: DATA + 4096, flags: 0x202 });
      const fault = run(100); equal(fault.kind, 4, "later read faults"); equal(fault.pc, CODE + 3, "resume begins after SAHF"); equal(fault.retired, 2, "prefix and SAHF completed");
      equal(fault.words[9] & 0xd5, 0xd5, "completed SAHF flags visible at fault"); equal(bytes(STACK)[0], 1, "prior store retained");
      map(DATA + 4096, READ | WRITE, page([0x12, 0x34, 0x56, 0x78])); state()[0] = 0; await compile(CODE + 3);
      const repaired = run(1); equal(repaired.words[2], 0x78563412, "faulting load alone resumes"); equal(repaired.words[9], fault.words[9], "changed AH cannot repeat prior SAHF"); equal(bytes(STACK)[0], 1, "prefix never replays");
    }));
  for (const [name, encoding, sourceBits] of [["CWDE", [0x98], 16], ["CBW", [0x66, 0x98], 8]]) {
    const converted = eax => sourceBits === 16 ? Number(BigInt.asUintN(32, BigInt.asIntN(16, BigInt(eax))))
      : ((eax & 0xffff0000) | Number(BigInt.asUintN(16, BigInt.asIntN(8, BigInt(eax))))) >>> 0;
    await test(`${name} exhausts its source bits with unrelated upper bits`, () => fixture(
      encoding, async ({ reset, run }) => {
        for (let low = 0; low < 2 ** sourceBits; low++) {
          const eax = ((sourceBits === 16 ? 0xa35c0000 : 0xa35c5a00) | low) >>> 0;
          const flags = 0x200202 | (low & 0xcd5);
          reset({ eax, edx: 0x12345678, flags }); const result = run(1);
          equal(result.words[0], converted(eax), "architectural sign extension");
          equal(result.words[2], 0x12345678, "conversion does not write DX/EDX");
          equal(result.words[9], flags, "full flags preserved for every input");
          equal(result.retired, 1, "exactly one conversion retired");
        }
      }));
    await test(`${name} preserves all state and flag validity at zero and one instruction budgets`, () => fixture(
      encoding, async ({ reset, state, run }) => {
        const flagBits = [CF, PF, AF, ZF, SF, OF];
        for (const eax of [0, 1, 0x7f, 0x80, 0xff, 0x7fff, 0x8000, 0xffff, 0x12340080, 0x12348000, 0xffffffff])
          for (let pattern = 0; pattern < 64; pattern++) for (const df of [0, 0x400]) {
            const flags = 0x200202 | df | flagBits.reduce((v, bit, n) => v | ((pattern >>> n & 1) ? bit : 0), 0);
            reset({ eax, ecx: 0x81234567, edx: 0x76543210, ebx: DATA, ebp: STACK, esi: DATA + 4, edi: DATA + 8, flags });
            seedShrdState(state()); state()[10] = pattern & 1 ? 0 : 0x800008d5;
            const before = Array.from(state()), zero = run(0);
            equal(zero.retired, 0, "zero budget leaves conversion unretired");
            for (let n = 0; n < 100; n++) equal(zero.words[n], before[n], `${name} idle word ${n}`);
            const result = run(1); equal(result.retired, 1, "one instruction completes");
            for (let n = 0; n < 100; n++) equal(result.words[n], n === 0 ? converted(eax) : n === 8 ? CODE + encoding.length : before[n], `${name} complete word ${n}`);
          }
      }));
    await test(`${name} retains a completed store and conversion across subsequent read fault repair`, () => fixture(
      [0xff, 0x07, ...encoding, 0x8b, 0x13], async ({ reset, state, run, bytes, map, compile }) => {
        reset({ eax: 0x12348080, edi: STACK, ebx: DATA + 4096, flags: 0x203 }); seedShrdState(state());
        const fault = run(100); equal(fault.kind, 4, "following read faults"); equal(fault.pc, CODE + 2 + encoding.length, "fault belongs to read after conversion");
        equal(fault.faultPc, fault.pc, "precise read fault PC"); equal(fault.address, DATA + 4096, "precise read address"); equal(fault.retired, 2, "store and conversion completed");
        equal(fault.words[0], converted(0x12348080), "conversion visible at fault"); equal(bytes(STACK)[0], 1, "store completed exactly once");
        map(DATA + 4096, READ | WRITE, page([0x12, 0x34, 0x56, 0x78])); state()[0] = 0x24680080; await compile(fault.pc);
        const repaired = run(1); equal(repaired.retired, 1, "only failed read resumes"); equal(repaired.words[2], 0x78563412, "read repair result");
        equal(repaired.words[0], 0x24680080, "conversion was not replayed"); equal(repaired.words[9], fault.words[9], "completed flags preserved"); equal(bytes(STACK)[0], 1, "preceding store was not replayed");
      }));
    for (const missing of [false, true]) await test(`${name} completes before ${missing ? "missing" : "NX"} next-page fetch fault`, () => fixture(
      [0x90], async ({ map, compile, reset, state, run, protect }) => {
        const base = missing ? 0x5000 : CODE, nextPage = base + 4096, entry = nextPage - encoding.length;
        const code = page([], 0xcc); code.set(encoding, 4096 - encoding.length); map(base, READ | EXECUTE | SEALED, code);
        reset({ eax: 0x12348080, eip: entry, flags: 0x200ed7 }); seedShrdState(state()); const before = Array.from(state()); await compile(entry);
        const fault = run(100); equal(fault.kind, 4, "next page instruction fetch fault"); equal(fault.error, missing ? 20 : 21, "fetch protection code");
        equal(fault.retired, 1, "conversion completed before fetch failure"); equal(fault.faultPc, nextPage, "fault belongs to next instruction"); equal(fault.address, nextPage, "unavailable fetch address");
        for (let n = 0; n < 100; n++) equal(fault.words[n], n === 0 ? converted(before[0]) : n === 8 ? nextPage : before[n], `next fetch preserves word ${n}`);
        if (!missing) protect(nextPage, READ | EXECUTE | SEALED);
        map(nextPage, READ | EXECUTE | SEALED, page([0x90], 0xcc)); await compile(nextPage); state()[0] = 0x24680080;
        const resumed = run(1); equal(resumed.words[0], 0x24680080, "repair cannot repeat conversion"); equal(resumed.pc, nextPage + 1, "next instruction alone resumes"); equal(resumed.words[9], before[9], "flags survive repair");
      }));
    await test(`${name} non-executable instruction is unretired until mapping repair`, () => fixture(
      encoding, async ({ reset, state, run, protect, compile }) => {
        reset({ eax: 0x12348080, flags: 0xad7 }); seedShrdState(state()); const before = Array.from(state());
        const fault = run(100); equal(fault.kind, 4, "conversion fetch requires execute permission"); equal(fault.error, 21, "NX fetch fault"); equal(fault.retired, 0, "conversion did not execute");
        for (let n = 0; n < 100; n++) equal(fault.words[n], before[n], `NX conversion word ${n}`);
        protect(CODE, READ | EXECUTE | SEALED); await compile(CODE); const resumed = run(1);
        equal(resumed.words[0], converted(before[0]), "repaired instruction executes"); equal(resumed.words[9], before[9], "flags unchanged after repair"); equal(resumed.retired, 1, "repair completes once");
      }, READ | SEALED));
    for (const [prefix, kind] of [[0xf0, 8], [0x67, 3], [0xf2, 3], [0xf3, 3]]) await test(`${name} prefix ${prefix.toString(16)} fails without architectural mutation`, () => fixture(
      [prefix, ...encoding], async ({ reset, state, run }) => {
        reset({ eax: 0x12348080, flags: 0xad7 }); seedShrdState(state()); const before = Array.from(state()), result = run(100);
        equal(result.kind, kind, "illegal LOCK or unsupported prefix is explicit"); equal(result.retired, 0, "invalid conversion is unretired");
        for (let n = 0; n < 100; n++) equal(result.words[n], before[n], `rejected conversion word ${n}`);
      }));
    await test(`${name} completion wraps EIP at 32 bits`, () => fixture(
      [0x90], async ({ map, compile, reset, run }) => {
        const code = page([], 0xcc), entry = (0x100000000 - encoding.length) >>> 0; code.set(encoding, 4096 - encoding.length);
        map(0xfffff000, READ | EXECUTE | SEALED, code); await compile(entry, 1); reset({ eip: entry, eax: 0x12348080, flags: 0xed7 });
        const result = run(1); equal(result.pc, 0, "complete instruction wraps exactly once"); equal(result.words[0], converted(0x12348080), "wrapped conversion result"); equal(result.words[9], 0xed7, "wrapped flags unchanged"); equal(result.retired, 1, "wrapped instruction retired");
      }));
    await test(`${name} preserves live arithmetic flags consumed by SETZ`, () => fixture(
      [0x83, 0xc3, 1, ...encoding, 0x0f, 0x94, 0xc2], async ({ reset, run }) => {
        for (const ebx of [0xffffffff, 0x7fffffff, 0]) {
          reset({ eax: 0x12348080, ebx, edx: 0x12345600, flags: 0x202 }); const result = run(3);
          const expected = arithmetic("add", 32, ebx, 1, 0);
          equal(result.words[0], converted(0x12348080), "conversion remains in arithmetic chain"); equal(result.words[2], 0x12345600 | Number(expected.result === 0), "SETZ consumes flags from prior ADD");
          equal(result.words[9], 0x202 | expected.flags, "all preceding ADD flags survive conversion"); equal(result.retired, 3, "ADD conversion SETZ all retire");
        }
      }));
  }
  await test("CBW split fetch preserves source until the opcode page is executable", () => fixture(
    [0x90], async ({ reset, state, map, compile, run, protect }) => {
      const code = page([], 0xcc); code[4095] = 0x66; map(CODE, READ | EXECUTE | SEALED, code);
      reset({ eip: CODE + 4095, eax: 0x12340080, flags: 0xad7 }); seedShrdState(state()); const before = Array.from(state()); await compile(CODE + 4095);
      const fault = run(100); equal(fault.kind, 4, "opcode page fetch faults"); equal(fault.error, 21, "NX opcode page"); equal(fault.address, DATA, "exact missing byte"); equal(fault.faultPc, CODE + 4095, "fault instruction begins at operand prefix"); equal(fault.retired, 0, "prefix alone never retires");
      for (let n = 0; n < 100; n++) equal(fault.words[n], before[n], `partial conversion fetch word ${n}`);
      protect(DATA, READ | EXECUTE | SEALED); map(DATA, READ | EXECUTE | SEALED, page([0x98, 0x90], 0xcc)); await compile(CODE + 4095);
      const repaired = run(1); equal(repaired.words[0], 0x1234ff80, "CBW preserves upper16 and sign extends AL only"); equal(repaired.pc, DATA + 1, "whole prefixed instruction completed"); equal(repaired.words[9], before[9], "flags unchanged after split fetch repair"); equal(repaired.retired, 1, "complete conversion retires once");
    }));
  for (const encoding of [[0x66, 0x66, 0x98], [0x64, 0x66, 0x98]]) await test(`CBW legal redundant/segment prefixes ${encoding.join("-")} retain operand size`, () => fixture(
    encoding, async ({ reset, run }) => {
      reset({ eax: 0x12340080, fs: 0xfffff000, flags: 0xed7 }); const result = run(1);
      equal(result.words[0], 0x1234ff80, "prefix does not turn CBW into CWDE or a memory operation"); equal(result.pc, CODE + encoding.length, "all prefix bytes accounted"); equal(result.words[9], 0xed7, "prefix has no flags effect"); equal(result.retired, 1, "prefixed conversion retires once");
    }));
  await test("CBW then CWDE consumes the newly extended AX", () => fixture(
    [0x66, 0x98, 0x98], async ({ reset, run }) => {
      for (const eax of [0x12340080, 0x1234807f, 0x123400ff]) {
        reset({ eax, flags: 0xed7 }); const result = run(2);
        equal(result.words[0], Number(BigInt.asUintN(32, BigInt.asIntN(8, BigInt(eax)))), "both conversions use current register values"); equal(result.words[9], 0xed7, "chain preserves flags"); equal(result.retired, 2, "chain retires twice");
      }
    }));
  for (const [name, opcode] of [["LOOPNE", 0xe0], ["LOOPE", 0xe1], ["LOOP", 0xe2], ["JECXZ", 0xe3]]) {
    await test(`${name} uses the full count and preserves all flags and other architectural state`, () => fixture(
      [opcode, 2, 0x90, 0x90, 0x90], async ({ reset, state, run }) => {
        const flagBits = [CF, PF, AF, ZF, SF, OF];
        for (const count of [0, 1, 2, 0xffffffff, 0x10000, 0x10001, 0x80000000]) for (let pattern = 0; pattern < 64; pattern++) for (const df of [0, 0x400]) {
          const flags = 0x200202 | df | flagBits.reduce((v, bit, n) => v | ((pattern >>> n & 1) ? bit : 0), 0);
          reset({ eax: 0x12345678, ecx: count, edx: 0x87654321, ebx: DATA, ebp: 0x56781234, esi: 0x13572468, edi: 0x24681357, flags });
          seedShrdState(state()); state()[10] = pattern & 1 ? 0 : 0x800008d5;
          const before = Array.from(state());
          const zero = run(0); equal(zero.retired, 0, "zero budget does not consume the count");
          for (let n = 0; n < 100; n++) equal(zero.words[n], before[n], `${name} zero-budget word ${n}`);
          const result = run(1), nextCount = opcode === 0xe3 ? count : (count - 1) >>> 0;
          const taken = opcode === 0xe3 ? count === 0 : nextCount !== 0 && (opcode === 0xe2 || Boolean(flags & ZF) === (opcode === 0xe1));
          equal(result.retired, 1, "count branch retires exactly once");
          for (let n = 0; n < 100; n++) equal(result.words[n], n === 1 ? nextCount : n === 8 ? CODE + (taken ? 4 : 2) : before[n], `${name} architectural word ${n}`);
        }
      }));
    for (const missing of [false, true]) await test(`${name} retains a completed prefix and count across ${missing ? "missing" : "NX"} next-target fetch repair`, () => fixture(
      [0x90], async ({ reset, state, run, map, bytes, compile, protect }) => {
        const base = missing ? 0x5000 : CODE, nextPage = base + 4096, target = nextPage + 2;
        const code = page([], 0xcc); code.set([0xff, 0x07, opcode, 2], 4092); map(base, READ | EXECUTE | SEALED, code);
        const initial = opcode === 0xe1 ? 0xffffffff : 0;
        setWord(bytes(STACK), 0, initial);
        reset({ ecx: opcode === 0xe3 ? 0 : 2, edi: STACK, eax: 0x12345678, eip: base + 4092, flags: 0x203 });
        seedShrdState(state()); await compile(base + 4092);
        const fault = run(100), expectedCount = opcode === 0xe3 ? 0 : 1;
        equal(fault.kind, 4, "next target instruction fetch faults"); equal(fault.error, missing ? 20 : 21, "fetch presence and execute error");
        equal(fault.pc, target, "fault belongs to target, not completed branch"); equal(fault.faultPc, target, "precise fault instruction"); equal(fault.address, target, "first unavailable byte");
        equal(fault.retired, 2, "prefix and count branch completed"); equal(fault.words[1], expectedCount, "completed count change survives fetch fault");
        equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset, 4096).getUint32(0, true), (initial + 1) >>> 0, "prefix store completed exactly once");
        const fixed = page([], 0xcc); fixed[2] = 0x90;
        if (!missing) protect(nextPage, READ | EXECUTE | SEALED);
        map(nextPage, READ | EXECUTE | SEALED, fixed); await compile(target);
        const repaired = run(1); equal(repaired.retired, 1, "repair executes target NOP alone"); equal(repaired.pc, target + 1, "resume advances target only");
        for (let n = 0; n < 100; n++) equal(repaired.words[n], n === 8 ? target + 1 : fault.words[n], `repair preserves completed word ${n}`);
        equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset, 4096).getUint32(0, true), (initial + 1) >>> 0, "repair never repeats prefix store");
      }));
    await test(`${name} signed relative targets and fallthrough wrap at 32 bits`, () => fixture(
      [0x90], async ({ reset, state, map, compile, run }) => {
        for (const [entry, displacement, target] of [[0xfffffffe, 0x7f, 0x7f], [0, 0x80, 0xffffff82]]) {
          const code = page([], 0xcc); code.set([opcode, displacement], entry & 4095); map((entry & 0xfffff000) >>> 0, READ | EXECUTE | SEALED, code);
          reset({ ecx: opcode === 0xe3 ? 0 : 2, eip: entry, flags: opcode === 0xe1 ? 0x242 : 0x202 }); seedShrdState(state());
          await compile(entry, 1); const before = Array.from(state()), result = run(1);
          equal(result.retired, 1, "wrapped branch retires"); equal(result.pc, target, "signed rel8 plus EIP wraps once at 32 bits");
          for (let n = 0; n < 100; n++) equal(result.words[n], n === 1 ? (opcode === 0xe3 ? 0 : 1) : n === 8 ? target : before[n], `wrapped branch word ${n}`);
        }
      }));
    for (const [prefix, kind] of [[0x67, 3], [0x66, 3], [0xf0, 8]]) await test(`${name} prefix ${prefix.toString(16)} exits before consuming count`, () => fixture(
      [prefix, opcode, 0], async ({ reset, state, run }) => {
        reset({ ecx: 0xffffffff, flags: 0xad7 }); seedShrdState(state()); const before = Array.from(state()), result = run(100);
        equal(result.kind, kind, "unimplemented size or illegal LOCK is explicit"); equal(result.retired, 0, "count instruction is not completed");
        for (let n = 0; n < 100; n++) equal(result.words[n], before[n], `unsupported prefix preserves word ${n}`);
      }));
  }
  await test("LOOP keeps exact iteration progress at every partial budget", () => fixture(
    [0xff, 0x03, 0xe2, 0xfc], async ({ reset, state, bytes, run }) => {
      for (let budget = 0; budget <= 6; budget++) {
        setWord(bytes(DATA), 0, 0); reset({ ecx: 3, ebx: DATA, flags: 0x203 }); seedShrdState(state());
        const result = run(budget), stores = Math.ceil(budget / 2);
        equal(result.retired, budget, "exact instruction budget"); equal(result.words[1], 3 - Math.floor(budget / 2), "only completed LOOP instructions decrement");
        equal(result.pc, budget === 6 ? CODE + 4 : CODE + (budget & 1 ? 2 : 0), "resume identifies precise loop prefix");
        equal(bytes(DATA)[0], stores, "one store per completed body");
        const flags = stores ? (0x203 & ~(ARITH & ~CF)) | (arithmetic("add", 32, stores - 1, 1, 0).flags & ~CF) : 0x203;
        equal(result.words[9], flags, "LOOP does not overwrite flags from the body");
      }
    }));
  await test("LOOP split instruction fetch does not decrement before repair", () => fixture(
    [0x90], async ({ reset, state, map, compile, run, protect }) => {
      const code = page([], 0xcc); code[4095] = 0xe2; map(CODE, READ | EXECUTE | SEALED, code);
      reset({ eip: CODE + 4095, ecx: 0, flags: 0xad7 }); seedShrdState(state()); const before = Array.from(state()); await compile(CODE + 4095);
      const fault = run(100); equal(fault.kind, 4, "missing displacement fetch faults"); equal(fault.error, 21, "NX displacement page"); equal(fault.address, DATA, "fault at displacement byte"); equal(fault.retired, 0, "partial decode never retires");
      for (let n = 0; n < 100; n++) equal(fault.words[n], before[n], `partial fetch preserves word ${n}`);
      protect(DATA, READ | EXECUTE | SEALED); map(DATA, READ | EXECUTE | SEALED, page([0, 0x90], 0xcc)); await compile(CODE + 4095);
      const repaired = run(1); equal(repaired.retired, 1, "repaired instruction retires once"); equal(repaired.words[1], 0xffffffff, "zero count wraps only after successful fetch"); equal(repaired.pc, DATA + 1, "relative zero target is full next EIP"); equal(repaired.words[9], before[9], "flags stay untouched");
    }));
  if (typedKernelCount >= 23) {
    const cosTag = ([sig, exp]) => !(exp & 0x7fff) && sig === 0n ? 1
      : !(exp & 0x7fff) || (exp & 0x7fff) === 0x7fff || sig < 0x8000000000000000n ? 2 : 0;
    const cosPut = (words, top, [sig, exp]) => words.set([Number(sig & 0xffffffffn), Number(sig >> 32n), exp], 35 + top * 3);
    const seedCos = (words, input, control, top = 0) => {
      for (let r = 0; r < 8; r++) words.set([r + 11, 0x80000000, 0x3fff], 35 + r * 3);
      cosPut(words, top, input);
      words[59] = control; words[60] = (top << 11) | 0x4700;
      words[61] = (0xffff & ~(3 << (top * 2))) | (cosTag(input) << (top * 2));
      words[62] = 0x321; words[63] = 0x11223344; words[64] = 0x1b;
      words[65] = 0x55667788; words[66] = 0x23; words[15] = 0x1b;
      for (let n = 67; n < 100; n++) words[n] = n === 99 ? 0x1f80 : (n * 0x1234567) >>> 0;
    };
    const cosExpected = (before, value, status, pc, commit = true) => {
      const expected = Uint32Array.from(before), top = before[60] >> 11 & 7;
      expected[8] = pc + 2; expected[60] = status;
      expected[62] = 0x1ff; expected[63] = pc; expected[64] = before[15];
      if (commit) {
        cosPut(expected, top, value);
        expected[61] = (before[61] & ~(3 << (top * 2))) | (cosTag(value) << (top * 2));
      }
      return expected;
    };
    const checkCos = (actual, expected) => {
      equal(actual.retired, 1, "FCOS retires exactly once without popping");
      for (let n = 0; n < 100; n++) equal(actual.words[n], expected[n], `FCOS complete architectural word ${n}`);
    };
    await test("FCOS signed zero preserves sticky precision, every TOP and all PC/RC", () => fixture(
      [0xd9, 0xff], async ({ reset, state, run }) => {
        for (const sign of [0, 0x8000]) for (const pc of [0, 2, 3]) for (const rc of [0, 1, 2, 3]) for (let top = 0; top < 8; top++) {
          reset({ eax: 0x76543210, flags: 0xad7 }); seedCos(state(), [0n, sign], 0x7f | pc << 8 | rc << 10, top); state()[60] |= 32;
          const before = Array.from(state()), zero = run(0);
          for (let n = 0; n < 100; n++) equal(zero.words[n], before[n], "zero budget preserves full state");
          checkCos(run(1), cosExpected(before, [0x8000000000000000n, 0x3fff], (top << 11) | 0x4120, CODE));
        }
      }));
    // Independently certified rational Machin-pi / Taylor intervals for
    // cos(x*pi/p), p = 0xc90fdaa22168c234c * 2^-66. This is the declared Intel
    // numerical contract, not a claim of bit identity with every native CPU.
    const cosSamples = [
      {name:"one",input:[0x8000000000000000n,16383],rounded:[[0x8a51407da8345c92n,16382,1],[0x8a51407da8345c91n,16382,0],[0x8a51407da8345c92n,16382,1],[0x8a51407da8345c91n,16382,0]]},
      {name:"negative-one",input:[0x8000000000000000n,49151],rounded:[[0x8a51407da8345c92n,16382,1],[0x8a51407da8345c91n,16382,0],[0x8a51407da8345c92n,16382,1],[0x8a51407da8345c91n,16382,0]]},
      {name:"two",input:[0x8000000000000000n,16384],rounded:[[0xd51132ba9b902522n,49149,1],[0xd51132ba9b902522n,49149,1],[0xd51132ba9b902521n,49149,0],[0xd51132ba9b902521n,49149,0]]},
      {name:"tiny-2m32",input:[0x8000000000000000n,16351],rounded:[[0x8000000000000000n,16383,1],[0xffffffffffffffffn,16382,0],[0x8000000000000000n,16383,1],[0xffffffffffffffffn,16382,0]]},
      {name:"p-half-lower",input:[0xc90fdaa22168c234n,16383],rounded:[[0xc000000000000000n,16319,0],[0xc000000000000000n,16319,0],[0xc000000000000001n,16319,1],[0xc000000000000000n,16319,0]]},
      {name:"p-half-upper",input:[0xc90fdaa22168c235n,16383],rounded:[[0x8000000000000000n,49086,0],[0x8000000000000001n,49086,1],[0x8000000000000000n,49086,0],[0x8000000000000000n,49086,0]]},
    ];
    for (const sample of cosSamples) await test(`FCOS certified finite ${sample.name} preserves all unrelated state`, () => fixture(
      [0xd9, 0xff], async ({ reset, state, run }) => {
        for (const pc of [0, 2, 3]) for (const rc of [0, 1, 2, 3]) for (const top of [0, 6, 7]) {
          reset({ flags: 0xad7 }); seedCos(state(), sample.input, 0x7f | pc << 8 | rc << 10, top);
          const before = Array.from(state()), [sig, exp, up] = sample.rounded[rc];
          checkCos(run(1), cosExpected(before, [sig, exp], (top << 11) | 0x4120 | up << 9, CODE));
        }
      }));
    await test("FCOS out-of-range keeps raw ST0 and exposes C2 through FNSTSW/SAHF", () => fixture(
      [0xd9, 0xff, 0xdf, 0xe0, 0x9e], async ({ reset, state, run, compile, select }) => {
        await compile(CODE + 2);
        for (const input of [[0x8000000000000000n,0x403e],[0x8000000000000000n,0xc03e],[0xffffffffffffffffn,0x7ffe],[0xffffffffffffffffn,0xfffe]]) {
          reset({ eax: 0x76540000, flags: 0xad7 }); seedCos(state(), input, 0x35f, 7); select(CODE);
          const before = Array.from(state());
          checkCos(run(1), cosExpected(before, input, (7 << 11) | 0x4500, CODE, false));
          select(CODE + 2); const observed = run(2);
          equal(observed.retired, 2, "status read and SAHF complete"); equal(observed.words[0] & 0x400, 0x400, "FNSTSW returns C2");
          equal(observed.words[9] & 4, 4, "SAHF exposes C2 as PF"); equal(observed.words[0] >>> 16, 0x7654, "FNSTSW preserves upper EAX");
        }
      }));
    const cosSpecials = [
      {name:"quiet NaN",input:[0xc000000000000123n,0xffff],value:[0xc000000000000123n,0xffff],flags:0},
      {name:"signaling NaN",input:[0x8000000000000123n,0x7fff],value:[0xc000000000000123n,0x7fff],flags:1},
      {name:"unsupported encoding",input:[0x4000000000000000n,0x3fff],value:[0xc000000000000000n,0xffff],flags:1},
      {name:"positive infinity",input:[0x8000000000000000n,0x7fff],value:[0xc000000000000000n,0xffff],flags:1},
      {name:"negative infinity",input:[0x8000000000000000n,0xffff],value:[0xc000000000000000n,0xffff],flags:1},
    ];
    for (const sample of cosSpecials) await test(`FCOS ${sample.name} preserves precise suppression and metadata`, () => fixture(
      [0xd9, 0xff], async ({ reset, state, run }) => {
        for (const masked of [false, true]) for (const top of [0, 7]) {
          reset({ flags: 0xad7 }); seedCos(state(), sample.input, masked ? 0x37f : 0x37e, top);
          const before = Array.from(state()), suppressed = !masked && sample.flags !== 0;
          checkCos(run(1), cosExpected(before, sample.value, (top << 11) | 0x4100 | sample.flags | (suppressed ? 0x8080 : 0), CODE, !suppressed));
        }
      }));
    await test("FCOS denormal and pseudo-denormal keep directed near-one precision and DE priority", () => fixture(
      [0xd9, 0xff], async ({ reset, state, run }) => {
        for (const input of [[1n,0],[0x8000000000000000n,0]]) for (const pc of [0,2,3]) for (const rc of [0,1,2,3]) for (const unmask of [0,2]) {
          reset({ flags: 0xad7 }); seedCos(state(), input, (0x7f | pc << 8 | rc << 10) & ~unmask, 7);
          const before = Array.from(state()), up = rc === 0 || rc === 2;
          const value = up ? [0x8000000000000000n,0x3fff] : [0xffffffffffffffffn,0x3ffe];
          const status = (7 << 11) | 0x4100 | (unmask ? 0x8082 : 0x22 | (up ? 0x200 : 0));
          checkCos(run(1), cosExpected(before, value, status, CODE, !unmask));
        }
      }));
    await test("FCOS empty stack has no pop and clears C2 even for unmasked invalid", () => fixture(
      [0xd9, 0xff], async ({ reset, state, run }) => {
        for (const masked of [false, true]) for (const top of [0, 7]) {
          reset(); seedCos(state(), [0x8000000000000000n,0x3fff], masked ? 0x37f : 0x37e, top); state()[61] = 0xffff;
          const before = Array.from(state());
          checkCos(run(1), cosExpected(before, [0xc000000000000000n,0xffff], (top << 11) | 0x4141 | (masked ? 0 : 0x8080), CODE, masked));
        }
      }));
    await test("FCOS pending MF preserves completed store and resumes only the faulting instruction", () => fixture(
      [0xff, 0x03, 0xd9, 0xff], async ({ reset, state, run, bytes, compile }) => {
        reset({ ebx: DATA }); seedCos(state(), [0n,0], 0x35f); state()[60] = 0x80a0;
        const before = Array.from(state()), fault = run(100);
        equal(fault.kind, 14, "waiting FCOS delivers pending MF"); equal(fault.pc, CODE + 2, "precise FCOS fault PC"); equal(fault.retired, 1, "prefix alone retires");
        for (let n = 35; n < 100; n++) equal(fault.words[n], before[n], "pending MF cannot mutate FP or vector state");
        equal(bytes(DATA)[0], 1, "prefix store completed once"); state()[59] = 0x37f; state()[60] = 0; await compile(CODE + 2);
        const repaired = run(1); equal(repaired.retired, 1, "repaired FCOS alone retires"); equal(bytes(DATA)[0], 1, "no prefix replay");
        equal(extRegister(repaired.words,0).significand, 0x8000000000000000n, "repaired cosine is one"); equal(repaired.words[61], 0xfffc, "one physical destination remains occupied");
      }));
    await test("FCOS unmasked precision commits before the next waiting memory fault", () => fixture(
      [0xd9, 0xff, 0x40, 0xd9, 0x02], async ({ reset, state, run }) => {
        reset({ eax: 7, edx: 0xffffffff }); seedCos(state(), [0x8000000000000000n,0x3fff], 0x35f, 7); state()[60] = 7 << 11;
        const result = run(100); equal(result.kind, 14, "pending precision precedes missing memory"); equal(result.error, 32, "precision MF cause");
        equal(result.pc, CODE + 3, "next waiting operation faults"); equal(result.retired, 2, "cosine and integer operation complete"); equal(result.words[0], 8, "integer operation executes once");
        equal(extRegister(result.words,7).significand, 0x8a51407da8345c92n, "precision result committed"); equal(result.words[60], (7 << 11) | 0x82a0, "committed C1 and pending precision");
        equal(result.words[61], 0x3fff, "FCOS does not pop"); equal(result.words[63], CODE, "causing FCOS metadata survives MF");
      }));
    if (options.typedNumeric) await test("FCOS reserved precision preserves state and exact restart without replay", () => fixture(
      [0xff, 0x03, 0xd9, 0xff], async ({ reset, state, run, bytes, compile }) => {
        reset({ ebx: DATA }); seedCos(state(), [0n,0], 0x17f); const before = Array.from(state()), fault = run(100);
        equal(fault.kind, 3, "reserved precision is explicit unsupported"); equal(fault.error, 1, "numeric error transported immediately");
        equal(fault.pc, CODE + 2, "exact failing FCOS"); equal(fault.retired, 1, "only prefix retires");
        for (let n = 35; n < 100; n++) equal(fault.words[n], before[n], "failed numerical operation cannot partially commit");
        state()[59] = 0x37f; await compile(CODE + 2); const repaired = run(1);
        equal(repaired.retired, 1, "repaired cosine alone executes"); equal(bytes(DATA)[0], 1, "no repeated prefix write"); equal(repaired.words[61], 0xfffc, "one raw destination commits");
      }));
    await test("FCOS completed result survives a later raw80 store fault and repair", () => fixture(
      [0xff, 0x07, 0xd9, 0xff, 0xdb, 0x3b], async ({ reset, state, run, bytes, map, compile }) => {
        reset({ edi: STACK, ebx: DATA + 4091 }); seedCos(state(), [0n,0], 0x37f, 7);
        const fault = run(100); equal(fault.kind, 4, "following store crosses missing page"); equal(fault.pc, CODE + 4, "only store resumes"); equal(fault.retired, 2, "prefix and cosine completed");
        equal(fault.words[61], 0x3fff, "fault cannot pop FCOS result"); equal(bytes(STACK)[0], 1, "prefix effect completed once");
        map(DATA + 4096, READ | WRITE); await compile(CODE + 4); const resumed = run(1);
        equal(resumed.retired, 1, "repaired store alone retires"); equal(resumed.words[61], 0xffff, "one store pop completes"); equal(bytes(STACK)[0], 1, "earlier store never replayed");
        const raw = extBytes(0x8000000000000000n,0x3fff); for (let n = 0; n < 10; n++) equal(bytes(DATA + (4091 + n & ~4095))[(4091 + n) & 4095], raw[n], "resumed raw80 output");
      }));
  }
  if (typedKernelCount >= 24) {
    const sinTag = ([sig, exp]) => !(exp & 0x7fff) && sig === 0n ? 1
      : !(exp & 0x7fff) || (exp & 0x7fff) === 0x7fff || sig < 0x8000000000000000n ? 2 : 0;
    const putSin = (words, top, [sig, exp]) => words.set([Number(sig & 0xffffffffn), Number(sig >> 32n), exp], 35 + top * 3);
    const seedSin = (words, input, control, top = 0) => {
      for (let r = 0; r < 8; r++) words.set([r + 17, 0x80000000, 0x3fff], 35 + r * 3);
      putSin(words, top, input);
      words[59] = control; words[60] = (top << 11) | 0x4700;
      words[61] = (0xffff & ~(3 << (top * 2))) | (sinTag(input) << (top * 2));
      words[62] = 0x321; words[63] = 0x11223344; words[64] = 0x1b;
      words[65] = 0x55667788; words[66] = 0x23; words[15] = 0x1b;
      for (let n = 67; n < 100; n++) words[n] = n === 99 ? 0x1f80 : (n * 0x1234567) >>> 0;
    };
    const sinExpected = (before, value, status, pc = CODE, commit = true) => {
      const expected = Uint32Array.from(before), top = before[60] >> 11 & 7;
      expected[8] = pc + 2; expected[60] = status; expected[62] = 0x1fe; expected[63] = pc; expected[64] = before[15];
      if (commit) { putSin(expected, top, value); expected[61] = (before[61] & ~(3 << (top * 2))) | (sinTag(value) << (top * 2)); }
      return expected;
    };
    const checkSin = (actual, expected) => {
      equal(actual.retired, 1, "FSIN retires exactly once without popping");
      for (let n = 0; n < 100; n++) equal(actual.words[n], expected[n], `FSIN complete architectural word ${n}`);
    };
    await test("FSIN signed zero preserves its sign, sticky precision, every TOP and PC/RC", () => fixture(
      [0xd9, 0xfe], async ({ reset, state, run }) => {
        for (const sign of [0, 0x8000]) for (const pc of [0, 2, 3]) for (const rc of [0, 1, 2, 3]) for (let top = 0; top < 8; top++) {
          reset({ eax: 0x76543210, flags: 0xad7 }); seedSin(state(), [0n, sign], 0x7f | pc << 8 | rc << 10, top); state()[60] |= 32;
          const before = Array.from(state()), zero = run(0);
          for (let n = 0; n < 100; n++) equal(zero.words[n], before[n], "zero budget preserves full state");
          checkSin(run(1), sinExpected(before, [0n, sign], (top << 11) | 0x4120));
        }
      }));
    // Independent rational intervals for sin(x*pi/p), p=0xc90fdaa22168c234c*2^-66.
    // These are the chosen precision contract, not a native CPU approximation.
    for (const sample of [
      { name: "one", input: [0x8000000000000000n, 0x3fff], sig: [0xd76aa47848677021n, 0xd76aa47848677020n, 0xd76aa47848677021n, 0xd76aa47848677020n], exp: 0x3ffe, up: [1, 0, 1, 0] },
      { name: "negative one", input: [0x8000000000000000n, 0xbfff], sig: [0xd76aa47848677021n, 0xd76aa47848677021n, 0xd76aa47848677020n, 0xd76aa47848677020n], exp: 0xbffe, up: [1, 1, 0, 0] },
      { name: "tiny normal", input: [0x8000000000000000n, 0x3fbf], sig: [0x8000000000000000n, 0x8000000000000000n, 0x8000000000000001n, 0x8000000000000000n], exp: 0x3fbf, up: [0, 0, 1, 0] },
    ]) await test(`FSIN certified ${sample.name} retains full precision under every PC/RC`, () => fixture(
      [0xd9, 0xfe], async ({ reset, state, run }) => {
        for (const pc of [0, 2, 3]) for (const rc of [0, 1, 2, 3]) for (const top of [0, 7]) {
          reset({ flags: 0xad7 }); seedSin(state(), sample.input, 0x7f | pc << 8 | rc << 10, top);
          const before = Array.from(state());
          checkSin(run(1), sinExpected(before, [sample.sig[rc], sample.exp], (top << 11) | 0x4120 | sample.up[rc] << 9));
        }
      }));
    await test("FSIN underflow uses unbounded rounded exponent and commits a biased result when unmasked", () => fixture(
      [0xd9, 0xfe], async ({ reset, state, run }) => {
        for (const large of [false, true]) for (const rc of [0, 1, 2, 3]) for (const unmask of [0, 2, 16, 18, 32]) {
          reset({ flags: 0xad7 }); seedSin(state(), [large ? 0x7fffffffffffffffn : 1n, 0], (0x37f | rc << 10) & ~unmask, 7);
          const before = Array.from(state()), up = rc === 2, blocked = !!(unmask & 2), biased = !!(unmask & 16);
          let value = large ? (up ? [0x8000000000000000n, 1] : [0x7fffffffffffffffn, 0]) : [up ? 2n : 1n, 0];
          if (biased) value = large ? [up ? 0xffffffffffffffffn : 0xfffffffffffffffen, 24576] : [up ? 0x8000000000000001n : 0x8000000000000000n, 24514];
          const status = (7 << 11) | 0x4100 | (blocked ? 0x8082 : 0x32 | (up ? 0x200 : 0) | (unmask ? 0x8080 : 0));
          checkSin(run(1), sinExpected(before, value, status, CODE, !blocked));
        }
      }));
    await test("FSIN masked special values and range failure preserve stack shape and unrelated state", () => fixture(
      [0xd9, 0xfe], async ({ reset, state, run }) => {
        for (const [input, value, flags, range] of [
          [[0xc000000000000123n, 0xffff], [0xc000000000000123n, 0xffff], 0, false],
          [[0x8000000000000123n, 0x7fff], [0xc000000000000123n, 0x7fff], 1, false],
          [[0x8000000000000000n, 0x7fff], [0xc000000000000000n, 0xffff], 1, false],
          [[0x4000000000000000n, 0x3fff], [0xc000000000000000n, 0xffff], 1, false],
          [[0x8000000000000000n, 0x403e], [0x8000000000000000n, 0x403e], 0, true],
        ]) {
          reset({ flags: 0xad7 }); seedSin(state(), input, 0x37f, 7); const before = Array.from(state());
          checkSin(run(1), sinExpected(before, value, (7 << 11) | 0x4100 | flags | (range ? 0x400 : 0), CODE, !range));
        }
      }));
    await test("FSIN unmasked underflow survives MF service and stores its biased result without replay", () => fixture(
      [0xd9, 0xfe, 0x40, 0x9b, 0xdb, 0x3b], async ({ reset, state, run, compile, bytes }) => {
        reset({ eax: 7, ebx: DATA }); seedSin(state(), [1n, 0], 0xb6f, 7); state()[60] = 7 << 11;
        const result = run(100); equal(result.kind, 14, "next waiting operation delivers pending underflow"); equal(result.error, 16, "underflow MF cause");
        equal(result.pc, CODE + 3, "next waiting instruction faults"); equal(result.retired, 2, "FSIN and integer instruction retire"); equal(result.words[0], 8, "integer effect once");
        equal(extRegister(result.words, 7).significand, 0x8000000000000001n, "unmasked result rounded before exponent bias");
        equal(extRegister(result.words, 7).exponent, 24514, "full underflow exponent bias"); equal(result.words[60], (7 << 11) | 0x82b2, "UE, DE, PE and C1 committed");
        equal(result.words[63], CODE, "FSIN location survives deferred MF"); equal(result.words[61], 0x3fff, "no stack pop");
        state()[59] |= 16; state()[60] &= ~0x80ff; // Guest exception service masks UE and clears pending causes.
        await compile(CODE + 3); const resumed = run(2);
        equal(resumed.retired, 2, "FWAIT and the following raw80 store alone resume"); equal(resumed.words[0], 8, "integer prefix cannot replay");
        equal(resumed.words[61], 0xffff, "one store pops the completed FSIN result");
        const raw = extBytes(0x8000000000000001n, 24514);
        for (let n = 0; n < 10; n++) equal(bytes(DATA)[n], raw[n], "resumed store retains exponent-biased raw80");
      }));
    await test("FSIN completed signed result survives a later store fault and repair without prefix replay", () => fixture(
      [0xff, 0x07, 0xd9, 0xfe, 0xdb, 0x3b], async ({ reset, state, run, bytes, map, compile }) => {
        reset({ edi: STACK, ebx: DATA + 4091 }); seedSin(state(), [0n, 0x8000], 0x37f, 7);
        const fault = run(100); equal(fault.kind, 4, "raw80 store crosses missing page"); equal(fault.pc, CODE + 4, "store alone resumes"); equal(fault.retired, 2, "prefix and FSIN completed");
        equal(fault.words[61], 0x7fff, "negative-zero result remains on stack"); equal(bytes(STACK)[0], 1, "prefix store completed once");
        map(DATA + 4096, READ | WRITE); await compile(CODE + 4); const resumed = run(1);
        equal(resumed.retired, 1, "repaired store alone retires"); equal(resumed.words[61], 0xffff, "single store pop"); equal(bytes(STACK)[0], 1, "prefix never replays");
        const raw = extBytes(0n, 0x8000); for (let n = 0; n < 10; n++) equal(bytes(DATA + (4091 + n & ~4095))[(4091 + n) & 4095], raw[n], "resumed signed raw80 output");
      }));
  }
  if (typedKernelCount >= 26) {
    const tagExp = ([sig, exp]) => !(exp & 0x7fff) && sig === 0n ? 1
      : !(exp & 0x7fff) || (exp & 0x7fff) === 0x7fff || sig < 0x8000000000000000n ? 2 : 0;
    const putExp = (words, r, [sig, exp]) => words.set([Number(sig & 0xffffffffn), Number(sig >> 32n), exp], 35 + r * 3);
    const seedExp = (words, x, y, control, top = 7) => {
      for (let r = 0; r < 8; r++) words.set([r + 19, 0x80000000, 0x3fff], 35 + r * 3);
      putExp(words, top, x); putExp(words, (top + 1) & 7, y);
      words[59] = control; words[60] = (top << 11) | 0x4700;
      words[61] = (0xffff & ~(3 << (top * 2)) & ~(3 << (((top + 1) & 7) * 2)))
        | (tagExp(x) << (top * 2)) | (tagExp(y) << (((top + 1) & 7) * 2));
      words[62] = 0x321; words[63] = 0x11223344; words[64] = 0x1b;
      words[65] = 0x55667788; words[66] = 0x23; words[15] = 0x1b;
      for (let n = 67; n < 100; n++) words[n] = n === 99 ? 0x1f80 : (n * 0x1234567) >>> 0;
    };
    const expectedExp = (before, value, flags, up, second, commit = true, eip = CODE) => {
      const expected = Uint32Array.from(before), top = before[60] >> 11 & 7;
      expected[8] = eip + 2; expected[60] = (before[60] & ~0x200) | flags | (up ? 0x200 : 0);
      if (flags & ~before[59] & 0x3f) expected[60] |= 0x8080;
      expected[62] = 0x100 | second; expected[63] = eip; expected[64] = before[15];
      if (commit) { putExp(expected, top, value); expected[61] = (before[61] & ~(3 << (top * 2))) | (tagExp(value) << (top * 2)); }
      return expected;
    };
    const checkExp = (actual, expected) => {
      equal(actual.retired, 1, "one exponential instruction retires without pop");
      for (let n = 0; n < 100; n++) equal(actual.words[n], expected[n], `exponential complete architectural word ${n}`);
    };
    await test("F2XM1 signed zero, exact endpoints and full64 certified half obey every PC RC TOP", () => fixture(
      [0xd9, 0xf0], async ({ reset, state, run }) => {
        for (const pc of [0,2,3]) for (const rc of [0,1,2,3]) for (let top = 0; top < 8; top++) {
          for (const sign of [0,0x8000]) for (const kind of [0,1,2]) {
            const input = kind === 0 ? [0n,sign] : [0x8000000000000000n,sign | (kind === 1 ? 0x3fff : 0x3ffe)];
            const positive = [0xd413cccfe7799211n,0xd413cccfe7799211n,0xd413cccfe7799212n,0xd413cccfe7799211n];
            const negative = [0x95f619980c4336f7n,0x95f619980c4336f8n,0x95f619980c4336f7n,0x95f619980c4336f7n];
            const sigs = sign ? negative : positive;
            const value = kind === 0 ? input : kind === 1 ? [input[0],sign ? 0xbffe : 0x3fff] : [sigs[rc],sign | 0x3ffd];
            reset({ flags:0xad7 }); seedExp(state(),input,[0n,0],0x7f | pc << 8 | rc << 10,top);
            const before = Array.from(state());
            checkExp(run(1),expectedExp(before,value,kind ? 32 : 0,kind === 2 && sigs[rc] !== sigs[3],0xf0));
          }
        }
      }));
    await test("FSCALE truncates the scale independently of RC and keeps full significand and ST1", () => fixture(
      [0xd9, 0xfd], async ({ reset, state, run }) => {
        for (const pc of [0,2,3]) for (const rc of [0,1,2,3]) for (const sign of [0,0x8000]) for (let top = 0; top < 8; top++) {
          reset({ flags:0xad7 }); seedExp(state(),[0x8000000000000001n,0x3fff],[0xe000000000000000n,sign | 0x3fff],0x7f | pc << 8 | rc << 10,top);
          const before=Array.from(state());
          checkExp(run(1),expectedExp(before,[0x8000000000000001n,sign ? 0x3ffe : 0x4000],0,false,0xfd));
        }
      }));
    await test("FSCALE masked and unmasked range results retain the original significand before exponent bias", () => fixture(
      [0xd9, 0xfd], async ({ reset, state, run }) => {
        for (const overflow of [false,true]) for (const unmask of [0,8,16,32]) {
          const x = overflow ? [0xffffffffffffffffn,0x7ffe] : [0x8000000000000001n,1];
          const y = [0x8000000000000000n,overflow ? 0x3fff : 0xbfff], range=overflow ? 8 : 16;
          const biased = !!(unmask & range), value = biased ? [x[0],overflow ? 0x1fff : 0x6000]
            : overflow ? [0x8000000000000000n,0x7fff] : [0x4000000000000000n,0];
          reset(); seedExp(state(),x,y,0x37f & ~unmask); const before=Array.from(state());
          checkExp(run(1),expectedExp(before,value,range | (biased ? 0 : 32),overflow && !biased,0xfd));
        }
      }));
    await test("F2XM1 tiny result, pre-denormal and biased underflow preserve all architectural state", () => fixture(
      [0xd9,0xf0], async ({reset,state,run}) => {
        for(const rc of [0,1,2,3]) for(const unmask of [0,2,16,18,32]) {
          reset(); seedExp(state(),[1n,0],[0n,0],(0x37f | rc << 10) & ~unmask);
          const before=Array.from(state()), blocked=!!(unmask & 2), biased=!!(unmask & 16), up=rc===0 || rc===2;
          const value=biased ? [up ? 0xb17217f7d1cf79acn : 0xb17217f7d1cf79abn,0x5fc1] : [up ? 1n : 0n,0];
          checkExp(run(1),expectedExp(before,value,blocked ? 2 : 0x32,!blocked && up,0xf0,!blocked));
        }
      }));
    for(const second of [0xf0,0xfd]) await test(`${second===0xf0 ? "F2XM1" : "FSCALE"} stack faults precede reserved precision and suppressed results never pop`,()=>fixture(
      [0xd9,second],async({reset,state,run})=>{
        for(const missing of [0,1]) for(const masked of [false,true]) {
          if(second===0xf0 && missing===1)continue;
          reset(); seedExp(state(),[0x8000000000000000n,0x3ffe],[0x8000000000000000n,0x3fff],masked ? 0x17f : 0x17e);
          state()[61]|=3 << (((7+missing)&7)*2); const before=Array.from(state());
          checkExp(run(1),expectedExp(before,[0xc000000000000000n,0xffff],65,false,second,masked));
        }
      }));
    await test("F2XM1 finite domain and FSCALE double range unsupported exits publish no partial state", async()=>{
      for(const [second,x,y,cw] of [
        [0xf0,[0x8000000000000000n,0x4000],[0n,0],0x37f],
        [0xfd,[1n,0],[0xc000000000000000n,0xc00d],0x36f],
      ]) await fixture([0xd9,second],async({reset,state,run})=>{
        reset();seedExp(state(),x,y,cw);const before=Array.from(state()),actual=run(1);
        equal(actual.kind,3,"unsupported is explicit");equal(actual.retired,0,"no retirement");
        for(let n=0;n<100;n++)equal(actual.words[n],before[n],`unsupported state ${n}`);
      });
    });
    await test("F2XM1 precision commits before deferred MF and resumes without repeating integer side effects",()=>fixture(
      [0xd9,0xf0,0x40,0x9b,0xdb,0x3b],async({reset,state,run,compile,bytes})=>{
        reset({eax:7,ebx:DATA});seedExp(state(),[0x8000000000000000n,0x3ffe],[0n,0],0x35f);
        const result=run(100);equal(result.kind,14,"next waiting instruction faults");equal(result.error,32,"precision cause");
        equal(result.pc,CODE+3,"fault belongs to FWAIT");equal(result.retired,2,"numeric and integer retire");equal(result.words[0],8,"integer once");
        equal(result.words[63],CODE,"original numeric location retained");
        state()[59]|=32;state()[60]&=~0x80ff;
        await compile(CODE+3);const resumed=run(2);equal(resumed.retired,2,"wait and store resume");equal(resumed.words[0],8,"no replay");
        const expected=extBytes(0xd413cccfe7799211n,0x3ffd);for(let n=0;n<10;n++)equal(bytes(DATA)[n],expected[n],"raw80 result preserved");
      }));
    await test("FSCALE result survives subsequent page fault and writes once after repair",()=>fixture(
      [0xff,0x07,0xd9,0xfd,0xdb,0x3b],async({reset,state,run,compile,bytes,map})=>{
        reset({edi:STACK,ebx:DATA+4091});seedExp(state(),[0x8000000000000001n,0x3fff],[0x8000000000000000n,0x3fff],0x37f);
        const fault=run(100);equal(fault.kind,4,"store fault");equal(fault.pc,CODE+4,"precise fault PC");equal(fault.retired,2,"prefix and FSCALE retire");equal(bytes(STACK)[0],1,"prefix once");
        map(DATA+4096,READ|WRITE);await compile(CODE+4);const resumed=run(1);equal(resumed.retired,1,"one store");equal(bytes(STACK)[0],1,"no replay");
        const expected=extBytes(0x8000000000000001n,0x4000);for(let n=0;n<10;n++)equal(bytes(DATA+((4091+n)&~4095))[(4091+n)&4095],expected[n],"exact output");
      }));
  }
  if (typedKernelCount >= 22) {
    const atanTag = ([sig, exp]) => !(exp & 0x7fff) && sig === 0n ? 1
      : !(exp & 0x7fff) || (exp & 0x7fff) === 0x7fff || sig < 0x8000000000000000n ? 2 : 0;
    const seedAtan = (words, x, y, control, top = 0) => {
      for (let r = 0; r < 8; r++) words.set([r + 11, 0x80000000, 0x3fff], 35 + r * 3);
      const put = (r, [sig, exp]) => words.set([Number(sig & 0xffffffffn), Number(sig >> 32n), exp], 35 + r * 3);
      const destination = (top + 1) & 7; put(top, x); put(destination, y);
      words[59] = control; words[60] = (top << 11) | 0x4700;
      words[61] = (0xffff & ~(3 << (top * 2)) & ~(3 << (destination * 2)))
        | (atanTag(x) << (top * 2)) | (atanTag(y) << (destination * 2));
      words[62] = 0x321; words[63] = 0x11223344; words[64] = 0x1b;
      words[65] = 0x55667788; words[66] = 0x23; words[15] = 0x1b;
      for (let n = 67; n < 100; n++) words[n] = n === 99 ? 0x1f80 : (n * 0x1234567) >>> 0;
    };
    const atanValues = [
      {name:"one", raw:[0x8000000000000000n,0x3fff]},
      {name:"negative one", raw:[0x8000000000000000n,0xbfff], negative:true},
      {name:"positive zero", raw:[0n,0], zero:true},
      {name:"negative zero", raw:[0n,0x8000], zero:true, negative:true},
      {name:"positive infinity", raw:[0x8000000000000000n,0x7fff], infinity:true},
      {name:"negative infinity", raw:[0x8000000000000000n,0xffff], infinity:true, negative:true},
    ];
    // Independent interval bounds for pi and 3*pi/4 determine these directed
    // results. Architectural PE is checked separately from mathematical bits.
    const atanAngle = (x, y, rc) => {
      let angle;
      if (y.zero) angle = x.negative ? "pi" : "zero";
      else if (x.zero || (y.infinity && !x.infinity)) angle = "half";
      else if (x.infinity && !y.infinity) angle = x.negative ? "pi" : "zero";
      else angle = x.negative ? "three quarters" : "quarter";
      if (angle === "zero") return {sig:0n, exp:y.negative ? 0x8000 : 0, precision:false, increment:false};
      const lower = angle === "three quarters" ? 0x96cbe3f9990e91a7n : 0xc90fdaa22168c234n;
      const increment = rc === 0 || rc === (y.negative ? 1 : 2);
      return {sig:lower + (increment ? 1n : 0n), exp:(angle === "quarter" ? 0x3ffe : angle === "half" ? 0x3fff : 0x4000) | (y.negative ? 0x8000 : 0), precision:true, increment};
    };
    for (const x of atanValues) await test(`FPATAN x=${x.name}: quadrants, signed zeros/infinities and all PC/RC`, () => fixture(
      [0xd9,0xf3,0xdb,0x3b], async ({reset,state,run,compile,select,bytes}) => {
        await compile(CODE + 2);
        for (const y of atanValues) for (const pc of [0,2,3]) for (const rc of [0,1,2,3]) for (const top of [0,6,7]) {
          reset({ebx:DATA,flags:0xad7}); seedAtan(state(),x.raw,y.raw,0x7f|(pc<<8)|(rc<<10),top);
          const before=Array.from(state()),expected=atanAngle(x,y,rc),destination=(top+1)&7; select(CODE);
          const zero=run(0); equal(zero.retired,0,"zero budget cannot execute FPATAN");
          for(let n=35;n<100;n++)equal(zero.words[n],before[n],"zero budget preserves FP/vector state");
          const result=run(1);equal(result.retired,1,"FPATAN retires once");equal(result.pc,CODE+2,"budget preserves store continuation");
          equal(extRegister(result.words,destination).significand,expected.sig,"interval-certified angle significand");
          equal(extRegister(result.words,destination).exponent,expected.exp,"quadrant and sign including signed zero");
          equal(result.words[60],(destination<<11)|0x4500|(expected.precision?32:0)|(expected.increment?0x200:0),"TOP, mathematical C1 and architectural PE");
          equal(result.words[61],(0xffff&~(3<<(destination*2)))|(atanTag([expected.sig,expected.exp])<<(destination*2)),"classified destination and exactly one pop");
          for(let r=0;r<8;r++)if(r!==destination)for(let n=0;n<3;n++)equal(result.words[35+r*3+n],before[35+r*3+n],"unwritten raw physical bank preserved");
          equal(result.words[59],before[59],"control unchanged");equal(result.words[62],0x1f3,"FPATAN FOP");equal(result.words[63],CODE,"FPATAN FIP");equal(result.words[64],0x1b,"FPATAN FCS");
          equal(result.words[65],before[65],"register operation retains FDP");equal(result.words[66],before[66],"register operation retains FDS");equal(result.words[9],0xad7,"integer flags preserved");
          for(let n=67;n<100;n++)equal(result.words[n],before[n],"vectors/MXCSR preserved");
          select(CODE+2);const resumed=run(1);equal(resumed.retired,1,"store resumes without repeating atan or pop");
          const expectedBytes=extBytes(expected.sig,expected.exp);for(let n=0;n<10;n++)equal(bytes(DATA)[n],expectedBytes[n],"resumed raw80 store");
          equal(resumed.words[61],0xffff,"second completed pop empties remaining value");
        }
      }));
    await test("FPATAN atan2(3,2) uses certified nonconstant full-precision results across PC/RC",()=>fixture(
      [0xd9,0xf3],async({reset,state,run})=>{
        for(const pc of [0,2,3])for(const rc of [0,1,2,3]){
          reset();seedAtan(state(),[0x8000000000000000n,0x4000],[0xc000000000000000n,0x4000],0x7f|(pc<<8)|(rc<<10));
          const result=run(1);equal(result.retired,1,"finite angle retires once");equal(extRegister(result.words,1).significand,rc===2?0xfb985e940fb4d901n:0xfb985e940fb4d900n,"independent 320-bit rational interval result");
          equal(extRegister(result.words,1).exponent,0x3ffe,"atan2(3,2) exponent");equal(result.words[60],0x4d20|(rc===2?0x200:0),"nearest does not increment this angle; RU does");
        }
      }));
    const atanBoundarySeeds=[
      {name:"subnormal quotient 2^-62",x:[0x4000000000000000n,0],y:[1n,0],
        rounded:[[0x8000000000000000n,0x3fc1,1,0x22],[0xffffffffffffffffn,0x3fc0,0,0x22],[0x8000000000000000n,0x3fc1,1,0x22],[0xffffffffffffffffn,0x3fc0,0,0x22]]},
      {name:"pseudo-denormal quotient 2^-63",x:[0x8000000000000000n,0],y:[1n,0],
        rounded:[[0x8000000000000000n,0x3fc0,1,0x22],[0xffffffffffffffffn,0x3fbf,0,0x22],[0x8000000000000000n,0x3fc0,1,0x22],[0xffffffffffffffffn,0x3fbf,0,0x22]]},
      {name:"near-one divisor rounds across minimum-normal boundary",x:[0x8000000000000001n,0x3fff],y:[0x8000000000000000n,1],
        rounded:[[0x7fffffffffffffffn,0,0,0x30],[0x7fffffffffffffffn,0,0,0x30],[0x8000000000000000n,1,1,0x20],[0x7fffffffffffffffn,0,0,0x30]]},
      {name:"below-one divisor preserves tiny correction at normal precision",x:[0xffffffffffffffffn,0x3ffe],y:[0x8000000000000000n,1],
        rounded:[[0x8000000000000001n,1,1,0x20],[0x8000000000000000n,1,0,0x20],[0x8000000000000001n,1,1,0x20],[0x8000000000000000n,1,0,0x20]]},
    ];
    for(const sample of atanBoundarySeeds)await test(`FPATAN certified boundary: ${sample.name}`,()=>fixture(
      [0xd9,0xf3],async({handle,reset,state,run})=>{
        for(const pc of [0,2,3])for(const rc of [0,1,2,3]){
          reset({flags:0xad7});seedAtan(state(),sample.x,sample.y,0x7f|(pc<<8)|(rc<<10),7);
          const before=Array.from(state()),[sig,exp,up,flags]=sample.rounded[rc],result=run(1);
          equal(result.retired,1,"certified boundary must complete rather than report uncertainty");equal(result.pc,CODE+2,"boundary operation retires exactly once");
          equal(extRegister(result.words,0).significand,sig,"independent interval significand");equal(extRegister(result.words,0).exponent,exp,"independent interval exponent");
          equal(result.words[60],0x4500|flags|(up?0x200:0),"after-rounding range flags and certified C1");equal(result.words[61],0xfffc|atanTag([sig,exp]),"TOP wrap and result classification");
          for(let r=1;r<8;r++)for(let n=0;n<3;n++)equal(result.words[35+r*3+n],before[35+r*3+n],"unwritten physical values preserved");
          state().set(before);equal(api.backend_x87(handle,24|(1<<8)|(1<<14)|(1<<15),0xffffffff,0,CODE,0x1f3),0,"same precise value through architectural state helper");state()[8]=CODE+2;
          for(let n=0;n<100;n++)equal(result.words[n],state()[n],"all defined FP/environment and general state agrees across lowering");
        }
      }));
    await test("FPATAN actual one/one input commits precision result and pop before deferred MF",()=>fixture(
      [0xd9,0xf3,0x40,0xd9,0x02],async({reset,state,run})=>{
        reset({eax:7,edx:0xffffffff});seedAtan(state(),atanValues[0].raw,atanValues[0].raw,0x25f,6);state()[60]=0x3000;
        const result=run(100);equal(result.kind,14,"next waiting instruction sees unmasked PE");equal(result.error,32,"precision is the pending cause");equal(result.pc,CODE+3,"MF precedes missing memory operand");equal(result.retired,2,"atan and integer operation complete");
        equal(result.words[0],8,"integer operation executes once");equal(result.words[60],(7<<11)|0x82a0,"precision and C1 with committed TOP");equal(result.words[61],0x3fff,"unmasked PE still commits pop");
        equal(extRegister(result.words,7).significand,0xc90fdaa22168c235n,"certified actual pi/4 significand");equal(extRegister(result.words,7).exponent,0x3ffe,"actual pi/4 exponent");equal(result.words[63],CODE,"causing FP instruction metadata retained");
      }));
    await test("FPATAN reached masked control/status retain existing precision while producing pi/4",()=>fixture(
      [0xd9,0xf3],async({reset,state,run})=>{
        reset();seedAtan(state(),atanValues[0].raw,atanValues[0].raw,0x27f,6);state()[60]=0x3020;
        equal(state()[61],0x0fff,"actual initial physical tags");const result=run(1);
        equal(result.retired,1,"masked existing precision does not fault");equal(result.words[59],0x27f,"actual control remains");equal(result.words[60],0x3a20,"actual status has sticky PE, C1 and new TOP");equal(result.words[61],0x3fff,"actual result pops once");
        equal(extRegister(result.words,7).significand,0xc90fdaa22168c235n,"actual raw significand");equal(extRegister(result.words,7).exponent,0x3ffe,"actual raw exponent");
      }));
    await test("FPATAN pending MF preserves prior store and repairs at the exact instruction",()=>fixture(
      [0xff,0x03,0xd9,0xf3],async({reset,state,run,bytes,compile})=>{
        reset({ebx:DATA});seedAtan(state(),atanValues[0].raw,atanValues[0].raw,0x35f);state()[60]=0x80a0;
        const before=Array.from(state().slice(35,100)),fault=run(100);equal(fault.kind,14,"pending precision faults before atan");equal(fault.retired,1,"prior store alone completes");equal(fault.pc,CODE+2,"exact waiting instruction");equal(bytes(DATA)[0],1,"prior store retained once");
        for(let n=0;n<before.length;n++)equal(fault.words[35+n],before[n],"pending MF leaves FP environment and payload untouched");
        state()[59]=0x37f;state()[60]=0;await compile(CODE+2);const repaired=run(1);equal(repaired.retired,1,"repair executes atan only");equal(repaired.words[61],0xfff3,"repair pops exactly once");equal(bytes(DATA)[0],1,"prior store not replayed");
      }));
    for(const empty of [1,2,3])await test(`FPATAN empty input mask${empty} is atomic before numeric evaluation`,()=>fixture(
      [0xd9,0xf3],async({reset,state,run})=>{
        for(const masked of [false,true])for(const top of [0,7]){
          reset();seedAtan(state(),atanValues[0].raw,atanValues[0].raw,masked?0x37f:0x37e,top);const destination=(top+1)&7;
          state()[60]=(top<<11)|0x200;if(empty&1)state()[61]|=3<<(top*2);if(empty&2)state()[61]|=3<<(destination*2);
          const before=Array.from(state()),result=run(1);equal(result.retired,1,"new stack fault is deferred");equal(result.words[60],masked?(destination<<11)|0x41:(top<<11)|0x80c1,"empty input clears C1 and pops only masked");
          equal(result.words[61],masked?(0xffff&~(3<<(destination*2)))|(2<<(destination*2)):before[61],"masked indefinite or suppressed tags");
          if(masked){equal(extRegister(result.words,destination).significand,0xc000000000000000n,"masked indefinite raw payload");equal(extRegister(result.words,destination).exponent,0xffff,"masked indefinite exponent");}
          for(let r=0;r<8;r++)if(!masked||r!==destination)for(let n=0;n<3;n++)equal(result.words[35+r*3+n],before[35+r*3+n],"suppressed/unwritten raw slots remain");
          equal(result.words[62],0x1f3,"stack fault records FOP");equal(result.words[63],CODE,"stack fault records FIP");
        }
      }));
    const atanSpecials=[
      {name:"quiet NaN suppresses denormal",x:[0xc000000000000123n,0x7fff],y:[1n,0],value:[0xc000000000000123n,0x7fff],flags:0},
      {name:"signaling NaN quiets before other exceptions",x:[0x8000000000000123n,0x7fff],y:[1n,0],value:[0xc000000000000123n,0x7fff],flags:1},
      {name:"unsupported encoding precedes denormal",x:[0x4000000000000000n,0x3fff],y:[1n,0],value:[0xc000000000000000n,0xffff],flags:1},
      {name:"denormal is observed for exact signed zero",x:[1n,0],y:[0n,0x8000],value:[0n,0x8000],flags:2},
      {name:"pseudo-denormal is observed before exact zero",x:[0x8000000000000000n,0],y:[0n,0],value:[0n,0],flags:2},
    ];
    for(const sample of atanSpecials)await test(`FPATAN ${sample.name}: native exception priority and atomic pop`,()=>fixture(
      [0xd9,0xf3],async({reset,state,run})=>{
        for(const pc of [0,2,3])for(const rc of [0,1,2,3])for(const unmask of [0,sample.flags||0x3f]){
          reset({flags:0xad7});seedAtan(state(),sample.x,sample.y,(0x7f|(pc<<8)|(rc<<10))&~unmask);
          const before=Array.from(state()),result=run(1),suppressed=Boolean(sample.flags&unmask);
          equal(result.retired,1,"new architectural exception is deferred");equal(result.words[60],0x4500|sample.flags|(suppressed?0x8080:0x800),"native-defined exception priority, C1 and TOP");
          equal(result.words[61],suppressed?before[61]:0xfff3|(atanTag(sample.value)<<2),"suppressed result/pop or classified result");
          if(!suppressed){equal(extRegister(result.words,1).significand,sample.value[0],"special raw80 payload");equal(extRegister(result.words,1).exponent,sample.value[1],"special raw80 sign/exponent");}
          for(let r=0;r<8;r++)if(suppressed||r!==1)for(let n=0;n<3;n++)equal(result.words[35+r*3+n],before[35+r*3+n],"unwritten raw80 retained");
          equal(result.words[62],0x1f3,"special FOP");equal(result.words[63],CODE,"special FIP");equal(result.words[65],before[65],"special retains FDP");equal(result.words[66],before[66],"special retains FDS");equal(result.words[9],0xad7,"integer flags retained");
          for(let n=67;n<100;n++)equal(result.words[n],before[n],"special retains vectors/MXCSR");
        }
      }));
    await test("FPATAN denormal unmask suppresses both precision and destination even on an axis",()=>fixture(
      [0xd9,0xf3],async({reset,state,run})=>{
        for(const rc of [0,1,2,3])for(const unmask of [0,2]){
          reset();seedAtan(state(),[0n,0x8000],[1n,0],(0x37f|(rc<<10))&~unmask);const before=Array.from(state()),result=run(1);
          equal(result.retired,1,"new DE is deferred");equal(result.words[60],unmask?0xc582:0x4d22|((rc===0||rc===2)?0x200:0),"DE priority and mathematical C1 when completed");
          equal(result.words[61],unmask?before[61]:0xfff3,"unmasked DE suppresses pop");
          if(unmask)for(let n=35;n<59;n++)equal(result.words[n],before[n],"unmasked DE leaves raw payload unchanged");
          else{equal(extRegister(result.words,1).significand,(rc===0||rc===2)?0xc90fdaa22168c235n:0xc90fdaa22168c234n,"axis pi/2 rounding");equal(extRegister(result.words,1).exponent,0x3fff,"axis pi/2 exponent");}
        }
      }));
    await test("FPATAN tiny result has certified directed rounding and explicit unmasked-range boundary",()=>fixture(
      [0xff,0x03,0xd9,0xf3],async({reset,state,run,bytes})=>{
        for(const rc of [0,1,2,3])for(const unmask of [0,16]){
          reset({ebx:DATA});bytes(DATA).fill(0);seedAtan(state(),atanValues[0].raw,[1n,0],(0x37f|(rc<<10))&~unmask);const before=Array.from(state().slice(35,100)),result=run(100);
          equal(bytes(DATA)[0],1,"preceding store remains once");
          if(unmask){equal(result.kind,3,"unsupported unmasked underflow is explicit");equal(result.error,0,"existing architectural range diagnostic");equal(result.pc,CODE+2,"exact unsupported FPATAN");equal(result.retired,1,"unsupported atan never retires");for(let n=0;n<before.length;n++)equal(result.words[35+n],before[n],"unsupported range preserves full FP state");}
          else{const up=rc===0||rc===2;equal(result.retired,2,"masked tiny angle completes");equal(result.words[60],0x4d32|(up?0x200:0),"DE/UE/PE with certified magnitude increment");equal(extRegister(result.words,1).significand,up?1n:0n,"strict atan(tiny)<tiny directed rounding");equal(extRegister(result.words,1).exponent,0,"tiny positive exponent");equal(result.words[61],up?0xfffb:0xfff7,"subnormal versus zero tag follows actual rounded result");}
        }
      }));
    await test("FPATAN pure typed kernel preserves its stable output and architectural owners on errors",()=>fixture(
      [0x90],async({handle,reset,state,bytes})=>{
        reset({eax:0x76543210});bytes(DATA).fill(0x53);const before=Array.from(state()),image=bytes(DATA).slice();
        const pointer=api.backend_fp_kernel_21(0x37f,0,0x80000000,0x3fff,0,0x80000000,0x3fff);check(pointer!==0,"pure angle kernel available without executing CPU");
        const output=Array.from(new Uint32Array(api.memory.buffer,pointer,7));equal(output[0],0x2168c235,"pi/4 low word");equal(output[1],0xc90fdaa2,"pi/4 high word");equal(output[2],0x3ffe,"pi/4 exponent");equal(output[5],32,"architectural precision output");equal(output[6],1,"certified round-up output");
        for(const[control,a2,b2,errorCode]of[[0x17f,0x3fff,0x3fff,1],[0x1037f,0x3fff,0x3fff,4],[0x37f,0x10000,0x3fff,4],[0x37f,0x3fff,0x10000,4]]){
          equal(api.backend_fp_kernel_21(control,0,0x80000000,a2,0,0x80000000,b2),0,"invalid angle kernel input rejected");equal(api.backend_fp_kernel_error(),errorCode,"precise angle diagnostic");for(let n=0;n<7;n++)equal(new Uint32Array(api.memory.buffer,pointer,7)[n],output[n],"failed angle cannot replace output cell");
        }
        equal(api.backend_fp_kernel_21(0x37f,0,0x80000000,0x3fff,0,0,0x8000),pointer,"next valid angle reuses stable instance cell");equal(api.backend_fp_kernel_error(),0,"successful angle clears prior error");equal(new Uint32Array(api.memory.buffer,pointer,7)[2],0x8000,"signed zero is consumed immediately");
        for(let n=0;n<105;n++)equal(state()[n],before[n],"pure angle helpers do not touch CPU");for(let n=0;n<4096;n++)equal(bytes(DATA)[n],image[n],"pure angle helpers do not touch GuestMemory");check(api.backend_state(handle)!==0,"architectural owner remains live");
      }));
    if(options.typedNumeric)await test("FPATAN reserved precision failure leaves state untouched and resumes after repair",()=>fixture(
      [0xff,0x03,0xd9,0xf3],async({reset,state,run,bytes,compile})=>{
        reset({ebx:DATA});seedAtan(state(),atanValues[0].raw,atanValues[0].raw,0x17f);const before=Array.from(state().slice(35,100)),fault=run(100);
        equal(fault.kind,3,"reserved precision is unsupported");equal(fault.error,1,"kernel error copied before another call");equal(fault.retired,1,"only preceding store retires");equal(fault.pc,CODE+2,"failure preserves exact atan PC");for(let n=0;n<before.length;n++)equal(fault.words[35+n],before[n],"failed kernel has no FP partial commit");
        state()[59]=0x37f;await compile(CODE+2);const repaired=run(1);equal(repaired.retired,1,"only repaired atan retires");equal(repaired.words[61],0xfff3,"repair pops once");equal(bytes(DATA)[0],1,"prefix store never replayed");
      }));
    await test("fault after FPATAN cannot repeat its completed result or pop",()=>fixture(
      [0xff,0x07,0xd9,0xf3,0xdb,0x3b],async({reset,state,run,bytes,map,compile})=>{
        reset({edi:STACK,ebx:DATA+4091});seedAtan(state(),atanValues[0].raw,atanValues[0].raw,0x37f,6);
        const fault=run(100);equal(fault.kind,4,"following raw80 store faults across page boundary");equal(fault.retired,2,"store prefix and atan already complete");equal(fault.pc,CODE+4,"only failed raw80 store resumes");equal(fault.address,DATA+4096,"exact missing byte");equal(fault.words[61],0x3fff,"failed store cannot pop again");
        const fp=Array.from(fault.words.slice(35,67));equal(fault.words[63],CODE+2,"failed store retains preceding atan metadata");for(let n=4091;n<4096;n++)equal(bytes(DATA)[n],0,"cross-page store is atomic");
        map(DATA+4096,READ|WRITE);await compile(CODE+4);const repaired=run(1);equal(repaired.retired,1,"only failed store retires");equal(bytes(STACK)[0],1,"prefix store occurs once");equal(repaired.words[61],0xffff,"repaired store pops remaining result once");
        const expected=extBytes(0xc90fdaa22168c235n,0x3ffe);for(let n=0;n<10;n++)equal(bytes(n<5?DATA:DATA+4096)[n<5?4091+n:n-5],expected[n],"cross-page repaired angle bytes");
        for(let n=0;n<24;n++)equal(repaired.words[35+n],fp[n],"store preserves all raw physical payloads");
      }));
    if(hasTimestamp&&hasSseCvtt)await test("FPATAN typed kernel, SSE conversions and timestamp imports share one module",()=>fixture(
      [0xd9,0xf3,0xf3,0x0f,0x2c,0xc2,0xf2,0x0f,0x2c,0xcd,0x0f,0x31,0xdb,0x3b],async({handle,reset,state,run,compile,bytes})=>{
        reset({ebx:DATA});seedAtan(state(),atanValues[0].raw,atanValues[0].raw,0x37f);
        state()[75]=0x40400000;state()[87]=0;state()[88]=0x40180000;
        const requested=run(100);equal(requested.kind,15,"timestamp handoff follows all three arithmetic operations");equal(requested.retired,3,"timestamp remains unretired");equal(requested.pc,CODE+10,"exact RDTSC continuation");equal(requested.words[0],3,"binary32 conversion result");equal(requested.words[1],6,"binary64 conversion result");
        equal(extRegister(requested.words,1).significand,0xc90fdaa22168c235n,"atan value retained through shifted imports");equal(requested.words[61],0xfff3,"atan pop retained through SSE operations");
        ok(api.backend_complete_timestamp_counter(handle,CODE+10,0x12345678,0x9abcdef0),"complete actual pending timestamp");await compile(CODE+12);const resumed=run(1);equal(resumed.retired,1,"raw80 store resumes after timestamp once");
        equal(resumed.words[0],0x12345678,"timestamp low result");equal(resumed.words[2],0x9abcdef0,"timestamp high result");const expected=extBytes(0xc90fdaa22168c235n,0x3ffe);for(let n=0;n<10;n++)equal(bytes(DATA)[n],expected[n],"mixed-import stored angle");
      }));
  }
  const x87Constants = [
    { name: "FLDZ", opcode: 0xee, exponent: 0, lower: 0n, nearestUp: false, exact: true },
    { name: "FLD1", opcode: 0xe8, exponent: 0x3fff, lower: 0x8000000000000000n, nearestUp: false, exact: true },
    { name: "FLDL2T", opcode: 0xe9, exponent: 0x4000, lower: 0xd49a784bcd1b8afen, nearestUp: false },
    { name: "FLDL2E", opcode: 0xea, exponent: 0x3fff, lower: 0xb8aa3b295c17f0bbn, nearestUp: true },
    { name: "FLDPI", opcode: 0xeb, exponent: 0x4000, lower: 0xc90fdaa22168c234n, nearestUp: true },
    { name: "FLDLG2", opcode: 0xec, exponent: 0x3ffd, lower: 0x9a209a84fbcff798n, nearestUp: true },
    { name: "FLDLN2", opcode: 0xed, exponent: 0x3ffe, lower: 0xb17217f7d1cf79abn, nearestUp: true },
  ];
  for (const constant of x87Constants) {
    await test(`${constant.name} raw80 RC rounding ignores PC and never creates PE`, () => fixture(
      [0xd9, constant.opcode, 0xdb, 0x3b], async ({ reset, state, run, bytes, compile, select }) => {
        await compile(CODE + 2);
        for (const pc of [0, 2, 3]) for (const rc of [0, 1, 2, 3]) for (const variant of [0, 1, 2]) {
          reset({ eax: 0x12345678, ebx: DATA, flags: 0xad7 });
          state()[59] = (variant === 1 ? 0x5f : 0x7f) | pc << 8 | rc << 10;
          state()[60] = 0x200 | (variant === 2 ? 0x20 : 0);
          state()[62] = 0x321; state()[63] = 0x12345678; state()[64] = 0x1b;
          state()[65] = 0x76543210; state()[66] = 0x23; state()[15] = 0x1b;
          for (let r = 0; r < 8; r++) { state()[35 + r * 3] = 0x12345600 + r; state()[36 + r * 3] = 0x80000100 + r; state()[37 + r * 3] = 0x3fff; }
          const before = Array.from(state()); select(CODE);
          const zero = run(0); equal(zero.retired, 0, "zero budget cannot push a constant");
          for (let n = 35; n < 100; n++) equal(zero.words[n], before[n], "zero budget preserves FP and vector state");
          const result = run(1), value = constant.lower + (!constant.exact && (rc === 2 || rc === 0 && constant.nearestUp) ? 1n : 0n);
          equal(result.retired, 1, "one constant instruction completes without running store");
          equal(result.pc, CODE + 2, "budget stops immediately after constant");
          for (let n = 35; n < 56; n++) equal(result.words[n], before[n], "inactive physical register contents retained");
          equal(extRegister(result.words, 7).significand, value, "exact 64-bit significand");
          equal(extRegister(result.words, 7).exponent, constant.exponent, "exact sign/exponent");
          equal(result.words[59], before[59], "control word retained");
          equal(result.words[60], 0x3800 | (variant === 2 ? 0x20 : 0), "TOP changes, C1 clears, only prior sticky PE survives");
          equal(result.words[61], constant.exponent === 0 ? 0x7fff : 0x3fff, "pushed zero/valid tag is architectural");
          equal(result.words[62], 0x100 | constant.opcode, "constant FOP recorded");
          equal(result.words[63], CODE, "constant FIP recorded"); equal(result.words[64], 0x1b, "constant FCS recorded");
          equal(result.words[65], before[65], "no-memory constant retains FDP"); equal(result.words[66], before[66], "no-memory constant retains FDS");
          equal(result.words[9], 0xad7, "integer flags retained");
          for (let n = 67; n < 100; n++) equal(result.words[n], before[n], "constant never aliases SSE state");
          select(CODE + 2); const resumed = run(1);
          equal(resumed.retired, 1, "resume stores exactly once without replaying push");
          equal(resumed.words[61], 0xffff, "resumed store leaves stack empty");
          const expected = extBytes(value, constant.exponent);
          for (let n = 0; n < 10; n++) equal(bytes(DATA)[n], expected[n], "resumed raw80 memory bytes");
        }
      }));
    await test(`${constant.name} handles stack overflow and pending MF before any push`, () => fixture(
      [0xd9, constant.opcode], async ({ reset, state, run }) => {
        for (const masked of [false, true]) {
          reset({ flags: 0xad7 }); state()[59] = masked ? 0x37f : 0x37e; state()[61] = 0;
          for (let r = 0; r < 8; r++) { state()[35+r*3]=r+1;state()[36+r*3]=0x80000000;state()[37+r*3]=0x3fff; }
          const before = Array.from(state()), result = run(1);
          equal(result.retired, 1, "new stack fault retires as deferred exception");
          equal(result.words[60], masked ? 0x3a41 : 0x82c1, "native-verified overflow status and TOP");
          equal(result.words[61], masked ? 0x8000 : 0, "masked indefinite tag or unchanged unmasked tags");
          equal(result.words[59], before[59], "overflow retains control word");
          equal(result.words[62], 0x100 | constant.opcode, "overflow records the causing FOP");
          equal(result.words[63], CODE, "overflow records the causing FIP");
          equal(result.words[64], before[15], "overflow records current CS selector");
          equal(result.words[65], before[65], "constant overflow leaves memory pointer unchanged");
          equal(result.words[66], before[66], "constant overflow leaves data selector unchanged");
          for (let n = 35; n < (masked ? 56 : 59); n++) equal(result.words[n], before[n], "unwritten raw registers retained");
          if (masked) {
            equal(extRegister(result.words, 7).significand, 0xc000000000000000n, "masked overflow writes indefinite");
            equal(extRegister(result.words, 7).exponent, 0xffff, "indefinite sign/exponent");
          }
          equal(result.words[9], 0xad7, "stack overflow retains integer flags");
        }
        reset({ flags: 0xad7 }); state()[59] = 0x35f; state()[60] = 0x80a0;
        state()[62] = 0x321; state()[63] = 0x12345678; state()[65] = 0x76543210;
        const before = Array.from(state()), result = run(1);
        equal(result.kind, 14, "pending unmasked precision exception raises MF");
        equal(result.retired, 0, "pending MF prevents constant retirement");
        equal(result.pc, CODE, "pending MF identifies constant instruction");
        for (let n = 35; n < 100; n++) equal(result.words[n], before[n], "pending MF preserves raw stack, environment and SSE");
      }));
  }
  const widenedBinary = (bits, fractionBits, exponentBits, bias) => {
    const fractionMask=(1n<<BigInt(fractionBits))-1n,exponentMask=(1<<exponentBits)-1;
    const sign=Number(bits>>BigInt(fractionBits+exponentBits))*0x8000;
    const exponent=Number(bits>>BigInt(fractionBits)&BigInt(exponentMask)),fraction=bits&fractionMask;
    if(exponent===exponentMask){
      const nan=fraction!==0n,quiet=1n<<BigInt(fractionBits-1);
      return{significand:(1n<<63n)|(fraction<<BigInt(63-fractionBits))|(nan?1n<<62n:0n),exponent:sign|0x7fff,exceptions:nan&&!(fraction&quiet)?1:0,tag:2};
    }
    if(exponent===0){
      if(fraction===0n)return{significand:0n,exponent:sign,exceptions:0,tag:1};
      const highest=fraction.toString(2).length-1;
      return{significand:fraction<<BigInt(63-highest),exponent:sign|(0x3fff+1-bias-fractionBits+highest),exceptions:2,tag:0};
    }
    return{significand:((1n<<BigInt(fractionBits))|fraction)<<BigInt(63-fractionBits),exponent:sign|(0x3fff+exponent-bias),exceptions:0,tag:0};
  };
  for (const numericMode of options.fpIr ? [0,1,2] : [0]) {
    for (const [name, first, second, width, fraction, exponent, bias, inputs] of [
      ["binary32",0xd9,0x05,32,23,8,127,[0n,0x80000000n,1n,0x3fc00000n,0x7f800001n,0xffc12345n,0x7f800000n]],
      ["binary64",0xdd,0x05,64,52,11,1023,[0n,0x8000000000000000n,1n,0x3ff8000000000000n,0x7ff0000000000001n,0xfff8123456789abcn,0x7ff0000000000000n]],
      ["integer16",0xdf,0x05,16,0,0,0,[0n,1n,0x7fffn,0x8000n,0xffffn]],
      ["integer32",0xdb,0x05,32,0,0,0,[0n,1n,0x7fffffffn,0x80000000n,0xffffffffn]],
      ["integer64",0xdf,0x2d,64,0,0,0,[0n,1n,0x7fffffffffffffffn,0x8000000000000000n,0xffffffffffffffffn]],
    ]) await test(`x87 literal ${name} mode ${numericMode} preserves exact loads and changed inputs`,()=>fixture(
      [first,second,...wordBytes(DATA+4095),0x0f,0x0b],async({reset,run,state,bytes,map,protect,compile})=>{
        map(DATA+4096,READ|WRITE);protect(DATA,READ);protect(DATA+4096,READ);
        const put=raw=>{for(let b=0;b<width/8;b++)bytes((DATA+4095+b)&~4095)[(4095+b)&4095]=Number(raw>>BigInt(b*8)&255n);};
        let index=0;
        for(const prepared of inputs){
          put(prepared);reset();await compile(CODE,256,index++%2?[CODE]:null);
          for(const raw of [prepared,BigInt.asUintN(width,prepared^1n),prepared]){
            put(raw);
            let expected;
            if(fraction)expected=widenedBinary(raw,fraction,exponent,bias);
            else{
              const value=BigInt.asIntN(width,raw),magnitude=value<0n?-value:value,highest=magnitude===0n?0:magnitude.toString(2).length-1;
              expected={significand:magnitude<<BigInt(63-highest),exponent:magnitude===0n?0:(value<0n?0x8000:0)|(0x3fff+highest),exceptions:0,tag:magnitude===0n?1:0};
            }
            for(let pc=0;pc<4;pc++)for(let rc=0;rc<4;rc++){
              reset({flags:0xad7});state()[59]=0x7f|(pc<<8)|(rc<<10);
              const before=Array.from(state()),zero=run(0);equal(zero.retired,0,"zero budget performs no literal read");
              for(let n=35;n<100;n++)equal(zero.words[n],before[n],"zero budget preserves FP/SIMD context");
              const result=run(1),top=extRegister(result.words,7);
              equal(result.kind,1,"checked literal load yields at exact budget");equal(result.retired,1,"literal load retires once");equal(result.pc,CODE+6,"literal load continuation");
              equal(top.significand,expected.significand,"prepared or changed source exact significand");equal(top.exponent,expected.exponent,"source sign/exponent");
              equal(result.words[60],0x3800|expected.exceptions,"source exception and TOP");equal(result.words[61],0x3fff|(expected.tag<<14),"exact source tag");
              equal(result.words[63],CODE,"load records its original instruction");equal(result.words[65],DATA+4095,"split source address remains architectural");equal(result.words[9],0xad7,"integer flags retained");
            }
          }
        }
      },READ|EXECUTE|SEALED,{numericMode}));
  }
  await test("x87 literal pending exception, second-page fault and changed page contents remain precise",()=>fixture(
    [0xdd,0x05,...wordBytes(DATA+4093),0x0f,0x0b],async({reset,run,state,bytes,map,protect,compile})=>{
      map(DATA+4096,READ);protect(DATA,READ);
      const put=raw=>{for(let b=0;b<8;b++)bytes((DATA+4093+b)&~4095)[(4093+b)&4095]=Number(raw>>BigInt(b*8)&255n);};
      put(0x3ff8000000000000n);await compile(CODE,256,[CODE]);protect(DATA+4096,0);
      reset({flags:0xad7});state()[59]=0x37e;state()[60]=0x8081;state()[63]=0x12345678;state()[65]=0x87654321;
      const before=Array.from(state()),pending=run(1);equal(pending.kind,14,"pending MF precedes prepared memory read");equal(pending.retired,0,"pending MF does not retire");
      for(let n=35;n<100;n++)equal(pending.words[n],before[n],"pending MF preserves all FP/SIMD state");
      state()[59]=0x37f;state()[60]=0;const fault=run(1);equal(fault.kind,4,"prepared value cannot bypass missing permission");equal(fault.address,DATA+4096,"second-page fault address");equal(fault.error,5,"present read protection fault");equal(fault.retired,0,"failed read cannot push");equal(fault.words[63],0x12345678,"failed read preserves FIP");equal(fault.words[65],0x87654321,"failed read preserves FDP");
      protect(DATA+4096,READ);map(DATA+4096,READ);put(0xfff0000000000001n);
      const actual=run(1),expected=widenedBinary(0xfff0000000000001n,52,11,1023);
      equal(actual.retired,1,"same resident code resumes after page repair");equal(extRegister(actual.words,7).significand,expected.significand,"changed page uses current SNaN payload");equal(extRegister(actual.words,7).exponent,expected.exponent,"changed page preserves sign");equal(actual.words[60],0x3801,"changed SNaN raises IE");
    }));
  await test("x87 literal observes a preceding guest write through a writable alias",()=>fixture(
    [0xc7,0x05,...wordBytes(0x5000+36),...wordBytes(0x40040000),
      0xdd,0x05,...wordBytes(DATA+32),0xdd,0x1d,...wordBytes(STACK),0x0f,0x0b],async({reset,run,bytes,protect,compile,alias})=>{
      setWord(bytes(DATA),32,0);setWord(bytes(DATA),36,0x3ff80000);protect(DATA,READ);alias(0x5000,DATA,READ|WRITE);
      await compile(CODE,256,[CODE,CODE+10]);
      reset({eip:CODE+10});equal(run(2).retired,2,"arbitrary entry uses original literal");
      equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset).getFloat64(0,true),1.5,"original read-only value");
      reset();const actual=run(3);equal(actual.retired,3,"alias store, changed load and output complete once");
      equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset).getFloat64(0,true),2.5,"same unit uses post-store aliased bytes");
      equal(actual.words[61],0xffff,"load/store pop topology preserved");
    }));
  for(const [name,second,operation,pops] of [["reverse subtract",0x2d,22,0],["reverse divide",0x3d,23,0],["compare and pop",0x1d,7,1]]){
    await test(`x87 literal ${name} retains signaling and exception precedence`,()=>fixture(
      [0xd8,second,...wordBytes(DATA),0x0f,0x0b],async({handle,reset,run,state,bytes,protect,compile})=>{
        protect(DATA,READ);
        for(const raw of [0x3fc00000,1,0x7f800001,0xffc01234]){
          setWord(bytes(DATA),0,raw);reset();await compile();
          for(const control of [0x37f,0x37e,0x37d])for(const nan of [false,true]){
            reset({flags:0xad7});state().set(nan?[1,0xc0000000,0x7fff]:[0,0x80000000,0x3fff],35);state()[59]=control;state()[60]=0x4700;state()[61]=nan?0xfffe:0xfffc;
            const before=Array.from(state()),actual=run(1);equal(actual.retired,1,"source operation retires with deferred exceptions");
            state().set(before);equal(api.backend_x87(handle,operation|(1<<11)|(pops<<15),3,DATA,CODE,second),0,"independent architectural transition");state()[8]=CODE+6;
            for(let n=0;n<100;n++)equal(actual.words[n],state()[n],"literal source preserves complete architectural operation");
          }
        }
      }));
  }
  for(const [name,code,width,fractionBits,exponentBits,bias] of [
    ["binary32",[0xd9,0x03],32,23,8,127],["binary64",[0xdd,0x03],64,52,11,1023],
    ["integer16",[0xdf,0x03],16,0,0,0],["integer32",[0xdb,0x03],32,0,0,0],["integer64",[0xdf,0x2b],64,0,0,0],
  ])await test(`x87 exact source widening ${name} preserves payloads, signs, tags and control modes`,()=>fixture(code,async({reset,run,state,bytes,map})=>{
    map(DATA+4096,READ|WRITE);const inputs=[];
    if(fractionBits){
      const fractions=[0n,1n,(1n<<BigInt(fractionBits-1))-1n,1n<<BigInt(fractionBits-1),(1n<<BigInt(fractionBits))-1n];
      for(let e=0;e<(1<<exponentBits);e++)for(const f of fractions)for(const sign of [0n,1n])inputs.push((sign<<BigInt(width-1))|(BigInt(e)<<BigInt(fractionBits))|f);
    }else{
      inputs.push(0n);for(let bit=0;bit<width;bit++)for(const delta of [-1n,0n,1n])for(const sign of [-1n,1n])inputs.push(BigInt.asUintN(width,sign*((1n<<BigInt(bit))+delta)));
    }
    let sample=0;
    for(const raw of inputs){
      const address=DATA+(sample%17===0?4093:3),control=0x7f|((sample>>2)&3)<<8|(sample&3)<<10;
      for(let b=0;b<width/8;b++)bytes((address+b)&~4095)[(address+b)&4095]=Number(raw>>BigInt(b*8)&255n);
      let expected;
      if(fractionBits)expected=widenedBinary(raw,fractionBits,exponentBits,bias);
      else{
        const value=BigInt.asIntN(width,raw),magnitude=value<0n?-value:value,highest=magnitude===0n?0:magnitude.toString(2).length-1;
        expected={significand:magnitude<<BigInt(63-highest),exponent:magnitude===0n?0:(value<0n?0x8000:0)|(0x3fff+highest),exceptions:0,tag:magnitude===0n?1:0};
      }
      reset({ebx:address,flags:0xad7});state()[59]=control;state()[60]=0x4700;
      const actual=run(1),top=extRegister(actual.words,7);
      equal(actual.kind,1,"widening completes at exact budget");equal(actual.retired,1,"load retires once");equal(actual.pc,CODE+2,"load continuation");
      equal(top.significand,expected.significand,`${name} exact significand`);equal(top.exponent,expected.exponent,`${name} exact sign/exponent`);
      equal(actual.words[59],control,"all precision and rounding controls retained");equal(actual.words[60],0x7d00|expected.exceptions,"TOP, C1 and source exceptions");equal(actual.words[61],0x3fff|(expected.tag<<14),"converted raw80 tag");
      equal(actual.words[62],((code[0]&7)<<8)|code[1],"memory load opcode");equal(actual.words[63],CODE,"memory load instruction pointer");equal(actual.words[65],address,"unaligned or split-page source pointer");equal(actual.words[9],0xad7,"widening preserves integer flags");
      sample++;
    }
  }));
  await test("x87 exact source widening preserves pending exception and split-read fault precedence",()=>fixture([0xdd,0x03],async({reset,run,state,map,bytes})=>{
    map(0x3000,READ);const raw=0xfff0000000000001n,address=0x3ffd;
    for(let b=0;b<3;b++)bytes(0x3000)[4093+b]=Number(raw>>BigInt(b*8)&255n);
    reset({ebx:address,flags:0xad7});state()[59]=0x37e;state()[60]=0x8081;state()[63]=0x12345678;state()[65]=0x87654321;
    const before=Array.from(state()),pending=run(1);equal(pending.kind,14,"pending MF precedes missing upper page");equal(pending.retired,0,"pending MF does not retire load");
    for(let n=35;n<100;n++)equal(pending.words[n],before[n],"pending MF leaves all FP/SIMD state intact");
    state()[59]=0x37f;state()[60]=0;const fault=run(1);equal(fault.kind,4,"source read faults before conversion");equal(fault.address,0x4000,"first absent source page");equal(fault.retired,0,"failed read does not retire");equal(fault.words[61],0xffff,"failed read does not push");equal(fault.words[63],0x12345678,"failed read retains previous FIP");equal(fault.words[65],0x87654321,"failed read retains previous FDP");
    map(0x4000,READ);for(let b=3;b<8;b++)bytes(0x4000)[b-3]=Number(raw>>BigInt(b*8)&255n);
    const repaired=run(1),expected=widenedBinary(raw,52,11,1023);equal(repaired.kind,1,"repair completes conversion");equal(repaired.retired,1,"repair retires exactly once");equal(extRegister(repaired.words,7).significand,expected.significand,"SNaN payload survives repaired split read");equal(extRegister(repaired.words,7).exponent,expected.exponent,"SNaN sign retained");equal(repaired.words[60],0x3801,"masked signaling NaN pushes quiet value with IE");
  }));
  await test("x87 actual audio load multiply store and FNSTSW AX", () => fixture(
    [0xd9, 0x02, 0xd8, 0x0e, 0xd9, 0x5e, 0xfc, 0xdf, 0xe0], async ({ reset, run, bytes }) => {
      setWord(bytes(DATA), 0, 0x3fc00000); setWord(bytes(DATA), 4, 0x40000000);
      reset({ eax: 0xabcd1234, edx: DATA, esi: DATA + 4, flags: 0xa93 });
      const result = run(4);
      equal(result.retired, 4, "all actual audio forms completed");
      equal(new DataView(bytes(DATA).buffer, bytes(DATA).byteOffset).getUint32(0, true), 0x40400000, "exact 1.5 times 2 stored");
      equal(result.words[0], 0xabcd0000, "FNSTSW writes AX and preserves high EAX");
      equal(result.words[9], 0xa93, "x87 leaves EFLAGS intact"); equal(result.words[61], 0xffff, "store popped stack");
      equal(result.words[62], 0x15e, "FNSTSW does not replace last FOP");
      equal(result.words[63], CODE + 4, "last x87 data instruction pointer");
      equal(result.words[65], DATA, "last store data pointer");
    }));
  await test("captured audio loop body matches precise state transitions at every budget boundary", () => fixture(
    // Contiguous captured body: FLD [EDX]; FMUL [ESI]; ADD ESI,4;
    // DEC ECX; FSTP [ESI-4]. Addresses here are generic fixture mappings.
    [0xd9, 0x02, 0xd8, 0x0e, 0x83, 0xc6, 4, 0x49, 0xd9, 0x5e, 0xfc],
    async ({ handle, map, compile, select, reset, run, bytes, state }) => {
      const offsets = [0, 2, 4, 7, 8, 11];
      for (const offset of offsets.slice(1, -1)) await compile(CODE + offset);
      const initialData = page();
      setWord(initialData, 0, 0x3f800001); setWord(initialData, 4, 0x3f000001);
      const initialize = (control) => {
        map(DATA, READ | WRITE, initialData);
        reset({ eax: 0xaabbccdd, ecx: 2, edx: DATA, esi: DATA + 4, flags: 0xad7 });
        for (let n = 35; n < 59; n++) {
          state()[n] = (n * 0x1f123bb5) >>> 0;
          if ((n - 35) % 3 === 2) state()[n] &= 0xffff;
        }
        state()[59] = control;
        state()[62] = 0x321; state()[63] = 0x11223344; state()[65] = 0x55667788;
      };
      const snapshot = () => ({ words: Array.from(state().slice(0, 100)), data: bytes(DATA).slice() });
      const compare = (actual, expected, label) => {
        for (let n = 0; n < 100; n++) equal(actual.words[n], expected.words[n], `${label}: architectural word ${n}`);
        const data = bytes(DATA);
        for (let n = 0; n < data.length; n++) equal(data[n], expected.data[n], `${label}: guest output byte ${n}`);
      };
      for (const pc of [0, 2, 3]) for (const rc of [0, 1, 2, 3]) {
        const control = 0x7f | pc << 8 | rc << 10;
        initialize(control);
        const expected = [snapshot()];
        for (let instruction = 0; instruction < 5; instruction++) {
          const words = state(), eip = CODE + offsets[instruction];
          if (instruction === 0) equal(api.backend_x87(handle, 1 | 1 << 11, 3, words[2], eip, 0x102), 0, "reference FLD");
          else if (instruction === 1) equal(api.backend_x87(handle, 5 | 1 << 11, 3, words[6], eip, 0x00e), 0, "reference FMUL");
          else if (instruction === 4) equal(api.backend_x87(handle, 2 | 1 << 11 | 1 << 15, 3,
            (words[6] - 4) >>> 0, eip, 0x15e), 0, "reference FSTP");
          else {
            const index = instruction === 2 ? 6 : 1;
            const mask = instruction === 2 ? ARITH : ARITH & ~CF;
            const reference = arithmetic(instruction === 2 ? "add" : "sub", 32, words[index], instruction === 2 ? 4 : 1, 0);
            words[index] = reference.result;
            words[9] = (words[9] & ~mask) | (reference.flags & mask);
            words[10] |= mask;
          }
          state()[8] = CODE + offsets[instruction + 1];
          expected.push(snapshot());
        }
        ok(api.backend_finish(handle), "collect reference writes");
        for (let cut = 0; cut <= 5; cut++) {
          initialize(control); select(CODE);
          const prefix = run(cut);
          equal(prefix.retired, cut, "exact completed prefix budget");
          compare(prefix, expected[cut], `PC${pc}/RC${rc}/prefix${cut}`);
          if (cut < 5) {
            select(CODE + offsets[cut]);
            const resumed = run(5 - cut);
            equal(resumed.retired, 5 - cut, "resume completes only suffix");
            compare(resumed, expected[5], `PC${pc}/RC${rc}/resume${cut}`);
          }
        }
      }
    }));
  await test("x87 raw m80 load and store preserve every architectural bit", () => fixture(
    [0xdb, 0x28, 0xdb, 0x3a], async ({ reset, run, bytes }) => {
      for (const [significand, exponent] of [[0x8000000000000001n, 0x3fff],
        [0x8000000000000001n, 0x7fff], [0x4000000000000000n, 0x3fff], [0x8000000000000000n, 0]]) {
        const source = extBytes(significand, exponent); bytes(DATA).set(source);
        reset({ eax: DATA, edx: DATA + 32 });
        const result = run(2); equal(result.retired, 2, "raw extended transfer completed");
        for (let n = 0; n < 10; n++) equal(bytes(DATA)[32 + n], source[n], "all ten raw bytes retained");
        equal(extRegister(result.words, 7).significand, significand, "physical MMX-shared significand retained");
        equal(extRegister(result.words, 7).exponent, exponent, "physical raw exponent retained");
        equal(result.words[60], 0, "raw transfer raises no arithmetic exception");
      }
    }));
  await test("x87 m80 addition respects every precision and rounding setting", () => fixture(
    [0xdb, 0x28, 0xdb, 0x2a, 0xde, 0xc1], async ({ reset, run, bytes, state }) => {
      for (const pc of [0, 2, 3]) for (const rc of [0, 1, 2, 3]) for (const negative of [false, true]) {
        const sign = negative ? 0x8000 : 0;
        bytes(DATA).set(extBytes(0x8000000000000000n, 0x3fff | sign));
        bytes(DATA).set(extBytes(0x8000000000000000n, 0x3fc0 | sign), 16);
        reset({ eax: DATA, edx: DATA + 16 }); state()[59] = 0x7f | pc << 8 | rc << 10;
        const result = run(3); equal(result.retired, 3, "full precision load/load/addp completed");
        const upwardMagnitude = negative ? rc === 1 : rc === 2;
        const increment = pc === 3 ? 1n : upwardMagnitude ? 1n << BigInt(pc === 2 ? 11 : 40) : 0n;
        const actual = extRegister(result.words, 7);
        equal(actual.significand, 0x8000000000000000n + increment, `precision ${pc}, rounding ${rc}, sign ${negative}`);
        equal(actual.exponent, 0x3fff | sign, "full x87 exponent range and sign");
        equal(result.words[60] & 0x3f, pc === 3 ? 0 : 32, "inexact is reported only when rounded");
        equal(result.words[60] & 0x200, pc !== 3 && upwardMagnitude ? 0x200 : 0, "C1 records magnitude increment");
        equal(result.words[61], 0x3fff, "pop arithmetic leaves physical R7 live");
      }
    }));
  await test("x87 pending exception precedes operand range and mapping faults", () => fixture(
    [0xd9, 0x00], async ({ reset, run, state }) => {
      reset({ eax: 0xffffffff }); state()[59] = 0x37e; state()[60] = 0x80c1;
      const before = Array.from(state().slice(35, 67));
      const result = run(1); equal(result.kind, 14, "pending #MF precedes operand fault");
      equal(result.error, 1, "pending invalid cause retained"); equal(result.retired, 0, "waiting operation did not complete");
      equal(result.pc, CODE, "pending fault PC");
      for (let n = 0; n < before.length; n++) equal(result.words[35 + n], before[n], "pending fault leaves x87 state exact");
    }));
  await test("x87 deferred invalid permits integer work and faults at next waiting instruction", () => fixture(
    [0xd9, 0xc0, 0x40, 0xd9, 0x00], async ({ reset, run, state }) => {
      reset({ eax: DATA }); state()[59] = 0x37e;
      const result = run(3); equal(result.kind, 14, "next waiting instruction reports #MF");
      equal(result.retired, 2, "first invalid operation and intervening integer work completed");
      equal(result.words[0], DATA + 1, "intervening INC completed once");
      equal(result.pc, CODE + 3, "fault is at the next waiting instruction");
      equal(result.words[60], 0x80c1, "deferred stack invalid state retained");
      equal(result.words[61], 0xffff, "unmasked empty load did not push");
      equal(result.words[63], CODE, "saved FIP identifies original arithmetic cause");
    }));
  await test("x87 nonwaiting status and clear operate with a pending exception", () => fixture(
    [0xdf, 0xe0, 0xdb, 0xe2, 0xd9, 0x02], async ({ reset, run, state }) => {
      reset({ eax: 0x12340000, edx: 0xffffffff }); state()[59] = 0x37e; state()[60] = 0x80c1;
      state()[63] = 0x44556677;
      const result = run(3); equal(result.kind, 6, "after FNCLEX the fresh operand range fault is visible");
      equal(result.retired, 2, "FNSTSW and FNCLEX completed"); equal(result.words[0], 0x123480c1, "pending status returned in AX");
      equal(result.words[60], 0, "exception flags and summary cleared");
      equal(result.words[63], 0x44556677, "control instructions and failed operand do not replace FIP");
    }));
  await test("x87 full m80 store preflight faults without pop then retries once", () => fixture(
    [0xd9, 0xe8, 0xdb, 0x38], async ({ reset, run, bytes, state, map, compile }) => {
      bytes(DATA).fill(0x5a, 4091); reset({ eax: DATA + 4091 });
      const first = run(2); equal(first.kind, 4, "second page of ten-byte store faults");
      equal(first.retired, 1, "prior FLD1 remains completed"); equal(first.pc, CODE + 2, "store restart PC");
      equal(first.address, DATA + 4096, "exact inaccessible byte"); equal(first.error, 6, "write to absent user page");
      equal(first.words[61], 0x3fff, "failed store did not pop");
      for (let n = 4091; n < 4096; n++) equal(bytes(DATA)[n], 0x5a, "first page bytes unchanged");
      map(DATA + 4096, READ | WRITE); await compile(CODE + 2);
      const second = run(1); equal(second.retired, 1, "resolved store completed exactly once");
      equal(second.words[61], 0xffff, "successful retry popped once");
      const expected = extBytes(0x8000000000000000n, 0x3fff);
      for (let n = 0; n < 10; n++) {
        const address = DATA + 4091 + n;
        equal(bytes(address & ~4095)[address & 4095], expected[n], "complete extended store bytes");
      }
      equal(state()[60] >>> 11 & 7, 0, "retry returned TOP to zero");
    }));
  await test("x87 code-alias store exits after one completed pop and invalidates code", () => fixture(
    [0xd9, 0xe8, 0xd9, 0x1a], async ({ handle, reset, run, alias, bytes }) => {
      alias(0x4000, CODE, READ | WRITE); reset({ edx: 0x4100 });
      const result = run(5); equal(result.kind, 7, "code modification exits immediately");
      equal(result.retired, 2, "load and modifying store retired once each");
      equal(result.pc, CODE + 4, "SMC continuation is after store"); equal(result.words[61], 0xffff, "SMC store pop committed");
      equal(new DataView(bytes(CODE).buffer, bytes(CODE).byteOffset).getUint32(0x100, true), 0x3f800000, "code alias bytes updated");
      check(api.backend_validate(handle, CODE) !== 0, "old code is rejected after helper store");
    }, READ | WRITE | EXECUTE | SEALED));
  await test("scalar and x87 writes retain every dirty alias until outer collection", () => fixture(
    [0xff, 0x03, 0xd9, 0xe8, 0xd9, 0x1a, 0xff, 0x43, 4],
    async ({ handle, reset, run, alias, bytes }) => {
      alias(0x4000, DATA, READ | WRITE);
      alias(0x5000, STACK, READ | WRITE);
      reset({ ebx: DATA, edx: 0x5000 });
      const result = run(4);
      equal(result.retired, 4, "scalar, load, x87 store and scalar completion");
      equal(result.words[61], 0xffff, "successful store pops exactly once");
      for (const address of [DATA, 0x4000]) {
        equal(bytes(address)[0], 1, "earlier scalar store retained through helper");
        equal(bytes(address)[4], 1, "later scalar store retained through helper");
      }
      for (const address of [STACK, 0x5000]) {
        equal(new DataView(bytes(address).buffer, bytes(address).byteOffset).getUint32(0, true),
          0x3f800000, "both aliases observe exact floating store");
      }
      for (const address of [DATA, 0x4000, STACK, 0x5000]) {
        const entry = new Uint32Array(api.memory.buffer, api.backend_table(handle) + (address >>> 12) * 16, 4);
        equal(entry[2], 2, "each dirty alias remains pending authoritative writeback");
      }
    }));
  await test("x87 readonly second page preserves prior scalar store and exact FP state before retry", () => fixture(
    [0xff, 0x03, 0xd9, 0xe8, 0xdb, 0x3a],
    async ({ reset, run, bytes, state, map, protect, compile }) => {
      map(DATA + 4096, READ, page([], 0x6b));
      bytes(DATA).fill(0x5a, 4091);
      reset({ ebx: STACK, edx: DATA + 4091 });
      const prefix = run(2);
      equal(prefix.retired, 2, "prefix scalar store and FLD1 completed");
      const before = Array.from(state().slice(35, 67));
      await compile(CODE + 4);
      const fault = run(1);
      equal(fault.kind, 4, "second-page write protection fault");
      equal(fault.pc, CODE + 4, "only failed store resumes");
      equal(fault.address, DATA + 4096, "first protected byte is exact");
      equal(fault.error, 7, "present user write protection error");
      equal(fault.retired, 0, "failed store did not retire");
      equal(bytes(STACK)[0], 1, "completed scalar write retained");
      for (let n = 0; n < before.length; n++) equal(fault.words[35 + n], before[n], "failed store leaves all FP state exact");
      for (let n = 4091; n < 4096; n++) equal(bytes(DATA)[n], 0x5a, "first page remains unchanged");
      for (let n = 0; n < 5; n++) equal(bytes(DATA + 4096)[n], 0x6b, "second page remains unchanged");
      protect(DATA + 4096, READ | WRITE);
      const resumed = run(1), expected = extBytes(0x8000000000000000n, 0x3fff);
      equal(resumed.retired, 1, "repaired store completes once");
      equal(resumed.words[61], 0xffff, "repaired store pops once");
      equal(bytes(STACK)[0], 1, "prior scalar instruction was not replayed");
      for (let n = 0; n < 10; n++) {
        const address = DATA + 4091 + n;
        equal(bytes(address & ~4095)[address & 4095], expected[n], "all ten output bytes committed");
      }
    }));
  for (const [width, opcode, modrm] of [[2, 0xdf, 0x03], [4, 0xd9, 0x03], [8, 0xdd, 0x03], [10, 0xdb, 0x2b]]) {
    await test(`x87 ${width}-byte read checks both pages before stack or metadata mutation`, () => fixture(
      [opcode, modrm], async ({ reset, run, bytes, state, map }) => {
        map(DATA + 4096, WRITE, page([], 0x23));
        bytes(DATA).fill(0x45, 4095);
        reset({ ebx: DATA + 4095, flags: 0xad7 });
        state()[62] = 0x141; state()[63] = 0x12345678; state()[65] = 0x87654321;
        const before = Array.from(state().slice(35, 67));
        const result = run(1);
        equal(result.kind, 4, "second page read protection fault");
        equal(result.address, DATA + 4096, "exact first forbidden byte");
        equal(result.error, 5, "present user read protection error");
        equal(result.retired, 0, "failed read did not retire");
        equal(result.words[9], 0xad7, "integer flags retained");
        for (let n = 0; n < before.length; n++) equal(result.words[35 + n], before[n], "no partial FP or metadata change");
      }));
  }
  for (const suppressed of [false, true]) {
    await test(`x87 ${suppressed ? "suppressed invalid" : "ordinary"} store handles full dirty journal atomically`, () => fixture(
      [0xd9, 0x1b], async ({ handle, reset, run, bytes, state }) => {
        reset({ ebx: DATA, flags: 0xad7 });
        if (suppressed) state()[59] = 0x37e;
        else {
          state()[35] = 0; state()[36] = 0x80000000; state()[37] = 0x3fff;
          state()[61] = 0xfffc;
        }
        const before = Array.from(state().slice(35, 67));
        const header = () => new Uint32Array(api.memory.buffer, api.backend_table(handle) - 16, 4);
        header()[1] = header()[2];
        const result = run(1, () => { header()[1] = 0; });
        equal(bytes(DATA)[0], 0, "failed or suppressed output writes no guest bytes");
        equal(result.words[9], 0xad7, "integer flags unchanged");
        if (suppressed) {
          equal(result.retired, 1, "new unmasked FP exception completes as deferred exception");
          equal(result.words[60], 0x80c1, "invalid empty-stack cause retained");
          equal(result.words[61], 0xffff, "suppressed store does not pop");
        } else {
          equal(result.kind, 10, "full journal reports explicit resource exit");
          equal(result.retired, 0, "unadmitted store does not retire");
          equal(result.pc, CODE, "store retains exact retry PC");
          for (let n = 0; n < before.length; n++) equal(result.words[35 + n], before[n], "failed output preserves full FP state");
        }
      }));
  }
  await test("x87 FS data pointer remains logical and FNSTSW preserves environment", () => fixture(
    [0x64, 0xd9, 0x00, 0xdf, 0xe0], async ({ reset, run, bytes, state }) => {
      setWord(bytes(DATA), 4, 0x3f800000); reset({ fs: DATA, eax: 4 });
      state()[27] = 0x3b; state()[15] = 0x1b;
      const result = run(2); equal(result.retired, 2, "FS load and status completed");
      equal(result.words[62], 0x100, "FOP excludes FS prefix"); equal(result.words[63], CODE, "status does not replace FIP");
      equal(result.words[64], 0x1b, "instruction selector retained");
      equal(result.words[65], 4, "FDP is guest segment offset"); equal(result.words[66], 0x3b, "data selector identifies FS");
      equal(result.words[0], 0x3800, "FNSTSW returns live TOP");
    }));
  await test("MMX x87 MMX transitions preserve new extended exponents and shared significands", () => fixture(
    [0x0f, 0x6e, 0xf8, 0x0f, 0x77, 0xdb, 0x2a, 0xdb, 0x3b, 0x0f, 0x7e, 0xf8], async ({ reset, run, bytes }) => {
      const source = extBytes(0x8000000000000001n, 0x3fff); bytes(DATA).set(source);
      reset({ eax: 0xdeadbeef, edx: DATA, ebx: DATA + 32 });
      const result = run(5); equal(result.retired, 5, "all mixed MMX/x87 operations completed");
      equal(result.words[0], 1, "MMX reads new physical R7 significand");
      equal(extRegister(result.words, 7).exponent, 0x3fff, "stale MMX write bitmap did not overwrite x87 exponent");
      equal(extRegister(result.words, 7).significand, 0x8000000000000001n, "extended significand retained after MMX read");
      for (let n = 0; n < 10; n++) equal(bytes(DATA)[32 + n], source[n], "m80 store sees the post-x87 raw value");
    }));
  await test("exhausted fuel precedes unknown direct, indirect and return-target admission", async () => {
    for (const code of [[0xe9, ...wordBytes(0x6000 - CODE - 5)], [0xff, 0xe0], [0xff, 0xd0], [0xc3]]) {
      await fixture(code, async ({ reset, run, bytes, compile }) => {
        await compile(CODE, 1); // The target belongs to a different compilation unit.
        reset({ eax: 0x6000, flags: 0xad7 }); setWord(bytes(STACK), 0x800, 0x6000);
        const stopped = run(0); equal(stopped.kind, 1, "zero budget executes no transfer");
        equal(stopped.pc, CODE, "zero budget retains source PC"); equal(stopped.retired, 0, "zero work");
        equal(stopped.words[4], STACK + 0x800, "zero budget has no stack effect");
        const result = run(1);
        equal(result.kind, 1, "completed transfer exhausts fuel before unavailable target fetch");
        equal(result.retired, 1, "transfer retires exactly once"); equal(result.pc, 0x6000, "next PC is retained");
        equal(result.words[9], 0xad7, "transfer preserves materialized flags");
        const call = code[0] === 0xff && code[1] === 0xd0;
        equal(result.words[4], STACK + 0x800 + (call ? -4 : code[0] === 0xc3 ? 4 : 0), "completed stack effect survives budget exit");
        if (call) equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0x7fc, true), CODE + 2, "pushed return address survives budget exit");
        await compile(result.pc);
        const fault = run(1); equal(fault.kind, 4, "next nonzero budget observes the actual missing executable target");
        equal(fault.pc, 0x6000, "precise fault PC"); equal(fault.address, 0x6000, "precise fetch address");
        equal(fault.error, 20, "nonpresent user execute fault"); equal(fault.retired, 0, "fault does not retire an instruction");
        equal(fault.words[4], result.words[4], "fault never replays the completed transfer");
      });
    }
  });
  await test("shared return continuations preserve outgoing flags, exact budgets and fault state", () => {
    const code = new Array(52).fill(0xcc);
    code.splice(0, 12, 0xe8,27,0,0,0, 0x83,0xd2,0, 0x8b,0x1e, 0x0f,0x0b);
    code.splice(16, 12, 0xe8,11,0,0,0, 0x83,0xda,0, 0x8b,0x1e, 0x0f,0x0b);
    code.splice(32, 4, 0x83,0xc0,1,0xc3);
    code.splice(48, 3, 0x47,0x0f,0x0b);
    return fixture(code, async ({compile, reset, run, bytes}) => {
      await compile(CODE, 128, [CODE,CODE+16,CODE+32,CODE+35,CODE+48]);
      setWord(bytes(DATA), 0, 0x12345678);
      for (const entry of [0,16]) for (const a of [0xffffffff,0x7fffffff,0])
        for (const fault of [false,true]) for (const budget of [0,1,2,3,4,5,100]) {
          reset({eip:CODE+entry,eax:a,edx:0xffffffff,ebx:7,esi:DATA+(fault?4096:0),flags:0xad7});
          const added=arithmetic("add",32,a,1,0);
          const consumed=arithmetic(entry===0?"adc":"sbb",32,0xffffffff,0,added.flags&CF);
          const retired=Math.min(budget,fault?4:5);
          const expectedPc=[CODE+entry,CODE+32,CODE+35,CODE+entry+5,CODE+entry+8,CODE+entry+10][retired];
          const result=run(budget);
          equal(result.retired,retired,"shared CALL/RET prefix retires once");
          equal(result.pc,expectedPc,"exact shared continuation boundary");
          equal(result.kind,fault&&budget>4?4:budget>5?8:1,"budget wins before a later fault or trap");
          equal(result.words[0],retired>=2?added.result:a,"callee arithmetic commits at its own boundary");
          equal(result.words[2],retired>=4?consumed.result:0xffffffff,"continuation consumes callee CF");
          equal(result.words[3],retired>=5?0x12345678:7,"failed load preserves destination");
          equal(result.words[4],STACK+0x800-(retired===1||retired===2?4:0),"actual CALL/RET stack progress");
          equal(result.words[9],retired>=4?0x202|consumed.flags:retired>=2?0x202|added.flags:0xad7,"flags survive return and recovery");
          if(fault&&budget>4) equal(result.address,DATA+4096,"faulting continuation address");
        }
      for(const target of [5,21,48]) for(const carry of [0,1]) {
        reset({eip:CODE+35,eax:19,edx:0xffffffff,edi:9,esi:DATA,flags:0x202|carry});
        setWord(bytes(STACK),0x800,CODE+target);
        const result=run(20);
        equal(result.kind,8,"raw RET follows both guarded and unrelated destinations");
        equal(result.words[0],19,"raw RET has no hidden caller state");
        equal(result.words[4],STACK+0x804,"raw return pops the actual stack");
        equal(result.retired,target===48?2:3,"raw target path executes once");
        if(target===48) {
          equal(result.words[7],10,"unexpected in-unit target executes normally");
          equal(result.words[9]&CF,carry,"unexpected INC preserves incoming CF");
        }else{
          const expected=arithmetic(target===5?"adc":"sbb",32,0xffffffff,0,carry);
          equal(result.words[2],expected.result,"guarded raw target consumes arbitrary incoming flags");
          equal(result.words[9],0x202|expected.flags,"guarded raw target computes exact flags");
        }
      }
      reset({eip:CODE+35,esp:STACK+4094,eax:19,edx:29,flags:0xad7});
      const fault=run(20);
      equal(fault.kind,4,"split-page RET stack load faults before any guard");
      equal(fault.retired,0,"faulting RET never retires");
      equal(fault.pc,CODE+35,"fault remains at RET");
      equal(fault.words[4],STACK+4094,"failed RET does not pop stack");
      equal(fault.words[9],0xad7,"failed RET retains all flags");
    });
  });
  if (options.linkedRegions) {
    await test("linked indirect CALL and altered RET observe the actual guest stack", () => fixture(
      [0xff, 0xd0, 0x0f, 0x0b], async ({ map, compile, reset, run, bytes }) => {
        const callee = 0x4000, continuation = 0x6000;
        map(callee, READ | EXECUTE | SEALED,
          page([0x8b, 0x3c, 0x24, 0xc7, 0x04, 0x24, ...wordBytes(continuation), 0xc3], 0xcc));
        map(continuation, READ | EXECUTE | SEALED, page([0x43, 0x0f, 0x0b], 0xcc));
        await compile(callee); await compile(continuation);
        reset({ eax: callee, ebx: 9 });
        const result = run(100);
        equal(result.kind, 8, "altered guest continuation reaches its UD2");
        equal(result.retired, 5, "CALL, stack read, stack rewrite, RET and increment complete");
        equal(result.pc, continuation + 1, "RET chooses the rewritten address");
        equal(result.words[7], CODE + 2, "callee observed real pushed return address");
        equal(result.words[4], STACK + 0x800, "guest stack balances after altered RET");
        equal(result.words[3], 10, "altered continuation executed once");
        equal(new DataView(bytes(STACK).buffer, bytes(STACK).byteOffset).getUint32(0x7fc, true),
          continuation, "rewritten guest return remains in guest memory");
      }));
    await test("linked MMX x87 and XMM share exact payloads while live GPRs cross units", () => fixture(
      [0x0f, 0x6e, 0xf8, 0x0f, 0x10, 0x01, 0x83, 0xc1, 4, 0xff, 0xe2],
      async ({ map, compile, reset, run, bytes, state }) => {
        const target = 0x4000;
        map(target, READ | EXECUTE | SEALED, page([
          0x0f, 0x7e, 0xfd, 0x0f, 0x77, 0xdb, 0x29, 0xdb, 0x3b,
          0x0f, 0x7e, 0xf8, 0x0f, 0x11, 0x07, 0x0f, 0x0b], 0xcc));
        await compile(target);
        const raw = extBytes(0x8000000000000001n, 0x3fff);
        bytes(DATA).set(raw, 64);
        const vector = bytes(DATA).slice(60, 76);
        reset({ eax: 0xdeadbeef, ecx: DATA + 60, edx: target, ebx: DATA + 96, edi: DATA + 128 });
        const mxcsr = state()[99], control = state()[59];
        const result = run(100);
        equal(result.kind, 8, "mixed target reaches precise UD2");
        equal(result.retired, 10, "all source and target mixed operations complete");
        equal(result.words[5], 0xdeadbeef, "target reads source MMX value before EMMS");
        equal(result.words[1], DATA + 64, "live ECX address crosses link");
        equal(result.words[0], 1, "MMX reads the new raw extended significand");
        equal(extRegister(result.words, 7).significand, 0x8000000000000001n, "physical80 significand unchanged");
        equal(extRegister(result.words, 7).exponent, 0x3fff, "x87 exponent not overwritten by stale MMX state");
        equal(result.words[59], control, "x87 control retained");
        equal(result.words[99], mxcsr, "MXCSR retained");
        for (let n = 0; n < 10; n++) equal(bytes(DATA)[96 + n], raw[n], "full m80 target store");
        for (let n = 0; n < 16; n++) equal(bytes(DATA)[128 + n], vector[n], "source XMM payload reaches target store");
      }));
    await test("linked entry rejects wrong compiled identity before executing guest work", () => fixture(
      [0x40], async ({ handle, select, reset, state, generatedUnits }) => {
        reset({ eax: 7 });
        const id = select(CODE), unit = generatedUnits.get(id), before = Array.from(state().slice(0, 100));
        const retired = unit.instance.exports.link(api.backend_state(handle), api.backend_table(handle), 7,
          ...before.slice(0, 8), before[9], before[8], 7, before[10], 1, id + 1, 4096);
        equal(retired, 0, "wrong identity retires no guest instruction");
        equal(state()[100], 3, "wrong identity uses explicit unsupported exit");
        equal(state()[103], 1, "wrong identity error");
        equal(state()[104], 0, "wrong identity completed count");
        for (let n = 0; n < 100; n++) equal(state()[n], before[n], "wrong identity preserves architectural state");
      }));
  }
  if (options.linkedRegions) {
    await test("linked SCAS preserves completed elements and entry flags across target page fault", () => fixture(
      [0x90, 0xff, 0xe2], async ({ map, compile, reset, run, bytes }) => {
        const target = 0x6000;
        map(target, READ | EXECUTE | SEALED, page([0xf3, 0xae, 0x0f, 0x0b], 0xcc));
        map(0x4000, READ, page([])); bytes(0x4000)[4095] = 0x55;
        await compile(target);
        reset({ eax: 0x55, ecx: 2, edx: target, edi: 0x4fff, flags: 0xad7 });
        const first = run(100);
        equal(first.retired, 2, "only source prefix retires before partial SCAS fault");
        equal(first.kind, 4, "target reports precise memory fault");
        equal(first.pc, target, "fault retains target string instruction");
        equal(first.words[1], 1, "completed string element remains consumed");
        equal(first.words[7], 0x5000, "destination points at faulting element");
        equal(first.words[9], 0xad7, "fault restores instruction-entry flags");
        map(0x5000, READ, page([0x55]));
        const resumed = run(100);
        equal(resumed.retired, 1, "repair retires the string once without replaying source prefix");
        equal(resumed.kind, 8, "repair reaches target UD2");
        equal(resumed.words[7], 0x5001, "only remaining element is compared");
      }));
  }
  if (hasTimestamp) {
    if (options.linkedRegions) await test("linked RDTSC completion counts the source and target prefixes once", () => fixture(
      [0x40, 0xff, 0xe2], async ({ handle, map, compile, reset, run, state }) => {
        const target = 0x6000;
        map(target, READ | EXECUTE | SEALED, page([0x41, 0x0f, 0x31, 0x0f, 0x0b], 0xcc));
        await compile(target);
        reset({ eax: 7, ecx: 9, edx: target });
        const first = run(100);
        equal(first.retired, 3, "source and target prefixes retire before timestamp");
        equal(first.kind, 15, "target timestamp creates completion exit");
        equal(first.pc, target + 1, "ticket belongs to terminal unit instruction");
        check(first.address > 0, "terminal unit owns nonzero completion ticket");
        ok(api.backend_complete_timestamp_counter(handle, target + 1, 0x12345678, 0x87654321), "complete terminal timestamp");
        equal(state()[104], 4, "completion appends exactly one instruction to the whole prefix");
        equal(state()[0], 0x12345678, "timestamp low word committed");
        equal(state()[2], 0x87654321, "timestamp high word committed");
        await compile(target + 3);
        const resumed = run(1);
        equal(resumed.kind, 8, "resume reaches target UD2");
        equal(resumed.retired, 0, "resume never replays the completed prefix");
      }));
    for (const prefix of [[], [0x66]]) {
      await test(`RDTSC ${prefix.length ? "prefixed" : "default"} completes full EDX:EAX once after committed effects`, () => fixture(
        [0xc6, 0x03, 7, ...prefix, 0x0f, 0x31, 0x43], async ({ handle, state, reset, run, compile, bytes }) => {
          reset({ eax: 0xaaaa5555, ecx: 22, edx: 0xbbbb6666, ebx: DATA, flags: 0xad7 });
          const before = Array.from(state().slice(0, 100));
          const first = run(100), pending = Array.from(state());
          equal(first.kind, 15, "timestamp produces explicit Runtime exit");
          equal(first.retired, 1, "only preceding store retires before clock completion");
          equal(first.pc, CODE + 3, "timestamp remains at its own EIP");
          equal(first.faultPc, CODE + 3, "timestamp records its instruction position");
          equal(first.error, prefix.length + 2, "decoded length is exact");
          check(first.address > 0, "timestamp exit owns a nonzero one-use ticket");
          equal(bytes(DATA)[0], 7, "preceding write completed exactly once");
          for (let n = 0; n < 100; n++) if (n !== 8) equal(state()[n], before[n], "timestamp leaves architectural input untouched");
          check(api.backend_complete_timestamp_counter(handle, CODE + 4, 1, 2) !== 0, "wrong EIP rejects completion");
          for (let n = 0; n < 105; n++) equal(state()[n], pending[n], "failed completion preserves pending context");
          ok(api.backend_complete_timestamp_counter(handle, CODE + 3, 0x89abcdef, 0x12345678), "complete Runtime sample");
          equal(state()[0], 0x89abcdef, "complete full EAX even with operand prefix");
          equal(state()[2], 0x12345678, "complete full EDX even with operand prefix");
          equal(state()[8], CODE + 5 + prefix.length, "advance by exact decoded length");
          equal(state()[100], 0, "timestamp exit cleared after completion");
          equal(state()[104], 2, "timestamp retirement added exactly once");
          for (let n = 0; n < 100; n++) if (![0, 2, 8].includes(n)) equal(state()[n], before[n], "completion preserves other architectural state");
          const completed = Array.from(state());
          check(api.backend_complete_timestamp_counter(handle, CODE + 3, 3, 4) !== 0, "duplicate completion rejects");
          for (let n = 0; n < 105; n++) equal(state()[n], completed[n], "duplicate completion cannot mutate state");
          await compile(state()[8]);
          const resumed = run(1);
          equal(resumed.words[3], DATA + 1, "following instruction resumes once");
          equal(bytes(DATA)[0], 7, "preceding write is not replayed");
        }));
    }
    await test("RDTSC changed architectural context rejects atomically and exact pending retry remains valid", () => fixture(
      [0x0f, 0x31], async ({ handle, state, reset, run }) => {
        reset(); run(1);
        const pending = Array.from(state());
        for (const index of [0, 9, 35, 61, 67, 99]) {
          state()[index] ^= 1;
          const changed = Array.from(state());
          check(api.backend_complete_timestamp_counter(handle, CODE, 1, 2) !== 0, `reject changed architectural word ${index}`);
          for (let n = 0; n < 105; n++) equal(state()[n], changed[n], "rejection cannot partially complete");
          state().set(pending);
        }
        ok(api.backend_complete_timestamp_counter(handle, CODE, 3, 4), "exact pending context permits explicit retry");
        equal(state()[0], 3, "retry low value"); equal(state()[2], 4, "retry high value");
        state().set(pending);
        check(api.backend_complete_timestamp_counter(handle, CODE, 5, 6) !== 0, "restoring consumed state does not revive a ticket");
        for (let n = 0; n < 105; n++) equal(state()[n], pending[n], "stale restored state remains unmodified");
      }));
    await test("RDTSC cancellation rejects even an identical restored pending context", () => fixture(
      [0x0f, 0x31], async ({ handle, state, reset, run }) => {
        reset(); run(1); const pending = Array.from(state());
        ok(api.backend_cancel_timestamp_counter(handle), "cancel pending Runtime operation");
        state().set(pending);
        check(api.backend_complete_timestamp_counter(handle, CODE, 1, 2) !== 0, "cancelled ticket cannot resume");
        for (let n = 0; n < 105; n++) equal(state()[n], pending[n], "cancelled completion preserves state");
      }));
    for (const change of ["code", "protection"]) {
      await test(`RDTSC ${change} change invalidates suspended completion`, () => fixture(
        [0x0f, 0x31], async ({ handle, state, reset, run, map, protect }) => {
          reset(); run(1); const pending = Array.from(state());
          if (change === "code") map(CODE, READ | EXECUTE | SEALED, page([0x90, 0x90], 0xcc));
          else protect(CODE, READ | SEALED);
          check(api.backend_complete_timestamp_counter(handle, CODE, 1, 2) !== 0, "changed instruction authority rejects completion");
          for (let n = 0; n < 105; n++) equal(state()[n], pending[n], "code rejection preserves pending CPU state");
        }));
    }
    await test("RDTSC shares imported x87 helper and preserves linked live flags before Runtime completion", () => fixture(
      [0x83, 0xc0, 1, 0xff, 0xe7], async ({ handle, map, compile, select, reset, run, bytes, state }) => {
        const target = 0x4000;
        map(target, READ | EXECUTE | SEALED, page([0xd9, 0x03, 0x0f, 0x31, 0x0f, 0x0b], 0xcc));
        await compile(target); setWord(bytes(DATA), 0, 0x3f800000);
        reset({ eax: 0xffffffff, ebx: DATA, edx: 55, edi: target }); select(CODE);
        let first = run(100), completedBefore = 0;
        if (!options.linkedRegions) {
          equal(first.kind, 2, "unlinked branch exposes target"); completedBefore = first.retired;
          select(first.pc); first = run(100);
        }
        equal(first.kind, 15, "target timestamp reaches Runtime");
        equal(first.pc, target + 2, "timestamp follows target x87 instruction");
        equal(first.retired + completedBefore, 3, "ADD, branch and FLD retire before timestamp");
        equal(first.words[0], 0, "live EAX reached target");
        equal(first.words[9] & (CF | ZF), CF | ZF, "live ADD flags survived helper and link");
        equal(extRegister(first.words, 7).significand, 0x8000000000000000n, "x87 helper executed before timestamp");
        equal(extRegister(first.words, 7).exponent, 0x3fff, "x87 raw exponent retained");
        const fp = Array.from(state().slice(35, 100));
        ok(api.backend_complete_timestamp_counter(handle, target + 2, 123, 456), "complete linked timestamp");
        equal(state()[104] + completedBefore, 4, "completion accounts for all guest work");
        for (let n = 0; n < fp.length; n++) equal(state()[35 + n], fp[n], "clock sample cannot modify floating state");
      }));
    await test("RDTSC zero budget does not request a sample and LOCK faults without a ticket", async () => {
      await fixture([0x0f, 0x31], async ({ handle, reset, run }) => {
        reset(); const idle = run(0);
        equal(idle.kind, 1, "zero-budget yield"); equal(idle.retired, 0, "zero-budget instruction untouched");
        equal(idle.address, 0, "zero-budget has no timestamp ticket");
        check(api.backend_complete_timestamp_counter(handle, CODE, 1, 2) !== 0, "no sample may complete before instruction");
        const requested = run(1);
        equal(requested.kind, 15, "one-instruction budget reaches clock request");
        equal(requested.retired, 0, "timestamp stays unretired until Runtime completes");
        check(requested.address > 0, "one-instruction request has a pending ticket");
      });
      await fixture([0xf0, 0x0f, 0x31], async ({ handle, reset, run }) => {
        reset({ eax: 7, edx: 11 }); const fault = run(100);
        equal(fault.kind, 8, "LOCK RDTSC is invalid opcode"); equal(fault.retired, 0, "invalid instruction not retired");
        equal(fault.words[0], 7, "invalid timestamp preserves EAX"); equal(fault.words[2], 11, "invalid timestamp preserves EDX");
        check(api.backend_complete_timestamp_counter(handle, CODE, 1, 2) !== 0, "invalid opcode has no pending clock request");
      });
    });
    await test("RDTSC completion resumes at a following fault without replaying a prior store", () => fixture(
      [0xc6, 0x03, 7, 0x0f, 0x31, 0x8b, 0x09], async ({ handle, map, compile, reset, run, bytes, state }) => {
        const missing = 0x6000;
        reset({ ebx: DATA, ecx: missing });
        const first = run(100);
        equal(first.kind, 15, "store reaches timestamp handoff"); equal(first.retired, 1, "prior store retired");
        ok(api.backend_complete_timestamp_counter(handle, CODE + 3, 123, 456), "complete before following fault");
        equal(state()[104], 2, "store and timestamp both complete");
        await compile(state()[8]);
        const fault = run(100);
        equal(fault.kind, 4, "following read faults precisely"); equal(fault.retired, 0, "failed read did not retire");
        equal(fault.pc, CODE + 5, "resume did not replay timestamp"); equal(fault.address, missing, "exact failing read address");
        equal(fault.words[0], 123, "completed low clock output retained"); equal(fault.words[2], 456, "completed high clock output retained");
        equal(bytes(DATA)[0], 7, "prior store remains visible once");
        map(missing, READ | WRITE, page([42])); bytes(DATA)[0] = 99;
        const resumed = run(1);
        equal(resumed.words[1], 42, "only failed read resumes");
        equal(bytes(DATA)[0], 99, "prior store was not replayed during fault repair");
      }));
    await test("SMC before RDTSC exits before issuing a sample and requires fresh code admission", () => fixture(
      [0xc6, 0x03, 0x90, 0x0f, 0x31], async ({ handle, alias, compile, reset, run, state }) => {
        alias(0x4000, CODE, READ | WRITE);
        reset({ ebx: 0x4100 });
        const first = run(100);
        equal(first.kind, 7, "code alias write exits before timestamp"); equal(first.retired, 1, "SMC store completed");
        equal(first.pc, CODE + 3, "timestamp remains unexecuted");
        check(api.backend_complete_timestamp_counter(handle, CODE + 3, 1, 2) !== 0, "SMC exit cannot manufacture a timestamp completion");
        check(api.backend_validate(handle, CODE) !== 0, "old compiled code rejected after SMC");
        await compile(CODE + 3);
        const second = run(1);
        equal(second.kind, 15, "freshly admitted timestamp requests Runtime sample");
        ok(api.backend_complete_timestamp_counter(handle, CODE + 3, 1, 2), "fresh timestamp completion");
        equal(state()[8], CODE + 5, "fresh completion advances exactly once");
      }));
    await test("RDTSC completion advances EIP with architectural 32-bit wrap", () => fixture(
      [0xcc], async ({ handle, map, compile, reset, run, state }) => {
        const code = page([], 0xcc); code.set([0x0f, 0x31], 4094);
        map(0xfffff000, READ | EXECUTE | SEALED, code); await compile(0xfffffffe, 1);
        reset({ eip: 0xfffffffe }); const first = run(1);
        equal(first.kind, 15, "final address timestamp request"); equal(first.pc, 0xfffffffe, "full EIP retained");
        ok(api.backend_complete_timestamp_counter(handle, 0xfffffffe, 1, 2), "complete wrapping timestamp");
        equal(state()[8], 0, "EIP wraps after completed two-byte instruction");
      }));
  }
  if (options.flowIr) {
    await test("flow scalar chains retain exact values at raw ingress, partial budgets and memory faults", () => {
      const code=[], instructions=[];
      for(let index=0;index<64;index++) {
        for(const [bytes,kind] of [[[0x8d,0x04,0x08],"a"],[[0x8d,0x14,0x02],"d"],[[0x92],"swap"]]) {
          instructions.push({pc:code.length,kind});code.push(...bytes);
        }
      }
      const loadPc=code.length;
      instructions.push({pc:code.length,kind:"load"});code.push(0x8b,0x37);
      instructions.push({pc:code.length,kind:"store"});code.push(0x89,0x03);
      const trapPc=code.length;code.push(0x0f,0x0b);
      return fixture(code,async({compile,select,reset,run,bytes})=>{
        const roots=[CODE,CODE+instructions[93].pc];
        const unit=await compile(CODE,256,roots);
        for(const entry of roots) for(const fault of [false,true])
          for(const budget of [0,1,2,3,5,31,64,127,191,192,193,194,195,999]) {
            setWord(bytes(DATA),0,0x11223344);setWord(bytes(DATA),4,0x55667788);
            reset({eip:entry,eax:0xfffffff9,ecx:0x70000003,edx:0x80000001,
              ebx:DATA,esi:0xaabbccdd,edi:fault?DATA+4096:DATA+4,flags:0xad7});
            equal(select(entry),unit,"raw scalar-chain entry retains its admission");
            let eax=0xfffffff9,edx=0x80000001,esi=0xaabbccdd,stored=0x11223344;
            let index=instructions.findIndex(i=>CODE+i.pc===entry),retired=0,kind=1;
            while(retired<budget) {
              const instruction=instructions[index];
              if(!instruction){kind=8;break;}
              if(instruction.kind==="a")eax=(eax+0x70000003)>>>0;
              else if(instruction.kind==="d")edx=(edx+eax)>>>0;
              else if(instruction.kind==="swap")[eax,edx]=[edx,eax];
              else if(instruction.kind==="load"){if(fault){kind=4;break;}esi=0x55667788;}
              else stored=eax;
              index++;retired++;
            }
            const actual=run(budget);
            equal(actual.kind,kind,"chain precise stop");equal(actual.retired,retired,"chain completed prefix");
            equal(actual.pc,CODE+(instructions[index]?.pc??trapPc),"chain exact PC");
            equal(actual.words[0],eax,"chain EAX");equal(actual.words[2],edx,"chain EDX");
            equal(actual.words[6],esi,"load publishes only after success");
            equal(actual.words[9],0xad7,"address arithmetic preserves incoming flags");
            equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(0,true),stored,"store occurs only after successful read");
            if(kind===4){equal(actual.address,DATA+4096,"chain fault address");equal(actual.faultPc,CODE+loadPc,"chain fault instruction");}
          }
      });
    });
    await test("flow eliminates overwritten scalar work without losing partial-budget or fault state", () => {
      const code=[], instructions=[];
      const emit=(bytes,kind,value)=>{instructions.push({pc:code.length,kind,value});code.push(...bytes);};
      for(let index=0;index<24;index++) {
        emit([0x01,0xc8],"addAC");
        emit([0x01,0xc2],"addDA");
        const a=(0x7ffffffe+index)>>>0,d=(0xfffffff0-index)>>>0;
        emit([0xb8,...wordBytes(a)],"movA",a);
        emit([0x01,0xd0],"addAD");
        emit([0xba,...wordBytes(d)],"movD",d);
        if(index===11)emit([0x8b,0x3e],"load");
      }
      emit([0x89,0x03],"store");
      const trapPc=code.length;code.push(0x0f,0x0b);
      return fixture(code,async({compile,select,reset,run,bytes})=>{
        const roots=[CODE,CODE+instructions[48].pc];
        const unit=await compile(CODE,256,roots);
        for(const entry of roots) for(const fault of [false,true])
          for(const ecx of [1,0x80000000]) for(let budget=0;budget<=instructions.length+1;budget++) {
            setWord(bytes(DATA),0,0x11223344);setWord(bytes(DATA),4,0x55667788);
            reset({eip:entry,eax:0xfffffffe,ecx,edx:0x7fffffff,ebx:DATA,
              esi:fault?DATA+4096:DATA+4,edi:0xaabbccdd,flags:0xad7});
            equal(select(entry),unit,"overwritten-work raw port remains in the same unit");
            let eax=0xfffffffe,edx=0x7fffffff,edi=0xaabbccdd,flags=0xad7,stored=0x11223344;
            let index=instructions.findIndex(i=>CODE+i.pc===entry),retired=0,kind=1;
            const add=(a,b)=>{
              const r=(a+b)>>>0;let bits=0,byte=r&255;for(;byte;byte>>>=1)bits+=byte&1;
              flags=((flags&~ARITH)|(a>0xffffffff-b?CF:0)|(bits%2===0?PF:0)|
                ((a^b^r)&AF)|(r===0?ZF:0)|(r&0x80000000?SF:0)|
                ((~(a^b)&(a^r))>>>31?OF:0))>>>0;
              return r;
            };
            while(retired<budget) {
              const instruction=instructions[index];if(!instruction){kind=8;break;}
              if(instruction.kind==="addAC")eax=add(eax,ecx);
              else if(instruction.kind==="addDA")edx=add(edx,eax);
              else if(instruction.kind==="addAD")eax=add(eax,edx);
              else if(instruction.kind==="movA")eax=instruction.value;
              else if(instruction.kind==="movD")edx=instruction.value;
              else if(instruction.kind==="load"){if(fault){kind=4;break;}edi=0x55667788;}
              else stored=eax;
              index++;retired++;
            }
            const actual=run(budget);
            equal(actual.kind,kind,"overwritten work exact exit");equal(actual.retired,retired,"overwritten work completed prefix");
            equal(actual.pc,CODE+(instructions[index]?.pc??trapPc),"overwritten work exact PC");
            equal(actual.words[0],eax,"overwritten EAX");equal(actual.words[2],edx,"overwritten EDX");
            equal(actual.words[7],edi,"fault preserves load destination");equal(actual.words[9],flags,"flags before an overwrite remain recoverable");
            equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(0,true),stored,"later store cannot cross the fault");
            if(kind===4){equal(actual.address,DATA+4096,"overwritten-work fault address");equal(actual.faultPc,actual.pc,"overwritten-work exact fault instruction");}
          }
      });
    });
    await test("flow merges retain incoming flags at faults, budgets and raw entries", () => {
      const code=[0x74,8,0x83,0xc0,1,0xe9,6,0,0,0,0x83,0xc0,2,0xeb,1,0xcc,
        0x8b,0x1e,0x0f,0x92,0xc2,0x0f,0x0b];
      return fixture(code,async({compile,select,reset,run,bytes})=>{
        const roots=[CODE,CODE+2,CODE+10,CODE+16];
        const unit=await compile(CODE,64,roots);
        setWord(bytes(DATA),0,0x76543210);
        for(const entry of roots) for(const a of [0,0x7ffffffe,0xfffffffe,0xffffffff])
          for(const initialFlags of [0x202,0xad7]) for(const fault of [false,true])
            for(const budget of [0,1,2,3,4,5,6,9]) {
              reset({eip:entry,eax:a,ebx:0x33445566,edx:0x12345678,
                esi:fault?DATA+4096:DATA,flags:initialFlags});
              equal(select(entry),unit,"raw entry remains admitted to the same unit");
              let pc=entry-CODE,eax=a,ebx=0x33445566,edx=0x12345678,flags=initialFlags,retired=0,kind=1;
              while(retired<budget) {
                if(pc===21){kind=8;break;}
                if(pc===0)pc=flags&ZF?10:2;
                else if(pc===2||pc===10){
                  const n=pc===2?1:2,r=(eax+n)>>>0;
                  let bits=0,byte=r&255;for(;byte;byte>>>=1)bits+=byte&1;
                  flags=((flags&~ARITH)|(eax>0xffffffff-n?CF:0)|(bits%2===0?PF:0)|
                    ((eax^n^r)&AF)|(r===0?ZF:0)|(r&0x80000000?SF:0)|
                    ((~(eax^n)&(eax^r))>>>31?OF:0))>>>0;
                  eax=r;pc+=3;
                }else if(pc===5||pc===13)pc=16;
                else if(pc===16){if(fault){kind=4;break;}ebx=0x76543210;pc=18;}
                else if(pc===18){edx=((edx&0xffffff00)|(flags&CF?1:0))>>>0;pc=21;}
                else throw Error("invalid merged-flow reference PC");
                retired++;
              }
              const actual=run(budget);
              equal(actual.kind,kind,"exact budget/fault/UD exit");equal(actual.retired,retired,"exact completed prefix");
              equal(actual.pc,CODE+pc,"merged continuation PC");
              equal(actual.words[0],eax,"merged arithmetic result");equal(actual.words[3],ebx,"load commits only on success");
              equal(actual.words[2],edx,"carry consumer uses this incoming edge");equal(actual.words[9],flags,"all flags reconstruct this incoming edge");
              if(kind===4){equal(actual.address,DATA+4096,"checked load address");equal(actual.faultPc,CODE+16,"fault belongs to the unretired load");}
            }
      });
    });
    await test("flow SSA loop permutations preserve arbitrary AOT ports and indirect ingress at every budget", () => {
      const code = Array(22).fill(0xcc);
      code.splice(0,3,0x92,0xeb,5); // XCHG EAX,EDX; JMP second block
      code.splice(8,7,0x01,0xd8,0x49,0x75,0xf3,0x0f,0x0b); // ADD EAX,EBX; DEC ECX; JNZ first; UD2
      code.splice(20,2,0xff,0xe7); // JMP EDI, no C ABI assumption
      const szp = r => {
        let byte=r&255,count=0;for(;byte;byte>>>=1)count+=byte&1;
        return (count%2===0?PF:0)|(r===0?ZF:0)|(r&0x80000000?SF:0);
      };
      return fixture(code,async({compile,select,reset,run})=>{
        const roots=[CODE,CODE+8,CODE+20];
        const unit=await compile(CODE,64,roots);
        for(const entry of roots) for(const count of [1,4]) for(const budget of [0,1,2,3,4,5,6,7,10,19,37]) {
          reset({eax:0x7ffffffe,edx:0x80000003,ebx:0xfffffffb,ecx:count,edi:CODE+8,eip:entry,flags:0xad7});
          equal(select(entry),unit,"every arbitrary port reuses the same compiled unit");
          const expected=[0x7ffffffe,count,0x80000003,0xfffffffb];
          let pc=entry-CODE,flags=0xad7,retired=0,kind=1;
          while(retired<budget) {
            if(pc===13){kind=8;break;}
            if(pc===0){[expected[0],expected[2]]=[expected[2],expected[0]];pc=1;}
            else if(pc===1)pc=8;
            else if(pc===8){
              const a=expected[0],b=expected[3],wide=BigInt(a)+BigInt(b),r=Number(wide&0xffffffffn);
              flags=((flags&~ARITH)|szp(r)|(wide>0xffffffffn?CF:0)|((a^b^r)&AF)|((~(a^b)&(a^r))>>>31?OF:0))>>>0;
              expected[0]=r;pc=10;
            }else if(pc===10){
              const a=expected[1],r=(a-1)>>>0;
              flags=((flags&~(ARITH&~CF))|szp(r)|((a^1^r)&AF)|(a===0x80000000?OF:0))>>>0;
              expected[1]=r;pc=11;
            }else if(pc===11)pc=flags&ZF?13:0;
            else if(pc===20)pc=8;
            else throw Error("invalid independent fixture reference PC");
            retired++;
          }
          const actual=run(budget);
          equal(actual.kind,kind,"budget or exact UD fault");equal(actual.retired,retired,"completed prefix");
          equal(actual.pc,CODE+pc,"exact loop/interior continuation");
          if(kind===8)equal(actual.faultPc,CODE+13,"fault never reexecutes a completed permutation");
          for(let i=0;i<4;i++)equal(actual.words[i],expected[i],`loop permutation GPR ${i}`);
          equal(actual.words[9],flags,"all arithmetic and nonarithmetic flags");
        }
      });
    });
  }
  for (const [operation, opcode] of [["add",0x02],["adc",0x12],["sub",0x2a],["sbb",0x1a],["xor",0x32]]) {
    for (const width of [8,16,32]) await test(`deferred flags ${operation}/${width} survive joins and overwritten inputs`, async () => {
      const instructions = [], code = [];
      const add = (bytes, effect = () => {}) => { instructions.push({pc:code.length,effect}); code.push(...bytes); };
      add([...(width===16?[0x66]:[]),opcode+(width===8?0:1),0x07], s => {
        const result = operation==="xor"
          ? {result:Number((BigInt(s.regs[0])^BigInt(s.memory))&widthMask(width)),flags:0}
          : arithmetic(operation,width,s.regs[0],s.memory,s.flags&CF);
        if(operation==="xor")result.flags=common(BigInt(result.result),width);
        const written=operation==="xor"?ARITH&~AF:ARITH;
        s.regs[0]=retainedResult(s.regs[0],result.result,width);
        s.flags=((s.flags&~written)|result.flags)>>>0;
        s.defined=(operation==="xor"?s.defined&~AF:s.defined|ARITH)>>>0;
      });
      add([0xeb,0]);
      add([0xb8,...wordBytes(0x13579bdf)],s=>{s.regs[0]=0x13579bdf;});
      add([0xbb,...wordBytes(0x2468ace0)],s=>{s.regs[3]=0x2468ace0;});
      add([0xc7,0x07,...wordBytes(0xfedcba98)],s=>{s.memory=0xfedcba98;});
      add([0xeb,0]);
      for(const [condition,register,shift,bit] of [[2,2,0,CF],[10,2,8,PF],[4,3,0,ZF],[8,3,8,SF],[0,0,0,OF]]) {
        add([0x0f,0x90+condition,0xc0+register+(shift?4:0)],s=>{
          s.regs[register]=((s.regs[register]&~(255<<shift))|((s.flags&bit?1:0)<<shift))>>>0;
        });
      }
      const end=code.length;code.push(0x0f,0x0b);
      await fixture(code,async ({compile,reset,run,bytes})=>{
        await compile(CODE,256,[...instructions.map(i=>CODE+i.pc),CODE+end]);
        for(const [a,b] of [[0,0],[0xffffffff,1],[0x7fffffff,1],[0x80000000,0xffffffff],[0x0f,1]]) {
          for(const entry of [0,1,2,5,6,9,11]) for(const budget of [0,1,2,4,7,12]) {
            const expected={regs:[a,3,0xabcddcba,0x31415926],memory:b,flags:0xad7,defined:0xffffffff};
            const completed=Math.min(budget,instructions.length-entry);
            for(let index=entry;index<entry+completed;index++)instructions[index].effect(expected);
            const pc=instructions[entry+completed]?.pc??end;
            setWord(bytes(DATA),0,b);
            reset({eip:CODE+(instructions[entry]?.pc??end),eax:a,ecx:3,edx:0xabcddcba,ebx:0x31415926,edi:DATA,flags:0xad7});
            const actual=run(budget);
            equal(actual.kind,budget>completed?8:1,"raw entry or exact budget reaches its own prefix");
            equal(actual.pc,CODE+pc,"precise consumer/producer continuation");equal(actual.retired,completed,"no input or producer replay");
            for(let index=0;index<4;index++)equal(actual.words[index],expected.regs[index],`captured-input GPR ${index}`);
            equal(actual.words[9],expected.flags,"all flags survive source register and memory overwrite");
            equal(actual.words[10],expected.defined,"undefined flag mask survives join");
            equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(0,true),expected.memory,"only reached store commits");
          }
        }
      });
    });
  }
  await test("deferred flags different incoming producers preserve carry through partial writers",()=>fixture(
    [0xe3,4,0x01,0xd8,0xeb,2,0x19,0xd8,0xeb,0,0x42,0xeb,0,0x83,0xd3,0,0xeb,0,0xf7,0xd9,0xeb,0,0x85,0xc0,0x0f,0x0b],
    async({compile,reset,run})=>{
      const entries=[0,2,6,8,10,11,13,16,18,20,22,24];await compile(CODE,256,entries.map(pc=>CODE+pc));
      for(const entry of entries)for(const mode of [0,1])for(const [a,b] of [[0xffffffff,1],[0,1],[0x7fffffff,0xffffffff]])for(const budget of [0,1,2,3,5,8,12,20]){
        const regs=[a,mode,0x7fffffff,b];let pc=entry,flags=0xad7,defined=0xffffffff,retired=0;
        const change=(index,op,left,right,mask=ARITH)=>{const value=arithmetic(op,32,left,right,flags&CF);regs[index]=value.result;flags=((flags&~mask)|(value.flags&mask))>>>0;defined=(defined|mask)>>>0;};
        while(retired<budget&&pc!==24){
          if(pc===0)pc=regs[1]===0?6:2;
          else if(pc===2){change(0,"add",regs[0],regs[3]);pc=4;}
          else if(pc===4)pc=8;
          else if(pc===6){change(0,"sbb",regs[0],regs[3]);pc=8;}
          else if([8,11,16,20].includes(pc))pc+=2;
          else if(pc===10){change(2,"add",regs[2],1,ARITH&~CF);pc=11;}
          else if(pc===13){change(3,"adc",regs[3],0);pc=16;}
          else if(pc===18){change(1,"sub",0,regs[1]);pc=20;}
          else if(pc===22){flags=((flags&~(ARITH&~AF))|common(BigInt(regs[0]),32))>>>0;defined=(defined&~AF)>>>0;pc=24;}
          else throw Error("invalid producer-join reference PC");
          retired++;
        }
        reset({eip:CODE+entry,eax:a,ecx:mode,edx:0x7fffffff,ebx:b,flags:0xad7});const actual=run(budget);
        equal(actual.pc,CODE+pc,"joined flag continuation");equal(actual.retired,retired,"joined flag retirement");equal(actual.kind,retired<budget?8:1,"joined budget or UD");
        for(let index=0;index<4;index++)equal(actual.words[index],regs[index],`joined GPR ${index}`);
        equal(actual.words[9],flags,"carry and preserved flags belong to the actual predecessor");equal(actual.words[10],defined,"partial writer definition mask");
      }
    }));
  await test("deferred flags RMW fault preserves prior producer then resumes once",()=>fixture(
    [0x01,0xd8,0xeb,0,0x83,0x17,0,0xeb,0,0x0f,0x0b],async({reset,run,map,bytes})=>{
      reset({eax:0xffffffff,ebx:1,edi:0x4000,flags:0x202});const fault=run(100);
      equal(fault.kind,4,"unmapped ADC destination faults");equal(fault.pc,CODE+4,"fault names unretired producer");equal(fault.retired,2,"prior ADD and branch only");
      equal(fault.words[0],0,"prior result survives");equal(fault.words[9],0x202|arithmetic("add",32,0xffffffff,1,0).flags,"prior carry survives failed replacement");
      map(0x4000,READ|WRITE);setWord(bytes(0x4000),0,0xffffffff);const resumed=run(100);
      equal(resumed.kind,8,"repaired RMW reaches UD");equal(resumed.retired,2,"repair does not replay prior ADD");
      equal(new DataView(bytes(0x4000).buffer,bytes(0x4000).byteOffset).getUint32(0,true),0,"repaired ADC consumes saved architectural carry once");
      equal(resumed.words[9],0x202|arithmetic("adc",32,0xffffffff,0,1).flags,"repaired producer supplies flags");
    }));
  await test("deferred flags committed code alias RMW publishes its own flags",()=>fixture(
    [0x01,0xd8,0xeb,0,0x83,0x17,0,0x42,0x0f,0x0b],async({reset,run,alias,bytes})=>{
      alias(0x4000,CODE,READ|WRITE);setWord(bytes(CODE),0x100,0xffffffff);
      reset({eax:0xffffffff,ebx:1,edx:17,edi:0x4100,flags:0x202});const result=run(100);
      equal(result.kind,7,"code alias write invalidates current unit");equal(result.pc,CODE+7,"SMC follows committed ADC");equal(result.retired,3,"completed RMW retires once");
      equal(result.words[2],17,"following INC never executes");equal(result.words[9],0x202|arithmetic("adc",32,0xffffffff,0,1).flags,"SMC resolves the committed producer");
      equal(new DataView(bytes(CODE).buffer,bytes(CODE).byteOffset).getUint32(0x100,true),0,"code bytes commit once");
    }));
  await test("deferred flags zero-count shift and comparison preserve incoming flags",()=>fixture(
    [0x01,0xd8,0xeb,0,0xd3,0xe2,0xeb,0,0xf3,0xa7,0xeb,0,0x0f,0x92,0xc2,0x0f,0x0b],async({reset,run})=>{
      for(const budget of [1,2,3,4,5,6,7,20]){
        reset({eax:0xffffffff,ebx:1,ecx:0,edx:0x12345678,esi:0x4000,edi:0x5000,flags:0x202});const result=run(budget);
        equal(result.retired,Math.min(budget,7),"zero-count operations retire without touching absent spans");equal(result.kind,budget>7?8:1,"zero-count paths remain resumable");
        equal(result.words[9],0x202|arithmetic("add",32,0xffffffff,1,0).flags,"zero counts preserve every prior flag");equal(result.words[10],0xffffffff,"zero counts preserve definition mask");
        equal(result.words[2],budget>=7?0x12345601:0x12345678,"carry consumer follows both no-op writers");
      }
    }));
  await test("flag lookahead preserves narrow and high-byte producers at every loop budget",async()=>{
    for(const [width,shift,code] of [[8,0,[0x80,0xc0,1]],[8,8,[0x80,0xc4,1]],[16,0,[0x66,0x83,0xc0,1]],[32,0,[0x83,0xc0,1]]]){
      const addLength=code.length;
      await fixture([...code,0xeb,(-(addLength+2))&255],async({reset,run})=>{
        for(const initial of [0,0xffffffff,0x7fffffff,0x80ff7fff,0x12347f00])for(let budget=0;budget<=37;budget++){
          let eax=initial,flags=0xad7;
          for(let n=0;n<Math.ceil(budget/2);n++){
            const result=arithmetic("add",width,eax>>>shift,1,0);
            eax=shift?((eax&0xffff00ff)|(result.result<<8))>>>0:retainedResult(eax,result.result,width);
            flags=((flags&~ARITH)|result.flags)>>>0;
          }
          reset({eax:initial,flags:0xad7});const actual=run(budget);
          equal(actual.kind,1,"bounded loop yields");equal(actual.retired,budget,"loop counts every branch");equal(actual.pc,CODE+(budget%2?addLength:0),"loop prefix PC");
          equal(actual.words[0],eax,"partial register writes preserve other bits");equal(actual.words[9],flags,"last completed narrow producer owns flags");equal(actual.words[10],0xffffffff,"all arithmetic bits remain defined");
        }
      });
    }
  });
  await test("flag lookahead keeps every budget prefix across unequal loop arms and raw entries", () => fixture(
    [0x83,0xc0,1, 0xeb,3, 0xcc,0xcc,0xcc, 0x43,0x85,0xc9,0x74,5,
      0x31,0xd2,0xeb,0xef,0xcc, 0x83,0xee,1,0xeb,0xe9],
    async({compile,reset,run})=>{
      await compile(CODE,256,[0,8,13,18].map(pc=>CODE+pc));
      for(const entry of [0,8,13,18])for(const ecx of [0,1])for(const eax of [0,0xffffffff,0x7fffffff]){
        for(let budget=0;budget<=65;budget++){
          const regs=[eax,ecx,0x89abcdef,0x7fffffff,STACK+0x800,0,3,0];
          let pc=entry,flags=0xad7,defined=0xffffffff;
          const change=(index,op,right,mask=ARITH)=>{
            const result=arithmetic(op,32,regs[index],right,flags&CF);
            regs[index]=result.result;flags=((flags&~mask)|(result.flags&mask))>>>0;defined=(defined|mask)>>>0;
          };
          for(let n=0;n<budget;n++){
            if(pc===0){change(0,"add",1);pc=3;}
            else if(pc===3)pc=8;
            else if(pc===8){change(3,"add",1,ARITH&~CF);pc=9;}
            else if(pc===9){flags=((flags&~(ARITH&~AF))|common(BigInt(regs[1]),32))>>>0;defined=(defined&~AF)>>>0;pc=11;}
            else if(pc===11)pc=flags&ZF?18:13;
            else if(pc===13){regs[2]=0;flags=((flags&~(ARITH&~AF))|ZF|PF)>>>0;defined=(defined&~AF)>>>0;pc=15;}
            else if(pc===18){change(6,"sub",1);pc=21;}
            else if(pc===15||pc===21)pc=0;
            else throw Error("invalid flag lookahead reference PC");
          }
          reset({eip:CODE+entry,eax,ecx,edx:0x89abcdef,ebx:0x7fffffff,esi:3,flags:0xad7});
          const actual=run(budget);
          equal(actual.kind,1,"every bounded execution yields");equal(actual.retired,budget,"no branch or producer replay");equal(actual.pc,CODE+pc,"exact prefix PC");
          for(let r=0;r<8;r++)equal(actual.words[r],regs[r],`prefix GPR ${r}`);
          // Undefined AF may retain an older producer; defined bits and their
          // validity must match even at the last instruction of either arm.
          equal((actual.words[9]&defined)>>>0,(flags&defined)>>>0,"defined prefix flags");equal(actual.words[10],defined,"prefix flag validity");
        }
      }
    }));
  await test("flag lookahead observes a fault before the replacement on either incoming arm",()=>fixture(
    [0x83,0xc0,1,0xe3,5,0x83,0xc3,1,0xeb,2,0x8b,0x17,0x83,0xee,1,0x0f,0x0b],
    async({compile,reset,run,map})=>{
      await compile(CODE,256,[CODE,CODE+5,CODE+10,CODE+12]);
      reset({eax:0xffffffff,ecx:0,esi:0,edi:0x4000,flags:0x202});const fault=run(30);
      equal(fault.kind,4,"faulting arm is observable");equal(fault.pc,CODE+10,"fault before later flag replacement");equal(fault.retired,2,"branch prefix retained");
      equal(fault.words[9],0x202|arithmetic("add",32,0xffffffff,1,0).flags,"fault sees the prior producer");
      map(0x4000,READ);const resumed=run(30);
      equal(resumed.kind,8,"repair reaches UD");equal(resumed.retired,2,"repair resumes without replay");equal(resumed.words[6],0xffffffff,"later subtraction exactly once");
      reset({eax:0xffffffff,ecx:1,ebx:0x7fffffff,esi:0,edi:0x5000,flags:0x202});const other=run(30);
      equal(other.kind,8,"nonfaulting arm never reads absent pointer");equal(other.words[3],0x80000000,"other arm result");
    }));
  await test("flag lookahead stops at code invalidation before a successor replacement",()=>fixture(
    [0x83,0xc0,1,0xeb,0,0x89,0x1f,0xeb,0,0x83,0xc2,1,0x0f,0x0b],
    async({reset,run,alias,bytes})=>{
      alias(0x4000,CODE,READ|WRITE);
      reset({eax:0x7fffffff,ebx:0x13579bdf,edx:17,edi:0x4100,flags:0x202});const r=run(30);
      equal(r.kind,7,"code write ends invocation");equal(r.pc,CODE+7,"SMC continuation");equal(r.retired,3,"only pre-store instructions and store retire");
      equal(r.words[9],0x202|arithmetic("add",32,0x7fffffff,1,0).flags,"flags are observable at SMC");equal(r.words[2],17,"successor is unexecuted");
      equal(new DataView(bytes(CODE).buffer,bytes(CODE).byteOffset).getUint32(0x100,true),0x13579bdf,"store commits once");
    }));
  await test("flag lookahead preserves flags at unknown and later bound native unit edges",()=>fixture(
    [0x0f,0x0b],
    async({map,compile,select,reset,run})=>{
      const entry=0x3000;
      map(entry,READ|EXECUTE|SEALED,page([0x83,0xc0,1,0xeb,0,0x83,0xc3,1,0x0f,0x0b],0xcc));
      await compile(entry,2);
      reset({eip:entry,eax:0xffffffff,ebx:7,flags:0x202});const outside=run(10);
      equal(outside.kind,2,"untranslated edge exits");equal(outside.pc,entry+5,"exact external target");equal(outside.retired,2,"external edge prefix");
      equal(outside.words[9],0x202|arithmetic("add",32,0xffffffff,1,0).flags,"outside state includes predecessor flags");
      await compile(entry+5);select(entry);reset({eip:entry,eax:0xffffffff,ebx:7,flags:0x202});
      const boundary=run(2);equal(boundary.kind,1,"budget exhausted at a bound edge");equal(boundary.pc,entry+5,"bound edge keeps exact budget PC");
      equal(boundary.words[9],outside.words[9],"native binding does not hide flags at zero budget");
    }));
  await test("scalar rematerialization preserves affine copies, cycles, narrow writes and every prefix", async () => {
    const instructions = [
      [0x89,0xd8], [0x8d,0x44,0x80,7], [0x89,0xc1], [0x92], [0xf7,0xd1],
      [0xbb,0xfe,0xff,0xff,0xff], [0xb7,0x80], [0x0f,0xb6,0xf7], [0x0f,0xbe,0xfb],
      [0x8d,0x6c,0x52,0xfb], [0x66,0xba,0x65,0x87], [0x0f,0x44,0xc1], [0x55], [0x58],
      [0x89,0x0d,...wordBytes(DATA)],
    ];
    const offsets = [0]; for (const bytes of instructions) offsets.push(offsets.at(-1)+bytes.length);
    await fixture([...instructions.flat(),0x0f,0x0b], async ({compile,reset,run,bytes}) => {
      for (const rawEntries of [false,true]) {
        if (rawEntries) await compile(CODE,256,offsets.map(offset=>CODE+offset));
        for (const entry of rawEntries ? [0,3,5,10,12] : [0])
          for (const input of [0,1,0x7fffffff,0x80000000,0xffffffff])
            for (const flags of [0x202,0x242]) for (let budget=0;budget<=instructions.length+1;budget++) {
              const regs=[0x1234abcd,0x55667788,0x89abcdef,input,STACK+0x800,0,0,0];
              let stored=0,stack=0;
              const steps=[()=>{regs[0]=regs[3];},()=>{regs[0]=(Math.imul(regs[0],5)+7)>>>0;},
                ()=>{regs[1]=regs[0];},()=>{[regs[0],regs[2]]=[regs[2],regs[0]];},()=>{regs[1]=(~regs[1])>>>0;},
                ()=>{regs[3]=0xfffffffe;},()=>{regs[3]=((regs[3]&0xffff00ff)|0x8000)>>>0;},
                ()=>{regs[6]=(regs[3]>>>8)&255;},()=>{regs[7]=((regs[3]<<24)>>24)>>>0;},
                ()=>{regs[5]=(Math.imul(regs[2],3)-5)>>>0;},()=>{regs[2]=((regs[2]&0xffff0000)|0x8765)>>>0;},
                ()=>{if(flags&ZF)regs[0]=regs[1];},()=>{regs[4]-=4;stack=regs[5];},()=>{regs[0]=stack;regs[4]+=4;},
                ()=>{stored=regs[1];}];
              const completed=Math.min(budget,steps.length-entry);
              for(let n=0;n<completed;n++)steps[entry+n]();
              setWord(bytes(DATA),0,0);setWord(bytes(STACK),0x7fc,0);
              reset({eip:CODE+offsets[entry],eax:0x1234abcd,ecx:0x55667788,edx:0x89abcdef,ebx:input,flags});
              const actual=run(budget);
              equal(actual.kind,budget>completed?8:1,"exact prefix or UD boundary");
              equal(actual.retired,completed,"deferred computations retire exactly once");
              equal(actual.pc,CODE+offsets[entry+completed],"exact rematerialized prefix PC");
              for(let r=0;r<8;r++)equal(actual.words[r],regs[r],`rematerialized register ${r}`);
              equal(actual.words[9],flags,"pure scalar composition preserves flags");
              equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(0,true),stored,"ordered data store");
              equal(new DataView(bytes(STACK).buffer,bytes(STACK).byteOffset).getUint32(0x7fc,true),stack,"ordered stack store");
            }
      }
    });
  });
  await test("scalar rematerialization recovers exchange cycles before a split load and resumes once",()=>fixture(
    [0x91,0x89,0xc2,0x8d,0x3c,0x95,0xf8,0xff,0xff,0xff,0x8b,0x03,0x0f,0x0b],
    async({reset,run,map,bytes,compile})=>{
      setWord(bytes(DATA),4092,0x22110000);
      reset({eax:17,ecx:0x80000001,edx:99,ebx:DATA+4094,flags:0xad7});
      const fault=run(100);
      equal(fault.kind,4,"split source faults");equal(fault.address,DATA+4096,"upper page owns fault");
      equal(fault.pc,CODE+10,"precise fault instruction");equal(fault.retired,3,"pure prefix retires before fault");
      for(const [index,value] of [[0,0x80000001],[1,17],[2,0x80000001],[7,0xfffffffc]])equal(fault.words[index],value,"cyclic bindings recover old physical values");
      equal(fault.words[9],0xad7,"fault leaves flags intact");
      map(DATA+4096,READ|WRITE);bytes(DATA+4096)[0]=0x33;bytes(DATA+4096)[1]=0x44;
      await compile(CODE+10); // The product JIT admits this interior resume.
      const repaired=run(100);
      equal(repaired.kind,8,"repaired split read completes");equal(repaired.retired,1,"repair never replays exchange or LEA");
      equal(repaired.words[0],0x44332211,"loaded bytes replace destination only after success");
      for(const index of [1,2,7])equal(repaired.words[index],fault.words[index],"dependent copies survive destination overwrite");
    }));
  await test("scalar rematerialization publishes exact values on a committed code-alias store",()=>fixture(
    [0x91,0x89,0xc2,0x8d,0x3c,0x95,0xf8,0xff,0xff,0xff,0x89,0x03,0xb8,7,0,0,0,0x0f,0x0b],
    async({reset,run,alias,bytes})=>{
      alias(0x4000,CODE,READ|WRITE);reset({eax:17,ecx:0x80000001,edx:99,ebx:0x4100,flags:0xad7});
      const actual=run(100);
      equal(actual.kind,7,"committed alias store invalidates code");equal(actual.pc,CODE+12,"SMC exits before later assignment");equal(actual.retired,4,"SMC includes exactly the committed store");
      for(const [index,value] of [[0,0x80000001],[1,17],[2,0x80000001],[7,0xfffffffc]])equal(actual.words[index],value,"SMC publishes all deferred scalar results");
      equal(actual.words[9],0xad7,"SMC retains pure-prefix flags");
      equal(new DataView(bytes(CODE).buffer,bytes(CODE).byteOffset).getUint32(0x100,true),0x80000001,"alias store observes logical source");
    }));
  await test("scalar rematerialization keeps flags and values across affine arithmetic and a raw branch entry",()=>fixture(
    [0x89,0xd8,0x83,0xc0,3,0x89,0xc1,0x83,0xe9,5,0xf7,0xd1,0x8d,0x54,0x49,7,0x39,0xfe,0xeb,0xec],
    async({compile,reset,run})=>{
      await compile(CODE,256,[CODE,CODE+12]);
      for(const entry of [0,12])for(const input of [0,1,0x7fffffff,0x80000000,0xffffffff])for(let budget=0;budget<=64;budget++){
        const regs=[0x11223344,0x55667788,99,input,STACK+0x800,0,5,7];let pc=entry,flags=0xad7;
        const change=(index,op,value)=>{const result=arithmetic(op,32,regs[index],value,0);regs[index]=result.result;flags=((flags&~ARITH)|result.flags)>>>0;};
        for(let n=0;n<budget;n++){
          if(pc===0){regs[0]=regs[3];pc=2;}else if(pc===2){change(0,"add",3);pc=5;}
          else if(pc===5){regs[1]=regs[0];pc=7;}else if(pc===7){change(1,"sub",5);pc=10;}
          else if(pc===10){regs[1]=(~regs[1])>>>0;pc=12;}else if(pc===12){regs[2]=(Math.imul(regs[1],3)+7)>>>0;pc=16;}
          else if(pc===16){flags=((flags&~ARITH)|arithmetic("sub",32,5,7,0).flags)>>>0;pc=18;}else if(pc===18)pc=0;
          else throw Error("invalid rematerialization reference PC");
        }
        reset({eip:CODE+entry,eax:0x11223344,ecx:0x55667788,edx:99,ebx:input,esi:5,edi:7,flags:0xad7});
        const actual=run(budget);equal(actual.kind,1,"affine arithmetic loop yields");equal(actual.retired,budget,"all composed arithmetic retires");equal(actual.pc,CODE+pc,"precise affine loop prefix");
        for(let r=0;r<8;r++)equal(actual.words[r],regs[r],`affine arithmetic GPR ${r}`);
        equal(actual.words[9],flags,"short budgets expose intermediate flags");equal(actual.words[10],0xffffffff,"arithmetic definition remains exact");
      }
    }));
  await test("scalar demand preserves every prefix across dead chains, unequal arms and raw entries",()=>fixture(
    [0x89,0xd8,0x83,0xc0,1,0x89,0xc2,0x39,0xf0,0x0f,0x94,0xc2,0x8d,0x6c,0x58,7,0xe3,12,
      0xb8,0x78,0x56,0x34,0x12,0xba,9,0,0,0,0xeb,11,0x90,0xb8,0x21,0x43,0x65,0x87,0xba,11,0,0,0,
      0xbd,13,0,0,0,0x39,0xfe,0xeb,0xce],async({compile,reset,run})=>{
      // Only natural block entries are roots: interior producers must remain
      // in the same block so this also exercises elimination of entire chains.
      const entries=[0,18,30,41];await compile(CODE,256,entries.map(pc=>CODE+pc));
      for(const entry of entries)for(const ecx of [0,1])for(const ebx of [0,0xffffffff,0x7fffffff])for(let budget=0;budget<=80;budget++){
        const regs=[0x1234abcd,ecx,0x55667788,ebx,STACK+0x800,0x89abcdef,0x80000000,7];
        let pc=entry,flags=0xad7;
        const arithmeticFlags=(op,left,right)=>{const r=arithmetic(op,32,left,right,flags&CF);flags=((flags&~ARITH)|r.flags)>>>0;return r.result;};
        for(let n=0;n<budget;n++){
          if(pc===0){regs[0]=regs[3];pc=2;}
          else if(pc===2){regs[0]=arithmeticFlags("add",regs[0],1);pc=5;}
          else if(pc===5){regs[2]=regs[0];pc=7;}
          else if(pc===7){arithmeticFlags("sub",regs[0],regs[6]);pc=9;}
          else if(pc===9){regs[2]=((regs[2]&0xffffff00)|Number(Boolean(flags&ZF)))>>>0;pc=12;}
          else if(pc===12){regs[5]=(regs[0]+2*regs[3]+7)>>>0;pc=16;}
          else if(pc===16)pc=regs[1]===0?30:18;
          else if(pc===18){regs[0]=0x12345678;pc=23;}
          else if(pc===23){regs[2]=9;pc=28;}
          else if(pc===28)pc=41;
          else if(pc===30)pc=31;
          else if(pc===31){regs[0]=0x87654321;pc=36;}
          else if(pc===36){regs[2]=11;pc=41;}
          else if(pc===41){regs[5]=13;pc=46;}
          else if(pc===46){arithmeticFlags("sub",regs[6],regs[7]);pc=48;}
          else if(pc===48)pc=0;
          else throw Error("invalid scalar reference PC");
        }
        reset({eip:CODE+entry,eax:0x1234abcd,ecx,edx:0x55667788,ebx,ebp:0x89abcdef,esi:0x80000000,edi:7,flags:0xad7});
        const actual=run(budget);equal(actual.kind,1,"bounded scalar loop yields");equal(actual.retired,budget,"elided instructions still retire");equal(actual.pc,CODE+pc,"exact scalar prefix PC");
        for(let r=0;r<8;r++)equal(actual.words[r],regs[r],`scalar prefix GPR ${r}`);
        equal(actual.words[9],flags,"all scalar prefix flags");equal(actual.words[10],0xffffffff,"scalar prefix flag validity");
      }
    }));
  await test("scalar demand preserves AL AH AX and sign-extension dependencies at every budget",()=>fixture(
    [0xb0,1,0xb4,2,0x66,0xb8,3,0x80,0x66,0x99,0x66,0xba,0x22,0x11,0x0f,0xb6,0xfc,0x0f,0xbe,0xe8,
      0x99,0xba,0x66,0x77,0x88,0x99,0x89,0xc6,0xb8,0x78,0x56,0x34,0x12,0x0f,0x0b],async({reset,run})=>{
      const pcs=[0,2,4,8,10,14,17,20,21,26,28,33];
      for(const eax of [0,0x12345678,0xffffffff,0x80001234])for(let budget=0;budget<=15;budget++){
        const regs=[eax,0,0x55667788,0,STACK+0x800,0,0,0];
        const steps=[()=>{regs[0]=((regs[0]&0xffffff00)|1)>>>0;},()=>{regs[0]=((regs[0]&0xffff00ff)|0x200)>>>0;},
          ()=>{regs[0]=((regs[0]&0xffff0000)|0x8003)>>>0;},()=>{regs[2]=((regs[2]&0xffff0000)|0xffff)>>>0;},
          ()=>{regs[2]=((regs[2]&0xffff0000)|0x1122)>>>0;},()=>{regs[7]=(regs[0]>>>8)&255;},()=>{regs[5]=((regs[0]<<24)>>24)>>>0;},
          ()=>{regs[2]=regs[0]&0x80000000?0xffffffff:0;},()=>{regs[2]=0x99887766;},()=>{regs[6]=regs[0];},()=>{regs[0]=0x12345678;}];
        const completed=Math.min(budget,steps.length);for(let n=0;n<completed;n++)steps[n]();
        reset({eax,edx:0x55667788,flags:0xad7});const actual=run(budget);
        equal(actual.kind,budget>steps.length?8:1,"narrow sequence budget or UD");equal(actual.retired,completed,"narrow sequence retirement");equal(actual.pc,CODE+pcs[completed],"narrow sequence PC");
        for(let r=0;r<8;r++)equal(actual.words[r],regs[r],`narrow GPR ${r}`);
        equal(actual.words[9],0xad7,"moves and extensions preserve flags");equal(actual.words[10],0xffffffff,"moves preserve flag validity");
      }
    }));
  await test("scalar demand keeps conditional writes and zero-count shift barriers",()=>fixture(
    [0xb8,0x78,0x56,0x34,0x12,0x0f,0x44,0xc3,0x89,0xc2,0xb8,7,0,0,0,0x0f,0x92,0xc0,0xd3,0xe6,0x39,0xef,0x0f,0x0b],
    async({reset,run})=>{
      const pcs=[0,5,8,10,15,18,20,22];
      for(const flags of [0x202,0x203,0x242,0x243])for(let budget=0;budget<=10;budget++){
        let eax=0xfeedbeef,edx=0x55667788,expectedFlags=flags;
        const steps=[()=>{eax=0x12345678;},()=>{if(flags&ZF)eax=0x89abcdef;},()=>{edx=eax;},()=>{eax=7;},
          ()=>{eax=((eax&0xffffff00)|(flags&CF))>>>0;},()=>{},()=>{expectedFlags=(flags&~ARITH)|arithmetic("sub",32,7,0,0).flags;}];
        const completed=Math.min(budget,steps.length);for(let n=0;n<completed;n++)steps[n]();
        reset({eax:0xfeedbeef,edx:0x55667788,ebx:0x89abcdef,ecx:0,esi:0x80000001,edi:7,flags});const actual=run(budget);
        equal(actual.kind,budget>steps.length?8:1,"conditional sequence budget or UD");equal(actual.retired,completed,"conditional sequence retirement");equal(actual.pc,CODE+pcs[completed],"conditional sequence PC");
        equal(actual.words[0],eax,"CMOV false retains the previous destination");equal(actual.words[2],edx,"later copy observes conditional value");equal(actual.words[6],0x80000001,"zero shift preserves its destination");
        equal(actual.words[9],expectedFlags,"zero shift does not kill carry");equal(actual.words[10],0xffffffff,"zero shift leaves flag validity intact");
      }
    }));
  await test("scalar demand retains untouched carry and overflow recipes across erased flag writers",async()=>{
    for(const kind of ["inc","sahf"]){
      const code=kind==="inc"?[0x01,0xd8,0x42,0xba,7,0,0,0,0x83,0xd7,0,0x29,0xed,0x0f,0x0b]
        :[0x01,0xd8,0x9e,0x0f,0x90,0xc2,0xb8,7,0,0,0,0x39,0xf1,0x0f,0x0b];
      const pcs=kind==="inc"?[0,2,3,8,11,13]:[0,2,3,6,11,13];
      await fixture(code,async({reset,run})=>{
        for(const [eax,ebx] of [[0,0],[0xffffffff,1],[0x7fffffff,1],[0x80000000,0xffffffff]])for(let budget=0;budget<=8;budget++){
          const regs=[eax,3,0x7fffffff,ebx,STACK+0x800,0x55667788,5,0xffffffff];let flags=0xad7;
          const arithmeticFlags=(op,left,right,mask=ARITH)=>{const a=arithmetic(op,32,left,right,flags&CF);flags=((flags&~mask)|(a.flags&mask))>>>0;return a.result;};
          const steps=[()=>{regs[0]=arithmeticFlags("add",regs[0],regs[3]);},
            ()=>{if(kind==="inc")regs[2]=arithmeticFlags("add",regs[2],1,ARITH&~CF);else flags=((flags&~0xd5)|((regs[0]>>>8)&0xd5))>>>0;},
            ()=>{regs[2]=kind==="inc"?7:((regs[2]&0xffffff00)|Number(Boolean(flags&OF)))>>>0;},
            ()=>{if(kind==="inc")regs[7]=arithmeticFlags("adc",regs[7],0);else regs[0]=7;},
            ()=>{if(kind==="inc")regs[5]=arithmeticFlags("sub",regs[5],regs[5]);else arithmeticFlags("sub",regs[1],regs[6]);}];
          const completed=Math.min(budget,steps.length);for(let n=0;n<completed;n++)steps[n]();
          reset({eax,ebx,ecx:3,edx:0x7fffffff,ebp:0x55667788,esi:5,edi:0xffffffff,flags:0xad7});const actual=run(budget);
          equal(actual.kind,budget>steps.length?8:1,"partial flag sequence budget or UD");equal(actual.retired,completed,"partial flag sequence retirement");equal(actual.pc,CODE+pcs[completed],"partial flag sequence PC");
          for(let r=0;r<8;r++)equal(actual.words[r],regs[r],`partial flag GPR ${r}`);
          equal(actual.words[9],flags,"erasure preserves untouched flag bits");equal(actual.words[10],0xffffffff,"erasure preserves exact flag validity");
        }
      });
    }
  });
  await test("scalar demand publishes dead-looking values before a fault and resumes once",()=>fixture(
    [0x89,0xd8,0x83,0xc0,1,0xeb,0,0x8b,0x17,0xb8,7,0,0,0,0x39,0xf1,0x0f,0x0b],async({reset,run,map,bytes})=>{
      reset({eax:17,ebx:0x7fffffff,edx:0x55667788,ecx:3,esi:5,edi:0x4000,flags:0x202});const fault=run(100);
      equal(fault.kind,4,"load before overwrite faults");equal(fault.pc,CODE+7,"exact fault PC");equal(fault.retired,3,"dead-looking copy and ADD retire");
      equal(fault.words[0],0x80000000,"fault exposes original ADD result");equal(fault.words[2],0x55667788,"failed load preserves destination");equal(fault.words[9],0x202|arithmetic("add",32,0x7fffffff,1,0).flags,"fault exposes original ADD flags");
      map(0x4000,READ);setWord(bytes(0x4000),0,0x89abcdef);const repaired=run(100);
      equal(repaired.kind,8,"repair reaches UD");equal(repaired.retired,3,"repair does not replay prefix");equal(repaired.words[0],7,"replacement executes after repair");equal(repaired.words[2],0x89abcdef,"repaired load executes once");equal(repaired.words[9],0x202|arithmetic("sub",32,3,5,0).flags,"later CMP replaces flags after repair");
    }));
  await test("scalar demand publishes registers and flags at executable alias invalidation",()=>fixture(
    [0x89,0xd8,0x83,0xc0,1,0x89,0x07,0xb8,7,0,0,0,0x39,0xf1,0x0f,0x0b],async({reset,run,alias,bytes})=>{
      alias(0x4000,CODE,READ|WRITE);reset({eax:17,ebx:0xffffffff,edi:0x4100,ecx:3,esi:5,flags:0x202});const actual=run(100);
      equal(actual.kind,7,"alias write invalidates current unit");equal(actual.pc,CODE+7,"SMC before later register replacement");equal(actual.retired,3,"committed store retires once");equal(actual.words[0],0,"SMC exposes original ADD result");
      equal(actual.words[9],0x202|arithmetic("add",32,0xffffffff,1,0).flags,"SMC exposes original flags");equal(new DataView(bytes(CODE).buffer,bytes(CODE).byteOffset).getUint32(0x100,true),0,"alias receives original result");
    }));
  await test("scalar demand retains values at unknown and later bound native unit edges",()=>fixture([0x0f,0x0b],async({map,compile,select,reset,run})=>{
    const entry=0x3000;map(entry,READ|EXECUTE|SEALED,page([0x89,0xd8,0x83,0xc0,1,0xeb,0,0xb8,7,0,0,0,0x39,0xf1,0x0f,0x0b],0xcc));
    await compile(entry,3);reset({eip:entry,ebx:0xffffffff,ecx:3,esi:5,flags:0x202});const outside=run(100);
    equal(outside.kind,2,"unknown target yields to translation");equal(outside.pc,entry+7,"untranslated target");equal(outside.retired,3,"complete unknown edge prefix");equal(outside.words[0],0,"unknown exit retains preceding value");equal(outside.words[9],0x202|arithmetic("add",32,0xffffffff,1,0).flags,"unknown exit retains preceding flags");
    await compile(entry+7);select(entry);reset({eip:entry,ebx:0xffffffff,ecx:3,esi:5,flags:0x202});const boundary=run(3);
    equal(boundary.kind,1,"bound edge with zero remaining budget yields");equal(boundary.pc,entry+7,"bound edge PC");equal(boundary.words[0],outside.words[0],"native chaining preserves exact budget register");equal(boundary.words[9],outside.words[9],"native chaining preserves exact budget flags");
  }));
  await test("nested control raw entries and every partial budget preserve both loop counters", () => fixture(
    [0xb9,3,0,0,0, 0xba,4,0,0,0, 0x40,0x4a,0x75,0xfc, 0x49,0x75,0xf4, 0x0f,0x0b],
    async ({compile,reset,run}) => {
      const entries=[0,5,10,11,12,14,15,17];
      await compile(CODE,256,entries.map(pc=>CODE+pc));
      for(const entry of entries) for(let budget=0;budget<=50;budget++) {
        const regs=[7,2,2,11]; let pc=entry,flags=0xad7,retired=0,kind=1;
        const change=(index,op)=>{const a=arithmetic(op,32,regs[index],1,0);regs[index]=a.result;flags=((flags&~(ARITH&~CF))|(a.flags&(ARITH&~CF)))>>>0;};
        while(retired<budget) {
          if(pc===17){kind=8;break;}
          if(pc===0){regs[1]=3;pc=5;}
          else if(pc===5){regs[2]=4;pc=10;}
          else if(pc===10){change(0,"add");pc=11;}
          else if(pc===11){change(2,"sub");pc=12;}
          else if(pc===12)pc=flags&ZF?14:10;
          else if(pc===14){change(1,"sub");pc=15;}
          else if(pc===15)pc=flags&ZF?17:5;
          else throw Error("invalid nested-loop reference PC");
          retired++;
        }
        reset({eip:CODE+entry,eax:7,ecx:2,edx:2,ebx:11,flags:0xad7});const actual=run(budget);
        equal(actual.pc,CODE+pc,"exact nested-loop prefix PC");equal(actual.retired,retired,"exact nested-loop retirement");equal(actual.kind,kind,"budget or final UD");
        for(let i=0;i<4;i++)equal(actual.words[i],regs[i],`nested-loop GPR ${i}`);
        equal(actual.words[9],flags,"nested-loop arithmetic flags");equal(actual.words[10],0xffffffff,"defined flags survive both loops");
      }
    }));
  await test("nested control irreducible inner loop accepts both incoming arms and raw ports", () => fixture(
    [0xb9,3,0,0,0, 0xba,4,0,0,0, 0x85,0xf6, 0x74,2, 0xeb,6,
      0x40,0x4a,0x74,6,0xeb,0, 0x43,0x4a,0x75,0xf6, 0x49,0x75,0xe8, 0x0f,0x0b],
    async ({compile,reset,run}) => {
      const entries=[0,5,14,16,20,22,26,29];await compile(CODE,256,entries.map(pc=>CODE+pc));
      for(const mode of [0,1]) for(const entry of entries) for(const budget of [0,1,2,3,4,7,12,19,31,60,100]) {
        const regs=[7,2,4,11];let pc=entry,flags=0xad7,defined=0xffffffff,retired=0,kind=1;
        const change=(index,op)=>{const a=arithmetic(op,32,regs[index],1,0);regs[index]=a.result;flags=((flags&~(ARITH&~CF))|(a.flags&(ARITH&~CF)))>>>0;defined=(defined|(ARITH&~CF))>>>0;};
        while(retired<budget) {
          if(pc===29){kind=8;break;}
          if(pc===0){regs[1]=3;pc=5;}
          else if(pc===5){regs[2]=4;pc=10;}
          else if(pc===10){flags=((flags&~(ARITH&~AF))|common(BigInt(mode),32))>>>0;defined=(defined&~AF)>>>0;pc=12;}
          else if(pc===12)pc=flags&ZF?16:14;
          else if(pc===14)pc=22;
          else if(pc===16){change(0,"add");pc=17;}
          else if(pc===17){change(2,"sub");pc=18;}
          else if(pc===18)pc=flags&ZF?26:20;
          else if(pc===20)pc=22;
          else if(pc===22){change(3,"add");pc=23;}
          else if(pc===23){change(2,"sub");pc=24;}
          else if(pc===24)pc=flags&ZF?26:16;
          else if(pc===26){change(1,"sub");pc=27;}
          else if(pc===27)pc=flags&ZF?29:5;
          else throw Error("invalid irreducible-loop reference PC");
          retired++;
        }
        reset({eip:CODE+entry,eax:7,ecx:2,edx:4,ebx:11,esi:mode,flags:0xad7});const actual=run(budget);
        equal(actual.pc,CODE+pc,"exact multi-entry nested PC");equal(actual.retired,retired,"multi-entry nested retirement");equal(actual.kind,kind,"multi-entry budget or UD");
        for(let i=0;i<4;i++)equal(actual.words[i],regs[i],`irreducible nested GPR ${i}`);
        equal(((actual.words[9]^flags)&defined)>>>0,0,"defined flags across both incoming arms");equal(actual.words[10],defined,"nested flag definition mask");
      }
    }));
  await test("nested control page fault in the second outer iteration resumes its exact inner prefix", () => fixture(
    [0xb9,2,0,0,0, 0xba,2,0,0,0, 0x8b,0x06,0x83,0xc6,4,0x4a,0x75,0xf8,0x49,0x75,0xf0,0x0f,0x0b],
    async ({bytes,map,reset,run}) => {
      for(let n=0;n<3;n++)setWord(bytes(DATA),4084+n*4,(n+1)*11);
      reset({esi:DATA+4084,eax:99});const fault=run(100);
      equal(fault.kind,4,"nested fourth load faults");equal(fault.pc,CODE+10,"fault names the inner load");equal(fault.retired,17,"only completed nested iterations retire");
      equal(fault.words[0],33,"third load retained");equal(fault.words[1],1,"outer counter retained");equal(fault.words[2],1,"inner counter retained");equal(fault.address,DATA+4096,"exact missing-page address");
      map(DATA+4096,READ|WRITE);setWord(bytes(DATA+4096),0,44);const resumed=run(100);
      equal(resumed.kind,8,"repaired nested loops finish");equal(resumed.retired,6,"repair executes only the remaining nested suffix");
      equal(resumed.words[0],44,"fourth load observed");equal(resumed.words[1],0,"outer loop completes once");equal(resumed.words[2],0,"inner loop completes once");equal(resumed.words[6],DATA+4100,"pointer never replays completed loads");
    }));
  await test("nested control executable alias store stops before the next inner-loop effect", () => fixture(
    [0xb9,2,0,0,0, 0xba,2,0,0,0, 0x40,0x89,0x07,0x83,0xc7,4,0x4a,0x75,0xf7,0x49,0x75,0xef,0x0f,0x0b],
    async ({alias,compile,bytes,reset,run}) => {
      alias(DATA+4096,CODE,READ|WRITE);reset({edi:DATA+4092,eax:0,flags:0xad7});const smc=run(100);
      equal(smc.kind,7,"inner-loop alias store invalidates code");equal(smc.retired,9,"only completed nested effects retire");equal(smc.pc,CODE+13,"SMC continues after the committed store");
      equal(smc.words[0],2,"both INC results retained");equal(smc.words[1],2,"outer decrement has not executed");equal(smc.words[2],1,"second inner decrement has not executed");equal(smc.words[7],DATA+4096,"post-store address update has not executed");
      equal(new DataView(bytes(DATA).buffer,bytes(DATA).byteOffset).getUint32(4092,true),1,"first non-code store committed");
      equal(new DataView(bytes(CODE).buffer,bytes(CODE).byteOffset).getUint32(0,true),2,"code-alias store committed once");
      await compile(CODE+13);const resumed=run(1);
      equal(resumed.retired,1,"new code admits only the next instruction");equal(resumed.pc,CODE+16,"recompiled continuation advances once");equal(resumed.words[7],DATA+4100,"rebuild does not repeat the store or its prefix");
    }));
  if (!(options.valueIr || options.flowIr || options.rootedSsa || options.functionCfg || options.structuredLoops || options.affineMemory || options.loopMemory || options.predicateIr)) {
    await test("nested control depth bound retains all deep-loop entries", async () => {
      const code=[];for(let n=0;n<64;n++)code.push(0x74,n===0?2:0xfa,0xeb,0);code.push(0x0f,0x0b);
      await fixture(code,async ({compile,reset,run}) => {
        await compile(CODE,512,Array.from({length:64},(_,n)=>CODE+n*4));
        for(const entry of [0,1,15,16,31,63]) for(const zero of [false,true]) for(const budget of [0,1,2,17,64,128,200]) {
          let pc=entry*4,retired=0,kind=1;const flags=zero?0xad7:0xad7&~ZF;
          while(retired<budget) {
            if(pc===256){kind=8;break;}
            if(pc%4===0)pc=zero?(pc===0?4:pc-4):pc+2;
            else pc+=2;
            retired++;
          }
          reset({eip:CODE+entry*4,eax:73,flags});const result=run(budget);
          equal(result.pc,CODE+pc,"deep raw entry follows exact branch graph");equal(result.retired,retired,"depth fallback keeps exact budget");equal(result.kind,kind,"deep cycle or final UD");equal(result.words[0],73,"deep routing preserves GPR");equal(result.words[9],flags,"deep routing preserves flags");
        }
      });
    });
    await test("control regions maximum admitted branch graph preserves every entry", () => fixture([0x0f,0x0b],
      async ({ map,compile,reset,run }) => {
        const jumps=page(); for(let at=0;at<4096;at+=2) { jumps[at]=0xeb; jumps[at+1]=0; }
        for(let n=0;n<4;n++) map(0x10000+n*4096,READ|EXECUTE|SEALED,jumps);
        await compile(0x10000,8192);
        for(const [entry,budget] of [[0,0],[0,1],[0,127],[0,128],[0,4095],[0,4096],[0,8191],[0,8192],[0,8193],[8191,1]]) {
          reset({eip:0x10000+entry*2,eax:73,flags:0xad7}); const result=run(budget),retired=Math.min(budget,8192-entry);
          equal(result.retired,retired,"large branch graph retires exact prefix"); equal(result.pc,0x10000+(entry+retired)*2,"large branch graph exact PC");
          equal(result.kind,budget<=8192-entry?1:2,"budget or undecoded successor"); equal(result.words[0],73,"control-only graph preserves GPR"); equal(result.words[9],0xad7,"control-only graph preserves flags");
        }
      }));
  }
  await test("control regions exact indirect targets reject colliding and interior PCs", async () => {
    const code = new Array(16 + 32 * 8).fill(0xcc); code[0] = 0xff; code[1] = 0xe0;
    const roots = [CODE];
    for (let n=0;n<32;n++) { const at=16+n*8; roots.push(CODE+at); code.splice(at,7,0xb9,...wordBytes(n+1),0x0f,0x0b); }
    await fixture(code,async ({compile,reset,run}) => {
      await compile(CODE,256,roots);
      for (let n=0;n<32;n++) {
        const target=roots[n+1]; reset({eax:target,ecx:0xabcdef01,flags:0xad7}); const result=run(3);
        equal(result.kind,8,"known indirect target reaches its own UD"); equal(result.retired,2,"JMP and selected MOV only");
        equal(result.pc,target+5,"exact selected instruction stream"); equal(result.words[1],n+1,"entry identity selects the correct payload");
        equal(result.words[9],0xad7,"entry lookup preserves flags");
      }
      for (const target of [CODE+1,CODE+3,...roots.slice(1).map(pc=>pc+1),0x80001010,0xfffffff0,0xffffffff]) {
        reset({eax:target,ecx:0xabcdef01,flags:0xad7}); const result=run(3);
        equal(result.kind,2,"unknown or interior PC retains ordinary external lookup"); equal(result.retired,1,"no colliding block executes");
        equal(result.pc,target,"full 32-bit destination survives miss"); equal(result.words[1],0xabcdef01,"miss publishes no candidate payload");
        equal(result.words[9],0xad7,"lookup miss preserves flags");
      }
    });
  });
  for (const entry of [0,4,8,10,14,16]) for (const mode of [0,1]) {
    await test(`control regions multi-entry cycle PC${entry} arm${mode}`, () => fixture(
      [0x85,0xd2, 0x74,6, 0x40, 0x49, 0x74,8, 0xeb,0, 0x43, 0x49, 0x75,0xf6, 0xeb,0, 0x0f,0x0b],
      async ({ compile, reset, run }) => {
        await compile(CODE, 256, [0,4,8,10,14,16].map(pc => CODE + pc));
        for (const budget of [0,1,2,3,4,7,12,19,20,21,40]) {
          const regs = [7,5,mode,11]; let pc = entry, flags = 0xad7, defined = 0xffffffff, retired = 0, kind = 1;
          const change = (register, subtract) => {
            const reference = arithmetic(subtract ? "sub" : "add", 32, regs[register], 1, 0);
            regs[register] = reference.result; flags = (flags & ~(ARITH & ~CF)) | (reference.flags & ~CF);
            defined = (defined | (ARITH & ~CF)) >>> 0;
          };
          while (retired < budget) {
            if (pc === 16) { kind = 8; break; }
            switch (pc) {
              case 0: flags = (flags & ~(ARITH & ~AF)) | common(BigInt(mode),32); defined = (defined & ~AF) >>> 0; pc = 2; break;
              case 2: pc = flags & ZF ? 10 : 4; break;
              case 4: change(0,false); pc = 5; break;
              case 5: change(1,true); pc = 6; break;
              case 6: pc = flags & ZF ? 16 : 8; break;
              case 8: pc = 10; break;
              case 10: change(3,false); pc = 11; break;
              case 11: change(1,true); pc = 12; break;
              case 12: pc = flags & ZF ? 14 : 4; break;
              case 14: pc = 16; break;
              default: throw Error(`unmodelled cycle PC ${pc}`);
            }
            retired++;
          }
          reset({ eax:7, ecx:5, edx:mode, ebx:11, flags:0xad7, eip:CODE+entry });
          const result = run(budget);
          equal(result.kind,kind,"exact budget or UD exit"); equal(result.retired,retired,"ordered cycle prefix");
          equal(result.pc,CODE+pc,"every arbitrary entry retains precise continuation");
          for (let n=0;n<4;n++) equal(result.words[n],regs[n],`live GPR ${n} through cycle scopes`);
          equal(result.words[9],flags,"flags through both loop entries"); equal(result.words[10],defined,"exact defined-flag mask");
        }
      }));
  }
  await test("control regions backward acyclic edges preserve entry order and values", () => fixture(
    [0xeb,8, 0x43, 0xeb,11, 0xcc,0xcc,0xcc,0xcc,0xcc, 0x40, 0xeb,0xf5, 0xcc,0xcc,0xcc, 0x0f,0x0b],
    async ({ compile, reset, run }) => {
      await compile(CODE,256,[CODE,CODE+2,CODE+10,CODE+16]);
      for (const [entry,retired,eax,ebx] of [[0,5,8,12],[2,2,7,12],[10,4,8,12],[16,0,7,11]]) {
        reset({ eax:7, ebx:11, eip:CODE+entry }); const result=run(100);
        equal(result.kind,8,"reordered DAG reaches UD"); equal(result.pc,CODE+16,"exact DAG exit");
        equal(result.retired,retired,"no skipped or repeated block"); equal(result.words[0],eax,"EAX update order"); equal(result.words[3],ebx,"EBX update order");
      }
    }));
  await test("control regions separate consecutive self loops and retain raw loop entry", () => fixture(
    [0xb9,3,0,0,0, 0x40,0x49,0x75,0xfc, 0xb9,2,0,0,0, 0x43,0x49,0x75,0xfc, 0x0f,0x0b],
    async ({ compile, reset, run }) => {
      await compile(CODE,256,[CODE,CODE+5,CODE+14]); reset({eax:7,ebx:11});
      const result=run(100); equal(result.kind,8,"both loops finish"); equal(result.retired,17,"loop setup and iterations counted once");
      equal(result.words[0],10,"first loop state survives second scope"); equal(result.words[3],13,"second scope executes twice"); equal(result.words[1],0,"second counter terminates");
      reset({eax:7,ebx:11,ecx:3,eip:CODE+14}); const direct=run(100);
      equal(direct.retired,9,"raw second-loop entry bypasses first scope"); equal(direct.words[0],7,"first loop remains untouched"); equal(direct.words[3],14,"raw loop uses supplied count");
    }));
  await test("control regions fault after completed iteration resumes without replay", () => fixture(
    [0x8b,0x06, 0x83,0xc6,4, 0x49, 0x75,0xf8, 0x0f,0x0b], async ({ bytes,map,reset,run }) => {
      setWord(bytes(DATA),4092,27); reset({esi:DATA+4092,ecx:2,eax:99});
      const fault=run(100); equal(fault.kind,4,"next loop iteration faults"); equal(fault.retired,4,"first iteration retired once");
      equal(fault.pc,CODE,"fault names loop header"); equal(fault.words[0],27,"completed load survives"); equal(fault.words[1],1,"completed decrement survives");
      equal(fault.address,DATA+4096,"next page fault address"); map(DATA+4096,READ|WRITE); setWord(bytes(DATA+4096),0,42);
      const resumed=run(100); equal(resumed.kind,8,"repaired loop finishes"); equal(resumed.retired,4,"repair does not replay completed iteration");
      equal(resumed.words[0],42,"repaired load observed"); equal(resumed.words[1],0,"remaining counter only"); equal(resumed.words[6],DATA+4100,"pointer advances once after repair");
    }));
  return { cases, checks, failures, functionCfg: options.functionCfg ?? false, timestampCounter: hasTimestamp, fpIr: options.fpIr ?? false, typedNumeric: options.typedNumeric ?? false, valueIr: options.valueIr ?? false, rootedSsa: options.rootedSsa ?? false,
    expandedCache: options.expandedCache ?? false, cacheCapacity: requestedCacheCapacity, cacheByteLimit: hasCacheCapacity ? requestedCacheByteLimit : null,
    invocationAdmission: hasLinkInvocation, passed: failures.length === 0 };
}

if (typeof process !== "undefined" && process.argv[1]?.endsWith("test-recompiler-semantics.mjs")) {
  const { readFile, mkdtemp, rm } = await import("node:fs/promises");
  const { existsSync } = await import("node:fs");
  const { spawn } = await import("node:child_process");
  const { tmpdir } = await import("node:os");
  const path = await import("node:path");
  const wasmPath = process.env.VINE_RECOMPILER_WASM ?? "target/wasm32-unknown-unknown/release/vine_recompiler.wasm";
  const wasmBytes = await readFile(wasmPath);
  const options = { functionCfg: process.argv.includes("--function-cfg"), loopMemory: process.argv.includes("--loop-memory"), structuredLoops: process.argv.includes("--structured-loops"), affineMemory: process.argv.includes("--affine-memory"),
    linkedRegions: process.argv.includes("--linked-regions"), directLinks: process.argv.includes("--direct-links"), timestampCounter: process.argv.includes("--timestamp-counter"),
    fpIr: process.argv.includes("--fp-ir"), typedNumeric: process.argv.includes("--typed-numeric"), expandedCache: process.argv.includes("--expanded-cache"), valueIr: process.argv.includes("--value-ir"), rootedSsa: process.argv.includes("--rooted-ssa"), predicateIr: process.argv.includes("--predicate-ir"),
    flowIr: process.argv.includes("--flow-ir"), controlGraph: process.argv.includes("--control-graph"), bindingGlobal: process.argv.includes("--binding-global"), filter: process.argv.includes("--filter") ? process.argv[process.argv.indexOf("--filter") + 1] : undefined };
  let result;
  if (!process.argv.includes("--chrome")) {
    result = await runRecompilerSemantics(wasmBytes, options);
  } else {
    const executable = [process.env.VINE_CHROME,
      process.env.ProgramFiles && path.join(process.env.ProgramFiles, "Google/Chrome/Application/chrome.exe"),
      process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, "Google/Chrome/Application/chrome.exe")]
      .find((candidate) => candidate && existsSync(candidate));
    if (!executable) throw Error("Chrome was not found; set VINE_CHROME");
    const profile = await mkdtemp(path.join(tmpdir(), "vine-recompiler-semantics-"));
    let chrome, socket;
    try {
      const endpoint = await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => reject(Error("Chrome DevTools startup timed out")), 15000);
        let stderr = "";
        chrome = spawn(executable, ["--headless=new", "--no-first-run", "--no-default-browser-check",
          "--disable-background-networking", "--remote-debugging-port=0", `--user-data-dir=${profile}`, "about:blank"],
        { stdio: ["ignore", "ignore", "pipe"], windowsHide: true });
        chrome.stderr.on("data", (chunk) => {
          stderr += String(chunk);
          const address = /DevTools listening on (ws:\/\/\S+)/u.exec(stderr)?.[1];
          if (address) { clearTimeout(timeout); resolve(address); }
        });
        chrome.once("error", (failure) => { clearTimeout(timeout); reject(failure); });
        chrome.once("exit", (code) => { clearTimeout(timeout); reject(Error(`Chrome exited ${code}: ${stderr}`)); });
      });
      const created = await fetch(`http://${new URL(endpoint).host}/json/new?about:blank`, { method: "PUT" });
      if (!created.ok) throw Error(`Chrome refused test target: ${created.status}`);
      socket = new WebSocket((await created.json()).webSocketDebuggerUrl);
      await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
      let nextId = 0;
      const requests = new Map();
      socket.onmessage = ({ data }) => {
        const message = JSON.parse(data);
        const request = requests.get(message.id);
        if (!request) return;
        requests.delete(message.id);
        clearTimeout(request.timeout);
        if (message.error) request.reject(Error(JSON.stringify(message.error)));
        else request.resolve(message.result);
      };
      const send = (method, params) => new Promise((resolve, reject) => {
        const id = ++nextId;
        const timeout = setTimeout(() => { requests.delete(id); reject(Error(`${method} timed out`)); }, 60000);
        requests.set(id, { resolve, reject, timeout });
        socket.send(JSON.stringify({ id, method, params }));
      });
      const base64 = wasmBytes.toString("base64");
      const evaluated = await send("Runtime.evaluate", {
        expression: `(${runRecompilerSemantics.toString()})(Uint8Array.from(atob(${JSON.stringify(base64)}), c => c.charCodeAt(0)), ${JSON.stringify(options)})`,
        awaitPromise: true, returnByValue: true,
      });
      if (evaluated.exceptionDetails) throw Error(JSON.stringify(evaluated.exceptionDetails));
      result = evaluated.result.value;
    } finally {
      socket?.close();
      if (chrome && chrome.exitCode === null) {
        const stopped = new Promise((resolve) => chrome.once("exit", resolve));
        chrome.kill();
        await stopped;
      }
      const resolvedProfile = path.resolve(profile);
      if (path.dirname(resolvedProfile) !== path.resolve(tmpdir())
        || !path.basename(resolvedProfile).startsWith("vine-recompiler-semantics-")) {
        throw Error("Refusing to remove a temporary profile outside the test directory");
      }
      await rm(resolvedProfile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
    }
  }
  console.log(JSON.stringify({ environment: process.argv.includes("--chrome") ? "Chrome" : "Node", ...options, ...result }, null, 2));
  if (!result.passed) process.exitCode = 1;
}
