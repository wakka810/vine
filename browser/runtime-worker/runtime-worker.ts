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
  type RuntimeCurrentDirectorySnapshot,
  type RuntimeComApartmentSnapshot,
  type RuntimeMutexSnapshot,
  type RuntimeWaitableTimerSnapshot,
  type RuntimeIconSnapshot,
  type RuntimeCursorSnapshot,
  type RuntimeWindowClassSnapshot,
  type RuntimeWindowSnapshot,
  type RuntimeSourceReadSnapshot,
  type RuntimeWritableCommitSnapshot,
  type RuntimeGdiSnapshot,
  type RuntimeGdiplusSnapshot,
  type RuntimeImm32Snapshot,
  type RuntimeVirtualMemorySnapshot,
  type WorkerToMainMessage,
} from "./protocol.js";
import {
  decodeHostDelta,
  decodeReflection,
  type MemoryRegionSnapshot,
} from "./memory-transport.js";
import {
  ExecutionMemoryAccessFault,
  prepareExecutionEngine,
  type ExecutionCpuContext,
  type ExecutionEngine,
  type ExecutionStop,
} from "./execution-engine.js";
import {
  ExecutionStateMachine,
  type HostCompletionPolicy,
  PendingHostRequestTable,
} from "./execution-state.js";
import {
  type ApiCompletion,
  type BridgeOutput,
  type CpuResume,
  type PreparedMetadata,
  RuntimeBridge,
} from "./runtime-bridge.js";
import {
  SEMANTIC_CALL_DIGEST_ALGORITHM,
  SemanticCallDigest,
} from "./semantic-call.mjs";

const worker = self as DedicatedWorkerGlobalScope;
const EMPTY_SEMANTIC_CALL_SHA256 = new SemanticCallDigest().digestHex();
const yieldChannel = new MessageChannel();
const yieldResolvers: Array<() => void> = [];
yieldChannel.port1.addEventListener("message", () => {
  const resolve = yieldResolvers.shift();
  invariant(resolve !== undefined, "Worker yield channel received an unowned task");
  resolve();
});
yieldChannel.port1.start();

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

interface WritableCommitPayloadMetadata {
  kind: "writable-commit-payload";
  requestId: string;
  request: RuntimeWritableCommitSnapshot["request"];
}

type ExecutableSchedulerMetadata =
  | { state: "runnable" }
  | { state: "in-callback"; depth: number };

interface ThreadScheduledMetadata {
  kind: "thread-scheduled";
  previousThreadId: number;
  threadId: number;
  scheduler: ExecutableSchedulerMetadata;
}

interface ApiBoundaryThreadSelection {
  previousThreadId: number;
  threadId: number;
  state: ExecutableSchedulerMetadata;
}

interface SchedulerIdleMetadata {
  kind: "scheduler-idle";
  previousThreadId: number;
}

interface ThreadExitMetadata {
  kind: "thread-exit";
  threadId: number;
  exitCode: number;
  markerAddress?: number;
  markerValue?: number;
  scheduler: { state: "terminated"; exitCode: number };
}

type CreatedThreadSchedulerState =
  | { state: "suspended"; count: number }
  | { state: "runnable" };

interface CreatedThreadMetadata {
  handle: number;
  thread: {
    id: number;
    lifecycle: { state: "running" };
    stack?: {
      threadId: number;
      initialStackPointer: number;
      returnSentinel: number;
    };
    environment?: {
      threadId: number;
      tebAddress: number;
    };
    guestBootstrap?: {
      threadId: number;
      entryPoint: number;
      parameter: number;
      initialStackPointer: number;
      fsBase: number;
      returnSentinel: number;
    };
  };
  schedulerState: CreatedThreadSchedulerState;
}

interface ResumedThreadMetadata {
  handle: number;
  threadId: number;
  previousSuspendCount: number;
  schedulerState: CreatedThreadSchedulerState;
}

interface GuestCallbackMetadata {
  kind: "guest-callback";
  frameId: number;
  replacedFrameId?: number;
  depth: number;
  target: number;
  allocationStart: number;
  stackPointer: number;
  scratchAddress: number;
  scratchBytes: number;
  frameBytes: number;
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
  currentDirectory?: RuntimeCurrentDirectorySnapshot;
  mutex?: RuntimeMutexSnapshot;
  waitableTimer?: RuntimeWaitableTimerSnapshot;
  icon?: RuntimeIconSnapshot;
  cursor?: RuntimeCursorSnapshot;
  windowClass?: RuntimeWindowClassSnapshot;
  window?: RuntimeWindowSnapshot;
  comApartment?: RuntimeComApartmentSnapshot;
  sourceRead?: RuntimeSourceReadSnapshot;
  writableCommit?: RuntimeWritableCommitSnapshot;
  gdi?: RuntimeGdiSnapshot;
  gdiplus?: RuntimeGdiplusSnapshot;
  imm32?: RuntimeImm32Snapshot;
  virtualMemory?: RuntimeVirtualMemorySnapshot;
  createdThread?: CreatedThreadMetadata;
  resumedThread?: ResumedThreadMetadata;
  threadSelection?: ApiBoundaryThreadSelection;
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

interface ComEndpointResolutionMetadata {
  kind: "com-endpoint-resolution";
  endpointAddress: number;
  live: boolean;
  memory: { generation: number; acknowledgedGeneration: number };
}

interface ComCompletionMetadata {
  kind: "com-complete";
  completion: CpuResume;
  call: {
    frame: {
      endpointAddress: number;
      callSite: number;
      stackPointer: number;
      returnStackPointer: number;
      returnAddress: number;
      interface: string;
      method: string;
      vtableSlot: number;
      thisPointer: number;
      explicitArguments: number[];
    };
    returnValue: { kind: "h-result" | "u32"; value: number };
    referenceCount: number;
    objectRetired: boolean;
    guestWrites: Array<{ address: number; bytes: number[] }>;
  };
  memory: { generation: number; acknowledgedGeneration: number };
}

interface ComFaultMetadata {
  kind: "com-fault";
  endpoint: unknown;
  capabilityCluster: string;
  fault: unknown;
  memory: { generation: number; acknowledgedGeneration: number };
  memoryContentSha256: string;
}

interface HostCompletionMetadata {
  kind: "host-complete";
  threadId: number;
  scheduler: ExecutableSchedulerMetadata;
  completion: ApiCompletion;
  endpoint?: CompletionMetadata["endpoint"];
  memory?: CompletionMetadata["memory"];
  currentDirectory?: RuntimeCurrentDirectorySnapshot;
  mutex?: RuntimeMutexSnapshot;
  waitableTimer?: RuntimeWaitableTimerSnapshot;
  icon?: RuntimeIconSnapshot;
  cursor?: RuntimeCursorSnapshot;
  windowClass?: RuntimeWindowClassSnapshot;
  window?: RuntimeWindowSnapshot;
  comApartment?: RuntimeComApartmentSnapshot;
  sourceRead?: RuntimeSourceReadSnapshot;
  writableCommit?: RuntimeWritableCommitSnapshot;
  gdi?: RuntimeGdiSnapshot;
  gdiplus?: RuntimeGdiplusSnapshot;
  imm32?: RuntimeImm32Snapshot;
  virtualMemory?: RuntimeVirtualMemorySnapshot;
  createdThread?: CreatedThreadMetadata;
  resumedThread?: ResumedThreadMetadata;
}

interface SourceManifest {
  schema: "vine.source-snapshot-manifest";
  schemaVersion: 1;
  titleFingerprint: string;
  snapshotId: string;
  mount: { sourceId: number; rootPathA: number[] };
  files: Array<{
    objectId: number;
    relativePathA: number[];
    byteLength: number;
    sha256: string;
  }>;
}

interface WritableProfileManifest {
  schema: "vine.writable-profile-snapshot";
  schemaVersion: 1;
  titleFingerprint: string;
  profileId: string;
  rootPathA: number[];
  quotaBytes: number;
  files: Array<{
    objectId: number;
    relativePathA: number[];
    exists: boolean;
    generation: number;
    byteLength: number;
    sha256: string | null;
    bytes: number[];
  }>;
}

interface CallbackCompletionMetadata {
  kind: "callback-complete";
  frameId: number;
  remainingDepth: number;
  endpoint: {
    id: number;
    address: number;
    module: string;
    symbol: { kind: "name"; name: string } | { kind: "ordinal"; ordinal: number };
  };
  completion: ApiCompletion;
  memory: { generation: number; acknowledgedGeneration: number };
  currentDirectory?: RuntimeCurrentDirectorySnapshot;
  mutex?: RuntimeMutexSnapshot;
  waitableTimer?: RuntimeWaitableTimerSnapshot;
  icon?: RuntimeIconSnapshot;
  cursor?: RuntimeCursorSnapshot;
  windowClass?: RuntimeWindowClassSnapshot;
  window?: RuntimeWindowSnapshot;
  comApartment?: RuntimeComApartmentSnapshot;
  sourceRead?: RuntimeSourceReadSnapshot;
  writableCommit?: RuntimeWritableCommitSnapshot;
  gdi?: RuntimeGdiSnapshot;
  gdiplus?: RuntimeGdiplusSnapshot;
  imm32?: RuntimeImm32Snapshot;
  virtualMemory?: RuntimeVirtualMemorySnapshot;
  createdThread?: CreatedThreadMetadata;
  resumedThread?: ResumedThreadMetadata;
}

interface InitialThreadStackGrowthMetadata {
  kind: "initial-thread-stack-growth";
  growth: {
    threadId: number;
    addressAllocationId: number;
    memoryOwner: number;
    faultAddress: number;
    pageFaultErrorCode: number;
    committedPage: { start: number; byteLength: number };
    previousGuardPage: { start: number; byteLength: number };
    nextGuardPage?: { start: number; byteLength: number };
    committedPages: number;
  };
  fromGeneration: number;
  generation: number;
  mappingOperations: ["commit"];
  dirtyPages: number;
}

interface PageFaultResolvedMetadata {
  kind: "page-fault-resolved";
  resolution:
    | {
        kind: "initial-thread-stack-growth";
        growth: InitialThreadStackGrowthMetadata["growth"];
      }
    | {
        kind: "private-heap-page-materialization";
        materialization: {
          handle: number;
          allocation: {
            id: number;
            address: number;
            byteLength: number;
            reservedByteLength: number;
            initialization: "zeroed" | "deterministic-uninitialized";
          };
          memoryOwner: number;
          addressAllocationId: number;
          faultAddress: number;
          pageFaultErrorCode: number;
          committedPage: { start: number; byteLength: number };
          materializedPages: number;
        };
      };
  fromGeneration: number;
  generation: number;
  mappingOperations: ["commit"];
  dirtyPages: number;
}

interface PageFaultRejectedMetadata {
  kind: "page-fault-rejected";
  faultAddress: number;
  pageFaultErrorCode: number;
  error: { code: string };
  memory: { generation: number; acknowledgedGeneration: number };
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
  contentSha256: string;
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

function failInvariant(message: string): never {
  throw new Error(message);
}

function validByteArray(bytes: number[], trailingNul: boolean): boolean {
  return (
    Array.isArray(bytes) &&
    bytes.length > 0 &&
    bytes.length <= 32_768 &&
    bytes.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 0xff) &&
    (trailingNul
      ? bytes.at(-1) === 0 && !bytes.slice(0, -1).includes(0)
      : !bytes.includes(0))
  );
}

function byteArraysEqual(left: number[], right: number[]): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const digest = new Uint8Array(
    await crypto.subtle.digest("SHA-256", ownedArrayBuffer(bytes)),
  );
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function ownedArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const buffer = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(buffer).set(bytes);
  return buffer;
}

function post(message: WorkerToMainMessage): void {
  worker.postMessage(message);
}

function yieldToWorkerLoop(): Promise<void> {
  return new Promise((resolve) => {
    yieldResolvers.push(resolve);
    yieldChannel.port2.postMessage(null);
  });
}

