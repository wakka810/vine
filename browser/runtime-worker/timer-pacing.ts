export type PageLifecycleState = "active" | "frozen";

export interface PageLifecycleSnapshot {
  state: PageLifecycleState;
  generation: number;
}

export interface TimerPacingDriver {
  now(): number;
  setTimeout(callback: () => void, delayMilliseconds: number): number;
  clearTimeout(handle: number): void;
}

export interface TimerPacingTaskSource {
  now(): number;
  postTask(): void;
  subscribe(callback: () => void): void;
}

interface PendingTaskTimer {
  callback: () => void;
  deadlineMilliseconds: number;
}

export class TaskTimerPacingDriver implements TimerPacingDriver {
  readonly #source: TimerPacingTaskSource;
  readonly #pending = new Map<number, PendingTaskTimer>();
  #nextHandle = 1;
  #taskPosted = false;

  constructor(source: TimerPacingTaskSource) {
    this.#source = source;
    source.subscribe(() => this.#runTask());
  }

  now(): number {
    return this.#source.now();
  }

  setTimeout(callback: () => void, delayMilliseconds: number): number {
    if (
      !Number.isFinite(delayMilliseconds) ||
      delayMilliseconds < 0 ||
      delayMilliseconds > 10
    ) {
      throw new Error("task timer delay is outside the bounded 0..10ms policy");
    }
    const handle = this.#nextHandle;
    this.#nextHandle += 1;
    if (!Number.isSafeInteger(this.#nextHandle)) {
      throw new Error("task timer handle space is exhausted");
    }
    this.#pending.set(handle, {
      callback,
      deadlineMilliseconds: this.now() + delayMilliseconds,
    });
    this.#postTask();
    return handle;
  }

  clearTimeout(handle: number): void {
    this.#pending.delete(handle);
  }

  #postTask(): void {
    if (this.#taskPosted || this.#pending.size === 0) {
      return;
    }
    this.#taskPosted = true;
    this.#source.postTask();
  }

  #runTask(): void {
    if (!this.#taskPosted) {
      throw new Error("task timer received an unowned task");
    }
    this.#taskPosted = false;
    const now = this.now();
    const due = [...this.#pending.entries()]
      .filter(([, timer]) => timer.deadlineMilliseconds <= now)
      .sort(
        (left, right) =>
          left[1].deadlineMilliseconds - right[1].deadlineMilliseconds ||
          left[0] - right[0],
      );
    for (const [handle, timer] of due) {
      if (!this.#pending.delete(handle)) {
        continue;
      }
      timer.callback();
    }
    this.#postTask();
  }
}

export interface TimerPacingRequest {
  requestId: string;
  threadId: number;
  delayMilliseconds: number;
  complete(): void;
}

export interface TimerPacingMetrics {
  requests: number;
  completions: number;
  cancellations: number;
  freezes: number;
  resumes: number;
  staleCallbacks: number;
  boundedCatchUps: number;
  pending: number;
}

interface PendingTimerPacing {
  request: TimerPacingRequest;
  token: number;
  remainingMilliseconds: number;
  startedAtMilliseconds: number | null;
  timeoutHandle: number | null;
  catchUp: boolean;
}

