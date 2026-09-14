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
  | { kind: "runtime-dispatch" }
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
  ecx: number;
  edx: number;
  ebx: number;
  esp: number;
  ebp: number;
  esi: number;
  edi: number;
  instructionPointer: number;
  previousInstructionPointer: number;
  eflags: number;
  es: number;
  cs: number;
  ss: number;
  ds: number;
  fs: number;
  gs: number;
}

export interface ExecutionSlice {
  stop: ExecutionStop;
  instructions: number;
  context: ExecutionCpuContext;
  executionIsland?: {
    fastApiCompletions: number;
    writebackBatches: number;
    writebackPages: number;
    mirroredDeltaPages: number;
    nativeHeapPages?: number;
    generation: bigint;
  };
}

export interface ExecutionClockSample {
  retiredInstructions: number;
  hostElapsedTime100ns: bigint;
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
  disableJit?: boolean;
  relaxJitInstructionBudget?: boolean;
  jitHotnessThreshold?: number;
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

export interface ExecutionEnginePerformanceMetrics {
  jitCompilations: number;
  jitClears: number;
  jitActiveFunctions: number;
  jitInstructions: number;
  interpretedInstructions: number;
}

export type ExecutionEngineFactory = (options: ExecutionEngineOptions) => ExecutionEngine;

export interface ExecutionEngine {
  identity(): ExecutionEngineIdentity;
  initialize(reflection: MemoryReflection, bootstrap: ExecutionBootstrap): void;
  rebuildMemory(reflection: MemoryReflection, regions: MemoryRegionSnapshot[]): void;
  memoryContractState(): string;
  cacheEffectCounts(): Readonly<ExecutionCacheEffectCounts>;
  performanceMetrics(): Readonly<ExecutionEnginePerformanceMetrics>;
  memoryCapacity(): Readonly<ExecutionEngineMemoryCapacity>;
  linearMemoryBytes(): number;
  executeBudget(instructionBudget: number, clock?: ExecutionClockSample): ExecutionSlice;
  createCpuContext(bootstrap: ExecutionThreadBootstrap): ExecutionCpuContext;
  saveCpuContext(): ExecutionCpuContext;
  restoreCpuContext(context: ExecutionCpuContext): void;
  applyCpuResume(resume: CpuResume): void;
  applyStoppedCpuResume(resume: CpuResume): void;
  enterGuestCallback(target: number, stackPointer: number): void;
  readGuestU32(address: number): number;
  // A borrowed view is read-only and valid only until the next runtime/engine call.
  // Consume it synchronously; never retain, mutate or transfer its backing buffer.
  readGuestBytes(address: number, byteLength: number, borrow?: boolean): Uint8Array;
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
