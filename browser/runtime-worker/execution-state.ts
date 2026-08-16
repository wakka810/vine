export type ExecutionState =
  | "ready"
  | "running"
  | "memory-fault"
  | "budget-yield"
  | "api-trap"
  | "thread-switch"
  | "thread-exit"
  | "waiting-host"
  | "host-completion"
  | "guest-callback"
  | "callback-return"
  | "stopping"
  | "stopped";

export interface ExecutionTransition {
  sequence: number;
  from: ExecutionState;
  to: ExecutionState;
  cause: string;
  threadId: number | null;
  requestId: string | null;
}

export interface ExecutionTransitionContext {
  cause: string;
  threadId?: number | null;
  requestId?: string | null;
}

const allowedTransitions: Readonly<Record<ExecutionState, ReadonlySet<ExecutionState>>> = {
  ready: new Set(["running", "stopping"]),
  running: new Set(["budget-yield", "api-trap", "memory-fault", "thread-exit", "stopping"]),
  "memory-fault": new Set(["ready", "guest-callback", "stopping"]),
  "budget-yield": new Set(["ready", "guest-callback", "stopping"]),
  "api-trap": new Set([
    "ready",
    "thread-switch",
    "thread-exit",
    "waiting-host",
    "guest-callback",
    "stopping",
  ]),
  "thread-switch": new Set(["ready", "guest-callback", "stopping"]),
  "thread-exit": new Set(["thread-switch", "waiting-host", "stopping"]),
  "waiting-host": new Set(["host-completion", "stopping"]),
  "host-completion": new Set(["thread-switch", "waiting-host", "ready", "guest-callback", "stopping"]),
  "guest-callback": new Set([
    "budget-yield",
    "api-trap",
    "memory-fault",
    "callback-return",
    "stopping",
  ]),
  "callback-return": new Set(["ready", "guest-callback", "stopping"]),
  stopping: new Set(["stopped"]),
  stopped: new Set(),
};

export class ExecutionTransitionError extends Error {
  readonly code = "execution-transition-illegal";
  readonly from: ExecutionState;
  readonly to: ExecutionState;

  constructor(from: ExecutionState, to: ExecutionState) {
    super(`illegal execution transition ${from} -> ${to}`);
    this.from = from;
    this.to = to;
  }
}

export class ExecutionStateMachine {
  #state: ExecutionState = "ready";
  #transitionCount = 0;
  #lastTransition: ExecutionTransition | null = null;

  state(): ExecutionState {
    return this.#state;
  }

  transition(to: ExecutionState, context: ExecutionTransitionContext): ExecutionTransition {
    const from = this.#state;
    if (!allowedTransitions[from].has(to)) {
      throw new ExecutionTransitionError(from, to);
    }
    if (context.cause.length === 0 || context.cause.length > 128) {
      throw new Error("execution transition cause is outside the bounded contract");
    }
    const threadId = context.threadId ?? null;
    if (
      threadId !== null &&
      (!Number.isInteger(threadId) || threadId <= 0 || threadId > 0xffff_ffff)
    ) {
      throw new Error("execution transition thread ID is invalid");
    }
    const requestId = context.requestId ?? null;
    if (
      requestId !== null &&
      (!/^(0|[1-9][0-9]*)$/u.test(requestId) ||
        BigInt(requestId) > 0xffff_ffff_ffff_ffffn)
    ) {
      throw new Error("execution transition request ID is invalid");
    }
    const transition = {
      sequence: this.#transitionCount + 1,
      from,
      to,
      cause: context.cause,
      threadId,
      requestId,
    } satisfies ExecutionTransition;
    this.#state = to;
    this.#transitionCount = transition.sequence;
    this.#lastTransition = transition;
    return transition;
  }

  stop(context: ExecutionTransitionContext): void {
    if (this.#state === "stopped") {
      return;
    }
    if (this.#state !== "stopping") {
      this.transition("stopping", context);
    }
    this.transition("stopped", {
      ...context,
      cause: `${context.cause}:stopped`.slice(0, 128),
    });
  }

  snapshot(): {
    state: ExecutionState;
    transitionCount: number;
    lastTransition: ExecutionTransition | null;
  } {
    return {
      state: this.#state,
      transitionCount: this.#transitionCount,
      lastTransition:
        this.#lastTransition === null ? null : structuredClone(this.#lastTransition),
    };
  }
}

export type HostCompletionPolicy = "resume-ready" | "resume-callback";
export type HostRequestCancelState = "active" | "cancelled";

export interface HostDelivery {
  result: number;
  completionPolicy: HostCompletionPolicy;
}

