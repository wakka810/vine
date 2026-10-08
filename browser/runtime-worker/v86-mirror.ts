import {
  type CpuResume,
  type ExecutionBootstrap,
  type ExecutionCpuContext,
  type ExecutionEngine,
  type ExecutionEngineOptions,
  type ExecutionSlice,
  type ExecutionStop,
  type ExecutionThreadBootstrap,
} from "./execution-engine-contract.js";
import {
  type CacheEffects,
  type DirtyMirrorPage,
  type HostMemoryDelta,
  type MappingOperation,
  type MemoryRange,
  type MemoryProtection,
  type MemoryReflection,
  type MemoryRegionKind,
  type MemoryRegionSnapshot,
  memoryPageSize,
} from "./memory-transport.js";

const dirtyPageCandidateCapacity = 16 * 1024;
const jitTableOffset = 1_024;
const jitTableSize = 900;
const initialPhysicalPoolBytes = 64 * 1024;
// Bound the live guest set and leave room for the mirror's page tables.
const hardMaximumPhysicalPoolBytes = 1600 * 1024 * 1024;
const excludedPhysicalRanges = [{ start: 0x000a_0000, end: 0x000c_0000 }] as const;
const pageFlags = { present: 1, writable: 2, user: 4, dirty: 0x40 } as const;
const stateWord = {
  magic: 0,
  wordCount: 1,
  eax: 2,
  ecx: 3,
  edx: 4,
  ebx: 5,
  esp: 6,
  ebp: 7,
  esi: 8,
  edi: 9,
  instructionPointer: 10,
  previousInstructionPointer: 11,
  flags: 12,
  cpl: 18,
  protectedMode: 19,
  operandSize32: 20,
  stackSize32: 21,
  inHalt: 22,
  segmentSelectorBase: 26,
  segmentOffsetBase: 34,
  segmentLimitBase: 42,
  segmentMetaBase: 50,
  controlRegisterBase: 58,
} as const;

export const v86StopReason = {
  budget: 1,
  halt: 2,
  executeProtection: 3,
  runtimeDispatch: 4,
  divideError: 0x100,
  invalidOpcode: 0x106,
  generalProtection: 0x10d,
  pageFault: 0x10e,
} as const;

interface V86Exports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  rust_init(): void;
  allocate_memory(bytes: number): number;
  zero_memory(address: number, bytes: number): void;
  reset_cpu(): void;
  full_clear_tlb(): void;
  jit_clear_cache_js(): void;
  jit_dirty_cache(start: number, end: number): void;
  get_jit_config(index: number): number;
  codegen_finalize_finished(index: number, start: number, stateFlags: number): void;
  vine_cpu_state_ptr(): number;
  vine_cpu_state_size(): number;
  vine_cpu_state_save(): void;
  vine_cpu_state_restore(): number;
  vine_cpu_apply_stopped_resume(eax: number, writeEax: number, esp: number, eip: number): number;
  vine_rearm_dirty_page(address: number): number;
  vine_dirty_pages_ptr(): number;
  vine_dirty_page_count(): number;
  vine_dirty_pages_overflowed(): number;
  vine_reset_dirty_pages(): void;
  vine_execute_budget(instructions: number): number;
  vine_get_stop_error_code(): number;
  vine_instruction_counter(): number;
  vine_reset_execute_permissions(): void;
  vine_set_memory_size(bytes: number): number;
  vine_set_page_executable(pageAddress: number, executable: number): number;
}

interface MirrorMapping {
  address: number;
  addressAllocationId?: number;
  owner: number;
  protection: MemoryProtection;
  physicalBase: number;
  pteAddress: number;
  pteWordIndex: number;
}

interface MirrorReservation {
  addressAllocationId?: number;
  owner: number;
  kind: MemoryRegionKind;
  range: MemoryRange;
}

interface PlannedMapping {
  addressAllocationId?: number;
  owner: number;
  protection: MemoryProtection;
}

interface PhysicalCapacityRequirement {
  guestPages: number;
  pageTables: number;
  bytes: number;
}

interface PreparedAdapter {
  wasm: V86Exports;
  physicalBase: number;
  dirtyPagesPointer: number;
}

function physicalCapacityError(
  requestedPhysicalPages: number,
  maximumPhysicalPages: number,
  requiredPhysicalPoolBytes: number,
  maximumPhysicalPoolBytes: number,
): Error {
  return new Error(
    `v86 physical pool exhausted: ${requestedPhysicalPages} guest pages require ` +
      `${requiredPhysicalPoolBytes} mirror bytes; limits are ${maximumPhysicalPages} pages ` +
      `and ${maximumPhysicalPoolBytes} bytes`,
  );
}

export interface CpuState {
  words: Uint32Array;
  eax: number;
  esp: number;
  instructionPointer: number;
  previousInstructionPointer: number;
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function requirePageCount(value: number, label: string): number {
  invariant(Number.isSafeInteger(value) && value > 0, `${label} must be a positive integer`);
  return value;
}

function physicalPoolBytesForAllocations(allocationCount: number): number {
  invariant(
    Number.isSafeInteger(allocationCount) && allocationCount > 0,
    "physical allocation count is invalid",
  );
  let remaining = allocationCount;
  let cursor = memoryPageSize;
  for (const excluded of excludedPhysicalRanges) {
    invariant(
      excluded.start % memoryPageSize === 0 &&
        excluded.end % memoryPageSize === 0 &&
        excluded.start >= cursor &&
        excluded.end > excluded.start,
      "excluded physical range is invalid",
    );
    const pagesBefore = (excluded.start - cursor) / memoryPageSize;
    if (remaining <= pagesBefore) {
      return cursor + remaining * memoryPageSize;
    }
    remaining -= pagesBefore;
    cursor = excluded.end;
  }
  return cursor + remaining * memoryPageSize;
}

function maximumPhysicalPoolBytes(maximumPhysicalPages: number): number {
  const maximumPageTables = Math.min(maximumPhysicalPages, 1024);
  return physicalPoolBytesForAllocations(
    1 + maximumPageTables + maximumPhysicalPages,
  );
}


export function classifyV86Stop(
  reason: number,
  errorCode: number,
  faultAddress?: number,
): ExecutionStop {
  switch (reason) {
    case v86StopReason.budget:
      return { kind: "budget-yield" };
    case v86StopReason.halt:
      return { kind: "halt" };
    case v86StopReason.runtimeDispatch:
      return { kind: "runtime-dispatch" };
    case v86StopReason.executeProtection:
      return {
        kind: "fault",
        fault: "execute-protection",
        errorCode,
        engineReason: reason,
        faultAddress,
      };
    case v86StopReason.invalidOpcode:
      return { kind: "trap", trap: "invalid-opcode", errorCode };
    case v86StopReason.divideError:
      return { kind: "fault", fault: "divide-error", errorCode, engineReason: reason };
    case v86StopReason.generalProtection:
      return {
        kind: "fault",
        fault: "general-protection",
        errorCode,
        engineReason: reason,
      };
    case v86StopReason.pageFault:
      return {
        kind: "fault",
        fault: "page-fault",
        errorCode,
        engineReason: reason,
        faultAddress,
      };
    default:
      return { kind: "fault", fault: "engine-stop", errorCode, engineReason: reason };
  }
}

function sameProtection(left: MemoryProtection, right: MemoryProtection): boolean {
  return left.read === right.read && left.write === right.write && left.execute === right.execute;
}

function rangeEnd(range: MemoryRange): number {
  return range.base + range.byteLength;
}

function rangesOverlap(left: MemoryRange, right: MemoryRange): boolean {
  return left.base < rangeEnd(right) && right.base < rangeEnd(left);
}

function rangeContains(outer: MemoryRange, inner: MemoryRange): boolean {
  return outer.base <= inner.base && rangeEnd(inner) <= rangeEnd(outer);
}

export class V86Mirror implements ExecutionEngine {
  readonly #module: WebAssembly.Module;
  #wasm!: V86Exports;
  readonly #mappings = new Map<number, MirrorMapping>();
  readonly #reservations = new Map<number, MirrorReservation>();
  readonly #directoryMappingCounts = new Map<number, number>();
  readonly #dirtyMappingsScratch: MirrorMapping[] = [];
  readonly #borrowedDirtyPages: DirtyMirrorPage[] = [];
  readonly #borrowedDirtyPagePool: DirtyMirrorPage[] = [];
  #dirtyPageCandidates!: Uint32Array;
  #dirtyPagesPointer = 0;
  #physicalBase = 0;
  #physicalMemory!: Uint8Array;
  #physicalWords!: Uint32Array;
  #physicalView!: DataView;
  readonly #maximumPhysicalPages: number;
  readonly #maximumPhysicalPoolBytes: number;
  #physicalPoolBytes: number;
  #pageDirectory = 0;
  #nextPhysicalPage = memoryPageSize;
  readonly #freePhysicalPages: number[] = [];
  readonly #pageTables = new Map<number, number>();
  #forceFullDirtyScan = false;
  #threadContextTemplate: Uint32Array | null = null;

