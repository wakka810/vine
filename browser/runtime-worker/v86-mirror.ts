import engineProfile from "../../fixtures/execution-engines/v86-browser-v1.json";
import {
  ExecutionMemoryAccessFault,
  type CpuRegisterUpdate,
  type CpuResume,
  type ExecutionBootstrap,
  type ExecutionClockSample,
  type ExecutionCpuContext,
  type ExecutionEngine,
  type ExecutionEngineIdentity,
  type ExecutionEngineMemoryCapacity,
  type ExecutionEnginePerformanceMetrics,
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

const defaultMaximumPhysicalPages = 16 * 1024;
const dirtyPageCandidateCapacity = 16 * 1024;
const jitTableOffset = 1_024;
const jitTableSize = 900;
const initialPhysicalPoolBytes = 64 * 1024;
// The BGI runtime currently permits a bounded 1.5 GiB live guest set. Leave
// room for its page directory/tables while retaining an explicit mirror cap.
const hardMaximumPhysicalPoolBytes = 1600 * 1024 * 1024;
const excludedPhysicalRanges = [{ start: 0x000a_0000, end: 0x000c_0000 }] as const;
const pageFlags = { present: 1, writable: 2, user: 4, dirty: 0x40 } as const;
const adapterAbiVersion = 1;
const stateWord = {
  magic: 0,
  version: 1,
  wordCount: 2,
  eax: 4,
  ecx: 5,
  edx: 6,
  ebx: 7,
  esp: 8,
  ebp: 9,
  esi: 10,
  edi: 11,
  instructionPointer: 12,
  previousInstructionPointer: 13,
  flags: 14,
  cpl: 20,
  protectedMode: 21,
  operandSize32: 22,
  stackSize32: 23,
  inHalt: 24,
  segmentSelectorBase: 28,
  segmentOffsetBase: 36,
  segmentLimitBase: 44,
  segmentMetaBase: 52,
  controlRegisterBase: 60,
} as const;

export const v86StopReason = {
  budget: 1,
  halt: 2,
  executeProtection: 3,
  runtimeDispatch: 4,
  executionIslandError: 6,
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
  set_jit_config(index: number, value: number): void;
  get_jit_config(index: number): number;
  codegen_finalize_finished(index: number, start: number, stateFlags: number): void;
  vine_cpu_state_ptr(): number;
  vine_cpu_state_size(): number;
  vine_cpu_state_version(): number;
  vine_cpu_state_save(): void;
  vine_cpu_state_restore(): number;
  vine_adapter_abi_version(): number;
  vine_dirty_page_tracking_version(): number;
  vine_dirty_pages_ptr(): number;
  vine_dirty_page_count(): number;
  vine_dirty_pages_overflowed(): number;
  vine_reset_dirty_pages(): void;
  vine_execute_budget(instructions: number): number;
  vine_get_stop_error_code(): number;
  vine_instruction_counter(): number;
  vine_jit_retired_instructions_low(): number;
  vine_jit_retired_instructions_high(): number;
  vine_interpreted_retired_instructions_low(): number;
  vine_interpreted_retired_instructions_high(): number;
  vine_reset_execute_permissions(): void;
  vine_set_memory_size(bytes: number): number;
  vine_set_page_executable(pageAddress: number, executable: number): number;
}

interface V86IslandExports {
  vine_execution_island_heap_pages_count?(): number;
  vine_execution_island_heap_pages_ptr?(): number;
  vine_execution_island_enable_direct_memory(poolBytes: number): number;
  vine_execution_island_page_offset(address: number, owner: number, protection: number): number;
  vine_execution_island_abi_version(): number;
  vine_execution_island_execute_budget(
    instructions: number,
    retiredInstructionsLow: number,
    retiredInstructionsHigh: number,
    hostElapsedTime100nsLow: number,
    hostElapsedTime100nsHigh: number,
  ): number;
  vine_execution_island_fast_api_count(): number;
  vine_execution_island_writeback_batches(): number;
  vine_execution_island_writeback_pages(): number;
  vine_execution_island_mirrored_delta_pages(): number;
  vine_execution_island_generation_low(): number;
  vine_execution_island_generation_high(): number;
  vine_bridge_metadata_ptr(): number;
  vine_bridge_metadata_len(): number;
}

interface V86JitMetrics {
  compilations: number;
  clears: number;
  activeFunctions: number;
}

const jitMetricsByInstance = new WeakMap<V86Exports, V86JitMetrics>();

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
  physicalBase?: number;
}

interface PreparedV86Adapter {
  module: WebAssembly.Module;
  identity: ExecutionEngineIdentity;
}

export interface V86MirrorOptions {
  maximumPhysicalPages?: ExecutionEngineOptions["maximumPhysicalPages"];
  initialPhysicalPoolBytes?: number;
  disableJit?: boolean;
  relaxJitInstructionBudget?: boolean;
  jitHotnessThreshold?: number;
}

export type V86MirrorFactory = (options?: V86MirrorOptions) => V86Mirror;

export interface V86ExecutionIsland {
  runtimeExports: WebAssembly.Exports;
  createEngine(options: V86MirrorOptions): V86Mirror;
}

interface PhysicalCapacityRequirement {
  guestPages: number;
  pageTables: number;
  bytes: number;
  directMappings?: Map<number, PlannedMapping | null>;
}

interface MirrorPageSnapshot {
  address: number;
  addressAllocationId?: number;
  owner: number;
  protection: MemoryProtection;
  bytes: Uint8Array;
  dirty: boolean;
}

export class V86PhysicalPoolExhausted extends Error {
  readonly code = "v86-physical-pool-exhausted";
  readonly requestedPhysicalPages: number;
  readonly maximumPhysicalPages: number;
  readonly requiredPhysicalPoolBytes: number;
  readonly maximumPhysicalPoolBytes: number;

