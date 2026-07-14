import engineProfile from "../../fixtures/execution-engines/v86-browser-v1.json";
import {
  ExecutionMemoryAccessFault,
  type CpuRegisterUpdate,
  type CpuResume,
  type ExecutionBootstrap,
  type ExecutionCpuContext,
  type ExecutionEngine,
  type ExecutionEngineIdentity,
  type ExecutionSlice,
  type ExecutionStop,
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

const physicalPoolBytes = 2 * 1024 * 1024;
const excludedPhysicalRanges = [{ start: 0x000a_0000, end: 0x000c_0000 }] as const;
const pageFlags = { present: 1, writable: 2, user: 4, dirty: 0x40 } as const;
const adapterPrivateOffset = { instructionCounter: 664, memorySize: 812 } as const;
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
  vine_execute_budget(instructions: number): number;
  vine_get_stop_error_code(): number;
}

interface MirrorMapping {
  address: number;
  owner: number;
  protection: MemoryProtection;
  physicalBase: number;
  pteAddress: number;
}

interface MirrorReservation {
  owner: number;
  kind: MemoryRegionKind;
  range: MemoryRange;
}

interface PlannedMapping {
  owner: number;
  protection: MemoryProtection;
}

interface PreparedV86Adapter {
  module: WebAssembly.Module;
  identity: ExecutionEngineIdentity;
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
  readonly #wasm: V86Exports;
  readonly #identity: ExecutionEngineIdentity;
  readonly #mappings = new Map<number, MirrorMapping>();
  readonly #reservations = new Map<number, MirrorReservation>();
  readonly #physicalMemory: Uint8Array;
  readonly #physicalView: DataView;
  readonly #privateState: DataView;
  #pageDirectory: number;
  #nextPhysicalPage = memoryPageSize;
  readonly #freePhysicalPages: number[] = [];
  readonly #pageTables = new Map<number, number>();
  readonly #cacheEffectCounts = {
    tlbFlushes: 0,
    instructionFetchInvalidations: 0,
    generatedCodeInvalidations: 0,
  };

  private constructor(wasm: V86Exports, identity: ExecutionEngineIdentity) {
    this.#wasm = wasm;
    this.#identity = structuredClone(identity);
    wasm.rust_init();
    invariant(
      wasm.vine_cpu_state_version() === identity.cpuContextAbi.version &&
        wasm.vine_cpu_state_size() === identity.cpuContextAbi.bytes,
      "execution engine CPU context ABI mismatch",
    );
    let privateState = new DataView(wasm.memory.buffer);
    privateState.setUint32(adapterPrivateOffset.memorySize, physicalPoolBytes, true);
    const physicalBase = wasm.allocate_memory(physicalPoolBytes);
    privateState = new DataView(wasm.memory.buffer);
    this.#privateState = privateState;
    this.#physicalMemory = new Uint8Array(wasm.memory.buffer, physicalBase, physicalPoolBytes);
    this.#physicalView = new DataView(
      this.#physicalMemory.buffer,
      this.#physicalMemory.byteOffset,
      this.#physicalMemory.byteLength,
    );
    wasm.zero_memory(0, physicalPoolBytes);
    this.#pageDirectory = this.#allocatePage();
  }

  static async instantiate(url: URL): Promise<V86Mirror> {
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
    return V86Mirror.#instantiatePrepared(await prepared);
  }

  static async instantiateBytes(
    bytes: Uint8Array,
    identity?: ExecutionEngineIdentity,
  ): Promise<V86Mirror> {
    return V86Mirror.#instantiatePrepared(await V86Mirror.#prepareBytes(bytes, identity));
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

  static async #instantiatePrepared(prepared: PreparedV86Adapter): Promise<V86Mirror> {
    const { module, identity } = prepared;
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
    const instance = await WebAssembly.instantiate(module, { env: environment });
    return new V86Mirror(instance.exports as V86Exports, identity);
  }

  identity(): ExecutionEngineIdentity {
    return structuredClone(this.#identity);
  }

  initialize(reflection: MemoryReflection, prepared: ExecutionBootstrap): void {
    invariant(this.#mappings.size === 0, "v86 mirror is already initialized");
    this.#installReflectedPages(reflection);
    this.#installReservations(prepared.regions);
    this.applyCacheEffects(reflection.cacheEffects);
    this.#initializeCpu(prepared);
  }

  install(reflection: MemoryReflection, prepared: ExecutionBootstrap): void {
    this.initialize(reflection, prepared);
  }

  rebuildMemory(reflection: MemoryReflection, regions: MemoryRegionSnapshot[]): void {
    const state = this.saveState().words;
    this.#mappings.clear();
    this.#reservations.clear();
    this.#pageTables.clear();
    this.#freePhysicalPages.length = 0;
    this.#nextPhysicalPage = memoryPageSize;
    this.#physicalMemory.fill(0);
    this.#pageDirectory = this.#allocatePage();
    this.#installReflectedPages(reflection);
    this.#installReservations(regions);
    this.applyCacheEffects(reflection.cacheEffects);
    state[stateWord.controlRegisterBase + 3] = this.#pageDirectory;
    this.#restoreState(state);
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

  linearMemoryBytes(): number {
    return this.#wasm.memory.buffer.byteLength;
  }

  execute(instructionBudget: number): SliceResult {
    const beforeState = this.saveState();
    this.#assertExecutableFetchWindow(beforeState.instructionPointer);
    const before = this.#privateState.getUint32(adapterPrivateOffset.instructionCounter, true);
    const reason = this.#wasm.vine_execute_budget(instructionBudget);
    const errorCode = this.#wasm.vine_get_stop_error_code();
    const state = this.saveState();
    const after = this.#privateState.getUint32(adapterPrivateOffset.instructionCounter, true);
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
    const bytes = new Uint8Array(4);
    for (let index = 0; index < bytes.length; index += 1) {
      const current = (address + index) >>> 0;
      invariant(current >= address, "guest read wrapped the address space");
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
    return new DataView(bytes.buffer).getUint32(0, true);
  }

  readU32(address: number): number {
    return this.readGuestU32(address);
  }

  collectChangedPages(): DirtyMirrorPage[] {
    const dirty: DirtyMirrorPage[] = [];
    const writable = [...this.#mappings.values()]
      .filter((mapping) => mapping.protection.write)
      .sort((left, right) => left.address - right.address);
    for (const mapping of writable) {
      const pte = this.#physicalView.getUint32(mapping.pteAddress, true);
      if ((pte & pageFlags.dirty) === 0) {
        continue;
      }
      this.#physicalView.setUint32(mapping.pteAddress, pte & ~pageFlags.dirty, true);
      dirty.push({
        address: mapping.address,
        owner: mapping.owner,
        bytes: Uint8Array.from(
          this.#physicalMemory.subarray(
            mapping.physicalBase,
            mapping.physicalBase + memoryPageSize,
          ),
        ),
      });
    }
    this.#wasm.full_clear_tlb();
    return dirty;
  }

  discoverDirtyPages(): DirtyMirrorPage[] {
    return this.collectChangedPages();
  }

  applyMemoryDelta(delta: HostMemoryDelta): void {
    this.#preflightHostDelta(delta);
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

  #preflightHostDelta(delta: HostMemoryDelta): void {
    const mappings = new Map<number, PlannedMapping>(
      [...this.#mappings].map(([address, mapping]) => [
        address,
        { owner: mapping.owner, protection: { ...mapping.protection } },
      ]),
    );
    const reservations = new Map<number, MirrorReservation>(
      [...this.#reservations].map(([address, reservation]) => [
        address,
        {
          owner: reservation.owner,
          kind: reservation.kind,
          range: { ...reservation.range },
        },
      ]),
    );
    const directoryIndices = new Set(this.#pageTables.keys());

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
                  rangeContains(reservation.range, operation.range),
              ),
            "commit operation is outside its mirror reservation",
          );
          for (const address of pageAddresses(operation.range)) {
            invariant(!mappings.has(address), "commit operation targets a committed mirror page");
            mappings.set(address, {
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
            mapping.protection = { ...operation.protection };
          }
          break;
        case "decommit":
          for (const address of pageAddresses(operation.range)) {
            const mapping = mappings.get(address);
            invariant(mapping !== undefined, "decommit operation targets an unmapped mirror page");
            invariant(mapping.owner === operation.owner, "decommit operation owner mismatch");
            mappings.delete(address);
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
            mappings.delete(address);
            matched = true;
          }
          invariant(matched, "release operation targets an unknown mirror range");
          break;
        }
      }
      invariant(
        mappings.size + directoryIndices.size + 1 <= physicalPoolBytes / memoryPageSize,
        "host delta exceeds the v86 physical mirror pool",
      );
    }

    const dirtyAddresses = new Set<number>();
    for (const page of delta.pages) {
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
          owner: operation.owner,
          kind: operation.kind,
          range: { ...operation.range },
        });
        break;
      case "commit":
        for (const address of pageAddresses(operation.range)) {
          this.#mapPage(address, operation.owner, operation.protection);
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
    for (const page of reflection.pages) {
      directoryIndices.add(page.address >>> 22);
    }
    invariant(
      reflection.pages.length + directoryIndices.size + 1 <=
        physicalPoolBytes / memoryPageSize,
      "reflection exceeds the v86 physical mirror pool",
    );
    for (const page of reflection.pages) {
      invariant((page.address & (memoryPageSize - 1)) === 0, "unaligned reflected page");
      invariant(page.bytes.byteLength === memoryPageSize, "invalid reflected page length");
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
        owner: region.owner,
        kind: region.kind,
        range,
      });
    }
    for (const mapping of this.#mappings.values()) {
      const page = { base: mapping.address, byteLength: memoryPageSize };
      invariant(
        [...this.#reservations.values()].some(
          (reservation) =>
            reservation.owner === mapping.owner && rangeContains(reservation.range, page),
        ),
        "reflected page is outside its owner region",
      );
    }
  }

  #mapPage(address: number, owner: number, protection: MemoryProtection): void {
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
    this.#mappings.set(address, {
      address,
      owner,
      protection: { ...protection },
      physicalBase,
      pteAddress,
    });
  }

  #updateProtection(address: number, protection: MemoryProtection): void {
    const mapping = this.#mappings.get(address)!;
    const dirty = this.#physicalView.getUint32(mapping.pteAddress, true) & pageFlags.dirty;
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
  }

  #unmapPage(address: number): void {
    const mapping = this.#mappings.get(address)!;
    this.#physicalView.setUint32(mapping.pteAddress, 0, true);
    this.#mappings.delete(address);
    this.#releasePage(mapping.physicalBase);
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

  #assertExecutableFetchWindow(instructionPointer: number): void {
    for (let offset = 0; offset < 15; offset += 1) {
      const address = instructionPointer + offset;
      invariant(address <= 0xffff_ffff, "instruction fetch window wraps the guest address space");
      invariant(
        this.#mappingFor(address).protection.execute,
        `execute gate rejected EIP 0x${instructionPointer.toString(16)}`,
      );
    }
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
    invariant(
      this.#nextPhysicalPage + memoryPageSize <= physicalPoolBytes,
      "v86 physical mirror pool exhausted",
    );
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