class BrowserRuntimeSession {
  readonly #sessionId: number;
  readonly #configuration: SessionConfiguration;
  readonly #startedAt = performance.now();
  #bridge: RuntimeBridge | null = null;
  #engine: ExecutionEngine | null = null;
  #prepared: PreparedMetadata | null = null;
  #generation = 0n;
  #terminal = false;
  #runtimePrepared = false;
  #initialThreadId = 0;
  #currentThreadId = 0;
  #liveThreadId: number | null = null;
  readonly #threadContexts = new Map<number, ExecutionCpuContext>();
  readonly #threadResumeStates = new Map<number, "ready" | "guest-callback">();
  readonly #threadExecutionStates = new Map<number, "ready" | "guest-callback">();
  readonly #runnableResolvers: Array<() => void> = [];
  readonly #execution = new ExecutionStateMachine();
  readonly #pendingHostRequests = new PendingHostRequestTable();
  readonly #sourceReadControllers = new Map<string, AbortController>();
  #sourceManifest: SourceManifest | null = null;
  #writableProfile: WritableProfileManifest | null = null;
  readonly #profileSessionKey = crypto.randomUUID();
  #nextBgiCallSequence = 0;
  readonly #bgiCallbackCallSequences = new Map<number, number>();
  readonly #semanticCallDigest = new SemanticCallDigest();
  #watchdogContext: SessionReport["watchdogContext"] = null;
  #watchdogArmedAt: number | null = null;
  #watchdogTimer: number | null = null;
  readonly #report = {
    instructions: 0,
    slices: 0,
    budgetYields: 0,
    hostRequests: 0,
    rustSchedulerTransitions: 0,
    registeredThreadIds: [] as number[],
    executedThreadIds: [] as number[],
    contextSwitches: 0,
    hostWaitExecutions: [] as SessionReport["hostWaitExecutions"],
    threadExits: [] as SessionReport["threadExits"],
    callbackEntries: 0,
    maximumCallbackDepth: 0,
    writebackBatches: 0,
    writebackPages: 0,
    stackGrowths: [] as SessionReport["stackGrowths"],
    heapPageMaterializations: undefined as SessionReport["heapPageMaterializations"],
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
    initialCurrentDirectory: null as SessionReport["initialCurrentDirectory"],
    maximumSliceMilliseconds: 0,
    supportedCallCount: 0,
    supportedCallSemanticsAlgorithm: SEMANTIC_CALL_DIGEST_ALGORITHM,
    supportedCallSemanticsSha256: null as string | null,
    supportedCalls: [] as SessionReport["supportedCalls"],
    sourceReads: [] as SessionReport["sourceReads"],
    writableCommits: [] as SessionReport["writableCommits"],
    firstUnsupported: null as SessionReport["firstUnsupported"],
  };

  constructor(sessionId: number, configuration: SessionConfiguration) {
    this.#sessionId = sessionId;
    this.#configuration = configuration;
  }

  async run(): Promise<void> {
    try {
      const workload = this.#configuration.workload ?? "controlled-fixture";
      invariant(
        this.#configuration.evidenceLevel === "full" ||
          this.#configuration.evidenceLevel === "summary",
        "runtime evidence level is invalid",
      );
      invariant(
        this.#configuration.evidenceLevel === "full" || workload === "private-bgi",
        "summary runtime evidence is only supported for the private BGI workload",
      );
      const sourceBackedWorkload = workload === "controlled-source-read";
      const profileBackedWorkload = workload === "private-bgi";
      this.#status("loading", `Loading Rust runtime, execution engine, and ${workload}`);
      const coreUrl = new URL("./runtime-core.wasm", import.meta.url);
      const executableUrl = new URL(
        workload === "private-bgi"
          ? this.#configuration.executableUrl ?? "./private/BGI.exe"
          : workload === "controlled-source-read"
            ? this.#configuration.executableUrl ?? "./source-read.exe"
            : "./browser-vertical.exe",
        import.meta.url,
      );
      const [bridge, engineFactory, executableResponse] = await Promise.all([
        RuntimeBridge.instantiate(coreUrl),
        prepareExecutionEngine(new URL(".", import.meta.url)),
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
      const executable = new Uint8Array(await executableResponse.arrayBuffer());
      const sourceManifestBytes =
        sourceBackedWorkload
          ? await this.#fetchBytes(
              this.#sameOriginStorageUrl(
                this.#configuration.sourceManifestUrl,
                "/api/source/v1/manifest",
                "source manifest URL",
              ),
              "source manifest",
            )
          : null;
      const writableProfileBytes =
        profileBackedWorkload
          ? await this.#fetchBytes(
              this.#sameOriginStorageUrl(
                this.#configuration.writableProfileUrl,
                "/api/profile/v1/snapshot",
                "writable profile URL",
              ),
              "writable profile",
            )
          : null;
      this.#sourceManifest =
        sourceManifestBytes === null ? null : this.#parseSourceManifest(sourceManifestBytes);
      this.#writableProfile =
        writableProfileBytes === null
          ? null
          : this.#parseWritableProfile(writableProfileBytes);
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
              sourceManifestBytes,
              writableProfileBytes ?? failInvariant("BGI writable profile is missing"),
            )
          : workload === "controlled-source-read"
            ? bridge.prepareSourceRead(
                executable,
                await this.#fetchPolicy(
                  new URL(
                    this.#configuration.policyUrl ?? "./kernel32-source-file-read-v1.json",
                    import.meta.url,
                  ),
                ),
                sourceManifestBytes ?? failInvariant("source fixture manifest is missing"),
              )
            : bridge.prepare(executable);
      invariant(preparedOutput.metadata.schemaVersion === 2, "prepared metadata schema mismatch");
      const engine = engineFactory({
        maximumPhysicalPages: preparedOutput.metadata.maximumPhysicalPages,
      });
      this.#engine = engine;
      this.#runtimePrepared = true;
      this.#prepared = preparedOutput.metadata;
      invariant(
        Number.isInteger(preparedOutput.metadata.runtime.processId) &&
          preparedOutput.metadata.runtime.processId > 0,
        "prepared runtime process ID is invalid",
      );
      invariant(
        Number.isInteger(preparedOutput.metadata.runtime.threadId) &&
          preparedOutput.metadata.runtime.threadId > 0,
        "prepared runtime thread ID is invalid",
      );
      this.#initialThreadId = preparedOutput.metadata.runtime.threadId;
      this.#currentThreadId = this.#initialThreadId;
      const initialCurrentDirectory = preparedOutput.metadata.runtime.currentDirectoryA;
      invariant(
        initialCurrentDirectory.sourceAuthority.length > 0 &&
          initialCurrentDirectory.encoding === "windows-932-v1" &&
          initialCurrentDirectory.bytesWithNul.length >= 4 &&
          initialCurrentDirectory.bytesWithNul.at(-1) === 0 &&
          !initialCurrentDirectory.bytesWithNul.slice(0, -1).includes(0) &&
          Number.isSafeInteger(preparedOutput.metadata.runtime.currentDirectoryGeneration) &&
          preparedOutput.metadata.runtime.currentDirectoryGeneration >= 0,
        "prepared runtime current-directory metadata is invalid",
      );
      this.#report.initialCurrentDirectory = {
        directory: initialCurrentDirectory,
        generation: preparedOutput.metadata.runtime.currentDirectoryGeneration,
      };
      if (this.#configuration.mode !== "watchdog") {
        this.#armWatchdog();
      }
      invariant(
        (preparedOutput.metadata.workload ?? "controlled-fixture") === workload,
        "prepared workload mismatch",
      );
      if (sourceBackedWorkload) {
        this.#validatePreparedSourceSnapshot(preparedOutput.metadata);
      }
      if (profileBackedWorkload) {
        this.#validatePreparedWritableProfile(preparedOutput.metadata);
      }
      if (workload === "private-bgi") {
        const stack = preparedOutput.metadata.initialThreadStack;
        const allocation = preparedOutput.metadata.initialStackAddressAllocation;
        invariant(stack !== undefined, "BGI initial thread stack metadata is missing");
        invariant(allocation !== undefined, "BGI stack allocation metadata is missing");
        const guardPage = stack.guardPage;
        invariant(guardPage !== undefined, "BGI initial stack guard metadata is missing");
        invariant(stack.threadId === this.#currentThreadId, "BGI stack belongs to another thread");
        invariant(
          stack.addressAllocationId === allocation.id &&
            stack.memoryOwner > 0 &&
            stack.reservedRange.start === allocation.range.base &&
            stack.reservedRange.byteLength === allocation.range.byteLength,
          "BGI stack allocation identity mismatch",
        );
        invariant(
          stack.committedRange.start + stack.committedRange.byteLength === stack.stackTop &&
            stack.reservedRange.start + stack.reservedRange.byteLength === stack.stackTop &&
            guardPage.start + guardPage.byteLength === stack.committedRange.start,
          "BGI initial stack reserve, commit, and guard are not contiguous",
        );
        invariant(
          allocation.purpose.kind === "thread-stack" &&
            allocation.classification === "stack" &&
            allocation.lifetime === "thread" &&
            allocation.releasePolicy === "explicit" &&
            allocation.guardPages === 1 &&
            !allocation.executable,
          "BGI process stack allocation policy mismatch",
        );
      }
      const reflection = decodeReflection(preparedOutput.control, preparedOutput.payload);
      invariant(
        reflection.generation === BigInt(preparedOutput.metadata.generation),
        "Rust reflection generation metadata mismatch",
      );
      invariant(
        reflection.pages.length === preparedOutput.metadata.pageCount,
        "Rust reflection page count metadata mismatch",
      );
      engine.initialize(reflection, preparedOutput.metadata);
      this.#liveThreadId = this.#initialThreadId;
      this.#threadContexts.set(this.#initialThreadId, engine.saveCpuContext());
      this.#threadExecutionStates.set(this.#initialThreadId, "ready");
      this.#report.registeredThreadIds.push(this.#initialThreadId);
      for (const thread of preparedOutput.metadata.additionalThreads ?? []) {
        invariant(
          Number.isInteger(thread.threadId) &&
            thread.threadId > 0 &&
            thread.threadId <= 0xffff_ffff &&
            !this.#threadContexts.has(thread.threadId),
          "prepared additional thread identity is invalid or duplicate",
        );
        invariant(
          thread.returnSentinel === preparedOutput.metadata.entrySentinel,
          "additional thread return sentinel is outside the prepared runtime contract",
        );
        this.#threadContexts.set(
          thread.threadId,
          engine.createCpuContext({
            entryPoint: thread.entryPoint,
            initialStackPointer: thread.initialStackPointer,
            fsBase: thread.fsBase,
          }),
        );
        this.#threadExecutionStates.set(thread.threadId, "ready");
        this.#report.registeredThreadIds.push(thread.threadId);
      }
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
    return this.#fetchBytes(url, "runtime policy");
  }

  #sameOriginStorageUrl(configured: string | undefined, fallback: string, label: string): URL {
    const runtimeUrl = new URL(import.meta.url);
    const url = new URL(configured ?? fallback, runtimeUrl);
    invariant(url.origin === runtimeUrl.origin, `${label} must use the Runtime Worker origin`);
    return url;
  }

  async #fetchBytes(url: URL, label: string): Promise<Uint8Array> {
    const response = await fetch(url);
    invariant(response.ok, `${label} fetch failed with ${response.status}`);
    return new Uint8Array(await response.arrayBuffer());
  }

  #parseSourceManifest(bytes: Uint8Array): SourceManifest {
    const manifest = JSON.parse(new TextDecoder().decode(bytes)) as SourceManifest;
    invariant(manifest.schema === "vine.source-snapshot-manifest", "source manifest schema mismatch");
    invariant(manifest.schemaVersion === 1, "source manifest version mismatch");
    invariant(
      /^[a-zA-Z0-9:._-]{1,128}$/u.test(manifest.titleFingerprint) &&
        /^[a-zA-Z0-9._-]{1,128}$/u.test(manifest.snapshotId),
      "source manifest identity is invalid",
    );
    invariant(
      Number.isInteger(manifest.mount.sourceId) &&
        manifest.mount.sourceId > 0 &&
        validByteArray(manifest.mount.rootPathA, true),
      "source manifest mount is invalid",
    );
    invariant(manifest.files.length > 0 && manifest.files.length <= 4096, "source file count is invalid");
    const objectIds = new Set<number>();
    for (const file of manifest.files) {
      invariant(
        Number.isInteger(file.objectId) &&
          file.objectId > 0 &&
          !objectIds.has(file.objectId) &&
          validByteArray(file.relativePathA, false) &&
          Number.isSafeInteger(file.byteLength) &&
          file.byteLength >= 0 &&
          file.byteLength <= 0xffff_ffff &&
          /^[0-9a-f]{64}$/u.test(file.sha256),
        "source file descriptor is invalid",
      );
      objectIds.add(file.objectId);
    }
    return manifest;
  }

  #validatePreparedSourceSnapshot(prepared: PreparedMetadata): void {
    const manifest = this.#sourceManifest ?? failInvariant("source manifest was not registered");
    const snapshot = prepared.sourceSnapshot ?? failInvariant("Rust source snapshot metadata is missing");
    invariant(
      snapshot.titleFingerprint === manifest.titleFingerprint &&
        snapshot.snapshotId === manifest.snapshotId &&
        snapshot.sourceId === manifest.mount.sourceId &&
        byteArraysEqual(snapshot.rootPathA, manifest.mount.rootPathA),
      "Rust and browser source snapshot identities differ",
    );
    invariant(
      Object.keys(snapshot.files).length === manifest.files.length,
      "Rust and browser source file counts differ",
    );
    for (const file of manifest.files) {
      const preparedFile = snapshot.files[String(file.objectId)];
      invariant(
        preparedFile !== undefined &&
          preparedFile.objectId === file.objectId &&
          byteArraysEqual(preparedFile.relativePathA, file.relativePathA) &&
          preparedFile.byteLength === file.byteLength &&
          preparedFile.sha256 === file.sha256,
        `Rust and browser source object ${file.objectId} differ`,
      );
    }
  }

  #parseWritableProfile(bytes: Uint8Array): WritableProfileManifest {
    const profile = JSON.parse(new TextDecoder().decode(bytes)) as WritableProfileManifest;
    invariant(
      profile.schema === "vine.writable-profile-snapshot" && profile.schemaVersion === 1,
      "writable profile schema mismatch",
    );
    invariant(
      /^[a-zA-Z0-9:._-]{1,128}$/u.test(profile.titleFingerprint) &&
        /^[a-zA-Z0-9._-]{1,128}$/u.test(profile.profileId) &&
        validByteArray(profile.rootPathA, true) &&
        Number.isSafeInteger(profile.quotaBytes) &&
        profile.quotaBytes > 0 &&
        profile.quotaBytes <= 16 * 1024 * 1024,
      "writable profile identity or quota is invalid",
    );
    invariant(
      profile.files.length > 0 && profile.files.length <= 64,
      "writable profile file count is invalid",
    );
    const objectIds = new Set<number>();
    const paths = new Set<string>();
    let totalBytes = 0;
    for (const file of profile.files) {
      const pathIdentity = file.relativePathA
        .map((byte) => (byte >= 0x41 && byte <= 0x5a ? byte + 0x20 : byte))
        .join(",");
      invariant(
        Number.isInteger(file.objectId) &&
          file.objectId > 0 &&
          !objectIds.has(file.objectId) &&
          validByteArray(file.relativePathA, false) &&
          !paths.has(pathIdentity) &&
          typeof file.exists === "boolean" &&
          Number.isSafeInteger(file.generation) &&
          file.generation >= 0 &&
          Number.isSafeInteger(file.byteLength) &&
          file.byteLength >= 0 &&
          file.byteLength <= profile.quotaBytes &&
          Array.isArray(file.bytes) &&
          file.bytes.every(
            (byte) => Number.isInteger(byte) && byte >= 0 && byte <= 0xff,
          ) &&
          file.bytes.length === file.byteLength &&
          (file.sha256 === null || /^[0-9a-f]{64}$/u.test(file.sha256)) &&
          (file.exists
            ? file.generation > 0 && file.sha256 !== null
            : file.generation === 0 && file.byteLength === 0 && file.sha256 === null),
        "writable profile file descriptor is invalid or duplicated",
      );
      objectIds.add(file.objectId);
      paths.add(pathIdentity);
      totalBytes += file.byteLength;
    }
    invariant(totalBytes <= profile.quotaBytes, "writable profile content exceeds its quota");
    return profile;
  }

  #validatePreparedWritableProfile(prepared: PreparedMetadata): void {
    const profile = this.#writableProfile ?? failInvariant("writable profile was not registered");
    const runtime =
      prepared.writableProfile ?? failInvariant("Rust writable profile metadata is missing");
    invariant(
      runtime.titleFingerprint === profile.titleFingerprint &&
        runtime.profileId === profile.profileId &&
        byteArraysEqual(runtime.rootPathA, profile.rootPathA) &&
        runtime.quotaBytes === profile.quotaBytes,
      "Rust and browser writable profile identities differ",
    );
    invariant(
      Object.keys(runtime.files).length === profile.files.length,
      "Rust and browser writable profile file counts differ",
    );
    for (const file of profile.files) {
      const preparedFile = runtime.files[String(file.objectId)];
      invariant(
        preparedFile !== undefined &&
          preparedFile.objectId === file.objectId &&
          byteArraysEqual(preparedFile.relativePathA, file.relativePathA) &&
          preparedFile.exists === file.exists &&
          preparedFile.generation === file.generation &&
          preparedFile.byteLength === file.byteLength &&
          preparedFile.sha256 === file.sha256 &&
          !preparedFile.dirty,
        `Rust and browser writable object ${file.objectId} differ`,
      );
    }
  }

  completeHost(message: HostCompletionMessage): void {
    if (this.#terminal) {
      throw new Error("host completion arrived after the runtime session stopped");
    }
    try {
      const pending = this.#pendingHostRequests.complete(
        message.requestId,
        message.threadId,
        message.result,
      );
      const completion = this.#requireBridge().completeHost<HostCompletionMetadata>(
        BigInt(pending.requestId),
        message.result,
      ).metadata;
      invariant(completion.kind === "host-complete", "invalid host completion metadata");
      invariant(completion.threadId === pending.threadId, "Rust resumed a different guest thread");
      const resumeState =
        completion.scheduler.state === "in-callback" ? "guest-callback" : "ready";
      const expectedResumeState =
        pending.completionPolicy === "resume-callback" ? "guest-callback" : "ready";
      invariant(
        resumeState === expectedResumeState,
        "Rust host completion disagrees with the recorded completion policy",
      );
      this.#applyCpuResumeToThread(completion.threadId, completion.completion);
      this.#threadExecutionStates.set(completion.threadId, resumeState);
      this.#report.rustSchedulerTransitions += 1;
      pending.resolve({
        result: message.result >>> 0,
        completionPolicy: pending.completionPolicy,
      });
      if (this.#execution.state() === "waiting-host") {
        this.#execution.transition("host-completion", {
          cause: "browser-host-completion",
          threadId: pending.threadId,
          requestId: pending.requestId,
        });
        this.#signalRunnable();
      }
      this.#status("running", `${pending.service} completed`);
    } catch (error) {
      this.#fail(error instanceof Error ? error : new Error(String(error)));
      return;
    }
  }

  async #serviceSourceRead(metadata: HostRequestMetadata): Promise<void> {
    const manifest = this.#sourceManifest ?? failInvariant("source read has no registered manifest");
    invariant(metadata.service === "storage.source-read.v1", "unexpected source read service");
    invariant(metadata.arguments.length === 6, "source read request must contain six words");
    const [version, sourceId, objectId, offsetLow, offsetHigh, requestedBytes] =
      metadata.arguments;
    invariant(version === 1, "source read request version mismatch");
    invariant(sourceId === manifest.mount.sourceId, "source read mount identity mismatch");
    const file = manifest.files.find((candidate) => candidate.objectId === objectId);
    invariant(file !== undefined, "source read object identity is not registered");
    const offset = BigInt(offsetLow ?? 0) | (BigInt(offsetHigh ?? 0) << 32n);
    invariant(offset <= BigInt(file.byteLength), "source read offset is outside the object");
    invariant(
      Number.isInteger(requestedBytes) &&
        (requestedBytes ?? 0) > 0 &&
        (requestedBytes ?? 0) <= 1024 * 1024,
      "source read length is outside the transport bound",
    );
    const expectedBytes = Math.min(requestedBytes ?? 0, file.byteLength - Number(offset));
    const base = this.#sameOriginStorageUrl(
      this.#configuration.sourceApiUrl,
      "/api/source/v1/",
      "source API URL",
    );
    if (!base.pathname.endsWith("/")) {
      base.pathname += "/";
    }
    const url = new URL(
      `titles/${encodeURIComponent(manifest.titleFingerprint)}/` +
        `snapshots/${encodeURIComponent(manifest.snapshotId)}/` +
        `sources/${sourceId}/objects/${objectId}`,
      base,
    );
    url.searchParams.set("offset", offset.toString());
    url.searchParams.set("length", String(requestedBytes));
    const controller = new AbortController();
    invariant(
      !this.#sourceReadControllers.has(metadata.requestId),
      "source read request already owns a fetch",
    );
    this.#sourceReadControllers.set(metadata.requestId, controller);
    try {
      const response = await fetch(url, {
        method: "GET",
        cache: "no-store",
        signal: controller.signal,
      });
      invariant(response.status === 206, `source read failed with ${response.status}`);
      invariant(
        response.headers.get("x-vine-title-fingerprint") === manifest.titleFingerprint &&
          response.headers.get("x-vine-snapshot-id") === manifest.snapshotId &&
          response.headers.get("x-vine-source-id") === String(sourceId) &&
          response.headers.get("x-vine-object-id") === String(objectId) &&
          response.headers.get("x-vine-offset") === offset.toString() &&
          response.headers.get("x-vine-requested-bytes") === String(requestedBytes) &&
          response.headers.get("x-vine-transferred-bytes") === String(expectedBytes),
        "source read response identity mismatch",
      );
      const payload = new Uint8Array(await response.arrayBuffer());
      invariant(payload.byteLength === expectedBytes, "source read response length mismatch");
      const control = new Uint8Array(32);
      const view = new DataView(control.buffer);
      [
        1,
        sourceId ?? 0,
        objectId ?? 0,
        offsetLow ?? 0,
        offsetHigh ?? 0,
        requestedBytes ?? 0,
        payload.byteLength,
        0,
      ].forEach((word, index) => view.setUint32(index * 4, word, true));
      this.#completeSourceRead(metadata, control, payload);
    } finally {
      this.#sourceReadControllers.delete(metadata.requestId);
    }
  }

  async #serviceProfileRead(metadata: HostRequestMetadata): Promise<void> {
    const profile =
      this.#writableProfile ?? failInvariant("profile read has no registered profile");
    invariant(metadata.service === "storage.profile-read.v1", "unexpected profile read service");
    invariant(metadata.arguments.length === 6, "profile read request must contain six words");
    const [version, sourceId, objectId, offsetLow, offsetHigh, requestedBytes] =
      metadata.arguments;
    invariant(version === 1 && sourceId === 0, "profile read transport identity mismatch");
    const file = profile.files.find((candidate) => candidate.objectId === objectId);
    invariant(file !== undefined && file.exists, "profile read object is not committed");
    const offset = BigInt(offsetLow ?? 0) | (BigInt(offsetHigh ?? 0) << 32n);
    invariant(offset <= BigInt(file.byteLength), "profile read offset is outside the object");
    invariant(
      Number.isInteger(requestedBytes) &&
        (requestedBytes ?? 0) > 0 &&
        (requestedBytes ?? 0) <= 1024 * 1024,
      "profile read length is outside the transport bound",
    );
    const expectedBytes = Math.min(requestedBytes ?? 0, file.byteLength - Number(offset));
    const base = this.#sameOriginStorageUrl(
      this.#configuration.writableProfileApiUrl,
      "/api/profile/v1/",
      "writable profile API URL",
    );
    if (!base.pathname.endsWith("/")) {
      base.pathname += "/";
    }
    const url = new URL(
      `titles/${encodeURIComponent(profile.titleFingerprint)}/` +
        `profiles/${encodeURIComponent(profile.profileId)}/objects/${objectId}`,
      base,
    );
    url.searchParams.set("generation", String(file.generation));
    url.searchParams.set("offset", offset.toString());
    url.searchParams.set("length", String(requestedBytes));
    const controller = new AbortController();
    invariant(
      !this.#sourceReadControllers.has(metadata.requestId),
      "profile read request already owns a fetch",
    );
    this.#sourceReadControllers.set(metadata.requestId, controller);
    try {
      const response = await fetch(url, {
        method: "GET",
        cache: "no-store",
        signal: controller.signal,
      });
      invariant(response.status === 206, `profile read failed with ${response.status}`);
      invariant(
        response.headers.get("x-vine-title-fingerprint") === profile.titleFingerprint &&
          response.headers.get("x-vine-profile-id") === profile.profileId &&
          response.headers.get("x-vine-object-id") === String(objectId) &&
          response.headers.get("x-vine-generation") === String(file.generation) &&
          response.headers.get("x-vine-offset") === offset.toString() &&
          response.headers.get("x-vine-requested-bytes") === String(requestedBytes) &&
          response.headers.get("x-vine-transferred-bytes") === String(expectedBytes),
        "profile read response identity mismatch",
      );
      const payload = new Uint8Array(await response.arrayBuffer());
      invariant(payload.byteLength === expectedBytes, "profile read response length mismatch");
      const control = new Uint8Array(32);
      const view = new DataView(control.buffer);
      [
        1,
        0,
        objectId ?? 0,
        offsetLow ?? 0,
        offsetHigh ?? 0,
        requestedBytes ?? 0,
        payload.byteLength,
        0,
      ].forEach((word, index) => view.setUint32(index * 4, word, true));
      this.#completeSourceRead(metadata, control, payload);
    } finally {
      this.#sourceReadControllers.delete(metadata.requestId);
    }
  }

  async #serviceWritableCommit(metadata: HostRequestMetadata): Promise<void> {
    const profile =
      this.#writableProfile ?? failInvariant("writable commit has no registered profile");
    invariant(
      metadata.service === "storage.profile-commit.v1",
      "unexpected writable commit service",
    );
    invariant(metadata.arguments.length === 7, "writable commit request must contain seven words");
    const [
      version,
      objectId,
      baseLow,
      baseHigh,
      byteLengthLow,
      byteLengthHigh,
      closeAfterCommit,
    ] = metadata.arguments;
    invariant(version === 1, "writable commit request version mismatch");
    const baseGeneration = BigInt(baseLow ?? 0) | (BigInt(baseHigh ?? 0) << 32n);
    const byteLength = BigInt(byteLengthLow ?? 0) | (BigInt(byteLengthHigh ?? 0) << 32n);
    invariant(
      baseGeneration <= BigInt(Number.MAX_SAFE_INTEGER) &&
        byteLength <= BigInt(profile.quotaBytes) &&
        (closeAfterCommit === 0 || closeAfterCommit === 1),
      "writable commit request exceeds the profile bounds",
    );
    const file = profile.files.find((candidate) => candidate.objectId === objectId);
    invariant(file !== undefined, "writable commit object is not allowlisted");
    invariant(
      BigInt(file.generation) === baseGeneration,
      "writable commit base generation differs from the browser snapshot",
    );
    const payloadOutput = this.#requireBridge().writableCommitPayload<WritableCommitPayloadMetadata>(
      BigInt(metadata.requestId),
    );
    invariant(
      payloadOutput.metadata.kind === "writable-commit-payload" &&
        payloadOutput.metadata.requestId === metadata.requestId &&
        payloadOutput.control.byteLength === 0 &&
        BigInt(payloadOutput.payload.byteLength) === byteLength,
      "writable commit payload identity mismatch",
    );
    const request = payloadOutput.metadata.request;
    invariant(
      request.objectId === objectId &&
        BigInt(request.baseGeneration) === baseGeneration &&
        BigInt(request.byteLength) === byteLength &&
        request.closeAfterCommit === (closeAfterCommit === 1) &&
        /^[0-9a-f]{64}$/u.test(request.contentSha256),
      "writable commit Rust metadata disagrees with the transport words",
    );
    const contentSha256 = await sha256Hex(payloadOutput.payload);
    invariant(contentSha256 === request.contentSha256, "writable commit payload hash mismatch");
    const base = this.#sameOriginStorageUrl(
      this.#configuration.writableProfileApiUrl,
      "/api/profile/v1/",
      "writable profile API URL",
    );
    if (!base.pathname.endsWith("/")) {
      base.pathname += "/";
    }
    const url = new URL(
      `titles/${encodeURIComponent(profile.titleFingerprint)}/` +
        `profiles/${encodeURIComponent(profile.profileId)}/objects/${objectId}/commits`,
      base,
    );
    const requestKey = `${this.#profileSessionKey}:${metadata.requestId}`;
    let response: Response | null = null;
    let lastError: unknown = null;
    for (let attempt = 0; attempt < 2 && response === null; attempt += 1) {
      try {
        const candidate = await fetch(url, {
          method: "POST",
          cache: "no-store",
          headers: {
            "Content-Type": "application/octet-stream",
            "X-Vine-Request-Key": requestKey,
            "X-Vine-Base-Generation": baseGeneration.toString(),
            "X-Vine-Byte-Length": byteLength.toString(),
            "X-Vine-Content-Sha256": contentSha256,
          },
          body: ownedArrayBuffer(payloadOutput.payload),
        });
        if (candidate.status >= 500 && attempt === 0) {
          lastError = new Error(`writable commit failed with ${candidate.status}`);
          continue;
        }
        response = candidate;
      } catch (error) {
        lastError = error;
      }
    }
    if (response === null) {
      throw (lastError instanceof Error ? lastError : new Error(String(lastError)));
    }
    invariant(response.status === 200, `writable commit failed with ${response.status}`);
    invariant(
      response.headers.get("x-vine-title-fingerprint") === profile.titleFingerprint &&
        response.headers.get("x-vine-profile-id") === profile.profileId &&
        response.headers.get("x-vine-object-id") === String(objectId) &&
        response.headers.get("x-vine-base-generation") === baseGeneration.toString() &&
        response.headers.get("x-vine-content-sha256") === contentSha256,
      "writable commit response identity mismatch",
    );
    const control = new Uint8Array(await response.arrayBuffer());
    invariant(control.byteLength === 32, "writable commit response size mismatch");
    const completion = this.#completeWritableCommit(metadata, control);
    if (completion.succeeded) {
      file.exists = true;
      file.generation = completion.committedGeneration;
      file.byteLength = payloadOutput.payload.byteLength;
      file.sha256 = contentSha256;
      file.bytes = [...payloadOutput.payload];
    }
  }

  #completeSourceRead(
    request: HostRequestMetadata,
    control: Uint8Array,
    payload: Uint8Array,
  ): void {
    if (this.#terminal) {
      return;
    }
    const pending = this.#pendingHostRequests.complete(request.requestId, request.threadId, 1);
    const output = this.#requireBridge().completeSourceRead<HostCompletionMetadata>(
      BigInt(pending.requestId),
      control,
      payload,
    );
    const completion = output.metadata;
    invariant(completion.kind === "host-complete", "invalid source completion metadata");
    invariant(completion.threadId === pending.threadId, "Rust resumed a different source thread");
    invariant(completion.sourceRead !== undefined, "Rust omitted source completion evidence");
    this.#applyRequiredDelta(output);
    const resumeState = completion.scheduler.state === "in-callback" ? "guest-callback" : "ready";
    const expectedResumeState =
      pending.completionPolicy === "resume-callback" ? "guest-callback" : "ready";
    invariant(resumeState === expectedResumeState, "source completion resume policy mismatch");
    if (this.#requirePrepared().workload === "private-bgi") {
      this.#recordBgiCompletion(completion);
    }
    this.#report.sourceReads.push(completion.sourceRead);
    this.#applyCpuResumeToThread(completion.threadId, completion.completion);
    this.#threadExecutionStates.set(completion.threadId, resumeState);
    this.#report.rustSchedulerTransitions += 1;
    pending.resolve({ result: 1, completionPolicy: pending.completionPolicy });
    if (this.#execution.state() === "waiting-host") {
      this.#execution.transition("host-completion", {
        cause: "source-read-completion",
        threadId: pending.threadId,
        requestId: pending.requestId,
      });
      this.#signalRunnable();
    }
    this.#status("running", `${pending.service} completed`);
  }

  #completeWritableCommit(
    request: HostRequestMetadata,
    control: Uint8Array,
  ): RuntimeWritableCommitSnapshot {
    if (this.#terminal) {
      throw new SessionStopped("writable commit completed after the session stopped");
    }
    const pending = this.#pendingHostRequests.complete(request.requestId, request.threadId, 1);
    const output = this.#requireBridge().completeWritableCommit<HostCompletionMetadata>(
      BigInt(pending.requestId),
      control,
    );
    const completion = output.metadata;
    invariant(
      completion.kind === "host-complete" &&
        completion.threadId === pending.threadId &&
        completion.writableCommit !== undefined &&
        output.control.byteLength === 0 &&
        output.payload.byteLength === 0,
      "invalid writable commit completion metadata",
    );
    const resumeState = completion.scheduler.state === "in-callback" ? "guest-callback" : "ready";
    const expectedResumeState =
      pending.completionPolicy === "resume-callback" ? "guest-callback" : "ready";
    invariant(resumeState === expectedResumeState, "writable commit resume policy mismatch");
    if (this.#requirePrepared().workload === "private-bgi") {
      this.#recordBgiCompletion(completion);
    }
    this.#report.writableCommits.push(completion.writableCommit);
    this.#applyCpuResumeToThread(completion.threadId, completion.completion);
    this.#threadExecutionStates.set(completion.threadId, resumeState);
    this.#report.rustSchedulerTransitions += 1;
    pending.resolve({
      result: completion.writableCommit.succeeded ? 1 : 0,
      completionPolicy: pending.completionPolicy,
    });
    if (this.#execution.state() === "waiting-host") {
      this.#execution.transition("host-completion", {
        cause: "writable-commit-completion",
        threadId: pending.threadId,
        requestId: pending.requestId,
      });
      this.#signalRunnable();
    }
    this.#status("running", `${pending.service} completed`);
    return completion.writableCommit;
  }

  cancel(): void {
    this.#terminate("cancelled", "Session cancelled by user");
  }

  async #runGuest(): Promise<void> {
    const engine = this.#requireEngine();
    const prepared = this.#requirePrepared();
    const endpointAddresses = new Set(prepared.endpoints.map(({ address }) => address));
    while (!this.#terminal) {
      this.#checkWatchdog();
      invariant(
        this.#liveThreadId === this.#currentThreadId,
        "guest execution started without a restored current thread context",
      );
      if (!this.#report.executedThreadIds.includes(this.#currentThreadId)) {
        this.#report.executedThreadIds.push(this.#currentThreadId);
      }
      if (this.#execution.state() === "ready") {
        this.#execution.transition("running", {
          cause: "execute-slice",
          threadId: this.#currentThreadId,
        });
      } else {
        invariant(
          this.#execution.state() === "guest-callback",
          `guest execution cannot start from ${this.#execution.state()}`,
        );
      }
      const sliceState = this.#execution.state();
      const sliceStartedAt = performance.now();
      const slice = engine.executeBudget(this.#configuration.instructionBudget);
      this.#report.maximumSliceMilliseconds = Math.max(
        this.#report.maximumSliceMilliseconds,
        performance.now() - sliceStartedAt,
      );
      this.#report.instructions += slice.instructions;
      this.#report.slices += 1;
      this.#reconcileDirtyPages();
      this.#checkWatchdog();

      if (slice.stop.kind === "budget-yield") {
        const resumeState = sliceState === "running" ? "ready" : "guest-callback";
        this.#execution.transition("budget-yield", {
          cause: "instruction-budget",
          threadId: this.#currentThreadId,
        });
        this.#report.budgetYields += 1;
        if (this.#report.budgetYields % 16 === 0) {
          this.#status(
            "running",
            `${this.#report.instructions.toLocaleString()} guest instructions; periodic yield`,
          );
        }
        await yieldToWorkerLoop();
        if (this.#terminal) {
          throw new SessionStopped("session ended during a budget yield");
        }
        this.#execution.transition(resumeState, {
          cause: "worker-yield-complete",
          threadId: this.#currentThreadId,
        });
        continue;
      }

      if (
        slice.stop.kind === "fault" &&
        (slice.stop.fault === "page-fault" || slice.stop.fault === "execute-protection") &&
        prepared.workload === "private-bgi"
      ) {
        this.#execution.transition("memory-fault", {
          cause:
            slice.stop.fault === "page-fault"
              ? "guest-page-fault"
              : "guest-execution-protection",
          threadId: this.#currentThreadId,
        });
        if (slice.stop.fault === "page-fault") {
          const pageFaultResolution = this.#servicePageFault(slice.stop);
          if (pageFaultResolution !== null) {
            this.#execution.transition(sliceState === "running" ? "ready" : "guest-callback", {
              cause: `${pageFaultResolution}-complete`,
              threadId: this.#currentThreadId,
            });
            continue;
          }
        }
        this.#completeExecutionStructuredStop(slice.stop, slice.context);
        continue;
      }

      invariant(
        slice.stop.kind === "trap" &&
          slice.stop.trap === "invalid-opcode" &&
          slice.stop.errorCode === -1,
        `guest stopped unexpectedly: ${JSON.stringify(slice.stop)}`,
      );
      invariant(
        slice.context.instructionPointer === slice.context.previousInstructionPointer,
        "trap instruction pointer provenance mismatch",
      );
      const address = slice.context.instructionPointer;
      if (endpointAddresses.has(address)) {
        this.#threadResumeStates.set(
          this.#currentThreadId,
          sliceState === "running" ? "ready" : "guest-callback",
        );
        this.#execution.transition("api-trap", {
          cause: "owned-api-trap",
          threadId: this.#currentThreadId,
        });
        await this.#dispatchApi(address, slice.context.esp);
      } else if (address === prepared.callbackSentinel) {
        invariant(sliceState === "guest-callback", "callback return escaped callback execution");
        this.#execution.transition("callback-return", {
          cause: "callback-return-sentinel",
          threadId: this.#currentThreadId,
        });
        this.#completeCallback(slice.context.eax);
      } else if (address === prepared.entrySentinel) {
        invariant(sliceState === "running", "thread entry returned from a callback");
        if (this.#currentThreadId === this.#initialThreadId) {
          this.#completeProcess(slice.context.eax);
        } else {
          await this.#completeThread(slice.context.eax);
        }
      } else {
        const resolution = this.#requireBridge().resolveComEndpoint<ComEndpointResolutionMetadata>(
          address,
        );
        invariant(
          resolution.control.byteLength === 0 && resolution.payload.byteLength === 0,
          "COM endpoint resolution returned a memory delta",
        );
        invariant(
          resolution.metadata.kind === "com-endpoint-resolution" &&
            resolution.metadata.endpointAddress === address,
          "COM endpoint resolution identity mismatch",
        );
        invariant(
          resolution.metadata.memory.generation === Number(this.#generation) &&
            resolution.metadata.memory.acknowledgedGeneration === Number(this.#generation),
          "COM endpoint resolution observed unacknowledged memory",
        );
        if (!resolution.metadata.live) {
          throw new Error(`guest reached unowned #UD at 0x${address.toString(16)}`);
        }
        this.#threadResumeStates.set(
          this.#currentThreadId,
          sliceState === "running" ? "ready" : "guest-callback",
        );
        this.#execution.transition("api-trap", {
          cause: "owned-com-trap",
          threadId: this.#currentThreadId,
        });
        this.#dispatchCom(address, slice.context.esp);
      }
      await yieldToWorkerLoop();
    }
  }

  async #dispatchApi(endpoint: number, stackPointer: number): Promise<void> {
    const engine = this.#requireEngine();
    const returnAddress = engine.readGuestU32(stackPointer);
    const output = this.#requireBridge().dispatch<
      HostRequestMetadata | GuestCallbackMetadata | CompletionMetadata | ApiFaultMetadata
    >(endpoint, stackPointer, returnAddress);
    const metadata = output.metadata;
    switch (metadata.kind) {
      case "host-request": {
        invariant(
          output.control.byteLength === 0 && output.payload.byteLength === 0,
          "host request unexpectedly returned a delta",
        );
        this.#report.hostRequests += 1;
        this.#status("waiting-host", metadata.service);
        this.#postHost(metadata);
        await this.#scheduleAfterBlock(metadata.threadId, metadata.requestId);
        break;
      }
      case "guest-callback": {
        this.#takeApiResumeState();
        this.#applyRequiredDelta(output);
        if (this.#requirePrepared().workload === "private-bgi") {
          invariant(
            !this.#bgiCallbackCallSequences.has(metadata.frameId),
            "BGI callback frame reused an active call sequence",
          );
          this.#nextBgiCallSequence += 1;
          this.#bgiCallbackCallSequences.set(metadata.frameId, this.#nextBgiCallSequence);
        }
        this.#report.callbackEntries += 1;
        this.#report.maximumCallbackDepth = Math.max(
          this.#report.maximumCallbackDepth,
          metadata.depth,
        );
        engine.enterGuestCallback(metadata.target, metadata.stackPointer);
        this.#threadExecutionStates.set(this.#currentThreadId, "guest-callback");
        this.#execution.transition("guest-callback", {
          cause: "guest-callback-enter",
          threadId: this.#currentThreadId,
        });
        this.#status("callback", `Guest callback depth ${metadata.depth}`);
        break;
      }
      case "api-complete": {
        const resumeState = this.#takeApiResumeState();
        this.#applyOptionalDelta(output);
        if (metadata.createdThread !== undefined) {
          this.#registerCreatedThread(metadata.createdThread);
        }
        if (this.#requirePrepared().workload === "private-bgi") {
          this.#recordBgiCompletion(metadata);
        }
        engine.applyCpuResume(metadata.completion);
        this.#threadExecutionStates.set(this.#currentThreadId, resumeState);
        const runnableThreadId =
          metadata.createdThread?.schedulerState.state === "runnable"
            ? metadata.createdThread.thread.id
            : metadata.resumedThread?.schedulerState.state === "runnable" &&
                metadata.resumedThread.previousSuspendCount > 0
              ? metadata.resumedThread.threadId
              : null;
        if (runnableThreadId !== null) {
          invariant(
            this.#threadContexts.has(runnableThreadId),
            "runnable API-created guest thread has no CPU context",
          );
        }
        this.#completeApiExecutionBoundary(resumeState, metadata.threadSelection);
        break;
      }
      case "api-fault":
        this.#takeApiResumeState();
        this.#completeStructuredStop(metadata, output);
        break;
      default:
        throw new Error("runtime bridge returned an unknown dispatch outcome");
    }
  }

  #dispatchCom(endpoint: number, stackPointer: number): void {
    const engine = this.#requireEngine();
    const returnAddress = engine.readGuestU32(stackPointer);
    const output = this.#requireBridge().dispatchCom<ComCompletionMetadata | ComFaultMetadata>(
      endpoint,
      stackPointer,
      returnAddress,
    );
    const metadata = output.metadata;
    switch (metadata.kind) {
      case "com-complete": {
        const resumeState = this.#takeApiResumeState();
        invariant(
          metadata.call.frame.endpointAddress === endpoint &&
            metadata.call.frame.callSite === returnAddress &&
            metadata.call.frame.stackPointer === stackPointer &&
            metadata.call.frame.returnAddress === returnAddress &&
            metadata.call.frame.returnStackPointer === metadata.completion.stackPointer,
          "COM completion frame identity mismatch",
        );
        this.#applyOptionalDelta(output);
        if (this.#requirePrepared().workload === "private-bgi") {
          this.#recordBgiComCompletion(metadata);
        }
        engine.applyCpuResume(metadata.completion);
        this.#threadExecutionStates.set(this.#currentThreadId, resumeState);
        this.#execution.transition(resumeState, {
          cause: "synchronous-com-complete",
          threadId: this.#currentThreadId,
        });
        break;
      }
      case "com-fault":
        this.#takeApiResumeState();
        this.#completeStructuredStop(metadata, output);
        break;
      default:
        throw new Error("runtime bridge returned an unknown COM dispatch outcome");
    }
  }

  #recordBgiComCompletion(metadata: ComCompletionMetadata): void {
    const completion = metadata.call;
    const frame = completion.frame;
    const sequence = ++this.#nextBgiCallSequence;
    const call = {
      sequence,
      module: "<com>",
      symbol: `${frame.interface}.${frame.method}`,
      interface: frame.interface,
      vtableSlot: frame.vtableSlot,
      arguments: [frame.thisPointer, ...frame.explicitArguments],
      result: completion.returnValue,
      returnAddress: frame.returnAddress,
      stackPointerBefore: frame.stackPointer,
      stackPointerAfter: metadata.completion.stackPointer,
      referenceCount: completion.referenceCount,
      objectRetired: completion.objectRetired,
      guestWrites: completion.guestWrites.map(({ address, bytes }) => ({
        address,
        bytes,
      })),
    };
    this.#semanticCallDigest.update(call);
    this.#report.supportedCallCount += 1;
    if (this.#configuration.evidenceLevel === "full") {
      this.#report.supportedCalls.push(call);
    }
  }

  #recordBgiCompletion(
    metadata: CompletionMetadata | CallbackCompletionMetadata | HostCompletionMetadata,
    reservedSequence?: number,
  ): void {
    const endpoint = metadata.endpoint;
    invariant(endpoint !== undefined, "BGI completion omitted its endpoint identity");
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
    const isSetCurrentDirectory =
      completion.implementationId === "kernel32.set-current-directory-a.v1";
    invariant(
      isSetCurrentDirectory === (metadata.currentDirectory !== undefined),
      "BGI current-directory completion metadata mismatch",
    );
    const isCreateMutex = completion.implementationId === "kernel32.create-mutex-a.v1";
    invariant(
      isCreateMutex === (metadata.mutex !== undefined),
      "BGI mutex completion metadata mismatch",
    );
    const isCreateWaitableTimer =
      completion.implementationId === "kernel32.create-waitable-timer-a.v1";
    invariant(
      isCreateWaitableTimer === (metadata.waitableTimer !== undefined),
      "BGI waitable timer completion metadata mismatch",
    );
    const isCreateThread = completion.implementationId === "kernel32.create-thread.v1";
    invariant(
      isCreateThread === (metadata.createdThread !== undefined),
      "BGI CreateThread completion metadata mismatch",
    );
    const isResumeThread = completion.implementationId === "kernel32.resume-thread.v1";
    invariant(
      isResumeThread === (metadata.resumedThread !== undefined),
      "BGI ResumeThread completion metadata mismatch",
    );
    const isIconLoad =
      completion.implementationId === "user32.load-icon-a.v1" ||
      completion.implementationId === "user32.load-image-a.v1";
    invariant(
      isIconLoad === (metadata.icon !== undefined),
      "BGI icon completion metadata mismatch",
    );
    const isLoadCursor = completion.implementationId === "user32.load-cursor-a.v1";
    invariant(
      isLoadCursor === (metadata.cursor !== undefined),
      "BGI cursor completion metadata mismatch",
    );
    const isRegisterClass =
      completion.implementationId === "user32.register-class-ex-a.v1";
    invariant(
      isRegisterClass === (metadata.windowClass !== undefined),
      "BGI window class completion metadata mismatch",
    );
    const isLogicalWindowCompletion =
      completion.implementationId === "user32.create-window-ex-a.v1" ||
      completion.implementationId === "user32.set-window-pos.v1" ||
      completion.implementationId === "user32.set-window-pos.v2" ||
      completion.implementationId === "user32.set-window-long-a.gwl-style.v1";
    invariant(
      isLogicalWindowCompletion === (metadata.window !== undefined),
      "BGI logical window completion metadata mismatch",
    );
    const isCoInitialize = completion.implementationId === "ole32.co-initialize.v1";
    invariant(
      isCoInitialize === (metadata.comApartment !== undefined),
      "BGI COM apartment completion metadata mismatch",
    );
    const isGdiCompletion = [
      "user32.get-dc.window-or-screen-acquisition.v2",
      "gdi32.enum-font-families-ex-a.cp932-screen.v1",
      "gdi32.create-font-a.cp932-logical-object.v1",
      "gdi32.create-compatible-dc.null-screen-memory.v1",
      "gdi32.create-dib-section.primary-8bpp.v1",
      "gdi32.get-object-a.font-or-dib.v1",
      "gdi32.get-outline-text-metrics-a.cp932-selected-font.v1",
      "gdi32.text-out-a.cp932-8bpp-raster.v1",
      "gdi32.select-object.memory-font-bitmap.v1",
      "gdi32.delete-object.font-or-dib.v1",
      "gdi32.delete-dc.logical-memory.v1",
      "gdi32.get-stock-object.black-brush.v1",
      "user32.fill-rect.logical-surface.v1",
      "user32.release-dc.window-or-screen-acquisition.v2",
    ].includes(completion.implementationId);
    invariant(
      isGdiCompletion === (metadata.gdi !== undefined),
      "BGI GDI completion metadata mismatch",
    );
    const isGdiplusCompletion = [
      "gdiplus.startup.primary-v1",
      "gdiplus.shutdown.logical-session-v1",
    ].includes(completion.implementationId);
    invariant(
      isGdiplusCompletion === (metadata.gdiplus !== undefined),
      "BGI GDI+ completion metadata mismatch",
    );
    const isImm32Completion =
      completion.implementationId ===
        "imm32.get-default-ime-window.thread-logical-v1" ||
      completion.implementationId === "imm32.associate-context.logical-window-v1" ||
      (completion.implementationId === "user32.send-message-a.v1" &&
        frame.arguments[1]?.words[0] === 0x0283);
    invariant(
      isImm32Completion === (metadata.imm32 !== undefined),
      "BGI IMM32 completion metadata mismatch",
    );
    const isVirtualAllocCompletion =
      completion.implementationId === "kernel32.virtual-alloc.reserve-top-down.v1";
    invariant(
      isVirtualAllocCompletion === (metadata.virtualMemory !== undefined),
      "BGI virtual-memory completion metadata mismatch",
    );
    const sequence = reservedSequence ?? ++this.#nextBgiCallSequence;
    const call = {
      sequence,
      module: endpoint.module,
      symbol:
        endpoint.symbol.kind === "ordinal"
          ? `#${endpoint.symbol.ordinal}`
          : endpoint.symbol.name,
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
      ...(metadata.currentDirectory === undefined
        ? {}
        : { currentDirectory: metadata.currentDirectory }),
      ...(metadata.mutex === undefined ? {} : { mutex: metadata.mutex }),
      ...(metadata.waitableTimer === undefined
        ? {}
        : { waitableTimer: metadata.waitableTimer }),
      ...(metadata.createdThread === undefined
        ? {}
        : { createdThread: metadata.createdThread }),
      ...(metadata.resumedThread === undefined
        ? {}
        : { resumedThread: metadata.resumedThread }),
      ...(metadata.icon === undefined ? {} : { icon: metadata.icon }),
      ...(metadata.cursor === undefined ? {} : { cursor: metadata.cursor }),
      ...(metadata.windowClass === undefined
        ? {}
        : { windowClass: metadata.windowClass }),
      ...(metadata.window === undefined ? {} : { window: metadata.window }),
      ...(metadata.comApartment === undefined
        ? {}
        : { comApartment: metadata.comApartment }),
      ...(metadata.sourceRead === undefined ? {} : { sourceRead: metadata.sourceRead }),
      ...(metadata.writableCommit === undefined
        ? {}
        : { writableCommit: metadata.writableCommit }),
      ...(metadata.gdi === undefined ? {} : { gdi: metadata.gdi }),
      ...(metadata.gdiplus === undefined ? {} : { gdiplus: metadata.gdiplus }),
      ...(metadata.imm32 === undefined ? {} : { imm32: metadata.imm32 }),
      ...(metadata.virtualMemory === undefined
        ? {}
        : { virtualMemory: metadata.virtualMemory }),
    };
    this.#semanticCallDigest.update(call);
    this.#report.supportedCallCount += 1;
    if (this.#configuration.evidenceLevel === "full") {
      this.#report.supportedCalls.push(call);
    }
  }

  #completeStructuredStop(
    metadata: ApiFaultMetadata | ComFaultMetadata,
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

  #servicePageFault(
    stop: Extract<ExecutionStop, { kind: "fault" }>,
  ): "initial-stack-growth" | "private-heap-page-materialization" | null {
    invariant(stop.fault === "page-fault", "page-fault service requires a page fault");
    invariant(stop.faultAddress !== undefined, "page fault omitted its address");
    const output = this.#requireBridge().servicePageFault<
      PageFaultResolvedMetadata | PageFaultRejectedMetadata
    >(stop.faultAddress, stop.errorCode);
    const metadata = output.metadata;
    if (metadata.kind === "page-fault-rejected") {
      invariant(
        output.control.byteLength === 0 && output.payload.byteLength === 0,
        "rejected page fault returned a memory delta",
      );
      invariant(metadata.faultAddress === stop.faultAddress, "rejected fault address changed");
      invariant(
        metadata.pageFaultErrorCode === stop.errorCode,
        "rejected page-fault code changed",
      );
      invariant(
        metadata.memory.generation === Number(this.#generation) &&
          metadata.memory.acknowledgedGeneration === Number(this.#generation),
        "rejected page fault changed memory",
      );
      return null;
    }

    const resolution = metadata.resolution;
    const resolvedFault =
      resolution.kind === "initial-thread-stack-growth"
        ? resolution.growth
        : resolution.materialization;
    invariant(resolvedFault.faultAddress === stop.faultAddress, "resolved fault address changed");
    invariant(
      resolvedFault.pageFaultErrorCode === stop.errorCode,
      "resolved page-fault code changed",
    );
    invariant(metadata.fromGeneration === Number(this.#generation), "fault delta base changed");
    const delta = decodeHostDelta(output.control, output.payload);
    invariant(delta.fromGeneration === this.#generation, "fault delta base generation mismatch");
    invariant(
      delta.generation === BigInt(metadata.generation),
      "fault delta generation metadata mismatch",
    );
    invariant(
      delta.mappingOperations.length === 1 &&
        delta.mappingOperations[0]?.operation === "commit",
      "page-fault resolution did not contain exactly one commit operation",
    );
    const operation = delta.mappingOperations[0];
    invariant(operation.operation === "commit", "page-fault mapping is not a commit");
    invariant(
      operation.owner === resolvedFault.memoryOwner &&
        operation.range.base === resolvedFault.committedPage.start &&
        operation.range.byteLength === resolvedFault.committedPage.byteLength,
      "page-fault commit identity mismatch",
    );
    invariant(
      operation.protection.read && operation.protection.write && !operation.protection.execute,
      "page-fault commit protection mismatch",
    );
    invariant(delta.pages.length === 1, "page-fault resolution did not return one page");
    const page = delta.pages[0];
    invariant(page !== undefined, "page-fault page disappeared after validation");
    invariant(
      page.address === operation.range.base &&
        page.owner === operation.owner &&
        page.protection.read &&
        page.protection.write &&
        !page.protection.execute,
      "page-fault page ownership or protection mismatch",
    );

    if (resolution.kind === "initial-thread-stack-growth") {
      const growth = resolution.growth;
      invariant(growth.threadId === this.#currentThreadId, "stack growth belongs to the wrong thread");
      const preparedStack = this.#requirePrepared().initialThreadStack;
      invariant(preparedStack !== undefined, "prepared stack identity is unavailable");
      invariant(
        growth.addressAllocationId === preparedStack.addressAllocationId &&
          growth.memoryOwner === preparedStack.memoryOwner,
        "stack growth authority identity changed",
      );
      invariant(
        page.ownerSealed && page.bytes.every((byte) => byte === 0),
        "stack growth page bytes or sealing mismatch",
      );
      this.#report.stackGrowths.push({
        instructionCount: this.#report.instructions,
        faultAddress: growth.faultAddress,
        pageFaultErrorCode: growth.pageFaultErrorCode,
        addressAllocationId: growth.addressAllocationId,
        memoryOwner: growth.memoryOwner,
        committedPage: growth.committedPage,
        previousGuardPage: growth.previousGuardPage,
        ...(growth.nextGuardPage === undefined ? {} : { nextGuardPage: growth.nextGuardPage }),
        committedPages: growth.committedPages,
        fromGeneration: metadata.fromGeneration,
        generation: metadata.generation,
      });
    } else {
      const materialization = resolution.materialization;
      invariant(
        materialization.allocation.address <= materialization.faultAddress &&
          materialization.faultAddress <
            materialization.allocation.address + materialization.allocation.byteLength,
        "heap materialization fault escaped its allocation",
      );
      const expectedFill =
        materialization.allocation.initialization === "zeroed" ? 0 : 0xa5;
      invariant(
        page.bytes.every((byte) => byte === expectedFill),
        "heap materialization initialization mismatch",
      );
      (this.#report.heapPageMaterializations ??= []).push({
        instructionCount: this.#report.instructions,
        ...materialization,
        fromGeneration: metadata.fromGeneration,
        generation: metadata.generation,
      });
    }
    this.#report.mappingDeltas += 1;
    this.#report.mappingOperations.commit += 1;
    this.#report.mappingDirtyPages += 1;
    this.#applyAuthoritativeDelta(delta);
    return resolution.kind === "initial-thread-stack-growth"
      ? "initial-stack-growth"
      : "private-heap-page-materialization";
  }

  #completeExecutionStructuredStop(
    stop: Extract<ExecutionStop, { kind: "fault" }>,
    context: ExecutionCpuContext,
  ): void {
    const prepared = this.#requirePrepared();
    invariant(prepared.workload === "private-bgi", "fixture returned an execution stop");
    invariant(
      stop.fault === "page-fault" || stop.fault === "execute-protection",
      "unsupported execution fault kind",
    );
    invariant(stop.faultAddress !== undefined, "execution fault omitted its guest address");
    const stackRegion =
      stop.fault === "page-fault"
        ? prepared.regions.find(
            ({ kind, range }) =>
              kind === "stack" &&
              ((stop.faultAddress! >= range.start &&
                stop.faultAddress! < range.start + range.byteLength) ||
                (range.start >= 0x1000 && stop.faultAddress === range.start - 0x1000)),
          )
        : undefined;
    const capabilityCluster =
      stop.fault === "execute-protection"
        ? "guest-memory-execute-protection"
        : stackRegion === undefined
          ? "guest-memory-page-fault"
          : "initial-thread-stack-growth";
    const reflection = this.#requireBridge().currentReflection<CurrentReflectionMetadata>().metadata;
    invariant(reflection.kind === "current-reflection", "invalid execution-stop reflection");
    invariant(
      reflection.generation === Number(this.#generation) &&
        reflection.acknowledgedGeneration === Number(this.#generation),
      "execution stop memory is not fully acknowledged",
    );
    invariant(/^[0-9a-f]{64}$/.test(reflection.contentSha256), "invalid memory digest");
    this.#report.firstUnsupported = {
      capabilityCluster,
      endpoint: null,
      fault: {
        class: "fault",
        stage: "guest-execution",
        code: stop.fault,
        message:
          `guest ${stop.fault} at 0x${stop.faultAddress.toString(16)} ` +
          `while executing 0x${context.instructionPointer.toString(16)}`,
        instructionPointer: context.instructionPointer,
        previousInstructionPointer: context.previousInstructionPointer,
        faultAddress: stop.faultAddress,
        pageFaultErrorCode: stop.errorCode,
      },
      instructionCount: this.#report.instructions,
      memoryContentSha256: reflection.contentSha256,
    };
    const cancellation = this.#requireBridge().cancel<CancellationMetadata>().metadata;
    invariant(cancellation.kind === "cancelled", "execution-stop cleanup metadata is invalid");
    invariant(cancellation.pendingHostCalls === 0, "execution stop retained host work");
    invariant(cancellation.pendingCallbacks === 0, "execution stop retained callbacks");
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
    const output = this.#requireBridge().completeCallback<
      CallbackCompletionMetadata | GuestCallbackMetadata
    >(prepared.callbackSentinel, result);
    const metadata = output.metadata;
    if (metadata.kind === "guest-callback") {
      invariant(
        metadata.replacedFrameId !== undefined,
        "replacement callback omitted the retired frame identity",
      );
      this.#applyRequiredDelta(output);
      if (prepared.workload === "private-bgi") {
        const sequence = this.#bgiCallbackCallSequences.get(metadata.replacedFrameId);
        invariant(sequence !== undefined, "BGI replacement callback has no reserved call sequence");
        this.#bgiCallbackCallSequences.delete(metadata.replacedFrameId);
        invariant(
          !this.#bgiCallbackCallSequences.has(metadata.frameId),
          "BGI replacement callback reused a live frame identity",
        );
        this.#bgiCallbackCallSequences.set(metadata.frameId, sequence);
      }
      this.#report.callbackEntries += 1;
      this.#report.maximumCallbackDepth = Math.max(
        this.#report.maximumCallbackDepth,
        metadata.depth,
      );
      this.#requireEngine().enterGuestCallback(metadata.target, metadata.stackPointer);
      this.#threadExecutionStates.set(this.#currentThreadId, "guest-callback");
      this.#execution.transition("guest-callback", {
        cause: "guest-callback-replacement",
        threadId: this.#currentThreadId,
      });
      this.#status("callback", `Guest callback replaced at depth ${metadata.depth}`);
      return;
    }
    this.#applyOptionalDelta(output);
    if (prepared.workload === "private-bgi") {
      const sequence = this.#bgiCallbackCallSequences.get(metadata.frameId);
      invariant(sequence !== undefined, "BGI callback completion has no reserved call sequence");
      this.#bgiCallbackCallSequences.delete(metadata.frameId);
      this.#recordBgiCompletion(metadata, sequence);
    }
    this.#requireEngine().applyCpuResume(metadata.completion);
    const resumeState = metadata.remainingDepth > 0 ? "guest-callback" : "ready";
    this.#threadExecutionStates.set(this.#currentThreadId, resumeState);
    this.#execution.transition(resumeState, {
      cause: metadata.remainingDepth > 0 ? "nested-callback-resume" : "callback-stack-empty",
      threadId: this.#currentThreadId,
    });
    this.#status("running", `Callback returned; depth ${metadata.remainingDepth}`);
  }

  #registerCreatedThread(created: CreatedThreadMetadata): void {
    const { thread, schedulerState } = created;
    const stack = thread.stack;
    const environment = thread.environment;
    const bootstrap = thread.guestBootstrap;
    invariant(
      Number.isInteger(created.handle) &&
        created.handle > 0 &&
        Number.isInteger(thread.id) &&
        thread.id > 0 &&
        thread.id <= 0xffff_ffff &&
        thread.lifecycle.state === "running" &&
        !this.#threadContexts.has(thread.id),
      "CreateThread returned an invalid or duplicate logical thread identity",
    );
    invariant(
      stack !== undefined &&
        environment !== undefined &&
        bootstrap !== undefined &&
        stack.threadId === thread.id &&
        environment.threadId === thread.id &&
        bootstrap.threadId === thread.id &&
        bootstrap.initialStackPointer === stack.initialStackPointer &&
        bootstrap.fsBase === environment.tebAddress &&
        bootstrap.returnSentinel === stack.returnSentinel &&
        bootstrap.returnSentinel === this.#requirePrepared().entrySentinel,
      "CreateThread stack, TEB, and CPU bootstrap metadata disagree",
    );
    invariant(
      schedulerState.state === "runnable" ||
        (schedulerState.state === "suspended" && schedulerState.count > 0),
      "CreateThread returned an invalid scheduler state",
    );
    this.#threadContexts.set(
      thread.id,
      this.#requireEngine().createCpuContext({
        entryPoint: bootstrap.entryPoint,
        initialStackPointer: bootstrap.initialStackPointer,
        fsBase: bootstrap.fsBase,
      }),
    );
    this.#threadExecutionStates.set(thread.id, "ready");
    this.#report.registeredThreadIds.push(thread.id);
  }

  #completeApiExecutionBoundary(
    callerResumeState: "ready" | "guest-callback",
    selection: ApiBoundaryThreadSelection | undefined,
  ): void {
    if (selection === undefined) {
      this.#execution.transition(callerResumeState, {
        cause: "synchronous-api-complete",
        threadId: this.#currentThreadId,
      });
      return;
    }
    invariant(
      selection.previousThreadId === this.#currentThreadId &&
        selection.threadId !== this.#currentThreadId &&
        (selection.state.state === "runnable" || selection.state.state === "in-callback"),
      "Rust API boundary returned an invalid thread selection",
    );
    this.#saveLiveThreadContext();
    this.#execution.transition("thread-switch", {
      cause: "api-boundary-quantum-expired",
      threadId: selection.threadId,
    });
    this.#activateThread(selection.threadId, selection.state);
    const selectedResumeState =
      selection.state.state === "in-callback" ? "guest-callback" : "ready";
    this.#execution.transition(selectedResumeState, {
      cause: "api-boundary-thread-context-restored",
      threadId: selection.threadId,
    });
    this.#report.rustSchedulerTransitions += 1;
  }

  async #completeThread(exitCode: number): Promise<void> {
    const threadId = this.#currentThreadId;
    const preparedThread = this.#requirePrepared().additionalThreads?.find(
      (thread) => thread.threadId === threadId,
    );
    this.#execution.transition("thread-exit", {
      cause: "guest-thread-entry-return",
      threadId,
    });
    const output = this.#requireBridge().finishThread<ThreadExitMetadata>(exitCode);
    const metadata = output.metadata;
    invariant(metadata.kind === "thread-exit", "invalid guest thread exit metadata");
    invariant(metadata.threadId === threadId, "Rust exited a different guest thread");
    invariant(
      metadata.scheduler.state === "terminated" &&
        metadata.scheduler.exitCode === metadata.exitCode,
      "guest thread exit code or scheduler state mismatch",
    );
    if (preparedThread !== undefined) {
      invariant(
        metadata.exitCode === preparedThread.expectedExitCode &&
          metadata.markerAddress === preparedThread.markerAddress &&
          metadata.markerValue === preparedThread.expectedMarkerValue,
        "controlled guest thread exit evidence mismatch",
      );
    } else {
      invariant(
        metadata.markerAddress === undefined && metadata.markerValue === undefined,
        "dynamic guest thread exit unexpectedly returned controlled marker evidence",
      );
    }
    this.#applyOptionalDelta(output);
    this.#report.threadExits.push({
      threadId,
      exitCode: metadata.exitCode,
      ...(metadata.markerAddress === undefined
        ? {}
        : { markerAddress: metadata.markerAddress, markerValue: metadata.markerValue }),
    });
    this.#threadContexts.delete(threadId);
    this.#threadResumeStates.delete(threadId);
    this.#threadExecutionStates.delete(threadId);
    this.#liveThreadId = null;
    await this.#scheduleAfterBlock(threadId, null);
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
    const engine = this.#requireEngine();
    const capacityBefore = engine.memoryCapacity();
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
        const before = engine.memoryContractState();
        const invalid = {
          ...delta,
          pages: delta.pages.map((page, index) =>
            index === 0 ? { ...page, owner: page.owner + 1 } : page,
          ),
        };
        let rejected = false;
        try {
          engine.applyMemoryDelta(invalid);
        } catch {
          rejected = true;
        }
        invariant(rejected, "invalid mapping delta was not rejected");
        invariant(
          engine.memoryContractState() === before,
          "rejected mapping delta changed execution memory",
        );
        this.#report.mappingAtomicRejections += 1;
      }

      const effectsBefore = engine.cacheEffectCounts();
      this.#applyAuthoritativeDelta(delta);
      const capacityAfter = engine.memoryCapacity();
      invariant(
        capacityAfter.physicalPoolBytes <= capacityAfter.maximumPhysicalPoolBytes &&
          capacityAfter.mappedGuestPages <= capacityAfter.maximumPhysicalPages,
        "execution mirror exceeded its bounded physical capacity",
      );
      if (stageIndex === 4) {
        invariant(
          capacityAfter.releasedPages > capacityBefore.releasedPages,
          "mirror decommit did not release physical storage",
        );
      }
      if (stageIndex === 5) {
        invariant(
          capacityAfter.reusedPages > capacityBefore.reusedPages,
          "mirror recommit did not reuse released physical storage",
        );
      }
      const effectsAfter = engine.cacheEffectCounts();
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
          engine.readGuestU32(metadata.probeAddress);
        } catch (error) {
          fault = error;
        }
        invariant(
          fault instanceof ExecutionMemoryAccessFault &&
            fault.code === "execution-address-unmapped",
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
    this.#status("mapping", "Execution-engine mapping contract passed");
  }

  #reconcileDirtyPages(): void {
    const pages = this.#requireEngine().collectChangedPageViews();
    if (pages.length === 0) {
      return;
    }
    // The bridge copies these engine-owned views synchronously before execution can resume.
    this.#generation = this.#requireBridge().reconcileWritebackPages(
      this.#generation,
      pages,
    );
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
    this.#requireEngine().applyMemoryDelta(delta);
    try {
      this.#requireBridge().acknowledgeFast(delta.generation);
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
    this.#requireEngine().rebuildMemory(reflection, metadata.regions);
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

  #postHost(metadata: HostRequestMetadata): void {
    invariant(this.#execution.state() === "api-trap", "host request escaped API trap state");
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
    invariant(metadata.threadId === this.#currentThreadId, "host request belongs to another thread");
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
    const apiResumeState = this.#takeApiResumeState(metadata.threadId);
    const completionPolicy: HostCompletionPolicy =
      apiResumeState === "guest-callback" ? "resume-callback" : "resume-ready";
    this.#pendingHostRequests.register({
      requestId: metadata.requestId,
      threadId: metadata.threadId,
      service: metadata.service,
      completionPolicy,
      cancelState: "active",
      resolve: () => {},
      reject: () => {},
    });
    if (this.#configuration.mode === "watchdog" && this.#watchdogArmedAt === null) {
      this.#armWatchdog();
    }
    if (metadata.service === "storage.source-read.v1") {
      if (this.#configuration.mode === "success") {
        void this.#serviceSourceRead(metadata).catch((error: unknown) => {
          if (!this.#terminal) {
            this.#fail(error instanceof Error ? error : new Error(String(error)));
          }
        });
      }
    } else if (metadata.service === "storage.profile-read.v1") {
      if (this.#configuration.mode === "success") {
        void this.#serviceProfileRead(metadata).catch((error: unknown) => {
          if (!this.#terminal) {
            this.#fail(error instanceof Error ? error : new Error(String(error)));
          }
        });
      }
    } else if (metadata.service === "storage.profile-commit.v1") {
      if (this.#configuration.mode === "success") {
        void this.#serviceWritableCommit(metadata).catch((error: unknown) => {
          if (!this.#terminal) {
            this.#fail(error instanceof Error ? error : new Error(String(error)));
          }
        });
      }
    } else {
      post(request);
    }
  }

  #takeApiResumeState(threadId = this.#currentThreadId): "ready" | "guest-callback" {
    const resumeState = this.#threadResumeStates.get(threadId);
    invariant(resumeState !== undefined, "API trap omitted its thread resume state");
    this.#threadResumeStates.delete(threadId);
    return resumeState;
  }

  async #scheduleAfterBlock(blockedThreadId: number, requestId: string | null): Promise<void> {
    this.#saveLiveThreadContext();
    while (!this.#terminal) {
      const metadata = this.#requireBridge().scheduleNextThread<
        ThreadScheduledMetadata | SchedulerIdleMetadata
      >().metadata;
      if (metadata.kind === "thread-scheduled") {
        invariant(
          metadata.scheduler.state === "runnable" ||
            metadata.scheduler.state === "in-callback",
          "Rust scheduled a non-executable guest thread",
        );
        this.#execution.transition("thread-switch", {
          cause: "rust-scheduler-selection",
          threadId: metadata.threadId,
          requestId,
        });
        this.#activateThread(metadata.threadId, metadata.scheduler);
        if (requestId !== null && metadata.threadId !== blockedThreadId) {
          this.#report.hostWaitExecutions.push({
            requestId,
            waitingThreadId: blockedThreadId,
            executedThreadId: metadata.threadId,
          });
        }
        const executionState =
          metadata.scheduler.state === "in-callback" ? "guest-callback" : "ready";
        this.#execution.transition(executionState, {
          cause: "thread-context-restored",
          threadId: metadata.threadId,
          requestId,
        });
        return;
      }
      invariant(metadata.kind === "scheduler-idle", "Rust returned an unknown scheduler outcome");
      invariant(
        this.#pendingHostRequests.size() > 0,
        "scheduler became idle without a pending host request",
      );
      this.#liveThreadId = null;
      const pending = this.#pendingHostRequests.first();
      invariant(pending !== null, "pending host request disappeared before scheduler idle");
      if (this.#execution.state() !== "waiting-host") {
        this.#execution.transition("waiting-host", {
          cause: "scheduler-no-executable-thread",
          threadId: pending.threadId,
          requestId: pending.requestId,
        });
      }
      await this.#waitForRunnable();
      if (this.#terminal) {
        throw new SessionStopped("session ended while the scheduler was idle");
      }
      invariant(
        this.#execution.state() === "host-completion",
        "scheduler wake did not originate from a host completion",
      );
    }
  }

  #saveLiveThreadContext(): void {
    if (this.#liveThreadId === null) {
      return;
    }
    invariant(
      this.#threadContexts.has(this.#liveThreadId),
      "live guest thread has no CPU context record",
    );
    this.#threadContexts.set(this.#liveThreadId, this.#requireEngine().saveCpuContext());
  }

  #activateThread(threadId: number, scheduler: ExecutableSchedulerMetadata): void {
    const context = this.#threadContexts.get(threadId);
    invariant(context !== undefined, `scheduled guest thread ${threadId} has no CPU context`);
    const executionState = scheduler.state === "in-callback" ? "guest-callback" : "ready";
    invariant(
      this.#threadExecutionStates.get(threadId) === executionState,
      `Rust scheduler state for thread ${threadId} disagrees with its suspended execution state`,
    );
    const previousThreadId = this.#currentThreadId;
    this.#requireEngine().restoreCpuContext(context);
    this.#currentThreadId = threadId;
    this.#liveThreadId = threadId;
    if (previousThreadId !== threadId) {
      this.#report.contextSwitches += 1;
    }
  }

  #applyCpuResumeToThread(threadId: number, completion: ApiCompletion): void {
    const target = this.#threadContexts.get(threadId);
    invariant(target !== undefined, `host completion thread ${threadId} has no CPU context`);
    const engine = this.#requireEngine();
    const liveThreadId = this.#liveThreadId;
    if (liveThreadId !== null) {
      this.#threadContexts.set(liveThreadId, engine.saveCpuContext());
    }
    engine.restoreCpuContext(target);
    engine.applyCpuResume(completion);
    this.#threadContexts.set(threadId, engine.saveCpuContext());
    if (liveThreadId !== null) {
      const live = this.#threadContexts.get(liveThreadId);
      invariant(live !== undefined, "live guest thread context disappeared during host completion");
      engine.restoreCpuContext(live);
    }
  }

  #waitForRunnable(): Promise<void> {
    return new Promise((resolve) => {
      this.#runnableResolvers.push(resolve);
    });
  }

  #signalRunnable(): void {
    const resolve = this.#runnableResolvers.shift();
    invariant(resolve !== undefined, "host completion has no scheduler-idle waiter");
    resolve();
  }

  #checkWatchdog(): void {
    if (this.#report.instructions > this.#configuration.watchdogInstructionLimit) {
      this.#triggerWatchdog("instruction limit");
      throw new SessionStopped("watchdog terminated the guest");
    }
    if (
      this.#watchdogArmedAt !== null &&
      performance.now() - this.#watchdogArmedAt > this.#configuration.watchdogWallMilliseconds
    ) {
      this.#triggerWatchdog("wall-clock limit");
      throw new SessionStopped("watchdog terminated the guest");
    }
  }

  #armWatchdog(): void {
    invariant(this.#watchdogTimer === null, "watchdog timer is already armed");
    invariant(this.#watchdogArmedAt === null, "watchdog clock is already armed");
    this.#watchdogArmedAt = performance.now();
    this.#watchdogTimer = worker.setTimeout(() => {
      this.#watchdogTimer = null;
      this.#triggerWatchdog("wall-clock limit");
    }, this.#configuration.watchdogWallMilliseconds);
  }

  #triggerWatchdog(reason: "instruction limit" | "wall-clock limit"): void {
    if (this.#terminal) {
      return;
    }
    const execution = this.#execution.snapshot();
    const pending = this.#pendingHostRequests.first();
    const stoppedThreadId = this.#liveThreadId ?? pending?.threadId ?? this.#currentThreadId;
    this.#watchdogContext = {
      threadId: stoppedThreadId,
      requestId: pending?.requestId ?? null,
      lastTransition: execution.lastTransition,
    };
    this.#terminate(
      "watchdog",
      `Watchdog ${reason} stopped thread ${stoppedThreadId} after ` +
        `${this.#report.instructions} instructions; state=${execution.state}; ` +
        `request=${pending?.requestId ?? "none"}; last=` +
        `${execution.lastTransition?.from ?? "none"}->${execution.lastTransition?.to ?? "none"}`,
    );
  }

  #terminate(outcome: "cancelled" | "watchdog", detail: string): void {
    if (this.#terminal) {
      return;
    }
    let pendingHostCalls = this.#pendingHostRequests.size();
    let pendingCallbacks = 0;
    let error: string | null = detail;
    this.#abortSourceReads();
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
    this.#pendingHostRequests.rejectAll(new SessionStopped(detail));
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
    let pendingHostCalls = this.#pendingHostRequests.size();
    let pendingCallbacks = 0;
    this.#abortSourceReads();
    if (this.#runtimePrepared && this.#bridge !== null) {
      try {
        const metadata = this.#bridge.cancel<CancellationMetadata>().metadata;
        pendingHostCalls = metadata.pendingHostCalls;
        pendingCallbacks = metadata.pendingCallbacks;
      } catch (cancelError) {
        error = new Error(`${error.message}; cleanup failed: ${String(cancelError)}`);
      }
    }
    this.#pendingHostRequests.rejectAll(new SessionStopped(error.message));
    this.#finish({
      outcome: "fault",
      exitCode: null,
      pendingHostCalls,
      pendingCallbacks,
      error: error.message,
    });
  }

  #abortSourceReads(): void {
    for (const controller of this.#sourceReadControllers.values()) {
      controller.abort();
    }
    this.#sourceReadControllers.clear();
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
    if (this.#watchdogTimer !== null) {
      worker.clearTimeout(this.#watchdogTimer);
      this.#watchdogTimer = null;
    }
    this.#execution.stop({
      cause: `session-${terminal.outcome}`,
      threadId: this.#currentThreadId === 0 ? null : this.#currentThreadId,
      requestId: this.#pendingHostRequests.first()?.requestId ?? null,
    });
    const execution = this.#execution.snapshot();
    invariant(
      this.#report.supportedCallCount === this.#semanticCallDigest.count(),
      "BGI semantic call digest count diverged from completed sequence identity",
    );
    this.#report.supportedCallSemanticsSha256 = this.#semanticCallDigest.digestHex();
    if (this.#configuration.evidenceLevel === "full") {
      invariant(
        this.#report.supportedCalls.length === this.#report.supportedCallCount,
        "full BGI call evidence omitted a completed sequence",
      );
      this.#report.supportedCalls.sort((left, right) => left.sequence - right.sequence);
    } else {
      invariant(
        this.#report.supportedCalls.length === 0,
        "summary BGI evidence retained detailed call payloads",
      );
    }
    this.#terminal = true;
    for (const resolve of this.#runnableResolvers.splice(0)) {
      resolve();
    }
    const message: SessionFinishedMessage = {
      type: "session-finished",
      protocolVersion,
      sessionId: this.#sessionId,
      report: {
        evidenceLevel: this.#configuration.evidenceLevel,
        ...terminal,
        elapsedMilliseconds: performance.now() - this.#startedAt,
        ...this.#report,
        executionState: execution.state,
        executionTransitionCount: execution.transitionCount,
        lastExecutionTransition: execution.lastTransition,
        pendingHostRequestRecords: this.#pendingHostRequests.size(),
        watchdogContext: this.#watchdogContext,
        wasmBridgeCalls: this.#bridge?.callCount() ?? 0,
        runtimeCoreLinearMemoryBytes: this.#bridge?.linearMemoryBytes() ?? 0,
        executionEngine: this.#engine?.identity() ?? null,
        executionEngineLinearMemoryBytes: this.#engine?.linearMemoryBytes() ?? 0,
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

  #requireEngine(): ExecutionEngine {
    invariant(this.#engine !== null, "execution engine is unavailable");
    return this.#engine;
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
            evidenceLevel: message.configuration.evidenceLevel,
            outcome: "fault",
            exitCode: null,
            elapsedMilliseconds: 0,
            instructions: 0,
            slices: 0,
            budgetYields: 0,
            hostRequests: 0,
            rustSchedulerTransitions: 0,
            registeredThreadIds: [],
            executedThreadIds: [],
            contextSwitches: 0,
            hostWaitExecutions: [],
            threadExits: [],
            executionState: "stopped",
            executionTransitionCount: 0,
            lastExecutionTransition: null,
            pendingHostRequestRecords: 0,
            watchdogContext: null,
            callbackEntries: 0,
            maximumCallbackDepth: 0,
            writebackBatches: 0,
            writebackPages: 0,
            stackGrowths: [],
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
            initialCurrentDirectory: null,
            finalGeneration: 0,
            pendingHostCalls: 0,
            pendingCallbacks: 0,
            maximumSliceMilliseconds: 0,
            wasmBridgeCalls: 0,
            runtimeCoreLinearMemoryBytes: 0,
            executionEngine: null,
            executionEngineLinearMemoryBytes: 0,
            supportedCallCount: 0,
            supportedCallSemanticsAlgorithm: SEMANTIC_CALL_DIGEST_ALGORITHM,
            supportedCallSemanticsSha256: EMPTY_SEMANTIC_CALL_SHA256,
            supportedCalls: [],
            sourceReads: [],
            writableCommits: [],
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
