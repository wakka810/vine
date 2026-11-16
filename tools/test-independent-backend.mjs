// Generic Chrome compiler/execution test. Private code, captures, and reports stay external.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

/** This function also runs unmodified in an owned Chrome Worker. */
export async function runIndependentReplay(manifest, compilerBytes, beforePool, expectedPool, settings = {}) {
  const check = (value, message) => { if (!value) throw Error(message); };
  check(manifest.schema === "vine-independent-replay-v1", "Unknown replay schema");
  check(Array.isArray(manifest.pages) && manifest.pages.length > 0 && manifest.pages.length <= 131072, "Page bound");
  check(beforePool.byteLength === expectedPool.byteLength, "Pool length mismatch");
  check(Number.isSafeInteger(manifest.budget) && manifest.budget > 0 && manifest.budget <= 10000000, "Budget bound");
  check(manifest.beforeState.length === 420 && manifest.expectedState.length === 420, "Canonical state size");
  const seen = new Set(), backingOffsets = new Set();
  for (const p of manifest.pages) {
    check(Number.isInteger(p.address) && p.address >= 0 && p.address <= 0xfffff000 && p.address % 4096 === 0 && !seen.has(p.address), "Invalid virtual page");
    check(Number.isInteger(p.offset) && p.offset >= 0 && p.offset + 4096 <= beforePool.byteLength && !backingOffsets.has(p.offset), "Input page range/alias (alias fixtures require shared backing API)");
    check(Number.isInteger(p.flags) && p.flags >= 0 && p.flags <= 7 && Number.isInteger(p.owner) && p.owner > 0, "Mapping metadata");
    check(!(p.flags & 4) || p.ownerSealed === true, "Executable mapping requires explicit sealed owner fixture admission");
    seen.add(p.address);
    backingOffsets.add(p.offset);
  }
  const started = performance.now();
  const { instance } = await WebAssembly.instantiate(compilerBytes, {});
  const wasm = instance.exports;
  const typedKernelCount = typeof wasm.backend_fp_kernel_25 === "function" ? 26
    : typeof wasm.backend_fp_kernel_23 === "function" ? 24
    : typeof wasm.backend_fp_kernel_22 === "function" ? 23
    : typeof wasm.backend_fp_kernel_21 === "function" ? 22
    : typeof wasm.backend_fp_kernel_20 === "function" ? 21 : 20;
  const linkedRegions = settings.linkedRegions ?? false;
  const hasLinks = ["backend_link_slot", "backend_link_bind", "backend_link_resolve"]
    .every((name) => typeof wasm[name] === "function");
  const hasLinkInvocation = ["backend_link_begin", "backend_link_end"]
    .every((name) => typeof wasm[name] === "function");
  check(!linkedRegions || hasLinks, "Compiler does not support linked regions");
  const hasCacheCapacity = ["backend_set_cache_capacity", "backend_cache_capacity", "backend_cache_byte_limit"]
    .every((name) => typeof wasm[name] === "function");
  check(!settings.expandedCache || hasCacheCapacity, "Compiler does not support expanded cache configuration");
  const handle = wasm.backend_new(manifest.pages.length + 16) >>> 0;
  check(handle !== 0, "Backend allocation failed");
  const errorText = () => new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, wasm.backend_error_ptr(), wasm.backend_error_len()));
  const checkResult = (code, operation) => check(code === 0, `${operation}: ${errorText()}`);
  if (settings.directLinks) {
    check(linkedRegions && hasLinkInvocation && typeof wasm.backend_set_direct_links === "function", "Direct links require scoped linked regions");
    checkResult(wasm.backend_set_direct_links(handle, 1), "Enable coherent direct links");
  }
  if (settings.controlGraph) {
    check(typeof wasm.backend_set_control_graph === "function", "Compiler lacks control graph lowering");
    checkResult(wasm.backend_set_control_graph(handle, 1), "Enable binary-derived control graph");
  }
  check(!settings.valueIr || typeof wasm.backend_set_value_ir === "function", "Compiler does not support scalar value IR");
  if (typeof wasm.backend_set_value_ir === "function") {
    checkResult(wasm.backend_set_value_ir(handle, Number(settings.valueIr ?? false)), "Set scalar value IR configuration");
  }
  if (settings.predicateIr) {
    check(typeof wasm.backend_set_predicate_ir === "function", "Compiler does not support predicate IR");
    checkResult(wasm.backend_set_predicate_ir(handle, 1), "Set predicate IR configuration");
  }
  if (settings.flowIr) {
    check(typeof wasm.backend_set_flow_ir === "function", "Compiler does not support flow IR");
    checkResult(wasm.backend_set_flow_ir(handle, 1), "Set flow IR configuration");
  }
  check(!settings.rootedSsa || typeof wasm.backend_set_rooted_ssa === "function", "Compiler does not support rooted SSA");
  if (typeof wasm.backend_set_rooted_ssa === "function") {
    checkResult(wasm.backend_set_rooted_ssa(handle, Number(settings.rootedSsa ?? false)), "Set rooted SSA configuration");
  }
  if (hasCacheCapacity) checkResult(wasm.backend_set_cache_capacity(handle, Number(settings.expandedCache ?? false)), "Set cache capacity before compilation");
  const cacheCapacity = hasCacheCapacity ? wasm.backend_cache_capacity(handle) >>> 0 : 64;
  const cacheByteLimit = hasCacheCapacity ? wasm.backend_cache_byte_limit(handle) >>> 0 : null;
  check(cacheCapacity === (settings.expandedCache ? 512 : 64), "Configured cache capacity differs from requested mode");
  if (hasCacheCapacity) check(cacheByteLimit === 16 * 1024 * 1024, "Cache unit comparison must preserve the 16MiB byte limit");
  const links = new WebAssembly.Table({ element: "anyfunc", initial: cacheCapacity, maximum: cacheCapacity });
  if (settings.structuredLoops !== undefined) {
    check(typeof wasm.backend_set_structured_loops === "function", "Compiler does not support structured-loop configuration");
    checkResult(wasm.backend_set_structured_loops(handle, Number(settings.structuredLoops)), "Set structured-loop configuration");
  }
  if (settings.affineMemory !== undefined) {
    check(typeof wasm.backend_set_affine_memory === "function", "Compiler does not support affine-memory configuration");
    checkResult(wasm.backend_set_affine_memory(handle, Number(settings.affineMemory)), "Set affine-memory configuration");
  }
  if (settings.functionCfg !== undefined) {
    check(typeof wasm.backend_set_function_cfg === "function", "Compiler does not support function CFG");
    checkResult(wasm.backend_set_function_cfg(handle, Number(settings.functionCfg)), "Set function CFG");
  }
  if (settings.loopMemory !== undefined) {
    check(typeof wasm.backend_set_loop_memory === "function", "Compiler does not support loop memory proofs");
    checkResult(wasm.backend_set_loop_memory(handle, Number(settings.loopMemory)), "Set loop memory proofs");
  }
  check(!settings.fpIr || typeof wasm.backend_set_fp_ir === "function", "Compiler does not support explicit FP IR");
  if (typeof wasm.backend_set_fp_ir === "function") {
    checkResult(wasm.backend_set_fp_ir(handle, Number(settings.fpIr ?? false)), "Set explicit FP IR configuration");
  }
  check(!settings.typedNumeric || settings.fpIr, "Typed numeric kernels require explicit FP IR");
  check(!settings.typedNumeric || typeof wasm.backend_set_typed_numeric === "function", "Compiler does not support typed numeric kernels");
  if (typeof wasm.backend_set_typed_numeric === "function") {
    checkResult(wasm.backend_set_typed_numeric(handle, Number(settings.typedNumeric ?? false)), "Set typed numeric kernels");
  }
  if (settings.numericMode !== undefined) checkResult(wasm.backend_set_numeric_mode(handle, settings.numericMode), "Set numerical mode");
  const scratch = wasm.backend_alloc(4096) >>> 0;
  check(scratch !== 0, "Scratch allocation failed");
  for (const p of manifest.pages) {
    new Uint8Array(wasm.memory.buffer, scratch, 4096).set(beforePool.subarray(p.offset, p.offset + 4096));
    checkResult(wasm.backend_map_page(handle, p.address, p.owner, p.flags | (p.ownerSealed ? 16 : 0), scratch), "Map virtual page");
  }
  const statePointer = wasm.backend_state(handle) >>> 0;
  const tablePointer = wasm.backend_table(handle) >>> 0;
  const initialState = Uint8Array.from(manifest.beforeState);
  const expectedState = Uint8Array.from(manifest.expectedState);
  const expectedWords = new Uint32Array(expectedState.buffer);
  new Uint8Array(wasm.memory.buffer, statePointer, 420).set(initialState);
  const setupMilliseconds = performance.now() - started;
  let currentModule = null;
  let currentId = 0;
  const cachedModules = new Map();
  const hasRegionCache = typeof wasm.backend_cache_select === "function";
  function drainRetired() {
    if (!hasRegionCache) return;
    for (let count = 0; count <= cacheCapacity; count++) {
      const id = wasm.backend_cache_retired(handle) >>> 0;
      if (id === 0) return;
      if (id === 0xffffffff) {
        for (let slot = 0; slot < links.length; slot++) links.set(slot, null);
        cachedModules.clear(); currentModule = null; currentId = 0;
      }
      else {
        const retired = cachedModules.get(id);
        if (retired?.slot !== null && retired?.slot !== undefined) links.set(retired.slot, null);
        cachedModules.delete(id);
        if (id === currentId) { currentModule = null; currentId = 0; }
      }
    }
    throw Error("Region retirement notification bound exceeded");
  }
  const modules = [];
  const changedPages = manifest.pages.filter((p) => {
    for (let i = 0; i < 4096; i++) if (beforePool[p.offset + i] !== expectedPool[p.offset + i]) return true;
    return false;
  });
  function restore() {
    if (hasLinkInvocation) checkResult(wasm.backend_link_end(handle), "End links before context restore");
    if (typeof wasm.backend_cancel_timestamp_counter === "function") {
      checkResult(wasm.backend_cancel_timestamp_counter(handle), "Cancel pending timestamp before restore");
    }
    if (typeof wasm.backend_snapshot_size === "function") {
      checkResult(wasm.backend_initialize_state(handle, 0, 0, 0), "Replace complete backend context before replay restore");
    }
    for (const p of changedPages) {
      new Uint8Array(wasm.memory.buffer, scratch, 4096).set(beforePool.subarray(p.offset, p.offset + 4096));
      checkResult(wasm.backend_map_page(handle, p.address, p.owner, p.flags | (p.ownerSealed ? 16 : 0), scratch), "Restore virtual page");
      if (p.flags & 4) currentModule = null;
    }
    new Uint8Array(wasm.memory.buffer, statePointer, 420).set(initialState);
  }
  function verify() {
    const bytes = new Uint8Array(wasm.memory.buffer, statePointer, 420);
    const words = new Uint32Array(wasm.memory.buffer, statePointer, 105);
    for (let i = 0; i < 9; i++) check(words[i] === expectedWords[i], `Architectural word ${i}: ${words[i].toString(16)} != ${expectedWords[i].toString(16)}`);
    const mask = expectedWords[10] >>> 0;
    check(((words[9] ^ expectedWords[9]) & mask) === 0, "Defined EFLAGS differ");
    check((words[10] & mask) === (mask | 0), "Expected defined EFLAGS were lost");
    for (let i = 44; i < 400; i++) check(bytes[i] === expectedState[i], `Segment/floating state byte ${i} differs`);
    if (manifest.expectedX87SavedTag !== undefined) {
      let savedTag = 0;
      for (let i = 0; i < 8; i++) {
        const low = words[35 + i * 3], high = words[36 + i * 3], exponent = words[37 + i * 3] & 0x7fff;
        const liveTag = (words[61] >>> (i * 2)) & 3;
        const tag = liveTag === 3 ? 3 : exponent === 0 && low === 0 && high === 0 ? 1 :
          exponent === 0 || exponent === 0x7fff || (high & 0x80000000) === 0 ? 2 : 0;
        savedTag |= tag << (i * 2);
      }
      check(savedTag === manifest.expectedX87SavedTag, `Derived x87 saved tag ${savedTag.toString(16)} differs`);
    }
    let compared = 0;
    for (const p of manifest.pages) {
      const pointer = wasm.backend_page_ptr(handle, p.address) >>> 0;
      check(pointer !== 0, "Comparison unmapped page");
      const actual = new Uint8Array(wasm.memory.buffer, pointer, 4096);
      for (let i = 0; i < 4096; i++) if (actual[i] !== expectedPool[p.offset + i]) throw Error(`Guest byte ${(p.address + i).toString(16)} differs`);
      compared += 4096;
    }
    if (manifest.expectedStop) {
      const stop = manifest.expectedStop;
      check(words[100] === stop.kind && words[8] === stop.eip, "Expected boundary stop differs");
      for (const [name, word] of [["faultEip", 101], ["address", 102], ["error", 103]]) {
        if (stop[name] !== undefined) check(words[word] === stop[name], `Expected stop ${name} differs`);
      }
    }
    return { virtualBytesCompared: compared, definedEflagsMask: mask, completeArchitecturalState: true };
  }
  async function execute(preparation = null) {
    let remaining = manifest.budget, switches = 0, compileMilliseconds = 0, generatedMilliseconds = 0, finishMilliseconds = 0;
    let linkMemoHits = 0, linkMemoMisses = 0;
    const start = performance.now(), exits = {};
    while (remaining > 0) {
      check(switches++ < 100000, "Backend exit bound exceeded");
      let words = new Uint32Array(wasm.memory.buffer, statePointer, 105);
      const eip = preparation?.[0] ?? words[8];
      if (hasRegionCache) {
        checkResult(wasm.backend_cache_select(handle, eip), "Select cached region");
        drainRetired();
        currentId = wasm.backend_cache_selected(handle) >>> 0;
        currentModule = cachedModules.get(currentId)?.exports ?? null;
      } else if (currentModule && typeof wasm.backend_validate === "function" &&
                 wasm.backend_validate(handle, eip) !== 0) currentModule = null;
      if (!currentModule || !wasm.backend_contains(handle, eip)) {
        check(modules.length < 512, "Compilation resource bound exceeded (repeated region eviction or unresolved indirect targets)");
        const compileStart = performance.now();
        if (!hasRegionCache || currentId === 0) {
          if (preparation) {
            const pointer = wasm.backend_input_alloc(handle, preparation.length * 4);
            check(pointer !== 0, "AOT root transfer allocation");
            try {
              new Uint32Array(wasm.memory.buffer, pointer, preparation.length).set(preparation);
              checkResult(wasm.backend_compile_entries(handle, pointer, preparation.length, settings.regionInstructions ?? 512), "AOT compile roots");
            } finally { checkResult(wasm.backend_free(pointer, preparation.length * 4), "Free AOT root transfer"); }
          } else checkResult(wasm.backend_compile(handle, eip, settings.regionInstructions ?? 512), `Compile ${eip.toString(16)}`);
          drainRetired();
          if (hasRegionCache) currentId = wasm.backend_cache_selected(handle) >>> 0;
        }
        const code = Uint8Array.from(new Uint8Array(wasm.memory.buffer, wasm.backend_code_ptr(handle), wasm.backend_code_len(handle)));
        if (settings.diagnostics) console.error(JSON.stringify({ phase: "instantiate", entry: eip, bytes: code.byteLength, switches, remaining }));
        const generated = await WebAssembly.instantiate(code, { env: { memory: wasm.memory,
          x87: wasm.backend_x87, backend: handle, links, link_resolve: wasm.backend_link_resolve,
          ...(settings.directLinks ? {link_directory: wasm.backend_link_directory(handle)} : {}),
          begin_timestamp_counter: wasm.backend_begin_timestamp_counter, fp_numeric: wasm.backend_fp_numeric,
          fp_kernel_error: wasm.backend_fp_kernel_error,
          sse_cvtt_f32: wasm.backend_sse_cvtt_f32, sse_cvtt_f64: wasm.backend_sse_cvtt_f64,
          sse_i32_to_f32: wasm.backend_sse_i32_to_f32,
          sse_mul_f32: wasm.backend_sse_mul_f32,
          sse_add_f32: wasm.backend_sse_add_f32,
          sse_sub_f32: wasm.backend_sse_sub_f32,
          sse_cvt_f64: wasm.backend_sse_cvt_f64,
          sse_add_f64: wasm.backend_sse_add_f64,
          sse_sub_f64: wasm.backend_sse_sub_f64,
          sse_mul_f64: wasm.backend_sse_mul_f64,
          scas_begin: wasm.backend_scas_begin, scas_complete: wasm.backend_scas_complete, scas_finish: wasm.backend_scas_finish,
          ...Object.fromEntries(Array.from({ length: typedKernelCount }, (_, i) => [`fp_kernel_${i}`, wasm[`backend_fp_kernel_${i}`]])),
          ...Object.fromEntries(["balanced", "fast"].flatMap(mode => Array.from({length: 26}, (_, i) => [`fp_${mode}_${i}`, wasm[`backend_fp_${mode}_${i}`]]))) } });
        if (settings.fpIr) check(!WebAssembly.Module.imports(generated.module).some(({ module, name }) => module === "env" && name === "x87"),
          "Explicit FP IR imported the descriptor/state-clone execution path");
        if (settings.typedNumeric) check(!WebAssembly.Module.imports(generated.module).some(({ module, name }) => module === "env" && name === "fp_numeric"),
          "Typed numeric kernels imported the handle/opcode dispatch path");
        const valueSections = WebAssembly.Module.customSections(generated.module, "vine.value-ir.v1");
        check(valueSections.length === Number(settings.valueIr ?? false), "Compiled scalar value IR mode differs from requested mode");
        const valueIrFacts = valueSections.length ? JSON.parse(new TextDecoder().decode(valueSections[0])) : null;
        const predicateSections = WebAssembly.Module.customSections(generated.module, "vine.predicate-ir.v1");
        check(predicateSections.length === Number(settings.predicateIr ?? false), "Compiled predicate IR mode differs from requested mode");
        const predicateIrFacts = predicateSections.length ? JSON.parse(new TextDecoder().decode(predicateSections[0])) : null;
        const flowSections = WebAssembly.Module.customSections(generated.module, "vine.flow-ir.v3");
        check(flowSections.length === Number(settings.flowIr ?? false), "Compiled flow IR mode differs from requested mode");
        const flowIrFacts = flowSections.length ? JSON.parse(new TextDecoder().decode(flowSections[0])) : null;
        const rootedSections = WebAssembly.Module.customSections(generated.module, "vine.rooted-ssa.v1");
        check(rootedSections.length === Number(settings.rootedSsa ?? false), "Compiled rooted SSA mode differs from requested mode");
        const rootedSsaFacts = rootedSections.length ? JSON.parse(new TextDecoder().decode(rootedSections[0])) : null;
        const graphSections = WebAssembly.Module.customSections(generated.module, "vine.control-graph.v1");
        const controlGraphFacts = graphSections.length ? JSON.parse(new TextDecoder().decode(graphSections[0])) : null;
        currentModule = generated.instance.exports;
        let slot = null;
        if (hasRegionCache) {
          check(currentId > 0 && currentId < 0xffffffff, "Invalid selected region ID");
          drainRetired();
          if (hasLinks) {
            const slotPlusOne = wasm.backend_link_slot(handle, currentId) >>> 0;
            check(slotPlusOne >= 1 && slotPlusOne <= links.length, "Invalid region link slot");
            slot = slotPlusOne - 1;
            check(links.get(slot) === null, "Region link slot was not retired before reuse");
            check(typeof currentModule.link === "function", "Missing region link entry");
            links.set(slot, currentModule.link);
            const status = wasm.backend_link_bind(handle, currentId, slotPlusOne);
            if (status !== 0) links.set(slot, null);
            checkResult(status, "Publish region link");
          }
          cachedModules.set(currentId, { exports: currentModule, slot });
        }
        const factSections = WebAssembly.Module.customSections(generated.module, "vine.affine.v1");
        check(factSections.length <= 1, "Duplicate affine facts section");
        let affineFacts = null;
        if (factSections.length) {
          const view = new DataView(factSections[0]);
          check(view.byteLength >= 1 && view.byteLength === 1 + view.getUint8(0) * 21, "Affine facts metadata size");
          affineFacts = Array.from({ length: view.getUint8(0) }, (_,i) => { const o=1+i*21; return {base:view.getUint8(o),min:view.getInt32(o+1,true),max:view.getInt32(o+5,true),permissions:view.getUint32(o+9,true),staticReferences:view.getUint32(o+13,true),distinctOffsets:view.getUint32(o+17,true)}; });
        }
        const fpSections = WebAssembly.Module.customSections(generated.module, "vine.fp-ir.v1");
        check(fpSections.length <= 1, "Duplicate explicit FP IR metadata");
        let fpIrFacts = null;
        if (fpSections.length) {
          const view = new DataView(fpSections[0]);
          check(view.byteLength === 8, "Explicit FP IR metadata size");
          fpIrFacts = { instructions: view.getUint32(0, true), descriptorFallbacks: view.getUint32(4, true) };
          if (settings.fpIr) check(fpIrFacts.descriptorFallbacks === 0, "Explicit FP IR contained descriptor fallbacks");
        }
        const fpFunctionSections = WebAssembly.Module.customSections(generated.module, "vine.fp-ir.functions.v1");
        check(fpFunctionSections.length <= 1, "Duplicate explicit FP function metadata");
        if (fpFunctionSections.length) {
          const view = new DataView(fpFunctionSections[0]);
          check(view.byteLength === 4 && fpIrFacts !== null, "Explicit FP function metadata shape");
          fpIrFacts.sharedFunctions = view.getUint32(0, true);
        }
        const kernelSections = WebAssembly.Module.customSections(generated.module, "vine.fp-kernels.v1");
        check(kernelSections.length <= 1, "Duplicate typed numeric metadata");
        if (kernelSections.length) {
          const view = new DataView(kernelSections[0]);
          check(view.byteLength === 4 && fpIrFacts !== null, "Typed numeric metadata shape");
          fpIrFacts.typedKernels = view.getUint32(0, true);
          check(fpIrFacts.typedKernels === typedKernelCount, "Unexpected typed numeric kernel protocol");
        }
        const elapsed = performance.now() - compileStart;
        compileMilliseconds += elapsed;
        modules.push({ entry: eip, cacheId: hasRegionCache ? currentId : null, linkSlot: slot,
          bytes: code.byteLength, compileMilliseconds: elapsed, affineFacts, fpIrFacts, valueIrFacts, predicateIrFacts, flowIrFacts, rootedSsaFacts, controlGraphFacts,
          imports: WebAssembly.Module.imports(generated.module).map(({ module, name, kind }) => ({ module, name, kind })),
          blocks: wasm.backend_block_count?.(handle) ?? null,
          instructions: wasm.backend_instruction_count?.(handle) ?? null });
      }
      if (preparation) return { compileMilliseconds };
      if (linkedRegions && hasLinkInvocation) checkResult(wasm.backend_link_begin(handle, currentId, eip), "Begin linked code admission");
      else if (hasRegionCache) checkResult(wasm.backend_cache_validate(handle, currentId, eip), "Cached code admission");
      else if (typeof wasm.backend_validate === "function") checkResult(wasm.backend_validate(handle, eip), "Code admission");
      const run = linkedRegions ? currentModule.run_linked : currentModule.run;
      check(typeof run === "function", "Missing selected region execution entry");
      const runStart = performance.now();
      let retired;
      try {
        retired = run(statePointer, tablePointer, remaining) >>> 0;
        generatedMilliseconds += performance.now() - runStart;
      } finally {
        if (linkedRegions && hasLinkInvocation) checkResult(wasm.backend_link_end(handle), "End linked code admission");
      }
      if (linkedRegions && hasLinkInvocation) {
        linkMemoHits += wasm.backend_link_memo_hits?.(handle) ?? 0;
        linkMemoMisses += wasm.backend_link_memo_misses?.(handle) ?? 0;
      }
      check(retired <= remaining, "Backend exceeded budget");
      remaining -= retired;
      const finishStart = performance.now();
      checkResult(wasm.backend_finish(handle), "Collect mirror dirty metadata");
      finishMilliseconds += performance.now() - finishStart;
      words = new Uint32Array(wasm.memory.buffer, statePointer, 105);
      const kind = words[100];
      exits[kind] = (exits[kind] ?? 0) + 1;
      if (manifest.expectedStop && kind === manifest.expectedStop.kind) break;
      const partial = kind === 12 || kind === 13;
      check(kind === 1 || kind === 2 || kind === 7 || partial, `Backend stopped kind=${kind} eip=${words[8].toString(16)} faultEip=${words[101].toString(16)} address=${words[102].toString(16)} remaining=${remaining}`);
      if (partial) check(words[103] > 0 && words[103] <= 4096, "Invalid REP partial progress");
      check(retired > 0 || words[8] !== eip || partial, "Backend made no progress");
      if (kind === 7 || kind === 13) currentModule = null;
    }
    const completed = remaining === 0;
    return { executionMilliseconds: performance.now() - start, generatedMilliseconds, compileMilliseconds, finishMilliseconds,
      switches, exits, linkMemoHits, linkMemoMisses, requested: manifest.budget, retired: manifest.budget - remaining,
      fallbackMilliseconds: completed ? 0 : null, fallbackSwitches: completed ? 0 : null, completedWithoutFallback: completed };
  }
  let aotPreparationMilliseconds = 0;
  if (settings.aotRoots) {
    check(Array.isArray(settings.aotRoots) && settings.aotRoots.length > 0 && settings.aotRoots.length <= 256 &&
      settings.aotRoots.every(pc => Number.isInteger(pc) && pc >= 0 && pc <= 0xffffffff), "Bounded AOT roots");
    const cpuBefore = new Uint8Array(wasm.memory.buffer, statePointer, 420).slice(), began = performance.now();
    await execute(settings.aotRoots);
    aotPreparationMilliseconds = performance.now() - began;
    const cpuAfter = new Uint8Array(wasm.memory.buffer, statePointer, 420);
    check(cpuBefore.every((b,n)=>b===cpuAfter[n]), "AOT compilation and instantiation changed guest state");
  }
  const cold = await execute();
  cold.verification = verify();
  const warm = [];
  for (let i = 0; i < (settings.repeats ?? 6); i++) {
    restore();
    const row = await execute();
    row.verification = verify();
    warm.push(row);
  }
  const warmCacheStable = warm.every((r) => r.compileMilliseconds === 0);
  // Versioned IR can exceed the fixed byte cache. Recompilation remains in
  // every reported warm execution/compile time; it is never hidden by growing
  // the cache or bypassing the CPU and virtual-memory verification above.
  if (!settings.rootedSsa && !settings.flowIr) check(warmCacheStable, "Warm replay unexpectedly compiled code");
  wasm.backend_free(scratch, 4096);
  return { schema: "vine-independent-result-v1", pass: true, regionCache: hasRegionCache, linkedRegions, invocationAdmission: hasLinkInvocation,
    directLinks: settings.directLinks ? {bytes: wasm.backend_link_directory_bytes(handle),
      hits: wasm.backend_link_directory_count(handle, 0), misses: wasm.backend_link_directory_count(handle, 1),
      capacityMisses: wasm.backend_link_directory_count(handle, 2)} : null,
    expandedCache: settings.expandedCache ?? false, valueIr: settings.valueIr ?? false, rootedSsa: settings.rootedSsa ?? false, cacheCapacity, cacheByteLimit, warmCacheStable, setupMilliseconds, aotPreparationMilliseconds, cold, warm, modules,
    boundary: "Generated Wasm plus region exits, code admission, and backend mirror dirty accounting. Restore and semantic validation are excluded. Runtime GuestMemory writeback, OPFS, graphics and display timing are not measured.",
    inputProvenance: manifest.provenance ?? null };
}

