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
  | { kind: "runtime-output" }
  | { kind: "timestamp-counter" }
  | { kind: "trap"; trap: "invalid-opcode"; errorCode: number }
  | {
      kind: "fault";
      fault:
        | "divide-error"
        | "floating-point"
        | "simd-floating-point"
        | "general-protection"
        | "stack-segment"
        | "page-fault"
        | "execute-protection"
        | "engine-stop";
      errorCode: number;
      engineReason: number;
      faultAddress?: number;
    }
  | { kind: "halt" };

export type ExecutionBackendSnapshot =
  | { backend: "v86"; abiVersion: 1; words: Uint32Array }
  | { backend: "recompiler"; abiVersion: 2; words: Uint32Array };

// The snapshot belongs to one backend. Runtime consumes only this materialized
// architectural projection; it never interprets the backend's private words.
export interface ExecutionCpuContext {
  snapshot: ExecutionBackendSnapshot;
  eax: number;
  ecx: number;
  edx: number;
  ebx: number;
  esp: number;
  ebp: number;
  esi: number;
  edi: number;
  instructionPointer: number;
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
  backend?: "v86" | "recompiler";
  /** Approximate binary64 x87; defaults to fast on capable CPUs, strict on older pins. */
  v86NumericMode?: "strict" | "fast";
  /** Native unit chaining, enabled by default when supported by the compiler. */
  recompilerLinkedRegions?: boolean;
  /** Explicit x87 state lowering; enabled by default and required for approximation. */
  recompilerFpIr?: boolean;
  /** Fixed-operation numerical imports; enabled by default when FP IR is enabled. */
  recompilerTypedNumeric?: boolean;
  /** Defaults to balanced. Memory, stack, fault and durable-save contracts remain shared. */
  recompilerNumericMode?: "strict" | "balanced" | "fast";
  /** Experimental scalar value/effect IR; precise architectural exits are unchanged. */
  recompilerValueIr?: boolean;
  /** Numeric predicate/scalar IR with state-carrying internal continuations. */
  recompilerPredicateIr?: boolean;
  /** Whole-unit SSA with unrestricted ingress and effect-local recovery. */
  recompilerFlowIr?: boolean;
  /** Experimental single-root CFG SSA; every external PC uses an ordinary rooted version. */
  recompilerRootedSsa?: boolean;
  /** Experimental bounded cache retention; default capacity stays unchanged. */
  recompilerExpandedCache?: boolean;
  /** Reuse code content independently of immutable per-instance unit bindings. */
  recompilerBindingGlobal?: boolean;
  /** Opt-in coherent Rust-owned target directory; requires linked regions. */
  recompilerDirectLinks?: boolean;
  recompilerControlGraph?: boolean;
  /** Page-bounded loop memory proofs; Runtime ownership and precise exits are unchanged. */
  recompilerLoopMemory?: boolean;
  /** Experimental private tail-call CFG partitions; public CPU ABI is unchanged. */
  recompilerFunctionCfg?: boolean;
}

export interface ExecutionEngine {
  initialize(reflection: MemoryReflection, bootstrap: ExecutionBootstrap): void;
  // Optional compilation/Chrome instantiation only; never executes guest code.
  prepareExecution?(entries: number[]): Promise<{ milliseconds: number; limited: boolean }>;
  rebuildMemory(reflection: MemoryReflection, regions: MemoryRegionSnapshot[]): void;
  executeBudget(instructionBudget: number): ExecutionSlice;
  // Unified execution can finish synchronous APIs before returning a slice.
  setRuntimeClock?(retiredInstructions: number, elapsedMilliseconds: number): void;
  memoryGeneration?(): bigint | undefined;
  // Completes one stopped RDTSC after Runtime has accepted the real host clock.
  completeTimestampCounter?(instructionPointer: number, low: number, high: number): void;
  createCpuContext(bootstrap: ExecutionThreadBootstrap): ExecutionCpuContext;
  saveCpuContext(): ExecutionCpuContext;
  restoreCpuContext(context: Pick<ExecutionCpuContext, "snapshot">): void;
  applyCpuResume(resume: CpuResume): void;
  applyStoppedCpuResume(resume: CpuResume): void;
  enterGuestCallback(target: number, stackPointer: number): void;
  readGuestU32(address: number): number;
  // A borrowed view is read-only and valid only until the next runtime/engine call.
  // Consume it synchronously; never retain, mutate or transfer its backing buffer.
  readGuestBytes(address: number, byteLength: number, borrow?: boolean): Uint8Array;
  collectChangedPages(): DirtyMirrorPage[];
  collectChangedPageViews(): DirtyMirrorPage[];
  // Called only after successful whole-batch Runtime reconciliation. A rejected
  // writeback must leave the execution mirror's dirty state pending.
  acknowledgeWriteback?(generation: bigint): void;
  applyMemoryDelta(delta: HostMemoryDelta): void;
  applyCacheEffects(effects: CacheEffects): void;
}
