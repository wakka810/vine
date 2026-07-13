/// <reference lib="webworker" />

import {
  assertProtocolVersion,
  type HostCompletionMessage,
  type HostRequestMessage,
  type MainToWorkerMessage,
  protocolVersion,
  type SessionConfiguration,
  type SessionFinishedMessage,
  type SessionReport,
  type WorkerToMainMessage,
} from "./protocol.js";
import {
  decodeHostDelta,
  decodeReflection,
  encodeCpuWriteback,
  type MemoryRegionSnapshot,
} from "./memory-transport.js";
import {
  type ApiCompletion,
  type BridgeOutput,
  type PreparedMetadata,
  RuntimeBridge,
} from "./runtime-bridge.js";
import { MirrorAccessFault, V86Mirror, v86StopReason } from "./v86-mirror.js";

const worker = self as DedicatedWorkerGlobalScope;

interface HostRequestMetadata {
  kind: "host-request";
  requestId: string;
  threadId: number;
  service: string;
  arguments: number[];
  scheduler: {
    state: "waiting-host";
    requestId: string;
  };
}

interface GuestCallbackMetadata {
  kind: "guest-callback";
  frameId: number;
  depth: number;
  target: number;
  stackPointer: number;
  returnSentinel: number;
  generation: number;
}

interface CompletionMetadata {
  kind: "api-complete";
  completion: ApiCompletion;
  endpoint?: {
    id: number;
    address: number;
    module: string;
    symbol: { kind: "name"; name: string } | { kind: "ordinal"; ordinal: number };
  };
  memory?: { generation: number; acknowledgedGeneration: number };
}

interface ApiFaultMetadata {
  kind: "api-fault";
  endpoint: {
    id: number;
    address: number;
    module: string;
    symbol: { kind: "name"; name: string } | { kind: "ordinal"; ordinal: number };
  };
  capabilityCluster: string;
  fault: unknown;
  memory: { generation: number; acknowledgedGeneration: number };
  memoryContentSha256: string;
}

interface HostCompletionMetadata {
  kind: "host-complete";
  threadId: number;
  scheduler: { state: "runnable" } | { state: "in-callback"; depth: number };
  completion: ApiCompletion;
}

interface CallbackCompletionMetadata {
  kind: "callback-complete";
  frameId: number;
  remainingDepth: number;
  completion: ApiCompletion;
}

interface WritebackMetadata {
  kind: "writeback-reconciled";
  receipt: {
    generation: number;
    touchedPages: number;
  };
}

interface MappingFixtureMetadata {
  kind: "mapping-fixture-delta";
  stageIndex: number;
  stage: string;
  fromGeneration: number;
  generation: number;
  mappingOperations: Array<"reserve" | "commit" | "protect" | "decommit" | "release">;
  dirtyPages: number;
  cacheEffects: {
    flushTlb: boolean;
    invalidateInstructionFetch: boolean;
    invalidateGeneratedCode: boolean;
  };
  probeAddress: number;
}

interface CurrentReflectionMetadata {
  kind: "current-reflection";
  generation: number;
  acknowledgedGeneration: number;
  pageCount: number;
  regions: MemoryRegionSnapshot[];
}

interface CancellationMetadata {
  kind: "cancelled";
  cancellation: {
    hostRequestsCancelled: number;
    callbackFramesCancelled: number;
  };
  pendingHostCalls: number;
  pendingCallbacks: number;
  processLifecycle: { state: string; exitCode?: number };
  dllLifecycle: string;
}

interface ProcessExitMetadata {
  kind: "process-exit";
  exitCode: number;
  pendingHostCalls: number;
  pendingCallbacks: number;
}

class SessionStopped extends Error {}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function post(message: WorkerToMainMessage): void {
  worker.postMessage(message);
}

