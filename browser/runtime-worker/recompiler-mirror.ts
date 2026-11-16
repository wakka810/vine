import type { CpuResume, ExecutionBootstrap, ExecutionCpuContext, ExecutionEngine,
  ExecutionEngineOptions, ExecutionSlice, ExecutionStop, ExecutionThreadBootstrap } from "./execution-engine-contract.js";
import type { CacheEffects, DirtyMirrorPage, HostMemoryDelta, MappingOperation,
  MemoryReflection, MemoryRegionKind, MemoryRegionSnapshot } from "./memory-transport.js";

type ScalarBackendFunction =
  | "backend_apply_runtime_update" | "backend_begin_timestamp_counter"
  | "backend_cache_invalidate" | "backend_cache_retired" | "backend_cache_select"
  | "backend_cache_selected" | "backend_cache_validate" | "backend_cancel_timestamp_counter"
  | "backend_set_cache_capacity" | "backend_cache_capacity" | "backend_cache_byte_limit"
  | "backend_cache_maximum_capacity"
  | "backend_set_binding_global" | "backend_set_loop_memory" | "backend_set_function_cfg"
  | "backend_set_direct_links" | "backend_link_directory"
  | "backend_set_control_graph" | "backend_set_numeric_mode"
  | "backend_code_len" | "backend_code_ptr" | "backend_compile" | "backend_complete_timestamp_counter"
  | "backend_prepare_begin" | "backend_prepare_next" | "backend_prepare_status" | "backend_prepare_end"
  | "backend_create_context" | "backend_drop" | "backend_error_len" | "backend_error_ptr"
  | "backend_snapshot_size" | "backend_save_snapshot" | "backend_restore_snapshot" | "backend_create_snapshot"
  | "backend_scas_begin" | "backend_scas_complete" | "backend_scas_finish"
  | "backend_finish" | "backend_fp_kernel_error" | "backend_fp_numeric" | "backend_free"
  | "backend_initialize_state" | "backend_input_alloc" | "backend_link_begin" | "backend_link_bind"
  | "backend_link_end" | "backend_link_resolve" | "backend_link_slot" | "backend_new"
  | "backend_read_guest" | "backend_resume" | "backend_set_fp_ir" | "backend_set_typed_numeric" | "backend_set_value_ir" | "backend_set_rooted_ssa" | "backend_set_predicate_ir" | "backend_set_flow_ir"
  | "backend_state" | "backend_table" | "backend_writeback_ack" | "backend_writeback_control_len"
  | "backend_writeback_control_ptr" | "backend_writeback_payload_len" | "backend_writeback_payload_ptr"
  | "backend_writeback_prepare" | "backend_x87" | `backend_fp_kernel_${number}`
  | `backend_fp_balanced_${number}` | `backend_fp_fast_${number}`;
type BackendExports = WebAssembly.Exports & Record<ScalarBackendFunction, (...args: number[]) => number> & {
  memory: WebAssembly.Memory;
  // Packed i64 helpers pass directly between Wasm instances. If called from
  // JavaScript their return type is BigInt, unlike transport scalar exports.
  backend_sse_cvtt_f32: (mxcsr: number, low: number, high: number) => bigint;
  backend_sse_cvtt_f64: (mxcsr: number, low: number, high: number) => bigint;
  backend_sse_i32_to_f32: (mxcsr: number, low: number, high: number) => bigint;
  backend_sse_mul_f32: (mxcsr: number, aBits: number, bBits: number) => bigint;
  backend_sse_add_f32: (mxcsr: number, aBits: number, bBits: number) => bigint;
  backend_sse_sub_f32: (mxcsr: number, aBits: number, bBits: number) => bigint;
  backend_sse_cvt_f64: (mxcsr: number, low: number, high: number) => bigint;
  backend_sse_add_f64: (mxcsr: number, a: bigint, b: bigint) => number;
  backend_sse_sub_f64: (mxcsr: number, a: bigint, b: bigint) => number;
  backend_sse_mul_f64: (mxcsr: number, a: bigint, b: bigint) => number;
};
type GeneratedExports = {
  prepare?: () => number;
  run: (state: number, table: number, budget: number) => number;
  run_linked?: (state: number, table: number, budget: number) => number;
  link?: (...args: number[]) => number;
};
type GeneratedResource = { exports: GeneratedExports; slot: number | null; prepared: boolean };
function linkTable(slotCount: number, maximum = slotCount): WebAssembly.Table {
  return new WebAssembly.Table({ element: "anyfunc", initial: slotCount, maximum });
}
const pageSize = 4096, stateBytes = 420, snapshotBytes = 512;
const kinds: MemoryRegionKind[] = ["image", "stack", "thread-environment-block",
  "process-environment-block", "process-parameters", "command-line", "heap", "anonymous",
  "runtime", "surface", "graphics-resource", "audio-resource", "gdi-resource"];