  constructor(
    requestedPhysicalPages: number,
    maximumPhysicalPages: number,
    requiredPhysicalPoolBytes: number,
    maximumPhysicalPoolBytes: number,
  ) {
    super(
      `v86-physical-pool-exhausted: ${requestedPhysicalPages} guest pages require ` +
        `${requiredPhysicalPoolBytes} mirror bytes; limits are ${maximumPhysicalPages} pages ` +
        `and ${maximumPhysicalPoolBytes} bytes`,
    );
    this.name = "V86PhysicalPoolExhausted";
    this.requestedPhysicalPages = requestedPhysicalPages;
    this.maximumPhysicalPages = maximumPhysicalPages;
    this.requiredPhysicalPoolBytes = requiredPhysicalPoolBytes;
    this.maximumPhysicalPoolBytes = maximumPhysicalPoolBytes;
  }
}

export interface CpuState {
  words: Uint32Array;
  eax: number;
  esp: number;
  instructionPointer: number;
  previousInstructionPointer: number;
}

export interface SliceResult {
  reason: number;
  errorCode: number;
  instructions: number;
  state: CpuState;
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

function alignPoolBytes(value: number): number {
  invariant(Number.isSafeInteger(value) && value > 0, "physical pool byte size is invalid");
  return Math.ceil(value / memoryPageSize) * memoryPageSize;
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

const directPageTableBytes = physicalPoolBytesForAllocations(1 + 1024);

interface V86ArtifactProfile {
  hostProfile: string;
  bytes: number;
  sha256: string;
}

interface V86ExecutionEngineProfile {
  schema: "vine.execution-engine-profile";
  schemaVersion: 1;
  engine: "v86";
  sourceRevision: string;
  cpuContextAbi: { version: number; bytes: number };
  artifacts: V86ArtifactProfile[];
}

function executionEngineProfile(value: unknown): V86ExecutionEngineProfile {
  invariant(value !== null && typeof value === "object", "execution engine profile is invalid");
  const profile = value as Record<string, unknown>;
  invariant(profile.schema === "vine.execution-engine-profile", "execution engine schema mismatch");
  invariant(profile.schemaVersion === 1, "execution engine profile version mismatch");
  invariant(profile.engine === "v86", "unsupported execution engine profile");
  invariant(
    typeof profile.sourceRevision === "string" && /^[0-9a-f]{40}$/u.test(profile.sourceRevision),
    "execution engine source revision is invalid",
  );
  const cpuContextAbi = profile.cpuContextAbi as Record<string, unknown> | undefined;
  invariant(
    cpuContextAbi?.version === 1 && cpuContextAbi.bytes === 640,
    "execution engine CPU context ABI profile mismatch",
  );
  invariant(Array.isArray(profile.artifacts) && profile.artifacts.length > 0, "execution engine artifacts are missing");
  const hostProfiles = new Set<string>();
  const artifactIdentities = new Set<string>();
  for (const artifact of profile.artifacts as Array<Record<string, unknown>>) {
    invariant(
      typeof artifact.hostProfile === "string" && artifact.hostProfile.length > 0,
      "execution engine host profile is invalid",
    );
    invariant(!hostProfiles.has(artifact.hostProfile), "execution engine host profile is duplicated");
    hostProfiles.add(artifact.hostProfile);
    invariant(
      Number.isSafeInteger(artifact.bytes) && Number(artifact.bytes) > 0,
      "execution engine artifact size is invalid",
    );
    invariant(
      typeof artifact.sha256 === "string" && /^[0-9a-f]{64}$/u.test(artifact.sha256),
      "execution engine artifact digest is invalid",
    );
    const artifactIdentity = `${String(artifact.bytes)}/${artifact.sha256}`;
    invariant(
      !artifactIdentities.has(artifactIdentity),
      "execution engine artifact identity is duplicated",
    );
    artifactIdentities.add(artifactIdentity);
  }
  return structuredClone(value) as V86ExecutionEngineProfile;
}

const supportedProfile = executionEngineProfile(engineProfile);

function identityForArtifact(
  profile: V86ExecutionEngineProfile,
  artifact: V86ArtifactProfile,
): ExecutionEngineIdentity {
  return {
    schema: profile.schema,
    schemaVersion: profile.schemaVersion,
    engine: profile.engine,
    sourceRevision: profile.sourceRevision,
    cpuContextAbi: structuredClone(profile.cpuContextAbi),
    artifact: { bytes: artifact.bytes, sha256: artifact.sha256 },
  };
}

function sameExecutionEngineIdentity(
  left: ExecutionEngineIdentity,
  right: ExecutionEngineIdentity,
): boolean {
  return (
    left.schema === right.schema &&
    left.schemaVersion === right.schemaVersion &&
    left.engine === right.engine &&
    left.sourceRevision === right.sourceRevision &&
    left.cpuContextAbi.version === right.cpuContextAbi.version &&
    left.cpuContextAbi.bytes === right.cpuContextAbi.bytes &&
    left.artifact.bytes === right.artifact.bytes &&
    left.artifact.sha256 === right.artifact.sha256
  );
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const owned = new Uint8Array(bytes.byteLength);
  owned.set(bytes);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", owned.buffer));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
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

function pageAddresses(range: MemoryRange): number[] {
  const addresses: number[] = [];
  for (let address = range.base; address < rangeEnd(range); address += memoryPageSize) {
    addresses.push(address);
  }
  return addresses;
}

export class V86Mirror implements ExecutionEngine {
  static readonly #preparedUrls = new Map<string, Promise<PreparedV86Adapter>>();
  readonly #module: WebAssembly.Module;
  #wasm!: V86Exports;
  readonly #identity: ExecutionEngineIdentity;
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
  readonly #disableJit: boolean;
  readonly #relaxJitInstructionBudget: boolean;
  readonly #jitHotnessThreshold: number | undefined;
  readonly #sharedRuntime: boolean;
  readonly #directMemory: boolean;
  readonly #initialPhysicalPoolBytes: number;
  readonly #maximumPhysicalPoolBytes: number;
  #physicalPoolBytes: number;
  #pageDirectory = 0;
  #nextPhysicalPage = memoryPageSize;
  readonly #freePhysicalPages: number[] = [];
  readonly #pageTables = new Map<number, number>();
  #physicalPoolGrowths = 0;
  #releasedPages = 0;
  #reusedPages = 0;
  #forceFullDirtyScan = false;
  #threadContextTemplate: Uint32Array | null = null;
  readonly #cacheEffectCounts = {
    tlbFlushes: 0,
    instructionFetchInvalidations: 0,
    generatedCodeInvalidations: 0,
  };

  private constructor(
    module: WebAssembly.Module,
    identity: ExecutionEngineIdentity,
    options: V86MirrorOptions = {},
    existingWasm?: V86Exports,
  ) {
    this.#module = module;
    this.#identity = structuredClone(identity);
    this.#disableJit = options.disableJit ?? false;
    this.#relaxJitInstructionBudget =
      options.relaxJitInstructionBudget ?? false;
    this.#jitHotnessThreshold = options.jitHotnessThreshold;
    this.#sharedRuntime = existingWasm !== undefined;
    this.#directMemory = this.#sharedRuntime &&
      typeof existingWasm?.vine_execution_island_enable_direct_memory === "function";
    this.#maximumPhysicalPages = requirePageCount(
      options.maximumPhysicalPages ?? defaultMaximumPhysicalPages,
      "maximum physical page count",
    );
    this.#maximumPhysicalPoolBytes = this.#directMemory
      ? directPageTableBytes + this.#maximumPhysicalPages * memoryPageSize
      : maximumPhysicalPoolBytes(this.#maximumPhysicalPages);
    invariant(
      this.#maximumPhysicalPoolBytes <= hardMaximumPhysicalPoolBytes,
      `configured physical page limit requires ${this.#maximumPhysicalPoolBytes} bytes, ` +
        `above the hard ${hardMaximumPhysicalPoolBytes}-byte mirror limit`,
    );
    const requestedInitialBytes = this.#sharedRuntime
      ? this.#maximumPhysicalPoolBytes
      : alignPoolBytes(options.initialPhysicalPoolBytes ?? initialPhysicalPoolBytes);
    this.#initialPhysicalPoolBytes = Math.min(
      Math.max(requestedInitialBytes, physicalPoolBytesForAllocations(1)),
      this.#maximumPhysicalPoolBytes,
    );
    this.#physicalPoolBytes = this.#initialPhysicalPoolBytes;
    this.#installAdapter(
      existingWasm ?? V86Mirror.#instantiateAdapter(this.#module),
      this.#physicalPoolBytes,
    );
  }

