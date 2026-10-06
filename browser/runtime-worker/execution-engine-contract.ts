import type {
  CacheEffects,
  DirtyMirrorPage,
  HostMemoryDelta,
  MemoryReflection,
  MemoryRegionSnapshot,
} from "./memory-transport.js";

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
}

export interface CpuResume {
  instructionPointer: number;
  stackPointer: number;
  eax: number | null;
}

export interface ExecutionEngineOptions {
  maximumPhysicalPages: number;
}

export interface ExecutionEngine {
  initialize(reflection: MemoryReflection, bootstrap: ExecutionBootstrap): void;
  rebuildMemory(reflection: MemoryReflection, regions: MemoryRegionSnapshot[]): void;
  executeBudget(instructionBudget: number): ExecutionSlice;
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
