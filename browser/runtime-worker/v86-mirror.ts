import engineProfile from "../../fixtures/execution-engines/v86-browser-v1.json";
import {
  ExecutionMemoryAccessFault,
  type CpuRegisterUpdate,
  type CpuResume,
  type ExecutionBootstrap,
  type ExecutionCpuContext,
  type ExecutionEngine,
  type ExecutionEngineIdentity,
  type ExecutionEngineMemoryCapacity,
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
const initialPhysicalPoolBytes = 64 * 1024;
const hardMaximumPhysicalPoolBytes = 128 * 1024 * 1024;
const excludedPhysicalRanges = [{ start: 0x000a_0000, end: 0x000c_0000 }] as const;
const pageFlags = { present: 1, writable: 2, user: 4, dirty: 0x40 } as const;
const adapterAbiVersion = 1;
const stateWord = {
  magic: 0,
  version: 1,
  wordCount: 2,
  eax: 4,
  edx: 6,
  esp: 8,
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
  vine_cpu_state_ptr(): number;
  vine_cpu_state_size(): number;
  vine_cpu_state_version(): number;
  vine_cpu_state_save(): void;
  vine_cpu_state_restore(): number;
  vine_adapter_abi_version(): number;
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

interface PreparedV86Adapter {
  module: WebAssembly.Module;
  identity: ExecutionEngineIdentity;
}

export interface V86MirrorOptions {
  maximumPhysicalPages?: ExecutionEngineOptions["maximumPhysicalPages"];
  initialPhysicalPoolBytes?: number;
}

export type V86MirrorFactory = (options?: V86MirrorOptions) => V86Mirror;

interface PhysicalCapacityRequirement {
  guestPages: number;
  pageTables: number;
  bytes: number;
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
  readonly #writableMappings: MirrorMapping[] = [];
  readonly #reservations = new Map<number, MirrorReservation>();
  readonly #dirtyMappingsScratch: MirrorMapping[] = [];
  readonly #borrowedDirtyPages: DirtyMirrorPage[] = [];
  readonly #borrowedDirtyPagePool: DirtyMirrorPage[] = [];
  #physicalMemory!: Uint8Array;
  #physicalView!: DataView;
  readonly #maximumPhysicalPages: number;
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
  ) {
    this.#module = module;
    this.#identity = structuredClone(identity);
    this.#maximumPhysicalPages = requirePageCount(
      options.maximumPhysicalPages ?? defaultMaximumPhysicalPages,
      "maximum physical page count",
    );
    this.#maximumPhysicalPoolBytes = maximumPhysicalPoolBytes(this.#maximumPhysicalPages);
    invariant(
      this.#maximumPhysicalPoolBytes <= hardMaximumPhysicalPoolBytes,
      `configured physical page limit requires ${this.#maximumPhysicalPoolBytes} bytes, ` +
        `above the hard ${hardMaximumPhysicalPoolBytes}-byte mirror limit`,
    );
    const requestedInitialBytes = alignPoolBytes(
      options.initialPhysicalPoolBytes ?? initialPhysicalPoolBytes,
    );
    this.#initialPhysicalPoolBytes = Math.min(
      Math.max(requestedInitialBytes, physicalPoolBytesForAllocations(1)),
      this.#maximumPhysicalPoolBytes,
    );
    this.#physicalPoolBytes = this.#initialPhysicalPoolBytes;
    this.#installFreshAdapter(this.#physicalPoolBytes);
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

  static #instantiateAdapter(module: WebAssembly.Module): V86Exports {
    const imports = WebAssembly.Module.imports(module);
    const environment: Record<string, WebAssembly.ImportValue> = {
      __indirect_function_table: new WebAssembly.Table({
        element: "anyfunc",
        initial: 1_924,
      }),
    };
    for (const imported of imports) {
      invariant(imported.module === "env", `unexpected v86 import module ${imported.module}`);
      if (imported.kind !== "function") {
        invariant(imported.kind === "table", `unexpected v86 import ${imported.name}`);
        continue;
      }
      environment[imported.name] = (..._arguments: unknown[]) => {
        if (imported.name === "microtick" || imported.name === "get_rand_int") {
          return 0;
        }
        if (
          imported.name === "jit_clear_func" ||
          imported.name === "stop_idling" ||
          imported.name === "cpu_event_halt" ||
          imported.name === "run_hardware_timers"
        ) {
          return 0;
        }
        throw new Error(`v86 reached unsupported host import ${imported.name}`);
      };
    }
    return new WebAssembly.Instance(module, { env: environment }).exports as V86Exports;
  }

  identity(): ExecutionEngineIdentity {
    return structuredClone(this.#identity);
  }

  #installFreshAdapter(poolBytes: number): void {
    const wasm = V86Mirror.#instantiateAdapter(this.#module);
    wasm.rust_init();
    invariant(
      wasm.vine_adapter_abi_version() === adapterAbiVersion,
      "execution engine adapter ABI mismatch",
    );
    invariant(
      wasm.vine_cpu_state_version() === this.#identity.cpuContextAbi.version &&
        wasm.vine_cpu_state_size() === this.#identity.cpuContextAbi.bytes,
      "execution engine CPU context ABI mismatch",
    );
    invariant(wasm.vine_set_memory_size(poolBytes) === 0, "v86 rejected mirror memory size");
    wasm.vine_reset_execute_permissions();
    const physicalBase = wasm.allocate_memory(poolBytes);
    this.#wasm = wasm;
    this.#physicalMemory = new Uint8Array(wasm.memory.buffer, physicalBase, poolBytes);
    this.#physicalView = new DataView(
      this.#physicalMemory.buffer,
      this.#physicalMemory.byteOffset,
      this.#physicalMemory.byteLength,
    );
    this.#physicalPoolBytes = poolBytes;
    this.#nextPhysicalPage = memoryPageSize;
    this.#freePhysicalPages.length = 0;
    this.#pageTables.clear();
    wasm.zero_memory(0, poolBytes);
    this.#pageDirectory = this.#allocatePage();
  }

  #capacityRequirement(guestPages: number, pageTables: number): PhysicalCapacityRequirement {
    const bytes = physicalPoolBytesForAllocations(1 + pageTables + guestPages);
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
    this.#writableMappings.length = 0;
    this.#installFreshAdapter(poolBytes);
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
      }
    }
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
    const state = this.saveState().words;
    this.#mappings.clear();
    this.#writableMappings.length = 0;
    this.#reservations.clear();
    this.#pageTables.clear();
    this.#freePhysicalPages.length = 0;
    this.#nextPhysicalPage = memoryPageSize;
    this.#physicalMemory.fill(0);
    this.#wasm.vine_reset_execute_permissions();
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

  executeBudget(instructionBudget: number): ExecutionSlice {
    const result = this.execute(instructionBudget);
    return {
      stop: classifyV86Stop(
        result.reason,
        result.errorCode,
        result.state.words[stateWord.controlRegisterBase + 2],
      ),
      instructions: result.instructions,
      context: this.#executionContext(result.state),
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
    this.#applyRegisterUpdate(state, stateWord.eax, completion.eax);
    this.#applyRegisterUpdate(state, stateWord.edx, completion.edx);
    this.#applyRegisterUpdate(state, stateWord.flags, completion.eflags);
    state[stateWord.esp] = completion.stackPointer >>> 0;
    state[stateWord.instructionPointer] = completion.instructionPointer >>> 0;
    state[stateWord.previousInstructionPointer] = completion.instructionPointer >>> 0;
    state[stateWord.inHalt] = 0;
    this.#restoreState(state);
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
    const bytes = this.readGuestBytes(address, 4);
    return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(0, true);
  }

  readGuestBytes(address: number, byteLength: number): Uint8Array {
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
    const bytes = new Uint8Array(byteLength);
    for (let index = 0; index < bytes.length; index += 1) {
      const current = address + index;
      const mapping = this.#mappingFor(current);
      if (!mapping.protection.read) {
        throw new ExecutionMemoryAccessFault(
          "execution-read-protection",
          current,
          `guest mirror read targets unreadable address 0x${current.toString(16)}`,
        );
      }
      bytes[index] = this.#physicalMemory[mapping.physicalBase + (current & 0xfff)] ?? 0;
    }
    return bytes;
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
    const dirtyMappings = this.#dirtyMappingsScratch;
    dirtyMappings.length = 0;
    for (const mapping of this.#writableMappings) {
      const pte = this.#physicalView.getUint32(mapping.pteAddress, true);
      if ((pte & pageFlags.dirty) !== 0) {
        dirtyMappings.push(mapping);
      }
    }
    const dirty = copyBytes ? [] : this.#borrowedDirtyPages;
    dirty.length = 0;
    for (const mapping of dirtyMappings) {
      const pte = this.#physicalView.getUint32(mapping.pteAddress, true);
      this.#physicalView.setUint32(mapping.pteAddress, pte & ~pageFlags.dirty, true);
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
    const capacity = this.#preflightHostDelta(delta);
    this.#ensurePhysicalCapacity(capacity);
    for (const operation of delta.mappingOperations) {
      this.#applyMappingOperation(operation);
    }
    for (const page of delta.pages) {
      const mapping = this.#mappings.get(page.address)!;
      this.#physicalMemory.set(page.bytes, mapping.physicalBase);
    }
    this.applyCacheEffects(delta.cacheEffects);
  }

  applyHostDelta(delta: HostMemoryDelta): void {
    this.applyMemoryDelta(delta);
  }

  #preflightHostDelta(delta: HostMemoryDelta): PhysicalCapacityRequirement {
    if (delta.mappingOperations.length === 0) {
      this.#preflightDeltaPages(delta.pages, this.#mappings);
      return this.#capacityRequirement(this.#mappings.size, this.#pageTables.size);
    }

    const mappings = new Map<number, PlannedMapping>(
      [...this.#mappings].map(([address, mapping]) => [
        address,
        {
          ...(mapping.addressAllocationId === undefined
            ? {}
            : { addressAllocationId: mapping.addressAllocationId }),
          owner: mapping.owner,
          protection: { ...mapping.protection },
        },
      ]),
    );
    const reservations = new Map<number, MirrorReservation>(
      [...this.#reservations].map(([address, reservation]) => [
        address,
        {
          ...(reservation.addressAllocationId === undefined
            ? {}
            : { addressAllocationId: reservation.addressAllocationId }),
          owner: reservation.owner,
          kind: reservation.kind,
          range: { ...reservation.range },
        },
      ]),
    );
    let directoryIndices = new Set(this.#pageTables.keys());
    let capacity = this.#capacityRequirement(mappings.size, directoryIndices.size);

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
            ![...mappings.keys()].some(
              (address) =>
                address < rangeEnd(operation.range) &&
                operation.range.base < address + memoryPageSize,
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
            invariant(!mappings.has(address), "commit operation targets a committed mirror page");
            mappings.set(address, {
              ...(operation.addressAllocationId === undefined
                ? {}
                : { addressAllocationId: operation.addressAllocationId }),
              owner: operation.owner,
              protection: { ...operation.protection },
            });
            directoryIndices.add(address >>> 22);
          }
          break;
        }
        case "protect":
          for (const address of pageAddresses(operation.range)) {
            const mapping = mappings.get(address);
            invariant(mapping !== undefined, "protect operation targets an unmapped mirror page");
            invariant(mapping.owner === operation.owner, "protect operation owner mismatch");
            invariant(
              mapping.addressAllocationId === operation.addressAllocationId,
              "protect operation address allocation mismatch",
            );
            mapping.protection = { ...operation.protection };
          }
          break;
        case "decommit":
          for (const address of pageAddresses(operation.range)) {
            const mapping = mappings.get(address);
            invariant(mapping !== undefined, "decommit operation targets an unmapped mirror page");
            invariant(mapping.owner === operation.owner, "decommit operation owner mismatch");
            invariant(
              mapping.addressAllocationId === operation.addressAllocationId,
              "decommit operation address allocation mismatch",
            );
            mappings.delete(address);
          }
          directoryIndices = new Set([...mappings.keys()].map((address) => address >>> 22));
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
          for (const [address, mapping] of [...mappings]) {
            if (address < operation.range.base || address >= rangeEnd(operation.range)) {
              continue;
            }
            invariant(mapping.owner === operation.owner, "release operation owner mismatch");
            invariant(
              mapping.addressAllocationId === operation.addressAllocationId,
              "release operation address allocation mismatch",
            );
            mappings.delete(address);
            matched = true;
          }
          invariant(matched, "release operation targets an unknown mirror range");
          directoryIndices = new Set([...mappings.keys()].map((address) => address >>> 22));
          break;
        }
      }
      const nextCapacity = this.#capacityRequirement(mappings.size, directoryIndices.size);
      if (nextCapacity.bytes > capacity.bytes) {
        capacity = nextCapacity;
      }
    }

    this.#preflightDeltaPages(delta.pages, mappings);
    return capacity;
  }

  #preflightDeltaPages(
    pages: HostMemoryDelta["pages"],
    mappings: ReadonlyMap<number, PlannedMapping>,
  ): void {
    const dirtyAddresses = new Set<number>();
    for (const page of pages) {
      invariant((page.address & (memoryPageSize - 1)) === 0, "host delta page is unaligned");
      invariant(page.bytes.byteLength === memoryPageSize, "host delta page is not 4 KiB");
      invariant(!dirtyAddresses.has(page.address), "host delta repeats a dirty page");
      dirtyAddresses.add(page.address);
      const mapping = mappings.get(page.address);
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

  applyCacheEffects(effects: CacheEffects): void {
    if (effects.flushTlb || effects.invalidateInstructionFetch) {
      this.#wasm.full_clear_tlb();
      this.#cacheEffectCounts.tlbFlushes += 1;
    }
    this.#cacheEffectCounts.instructionFetchInvalidations += Number(
      effects.invalidateInstructionFetch,
    );
    if (effects.invalidateGeneratedCode) {
      this.#wasm.jit_clear_cache_js();
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
      this.#physicalMemory.set(page.bytes, mapping.physicalBase);
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
    };
    this.#mappings.set(address, mapping);
    if (mapping.protection.write) {
      this.#trackWritableMapping(mapping);
    }
    invariant(
      this.#wasm.vine_set_page_executable(address, Number(protection.execute)) === 0,
      "v86 rejected mapped execute permission",
    );
  }

  #updateProtection(address: number, protection: MemoryProtection): void {
    const mapping = this.#mappings.get(address)!;
    const dirty = this.#physicalView.getUint32(mapping.pteAddress, true) & pageFlags.dirty;
    invariant(
      protection.write || dirty === 0,
      "mirror write protection changed while the CPU page was still dirty",
    );
    const wasWritable = mapping.protection.write;
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
    if (wasWritable && !protection.write) {
      this.#untrackWritableMapping(mapping);
    } else if (!wasWritable && protection.write) {
      this.#trackWritableMapping(mapping);
    }
    invariant(
      this.#wasm.vine_set_page_executable(address, Number(protection.execute)) === 0,
      "v86 rejected protected execute permission",
    );
  }

  #unmapPage(address: number): void {
    const mapping = this.#mappings.get(address)!;
    if (mapping.protection.write) {
      this.#untrackWritableMapping(mapping);
    }
    this.#physicalView.setUint32(mapping.pteAddress, 0, true);
    invariant(
      this.#wasm.vine_set_page_executable(address, 0) === 0,
      "v86 rejected unmapped execute permission",
    );
    this.#mappings.delete(address);
    this.#releasePage(mapping.physicalBase);
    const directoryIndex = address >>> 22;
    if (![...this.#mappings.keys()].some((candidate) => candidate >>> 22 === directoryIndex)) {
      const table = this.#pageTables.get(directoryIndex);
      invariant(table !== undefined, "empty mirror directory has no page table");
      this.#physicalView.setUint32(this.#pageDirectory + directoryIndex * 4, 0, true);
      this.#pageTables.delete(directoryIndex);
      this.#releasePage(table);
    }
  }

  #trackWritableMapping(mapping: MirrorMapping): void {
    let lower = 0;
    let upper = this.#writableMappings.length;
    while (lower < upper) {
      const middle = (lower + upper) >>> 1;
      if (this.#writableMappings[middle]!.address < mapping.address) {
        lower = middle + 1;
      } else {
        upper = middle;
      }
    }
    invariant(
      this.#writableMappings[lower]?.address !== mapping.address,
      "writable mirror mapping is already indexed",
    );
    this.#writableMappings.splice(lower, 0, mapping);
  }

  #untrackWritableMapping(mapping: MirrorMapping): void {
    let lower = 0;
    let upper = this.#writableMappings.length;
    while (lower < upper) {
      const middle = (lower + upper) >>> 1;
      if (this.#writableMappings[middle]!.address < mapping.address) {
        lower = middle + 1;
      } else {
        upper = middle;
      }
    }
    invariant(this.#writableMappings[lower] === mapping, "writable mirror mapping index is stale");
    this.#writableMappings.splice(lower, 1);
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
      opaqueWords: Uint32Array.from(state.words),
      eax: state.eax,
      esp: state.esp,
      instructionPointer: state.instructionPointer,
      previousInstructionPointer: state.previousInstructionPointer,
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
    if (this.#nextPhysicalPage + memoryPageSize > this.#physicalPoolBytes) {
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