  static async prepare(url: URL): Promise<V86MirrorFactory> {
    const key = url.href;
    let prepared = V86Mirror.#preparedUrls.get(key);
    if (prepared === undefined) {
      prepared = (async () => {
        const response = await fetch(url);
        invariant(response.ok, `v86 adapter fetch failed with ${response.status}`);
        return V86Mirror.#prepareBytes(
          new Uint8Array(await response.arrayBuffer()),
        );
      })();
      V86Mirror.#preparedUrls.set(key, prepared);
    }
    const resolved = await prepared;
    return (options = {}) => V86Mirror.#instantiatePrepared(resolved, options);
  }

  static async instantiate(url: URL, options: V86MirrorOptions = {}): Promise<V86Mirror> {
    return (await V86Mirror.prepare(url))(options);
  }

  static async instantiateBytes(
    bytes: Uint8Array,
    identity?: ExecutionEngineIdentity,
    options: V86MirrorOptions = {},
  ): Promise<V86Mirror> {
    return V86Mirror.#instantiatePrepared(
      await V86Mirror.#prepareBytes(bytes, identity),
      options,
    );
  }

  static async prepareExecutionIsland(url: URL): Promise<V86ExecutionIsland> {
    const key = url.href;
    let prepared = V86Mirror.#preparedUrls.get(key);
    if (prepared === undefined) {
      prepared = (async () => {
        const response = await fetch(url);
        invariant(response.ok, `v86 execution island fetch failed with ${response.status}`);
        return V86Mirror.#prepareBytes(new Uint8Array(await response.arrayBuffer()));
      })();
      V86Mirror.#preparedUrls.set(key, prepared);
    }
    const resolved = await prepared;
    const wasm = V86Mirror.#instantiateAdapter(resolved.module, true);
    const islandWasm = wasm as V86Exports & Partial<V86IslandExports>;
    invariant(
      islandWasm.vine_execution_island_abi_version?.() === 1,
      "execution island ABI mismatch",
    );
    let claimed = false;
    return {
      runtimeExports: wasm,
      createEngine(options) {
        invariant(!claimed, "execution island engine was created more than once");
        claimed = true;
        return new V86Mirror(resolved.module, resolved.identity, options, wasm);
      },
    };
  }

  static async #prepareBytes(
    bytes: Uint8Array,
    expectedIdentity?: ExecutionEngineIdentity,
  ): Promise<PreparedV86Adapter> {
    const digest = await sha256(bytes);
    const artifact = supportedProfile.artifacts.find(
      (candidate) => candidate.bytes === bytes.byteLength && candidate.sha256 === digest,
    );
    invariant(artifact !== undefined, "execution engine artifact identity mismatch");
    const identity = identityForArtifact(supportedProfile, artifact);
    if (expectedIdentity !== undefined) {
      invariant(
        sameExecutionEngineIdentity(expectedIdentity, identity),
        "execution engine source revision, ABI, or artifact profile mismatch",
      );
    }
    const ownedBytes = new Uint8Array(bytes.byteLength);
    ownedBytes.set(bytes);
    const module = await WebAssembly.compile(ownedBytes.buffer);
    return { module, identity };
  }

  static #instantiatePrepared(
    prepared: PreparedV86Adapter,
    options: V86MirrorOptions,
  ): V86Mirror {
    return new V86Mirror(prepared.module, prepared.identity, options);
  }

  static #instantiateAdapter(
    module: WebAssembly.Module,
    hostMonotonicClock = false,
  ): V86Exports {
    const imports = WebAssembly.Module.imports(module);
    const table = new WebAssembly.Table({
      element: "anyfunc",
      initial: jitTableOffset + jitTableSize,
    });
    let wasm: V86Exports | undefined;
    let jitImports: Record<string, WebAssembly.ImportValue> | undefined;
    const jitMetrics: V86JitMetrics = {
      compilations: 0,
      clears: 0,
      activeFunctions: 0,
    };
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
          return hostMonotonicClock ? performance.now() : 0;
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
          jitMetrics.compilations += 1;
          if (table.get(index + jitTableOffset) === null) {
            jitMetrics.activeFunctions += 1;
          }
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
          if (table.get(index + jitTableOffset) !== null) {
            jitMetrics.activeFunctions -= 1;
          }
          jitMetrics.clears += 1;
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
    jitMetricsByInstance.set(wasm, jitMetrics);
    jitImports = { m: wasm.memory };
    for (const [name, value] of Object.entries(wasm)) {
      if (!name.startsWith("_") && !name.startsWith("zstd") && !name.endsWith("_js")) {
        jitImports[name] = value;
      }
    }
    return wasm;
  }

  identity(): ExecutionEngineIdentity {
    return structuredClone(this.#identity);
  }

  #installFreshAdapter(poolBytes: number): void {
    this.#installAdapter(V86Mirror.#instantiateAdapter(this.#module), poolBytes);
  }

  #installAdapter(wasm: V86Exports, poolBytes: number): void {
    wasm.rust_init();
    if (this.#disableJit) {
      wasm.set_jit_config(0, 1);
    }
    if (this.#relaxJitInstructionBudget) {
      wasm.set_jit_config(4, 0);
      wasm.set_jit_config(6, 1);
    }
    if (this.#jitHotnessThreshold !== undefined) {
      wasm.set_jit_config(5, this.#jitHotnessThreshold);
    }
    invariant(
      wasm.vine_adapter_abi_version() === adapterAbiVersion,
      "execution engine adapter ABI mismatch",
    );
    invariant(
      wasm.vine_cpu_state_version() === this.#identity.cpuContextAbi.version &&
        wasm.vine_cpu_state_size() === this.#identity.cpuContextAbi.bytes,
      "execution engine CPU context ABI mismatch",
    );
    invariant(
      wasm.vine_dirty_page_tracking_version() === 1,
      "execution engine dirty-page tracking ABI mismatch",
    );
    invariant(wasm.vine_set_memory_size(poolBytes) === 0, "v86 rejected mirror memory size");
    wasm.vine_reset_execute_permissions();
    const physicalBase = this.#directMemory
      ? (wasm as V86Exports & V86IslandExports).vine_execution_island_enable_direct_memory(poolBytes)
      : wasm.allocate_memory(poolBytes);
    invariant(physicalBase > 0 && physicalBase % memoryPageSize === 0,
      "execution memory allocation is missing or unaligned");
    const dirtyPagesPointer = wasm.vine_dirty_pages_ptr();
    invariant(
      dirtyPagesPointer % Uint32Array.BYTES_PER_ELEMENT === 0 &&
        dirtyPagesPointer + dirtyPageCandidateCapacity * Uint32Array.BYTES_PER_ELEMENT <=
          wasm.memory.buffer.byteLength,
      "execution engine dirty-page candidate buffer is invalid",
    );
    this.#wasm = wasm;
    this.#dirtyPagesPointer = dirtyPagesPointer;
    this.#physicalBase = physicalBase;
    this.#physicalPoolBytes = poolBytes;
    this.#refreshLinearMemoryViews();
    this.#nextPhysicalPage = memoryPageSize;
    this.#freePhysicalPages.length = 0;
    this.#pageTables.clear();
    this.#directoryMappingCounts.clear();
    this.#forceFullDirtyScan = false;
    wasm.vine_reset_dirty_pages();
    if (!this.#directMemory) wasm.zero_memory(0, poolBytes);
    this.#pageDirectory = this.#allocatePage();
  }

  #capacityRequirement(guestPages: number, pageTables: number): PhysicalCapacityRequirement {
    const bytes = this.#directMemory
      ? directPageTableBytes + guestPages * memoryPageSize
      : physicalPoolBytesForAllocations(1 + pageTables + guestPages);
    if (
      guestPages > this.#maximumPhysicalPages ||
      bytes > this.#maximumPhysicalPoolBytes
    ) {
      throw new V86PhysicalPoolExhausted(
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
      throw new V86PhysicalPoolExhausted(
        requirement.guestPages,
        this.#maximumPhysicalPages,
        requirement.bytes,
        this.#maximumPhysicalPoolBytes,
      );
    }
    this.#reinstantiateAdapter(nextBytes);
  }

  #reinstantiateAdapter(poolBytes: number): void {
    const cpuState = this.#mappings.size === 0 ? null : this.saveState().words;
    const pages: MirrorPageSnapshot[] = [...this.#mappings.values()].map((mapping) => ({
      address: mapping.address,
      ...(mapping.addressAllocationId === undefined
        ? {}
        : { addressAllocationId: mapping.addressAllocationId }),
      owner: mapping.owner,
      protection: { ...mapping.protection },
      bytes: Uint8Array.from(
        this.#physicalMemory.subarray(
          mapping.physicalBase,
          mapping.physicalBase + memoryPageSize,
        ),
      ),
      dirty: Boolean(this.#physicalView.getUint32(mapping.pteAddress, true) & pageFlags.dirty),
    }));
    this.#mappings.clear();
    this.#installFreshAdapter(poolBytes);
    let restoredDirtyPage = false;
    for (const page of pages.sort((left, right) => left.address - right.address)) {
      this.#mapPage(
        page.address,
        page.owner,
        page.protection,
        page.addressAllocationId,
      );
      const mapping = this.#mappings.get(page.address)!;
      this.#physicalMemory.set(page.bytes, mapping.physicalBase);
      if (page.dirty) {
        const pte = this.#physicalView.getUint32(mapping.pteAddress, true);
        this.#physicalView.setUint32(mapping.pteAddress, pte | pageFlags.dirty, true);
        restoredDirtyPage = true;
      }
    }
    this.#forceFullDirtyScan = restoredDirtyPage;
    if (cpuState !== null) {
      cpuState[stateWord.controlRegisterBase + 3] = this.#pageDirectory;
      this.#restoreState(cpuState);
    }
    if (this.#threadContextTemplate !== null) {
      this.#threadContextTemplate[stateWord.controlRegisterBase + 3] = this.#pageDirectory;
    }
    this.#physicalPoolGrowths += 1;
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

  install(reflection: MemoryReflection, prepared: ExecutionBootstrap): void {
    this.initialize(reflection, prepared);
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
    this.#physicalMemory.fill(0, 0, this.#directMemory ? directPageTableBytes : undefined);
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

  rebuild(reflection: MemoryReflection, regions: MemoryRegionSnapshot[]): void {
    this.rebuildMemory(reflection, regions);
  }

  memoryContractState(): string {
    return this.contractState();
  }

  contractState(): string {
    return JSON.stringify({
      mappings: [...this.#mappings.values()]
        .sort((left, right) => left.address - right.address)
        .map(({ address, owner, protection }) => ({ address, owner, protection })),
      reservations: [...this.#reservations.values()]
        .sort((left, right) => left.range.base - right.range.base)
        .map(({ owner, kind, range }) => ({ owner, kind, range })),
    });
  }

  cacheEffectCounts(): Readonly<{
    tlbFlushes: number;
    instructionFetchInvalidations: number;
    generatedCodeInvalidations: number;
  }> {
    return { ...this.#cacheEffectCounts };
  }

  performanceMetrics(): Readonly<ExecutionEnginePerformanceMetrics> {
    const metrics = jitMetricsByInstance.get(this.#wasm);
    invariant(metrics !== undefined, "execution engine JIT metrics are unavailable");
    return {
      jitCompilations: metrics.compilations,
      jitClears: metrics.clears,
      jitActiveFunctions: metrics.activeFunctions,
      jitInstructions:
        (this.#wasm.vine_jit_retired_instructions_low() >>> 0) +
        (this.#wasm.vine_jit_retired_instructions_high() >>> 0) * 0x1_0000_0000,
      interpretedInstructions:
        (this.#wasm.vine_interpreted_retired_instructions_low() >>> 0) +
        (this.#wasm.vine_interpreted_retired_instructions_high() >>> 0) *
          0x1_0000_0000,
    };
  }

  memoryCapacity(): Readonly<ExecutionEngineMemoryCapacity> {
    return {
      initialPhysicalPoolBytes: this.#initialPhysicalPoolBytes,
      physicalPoolBytes: this.#physicalPoolBytes,
      maximumPhysicalPoolBytes: this.#maximumPhysicalPoolBytes,
      maximumPhysicalPages: this.#maximumPhysicalPages,
      mappedGuestPages: this.#mappings.size,
      pageTablePages: this.#pageTables.size,
      freePhysicalPages: this.#freePhysicalPages.length,
      growths: this.#physicalPoolGrowths,
      releasedPages: this.#releasedPages,
      reusedPages: this.#reusedPages,
    };
  }

  linearMemoryBytes(): number {
    return this.#wasm.memory.buffer.byteLength;
  }

  execute(instructionBudget: number): SliceResult {
    const before = this.#wasm.vine_instruction_counter();
    const reason = this.#wasm.vine_execute_budget(instructionBudget);
    this.#refreshLinearMemoryViews();
    const errorCode = this.#wasm.vine_get_stop_error_code();
    const state = this.saveState();
    const after = this.#wasm.vine_instruction_counter();
    return {
      reason,
      errorCode,
      instructions: (after - before) >>> 0,
      state,
    };
  }

  executeBudget(instructionBudget: number, clock?: ExecutionClockSample): ExecutionSlice {
    const before = this.#wasm.vine_instruction_counter();
    const island = this.#wasm as V86Exports & Partial<V86IslandExports>;
    const islandExecute = island.vine_execution_island_execute_budget;
    let reason: number;
    if (islandExecute === undefined) {
      reason = this.#wasm.vine_execute_budget(instructionBudget);
    } else {
      invariant(clock !== undefined, "execution island requires a host clock sample");
      invariant(
        Number.isSafeInteger(clock.retiredInstructions) && clock.retiredInstructions >= 0,
        "execution island retired instruction cursor is invalid",
      );
      invariant(
        clock.hostElapsedTime100ns >= 0n &&
          clock.hostElapsedTime100ns <= 0xffff_ffff_ffff_ffffn,
        "execution island host clock is outside the u64 range",
      );
      const retired = BigInt(clock.retiredInstructions);
      reason = islandExecute(
        instructionBudget,
        Number(retired & 0xffff_ffffn),
        Number((retired >> 32n) & 0xffff_ffffn),
        Number(clock.hostElapsedTime100ns & 0xffff_ffffn),
        Number((clock.hostElapsedTime100ns >> 32n) & 0xffff_ffffn),
      );
      if (reason === v86StopReason.executionIslandError) {
        const pointer = island.vine_bridge_metadata_ptr?.() ?? 0;
        const length = island.vine_bridge_metadata_len?.() ?? 0;
        invariant(
          pointer > 0 && length > 0 && pointer + length <= this.#wasm.memory.buffer.byteLength,
          "execution island failed without error metadata",
        );
        const metadata = JSON.parse(
          new TextDecoder().decode(new Uint8Array(this.#wasm.memory.buffer, pointer, length)),
        ) as { message?: unknown };
        throw new Error(
          typeof metadata.message === "string" ? metadata.message : "execution island failed",
        );
      }
    }
    this.#refreshLinearMemoryViews();
    const errorCode = this.#wasm.vine_get_stop_error_code();
    this.#wasm.vine_cpu_state_save();
    const nativeHeapPages = this.#adoptNativeHeapPages(island);
    const words = this.#liveStateWords();
    const after = this.#wasm.vine_instruction_counter();
    const state = {
      words,
      eax: words[stateWord.eax] ?? 0,
      esp: words[stateWord.esp] ?? 0,
      instructionPointer: words[stateWord.instructionPointer] ?? 0,
      previousInstructionPointer: words[stateWord.previousInstructionPointer] ?? 0,
    };
    const executionIsland =
      islandExecute === undefined
        ? undefined
        : {
            fastApiCompletions: island.vine_execution_island_fast_api_count?.() ?? 0,
            nativeHeapPages,
            writebackBatches: island.vine_execution_island_writeback_batches?.() ?? 0,
            writebackPages: island.vine_execution_island_writeback_pages?.() ?? 0,
            mirroredDeltaPages:
              island.vine_execution_island_mirrored_delta_pages?.() ?? 0,
            generation:
              BigInt(island.vine_execution_island_generation_low?.() ?? 0) |
              (BigInt(island.vine_execution_island_generation_high?.() ?? 0) << 32n),
          };
    return {
      stop: classifyV86Stop(
        reason,
        errorCode,
        words[stateWord.controlRegisterBase + 2],
      ),
      instructions: (after - before) >>> 0,
      context: this.#executionContext(state),
      ...(executionIsland === undefined ? {} : { executionIsland }),
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

  #adoptNativeHeapPages(island: Partial<V86IslandExports>): number {
    const count = island.vine_execution_island_heap_pages_count?.() ?? 0;
    if (count === 0) return 0;
    invariant(this.#directMemory && Number.isInteger(count) && count > 0 && count <= 128 &&
      this.#mappings.size + count <= this.#maximumPhysicalPages,
      "invalid native heap-page count");
    const pointer = island.vine_execution_island_heap_pages_ptr?.() ?? 0;
    invariant(pointer > 0 && pointer % 4 === 0 && pointer + count * 24 <= this.#wasm.memory.buffer.byteLength,
      "invalid native heap-page record range");
    const records = new Uint32Array(this.#wasm.memory.buffer, pointer, count * 6);
    const pages: MirrorMapping[] = [];
    const seen = new Set<number>();
    const protection = { read: true, write: true, execute: false };
    for (let i = 0; i < count; i += 1) {
      const address = records[i * 6]!;
      const owner = records[i * 6 + 1]!;
      const addressAllocationId = records[i * 6 + 2]! + records[i * 6 + 3]! * 0x1_0000_0000;
      const physicalBase = records[i * 6 + 4]!;
      const pteAddress = records[i * 6 + 5]!;
      invariant(Number.isSafeInteger(addressAllocationId) && addressAllocationId > 0 &&
        (address & 4095) === 0 && !seen.has(address) && !this.#mappings.has(address),
        "native heap-page identity is invalid or duplicated");
      const reservation = [...this.#reservations.values()].find(r => r.owner === owner &&
        r.addressAllocationId === addressAllocationId && r.kind === "heap" &&
        rangeContains(r.range, { base: address, byteLength: memoryPageSize }));
      invariant(reservation !== undefined, "native heap-page escaped its owner reservation");
      const table = this.#pageTables.get(address >>> 22);
      invariant(table !== undefined && pteAddress === table + ((address >>> 12) & 1023) * 4 &&
        (this.#physicalView.getUint32(pteAddress, true) & 0xffff_f007) >>> 0 === (physicalBase | 7) >>> 0 &&
        this.#authoritativePageOffset(address, owner, protection) === physicalBase,
        "native heap-page mapping is missing, stale, or has wrong protection");
      seen.add(address);
      pages.push({ address, owner, addressAllocationId, protection, physicalBase, pteAddress, pteWordIndex: pteAddress >>> 2 });
    }
    for (const page of pages) {
      this.#mappings.set(page.address, page);
      const directory = page.address >>> 22;
      this.#directoryMappingCounts.set(directory, (this.#directoryMappingCounts.get(directory) ?? 0) + 1);
    }
    return count;
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
    words.fill(0, 4, 12);
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
      context.version === this.#identity.cpuContextAbi.version &&
        context.byteLength === this.#identity.cpuContextAbi.bytes &&
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
        state[stateWord.version] === adapterAbiVersion &&
        state[stateWord.wordCount] === state.byteLength / Uint32Array.BYTES_PER_ELEMENT,
      "stopped CPU resume requires the current saved v86 state",
    );
    this.#applyCpuResumeToState(state, completion);
    invariant(this.#wasm.vine_cpu_state_restore() === 0, "v86 rejected a stopped CPU resume");
  }

  #applyCpuResumeToState(state: Uint32Array, completion: CpuResume): void {
    this.#applyRegisterUpdate(state, stateWord.eax, completion.eax);
    this.#applyRegisterUpdate(state, stateWord.edx, completion.edx);
    this.#applyRegisterUpdate(state, stateWord.flags, completion.eflags);
    state[stateWord.esp] = completion.stackPointer >>> 0;
    state[stateWord.instructionPointer] = completion.instructionPointer >>> 0;
    state[stateWord.previousInstructionPointer] = completion.instructionPointer >>> 0;
    state[stateWord.inHalt] = 0;
  }

  applyCompletion(
    completion: CpuResume & { implementationId?: string; lastError?: number },
  ): void {
    this.applyCpuResume(completion);
  }

  enterGuestCallback(target: number, stackPointer: number): void {
    const state = this.saveState().words;
    state[stateWord.esp] = stackPointer >>> 0;
    state[stateWord.instructionPointer] = target >>> 0;
    state[stateWord.previousInstructionPointer] = target >>> 0;
    state[stateWord.inHalt] = 0;
    this.#restoreState(state);
  }

  enterCallback(target: number, stackPointer: number): void {
    this.enterGuestCallback(target, stackPointer);
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
      throw new ExecutionMemoryAccessFault(
        "execution-read-protection",
        address,
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
        throw new ExecutionMemoryAccessFault(
          "execution-read-protection",
          current,
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
    if (dirtyMappings.length > 0) {
      this.#wasm.full_clear_tlb();
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
      this.applyCacheEffects(delta.cacheEffects, delta.pages.length !== 0);
      return;
    }
    const onlyOperation = delta.mappingOperations[0]!;
    if (this.#directMemory && delta.mappingOperations.length === 1 &&
        onlyOperation.operation === "protect" && this.#applyDirectProtection(delta, onlyOperation)) {
      this.applyCacheEffects(delta.cacheEffects, true);
      return;
    }
    const capacity = this.#preflightHostDelta(delta);
    this.#ensurePhysicalCapacity(capacity);
    if (capacity.directMappings !== undefined) {
      // Rust has already committed the complete transaction. Intermediate mappings may
      // no longer exist; validate the sequence, then install only its final page state.
      for (const [address, mapping] of capacity.directMappings) {
        const previous = this.#mappings.get(address);
        if (previous !== undefined && (mapping === null ||
            previous.owner !== mapping.owner ||
            previous.addressAllocationId !== mapping.addressAllocationId ||
            previous.physicalBase !== mapping.physicalBase)) this.#unmapPage(address);
      }
      for (const [address, mapping] of capacity.directMappings) {
        if (mapping === null) continue;
        const previous = this.#mappings.get(address);
        if (previous === undefined) {
          this.#mapPage(address, mapping.owner, mapping.protection,
            mapping.addressAllocationId, mapping.physicalBase);
        } else if (!sameProtection(previous.protection, mapping.protection)) {
          this.#updateProtection(address, mapping.protection);
        }
      }
      for (const operation of delta.mappingOperations) {
        if (operation.operation === "reserve") this.#applyMappingOperation(operation);
        else if (operation.operation === "release") this.#reservations.delete(operation.range.base);
      }
    } else {
      for (const operation of delta.mappingOperations) {
        this.#applyMappingOperation(operation);
      }
    }
    for (const page of delta.pages) {
      const mapping = this.#mappings.get(page.address)!;
      this.#installPageBytes(page.bytes, mapping);
    }
    this.applyCacheEffects(delta.cacheEffects,
      delta.pages.length !== 0 || delta.mappingOperations.some((op) => op.operation !== "reserve"));
  }

  applyHostDelta(delta: HostMemoryDelta): void {
    this.applyMemoryDelta(delta);
  }

  #applyDirectProtection(
    delta: HostMemoryDelta,
    operation: Extract<MappingOperation, { operation: "protect" }>,
  ): boolean {
    // A protection-only transaction cannot change capacity or ownership. Check the
    // existing pages in place; avoid building a new per-page planning map every frame.
    const addresses = pageAddresses(operation.range);
    for (const address of addresses) {
      const mapping = this.#mappings.get(address);
      invariant(mapping !== undefined, "protect operation targets an unmapped mirror page");
      invariant(mapping.owner === operation.owner, "protect operation owner mismatch");
      invariant(mapping.addressAllocationId === operation.addressAllocationId,
        "protect operation address allocation mismatch");
      if (this.#authoritativePageOffset(address, mapping.owner, operation.protection) !==
          mapping.physicalBase) return false; // A stale backing needs the complete mapping plan.
      invariant(operation.protection.write ||
        (this.#physicalWords[mapping.pteWordIndex]! & pageFlags.dirty) === 0,
        "mirror write protection changed while the CPU page was still dirty");
    }
    this.#preflightDeltaPagesWithLookup(delta.pages, (address) => {
      const mapping = this.#mappings.get(address);
      return mapping !== undefined && address >= operation.range.base && address < rangeEnd(operation.range)
        ? { ...mapping, protection: operation.protection } : mapping;
    });
    for (const address of addresses) {
      if (!sameProtection(this.#mappings.get(address)!.protection, operation.protection))
        this.#updateProtection(address, operation.protection);
    }
    for (const page of delta.pages) this.#invalidateMappedCode(this.#mappings.get(page.address)!);
    return true;
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
          invariant(
            pageAddresses(operation.range).every(
              (address) => plannedMapping(address) === undefined,
            ),
            "reserve operation overlaps a committed mirror page",
          );
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
          for (const address of pageAddresses(operation.range)) {
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
          for (const address of pageAddresses(operation.range)) {
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
          for (const address of pageAddresses(operation.range)) {
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
          for (const address of pageAddresses(operation.range)) {
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
    if (this.#directMemory) {
      for (const [address, mapping] of mappingChanges) {
        if (mapping === null) continue;
        // Keep the checked backing for this synchronous commit; Rust cannot change it
        // between preflight and application. Protection-only deltas retain the PTE.
        mapping.physicalBase = this.#authoritativePageOffset(address, mapping.owner, mapping.protection);
        const previous = this.#mappings.get(address);
        if (previous !== undefined && previous.owner === mapping.owner &&
            previous.addressAllocationId === mapping.addressAllocationId &&
            previous.physicalBase === mapping.physicalBase) {
          invariant(mapping.protection.write ||
            (this.#physicalWords[previous.pteWordIndex]! & pageFlags.dirty) === 0,
            "mirror write protection changed while the CPU page was still dirty");
        }
      }
      capacity.directMappings = mappingChanges;
    }
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
      if (this.#directMemory && mapping.physicalBase !== undefined) {
        invariant(this.#authoritativePageOffset(page.address, page.owner, page.protection) ===
          mapping.physicalBase, "host delta changed backing without a mapping operation");
      }
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
        for (const address of pageAddresses(operation.range)) {
          this.#mapPage(
            address,
            operation.owner,
            operation.protection,
            operation.addressAllocationId,
          );
        }
        break;
      case "protect":
        for (const address of pageAddresses(operation.range)) {
          this.#updateProtection(address, operation.protection);
        }
        break;
      case "decommit":
        for (const address of pageAddresses(operation.range)) {
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

  applyCacheEffects(effects: CacheEffects, mappedDelta = false): void {
    if (effects.flushTlb || effects.invalidateInstructionFetch) {
      this.#wasm.full_clear_tlb();
      this.#cacheEffectCounts.tlbFlushes += 1;
    }
    this.#cacheEffectCounts.instructionFetchInvalidations += Number(
      effects.invalidateInstructionFetch,
    );
    if (effects.invalidateGeneratedCode) {
      if (!this.#directMemory || !mappedDelta) this.#wasm.jit_clear_cache_js();
      this.#cacheEffectCounts.generatedCodeInvalidations += 1;
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
    checkedPhysicalBase?: number,
  ): void {
    const table = this.#ensurePageTable(address);
    const pteAddress = table + ((address >>> 12) & 0x3ff) * 4;
    const physicalBase = this.#directMemory
      ? checkedPhysicalBase ?? this.#authoritativePageOffset(address, owner, protection)
      : this.#allocatePage();
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
    this.#invalidateMappedCode(mapping);
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

  #authoritativePageOffset(address: number, owner: number, protection: MemoryProtection): number {
    const offset = (this.#wasm as V86Exports & V86IslandExports).vine_execution_island_page_offset(
      address, owner, Number(protection.read) | Number(protection.write) << 1 | Number(protection.execute) << 2,
    );
    if (!Number.isInteger(offset) || offset < directPageTableBytes ||
        offset % memoryPageSize !== 0 || offset + memoryPageSize > this.#physicalPoolBytes) {
      throw new Error(`authoritative execution page is missing, stale, or outside its backing: ` +
        `address=0x${address.toString(16)}, owner=${owner}, protection=${JSON.stringify(protection)}, offset=${offset}`);
    }
    return offset;
  }

  #invalidateMappedCode(mapping: MirrorMapping): void {
    if (this.#directMemory && mapping.protection.execute) {
      // Invalidate by execution-mirror physical page before reuse/unmapping.
      // v86 also discards modules spanning this page and in-flight compilations.
      this.#wasm.jit_dirty_cache(mapping.physicalBase, mapping.physicalBase + memoryPageSize);
    }
  }

  #installPageBytes(bytes: Uint8Array, mapping: MirrorMapping): void {
    if (this.#directMemory) {
      invariant(this.#authoritativePageOffset(mapping.address, mapping.owner, mapping.protection) ===
        mapping.physicalBase, "host delta changed backing without a mapping operation");
      this.#invalidateMappedCode(mapping);
    } else {
      this.#physicalMemory.set(bytes, mapping.physicalBase);
    }
  }

  #updateProtection(address: number, protection: MemoryProtection): void {
    const mapping = this.#mappings.get(address)!;
    const dirty = this.#physicalView.getUint32(mapping.pteAddress, true) & pageFlags.dirty;
    invariant(
      protection.write || dirty === 0,
      "mirror write protection changed while the CPU page was still dirty",
    );
    this.#invalidateMappedCode(mapping);
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
    this.#invalidateMappedCode(mapping);
    this.#physicalView.setUint32(mapping.pteAddress, 0, true);
    invariant(
      this.#wasm.vine_set_page_executable(address, 0) === 0,
      "v86 rejected unmapped execute permission",
    );
    this.#mappings.delete(address);
    if (this.#directMemory) this.#releasedPages += 1;
    else this.#releasePage(mapping.physicalBase);
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
    invariant(this.#wasm.vine_cpu_state_version() === 1, "unsupported v86 CPU state version");
    invariant(this.#wasm.vine_cpu_state_size() === 640, "unexpected v86 CPU state size");
    this.#wasm.vine_cpu_state_save();
    const words = this.#liveStateWords();
    invariant(words[stateWord.magic] === 0x5649_4e45, "invalid v86 CPU state magic");
    invariant(words[stateWord.version] === 1 && words[stateWord.wordCount] === 160, "invalid CPU state header");
    words.fill(0, 4, 12);
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

  #applyRegisterUpdate(words: Uint32Array, index: number, update: CpuRegisterUpdate): void {
    if (update.action === "set") {
      words[index] = update.value >>> 0;
    }
  }

  #mappingFor(address: number): MirrorMapping {
    const pageAddress = (address & ~(memoryPageSize - 1)) >>> 0;
    const mapping = this.#mappings.get(pageAddress);
    if (mapping === undefined) {
      throw new ExecutionMemoryAccessFault(
        "execution-address-unmapped",
        address,
        `guest address 0x${address.toString(16)} is unmapped`,
      );
    }
    return mapping;
  }

  #executionContext(state: CpuState): ExecutionCpuContext {
    return {
      version: this.#identity.cpuContextAbi.version,
      byteLength: this.#identity.cpuContextAbi.bytes,
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
      this.#reusedPages += 1;
      this.#physicalMemory.fill(0, reused, reused + memoryPageSize);
      return reused;
    }
    for (const excluded of excludedPhysicalRanges) {
      if (this.#nextPhysicalPage >= excluded.start && this.#nextPhysicalPage < excluded.end) {
        this.#nextPhysicalPage = excluded.end;
      }
    }
    if (this.#nextPhysicalPage + memoryPageSize >
        (this.#directMemory ? directPageTableBytes : this.#physicalPoolBytes)) {
      throw new V86PhysicalPoolExhausted(
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
    this.#releasedPages += 1;
  }
}