function invariant(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message);
}
function u32(value: number, name: string): number {
  invariant(Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff, `invalid ${name}`);
  return value;
}
function encodeUpdate(delta: HostMemoryDelta): { control: Uint8Array; payload: Uint8Array } {
  invariant(delta.fromGeneration >= 0n && delta.fromGeneration <= 0xffff_ffff_ffff_ffffn &&
    delta.generation >= 0n && delta.generation <= 0xffff_ffff_ffff_ffffn,
    "invalid Runtime memory generation");
  const control = new Uint8Array(52 + delta.mappingOperations.length * 40 + delta.pages.length * 16);
  const payload = new Uint8Array(delta.pages.length * pageSize);
  control.set(new TextEncoder().encode("VINEDELT"));
  const v = new DataView(control.buffer);
  v.setUint32(8, pageSize, true); v.setBigUint64(12, delta.fromGeneration, true);
  v.setBigUint64(20, delta.generation, true);
  v.setUint32(28, delta.mappingOperations.length, true); v.setUint32(32, 40, true);
  v.setUint32(36, delta.pages.length, true); v.setUint32(40, 16, true);
  v.setUint32(44, Number(delta.cacheEffects.flushTlb) | Number(delta.cacheEffects.invalidateInstructionFetch) << 1 |
    Number(delta.cacheEffects.invalidateGeneratedCode) << 2, true);
  let offset = 52;
  for (const op of delta.mappingOperations) {
    const operation = ["reserve", "commit", "protect", "decommit", "release"].indexOf(op.operation) + 1;
    invariant(operation > 0, "invalid Runtime mapping operation");
    v.setUint32(offset, operation, true); v.setUint32(offset + 4, u32(op.owner, "owner"), true);
    if (op.operation === "reserve") {
      const kind = kinds.indexOf(op.kind) + 1;
      invariant(kind > 0, "invalid Runtime region kind"); v.setUint32(offset + 8, kind, true);
    }
    if (op.operation === "commit" || op.operation === "protect") {
      v.setUint32(offset + 12, Number(op.protection.read) | Number(op.protection.write) << 1 |
        Number(op.protection.execute) << 2, true);
    }
    v.setUint32(offset + 16, u32(op.range.base, "mapping address"), true);
    invariant(Number.isSafeInteger(op.range.byteLength) && op.range.byteLength > 0 &&
      op.range.byteLength <= 0x1_0000_0000, "invalid Runtime mapping length");
    v.setBigUint64(offset + 20, BigInt(op.range.byteLength), true);
    const allocation = op.addressAllocationId ?? 0;
    invariant(Number.isSafeInteger(allocation) && allocation >= 0, "invalid Runtime allocation identity");
    v.setBigUint64(offset + 28, BigInt(allocation), true); offset += 40;
  }
  for (const [index, page] of delta.pages.entries()) {
    invariant(page.bytes.byteLength === pageSize, "Runtime page length is not 4096 bytes");
    v.setUint32(offset, u32(page.address, "page address"), true);
    v.setUint32(offset + 4, u32(page.owner, "page owner"), true);
    v.setUint32(offset + 8, Number(page.protection.read) | Number(page.protection.write) << 1 |
      Number(page.protection.execute) << 2 | Number(page.ownerSealed) << 3, true);
    payload.set(page.bytes, index * pageSize); offset += 16;
  }
  return { control, payload };
}