async function main() {
  const options = {};
  for (let i = 2; i < process.argv.length; i++) {
    const key = process.argv[i];
    if (key === "--node") { options.node = true; continue; }
    if (key === "--diagnostics") { options.diagnostics = true; continue; }
    assert(["--input", "--compiler", "--output", "--repeats", "--region-instructions", "--structured-loops", "--affine-memory", "--function-cfg", "--loop-memory", "--linked-regions", "--fp-ir", "--typed-numeric", "--expanded-cache", "--value-ir", "--rooted-ssa", "--direct-links", "--control-graph", "--predicate-ir"].includes(key), `Unknown argument ${key}`);
    assert(process.argv[i + 1] && !process.argv[i + 1].startsWith("--"), `Missing value ${key}`);
    options[key.slice(2)] = process.argv[++i];
  }
  assert(options.input && options.compiler && options.output, "Use --input MANIFEST --compiler COMPILER.wasm --output REPORT.json [--node]");
  const inputPath = path.resolve(options.input), compilerPath = path.resolve(options.compiler), outputPath = path.resolve(options.output);
  assert(!existsSync(outputPath), "Refusing to overwrite a previous report");
  const manifest = JSON.parse(await readFile(inputPath, "utf8"));
  const readInput = (name) => readFile(path.resolve(path.dirname(inputPath), name));
  const [compiler, before, after] = await Promise.all([readFile(compilerPath), readInput(manifest.beforePool), readInput(manifest.expectedPool)]);
  for (const [bytes, expected] of [[before, manifest.beforeSha256], [after, manifest.expectedSha256]]) assert.equal(sha256(bytes), expected, "Replay input hash");
  const settings = { repeats: Number(options.repeats ?? 6), regionInstructions: Number(options["region-instructions"] ?? 512), diagnostics: options.diagnostics ?? false };
  assert([undefined, "on", "off"].includes(options["linked-regions"]), "--linked-regions requires on|off");
  settings.linkedRegions = options["linked-regions"] === "on";
  assert([undefined, "on", "off"].includes(options["direct-links"]), "--direct-links requires on|off");
  settings.directLinks = options["direct-links"] === "on";
  assert([undefined, "on", "off"].includes(options["control-graph"]), "--control-graph requires on|off");
  settings.controlGraph = options["control-graph"] === "on";
  assert([undefined, "on", "off"].includes(options["fp-ir"]), "--fp-ir requires on|off");
  settings.fpIr = options["fp-ir"] === "on";
  assert([undefined, "on", "off"].includes(options["typed-numeric"]), "--typed-numeric requires on|off");
  settings.typedNumeric = options["typed-numeric"] === "on";
  assert([undefined, "on", "off"].includes(options["expanded-cache"]), "--expanded-cache requires on|off");
  settings.expandedCache = options["expanded-cache"] === "on";
  assert([undefined, "on", "off"].includes(options["value-ir"]), "--value-ir requires on|off");
  settings.valueIr = options["value-ir"] === "on";
  assert([undefined, "on", "off"].includes(options["predicate-ir"]), "--predicate-ir requires on|off");
  settings.predicateIr = options["predicate-ir"] === "on";
  settings.flowIr = options["flow-ir"] === "on";
  assert([undefined, "on", "off"].includes(options["rooted-ssa"]), "--rooted-ssa requires on|off");
  settings.rootedSsa = options["rooted-ssa"] === "on";
  if (options["function-cfg"] !== undefined) {
    assert(["on", "off"].includes(options["function-cfg"]), "--function-cfg requires on|off");
    settings.functionCfg = options["function-cfg"] === "on";
  }
  if (options["loop-memory"] !== undefined) {
    assert(["on", "off"].includes(options["loop-memory"]), "--loop-memory requires on|off");
    settings.loopMemory = options["loop-memory"] === "on";
  }
  if (options["structured-loops"] !== undefined) {
    assert(["on", "off"].includes(options["structured-loops"]), "--structured-loops requires on|off");
    settings.structuredLoops = options["structured-loops"] === "on";
  }
  if (options["affine-memory"] !== undefined) {
    assert(["on", "off"].includes(options["affine-memory"]), "--affine-memory requires on|off");
    settings.affineMemory = options["affine-memory"] === "on";
  }
  assert(Number.isInteger(settings.repeats) && settings.repeats >= 1 && settings.repeats <= 100);
  assert(Number.isInteger(settings.regionInstructions) && settings.regionInstructions >= 1 && settings.regionInstructions <= 8192);
  await mkdir(path.dirname(outputPath), { recursive: true });
  const receipt = { compilerPath, compilerSha256: sha256(compiler), inputPath, inputSha256: sha256(await readFile(inputPath)),
    runnerSha256: sha256(await readFile(fileURLToPath(import.meta.url))), settings, environment: options.node ? "Node" : "Chrome Worker", startedUtc: new Date().toISOString() };
  let server, chrome, socket;
  try {
    if (options.node) receipt.result = await runIndependentReplay(manifest, compiler, before, after, settings);
    else {
      const worker = `${runIndependentReplay.toString()}\nself.onmessage=async()=>{try{const get=async p=>{const r=await fetch(p);if(!r.ok)throw Error('fetch '+p);return r;};const manifest=await(await get('/input.json')).json();const bytes=await Promise.all(['/compiler.wasm','/before.bin','/after.bin'].map(async p=>new Uint8Array(await(await get(p)).arrayBuffer())));self.postMessage({result:await runIndependentReplay(manifest,...bytes,${JSON.stringify(settings)})});}catch(e){self.postMessage({error:String(e.stack??e)});}};`;
      const routes = new Map([["/", ["text/html", Buffer.from("<!doctype html><title>Independent PE32 replay</title>")]],
        ["/worker.mjs", ["text/javascript", Buffer.from(worker)]], ["/input.json", ["application/json", Buffer.from(JSON.stringify(manifest))]],
        ["/compiler.wasm", ["application/wasm", compiler]], ["/before.bin", ["application/octet-stream", before]], ["/after.bin", ["application/octet-stream", after]]]);
      server = createServer((request, response) => {
        const route = routes.get(request.url);
        if (request.method !== "GET" || !route) { response.writeHead(404).end(); return; }
        response.writeHead(200, { "Content-Type": route[0], "Content-Length": route[1].byteLength, "Cache-Control": "no-store",
          "Cross-Origin-Embedder-Policy": "require-corp", "Cross-Origin-Opener-Policy": "same-origin" });
        response.end(route[1]);
      });
      await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
      const executable = [process.env.VINE_CHROME,
        process.env.ProgramFiles && path.join(process.env.ProgramFiles, "Google/Chrome/Application/chrome.exe"),
        process.env.LOCALAPPDATA && path.join(process.env.LOCALAPPDATA, "Google/Chrome/Application/chrome.exe")].find((p) => p && existsSync(p));
      assert(executable, "Chrome not found; set VINE_CHROME");
      const profile = `${outputPath}.chrome-profile`;
      assert(!existsSync(profile), "Refusing to reuse browser profile");
      const endpoint = await new Promise((resolve, reject) => {
        let stderr = "";
        const timer = setTimeout(() => reject(Error("Chrome startup timeout")), 20000);
        chrome = spawn(executable, ["--headless=new", "--no-first-run", "--no-default-browser-check", "--disable-background-networking",
          "--disable-background-timer-throttling", "--disable-renderer-backgrounding", "--disable-extensions", "--remote-debugging-port=0", `--user-data-dir=${profile}`, "about:blank"],
        { windowsHide: true, stdio: ["ignore", "ignore", "pipe"] });
        chrome.stderr.on("data", (b) => { stderr = (stderr + b).slice(-16000); const match = /DevTools listening on (ws:\/\/\S+)/u.exec(stderr); if (match) { clearTimeout(timer); resolve(match[1]); } });
        chrome.once("error", (e) => { clearTimeout(timer); reject(e); });
        chrome.once("exit", (code) => { clearTimeout(timer); reject(Error(`Chrome exited ${code}: ${stderr}`)); });
      });
      const targetResponse = await fetch(`http://${new URL(endpoint).host}/json/new?about:blank`, { method: "PUT" });
      assert(targetResponse.ok, "Chrome refused test tab");
      const target = await targetResponse.json();
      socket = new WebSocket(target.webSocketDebuggerUrl);
      await new Promise((resolve, reject) => { socket.addEventListener("open", resolve, { once: true }); socket.addEventListener("error", reject, { once: true }); });
      let id = 0;
      const pending = new Map();
      let pageLoaded;
      socket.addEventListener("message", ({ data }) => {
        const r = JSON.parse(data), callback = pending.get(r.id);
        if (r.method === "Page.loadEventFired") pageLoaded?.();
        if (callback) { pending.delete(r.id); callback(r); }
      });
      const send = (method, params = {}) => new Promise((resolve, reject) => {
        const key = ++id, timer = setTimeout(() => { pending.delete(key); reject(Error(`${method} timeout`)); }, 900000);
        pending.set(key, (r) => { clearTimeout(timer); r.error ? reject(Error(r.error.message)) : resolve(r.result); });
        socket.send(JSON.stringify({ id: key, method, params }));
      });
      receipt.chromeVersion = await send("Browser.getVersion");
      await send("Page.enable");
      const loaded = new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(Error("Replay page load timeout")), 20000);
        pageLoaded = () => { clearTimeout(timer); resolve(); };
      });
      await send("Page.navigate", { url: `http://127.0.0.1:${server.address().port}/` });
      await loaded;
      const evaluated = await send("Runtime.evaluate", { expression: "new Promise((resolve,reject)=>{const w=new Worker('/worker.mjs',{type:'module'});w.onmessage=({data})=>{w.terminate();resolve(data)};w.onerror=e=>{w.terminate();reject(Error(e.message))};w.postMessage({});})", awaitPromise: true, returnByValue: true });
      assert(!evaluated.exceptionDetails, JSON.stringify(evaluated.exceptionDetails));
      assert(!evaluated.result.value.error, evaluated.result.value.error);
      receipt.result = evaluated.result.value.result;
    }
  } catch (error) { receipt.error = String(error.stack ?? error); process.exitCode = 1; }
  finally {
    socket?.close();
    if (chrome && chrome.exitCode === null) chrome.kill();
    if (server) await new Promise((resolve) => server.close(resolve));
    receipt.endedUtc = new Date().toISOString();
    await writeFile(outputPath, `${JSON.stringify(receipt, null, 2)}\n`, { flag: "wx" });
  }
  console.log(JSON.stringify({ outputPath, pass: receipt.result?.pass ?? false, cold: receipt.result?.cold, warm: receipt.result?.warm, error: receipt.error }));
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
