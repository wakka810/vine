import type { ExecutionState, ExecutionTransition } from "./execution-state.js";
import type { ExecutionEngineIdentity } from "./execution-engine-contract.js";

export const protocolVersion = 4 as const;

export type SessionMode = "success" | "cancel-pending" | "watchdog";
export type SessionWorkload = "controlled-fixture" | "private-bgi";

export interface SessionConfiguration {
  mode: SessionMode;
  workload?: SessionWorkload;
  executableUrl?: string;
  policyUrl?: string;
  instructionBudget: number;
  watchdogInstructionLimit: number;
  watchdogWallMilliseconds: number;
}

export interface StartSessionMessage {
  type: "start-session";
  protocolVersion: typeof protocolVersion;
  sessionId: number;
  configuration: SessionConfiguration;
}

export interface CancelSessionMessage {
  type: "cancel-session";
  protocolVersion: typeof protocolVersion;
  sessionId: number;
}

export interface HostCompletionMessage {
  type: "host-completion";
  protocolVersion: typeof protocolVersion;
  sessionId: number;
  requestId: string;
  threadId: number;
  result: number;
}

export type MainToWorkerMessage =
  | StartSessionMessage
  | CancelSessionMessage
  | HostCompletionMessage;

export interface SessionStatusMessage {
  type: "session-status";
  protocolVersion: typeof protocolVersion;
  sessionId: number;
  phase: string;
  detail: string;
}

export interface HostRequestMessage {
  type: "host-request";
  protocolVersion: typeof protocolVersion;
  sessionId: number;
  requestId: string;
  threadId: number;
  service: string;
  arguments: number[];
}

export interface SessionReport {
  outcome: "completed" | "structured-stop" | "cancelled" | "watchdog" | "fault";
  exitCode: number | null;
  elapsedMilliseconds: number;
  instructions: number;
  slices: number;
  budgetYields: number;
  hostRequests: number;
  rustSchedulerTransitions: number;
  executionState: ExecutionState;
  executionTransitionCount: number;
  lastExecutionTransition: ExecutionTransition | null;
  pendingHostRequestRecords: number;
  watchdogContext: {
    threadId: number;
    requestId: string | null;
    lastTransition: ExecutionTransition | null;
  } | null;
  callbackEntries: number;
  maximumCallbackDepth: number;
  writebackBatches: number;
  writebackPages: number;
  mappingDeltas: number;
  mappingOperations: {
    reserve: number;
    commit: number;
    protect: number;
    decommit: number;
    release: number;
  };
  mappingDirtyPages: number;
  mappingAtomicRejections: number;
  mappingStaleAccessStops: number;
  mappingCacheEffects: {
    tlbFlushes: number;
    instructionFetchInvalidations: number;
    generatedCodeInvalidations: number;
  };
  mirrorRebuilds: number;
  initialGeneration: number;
  finalGeneration: number;
  pendingHostCalls: number;
  pendingCallbacks: number;
  maximumSliceMilliseconds: number;
  wasmBridgeCalls: number;
  runtimeCoreLinearMemoryBytes: number;
  executionEngine: ExecutionEngineIdentity | null;
  executionEngineLinearMemoryBytes: number;
  supportedCalls: Array<{
    sequence: number;
    module: string;
    symbol: string;
    arguments: number[];
    result: { action: "preserve" } | { action: "set"; value: number };
    implementationId: string;
    returnAddress: number;
    stackPointerBefore: number;
    stackPointerAfter: number;
    lastError: number;
    guestWrites: Array<{ address: number; bytes: number[] }>;
  }>;
  firstUnsupported: {
    capabilityCluster: string;
    endpoint: unknown;
    fault: unknown;
    instructionCount: number;
    memoryContentSha256: string;
  } | null;
  error: string | null;
}

export interface SessionFinishedMessage {
  type: "session-finished";
  protocolVersion: typeof protocolVersion;
  sessionId: number;
  report: SessionReport;
}

export type WorkerToMainMessage =
  | SessionStatusMessage
  | HostRequestMessage
  | SessionFinishedMessage;

export function assertProtocolVersion(version: number): asserts version is typeof protocolVersion {
  if (version !== protocolVersion) {
    throw new Error(`unsupported browser runtime protocol version ${version}`);
  }
}
