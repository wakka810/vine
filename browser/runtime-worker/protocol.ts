export const protocolVersion = 2 as const;

export type SessionMode = "success" | "cancel-pending" | "watchdog";

export interface SessionConfiguration {
  mode: SessionMode;
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
  outcome: "completed" | "cancelled" | "watchdog" | "fault";
  exitCode: number | null;
  elapsedMilliseconds: number;
  instructions: number;
  slices: number;
  budgetYields: number;
  hostRequests: number;
  rustSchedulerTransitions: number;
  callbackEntries: number;
  maximumCallbackDepth: number;
  writebackBatches: number;
  writebackPages: number;
  finalGeneration: number;
  pendingHostCalls: number;
  pendingCallbacks: number;
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