// This class owns Chrome Wasm resources and transport buffers only. Rust validates
// mappings, CPU admission, code stamps and writeback; GuestMemory remains authoritative.
export class RecompilerMirror implements ExecutionEngine {
  #wasm: BackendExports;
  #handle: number;
  #maximumPages: number;
  #modules = new Map<number, GeneratedResource>();
  #links: WebAssembly.Table;
  #linkedRegions: boolean;
  #fpIr: boolean;
  #typedNumeric: boolean;
  #numericMode: "strict" | "balanced" | "fast";
  #valueIr: boolean;
  #predicateIr: boolean;
  #flowIr: boolean;
  #rootedSsa: boolean;
  #expandedCache: boolean;
  #bindingGlobal: boolean;
  #directLinks: boolean;
  #controlGraph: boolean;
  #loopMemory: boolean;
  #functionCfg: boolean;
  #supportsLinks: boolean;
  #supportsLinkAdmission: boolean;
  #initialized = false;
  #preparing = false;
  #snapshotPointer = 0;
  constructor(wasm: BackendExports, options: ExecutionEngineOptions) {
    invariant(options !== undefined && Number.isInteger(options.maximumPhysicalPages) &&
      options.maximumPhysicalPages > 0 && options.maximumPhysicalPages <= 0x10_0000,
    "recompiler requires a positive Runtime physical page limit within the x86 page count");
    this.#wasm = wasm; this.#maximumPages = options.maximumPhysicalPages;
    invariant(typeof wasm.backend_snapshot_size === "function" && wasm.backend_snapshot_size() === snapshotBytes &&
      [wasm.backend_save_snapshot, wasm.backend_restore_snapshot, wasm.backend_create_snapshot].every(fn => typeof fn === "function"),
    "compiler does not support the complete CPU snapshot ABI");
    this.#supportsLinks = typeof wasm.backend_link_slot === "function" &&
      typeof wasm.backend_link_bind === "function" && typeof wasm.backend_link_resolve === "function";
    this.#supportsLinkAdmission = typeof wasm.backend_link_begin === "function";
    invariant(this.#supportsLinkAdmission === (typeof wasm.backend_link_end === "function"),
      "compiler has an incomplete link-admission contract");
    this.#linkedRegions = options.recompilerLinkedRegions ?? this.#supportsLinks;
    invariant(typeof this.#linkedRegions === "boolean", "invalid linked-region option");
    invariant(!this.#linkedRegions || this.#supportsLinks, "compiler does not support linked regions");
    this.#directLinks = options.recompilerDirectLinks ?? false;
    this.#controlGraph = options.recompilerControlGraph ?? false;
    invariant(typeof this.#controlGraph === "boolean", "invalid control graph option");
    invariant(!this.#controlGraph || typeof wasm.backend_set_control_graph === "function",
      "compiler does not support binary-derived control graphs");
    invariant(!this.#controlGraph || !(options.recompilerValueIr || options.recompilerRootedSsa ||
      options.recompilerFunctionCfg || options.recompilerLoopMemory), "control graphs require ordinary instruction lowering");
    invariant(typeof this.#directLinks === "boolean", "invalid direct link option");
    invariant(!this.#directLinks || this.#linkedRegions && this.#supportsLinkAdmission &&
      typeof wasm.backend_set_direct_links === "function" && typeof wasm.backend_link_directory === "function",
      "direct links require compiler support and scoped linked regions");
    invariant(!this.#directLinks || !(options.recompilerValueIr || options.recompilerRootedSsa ||
      options.recompilerFunctionCfg || options.recompilerLoopMemory), "direct links require ordinary instruction lowering");
    this.#fpIr = options.recompilerFpIr ?? true;
    invariant(typeof this.#fpIr === "boolean", "invalid floating-point IR option");
    invariant(!this.#fpIr || typeof wasm.backend_set_fp_ir === "function" &&
      typeof wasm.backend_fp_numeric === "function", "compiler does not support floating-point IR");
    this.#numericMode = options.recompilerNumericMode ?? "balanced";
    invariant(["strict", "balanced", "fast"].includes(this.#numericMode), "invalid numerical mode");
    invariant(this.#numericMode === "strict" || this.#fpIr && typeof wasm.backend_set_numeric_mode === "function",
      "approximation requires compiler support and floating-point IR");
    this.#typedNumeric = options.recompilerTypedNumeric ?? this.#fpIr;
    invariant(typeof this.#typedNumeric === "boolean", "invalid typed numeric option");
    invariant(!this.#typedNumeric || this.#fpIr && typeof wasm.backend_set_typed_numeric === "function" &&
      typeof wasm.backend_fp_kernel_error === "function" &&
      Array.from({length: 26}, (_, index) => wasm[`backend_fp_kernel_${index}`]).every(fn => typeof fn === "function"),
    "compiler does not support typed numeric kernels with floating-point IR");
    invariant(this.#numericMode === "strict" || !this.#typedNumeric ||
      [0, 1, 2, 3, 4, 20, 21, 22, 23, 24].map(i => wasm[`backend_fp_${this.#numericMode === "balanced" ? "balanced" : "fast"}_${i}`]).every(fn => typeof fn === "function"),
      "compiler lacks requested numerical kernels");
    this.#expandedCache = options.recompilerExpandedCache ?? false;
    this.#bindingGlobal = options.recompilerBindingGlobal ?? true;
    invariant(typeof this.#bindingGlobal === "boolean", "invalid immutable binding option");
    invariant(!this.#bindingGlobal || typeof wasm.backend_set_binding_global === "function",
      "compiler does not support immutable unit imports");
    this.#rootedSsa = options.recompilerRootedSsa ?? false;
    invariant(typeof this.#rootedSsa === "boolean", "invalid rooted SSA option");
    invariant(!this.#rootedSsa || typeof wasm.backend_set_rooted_ssa === "function",
      "compiler does not support rooted SSA");
    invariant(!(this.#rootedSsa && options.recompilerValueIr), "rooted SSA and scalar value IR are exclusive");
    this.#valueIr = options.recompilerValueIr ?? false;
    this.#predicateIr = options.recompilerPredicateIr ?? false;
    this.#flowIr = options.recompilerFlowIr ?? false;
    invariant(typeof this.#flowIr === "boolean", "invalid flow IR option");
    invariant(!this.#flowIr || typeof wasm.backend_set_flow_ir === "function", "compiler lacks flow IR");
    invariant(!this.#flowIr || !(options.recompilerPredicateIr || options.recompilerValueIr ||
      options.recompilerRootedSsa || options.recompilerFunctionCfg || options.recompilerLoopMemory ||
      options.recompilerDirectLinks || options.recompilerControlGraph), "flow IR requires its own value/control lowering");
    invariant(typeof this.#predicateIr === "boolean", "invalid predicate IR option");
    invariant(!this.#predicateIr || typeof wasm.backend_set_predicate_ir === "function", "compiler lacks predicate IR");
    invariant(!this.#predicateIr || !(options.recompilerValueIr || options.recompilerRootedSsa ||
      options.recompilerFunctionCfg || options.recompilerLoopMemory || options.recompilerDirectLinks ||
      options.recompilerControlGraph), "predicate IR requires its own value/control lowering");
    invariant(typeof this.#valueIr === "boolean", "invalid scalar value IR option");
    invariant(!this.#valueIr || typeof wasm.backend_set_value_ir === "function",
      "compiler does not support scalar value IR");
    this.#functionCfg = options.recompilerFunctionCfg ?? false;
    invariant(typeof this.#functionCfg === "boolean", "invalid function CFG option");
    invariant(!this.#functionCfg || typeof wasm.backend_set_function_cfg === "function",
      "compiler does not support function CFG");
    invariant(!(this.#functionCfg && (this.#rootedSsa || this.#valueIr || options.recompilerLoopMemory)),
      "function CFG requires ordinary instruction lowering");
    this.#loopMemory = options.recompilerLoopMemory ?? false;
    invariant(typeof this.#loopMemory === "boolean", "invalid loop memory option");
    invariant(!this.#loopMemory || typeof wasm.backend_set_loop_memory === "function",
      "compiler does not support loop memory proofs");
    invariant(!(this.#loopMemory && (this.#rootedSsa || this.#valueIr)),
      "loop memory proofs require ordinary CFG lowering");
    invariant(typeof this.#expandedCache === "boolean", "invalid expanded-cache option");
    invariant(!this.#expandedCache || typeof wasm.backend_set_cache_capacity === "function" &&
      typeof wasm.backend_cache_capacity === "function", "compiler does not support expanded cache retention");
    this.#handle = this.#newHandle();
    try { this.#links = this.#newLinkTable(this.#handle); }
    catch (error) { this.#wasm.backend_drop!(this.#handle); throw error; }
  }
  static async instantiate(url: URL, options: ExecutionEngineOptions): Promise<RecompilerMirror> {
    const response = await fetch(url);
    invariant(response.ok, `independent backend fetch failed: ${response.status}`);
    return this.instantiateBytes(new Uint8Array(await response.arrayBuffer()), options);
  }
  static async instantiateBytes(bytes: Uint8Array, options: ExecutionEngineOptions): Promise<RecompilerMirror> {
    const instance = await WebAssembly.instantiate(Uint8Array.from(bytes).buffer);
    return new RecompilerMirror(instance.instance.exports as BackendExports, options);
  }
  #error(): string {
    return new TextDecoder().decode(new Uint8Array(this.#wasm.memory.buffer,
      this.#wasm.backend_error_ptr!(), this.#wasm.backend_error_len!()));
  }
  #check(result: number): void { invariant(result === 0, this.#error()); }
  #newLinkTable(handle: number): WebAssembly.Table {
    // Older fixed-capacity compiler artifacts retain their original table ABI.
    // New artifacts own the capacity; Chrome only allocates its matching resource.
    const count = typeof this.#wasm.backend_cache_capacity === "function"
      ? this.#wasm.backend_cache_capacity(handle) : 64;
    const maximum = typeof this.#wasm.backend_cache_maximum_capacity === "function"
      ? this.#wasm.backend_cache_maximum_capacity() : count;
    invariant(Number.isInteger(count) && count > 0 && count <= maximum &&
      Number.isInteger(maximum) && maximum <= 4608, "invalid compiler cache capacity");
    return linkTable(count, maximum);
  }
  #newHandle(): number {
    const handle = this.#wasm.backend_new!(this.#maximumPages);
    invariant(handle !== 0, this.#error());
    if (this.#directLinks && this.#wasm.backend_set_direct_links(handle, 1) !== 0) {
      const error = this.#error(); this.#wasm.backend_drop(handle); throw new Error(error);
    }
    if (this.#controlGraph && this.#wasm.backend_set_control_graph(handle, 1) !== 0) {
      const error = this.#error(); this.#wasm.backend_drop(handle); throw new Error(error);
    }
    if (this.#bindingGlobal && this.#wasm.backend_set_binding_global(handle, 1) !== 0) {
      const error = this.#error(); this.#wasm.backend_drop(handle); throw new Error(error);
    }
    if (this.#functionCfg && this.#wasm.backend_set_function_cfg(handle, 1) !== 0) {
      const error = this.#error(); this.#wasm.backend_drop(handle); throw new Error(error);
    }
    if (this.#loopMemory && this.#wasm.backend_set_loop_memory(handle, 1) !== 0) {
      const error = this.#error(); this.#wasm.backend_drop(handle); throw new Error(error);
    }
    if (typeof this.#wasm.backend_set_cache_capacity === "function" &&
        this.#wasm.backend_set_cache_capacity(handle, Number(this.#expandedCache)) !== 0) {
      const error = this.#error(); this.#wasm.backend_drop!(handle); throw new Error(error);
    }
    if (this.#fpIr && this.#wasm.backend_set_fp_ir!(handle, 1) !== 0) {
      const error = this.#error();
      this.#wasm.backend_drop!(handle);
      throw new Error(error);
    }
    if (this.#numericMode !== "strict" && this.#wasm.backend_set_numeric_mode(handle, this.#numericMode === "balanced" ? 1 : 2) !== 0) {
      const error = this.#error(); this.#wasm.backend_drop(handle); throw new Error(error);
    }
    if (this.#typedNumeric && this.#wasm.backend_set_typed_numeric!(handle, 1) !== 0) {
      const error = this.#error();
      this.#wasm.backend_drop!(handle);
      throw new Error(error);
    }
    if (this.#rootedSsa && this.#wasm.backend_set_rooted_ssa!(handle, 1) !== 0) {
      const error = this.#error(); this.#wasm.backend_drop!(handle); throw new Error(error);
    }
    if (this.#valueIr && this.#wasm.backend_set_value_ir!(handle, 1) !== 0) {
      const error = this.#error();
      this.#wasm.backend_drop!(handle);
      throw new Error(error);
    }
    if (this.#predicateIr && this.#wasm.backend_set_predicate_ir!(handle, 1) !== 0) {
      const error = this.#error(); this.#wasm.backend_drop!(handle); throw new Error(error);
    }
    if (this.#flowIr && this.#wasm.backend_set_flow_ir!(handle, 1) !== 0) {
      const error = this.#error(); this.#wasm.backend_drop!(handle); throw new Error(error);
    }
    return handle;
  }
  #allocate(length: number): number {
    if (length === 0) return 0;
    const ptr = this.#wasm.backend_input_alloc!(this.#handle, length);
    invariant(ptr !== 0, this.#error()); return ptr;
  }
  #free(ptr: number, length: number): void {
    if (ptr !== 0) this.#check(this.#wasm.backend_free!(ptr, length));
  }
  #update(initial: boolean, delta: HostMemoryDelta): void {
    const { control, payload } = encodeUpdate(delta);
    const cp = this.#allocate(control.length);
    let pp = 0;
    try {
      pp = this.#allocate(payload.length);
      new Uint8Array(this.#wasm.memory.buffer, cp, control.length).set(control);
      if (payload.length) new Uint8Array(this.#wasm.memory.buffer, pp, payload.length).set(payload);
      this.#check(this.#wasm.backend_apply_runtime_update!(this.#handle, Number(initial), cp,
        control.length, pp, payload.length));
      // Rust applies the complete delta before retiring stale dependencies.
      // The summary flags must not erase unrelated prepared/native code.
      if (delta.cacheEffects.invalidateGeneratedCode || delta.cacheEffects.invalidateInstructionFetch) {
        this.#releaseRetired();
      }
    } finally { this.#free(pp, payload.length); this.#free(cp, control.length); }
  }
  initialize(reflection: MemoryReflection, bootstrap: ExecutionBootstrap): void {
    invariant(!this.#initialized, "independent backend is already initialized");
    const entry = u32(bootstrap.entryPoint, "entry point"), esp = u32(bootstrap.initialStackPointer, "stack pointer"),
      fs = u32(bootstrap.fsBase ?? 0, "FS base");
    const mappingOperations: MappingOperation[] = bootstrap.regions.map(region => ({ operation: "reserve",
      owner: region.owner, kind: region.kind, addressAllocationId: region.addressAllocationId,
      range: { base: region.range.start, byteLength: region.range.byteLength } }));
    this.#update(true, { ...reflection, fromGeneration: 0n, mappingOperations });
    this.#check(this.#wasm.backend_initialize_state!(this.#handle, entry, esp, fs));
    this.#initialized = true;
  }
  rebuildMemory(reflection: MemoryReflection, regions: MemoryRegionSnapshot[]): void {
    invariant(!this.#preparing, "cannot rebuild memory during code preparation");
    invariant(this.collectChangedPageViews().length === 0,
      "CPU writeback must be acknowledged before rebuilding memory");
    const context = this.saveCpuContext(), oldHandle = this.#handle, oldModules = this.#modules,
      oldLinks = this.#links, oldSnapshotPointer = this.#snapshotPointer;
    const newHandle = this.#newHandle();
    let newLinks: WebAssembly.Table;
    try { newLinks = this.#newLinkTable(newHandle); }
    catch (error) { this.#wasm.backend_drop!(newHandle); throw error; }
    this.#handle = newHandle; this.#initialized = false; this.#modules = new Map(); this.#links = newLinks;
    this.#snapshotPointer = 0;
    try {
      this.initialize(reflection, { regions, entryPoint: context.instructionPointer,
        initialStackPointer: context.esp });
      this.restoreCpuContext(context);
    } catch (error) {
      this.#wasm.backend_drop!(newHandle); this.#handle = oldHandle;
      this.#modules = oldModules; this.#links = oldLinks; this.#snapshotPointer = oldSnapshotPointer;
      this.#initialized = true; throw error;
    }
    this.#check(this.#wasm.backend_drop!(oldHandle));
  }
  #words(): Uint32Array {
    return new Uint32Array(this.#wasm.memory.buffer, this.#wasm.backend_state!(this.#handle), stateBytes / 4);
  }
  #context(words: Uint32Array): ExecutionCpuContext {
    return { snapshot: { backend: "recompiler", abiVersion: 2, words }, eax: words[0]!, ecx: words[1]!,
      edx: words[2]!, ebx: words[3]!, esp: words[4]!, ebp: words[5]!, esi: words[6]!, edi: words[7]!,
      instructionPointer: words[8]!, eflags: words[9]!, es: words[11]!, cs: words[15]!, ss: words[19]!,
      ds: words[23]!, fs: words[27]!, gs: words[31]! };
  }
  #snapshotBuffer(): number {
    // The handle owns this reusable transport allocation; dropping the handle
    // also drops it. Never retain a view across memory growth or backend calls.
    if (this.#snapshotPointer === 0) this.#snapshotPointer = this.#allocate(snapshotBytes);
    return this.#snapshotPointer;
  }
  saveCpuContext(): ExecutionCpuContext {
    const ptr = this.#snapshotBuffer();
    this.#check(this.#wasm.backend_save_snapshot(this.#handle, ptr, snapshotBytes));
    return this.#context(Uint32Array.from(new Uint32Array(this.#wasm.memory.buffer, ptr, snapshotBytes / 4)));
  }
  restoreCpuContext(context: Pick<ExecutionCpuContext, "snapshot">): void {
    invariant(context.snapshot?.backend === "recompiler" && context.snapshot.abiVersion === 2 &&
      context.snapshot.words instanceof Uint32Array && context.snapshot.words.byteLength === snapshotBytes,
    "independent backend CPU context ABI mismatch");
    const ptr = this.#snapshotBuffer();
    new Uint32Array(this.#wasm.memory.buffer, ptr, snapshotBytes / 4).set(context.snapshot.words);
    this.#check(this.#wasm.backend_restore_snapshot(this.#handle, ptr, snapshotBytes));
  }
  createCpuContext(bootstrap: ExecutionThreadBootstrap): ExecutionCpuContext {
    const ptr = this.#snapshotBuffer();
    this.#check(this.#wasm.backend_create_snapshot(this.#handle, u32(bootstrap.entryPoint, "entry point"),
      u32(bootstrap.initialStackPointer, "stack pointer"), u32(bootstrap.fsBase ?? 0, "FS base"), ptr, snapshotBytes));
    return this.#context(Uint32Array.from(new Uint32Array(this.#wasm.memory.buffer, ptr, snapshotBytes / 4)));
  }
  applyCpuResume(resume: CpuResume): void {
    this.#check(this.#wasm.backend_resume!(this.#handle, u32(resume.instructionPointer, "resume EIP"),
      u32(resume.stackPointer, "resume ESP"), u32(resume.eax ?? 0, "resume EAX"), Number(resume.eax !== null)));
  }
  applyStoppedCpuResume(resume: CpuResume): void { this.applyCpuResume(resume); }
  enterGuestCallback(target: number, stackPointer: number): void {
    this.applyCpuResume({ instructionPointer: target, stackPointer, eax: null });
  }
  #releaseRetired(): void {
    // Rust bounds the queue to its capacity, replacing overflow with a flush event.
    // Dropping a JS instance does not remove its still-valid Rust code unit.
    for (let count = 0; count <= this.#links.length; count++) {
      const id = this.#wasm.backend_cache_retired!(this.#handle) >>> 0;
      if (id === 0) return;
      if (id === 0xffff_ffff) {
        for (let slot = 0; slot < this.#links.length; slot++) this.#links.set(slot, null);
        this.#modules.clear();
      } else {
        const retired = this.#modules.get(id);
        if (retired && retired.slot !== null) this.#links.set(retired.slot, null);
        this.#modules.delete(id);
      }
    }
    throw new Error("independent backend retirement notification bound exceeded");
  }
  #instantiateSelected(id: number): GeneratedResource {
    let resource = this.#modules.get(id);
    if (!resource) {
      const code = new Uint8Array(this.#wasm.memory.buffer, this.#wasm.backend_code_ptr!(this.#handle),
        this.#wasm.backend_code_len!(this.#handle));
      const module = new WebAssembly.Instance(new WebAssembly.Module(code),
        { env: { memory: this.#wasm.memory, x87: this.#wasm.backend_x87!, backend: this.#handle, unit: id,
          ...(this.#directLinks ? {link_directory: this.#wasm.backend_link_directory(this.#handle)} : {}),
          fp_numeric: this.#wasm.backend_fp_numeric!,
          fp_kernel_error: this.#wasm.backend_fp_kernel_error!,
          sse_cvtt_f32: this.#wasm.backend_sse_cvtt_f32,
          sse_cvtt_f64: this.#wasm.backend_sse_cvtt_f64,
          sse_i32_to_f32: this.#wasm.backend_sse_i32_to_f32,
          sse_mul_f32: this.#wasm.backend_sse_mul_f32,
          sse_add_f32: this.#wasm.backend_sse_add_f32,
          sse_sub_f32: this.#wasm.backend_sse_sub_f32,
          sse_cvt_f64: this.#wasm.backend_sse_cvt_f64,
          sse_add_f64: this.#wasm.backend_sse_add_f64,
          sse_sub_f64: this.#wasm.backend_sse_sub_f64,
          sse_mul_f64: this.#wasm.backend_sse_mul_f64,
          ...Object.fromEntries(Array.from({length: 26}, (_, index) =>
            [`fp_kernel_${index}`, this.#wasm[`backend_fp_kernel_${index}`]!])),
          ...Object.fromEntries((["balanced", "fast"] as const).flatMap(mode => Array.from({length: 26}, (_, i) =>
            [`fp_${mode}_${i}`, this.#wasm[`backend_fp_${mode}_${i}`]]))),
          begin_timestamp_counter: this.#wasm.backend_begin_timestamp_counter!,
          scas_begin: this.#wasm.backend_scas_begin, scas_complete: this.#wasm.backend_scas_complete,
          scas_finish: this.#wasm.backend_scas_finish,
          links: this.#links, ...(this.#supportsLinks ? { link_resolve: this.#wasm.backend_link_resolve! } : {}) } }).exports as GeneratedExports;
      let slot: number | null = null;
      if (this.#supportsLinks) {
        const slotId = this.#wasm.backend_link_slot!(this.#handle, id) >>> 0;
        invariant(slotId > 0 && slotId <= this.#links.length && typeof module.link === "function",
          "independent backend returned invalid link resource");
        slot = slotId - 1;
        invariant(this.#links.get(slot) === null,
          "independent backend reused a live link slot");
        this.#links.set(slot, module.link);
        const result = this.#wasm.backend_link_bind!(this.#handle, id, slotId);
        if (result !== 0) this.#links.set(slot, null);
        this.#check(result);
      }
      resource = { exports: module, slot, prepared: false };
      this.#modules.set(id, resource);
    }
    return resource;
  }
  async prepareExecution(entries: number[]): Promise<{ milliseconds: number; limited: boolean }> {
    invariant(this.#initialized && !this.#preparing, "invalid code preparation lifecycle");
    invariant(Array.isArray(entries) && entries.length > 0 && entries.length <= 4096, "invalid code preparation root count");
    const roots = Array.from(entries, entry => u32(entry, "code preparation root"));
    invariant([this.#wasm.backend_prepare_begin, this.#wasm.backend_prepare_next,
      this.#wasm.backend_prepare_status, this.#wasm.backend_prepare_end].every(fn => typeof fn === "function"),
    "compiler does not support code preparation");
    const started = performance.now(), ptr = this.#allocate(roots.length * 4);
    try {
      const view = new DataView(this.#wasm.memory.buffer, ptr, roots.length * 4);
      roots.forEach((entry, index) => view.setUint32(index * 4, entry, true));
      this.#check(this.#wasm.backend_prepare_begin(this.#handle, ptr, roots.length));
    } finally { this.#free(ptr, roots.length * 4); }
    this.#preparing = true;
    try {
      const capacity = this.#wasm.backend_cache_capacity(this.#handle);
      invariant(Number.isInteger(capacity) && capacity >= this.#links.length && capacity <= 4608,
        "invalid prepared code capacity");
      if (capacity > this.#links.length) this.#links.grow(capacity - this.#links.length);
      let yielded = started;
      for (;;) {
        this.#check(this.#wasm.backend_prepare_next(this.#handle));
        this.#releaseRetired();
        const status = this.#wasm.backend_prepare_status(this.#handle);
        if (status === 2 || status === 3) return { milliseconds: performance.now() - started, limited: status === 3 };
        invariant(status === 1, "invalid code preparation status");
        const id = this.#wasm.backend_cache_selected(this.#handle) >>> 0;
        invariant(id !== 0 && id !== 0xffff_ffff, "invalid prepared unit identity");
        const resource = this.#instantiateSelected(id);
        if (!resource.prepared && resource.exports.prepare) {
          invariant(resource.exports.prepare() === 0, "generated code preparation failed");
          resource.prepared = true;
        }
        if (performance.now() - yielded >= 8) {
          await new Promise<void>(resolve => setTimeout(resolve, 0));
          yielded = performance.now();
        }
      }
    } finally {
      this.#preparing = false;
      this.#check(this.#wasm.backend_prepare_end(this.#handle));
    }
  }
  executeBudget(instructionBudget: number): ExecutionSlice {
    invariant(!this.#preparing, "cannot execute guest code during preparation");
    invariant(this.#initialized, "independent backend is not initialized");
    invariant(Number.isInteger(instructionBudget) && instructionBudget > 0 && instructionBudget <= 0x7fff_ffff,
      "independent backend instruction budget is invalid");
    let remaining = instructionBudget;
    for (let exits = 0; exits < 4096; exits++) {
      const eip = this.#words()[8]!;
      this.#check(this.#wasm.backend_cache_select!(this.#handle, eip));
      this.#releaseRetired();
      let id = this.#wasm.backend_cache_selected!(this.#handle) >>> 0;
      if (id === 0) {
        this.#check(this.#wasm.backend_compile!(this.#handle, eip, 512));
        this.#releaseRetired();
        id = this.#wasm.backend_cache_selected!(this.#handle) >>> 0;
        invariant(id !== 0 && id !== 0xffff_ffff, "independent backend returned invalid compiled unit identity");
      }
      const resource = this.#instantiateSelected(id);
      const run = this.#linkedRegions ? resource.exports.run_linked : resource.exports.run;
      invariant(typeof run === "function", "independent backend has no requested execution entry");
      const scopedAdmission = this.#linkedRegions && this.#supportsLinkAdmission;
      this.#check((scopedAdmission ? this.#wasm.backend_link_begin! : this.#wasm.backend_cache_validate!)(
        this.#handle, id, eip));
      let retired: number, runFailed = false, runError: unknown;
      try {
        retired = run(this.#wasm.backend_state!(this.#handle),
          this.#wasm.backend_table!(this.#handle), remaining) >>> 0;
      } catch (error) {
        runFailed = true; runError = error; throw error;
      } finally {
        if (scopedAdmission) {
          try { this.#check(this.#wasm.backend_link_end!(this.#handle)); }
          catch (cleanup) {
            if (runFailed) throw new Error(`${String(runError)}; link admission cleanup failed: ${String(cleanup)}`,
              { cause: runError });
            throw cleanup;
          }
        }
      }
      invariant(retired <= remaining, "independent backend exceeded instruction budget");
      remaining -= retired;
      this.#check(this.#wasm.backend_finish!(this.#handle));
      const words = this.#words(), kind = words[100]!;
      if (kind === 12 || kind === 13) {
        // REP exposes only completed elements: EIP still names the string
        // instruction, ECX/ESI/EDI carry the exact remaining work. Whole guest
        // instruction retirement stays separate from this bounded progress.
        invariant(words[103]! > 0 && words[103]! <= 4096 && words[1]! > 0,
          "independent backend returned invalid partial REP progress");
        return { stop: { kind: "budget-yield" }, instructions: instructionBudget - remaining,
          context: this.saveCpuContext() };
      }
      if ((kind === 2 || kind === 7) && remaining > 0) {
        invariant(retired > 0 || words[8] !== eip, "independent backend made no progress");
        continue;
      }
      let stop: ExecutionStop;
      if (kind === 1 || ((kind === 2 || kind === 7) && remaining === 0)) stop = { kind: "budget-yield" };
      else if (kind === 5) stop = { kind: "runtime-dispatch" };
      else if (kind === 15) stop = { kind: "timestamp-counter" };
      else if (kind === 8) stop = { kind: "trap", trap: "invalid-opcode", errorCode: words[103]! };
      else stop = { kind: "fault", fault: kind === 4 ? "page-fault" : kind === 6 ?
        "general-protection" : kind === 9 ? "stack-segment" : kind === 11 ? "divide-error" : kind === 14 ? "floating-point" : kind === 16 ? "simd-floating-point" : "engine-stop", errorCode: words[103]!, engineReason: kind,
        faultAddress: words[102]! };
      return { stop, instructions: instructionBudget - remaining, context: this.saveCpuContext() };
    }
    // Region transfers also bound one host turn. A guest making real progress
    // must yield with its current PC/effects, not fail because its blocks are
    // small. Keep the final transfer reason so JIT discovery on resume does
    // not mistake this host scheduling boundary for an in-block budget suffix.
    invariant(remaining < instructionBudget, "independent backend made no progress across bounded region exits");
    return { stop: { kind: "budget-yield" }, instructions: instructionBudget - remaining,
      context: this.saveCpuContext() };
  }
  completeTimestampCounter(instructionPointer: number, low: number, high: number): void {
    invariant(typeof this.#wasm.backend_complete_timestamp_counter === "function",
      "independent backend does not support timestamp completion");
    this.#check(this.#wasm.backend_complete_timestamp_counter(this.#handle,
      u32(instructionPointer, "timestamp EIP"), u32(low, "timestamp low"), u32(high, "timestamp high")));
  }
  readGuestBytes(address: number, byteLength: number, _borrow = false): Uint8Array {
    u32(address, "guest read address");
    invariant(Number.isInteger(byteLength) && byteLength > 0 && byteLength <= 0xffff_ffff,
      "invalid guest read length");
    const ptr = this.#allocate(byteLength);
    try {
      this.#check(this.#wasm.backend_read_guest!(this.#handle, address, ptr, byteLength));
      return Uint8Array.from(new Uint8Array(this.#wasm.memory.buffer, ptr, byteLength));
    } finally { this.#free(ptr, byteLength); }
  }
  readGuestU32(address: number): number { return new DataView(this.readGuestBytes(address, 4).buffer).getUint32(0, true); }
  collectChangedPageViews(): DirtyMirrorPage[] {
    this.#check(this.#wasm.backend_writeback_prepare!(this.#handle));
    const control = new DataView(this.#wasm.memory.buffer, this.#wasm.backend_writeback_control_ptr!(this.#handle),
      this.#wasm.backend_writeback_control_len!(this.#handle));
    const count = control.getUint32(20, true), payload = this.#wasm.backend_writeback_payload_ptr!(this.#handle);
    invariant(control.byteLength === 28 + count * 8 &&
      this.#wasm.backend_writeback_payload_len!(this.#handle) === count * pageSize,
    "independent backend returned malformed writeback");
    return Array.from({ length: count }, (_, index) => ({ address: control.getUint32(28 + index * 8, true),
      owner: control.getUint32(32 + index * 8, true),
      bytes: new Uint8Array(this.#wasm.memory.buffer, payload + index * pageSize, pageSize) }));
  }
  collectChangedPages(): DirtyMirrorPage[] {
    return this.collectChangedPageViews().map(page => ({ ...page, bytes: Uint8Array.from(page.bytes) }));
  }
  acknowledgeWriteback(generation: bigint): void {
    invariant(generation >= 0n && generation <= 0xffff_ffff_ffff_ffffn, "invalid writeback generation");
    this.#check(this.#wasm.backend_writeback_ack!(this.#handle,
      Number(generation & 0xffff_ffffn), Number(generation >> 32n)));
  }
  applyMemoryDelta(delta: HostMemoryDelta): void { this.#update(false, delta); }
  applyCacheEffects(effects: CacheEffects): void {
    if (effects.invalidateGeneratedCode || effects.invalidateInstructionFetch) {
      this.#check(this.#wasm.backend_cache_invalidate!(this.#handle));
      this.#releaseRetired();
    }
  }
}