function yieldToWorkerLoop(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

class BrowserRuntimeSession {
  readonly #sessionId: number;
  readonly #configuration: SessionConfiguration;
  readonly #startedAt = performance.now();
  #bridge: RuntimeBridge | null = null;
  #mirror: V86Mirror | null = null;
  #prepared: PreparedMetadata | null = null;
  #generation = 0n;
  #terminal = false;
  #runtimePrepared = false;
  #pendingHostDelivery:
    | {
        requestId: string;
        threadId: number;
        resolve: (result: number) => void;
        reject: (error: Error) => void;
      }
    | null = null;
  readonly #report = {
    instructions: 0,
    slices: 0,
    budgetYields: 0,
    hostRequests: 0,
    rustSchedulerTransitions: 0,
    callbackEntries: 0,
    maximumCallbackDepth: 0,
    writebackBatches: 0,
    writebackPages: 0,
    mappingDeltas: 0,
    mappingOperations: {
      reserve: 0,
      commit: 0,
      protect: 0,
      decommit: 0,
      release: 0,
    },
    mappingDirtyPages: 0,
    mappingAtomicRejections: 0,
    mappingStaleAccessStops: 0,
    mappingCacheEffects: {
      tlbFlushes: 0,
      instructionFetchInvalidations: 0,
      generatedCodeInvalidations: 0,
    },
    mirrorRebuilds: 0,
    initialGeneration: 0,
    maximumSliceMilliseconds: 0,
    supportedCalls: [] as SessionReport["supportedCalls"],
    firstUnsupported: null as SessionReport["firstUnsupported"],
  };

  constructor(sessionId: number, configuration: SessionConfiguration) {
    this.#sessionId = sessionId;
    this.#configuration = configuration;
  }

  async run(): Promise<void> {
    try {
      const workload = this.#configuration.workload ?? "controlled-fixture";
      this.#status("loading", `Loading Rust runtime, v86 adapter, and ${workload}`);
      const coreUrl = new URL("./runtime-core.wasm", import.meta.url);
      const adapterUrl = new URL("./v86-adapter.wasm", import.meta.url);
      const executableUrl = new URL(
        workload === "private-bgi"
          ? this.#configuration.executableUrl ?? "./private/BGI.exe"
          : "./browser-vertical.exe",
        import.meta.url,
      );
      const [bridge, mirror, executableResponse] = await Promise.all([
        RuntimeBridge.instantiate(coreUrl),
        V86Mirror.instantiate(adapterUrl),
        fetch(executableUrl),
      ]);
      invariant(
        executableResponse.ok,
        `executable fetch failed with ${executableResponse.status}`,
      );
      if (this.#terminal) {
        return;
      }
      this.#bridge = bridge;
      this.#mirror = mirror;
      const executable = new Uint8Array(await executableResponse.arrayBuffer());
      const preparedOutput =
        workload === "private-bgi"
          ? bridge.prepareBgi(
              executable,
              await this.#fetchPolicy(
                new URL(
                  this.#configuration.policyUrl ?? "./private/kernel32-policy.json",
                  import.meta.url,
                ),
              ),
            )
          : bridge.prepare(executable);
      this.#runtimePrepared = true;
      this.#prepared = preparedOutput.metadata;
      invariant(
        (preparedOutput.metadata.workload ?? "controlled-fixture") === workload,
        "prepared workload mismatch",
      );
      const reflection = decodeReflection(preparedOutput.control, preparedOutput.payload);
      invariant(
        reflection.generation === BigInt(preparedOutput.metadata.generation),
        "Rust reflection generation metadata mismatch",
      );
      invariant(
        reflection.pages.length === preparedOutput.metadata.pageCount,
        "Rust reflection page count metadata mismatch",
      );
      mirror.install(reflection, preparedOutput.metadata);
      bridge.acknowledge(reflection.generation);
      this.#generation = reflection.generation;
      this.#report.initialGeneration = Number(reflection.generation);
      if (workload === "controlled-fixture") {
        this.#runMappingContract();
      }
      this.#status("running", "Guest entry is running in bounded Worker slices");
      await this.#runGuest();
    } catch (error) {
      if (error instanceof SessionStopped && this.#terminal) {
        return;
      }
      this.#fail(error instanceof Error ? error : new Error(String(error)));
    }
  }

  async #fetchPolicy(url: URL): Promise<Uint8Array> {
    const response = await fetch(url);
    invariant(response.ok, `runtime policy fetch failed with ${response.status}`);
    return new Uint8Array(await response.arrayBuffer());
  }

  completeHost(message: HostCompletionMessage): void {
    if (this.#terminal || this.#pendingHostDelivery === null) {
      return;
    }
    if (
      message.requestId !== this.#pendingHostDelivery.requestId ||
      message.threadId !== this.#pendingHostDelivery.threadId
    ) {
      this.#pendingHostDelivery.reject(
        new Error("host completion transport identity mismatch"),
      );
      this.#pendingHostDelivery = null;
      return;
    }
    const pending = this.#pendingHostDelivery;
    this.#pendingHostDelivery = null;
    pending.resolve(message.result >>> 0);
  }

  cancel(): void {
    this.#terminate("cancelled", "Session cancelled by user");
  }

  async #runGuest(): Promise<void> {
    const mirror = this.#requireMirror();
    const prepared = this.#requirePrepared();
    const endpointAddresses = new Set(prepared.endpoints.map(({ address }) => address));
    while (!this.#terminal) {
      this.#checkWatchdog();
      const sliceStartedAt = performance.now();
      const slice = mirror.execute(this.#configuration.instructionBudget);
      this.#report.maximumSliceMilliseconds = Math.max(
        this.#report.maximumSliceMilliseconds,
        performance.now() - sliceStartedAt,
      );
      this.#report.instructions += slice.instructions;
      this.#report.slices += 1;
      this.#reconcileDirtyPages();
      this.#checkWatchdog();

      if (slice.reason === v86StopReason.budget) {
        this.#report.budgetYields += 1;
        if (this.#report.budgetYields % 16 === 0) {
          this.#status(
            "running",
            `${this.#report.instructions.toLocaleString()} guest instructions; periodic yield`,
          );
        }
        await yieldToWorkerLoop();
        continue;
      }

      invariant(
        slice.reason === v86StopReason.invalidOpcode && slice.errorCode === -1,
        `guest stopped unexpectedly: reason=${slice.reason}, error=${slice.errorCode}`,
      );
      invariant(
        slice.state.instructionPointer === slice.state.previousInstructionPointer,
        "trap instruction pointer provenance mismatch",
      );
      const address = slice.state.instructionPointer;
      if (endpointAddresses.has(address)) {
        await this.#dispatchApi(address, slice.state.esp);
      } else if (address === prepared.callbackSentinel) {
        this.#completeCallback(slice.state.eax);
      } else if (address === prepared.entrySentinel) {
        this.#completeProcess(slice.state.eax);
      } else {
        throw new Error(`guest reached unowned #UD at 0x${address.toString(16)}`);
      }
      await yieldToWorkerLoop();
    }
  }

  async #dispatchApi(endpoint: number, stackPointer: number): Promise<void> {
    const mirror = this.#requireMirror();
    const returnAddress = mirror.readU32(stackPointer);
    const output = this.#requireBridge().dispatch<
      HostRequestMetadata | GuestCallbackMetadata | CompletionMetadata | ApiFaultMetadata
    >(endpoint, stackPointer, returnAddress);
    const metadata = output.metadata;
    switch (metadata.kind) {
      case "host-request": {
        invariant(output.control.byteLength === 0, "host request unexpectedly returned a delta");
        this.#report.hostRequests += 1;
        this.#status("waiting-host", metadata.service);
        const result = await this.#waitForHost(metadata);
        if (this.#terminal) {
          throw new SessionStopped("session ended while waiting for the host");
        }
        const completion = this.#requireBridge().completeHost<HostCompletionMetadata>(
          BigInt(metadata.requestId),
          result,
        ).metadata;
        invariant(completion.kind === "host-complete", "invalid host completion metadata");
        invariant(
          completion.threadId === metadata.threadId,
          "Rust resumed a different guest thread",
        );
        invariant(
          completion.scheduler.state === "runnable" ||
            completion.scheduler.state === "in-callback",
          "Rust did not authorize guest resume after host completion",
        );
        this.#report.rustSchedulerTransitions += 1;
        mirror.applyCompletion(completion.completion);
        this.#status("running", `${metadata.service} completed`);
        break;
      }
      case "guest-callback": {
        this.#applyRequiredDelta(output);
        this.#report.callbackEntries += 1;
        this.#report.maximumCallbackDepth = Math.max(
          this.#report.maximumCallbackDepth,
          metadata.depth,
        );
        mirror.enterCallback(metadata.target, metadata.stackPointer);
        this.#status("callback", `Guest callback depth ${metadata.depth}`);
        break;
      }
      case "api-complete": {
        this.#applyOptionalDelta(output);
        if (this.#requirePrepared().workload === "private-bgi") {
          this.#recordBgiCompletion(metadata);
        }
        mirror.applyCompletion(metadata.completion);
        break;
      }
      case "api-fault":
        this.#completeStructuredStop(metadata, output);
        break;
      default:
        throw new Error("runtime bridge returned an unknown dispatch outcome");
    }
  }

  #recordBgiCompletion(metadata: CompletionMetadata): void {
    const endpoint = metadata.endpoint;
    invariant(endpoint !== undefined, "BGI completion omitted its endpoint identity");
    invariant(endpoint.symbol.kind === "name", "BGI reached an ordinal API completion");
    invariant(
      metadata.memory?.generation === Number(this.#generation) &&
        metadata.memory.acknowledgedGeneration <= Number(this.#generation),
      "BGI completion memory generation mismatch",
    );
    const completion = metadata.completion;
    const frame = completion.frame;
    const guestWrites = completion.guestWrites;
    invariant(frame !== undefined, "BGI completion omitted its decoded ABI frame");
    invariant(guestWrites !== undefined, "BGI completion omitted its guest writes");
    this.#report.supportedCalls.push({
      sequence: this.#report.supportedCalls.length + 1,
      module: endpoint.module,
      symbol: endpoint.symbol.name,
      arguments: frame.arguments.flatMap(({ words }) => words),
      result: completion.eax,
      implementationId: completion.implementationId,
      returnAddress: completion.instructionPointer,
      stackPointerBefore: frame.stackPointer,
      stackPointerAfter: completion.stackPointer,
      lastError: completion.lastError,
      guestWrites: guestWrites.map(({ write }) => ({
        address: write.address,
        bytes: write.bytes,
      })),
    });
  }

  #completeStructuredStop(
    metadata: ApiFaultMetadata,
    output: Pick<BridgeOutput<unknown>, "control" | "payload">,
  ): void {
    invariant(this.#requirePrepared().workload === "private-bgi", "fixture returned a structured stop");
    invariant(
      output.control.byteLength === 0 && output.payload.byteLength === 0,
      "unsupported BGI call returned a memory delta",
    );
    invariant(
      metadata.memory.generation === Number(this.#generation) &&
        metadata.memory.acknowledgedGeneration === Number(this.#generation),
      "unsupported BGI call changed or failed to acknowledge memory",
    );
    this.#report.firstUnsupported = {
      capabilityCluster: metadata.capabilityCluster,
      endpoint: metadata.endpoint,
      fault: metadata.fault,
      instructionCount: this.#report.instructions,
      memoryContentSha256: metadata.memoryContentSha256,
    };
    const cancellation = this.#requireBridge().cancel<CancellationMetadata>().metadata;
    invariant(cancellation.kind === "cancelled", "structured-stop cleanup metadata is invalid");
    invariant(cancellation.pendingHostCalls === 0, "structured-stop cleanup retained host work");
    invariant(cancellation.pendingCallbacks === 0, "structured-stop cleanup retained callbacks");
    invariant(cancellation.processLifecycle.state === "exited", "BGI process did not exit");
    invariant(cancellation.dllLifecycle === "process-detached", "BGI modules did not detach");
    this.#finish({
      outcome: "structured-stop",
      exitCode: null,
      pendingHostCalls: cancellation.pendingHostCalls,
      pendingCallbacks: cancellation.pendingCallbacks,
      error: null,
    });
  }

  #completeCallback(result: number): void {
    const prepared = this.#requirePrepared();
    const metadata = this.#requireBridge().completeCallback<CallbackCompletionMetadata>(
      prepared.callbackSentinel,
      result,
    ).metadata;
    invariant(metadata.kind === "callback-complete", "invalid callback completion metadata");
    this.#requireMirror().applyCompletion(metadata.completion);
    this.#status("running", `Callback returned; depth ${metadata.remainingDepth}`);
  }

  #completeProcess(exitCode: number): void {
    const metadata = this.#requireBridge().finish<ProcessExitMetadata>(exitCode).metadata;
    invariant(metadata.kind === "process-exit", "invalid process exit metadata");
    invariant(metadata.pendingHostCalls === 0, "clean exit retained a host request");
    invariant(metadata.pendingCallbacks === 0, "clean exit retained a callback");
    this.#finish({
      outcome: "completed",
      exitCode: metadata.exitCode,
      pendingHostCalls: metadata.pendingHostCalls,
      pendingCallbacks: metadata.pendingCallbacks,
      error: null,
    });
  }

  #runMappingContract(): void {
    const bridge = this.#requireBridge();
    const mirror = this.#requireMirror();
    for (let stageIndex = 0; stageIndex < 7; stageIndex += 1) {
      const output = bridge.advanceMappingFixture<MappingFixtureMetadata>(stageIndex);
      const metadata = output.metadata;
      invariant(metadata.kind === "mapping-fixture-delta", "invalid mapping fixture metadata");
      invariant(metadata.stageIndex === stageIndex, "mapping fixture stage identity mismatch");
      const delta = decodeHostDelta(output.control, output.payload);
      invariant(delta.fromGeneration === this.#generation, "mapping delta base generation mismatch");
      invariant(
        delta.fromGeneration === BigInt(metadata.fromGeneration) &&
          delta.generation === BigInt(metadata.generation),
        "mapping delta generation metadata mismatch",
      );
      invariant(delta.pages.length === metadata.dirtyPages, "mapping dirty page count mismatch");
      invariant(
        delta.mappingOperations.map((operation) => operation.operation).join(",") ===
          metadata.mappingOperations.join(","),
        "mapping operation metadata mismatch",
      );

      if (stageIndex === 0) {
        const before = mirror.contractState();
        const invalid = {
          ...delta,
          pages: delta.pages.map((page, index) =>
            index === 0 ? { ...page, owner: page.owner + 1 } : page,
          ),
        };
        let rejected = false;
        try {
          mirror.applyHostDelta(invalid);
        } catch {
          rejected = true;
        }
        invariant(rejected, "invalid mapping delta was not rejected");
        invariant(mirror.contractState() === before, "rejected mapping delta changed the mirror");
        this.#report.mappingAtomicRejections += 1;
      }

      const effectsBefore = mirror.cacheEffectCounts();
      this.#applyAuthoritativeDelta(delta);
      const effectsAfter = mirror.cacheEffectCounts();
      this.#report.mappingDeltas += 1;
      this.#report.mappingDirtyPages += delta.pages.length;
      for (const operation of delta.mappingOperations) {
        this.#report.mappingOperations[operation.operation] += 1;
      }
      this.#report.mappingCacheEffects.tlbFlushes +=
        effectsAfter.tlbFlushes - effectsBefore.tlbFlushes;
      this.#report.mappingCacheEffects.instructionFetchInvalidations +=
        effectsAfter.instructionFetchInvalidations -
        effectsBefore.instructionFetchInvalidations;
      this.#report.mappingCacheEffects.generatedCodeInvalidations +=
        effectsAfter.generatedCodeInvalidations - effectsBefore.generatedCodeInvalidations;

      if (stageIndex === 4 || stageIndex === 6) {
        let fault: unknown = null;
        try {
          mirror.readU32(metadata.probeAddress);
        } catch (error) {
          fault = error;
        }
        invariant(
          fault instanceof MirrorAccessFault && fault.code === "mirror-address-unmapped",
          `${metadata.stage} did not produce a structured stale-access stop`,
        );
        this.#report.mappingStaleAccessStops += 1;
      }

      if (stageIndex === 1) {
        this.#rebuildMirrorFromAuthority();
      }
    }

    invariant(this.#report.mappingOperations.reserve === 1, "reserve mapping count mismatch");
    invariant(this.#report.mappingOperations.commit === 2, "commit mapping count mismatch");
    invariant(this.#report.mappingOperations.protect === 2, "protect mapping count mismatch");
    invariant(this.#report.mappingOperations.decommit === 1, "decommit mapping count mismatch");
    invariant(this.#report.mappingOperations.release === 1, "release mapping count mismatch");
    invariant(this.#report.mappingAtomicRejections === 1, "mapping atomic rejection was not tested");
    invariant(this.#report.mappingStaleAccessStops === 2, "stale mapping stops are incomplete");
    this.#status("mapping", "Browser mirror mapping contract passed");
  }

  #reconcileDirtyPages(): void {
    const pages = this.#requireMirror().discoverDirtyPages();
    if (pages.length === 0) {
      return;
    }
    const buffers = encodeCpuWriteback(this.#generation, pages);
    const metadata = this.#requireBridge().reconcileWriteback<WritebackMetadata>(
      buffers.control,
      buffers.payload,
    ).metadata;
    invariant(metadata.kind === "writeback-reconciled", "invalid writeback receipt metadata");
    invariant(metadata.receipt.touchedPages === pages.length, "writeback page count mismatch");
    this.#generation = BigInt(metadata.receipt.generation);
    this.#report.writebackBatches += 1;
    this.#report.writebackPages += pages.length;
  }

  #applyRequiredDelta(output: Pick<BridgeOutput<unknown>, "control" | "payload">): void {
    invariant(output.control.byteLength > 0, "guest callback did not return its stack delta");
    this.#applyOptionalDelta(output);
  }

  #applyOptionalDelta(output: Pick<BridgeOutput<unknown>, "control" | "payload">): void {
    if (output.control.byteLength === 0) {
      invariant(output.payload.byteLength === 0, "host delta payload has no control descriptor");
      return;
    }
    const delta = decodeHostDelta(output.control, output.payload);
    invariant(delta.fromGeneration === this.#generation, "host delta base generation mismatch");
    if (this.#requirePrepared().workload === "private-bgi") {
      this.#report.mappingDeltas += 1;
      this.#report.mappingDirtyPages += delta.pages.length;
      for (const operation of delta.mappingOperations) {
        this.#report.mappingOperations[operation.operation] += 1;
      }
    }
    this.#applyAuthoritativeDelta(delta);
  }

  #applyAuthoritativeDelta(delta: ReturnType<typeof decodeHostDelta>): void {
    this.#requireMirror().applyHostDelta(delta);
    try {
      const receipt = this.#requireBridge().acknowledge<{
        kind: "acknowledged";
        generation: number;
        dirtyPages: number;
      }>(delta.generation).metadata;
      invariant(receipt.kind === "acknowledged", "invalid memory acknowledgement receipt");
      invariant(BigInt(receipt.generation) === delta.generation, "acknowledged generation mismatch");
      this.#generation = delta.generation;
    } catch {
      this.#rebuildMirrorFromAuthority();
    }
  }

  #rebuildMirrorFromAuthority(): void {
    const output = this.#requireBridge().currentReflection<CurrentReflectionMetadata>();
    const metadata = output.metadata;
    invariant(metadata.kind === "current-reflection", "invalid current reflection metadata");
    const reflection = decodeReflection(output.control, output.payload);
    invariant(
      reflection.generation === BigInt(metadata.generation),
      "current reflection generation mismatch",
    );
    invariant(reflection.pages.length === metadata.pageCount, "current reflection page count mismatch");
    invariant(
      metadata.acknowledgedGeneration <= metadata.generation,
      "current reflection acknowledgement is ahead of authority",
    );
    this.#requireMirror().rebuild(reflection, metadata.regions);
    if (metadata.acknowledgedGeneration < metadata.generation) {
      const receipt = this.#requireBridge().acknowledge<{
        kind: "acknowledged";
        generation: number;
      }>(reflection.generation).metadata;
      invariant(receipt.kind === "acknowledged", "rebuild acknowledgement receipt is invalid");
    }
    this.#generation = reflection.generation;
    this.#report.mirrorRebuilds += 1;
  }

  #waitForHost(metadata: HostRequestMetadata): Promise<number> {
    invariant(
      this.#pendingHostDelivery === null,
      "more than one browser host delivery is pending",
    );
    invariant(
      /^(0|[1-9][0-9]*)$/.test(metadata.requestId) &&
        BigInt(metadata.requestId) <= 0xffff_ffff_ffff_ffffn,
      "Rust host request ID is not a bounded lossless u64",
    );
    invariant(
      Number.isInteger(metadata.threadId) &&
        metadata.threadId > 0 &&
        metadata.threadId <= 0xffff_ffff,
      "Rust host request thread ID is invalid",
    );
    invariant(
      metadata.scheduler.state === "waiting-host" &&
        metadata.scheduler.requestId === metadata.requestId,
      "Rust scheduler authorization does not match the host request",
    );
    invariant(
      metadata.service.length > 0 && metadata.service.length <= 128,
      "host service identity exceeds the transport bound",
    );
    invariant(
      metadata.arguments.length <= 64 &&
        metadata.arguments.every(
          (argument) => Number.isInteger(argument) && argument >= 0 && argument <= 0xffff_ffff,
        ),
      "host request payload exceeds the bounded u32 transport",
    );
    const request: HostRequestMessage = {
      type: "host-request",
      protocolVersion,
      sessionId: this.#sessionId,
      requestId: metadata.requestId,
      threadId: metadata.threadId,
      service: metadata.service,
      arguments: metadata.arguments,
    };
    post(request);
    return new Promise((resolve, reject) => {
      this.#pendingHostDelivery = {
        requestId: metadata.requestId,
        threadId: metadata.threadId,
        resolve,
        reject,
      };
    });
  }

  #checkWatchdog(): void {
    const elapsed = performance.now() - this.#startedAt;
    if (
      this.#report.instructions > this.#configuration.watchdogInstructionLimit ||
      elapsed > this.#configuration.watchdogWallMilliseconds
    ) {
      this.#terminate(
        "watchdog",
        `Watchdog stopped the session after ${this.#report.instructions} instructions`,
      );
      throw new SessionStopped("watchdog terminated the guest");
    }
  }

  #terminate(outcome: "cancelled" | "watchdog", detail: string): void {
    if (this.#terminal) {
      return;
    }
    let pendingHostCalls = Number(this.#pendingHostDelivery !== null);
    let pendingCallbacks = 0;
    let error: string | null = detail;
    if (this.#runtimePrepared && this.#bridge !== null) {
      try {
        const metadata = this.#bridge.cancel<CancellationMetadata>().metadata;
        invariant(metadata.kind === "cancelled", "invalid cancellation metadata");
        pendingHostCalls = metadata.pendingHostCalls;
        pendingCallbacks = metadata.pendingCallbacks;
        invariant(pendingHostCalls === 0, "cancellation retained a host request");
        invariant(pendingCallbacks === 0, "cancellation retained a callback");
        if (this.#prepared?.workload === "private-bgi") {
          invariant(metadata.processLifecycle.state === "exited", "BGI cancellation did not exit");
          invariant(metadata.dllLifecycle === "process-detached", "BGI cancellation did not detach");
        }
      } catch (cancelError) {
        error = `${detail}; cleanup failed: ${String(cancelError)}`;
      }
    }
    this.#pendingHostDelivery?.reject(new SessionStopped(detail));
    this.#pendingHostDelivery = null;
    this.#finish({
      outcome,
      exitCode: null,
      pendingHostCalls,
      pendingCallbacks,
      error,
    });
  }

  #fail(error: Error): void {
    if (this.#terminal) {
      return;
    }
    let pendingHostCalls = Number(this.#pendingHostDelivery !== null);
    let pendingCallbacks = 0;
    if (this.#runtimePrepared && this.#bridge !== null) {
      try {
        const metadata = this.#bridge.cancel<CancellationMetadata>().metadata;
        pendingHostCalls = metadata.pendingHostCalls;
        pendingCallbacks = metadata.pendingCallbacks;
      } catch (cancelError) {
        error = new Error(`${error.message}; cleanup failed: ${String(cancelError)}`);
      }
    }
    this.#pendingHostDelivery?.reject(new SessionStopped(error.message));
    this.#pendingHostDelivery = null;
    this.#finish({
      outcome: "fault",
      exitCode: null,
      pendingHostCalls,
      pendingCallbacks,
      error: error.message,
    });
  }

  #finish(
    terminal: Pick<
      SessionReport,
      "outcome" | "exitCode" | "pendingHostCalls" | "pendingCallbacks" | "error"
    >,
  ): void {
    if (this.#terminal) {
      return;
    }
    this.#terminal = true;
    const message: SessionFinishedMessage = {
      type: "session-finished",
      protocolVersion,
      sessionId: this.#sessionId,
      report: {
        ...terminal,
        elapsedMilliseconds: performance.now() - this.#startedAt,
        ...this.#report,
        wasmBridgeCalls: this.#bridge?.callCount() ?? 0,
        finalGeneration: Number(this.#generation),
      },
    };
    post(message);
  }

  #status(phase: string, detail: string): void {
    if (!this.#terminal) {
      post({
        type: "session-status",
        protocolVersion,
        sessionId: this.#sessionId,
        phase,
        detail,
      });
    }
  }

  #requireBridge(): RuntimeBridge {
    invariant(this.#bridge !== null, "runtime bridge is unavailable");
    return this.#bridge;
  }

  #requireMirror(): V86Mirror {
    invariant(this.#mirror !== null, "v86 mirror is unavailable");
    return this.#mirror;
  }

  #requirePrepared(): PreparedMetadata {
    invariant(this.#prepared !== null, "runtime preparation metadata is unavailable");
    return this.#prepared;
  }
}