  private constructor(
    module: WebAssembly.Module,
    options: ExecutionEngineOptions,
  ) {
    this.#module = module;
    this.#maximumPhysicalPages = requirePageCount(
      options.maximumPhysicalPages,
      "maximum physical page count",
    );
    this.#maximumPhysicalPoolBytes = maximumPhysicalPoolBytes(this.#maximumPhysicalPages);
    invariant(
      this.#maximumPhysicalPoolBytes <= hardMaximumPhysicalPoolBytes,
      `configured physical page limit requires ${this.#maximumPhysicalPoolBytes} bytes, ` +
        `above the hard ${hardMaximumPhysicalPoolBytes}-byte mirror limit`,
    );
    this.#physicalPoolBytes = Math.min(
      Math.max(initialPhysicalPoolBytes, physicalPoolBytesForAllocations(1)),
      this.#maximumPhysicalPoolBytes,
    );
    this.#publishAdapter(this.#prepareAdapter(this.#physicalPoolBytes), this.#physicalPoolBytes);
    this.#pageDirectory = this.#allocatePage();
  }

  static async instantiate(url: URL, options: ExecutionEngineOptions): Promise<V86Mirror> {
    const response = await fetch(url);
    invariant(response.ok, `v86 adapter fetch failed with ${response.status}`);
    return V86Mirror.instantiateBytes(new Uint8Array(await response.arrayBuffer()), options);
  }

  static async instantiateBytes(
    bytes: Uint8Array,
    options: ExecutionEngineOptions,
  ): Promise<V86Mirror> {
    invariant(options !== undefined, "v86 mirror requires the runtime physical page limit");
    const ownedBytes = new Uint8Array(bytes.byteLength);
    ownedBytes.set(bytes);
    const module = await WebAssembly.compile(ownedBytes.buffer);
    return new V86Mirror(module, options);
  }

  static #instantiateAdapter(module: WebAssembly.Module): V86Exports {
    const imports = WebAssembly.Module.imports(module);
    const table = new WebAssembly.Table({
      element: "anyfunc",
      initial: jitTableOffset + jitTableSize,
    });
    let wasm: V86Exports | undefined;
    let jitImports: Record<string, WebAssembly.ImportValue> | undefined;
    const environment: Record<string, WebAssembly.ImportValue> = {
      __indirect_function_table: table,
    };
    for (const imported of imports) {
      invariant(imported.module === "env", `unexpected v86 import module ${imported.module}`);
      if (imported.kind !== "function") {
        invariant(imported.kind === "table", `unexpected v86 import ${imported.name}`);
        continue;
      }
      environment[imported.name] = (...arguments_: unknown[]) => {
        if (imported.name === "microtick") {
          return 0;
        }
        if (imported.name === "get_rand_int") {
          return 0;
        }
        if (imported.name === "codegen_finalize") {
          invariant(arguments_.length === 5, "v86 generated an invalid JIT compile request");
          const [index, start, stateFlags, pointer, byteLength] = arguments_ as [
            number,
            number,
            number,
            number,
            number,
          ];
          invariant(wasm !== undefined && jitImports !== undefined, "JIT host bridge is not ready");
          invariant(
            Number.isInteger(index) && index >= 0 && index < jitTableSize,
            "v86 generated an invalid JIT table index",
          );
          invariant(
            Number.isInteger(pointer) &&
              Number.isInteger(byteLength) &&
              pointer >= 0 &&
              byteLength > 0 &&
              pointer + byteLength <= wasm.memory.buffer.byteLength,
            "v86 generated an invalid JIT module range",
          );
          const code = Uint8Array.from(
            new Uint8Array(wasm.memory.buffer, pointer, byteLength),
          );
          const generated = new WebAssembly.Module(code);
          for (const dependency of WebAssembly.Module.imports(generated)) {
            invariant(
              dependency.module === "e" && dependency.name in jitImports,
              `v86 JIT module requested unsupported import ${dependency.module}.${dependency.name}`,
            );
          }
          const generatedExports = new WebAssembly.Instance(generated, { e: jitImports }).exports;
          const entry = generatedExports.f;
          invariant(typeof entry === "function", "v86 JIT module did not export its entry point");
          table.set(index + jitTableOffset, entry);
          if (wasm.get_jit_config(6) === 0) {
            queueMicrotask(() => wasm?.codegen_finalize_finished(index, start, stateFlags));
          }
          return 0;
        }
        if (imported.name === "jit_clear_func") {
          invariant(arguments_.length === 1, "v86 generated an invalid JIT clear request");
          const [index] = arguments_ as [number];
          invariant(
            Number.isInteger(index) && index >= 0 && index < jitTableSize,
            "v86 requested an invalid JIT table clear",
          );
          table.set(index + jitTableOffset, null);
          return 0;
        }
        if (
          imported.name === "stop_idling" ||
          imported.name === "cpu_event_halt" ||
          imported.name === "run_hardware_timers"
        ) {
          return 0;
        }
        throw new Error(`v86 reached unsupported host import ${imported.name}`);
      };
    }
    wasm = new WebAssembly.Instance(module, { env: environment }).exports as V86Exports;
    jitImports = { m: wasm.memory };
    for (const [name, value] of Object.entries(wasm)) {
      if (!name.startsWith("_") && !name.startsWith("zstd") && !name.endsWith("_js")) {
        jitImports[name] = value;
      }
    }
    return wasm;
  }

  #prepareAdapter(poolBytes: number): PreparedAdapter {
    const wasm = V86Mirror.#instantiateAdapter(this.#module);
    wasm.rust_init();
    invariant(
      wasm.vine_cpu_state_size() === 612,
      "execution engine CPU context ABI mismatch",
    );
    invariant(wasm.vine_set_memory_size(poolBytes) === 0, "v86 rejected mirror memory size");
    wasm.vine_reset_execute_permissions();
    const physicalBase = wasm.allocate_memory(poolBytes);
    invariant(physicalBase > 0 && physicalBase % memoryPageSize === 0,
      "execution memory allocation is missing or unaligned");
    const dirtyPagesPointer = wasm.vine_dirty_pages_ptr();
    invariant(
      dirtyPagesPointer % Uint32Array.BYTES_PER_ELEMENT === 0 &&
        dirtyPagesPointer + dirtyPageCandidateCapacity * Uint32Array.BYTES_PER_ELEMENT <=
          wasm.memory.buffer.byteLength &&
        physicalBase + poolBytes <= wasm.memory.buffer.byteLength,
      "execution engine mirror buffer is invalid",
    );
    wasm.vine_reset_dirty_pages();
    wasm.zero_memory(0, poolBytes);
    return { wasm, physicalBase, dirtyPagesPointer };
  }

  #publishAdapter(adapter: PreparedAdapter, poolBytes: number): void {
    const { wasm, physicalBase, dirtyPagesPointer } = adapter;
    // Construct every view before publishing: allocation, permission setup and
    // CPU restoration on a replacement must not damage the live adapter on failure.
    const buffer = wasm.memory.buffer;
    const dirtyPageCandidates = new Uint32Array(buffer, dirtyPagesPointer, dirtyPageCandidateCapacity);
    const physicalMemory = new Uint8Array(buffer, physicalBase, poolBytes);
    const physicalWords = new Uint32Array(buffer, physicalBase, poolBytes / 4);
    const physicalView = new DataView(buffer, physicalBase, poolBytes);
    this.#wasm = wasm;
    this.#dirtyPagesPointer = dirtyPagesPointer;
    this.#physicalBase = physicalBase;
    this.#physicalPoolBytes = poolBytes;
    this.#dirtyPageCandidates = dirtyPageCandidates;
    this.#physicalMemory = physicalMemory;
    this.#physicalWords = physicalWords;
    this.#physicalView = physicalView;
    // Borrowed writeback views expire at the next engine call. An unused cache
    // tail must not retain the previous adapter's entire linear memory.
    this.#borrowedDirtyPages.length = 0;
    this.#borrowedDirtyPagePool.length = 0;
  }

  #capacityRequirement(guestPages: number, pageTables: number): PhysicalCapacityRequirement {
    const bytes = physicalPoolBytesForAllocations(1 + pageTables + guestPages);
    if (
      guestPages > this.#maximumPhysicalPages ||
      bytes > this.#maximumPhysicalPoolBytes
    ) {
      throw physicalCapacityError(
        guestPages,
        this.#maximumPhysicalPages,
        bytes,
        this.#maximumPhysicalPoolBytes,
      );
    }
    return { guestPages, pageTables, bytes };
  }

  #ensurePhysicalCapacity(requirement: PhysicalCapacityRequirement): void {
    if (requirement.bytes <= this.#physicalPoolBytes) {
      return;
    }
    let nextBytes = this.#physicalPoolBytes;
    while (nextBytes < requirement.bytes && nextBytes < this.#maximumPhysicalPoolBytes) {
      nextBytes = Math.min(nextBytes * 2, this.#maximumPhysicalPoolBytes);
    }
    if (nextBytes < requirement.bytes) {
      throw physicalCapacityError(
        requirement.guestPages,
        this.#maximumPhysicalPages,
        requirement.bytes,
        this.#maximumPhysicalPoolBytes,
      );
    }
    this.#reinstantiateAdapter(nextBytes);
  }

  #reinstantiateAdapter(poolBytes: number): void {
    const cpuState = this.#mappings.size === 0 && this.#threadContextTemplate === null
      ? null
      : this.saveState().words;
    const needsDirtyScan = this.#forceFullDirtyScan ||
      this.#wasm.vine_dirty_page_count() !== 0 || this.#wasm.vine_dirty_pages_overflowed() !== 0;
    const adapter = this.#prepareAdapter(poolBytes);
    // Guest physical addresses are offsets within the pool, independent of its
    // Wasm allocation. Copy paging structures and frames together so CR3, PTEs,
    // sparse directories and the allocator's free list keep their exact identity.
    this.#refreshLinearMemoryViews();
    const nextPhysicalMemory = new Uint8Array(adapter.wasm.memory.buffer, adapter.physicalBase, poolBytes);
    // Below half occupancy, skip at least half the old pool's bytes. The fresh
    // pool is already zeroed, including freed frames and the unallocated tail.
    const livePhysicalBytes = (1 + this.#pageTables.size + this.#mappings.size) * memoryPageSize;
    const copySparse = livePhysicalBytes <= this.#physicalPoolBytes / 2;
    const copyPage = (base: number): void => {
      nextPhysicalMemory.set(this.#physicalMemory.subarray(base, base + memoryPageSize), base);
    };
    if (copySparse) {
      copyPage(this.#pageDirectory);
      for (const table of this.#pageTables.values()) copyPage(table);
    } else {
      nextPhysicalMemory.set(this.#physicalMemory);
    }
    for (const mapping of this.#mappings.values()) {
      if (copySparse) copyPage(mapping.physicalBase);
      invariant(
        adapter.wasm.vine_set_page_executable(mapping.address, Number(mapping.protection.execute)) === 0,
        "v86 rejected restored execute permission",
      );
    }
    if (cpuState !== null) {
      new Uint32Array(
        adapter.wasm.memory.buffer,
        adapter.wasm.vine_cpu_state_ptr(),
        cpuState.length,
      ).set(cpuState);
      invariant(adapter.wasm.vine_cpu_state_restore() === 0, "v86 rejected a restored CPU state");
    }
    this.#publishAdapter(adapter, poolBytes);
    // CPU writes set a dirty PTE and enqueue its page together; writeback clears
    // both and flushes the TLB. Preserve an earlier forced scan across repeated
    // growth, but a clean adapter needs no scan just because its pool moved.
    this.#forceFullDirtyScan = needsDirtyScan;
  }

  initialize(reflection: MemoryReflection, prepared: ExecutionBootstrap): void {
    this.#refreshLinearMemoryViews();
    invariant(this.#mappings.size === 0, "v86 mirror is already initialized");
    this.#installReflectedPages(reflection);
    this.#installReservations(prepared.regions);
    this.applyCacheEffects(reflection.cacheEffects);
    this.#initializeCpu(prepared);
    this.#threadContextTemplate = this.saveState().words;
  }

  rebuildMemory(reflection: MemoryReflection, regions: MemoryRegionSnapshot[]): void {
    this.#refreshLinearMemoryViews();
    const state = this.saveState().words;
    this.#mappings.clear();
    this.#reservations.clear();
    this.#pageTables.clear();
    this.#directoryMappingCounts.clear();
    this.#freePhysicalPages.length = 0;
    this.#nextPhysicalPage = memoryPageSize;
    this.#physicalMemory.fill(0);
    this.#wasm.vine_reset_execute_permissions();
    this.#wasm.vine_reset_dirty_pages();
    this.#forceFullDirtyScan = false;
    this.#pageDirectory = this.#allocatePage();
    this.#installReflectedPages(reflection);
    this.#installReservations(regions);
    this.applyCacheEffects(reflection.cacheEffects);
    state[stateWord.controlRegisterBase + 3] = this.#pageDirectory;
    this.#restoreState(state);
    if (this.#threadContextTemplate !== null) {
      this.#threadContextTemplate[stateWord.controlRegisterBase + 3] = this.#pageDirectory;
    }
  }

  executeBudget(instructionBudget: number): ExecutionSlice {
    const before = this.#wasm.vine_instruction_counter();
    const reason = this.#wasm.vine_execute_budget(instructionBudget);
    this.#refreshLinearMemoryViews();
    const errorCode = this.#wasm.vine_get_stop_error_code();
    this.#wasm.vine_cpu_state_save();
    const words = this.#liveStateWords();
    const after = this.#wasm.vine_instruction_counter();
    const state = {
      words,
      eax: words[stateWord.eax] ?? 0,
      esp: words[stateWord.esp] ?? 0,
      instructionPointer: words[stateWord.instructionPointer] ?? 0,
      previousInstructionPointer: words[stateWord.previousInstructionPointer] ?? 0,
    };
    return {
      stop: classifyV86Stop(
        reason,
        errorCode,
        words[stateWord.controlRegisterBase + 2],
      ),
      instructions: (after - before) >>> 0,
      context: this.#executionContext(state),
    };
  }

  saveState(): CpuState {
    this.#wasm.vine_cpu_state_save();
    const words = Uint32Array.from(this.#liveStateWords());
    return {
      words,
      eax: words[stateWord.eax] ?? 0,
      esp: words[stateWord.esp] ?? 0,
      instructionPointer: words[stateWord.instructionPointer] ?? 0,
      previousInstructionPointer: words[stateWord.previousInstructionPointer] ?? 0,
    };
  }

  saveCpuContext(): ExecutionCpuContext {
    return this.#executionContext(this.saveState());
  }

  createCpuContext(bootstrap: ExecutionThreadBootstrap): ExecutionCpuContext {
    invariant(this.#threadContextTemplate !== null, "execution engine is not initialized");
    invariant(
      Number.isInteger(bootstrap.entryPoint) &&
        bootstrap.entryPoint >= 0 &&
        bootstrap.entryPoint <= 0xffff_ffff,
      "thread entry point is outside the u32 guest address space",
    );
    invariant(
      Number.isInteger(bootstrap.initialStackPointer) &&
        bootstrap.initialStackPointer >= 0 &&
        bootstrap.initialStackPointer <= 0xffff_ffff,
      "thread stack pointer is outside the u32 guest address space",
    );
    const fsBase = bootstrap.fsBase ?? 0;
    invariant(
      Number.isInteger(fsBase) && fsBase >= 0 && fsBase <= 0xffff_ffff,
      "thread FS base is outside the u32 guest address space",
    );
    invariant(
      this.#mappingFor(bootstrap.entryPoint).protection.execute,
      "thread entry point is not executable guest memory",
    );
    const stackMapping = this.#mappingFor(bootstrap.initialStackPointer);
    invariant(
      stackMapping.protection.read && stackMapping.protection.write,
      "thread stack pointer is not in readable writable guest memory",
    );
    const words = Uint32Array.from(this.#threadContextTemplate);
    words.fill(0, 2, 10);
    words[stateWord.esp] = bootstrap.initialStackPointer >>> 0;
    words[stateWord.instructionPointer] = bootstrap.entryPoint >>> 0;
    words[stateWord.previousInstructionPointer] = bootstrap.entryPoint >>> 0;
    words[stateWord.flags] = 2;
    words[stateWord.inHalt] = 0;
    words[stateWord.segmentOffsetBase + 4] = fsBase >>> 0;
    return this.#executionContext({
      words,
      eax: words[stateWord.eax] ?? 0,
      esp: words[stateWord.esp] ?? 0,
      instructionPointer: words[stateWord.instructionPointer] ?? 0,
      previousInstructionPointer: words[stateWord.previousInstructionPointer] ?? 0,
    });
  }

  restoreCpuContext(context: ExecutionCpuContext): void {
    invariant(
      context.byteLength === 612 &&
        context.opaqueWords.byteLength === context.byteLength,
      "execution CPU context ABI mismatch",
    );
    this.#restoreState(Uint32Array.from(context.opaqueWords));
  }

  applyCpuResume(completion: CpuResume): void {
    const state = this.saveState().words;
    this.#applyCpuResumeToState(state, completion);
    this.#restoreState(state);
  }

  applyStoppedCpuResume(completion: CpuResume): void {
    const state = this.#liveStateWords();
    invariant(
      state[stateWord.magic] === 0x5649_4e45 &&
        state[stateWord.wordCount] === state.byteLength / Uint32Array.BYTES_PER_ELEMENT,
      "stopped CPU resume requires the current saved v86 state",
    );
    if (completion.eax !== null) {
      invariant(
        Number.isInteger(completion.eax) && completion.eax >= 0 && completion.eax <= 0xffff_ffff,
        "CPU resume EAX must be an unsigned x86 register value or null",
      );
    }
    invariant(
      this.#wasm.vine_cpu_apply_stopped_resume(
        completion.eax ?? 0,
        Number(completion.eax !== null),
        completion.stackPointer >>> 0,
        completion.instructionPointer >>> 0,
      ) === 0,
      "v86 rejected a stopped CPU resume",
    );
  }

  #applyCpuResumeToState(state: Uint32Array, completion: CpuResume): void {
    if (completion.eax !== null) {
      invariant(
        Number.isInteger(completion.eax) && completion.eax >= 0 && completion.eax <= 0xffff_ffff,
        "CPU resume EAX must be an unsigned x86 register value or null",
      );
      state[stateWord.eax] = completion.eax;
    }
    state[stateWord.esp] = completion.stackPointer >>> 0;
    state[stateWord.instructionPointer] = completion.instructionPointer >>> 0;
    state[stateWord.previousInstructionPointer] = completion.instructionPointer >>> 0;
    state[stateWord.inHalt] = 0;
  }

  enterGuestCallback(target: number, stackPointer: number): void {
    const state = this.saveState().words;
    state[stateWord.esp] = stackPointer >>> 0;
    state[stateWord.instructionPointer] = target >>> 0;
    state[stateWord.previousInstructionPointer] = target >>> 0;
    state[stateWord.inHalt] = 0;
    this.#restoreState(state);
  }

  readGuestU32(address: number): number {
    this.#refreshLinearMemoryViews();
    invariant(
      Number.isSafeInteger(address) && address > 0 && address <= 0xffff_fffc,
      "guest mirror u32 read requires a valid x86 range",
    );
    const pageOffset = address & (memoryPageSize - 1);
    if (pageOffset > memoryPageSize - 4) {
      const bytes = this.readGuestBytes(address, 4);
      return (
        bytes[0]! |
        (bytes[1]! << 8) |
        (bytes[2]! << 16) |
        (bytes[3]! << 24)
      ) >>> 0;
    }
    const mapping = this.#mappingFor(address);
    if (!mapping.protection.read) {
      throw new Error(
        `guest mirror read targets unreadable address 0x${address.toString(16)}`,
      );
    }
    const physicalAddress = mapping.physicalBase + pageOffset;
    return this.#physicalView.getUint32(physicalAddress, true);
  }

  readGuestBytes(address: number, byteLength: number, borrow = false): Uint8Array {
    this.#refreshLinearMemoryViews();
    invariant(
      Number.isSafeInteger(address) && address > 0 &&
        Number.isSafeInteger(byteLength) && byteLength > 0,
      "guest mirror byte read requires a nonempty x86 range",
    );
    const end = address + byteLength;
    invariant(
      Number.isSafeInteger(end) && end <= 0x1_0000_0000,
      "guest mirror byte read wrapped the address space",
    );
    let bytes = borrow ? undefined : new Uint8Array(byteLength);
    let physicalStart = 0;
    let offset = 0;
    while (offset < byteLength) {
      const current = address + offset;
      const mapping = this.#mappingFor(current);
      if (!mapping.protection.read) {
        throw new Error(
          `guest mirror read targets unreadable address 0x${current.toString(16)}`,
        );
      }
      const pageOffset = current & (memoryPageSize - 1);
      const physical = mapping.physicalBase + pageOffset;
      if (offset === 0) physicalStart = physical;
      if (bytes === undefined && physical !== physicalStart + offset) {
        // Fragmented frames retain snapshot behavior; never expose unrelated backing.
        bytes = new Uint8Array(byteLength);
        bytes.set(this.#physicalMemory.subarray(physicalStart, physicalStart + offset));
      }
      const chunkLength = Math.min(byteLength - offset, memoryPageSize - pageOffset);
      bytes?.set(
        this.#physicalMemory.subarray(
          physical,
          physical + chunkLength,
        ),
        offset,
      );
      offset += chunkLength;
    }
    return bytes ?? this.#physicalMemory.subarray(physicalStart, physicalStart + byteLength);
  }

  readU32(address: number): number {
    return this.readGuestU32(address);
  }

  collectChangedPages(): DirtyMirrorPage[] {
    return this.#collectChangedPages(true);
  }

  collectChangedPageViews(): DirtyMirrorPage[] {
    return this.#collectChangedPages(false);
  }

  #collectChangedPages(copyBytes: boolean): DirtyMirrorPage[] {
    this.#refreshLinearMemoryViews();
    const dirtyMappings = this.#dirtyMappingsScratch;
    dirtyMappings.length = 0;
    const candidateCount = this.#wasm.vine_dirty_page_count();
    invariant(
      Number.isInteger(candidateCount) &&
        candidateCount >= 0 &&
        candidateCount <= dirtyPageCandidateCapacity,
      "execution engine returned an invalid dirty-page candidate count",
    );
    const scanAll =
      this.#forceFullDirtyScan || this.#wasm.vine_dirty_pages_overflowed() !== 0;
    this.#forceFullDirtyScan = false;
    if (scanAll) {
      // ponytail: rare overflow/restoration scans use the existing page map;
      // maintaining a second index costs work on every texture lock/unlock.
      for (const mapping of this.#mappings.values()) {
        if (!mapping.protection.write) continue;
        const pte = this.#physicalWords[mapping.pteWordIndex]!;
        if ((pte & pageFlags.dirty) !== 0) {
          this.#physicalWords[mapping.pteWordIndex] = pte & ~pageFlags.dirty;
          dirtyMappings.push(mapping);
        }
      }
    } else {
      for (let index = 0; index < candidateCount; index += 1) {
        const address = this.#dirtyPageCandidates[index]!;
        const mapping = this.#mappings.get(address);
        if (mapping === undefined || !mapping.protection.write) {
          continue;
        }
        const pte = this.#physicalWords[mapping.pteWordIndex]!;
        if ((pte & pageFlags.dirty) !== 0) {
          this.#physicalWords[mapping.pteWordIndex] = pte & ~pageFlags.dirty;
          dirtyMappings.push(mapping);
        }
      }
    }
    this.#wasm.vine_reset_dirty_pages();
    dirtyMappings.sort((left, right) => left.address - right.address);
    const dirty = copyBytes ? [] : this.#borrowedDirtyPages;
    dirty.length = 0;
    for (const mapping of dirtyMappings) {
      const bytes = this.#physicalMemory.subarray(
        mapping.physicalBase,
        mapping.physicalBase + memoryPageSize,
      );
      if (copyBytes) {
        dirty.push({
          address: mapping.address,
          owner: mapping.owner,
          bytes: Uint8Array.from(bytes),
        });
      } else {
        const index = dirty.length;
        let page = this.#borrowedDirtyPagePool[index];
        if (page === undefined) {
          page = { address: mapping.address, owner: mapping.owner, bytes };
          this.#borrowedDirtyPagePool.push(page);
        } else {
          page.address = mapping.address;
          page.owner = mapping.owner;
          page.bytes = bytes;
        }
        dirty.push(page);
      }
    }
    if (scanAll) {
      this.#wasm.full_clear_tlb();
    } else {
      for (const mapping of dirtyMappings) {
        invariant(
          this.#wasm.vine_rearm_dirty_page(mapping.address) === 0,
          "v86 rejected dirty-page write tracking",
        );
      }
    }
    return dirty;
  }

  discoverDirtyPages(): DirtyMirrorPage[] {
    return this.collectChangedPages();
  }

  applyMemoryDelta(delta: HostMemoryDelta): void {
    this.#refreshLinearMemoryViews();
    if (delta.mappingOperations.length === 0) {
      if (delta.pages.length === 1) {
        const page = delta.pages[0]!;
        invariant((page.address & (memoryPageSize - 1)) === 0, "host delta page is unaligned");
        invariant(page.bytes.byteLength === memoryPageSize, "host delta page is not 4 KiB");
        const mapping = this.#mappings.get(page.address);
        invariant(mapping !== undefined, "host delta targets an unmapped mirror page");
        invariant(mapping.owner === page.owner, "host delta owner does not match the mirror");
        invariant(
          sameProtection(mapping.protection, page.protection),
          "host delta protection does not match the mirror",
        );
        this.#installPageBytes(page.bytes, mapping);
      } else {
        this.#preflightDeltaPages(delta.pages, this.#mappings);
        for (const page of delta.pages) {
          const mapping = this.#mappings.get(page.address)!;
          this.#installPageBytes(page.bytes, mapping);
        }
      }
      this.applyCacheEffects(delta.cacheEffects);
      return;
    }
    const capacity = this.#preflightHostDelta(delta);
    this.#ensurePhysicalCapacity(capacity);
    for (const operation of delta.mappingOperations) {
      this.#applyMappingOperation(operation);
    }
    for (const page of delta.pages) {
      const mapping = this.#mappings.get(page.address)!;
      this.#installPageBytes(page.bytes, mapping);
    }
    this.applyCacheEffects(delta.cacheEffects);
  }

  #preflightHostDelta(delta: HostMemoryDelta): PhysicalCapacityRequirement {
    if (delta.mappingOperations.length === 0) {
      this.#preflightDeltaPages(delta.pages, this.#mappings);
      return this.#capacityRequirement(this.#mappings.size, this.#pageTables.size);
    }

    invariant(
      this.#directoryMappingCounts.size === this.#pageTables.size,
      "mirror page-table population index is inconsistent",
    );
    const mappingChanges = new Map<number, PlannedMapping | null>();
    const plannedMapping = (address: number): PlannedMapping | undefined => {
      if (mappingChanges.has(address)) {
        return mappingChanges.get(address) ?? undefined;
      }
      return this.#mappings.get(address);
    };
    const reservations = new Map(this.#reservations);
    const directoryMappingCounts = new Map(this.#directoryMappingCounts);
    let mappingCount = this.#mappings.size;
    let capacity = this.#capacityRequirement(
      mappingCount,
      directoryMappingCounts.size,
    );

    const addMapping = (address: number, mapping: PlannedMapping): void => {
      invariant(
        plannedMapping(address) === undefined,
        "commit operation targets a committed mirror page",
      );
      mappingChanges.set(address, mapping);
      mappingCount += 1;
      const directoryIndex = address >>> 22;
      directoryMappingCounts.set(
        directoryIndex,
        (directoryMappingCounts.get(directoryIndex) ?? 0) + 1,
      );
    };
    const removeMapping = (address: number): PlannedMapping => {
      const mapping = plannedMapping(address);
      invariant(
        mapping !== undefined,
        "mapping removal targets an unmapped mirror page",
      );
      mappingChanges.set(address, null);
      mappingCount -= 1;
      const directoryIndex = address >>> 22;
      const directoryMappingCount = directoryMappingCounts.get(directoryIndex);
      invariant(
        directoryMappingCount !== undefined && directoryMappingCount > 0,
        "planned mirror page has no directory population count",
      );
      if (directoryMappingCount === 1) {
        directoryMappingCounts.delete(directoryIndex);
      } else {
        directoryMappingCounts.set(directoryIndex, directoryMappingCount - 1);
      }
      return mapping;
    };

    for (const operation of delta.mappingOperations) {
      switch (operation.operation) {
        case "reserve": {
          invariant(
            ![...reservations.values()].some((reservation) =>
              rangesOverlap(reservation.range, operation.range),
            ),
            "reserve operation overlaps a mirror reservation",
          );
          for (let address = operation.range.base; address < rangeEnd(operation.range); address += memoryPageSize) {
            invariant(
              plannedMapping(address) === undefined,
              "reserve operation overlaps a committed mirror page",
            );
          }
          reservations.set(operation.range.base, {
            ...(operation.addressAllocationId === undefined
              ? {}
              : { addressAllocationId: operation.addressAllocationId }),
            owner: operation.owner,
            kind: operation.kind,
            range: { ...operation.range },
          });
          break;
        }
        case "commit": {
          const overlapping = [...reservations.values()].filter((reservation) =>
            rangesOverlap(reservation.range, operation.range),
          );
          invariant(
            overlapping.length === 0 ||
              overlapping.some(
                (reservation) =>
                  reservation.owner === operation.owner &&
                  reservation.addressAllocationId === operation.addressAllocationId &&
                  rangeContains(reservation.range, operation.range),
              ),
            "commit operation is outside its mirror reservation",
          );
          for (let address = operation.range.base; address < rangeEnd(operation.range); address += memoryPageSize) {
            addMapping(address, {
              ...(operation.addressAllocationId === undefined
                ? {}
                : { addressAllocationId: operation.addressAllocationId }),
              owner: operation.owner,
              protection: { ...operation.protection },
            });
          }
          break;
        }
        case "protect":
          for (let address = operation.range.base; address < rangeEnd(operation.range); address += memoryPageSize) {
            const mapping = plannedMapping(address);
            invariant(mapping !== undefined, "protect operation targets an unmapped mirror page");
            invariant(mapping.owner === operation.owner, "protect operation owner mismatch");
            invariant(
              mapping.addressAllocationId === operation.addressAllocationId,
              "protect operation address allocation mismatch",
            );
            mappingChanges.set(address, {
              ...(mapping.addressAllocationId === undefined
                ? {}
                : { addressAllocationId: mapping.addressAllocationId }),
              owner: mapping.owner,
              protection: { ...operation.protection },
            });
          }
          break;
        case "decommit":
          for (let address = operation.range.base; address < rangeEnd(operation.range); address += memoryPageSize) {
            const mapping = plannedMapping(address);
            invariant(
              mapping !== undefined,
              "decommit operation targets an unmapped mirror page",
            );
            invariant(mapping.owner === operation.owner, "decommit operation owner mismatch");
            invariant(
              mapping.addressAllocationId === operation.addressAllocationId,
              "decommit operation address allocation mismatch",
            );
            removeMapping(address);
          }
          break;
        case "release": {
          const overlapping = [...reservations.values()].filter((reservation) =>
            rangesOverlap(reservation.range, operation.range),
          );
          const reservation = reservations.get(operation.range.base);
          if (reservation !== undefined) {
            invariant(
              reservation.owner === operation.owner &&
                reservation.addressAllocationId === operation.addressAllocationId &&
                reservation.range.byteLength === operation.range.byteLength,
              "release operation does not match its mirror reservation",
            );
            reservations.delete(operation.range.base);
          } else {
            invariant(overlapping.length === 0, "release operation partially overlaps a reservation");
          }
          let matched = reservation !== undefined;
          for (let address = operation.range.base; address < rangeEnd(operation.range); address += memoryPageSize) {
            const mapping = plannedMapping(address);
            if (mapping === undefined) {
              continue;
            }
            invariant(mapping.owner === operation.owner, "release operation owner mismatch");
            invariant(
              mapping.addressAllocationId === operation.addressAllocationId,
              "release operation address allocation mismatch",
            );
            removeMapping(address);
            matched = true;
          }
          invariant(matched, "release operation targets an unknown mirror range");
          break;
        }
      }
      const nextCapacity = this.#capacityRequirement(
        mappingCount,
        directoryMappingCounts.size,
      );
      if (nextCapacity.bytes > capacity.bytes) {
        capacity = nextCapacity;
      }
    }

    this.#preflightDeltaPagesWithLookup(delta.pages, plannedMapping);
    return capacity;
  }

  #preflightDeltaPages(
    pages: HostMemoryDelta["pages"],
    mappings: ReadonlyMap<number, PlannedMapping>,
  ): void {
    this.#preflightDeltaPagesWithLookup(pages, (address) => mappings.get(address));
  }

  #preflightDeltaPagesWithLookup(
    pages: HostMemoryDelta["pages"],
    mappingForAddress: (address: number) => PlannedMapping | undefined,
  ): void {
    const dirtyAddresses = new Set<number>();
    for (const page of pages) {
      invariant((page.address & (memoryPageSize - 1)) === 0, "host delta page is unaligned");
      invariant(page.bytes.byteLength === memoryPageSize, "host delta page is not 4 KiB");
      invariant(!dirtyAddresses.has(page.address), "host delta repeats a dirty page");
      dirtyAddresses.add(page.address);
      const mapping = mappingForAddress(page.address);
      invariant(mapping !== undefined, "host delta targets an unmapped mirror page");
      invariant(mapping.owner === page.owner, "host delta owner does not match the mirror");
      invariant(
        sameProtection(mapping.protection, page.protection),
        "host delta protection does not match the mirror",
      );
    }
  }

  #applyMappingOperation(operation: MappingOperation): void {
    switch (operation.operation) {
      case "reserve":
        this.#reservations.set(operation.range.base, {
          ...(operation.addressAllocationId === undefined
            ? {}
            : { addressAllocationId: operation.addressAllocationId }),
          owner: operation.owner,
          kind: operation.kind,
          range: { ...operation.range },
        });
        break;
      case "commit":
        for (let address = operation.range.base; address < rangeEnd(operation.range); address += memoryPageSize) {
          this.#mapPage(
            address,
            operation.owner,
            operation.protection,
            operation.addressAllocationId,
          );
        }
        break;
      case "protect":
        for (let address = operation.range.base; address < rangeEnd(operation.range); address += memoryPageSize) {
          this.#updateProtection(address, operation.protection);
        }
        break;
      case "decommit":
        for (let address = operation.range.base; address < rangeEnd(operation.range); address += memoryPageSize) {
          this.#unmapPage(address);
        }
        break;
      case "release":
        for (const address of [...this.#mappings.keys()]) {
          if (address >= operation.range.base && address < rangeEnd(operation.range)) {
            this.#unmapPage(address);
          }
        }
        this.#reservations.delete(operation.range.base);
        break;
    }
  }

  applyCacheEffects(effects: CacheEffects): void {
    if (effects.flushTlb || effects.invalidateInstructionFetch) {
      this.#wasm.full_clear_tlb();
    }
    if (effects.invalidateGeneratedCode) {
      this.#wasm.jit_clear_cache_js();
    }
  }

  #installReflectedPages(reflection: MemoryReflection): void {
    const directoryIndices = new Set(this.#pageTables.keys());
    const addresses = new Set<number>();
    for (const page of reflection.pages) {
      invariant((page.address & (memoryPageSize - 1)) === 0, "unaligned reflected page");
      invariant(page.bytes.byteLength === memoryPageSize, "invalid reflected page length");
      invariant(!addresses.has(page.address), "reflection repeats a guest page");
      addresses.add(page.address);
      directoryIndices.add(page.address >>> 22);
    }
    this.#ensurePhysicalCapacity(
      this.#capacityRequirement(
        this.#mappings.size + reflection.pages.length,
        directoryIndices.size,
      ),
    );
    for (const page of reflection.pages) {
      this.#mapPage(page.address, page.owner, page.protection);
      const mapping = this.#mappings.get(page.address)!;
      this.#installPageBytes(page.bytes, mapping);
    }
  }

  #installReservations(regions: MemoryRegionSnapshot[]): void {
    for (const region of regions) {
      const range = { base: region.range.start, byteLength: region.range.byteLength };
      invariant(region.owner !== 0, "reflected region owner is zero");
      invariant((range.base & (memoryPageSize - 1)) === 0, "reflected region is unaligned");
      invariant(
        range.byteLength > 0 && range.byteLength % memoryPageSize === 0,
        "reflected region length is invalid",
      );
      invariant(rangeEnd(range) <= 0x1_0000_0000, "reflected region crosses x86 memory");
      invariant(
        ![...this.#reservations.values()].some((reservation) =>
          rangesOverlap(reservation.range, range),
        ),
        "reflected regions overlap",
      );
      this.#reservations.set(range.base, {
        ...(region.addressAllocationId === undefined
          ? {}
          : { addressAllocationId: region.addressAllocationId }),
        owner: region.owner,
        kind: region.kind,
        range,
      });
    }
    for (const mapping of this.#mappings.values()) {
      const page = { base: mapping.address, byteLength: memoryPageSize };
      const reservation = [...this.#reservations.values()].find(
          (reservation) =>
            reservation.owner === mapping.owner && rangeContains(reservation.range, page),
      );
      invariant(reservation !== undefined, "reflected page is outside its owner region");
      mapping.addressAllocationId = reservation.addressAllocationId;
    }
  }

  #mapPage(
    address: number,
    owner: number,
    protection: MemoryProtection,
    addressAllocationId?: number,
  ): void {
    const table = this.#ensurePageTable(address);
    const pteAddress = table + ((address >>> 12) & 0x3ff) * 4;
    const physicalBase = this.#allocatePage();
    this.#physicalView.setUint32(
      pteAddress,
      physicalBase |
        pageFlags.present |
        pageFlags.user |
        (protection.write ? pageFlags.writable : 0),
      true,
    );
    const mapping = {
      address,
      ...(addressAllocationId === undefined ? {} : { addressAllocationId }),
      owner,
      protection: { ...protection },
      physicalBase,
      pteAddress,
      pteWordIndex: pteAddress >>> 2,
    };
    this.#mappings.set(address, mapping);
    const directoryIndex = address >>> 22;
    this.#directoryMappingCounts.set(
      directoryIndex,
      (this.#directoryMappingCounts.get(directoryIndex) ?? 0) + 1,
    );
    invariant(
      this.#wasm.vine_set_page_executable(address, Number(protection.execute)) === 0,
      "v86 rejected mapped execute permission",
    );
  }

  #installPageBytes(bytes: Uint8Array, mapping: MirrorMapping): void {
    this.#physicalMemory.set(bytes, mapping.physicalBase);
  }

  #updateProtection(address: number, protection: MemoryProtection): void {
    const mapping = this.#mappings.get(address)!;
    const dirty = this.#physicalView.getUint32(mapping.pteAddress, true) & pageFlags.dirty;
    invariant(
      protection.write || dirty === 0,
      "mirror write protection changed while the CPU page was still dirty",
    );
    this.#physicalView.setUint32(
      mapping.pteAddress,
      mapping.physicalBase |
        pageFlags.present |
        pageFlags.user |
        (protection.write ? pageFlags.writable : 0) |
        dirty,
      true,
    );
    mapping.protection = { ...protection };
    invariant(
      this.#wasm.vine_set_page_executable(address, Number(protection.execute)) === 0,
      "v86 rejected protected execute permission",
    );
  }

  #unmapPage(address: number): void {
    const mapping = this.#mappings.get(address)!;
    this.#physicalView.setUint32(mapping.pteAddress, 0, true);
    invariant(
      this.#wasm.vine_set_page_executable(address, 0) === 0,
      "v86 rejected unmapped execute permission",
    );
    this.#mappings.delete(address);
    this.#releasePage(mapping.physicalBase);
    const directoryIndex = address >>> 22;
    const directoryMappingCount = this.#directoryMappingCounts.get(directoryIndex);
    invariant(
      directoryMappingCount !== undefined && directoryMappingCount > 0,
      "mapped mirror page has no directory population count",
    );
    if (directoryMappingCount === 1) {
      this.#directoryMappingCounts.delete(directoryIndex);
      const table = this.#pageTables.get(directoryIndex);
      invariant(table !== undefined, "empty mirror directory has no page table");
      this.#physicalView.setUint32(this.#pageDirectory + directoryIndex * 4, 0, true);
      this.#pageTables.delete(directoryIndex);
      this.#releasePage(table);
    } else {
      this.#directoryMappingCounts.set(directoryIndex, directoryMappingCount - 1);
    }
  }

  #initializeCpu(prepared: ExecutionBootstrap): void {
    this.#wasm.reset_cpu();
    invariant(this.#wasm.vine_cpu_state_size() === 612, "unexpected v86 CPU state size");
    this.#wasm.vine_cpu_state_save();
    const words = this.#liveStateWords();
    invariant(words[stateWord.magic] === 0x5649_4e45, "invalid v86 CPU state magic");
    invariant(words[stateWord.wordCount] === 153, "invalid CPU state header");
    words.fill(0, 2, 10);
    words[stateWord.esp] = prepared.initialStackPointer >>> 0;
    words[stateWord.instructionPointer] = prepared.entryPoint >>> 0;
    words[stateWord.previousInstructionPointer] = prepared.entryPoint >>> 0;
    words[stateWord.flags] = 2;
    words[stateWord.cpl] = 3;
    words[stateWord.protectedMode] = 1;
    words[stateWord.operandSize32] = 1;
    words[stateWord.stackSize32] = 1;
    words[stateWord.inHalt] = 0;
    for (let index = 0; index < 8; index += 1) {
      words[stateWord.segmentSelectorBase + index] = 0;
      words[stateWord.segmentOffsetBase + index] = 0;
      words[stateWord.segmentLimitBase + index] = 0xffff_ffff;
      words[stateWord.segmentMetaBase + index] = 0xf2;
    }
    words[stateWord.segmentOffsetBase + 4] = (prepared.fsBase ?? 0) >>> 0;
    words[stateWord.segmentMetaBase + 1] = 0xfa;
    words[stateWord.controlRegisterBase] =
      (words[stateWord.controlRegisterBase] ?? 0) | 0x8001_0001;
    words[stateWord.controlRegisterBase + 3] = this.#pageDirectory;
    words[stateWord.controlRegisterBase + 4] =
      (words[stateWord.controlRegisterBase + 4] ?? 0) | 0x200;
    invariant(this.#wasm.vine_cpu_state_restore() === 0, "v86 rejected the initial CPU state");
  }

  #liveStateWords(): Uint32Array {
    return new Uint32Array(
      this.#wasm.memory.buffer,
      this.#wasm.vine_cpu_state_ptr(),
      this.#wasm.vine_cpu_state_size() / 4,
    );
  }

  #refreshLinearMemoryViews(): void {
    const buffer = this.#wasm.memory.buffer;
    if (this.#physicalMemory?.buffer === buffer) {
      return;
    }
    invariant(
      this.#dirtyPagesPointer +
        dirtyPageCandidateCapacity * Uint32Array.BYTES_PER_ELEMENT <=
        buffer.byteLength &&
        this.#physicalBase + this.#physicalPoolBytes <= buffer.byteLength,
      "execution engine memory growth invalidated its mirror ranges",
    );
    this.#dirtyPageCandidates = new Uint32Array(
      buffer,
      this.#dirtyPagesPointer,
      dirtyPageCandidateCapacity,
    );
    this.#physicalMemory = new Uint8Array(
      buffer,
      this.#physicalBase,
      this.#physicalPoolBytes,
    );
    this.#physicalWords = new Uint32Array(
      buffer,
      this.#physicalBase,
      this.#physicalPoolBytes / Uint32Array.BYTES_PER_ELEMENT,
    );
    this.#physicalView = new DataView(
      buffer,
      this.#physicalBase,
      this.#physicalPoolBytes,
    );
  }

  #restoreState(words: Uint32Array): void {
    this.#liveStateWords().set(words);
    invariant(this.#wasm.vine_cpu_state_restore() === 0, "v86 rejected a resumed CPU state");
  }

  #mappingFor(address: number): MirrorMapping {
    const pageAddress = (address & ~(memoryPageSize - 1)) >>> 0;
    const mapping = this.#mappings.get(pageAddress);
    if (mapping === undefined) {
      throw new Error(
        `guest address 0x${address.toString(16)} is unmapped`,
      );
    }
    return mapping;
  }

  #executionContext(state: CpuState): ExecutionCpuContext {
    return {
      byteLength: 612,
      opaqueWords: state.words,
      eax: state.eax,
      ecx: state.words[stateWord.ecx] ?? 0,
      edx: state.words[stateWord.edx] ?? 0,
      ebx: state.words[stateWord.ebx] ?? 0,
      esp: state.esp,
      ebp: state.words[stateWord.ebp] ?? 0,
      esi: state.words[stateWord.esi] ?? 0,
      edi: state.words[stateWord.edi] ?? 0,
      instructionPointer: state.instructionPointer,
      previousInstructionPointer: state.previousInstructionPointer,
      eflags: state.words[stateWord.flags] ?? 0,
      es: state.words[stateWord.segmentSelectorBase] ?? 0,
      cs: state.words[stateWord.segmentSelectorBase + 1] ?? 0,
      ss: state.words[stateWord.segmentSelectorBase + 2] ?? 0,
      ds: state.words[stateWord.segmentSelectorBase + 3] ?? 0,
      fs: state.words[stateWord.segmentSelectorBase + 4] ?? 0,
      gs: state.words[stateWord.segmentSelectorBase + 5] ?? 0,
    };
  }

  #ensurePageTable(address: number): number {
    const directoryIndex = address >>> 22;
    const existing = this.#pageTables.get(directoryIndex);
    if (existing !== undefined) {
      return existing;
    }
    const table = this.#allocatePage();
    this.#pageTables.set(directoryIndex, table);
    this.#physicalView.setUint32(
      this.#pageDirectory + directoryIndex * 4,
      table | pageFlags.present | pageFlags.writable | pageFlags.user,
      true,
    );
    return table;
  }

  #allocatePage(): number {
    const reused = this.#freePhysicalPages.pop();
    if (reused !== undefined) {
        this.#physicalMemory.fill(0, reused, reused + memoryPageSize);
      return reused;
    }
    for (const excluded of excludedPhysicalRanges) {
      if (this.#nextPhysicalPage >= excluded.start && this.#nextPhysicalPage < excluded.end) {
        this.#nextPhysicalPage = excluded.end;
      }
    }
    if (this.#nextPhysicalPage + memoryPageSize >
        this.#physicalPoolBytes) {
      throw physicalCapacityError(
        this.#mappings.size + 1,
        this.#maximumPhysicalPages,
        this.#nextPhysicalPage + memoryPageSize,
        this.#maximumPhysicalPoolBytes,
      );
    }
    const address = this.#nextPhysicalPage;
    this.#nextPhysicalPage += memoryPageSize;
    this.#physicalMemory.fill(0, address, address + memoryPageSize);
    return address;
  }

  #releasePage(address: number): void {
    this.#physicalMemory.fill(0, address, address + memoryPageSize);
    this.#freePhysicalPages.push(address);
  }
}