export class TimerPacingService {
  readonly #driver: TimerPacingDriver;
  readonly #pending = new Map<string, PendingTimerPacing>();
  #lifecycle: PageLifecycleSnapshot;
  #nextToken = 1;
  #metrics: Omit<TimerPacingMetrics, "pending"> = {
    requests: 0,
    completions: 0,
    cancellations: 0,
    freezes: 0,
    resumes: 0,
    staleCallbacks: 0,
    boundedCatchUps: 0,
  };

  constructor(driver: TimerPacingDriver, lifecycle: PageLifecycleSnapshot) {
    this.#driver = driver;
    this.#validateLifecycle(lifecycle);
    this.#lifecycle = { ...lifecycle };
  }

  schedule(request: TimerPacingRequest): void {
    if (
      !/^(0|[1-9][0-9]*)$/u.test(request.requestId) ||
      BigInt(request.requestId) > 0xffff_ffff_ffff_ffffn
    ) {
      throw new Error("timer pacing request ID is outside the bounded u64 contract");
    }
    if (
      !Number.isInteger(request.threadId) ||
      request.threadId <= 0 ||
      request.threadId > 0xffff_ffff
    ) {
      throw new Error("timer pacing thread ID is invalid");
    }
    if (
      !Number.isInteger(request.delayMilliseconds) ||
      request.delayMilliseconds <= 0 ||
      request.delayMilliseconds > 10
    ) {
      throw new Error("timer pacing delay is outside the bounded 1..10ms policy");
    }
    if (this.#pending.has(request.requestId)) {
      throw new Error(`timer pacing request ${request.requestId} is already pending`);
    }
    const pending: PendingTimerPacing = {
      request,
      token: this.#allocateToken(),
      remainingMilliseconds: request.delayMilliseconds,
      startedAtMilliseconds: null,
      timeoutHandle: null,
      catchUp: false,
    };
    this.#pending.set(request.requestId, pending);
    this.#metrics.requests += 1;
    if (this.#lifecycle.state === "active") {
      this.#arm(pending, false);
    }
  }

  updateLifecycle(lifecycle: PageLifecycleSnapshot): void {
    this.#validateLifecycle(lifecycle);
    if (lifecycle.generation <= this.#lifecycle.generation) {
      throw new Error("timer pacing page lifecycle generation is stale");
    }
    const previous = this.#lifecycle;
    this.#lifecycle = { ...lifecycle };
    if (previous.state === lifecycle.state) {
      return;
    }
    if (lifecycle.state === "frozen") {
      this.#metrics.freezes += 1;
      const now = this.#driver.now();
      for (const pending of this.#pending.values()) {
        if (pending.timeoutHandle === null || pending.startedAtMilliseconds === null) {
          continue;
        }
        this.#driver.clearTimeout(pending.timeoutHandle);
        const elapsed = Math.max(0, now - pending.startedAtMilliseconds);
        pending.remainingMilliseconds = Math.max(0, pending.remainingMilliseconds - elapsed);
        pending.startedAtMilliseconds = null;
        pending.timeoutHandle = null;
        pending.catchUp = false;
        pending.token = this.#allocateToken();
      }
      return;
    }
    this.#metrics.resumes += 1;
    this.#armPendingAfterResume();
  }

  cancelAll(): number {
    const pending = [...this.#pending.values()];
    for (const record of pending) {
      if (record.timeoutHandle !== null) {
        this.#driver.clearTimeout(record.timeoutHandle);
      }
      record.token = this.#allocateToken();
    }
    this.#pending.clear();
    this.#metrics.cancellations += pending.length;
    return pending.length;
  }

  metrics(): TimerPacingMetrics {
    return {
      ...this.#metrics,
      pending: this.#pending.size,
    };
  }

  #armPendingAfterResume(): void {
    let catchUpArmed = false;
    for (const pending of this.#pending.values()) {
      if (pending.remainingMilliseconds <= 0) {
        if (!catchUpArmed) {
          this.#arm(pending, true);
          catchUpArmed = true;
        }
      } else {
        this.#arm(pending, false);
      }
    }
  }

  #arm(pending: PendingTimerPacing, catchUp: boolean): void {
    if (pending.timeoutHandle !== null || this.#lifecycle.state !== "active") {
      return;
    }
    const token = this.#allocateToken();
    pending.token = token;
    pending.startedAtMilliseconds = this.#driver.now();
    pending.catchUp = catchUp;
    pending.timeoutHandle = this.#driver.setTimeout(
      () => this.#complete(pending.request.requestId, token),
      catchUp ? 0 : Math.ceil(pending.remainingMilliseconds),
    );
  }

  #complete(requestId: string, token: number): void {
    const pending = this.#pending.get(requestId);
    if (
      pending === undefined ||
      pending.token !== token ||
      pending.timeoutHandle === null ||
      this.#lifecycle.state !== "active"
    ) {
      this.#metrics.staleCallbacks += 1;
      return;
    }
    pending.timeoutHandle = null;
    pending.startedAtMilliseconds = null;
    this.#pending.delete(requestId);
    this.#metrics.completions += 1;
    if (pending.catchUp) {
      this.#metrics.boundedCatchUps += 1;
    }
    pending.request.complete();
    if (pending.catchUp && this.#lifecycle.state === "active") {
      this.#armPendingAfterResume();
    }
  }

  #allocateToken(): number {
    const token = this.#nextToken;
    this.#nextToken += 1;
    if (!Number.isSafeInteger(this.#nextToken)) {
      throw new Error("timer pacing callback token space is exhausted");
    }
    return token;
  }

  #validateLifecycle(lifecycle: PageLifecycleSnapshot): void {
    if (
      (lifecycle.state !== "active" && lifecycle.state !== "frozen") ||
      !Number.isSafeInteger(lifecycle.generation) ||
      lifecycle.generation <= 0
    ) {
      throw new Error("timer pacing page lifecycle snapshot is invalid");
    }
  }
}