export interface PendingHostRequest {
  requestId: string;
  threadId: number;
  service: string;
  completionPolicy: HostCompletionPolicy;
  cancelState: HostRequestCancelState;
  resolve: (delivery: HostDelivery) => void;
  reject: (error: Error) => void;
}

export interface HostRequestSnapshot {
  requestId: string;
  threadId: number;
  service: string;
  completionPolicy: HostCompletionPolicy;
  cancelState: HostRequestCancelState;
}

export class HostRequestIdentityError extends Error {
  readonly code:
    | "host-request-duplicate"
    | "host-completion-duplicate"
    | "host-completion-late"
    | "host-request-not-pending"
    | "host-request-wrong-thread";

  constructor(code: HostRequestIdentityError["code"], message: string) {
    super(message);
    this.code = code;
  }
}

export class PendingHostRequestTable {
  readonly #requests = new Map<string, PendingHostRequest>();
  readonly #retired = new Map<
    string,
    { threadId: number; disposition: "completed" | "cancelled" }
  >();

  register(request: PendingHostRequest): void {
    this.#validateRequest(request);
    if (this.#requests.has(request.requestId) || this.#retired.has(request.requestId)) {
      throw new HostRequestIdentityError(
        "host-request-duplicate",
        `host request ${request.requestId} is already pending`,
      );
    }
    this.#requests.set(request.requestId, request);
  }

  complete(requestId: string, threadId: number, result: number): PendingHostRequest {
    const request = this.#requests.get(requestId);
    if (request === undefined) {
      const retired = this.#retired.get(requestId);
      if (retired !== undefined && retired.threadId !== threadId) {
        throw new HostRequestIdentityError(
          "host-request-wrong-thread",
          `host request ${requestId} belongs to thread ${retired.threadId}, not ${threadId}`,
        );
      }
      if (retired?.disposition === "completed") {
        throw new HostRequestIdentityError(
          "host-completion-duplicate",
          `host request ${requestId} was already completed`,
        );
      }
      if (retired?.disposition === "cancelled") {
        throw new HostRequestIdentityError(
          "host-completion-late",
          `host request ${requestId} was cancelled before completion`,
        );
      }
      throw new HostRequestIdentityError(
        "host-request-not-pending",
        `host request ${requestId} is not pending`,
      );
    }
    if (request.threadId !== threadId) {
      throw new HostRequestIdentityError(
        "host-request-wrong-thread",
        `host request ${requestId} belongs to thread ${request.threadId}, not ${threadId}`,
      );
    }
    if (!Number.isInteger(result) || result < 0 || result > 0xffff_ffff) {
      throw new Error("host completion result is outside the u32 contract");
    }
    this.#requests.delete(requestId);
    this.#retired.set(requestId, {
      threadId: request.threadId,
      disposition: "completed",
    });
    return request;
  }

  rejectAll(error: Error): number {
    const requests = [...this.#requests.values()];
    for (const request of requests) {
      request.cancelState = "cancelled";
      this.#retired.set(request.requestId, {
        threadId: request.threadId,
        disposition: "cancelled",
      });
    }
    this.#requests.clear();
    for (const request of requests) {
      request.reject(error);
    }
    return requests.length;
  }

  size(): number {
    return this.#requests.size;
  }

  first(): HostRequestSnapshot | null {
    const request = this.#requests.values().next().value as PendingHostRequest | undefined;
    return request === undefined ? null : this.#snapshot(request);
  }

  pending(requestId: string): HostRequestSnapshot | null {
    const request = this.#requests.get(requestId);
    return request === undefined ? null : this.#snapshot(request);
  }

  snapshots(): HostRequestSnapshot[] {
    return [...this.#requests.values()].map((request) => this.#snapshot(request));
  }

  #validateRequest(request: PendingHostRequest): void {
    if (
      !/^(0|[1-9][0-9]*)$/u.test(request.requestId) ||
      BigInt(request.requestId) > 0xffff_ffff_ffff_ffffn
    ) {
      throw new Error("host request ID is outside the bounded u64 contract");
    }
    if (
      !Number.isInteger(request.threadId) ||
      request.threadId <= 0 ||
      request.threadId > 0xffff_ffff
    ) {
      throw new Error("host request thread ID is invalid");
    }
    if (request.service.length === 0 || request.service.length > 128) {
      throw new Error("host request service is outside the bounded contract");
    }
    if (request.cancelState !== "active") {
      throw new Error("new host request must have active cancel state");
    }
  }

  #snapshot(request: PendingHostRequest): HostRequestSnapshot {
    return {
      requestId: request.requestId,
      threadId: request.threadId,
      service: request.service,
      completionPolicy: request.completionPolicy,
      cancelState: request.cancelState,
    };
  }
}
