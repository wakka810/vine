import type {
  CacheEffects,
  DirtyMirrorPage,
  HostMemoryDelta,
  MemoryReflection,
  MemoryRegionSnapshot,
} from "./memory-transport.js";

export interface ExecutionEngineIdentity {
  schema: "vine.execution-engine-profile";
  schemaVersion: 1;
  engine: string;
  sourceRevision: string;
  cpuContextAbi: {
    version: number;
    bytes: number;
  };
  artifact: {
    bytes: number;
    sha256: string;
  };
}

export interface ExecutionBootstrap {
  entryPoint: number;
  initialStackPointer: number;
  fsBase?: number;
  regions: MemoryRegionSnapshot[];
}

export interface ExecutionThreadBootstrap {
  entryPoint: number;
  initialStackPointer: number;
  fsBase?: number;
}

export type ExecutionStop =
  | { kind: "budget-yield" }
  | { kind: "trap"; trap: "invalid-opcode"; errorCode: number }
  | {
      kind: "fault";
      fault:
        | "divide-error"
        | "general-protection"
        | "page-fault"
        | "execute-protection"
        | "engine-stop";
      errorCode: number;
      engineReason: number;
      faultAddress?: number;
    }
  | { kind: "halt" };

export interface ExecutionCpuContext {
  version: number;
  byteLength: number;
  opaqueWords: Uint32Array;
  eax: number;
  esp: number;
  instructionPointer: number;
  previousInstructionPointer: number;
}

export interface ExecutionSlice {
  stop: ExecutionStop;
  instructions: number;
  context: ExecutionCpuContext;
}

export type CpuRegisterUpdate =
  | { action: "preserve" }
  | { action: "set"; value: number };

export interface CpuResume {
  instructionPointer: number;
  stackPointer: number;
  eax: CpuRegisterUpdate;
  edx: CpuRegisterUpdate;
  eflags: CpuRegisterUpdate;
}

export interface ExecutionCacheEffectCounts {
  tlbFlushes: number;
  instructionFetchInvalidations: number;
  generatedCodeInvalidations: number;
}

export interface ExecutionEngineOptions {
  maximumPhysicalPages: number;
}

export interface ExecutionEngineMemoryCapacity {
  initialPhysicalPoolBytes: number;
  physicalPoolBytes: number;
  maximumPhysicalPoolBytes: number;
  maximumPhysicalPages: number;
  mappedGuestPages: number;
  pageTablePages: number;
  freePhysicalPages: number;
  growths: number;
  releasedPages: number;
  reusedPages: number;
}

export type ExecutionEngineFactory = (options: ExecutionEngineOptions) => ExecutionEngine;

export interface ExecutionEngine {
  identity(): ExecutionEngineIdentity;
  initialize(reflection: MemoryReflection, bootstrap: ExecutionBootstrap): void;
  rebuildMemory(reflection: MemoryReflection, regions: MemoryRegionSnapshot[]): void;
  memoryContractState(): string;
  cacheEffectCounts(): Readonly<ExecutionCacheEffectCounts>;
  memoryCapacity(): Readonly<ExecutionEngineMemoryCapacity>;
  linearMemoryBytes(): number;
  executeBudget(instructionBudget: number): ExecutionSlice;
  createCpuContext(bootstrap: ExecutionThreadBootstrap): ExecutionCpuContext;
  saveCpuContext(): ExecutionCpuContext;
  restoreCpuContext(context: ExecutionCpuContext): void;
  applyCpuResume(resume: CpuResume): void;
  enterGuestCallback(target: number, stackPointer: number): void;
  readGuestU32(address: number): number;
  collectChangedPages(): DirtyMirrorPage[];
  collectChangedPageViews(): DirtyMirrorPage[];
  applyMemoryDelta(delta: HostMemoryDelta): void;
  applyCacheEffects(effects: CacheEffects): void;
}

export class ExecutionMemoryAccessFault extends Error {
  readonly code: "execution-address-unmapped" | "execution-read-protection";
  readonly address: number;

  constructor(
    code: "execution-address-unmapped" | "execution-read-protection",
    address: number,
    message: string,
  ) {
    super(message);
    this.name = "ExecutionMemoryAccessFault";
    this.code = code;
    this.address = address;
  }
}