let activeSession: BrowserRuntimeSession | null = null;

worker.addEventListener("message", (event: MessageEvent<MainToWorkerMessage>) => {
  const message = event.data;
  assertProtocolVersion(message.protocolVersion);
  switch (message.type) {
    case "start-session": {
      if (activeSession !== null) {
        post({
          type: "session-finished",
          protocolVersion,
          sessionId: message.sessionId,
          report: {
            outcome: "fault",
            exitCode: null,
            elapsedMilliseconds: 0,
            instructions: 0,
            slices: 0,
            budgetYields: 0,
            hostRequests: 0,
            rustSchedulerTransitions: 0,
            callbackEntries: 0,
            maximumCallbackDepth: 0,
            writebackBatches: 0,
            writebackPages: 0,
            mappingDeltas: 0,
            mappingOperations: {
              reserve: 0,
              commit: 0,
              protect: 0,
              decommit: 0,
              release: 0,
            },
            mappingDirtyPages: 0,
            mappingAtomicRejections: 0,
            mappingStaleAccessStops: 0,
            mappingCacheEffects: {
              tlbFlushes: 0,
              instructionFetchInvalidations: 0,
              generatedCodeInvalidations: 0,
            },
            mirrorRebuilds: 0,
            initialGeneration: 0,
            finalGeneration: 0,
            pendingHostCalls: 0,
            pendingCallbacks: 0,
            maximumSliceMilliseconds: 0,
            wasmBridgeCalls: 0,
            supportedCalls: [],
            firstUnsupported: null,
            error: "another runtime session is active",
          },
        });
        return;
      }
      const session = new BrowserRuntimeSession(message.sessionId, message.configuration);
      activeSession = session;
      void session.run().finally(() => {
        if (activeSession === session) {
          activeSession = null;
        }
      });
      break;
    }
    case "cancel-session":
      activeSession?.cancel();
      break;
    case "host-completion":
      activeSession?.completeHost(message);
      break;
  }
});
