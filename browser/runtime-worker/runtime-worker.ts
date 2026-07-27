/// <reference lib="webworker" />

import {
  assertProtocolVersion,
  type HostCompletionMessage,
  type HostRequestMessage,
  type MainToWorkerMessage,
  type PageLifecycleMessage,
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
  type RuntimeWritableDirectoryCreateSnapshot,
  type RuntimeGdiSnapshot,
  type RuntimeGdiplusSnapshot,
  type RuntimeCreateDirectoryACallSnapshot,
  type RuntimeGetFileAttributesACallSnapshot,
  type RuntimeSetWindowPosRepeatActivationCallSnapshot,
  type RuntimeSupportedCallSnapshot,
  type RuntimeImm32Snapshot,
  type RuntimeThreadSelectionSnapshot,
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
import {
  recoverableCommitHttpError,
  writableCommitFailureControl,
  writableDirectoryCreateFailureControl,
} from "./profile-storage.js";
import {
  TaskTimerPacingDriver,
  TimerPacingService,
} from "./timer-pacing.js";

const worker = self as DedicatedWorkerGlobalScope;
const EMPTY_SEMANTIC_CALL_SHA256 = new SemanticCallDigest().digestHex();
const BGI_SYNCHRONOUS_API_YIELD_INTERVAL = 64;
const yieldChannel = new MessageChannel();

function includesSetWindowPosRepeatActivation(
  profile: SessionConfiguration["capabilityProfile"],
): boolean {
  return (
    profile === "user32-set-window-pos-repeat-activation" ||
    profile === "d3d9-recreated-textures"
  );
}
const yieldResolvers: Array<() => void> = [];
yieldChannel.port1.addEventListener("message", () => {
  const resolve = yieldResolvers.shift();
  invariant(resolve !== undefined, "Worker yield channel received an unowned task");
  resolve();
});
yieldChannel.port1.start();
const timerPacingChannel = new MessageChannel();
const timerPacingDriver = new TaskTimerPacingDriver({
  now: () => performance.now(),
  postTask: () => timerPacingChannel.port2.postMessage(null),
  subscribe: (callback) =>
    timerPacingChannel.port1.addEventListener("message", callback),
});
timerPacingChannel.port1.start();

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
  writableDirectoryCreate?: RuntimeWritableDirectoryCreateSnapshot;
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
  writableDirectoryCreate?: RuntimeWritableDirectoryCreateSnapshot;
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
  schemaVersion: 3;
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
  }>;
  directories: Array<{
    directoryId: number;
    relativePathA: number[];
    exists: boolean;
    generation: number;
  }>;
}

interface WritableProfileLease {
  sessionKey: string;
  epoch: number;
  titleFingerprint: string;
  profileId: string;
}

interface WritableProfileHydratedMetadata {
  kind: "writable-profile-hydrated";
  objectId: number;
  generation: number;
  byteLength: number;
  sha256: string;
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
  writableDirectoryCreate?: RuntimeWritableDirectoryCreateSnapshot;
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

interface CurrentMemorySummaryMetadata {
  kind: "current-memory-summary";
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
  #stopping = false;
  #terminalTask: Promise<void> | null = null;
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
  readonly #deferredHostCompletions = new Map<string, () => void>();
  readonly #timerPacing: TimerPacingService;
  #hostBarrierRequestId: string | null = null;
  #hostBarrierResume:
    | {
        requestId: string;
        threadId: number;
        scheduler: ExecutableSchedulerMetadata;
      }
    | null = null;
  readonly #sourceReadControllers = new Map<string, AbortController>();
  readonly #profileTransportControllers = new Set<AbortController>();
  readonly #profileTransportTasks = new Set<Promise<unknown>>();
  readonly #writableMutationTasks = new Set<Promise<void>>();
  #sourceManifest: SourceManifest | null = null;
  #writableProfile: WritableProfileManifest | null = null;
  readonly #profileSessionKey = crypto.randomUUID();
  #profileLeaseAcquisitionTask: Promise<void> | null = null;
  #profileLease: WritableProfileLease | null = null;
  #nextBgiCallSequence = 0;
  readonly #bgiCallbackCallSequences = new Map<number, number>();
  #setWindowPosCompletionCount = 0;
  #setWindowPosRepeatActivationCallbackEntry: {
    action: "enter";
    sequence: number;
    frameId: number;
    instructionCount: number;
    target: number;
    allocationStart: number;
    stackPointer: number;
    scratchAddress: number;
    scratchBytes: number;
    frameBytes: number;
    returnSentinel: number;
    arguments: [number, number, number, number];
  } | null = null;
  readonly #semanticCallDigest = new SemanticCallDigest();
  #waitForSingleObjectExCompletions = 0;
  #enterCriticalSectionCompletions = 0;
  #leaveCriticalSectionCompletions = 0;
  #enterCriticalSectionAcquisition: {
    sequence: number;
    occurrence: number;
    instructionCount: number;
    criticalSectionAddress: number;
  } | null = null;
  readonly #enterCriticalSectionRoundtrips: Array<{
    criticalSectionAddress: number;
    acquisition: {
      implementationId: "kernel32.enter-critical-section.v1";
      sequence: number;
      occurrence: number;
      instructionCount: number;
    };
    release: {
      implementationId: "kernel32.leave-critical-section.v1";
      sequence: number;
      occurrence: number;
      instructionCount: number;
    };
  }> = [];
  readonly #mainLoopThreadSelections: RuntimeThreadSelectionSnapshot[] = [];
  #enterCriticalSectionContinuationRelease: {
    sequence: number;
    criticalSectionAddress: number;
  } | null = null;
  #leaveCriticalSectionContinuationAcquisition: {
    sequence: number;
    criticalSectionAddress: number;
  } | null = null;
  #bgiCheckpointPending = false;
  #synchronousApiBoundariesSinceYield = 0;
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
    writableDirectoryCreates: [] as SessionReport["writableDirectoryCreates"],
    dragAcceptFiles: null as SessionReport["dragAcceptFiles"],
    sleepZero: null as SessionReport["sleepZero"],
    globalMemoryStatus: null as SessionReport["globalMemoryStatus"],
    getFileAttributesA: null as SessionReport["getFileAttributesA"],
    createDirectoryA: null as SessionReport["createDirectoryA"],
    setWindowPosRepeatActivation:
      null as SessionReport["setWindowPosRepeatActivation"],
    d3d9RecreatedTextures: null as SessionReport["d3d9RecreatedTextures"],
    firstUnsupported: null as SessionReport["firstUnsupported"],
    checkpointCompletion: null as SessionReport["checkpointCompletion"],
  };

  constructor(sessionId: number, configuration: SessionConfiguration) {
    this.#sessionId = sessionId;
    this.#configuration = configuration;
    this.#timerPacing = new TimerPacingService(
      timerPacingDriver,
      configuration.pageLifecycle,
    );
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
      const sourceBackedWorkload =
        workload === "controlled-source-read" ||
        (workload === "private-bgi" &&
          (this.#configuration.capabilityProfile === "kernel32-archive-source-read" ||
            this.#configuration.capabilityProfile === "kernel32-sleep-zero" ||
            this.#configuration.capabilityProfile ===
              "kernel32-global-memory-status" ||
            this.#configuration.capabilityProfile ===
              "kernel32-get-file-attributes-a" ||
            this.#configuration.capabilityProfile ===
              "kernel32-create-directory-a" ||
            includesSetWindowPosRepeatActivation(
              this.#configuration.capabilityProfile,
            )));
      const profileBackedWorkload = workload === "private-bgi";
      invariant(
        this.#configuration.capabilityProfile === undefined || profileBackedWorkload,
        "capability profiles are only supported for the private BGI workload",
      );
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
      if (this.#terminal || this.#stopping) {
        return;
      }
      this.#bridge = bridge;
      const executable = new Uint8Array(await executableResponse.arrayBuffer());
      if (this.#terminal || this.#stopping) {
        return;
      }
      if (profileBackedWorkload) {
        const acquisition = this.#acquireWritableProfileLease();
        this.#profileLeaseAcquisitionTask = acquisition;
        try {
          await acquisition;
        } finally {
          if (this.#profileLeaseAcquisitionTask === acquisition) {
            this.#profileLeaseAcquisitionTask = null;
          }
        }
        this.#requireStorageSetupActive();
      }
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
          ? await this.#fetchWritableProfileSnapshot()
          : null;
      this.#sourceManifest =
        sourceManifestBytes === null ? null : this.#parseSourceManifest(sourceManifestBytes);
      this.#writableProfile =
        writableProfileBytes === null
          ? null
          : this.#parseWritableProfile(writableProfileBytes);
      const policy =
        workload === "private-bgi"
          ? await this.#fetchPolicy(
              new URL(
                this.#configuration.policyUrl ?? "./private/kernel32-policy.json",
                import.meta.url,
              ),
            )
          : workload === "controlled-source-read"
            ? await this.#fetchPolicy(
                new URL(
                  this.#configuration.policyUrl ?? "./kernel32-source-file-read-v1.json",
                  import.meta.url,
                ),
              )
            : null;
      if (this.#terminal || this.#stopping) {
        return;
      }
      const preparedOutput =
        workload === "private-bgi"
          ? bridge.prepareBgi(
              executable,
              policy ?? failInvariant("BGI runtime policy is missing"),
              sourceManifestBytes,
              writableProfileBytes ?? failInvariant("BGI writable profile is missing"),
            )
          : workload === "controlled-source-read"
            ? bridge.prepareSourceRead(
                executable,
                policy ?? failInvariant("source runtime policy is missing"),
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
      if (profileBackedWorkload) {
        this.#validatePreparedWritableProfile(preparedOutput.metadata);
        await this.#hydrateWritableProfileObjects();
      }
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
      if (error instanceof SessionStopped && (this.#terminal || this.#stopping)) {
        return;
      }
      this.#fail(error instanceof Error ? error : new Error(String(error)));
    } finally {
      if (this.#terminalTask !== null) {
        await this.#terminalTask;
      }
    }
  }

  async #fetchPolicy(url: URL): Promise<Uint8Array> {
    return this.#fetchBytes(url, "runtime policy");
  }

  async #acquireWritableProfileLease(): Promise<void> {
    invariant(this.#profileLease === null, "writable profile lease is already active");
    const url = this.#sameOriginStorageUrl(
      this.#configuration.writableProfileApiUrl,
      "/api/profile/v3/",
      "writable profile API URL",
    );
    url.pathname = `${url.pathname.replace(/\/?$/u, "/")}sessions`;
    const response = await fetch(url, {
      method: "POST",
      cache: "no-store",
      headers: { "X-Vine-Session-Key": this.#profileSessionKey },
    });
    invariant(response.status === 201, `writable profile lease failed with ${response.status}`);
    const epochText = response.headers.get("x-vine-session-epoch");
    const epoch = epochText === null ? Number.NaN : Number(epochText);
    const titleFingerprint = response.headers.get("x-vine-title-fingerprint");
    const profileId = response.headers.get("x-vine-profile-id");
    invariant(
      Number.isSafeInteger(epoch) &&
        epoch > 0 &&
        response.headers.get("x-vine-session-key") === this.#profileSessionKey &&
        titleFingerprint !== null &&
        /^[a-zA-Z0-9:._-]{1,128}$/u.test(titleFingerprint) &&
        profileId !== null &&
        /^[a-zA-Z0-9._-]{1,128}$/u.test(profileId),
      "writable profile lease response identity is invalid",
    );
    this.#profileLease = {
      sessionKey: this.#profileSessionKey,
      epoch,
      titleFingerprint,
      profileId,
    };
  }

  async #fetchWritableProfileSnapshot(): Promise<Uint8Array> {
    const lease = this.#requireProfileLease();
    const url = this.#sameOriginStorageUrl(
      this.#configuration.writableProfileUrl,
      "/api/profile/v3/snapshot",
      "writable profile URL",
    );
    return this.#runProfileTransport(async (signal) => {
      const response = await fetch(url, {
        cache: "no-store",
        headers: this.#profileLeaseHeaders(),
        signal,
      });
      invariant(response.status === 200, `writable profile fetch failed with ${response.status}`);
      invariant(
        response.headers.get("x-vine-title-fingerprint") === lease.titleFingerprint &&
          response.headers.get("x-vine-profile-id") === lease.profileId &&
          response.headers.get("x-vine-session-epoch") === String(lease.epoch),
        "writable profile snapshot lease identity mismatch",
      );
      const bytes = new Uint8Array(await response.arrayBuffer());
      this.#requireStorageSetupActive();
      return bytes;
    });
  }

  async #hydrateWritableProfileObjects(): Promise<void> {
    const profile =
      this.#writableProfile ?? failInvariant("writable profile hydration has no manifest");
    const bridge = this.#requireBridge();
    for (const file of profile.files) {
      this.#requireStorageSetupActive();
      if (!file.exists) {
        continue;
      }
      const bytes = new Uint8Array(file.byteLength);
      let offset = 0;
      while (offset < file.byteLength) {
        const requestedBytes = Math.min(1024 * 1024, file.byteLength - offset);
        const url = this.#profileObjectUrl(file.objectId);
        url.searchParams.set("generation", String(file.generation));
        url.searchParams.set("offset", String(offset));
        url.searchParams.set("length", String(requestedBytes));
        const chunk = await this.#runProfileTransport(async (signal) => {
          const response = await fetch(url, {
            method: "GET",
            cache: "no-store",
            headers: this.#profileLeaseHeaders(),
            signal,
          });
          invariant(
            response.status === 206,
            `writable profile hydration failed with ${response.status}`,
          );
          invariant(
            response.headers.get("x-vine-title-fingerprint") === profile.titleFingerprint &&
              response.headers.get("x-vine-profile-id") === profile.profileId &&
              response.headers.get("x-vine-object-id") === String(file.objectId) &&
              response.headers.get("x-vine-generation") === String(file.generation) &&
              response.headers.get("x-vine-offset") === String(offset) &&
              response.headers.get("x-vine-requested-bytes") === String(requestedBytes) &&
              response.headers.get("x-vine-transferred-bytes") === String(requestedBytes) &&
              response.headers.get("x-vine-session-epoch") ===
                String(this.#requireProfileLease().epoch),
            "writable profile hydration response identity mismatch",
          );
          const payload = new Uint8Array(await response.arrayBuffer());
          this.#requireStorageSetupActive();
          return payload;
        });
        invariant(
          chunk.byteLength === requestedBytes,
          "writable profile hydration response length mismatch",
        );
        bytes.set(chunk, offset);
        offset += chunk.byteLength;
      }
      this.#requireStorageSetupActive();
      const digest = await sha256Hex(bytes);
      this.#requireStorageSetupActive();
      invariant(digest === file.sha256, "writable profile hydration hash mismatch");
      const output = bridge.hydrateWritableProfileObject<WritableProfileHydratedMetadata>(
        file.objectId,
        BigInt(file.generation),
        bytes,
      );
      invariant(
        output.metadata.kind === "writable-profile-hydrated" &&
          output.metadata.objectId === file.objectId &&
          output.metadata.generation === file.generation &&
          output.metadata.byteLength === file.byteLength &&
          output.metadata.sha256 === file.sha256 &&
          output.control.byteLength === 0 &&
          output.payload.byteLength === 0,
        "Rust writable profile hydration metadata mismatch",
      );
    }
  }

  #profileObjectUrl(objectId: number): URL {
    const profile =
      this.#writableProfile ?? failInvariant("writable profile object URL has no manifest");
    const base = this.#sameOriginStorageUrl(
      this.#configuration.writableProfileApiUrl,
      "/api/profile/v3/",
      "writable profile API URL",
    );
    if (!base.pathname.endsWith("/")) {
      base.pathname += "/";
    }
    return new URL(
      `titles/${encodeURIComponent(profile.titleFingerprint)}/` +
        `profiles/${encodeURIComponent(profile.profileId)}/objects/${objectId}`,
      base,
    );
  }

  #profileDirectoryCommitUrl(directoryId: number): URL {
    const profile =
      this.#writableProfile ?? failInvariant("writable profile directory URL has no manifest");
    const base = this.#sameOriginStorageUrl(
      this.#configuration.writableProfileApiUrl,
      "/api/profile/v3/",
      "writable profile API URL",
    );
    if (!base.pathname.endsWith("/")) {
      base.pathname += "/";
    }
    return new URL(
      `titles/${encodeURIComponent(profile.titleFingerprint)}/` +
        `profiles/${encodeURIComponent(profile.profileId)}/` +
        `directories/${directoryId}/commits`,
      base,
    );
  }

  #profileLeaseHeaders(): Record<string, string> {
    const lease = this.#requireProfileLease();
    return {
      "X-Vine-Session-Key": lease.sessionKey,
      "X-Vine-Session-Epoch": String(lease.epoch),
    };
  }

  #requireProfileLease(): WritableProfileLease {
    return this.#profileLease ?? failInvariant("writable profile session lease is unavailable");
  }

  #requireStorageSetupActive(): void {
    if (this.#terminal || this.#stopping) {
      throw new SessionStopped("writable profile setup stopped with the runtime session");
    }
  }

  #runProfileTransport<T>(operation: (signal: AbortSignal) => Promise<T>): Promise<T> {
    const controller = new AbortController();
    this.#profileTransportControllers.add(controller);
    const task = (async () => {
      try {
        return await operation(controller.signal);
      } finally {
        this.#profileTransportControllers.delete(controller);
      }
    })();
    this.#profileTransportTasks.add(task);
    void task.then(
      () => this.#profileTransportTasks.delete(task),
      () => this.#profileTransportTasks.delete(task),
    );
    return task;
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
      profile.schema === "vine.writable-profile-snapshot" && profile.schemaVersion === 3,
      "writable profile schema mismatch",
    );
    invariant(
      /^[a-zA-Z0-9:._-]{1,128}$/u.test(profile.titleFingerprint) &&
        /^[a-zA-Z0-9._-]{1,128}$/u.test(profile.profileId) &&
        profile.titleFingerprint === this.#requireProfileLease().titleFingerprint &&
        profile.profileId === this.#requireProfileLease().profileId &&
        validByteArray(profile.rootPathA, true) &&
        Number.isSafeInteger(profile.quotaBytes) &&
        profile.quotaBytes > 0 &&
        profile.quotaBytes <= 16 * 1024 * 1024,
      "writable profile identity or quota is invalid",
    );
    invariant(
      Array.isArray(profile.files) && profile.files.length > 0 && profile.files.length <= 64,
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
          (file.sha256 === null || /^[0-9a-f]{64}$/u.test(file.sha256)) &&
          !Object.hasOwn(file, "bytes") &&
          (file.exists
            ? file.generation > 0 && file.sha256 !== null
            : file.generation === 0 && file.byteLength === 0 && file.sha256 === null),
        "writable profile file descriptor is invalid or duplicated",
      );
      objectIds.add(file.objectId);
      paths.add(pathIdentity);
      totalBytes += file.byteLength;
    }
    invariant(
      Array.isArray(profile.directories) && profile.directories.length <= 64,
      "writable profile directory count is invalid",
    );
    const directoryIds = new Set<number>();
    for (const directory of profile.directories) {
      const pathIdentity = directory.relativePathA
        .map((byte) => (byte >= 0x41 && byte <= 0x5a ? byte + 0x20 : byte))
        .join(",");
      invariant(
        Number.isInteger(directory.directoryId) &&
          directory.directoryId > 0 &&
          !directoryIds.has(directory.directoryId) &&
          validByteArray(directory.relativePathA, false) &&
          !paths.has(pathIdentity) &&
          typeof directory.exists === "boolean" &&
          Number.isSafeInteger(directory.generation) &&
          (directory.exists ? directory.generation > 0 : directory.generation === 0),
        "writable profile directory descriptor is invalid or duplicated",
      );
      directoryIds.add(directory.directoryId);
      paths.add(pathIdentity);
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
    invariant(
      Object.keys(runtime.directories).length === profile.directories.length,
      "Rust and browser writable profile directory counts differ",
    );
    for (const directory of profile.directories) {
      const preparedDirectory = runtime.directories[String(directory.directoryId)];
      invariant(
        preparedDirectory !== undefined &&
          preparedDirectory.directoryId === directory.directoryId &&
          byteArraysEqual(preparedDirectory.relativePathA, directory.relativePathA) &&
          preparedDirectory.exists === directory.exists &&
          preparedDirectory.generation === directory.generation,
        `Rust and browser writable directory ${directory.directoryId} differ`,
      );
    }
  }

  completeHost(message: HostCompletionMessage): void {
    if (this.#stopping) {
      return;
    }
    if (this.#terminal) {
      throw new Error("host completion arrived after the runtime session stopped");
    }
    try {
      this.#deliverInIssueOrder(message.requestId, () =>
        this.#deliverHostCompletion(message),
      );
    } catch (error) {
      this.#fail(error instanceof Error ? error : new Error(String(error)));
    }
  }

  // Host completions must reach Rust in request-issue order: the Node oracle
  // services every host request synchronously, so a younger completion (for
  // example a timer-pacing deadline) may not overtake an older in-flight
  // storage read without diverging from the frozen scheduling stream.
  #deliverInIssueOrder(requestId: string, deliver: () => void): void {
    const head = this.#pendingHostRequests.first();
    if (head?.requestId === requestId) {
      deliver();
      this.#drainDeferredHostCompletions();
      return;
    }
    invariant(
      this.#pendingHostRequests.pending(requestId) !== null,
      `host completion ${requestId} does not belong to a pending request`,
    );
    invariant(
      !this.#deferredHostCompletions.has(requestId),
      `host completion ${requestId} is already deferred`,
    );
    this.#deferredHostCompletions.set(requestId, deliver);
  }

  #drainDeferredHostCompletions(): void {
    while (!this.#terminal && !this.#stopping) {
      const head = this.#pendingHostRequests.first();
      if (head === null) {
        return;
      }
      const deliver = this.#deferredHostCompletions.get(head.requestId);
      if (deliver === undefined) {
        return;
      }
      this.#deferredHostCompletions.delete(head.requestId);
      deliver();
    }
  }

  #deliverHostCompletion(message: HostCompletionMessage): void {
    {
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
      if (
        pending.service === "runtime.timer-delay.v1" &&
        this.#requirePrepared().workload === "private-bgi"
      ) {
        this.#recordBgiCompletion(completion);
      }
      this.#applyCpuResumeToThread(completion.threadId, completion.completion);
      this.#threadExecutionStates.set(completion.threadId, resumeState);
      if (this.#hostBarrierRequestId === pending.requestId) {
        invariant(
          this.#hostBarrierResume === null,
          "host completion barrier retained an unconsumed owner resume",
        );
        this.#hostBarrierResume = {
          requestId: pending.requestId,
          threadId: completion.threadId,
          scheduler: completion.scheduler,
        };
      }
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
    }
  }

  updatePageLifecycle(message: PageLifecycleMessage): void {
    if (message.sessionId !== this.#sessionId || this.#terminal || this.#stopping) {
      return;
    }
    try {
      this.#timerPacing.updateLifecycle(message.lifecycle);
    } catch (error) {
      this.#fail(error instanceof Error ? error : new Error(String(error)));
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
      if (this.#stopping) {
        throw new SessionStopped("source read stopped with the runtime session");
      }
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
      this.#deliverInIssueOrder(metadata.requestId, () =>
        this.#completeSourceRead(metadata, control, payload),
      );
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
    const url = this.#profileObjectUrl(objectId ?? 0);
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
        headers: this.#profileLeaseHeaders(),
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
          response.headers.get("x-vine-transferred-bytes") === String(expectedBytes) &&
          response.headers.get("x-vine-session-epoch") ===
            String(this.#requireProfileLease().epoch),
        "profile read response identity mismatch",
      );
      const payload = new Uint8Array(await response.arrayBuffer());
      invariant(payload.byteLength === expectedBytes, "profile read response length mismatch");
      if (this.#stopping) {
        throw new SessionStopped("profile read stopped with the runtime session");
      }
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
      this.#deliverInIssueOrder(metadata.requestId, () =>
        this.#completeSourceRead(metadata, control, payload),
      );
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
    const url = this.#profileObjectUrl(objectId ?? 0);
    url.pathname += "/commits";
    const requestKey = `${this.#profileSessionKey}:${metadata.requestId}`;
    let response: Response | null = null;
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
            ...this.#profileLeaseHeaders(),
          },
          body: ownedArrayBuffer(payloadOutput.payload),
        });
        if (candidate.status >= 500 && attempt === 0) {
          continue;
        }
        response = candidate;
      } catch {}
    }
    if (response === null) {
      throw new Error("writable commit transport ended with an unknown publication outcome");
    }
    if (response.status !== 200) {
      const win32Error = recoverableCommitHttpError(
        response.status,
        response.headers.get("x-vine-error-code"),
      );
      invariant(win32Error !== null, `writable commit failed with ${response.status}`);
      this.#completeWritableCommit(
        metadata,
        writableCommitFailureControl(objectId ?? 0, baseGeneration, win32Error),
      );
      return;
    }
    invariant(
      response.headers.get("x-vine-title-fingerprint") === profile.titleFingerprint &&
        response.headers.get("x-vine-profile-id") === profile.profileId &&
        response.headers.get("x-vine-object-id") === String(objectId) &&
        response.headers.get("x-vine-base-generation") === baseGeneration.toString() &&
        response.headers.get("x-vine-content-sha256") === contentSha256 &&
        response.headers.get("x-vine-session-epoch") ===
          String(this.#requireProfileLease().epoch),
      "writable commit response identity mismatch",
    );
    const control = new Uint8Array(await response.arrayBuffer());
    invariant(control.byteLength === 32, "writable commit response size mismatch");
    this.#deliverInIssueOrder(metadata.requestId, () => {
      const completion = this.#completeWritableCommit(metadata, control);
      if (completion.succeeded) {
        file.exists = true;
        file.generation = completion.committedGeneration;
        file.byteLength = payloadOutput.payload.byteLength;
        file.sha256 = contentSha256;
      }
    });
  }

  async #serviceWritableDirectoryCreate(metadata: HostRequestMetadata): Promise<void> {
    const profile =
      this.#writableProfile ?? failInvariant("directory create has no registered profile");
    invariant(
      metadata.service === "storage.profile-directory-create.v1",
      "unexpected writable directory create service",
    );
    invariant(
      metadata.arguments.length === 4,
      "writable directory create request must contain four words",
    );
    const [version, directoryId, baseLow, baseHigh] = metadata.arguments;
    invariant(version === 1, "writable directory create request version mismatch");
    const baseGeneration = BigInt(baseLow ?? 0) | (BigInt(baseHigh ?? 0) << 32n);
    invariant(
      baseGeneration <= BigInt(Number.MAX_SAFE_INTEGER),
      "writable directory create generation exceeds the profile bound",
    );
    const directory = profile.directories.find(
      (candidate) => candidate.directoryId === directoryId,
    );
    invariant(directory !== undefined, "writable directory create identity is not allowlisted");
    invariant(
      !directory.exists && BigInt(directory.generation) === baseGeneration,
      "writable directory create base state differs from the browser snapshot",
    );
    const url = this.#profileDirectoryCommitUrl(directoryId ?? 0);
    const requestKey = `${this.#profileSessionKey}:${metadata.requestId}`;
    let response: Response | null = null;
    for (let attempt = 0; attempt < 2 && response === null; attempt += 1) {
      try {
        const candidate = await fetch(url, {
          method: "POST",
          cache: "no-store",
          headers: {
            "X-Vine-Request-Key": requestKey,
            "X-Vine-Base-Generation": baseGeneration.toString(),
            ...this.#profileLeaseHeaders(),
          },
        });
        if (candidate.status >= 500 && attempt === 0) {
          continue;
        }
        response = candidate;
      } catch {}
    }
    if (response === null) {
      throw new Error(
        "writable directory create transport ended with an unknown publication outcome",
      );
    }
    if (response.status !== 200) {
      const win32Error = recoverableCommitHttpError(
        response.status,
        response.headers.get("x-vine-error-code"),
      );
      invariant(win32Error !== null, `writable directory create failed with ${response.status}`);
      this.#completeWritableDirectoryCreate(
        metadata,
        writableDirectoryCreateFailureControl(
          directoryId ?? 0,
          baseGeneration,
          win32Error,
        ),
      );
      return;
    }
    invariant(
      response.headers.get("x-vine-title-fingerprint") === profile.titleFingerprint &&
        response.headers.get("x-vine-profile-id") === profile.profileId &&
        response.headers.get("x-vine-directory-id") === String(directoryId) &&
        response.headers.get("x-vine-base-generation") === baseGeneration.toString() &&
        response.headers.get("x-vine-session-epoch") ===
          String(this.#requireProfileLease().epoch),
      "writable directory create response identity mismatch",
    );
    const control = new Uint8Array(await response.arrayBuffer());
    invariant(control.byteLength === 32, "writable directory create response size mismatch");
    this.#deliverInIssueOrder(metadata.requestId, () => {
      const completion = this.#completeWritableDirectoryCreate(metadata, control);
      if (completion.succeeded) {
        directory.exists = true;
        directory.generation = completion.committedGeneration;
      }
    });
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
    if (this.#hostBarrierRequestId === pending.requestId) {
      invariant(
        this.#hostBarrierResume === null,
        "host completion barrier retained an unconsumed owner resume",
      );
      this.#hostBarrierResume = {
        requestId: pending.requestId,
        threadId: completion.threadId,
        scheduler: completion.scheduler,
      };
    }
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
    if (this.#hostBarrierRequestId === pending.requestId) {
      invariant(
        this.#hostBarrierResume === null,
        "host completion barrier retained an unconsumed owner resume",
      );
      this.#hostBarrierResume = {
        requestId: pending.requestId,
        threadId: completion.threadId,
        scheduler: completion.scheduler,
      };
    }
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

  #completeWritableDirectoryCreate(
    request: HostRequestMetadata,
    control: Uint8Array,
  ): RuntimeWritableDirectoryCreateSnapshot {
    if (this.#terminal) {
      throw new SessionStopped("writable directory create completed after the session stopped");
    }
    const pending = this.#pendingHostRequests.complete(request.requestId, request.threadId, 1);
    const output = this.#requireBridge().completeWritableDirectoryCreate<HostCompletionMetadata>(
      BigInt(pending.requestId),
      control,
    );
    const completion = output.metadata;
    invariant(
      completion.kind === "host-complete" &&
        completion.threadId === pending.threadId &&
        completion.writableDirectoryCreate !== undefined &&
        output.control.byteLength === 0 &&
        output.payload.byteLength === 0,
      "invalid writable directory create completion metadata",
    );
    const resumeState = completion.scheduler.state === "in-callback" ? "guest-callback" : "ready";
    const expectedResumeState =
      pending.completionPolicy === "resume-callback" ? "guest-callback" : "ready";
    invariant(
      resumeState === expectedResumeState,
      "writable directory create resume policy mismatch",
    );
    if (this.#requirePrepared().workload === "private-bgi") {
      this.#recordBgiCompletion(completion);
    }
    this.#report.writableDirectoryCreates.push(completion.writableDirectoryCreate);
    this.#applyCpuResumeToThread(completion.threadId, completion.completion);
    this.#threadExecutionStates.set(completion.threadId, resumeState);
    if (this.#hostBarrierRequestId === pending.requestId) {
      invariant(
        this.#hostBarrierResume === null,
        "host completion barrier retained an unconsumed owner resume",
      );
      this.#hostBarrierResume = {
        requestId: pending.requestId,
        threadId: completion.threadId,
        scheduler: completion.scheduler,
      };
    }
    this.#report.rustSchedulerTransitions += 1;
    pending.resolve({
      result: completion.writableDirectoryCreate.succeeded ? 1 : 0,
      completionPolicy: pending.completionPolicy,
    });
    if (this.#execution.state() === "waiting-host") {
      this.#execution.transition("host-completion", {
        cause: "writable-directory-create-completion",
        threadId: pending.threadId,
        requestId: pending.requestId,
      });
      this.#signalRunnable();
    }
    this.#status("running", `${pending.service} completed`);
    return completion.writableDirectoryCreate;
  }

  cancel(): void {
    this.#terminate("cancelled", "Session cancelled by user");
  }

  async #runGuest(): Promise<void> {
    const engine = this.#requireEngine();
    const prepared = this.#requirePrepared();
    const endpointAddresses = new Set(prepared.endpoints.map(({ address }) => address));
    while (!this.#terminal && !this.#stopping) {
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
        this.#synchronousApiBoundariesSinceYield = 0;
        if (this.#terminal || this.#stopping) {
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
      if (this.#shouldYieldAfterSynchronousApiBoundary()) {
        await yieldToWorkerLoop();
      }
    }
  }

  #shouldYieldAfterSynchronousApiBoundary(): boolean {
    if (
      this.#configuration.workload !== "private-bgi" ||
      this.#configuration.mode !== "success"
    ) {
      return true;
    }
    this.#synchronousApiBoundariesSinceYield += 1;
    if (
      this.#synchronousApiBoundariesSinceYield <
      BGI_SYNCHRONOUS_API_YIELD_INTERVAL
    ) {
      return false;
    }
    this.#synchronousApiBoundariesSinceYield = 0;
    return true;
  }

  async #dispatchApi(endpoint: number, stackPointer: number): Promise<void> {
    if (this.#bgiCheckpointPending) {
      this.#completeBgiCapabilityCheckpoint(endpoint, stackPointer);
      return;
    }
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
        await this.#scheduleAfterBlock(
          metadata.threadId,
          metadata.requestId,
          metadata.service === "runtime.timer-delay.v1" ||
            metadata.service === "storage.source-read.v1" ||
            metadata.service === "storage.profile-read.v1" ||
            metadata.service === "storage.profile-commit.v1" ||
            metadata.service === "storage.profile-directory-create.v1",
        );
        this.#synchronousApiBoundariesSinceYield = 0;
        break;
      }
      case "guest-callback": {
        this.#takeApiResumeState();
        this.#applyRequiredDelta(output);
        if (this.#requirePrepared().workload === "private-bgi") {
          const sequence = this.#nextBgiCallSequence + 1;
          invariant(
            !this.#bgiCallbackCallSequences.has(metadata.frameId),
            "BGI callback frame reused an active call sequence",
          );
          if (
            includesSetWindowPosRepeatActivation(
              this.#configuration.capabilityProfile,
            ) && sequence === 108_718
          ) {
            invariant(
              this.#setWindowPosRepeatActivationCallbackEntry === null,
              "SetWindowPos repeat activation entered more than one callback",
            );
            this.#setWindowPosRepeatActivationCallbackEntry = {
              action: "enter",
              sequence,
              frameId: metadata.frameId,
              instructionCount: this.#report.instructions,
              target: metadata.target,
              allocationStart: metadata.allocationStart,
              stackPointer: metadata.stackPointer,
              scratchAddress: metadata.scratchAddress,
              scratchBytes: metadata.scratchBytes,
              frameBytes: metadata.frameBytes,
              returnSentinel: metadata.returnSentinel,
              arguments: [
                engine.readGuestU32(metadata.stackPointer + 4),
                engine.readGuestU32(metadata.stackPointer + 8),
                engine.readGuestU32(metadata.stackPointer + 12),
                engine.readGuestU32(metadata.stackPointer + 16),
              ],
            };
          }
          this.#nextBgiCallSequence = sequence;
          this.#bgiCallbackCallSequences.set(metadata.frameId, sequence);
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
        this.#completeApiExecutionBoundary(
          resumeState,
          metadata.threadSelection,
          metadata.completion.executionBoundary,
        );
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
    const call: RuntimeSupportedCallSnapshot = {
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
    this.#recordD3d9RecreatedTextureCall(call);
    this.#semanticCallDigest.update(call);
    this.#report.supportedCallCount += 1;
    if (this.#configuration.evidenceLevel === "full") {
      this.#report.supportedCalls.push(call);
    }
  }

  #completeBgiCapabilityCheckpoint(endpointAddress: number, stackPointer: number): void {
    const checkpoint = this.#report.checkpointCompletion;
    invariant(
      checkpoint !== null &&
        this.#configuration.capabilityProfile === checkpoint.capability &&
        this.#requirePrepared().runtime.kernel32Policy ===
          "kernel32-wait-for-single-object-ex-v1",
      "BGI capability checkpoint has no matching compatibility profile",
    );
    const endpoint = this.#requirePrepared().endpoints.find(
      (candidate) => candidate.address === endpointAddress,
    );
    invariant(endpoint !== undefined, "BGI capability checkpoint endpoint is not owned");
    const returnAddress = this.#requireEngine().readGuestU32(stackPointer);
    invariant(
      returnAddress > 0 && returnAddress <= 0xffff_ffff,
      "BGI capability checkpoint return address is invalid",
    );
    const reflection =
      this.#requireBridge().currentMemorySummary<CurrentMemorySummaryMetadata>().metadata;
    invariant(
      reflection.kind === "current-memory-summary",
      "invalid checkpoint memory summary",
    );
    invariant(
      reflection.generation === Number(this.#generation) &&
        reflection.acknowledgedGeneration === Number(this.#generation),
      "checkpoint reflection generation mismatch",
    );
    invariant(
      /^[0-9a-f]{64}$/u.test(reflection.contentSha256),
      "checkpoint reflection digest is invalid",
    );
    checkpoint.instructionCount = this.#report.instructions;
    checkpoint.nextEndpoint = endpoint;
    checkpoint.memoryContentSha256 = reflection.contentSha256;
    this.#cancelAndFinish(
      "checkpoint-complete",
      checkpoint.capability === "kernel32-leave-critical-section"
        ? "LeaveCriticalSection main-thread release checkpoint completed"
        : checkpoint.capability ===
            "kernel32-leave-critical-section-continuation"
          ? "LeaveCriticalSection continuation checkpoint completed"
        : checkpoint.capability === "kernel32-enter-critical-section-continuation"
          ? "EnterCriticalSection continuation checkpoint completed"
        : checkpoint.capability === "primary-main-loop-progression"
          ? "API-boundary scheduler roundtrip checkpoint completed"
          : checkpoint.capability === "kernel32-enter-critical-section"
            ? "EnterCriticalSection acquisition/release cycle checkpoint completed"
            : "WaitForSingleObjectEx recurring-wait checkpoint completed",
      null,
    );
  }

  #recordBgiCompletion(
    metadata: CompletionMetadata | CallbackCompletionMetadata | HostCompletionMetadata,
    reservedSequence?: number,
    callbackResult?: number,
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
    const isWritableDirectoryCreate =
      completion.implementationId === "kernel32.create-directory-a.runtime-vfs.v1";
    invariant(
      isWritableDirectoryCreate === (metadata.writableDirectoryCreate !== undefined),
      "BGI writable directory completion metadata mismatch",
    );
    const isWaitableTimerMutation =
      completion.implementationId === "kernel32.create-waitable-timer-a.v1" ||
      completion.implementationId === "kernel32.set-waitable-timer.relative-one-shot.v1" ||
      completion.implementationId ===
        "kernel32.wait-for-single-object-ex.waitable-timer.v1";
    const isWaitForSingleObjectEx =
      completion.implementationId ===
      "kernel32.wait-for-single-object-ex.waitable-timer.v1";
    const waitableTimerMutationSucceeded =
      isWaitableTimerMutation &&
      completion.eax.action === "set" &&
      (isWaitForSingleObjectEx
        ? completion.eax.value !== 0xffff_ffff
        : completion.eax.value !== 0);
    invariant(
      waitableTimerMutationSucceeded === (metadata.waitableTimer !== undefined),
      "BGI waitable timer completion metadata mismatch",
    );
    if (metadata.waitableTimer !== undefined) {
      invariant(
        isWaitForSingleObjectEx ===
          (metadata.waitableTimer.timerClock !== undefined &&
            metadata.waitableTimer.nextWaitRegistrationId !== undefined),
        "BGI waitable timer clock metadata mismatch",
      );
    }
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
      completion.implementationId === "user32.set-window-pos.v3" ||
      completion.implementationId === "user32.set-window-pos.v4" ||
      completion.implementationId === "user32.set-window-long-a.gwl-style.v1" ||
      completion.implementationId ===
        "shell32.drag-accept-files.logical-window-style-v1";
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
    const call: RuntimeSupportedCallSnapshot = {
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
      ...(completion.executionBoundary === undefined
        ? {}
        : { executionBoundary: completion.executionBoundary }),
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
      ...(metadata.writableDirectoryCreate === undefined
        ? {}
        : { writableDirectoryCreate: metadata.writableDirectoryCreate }),
      ...(metadata.gdi === undefined ? {} : { gdi: metadata.gdi }),
      ...(metadata.gdiplus === undefined ? {} : { gdiplus: metadata.gdiplus }),
      ...(metadata.imm32 === undefined ? {} : { imm32: metadata.imm32 }),
      ...(metadata.virtualMemory === undefined
        ? {}
        : { virtualMemory: metadata.virtualMemory }),
    };
    this.#recordD3d9RecreatedTextureCall(call);
    if (
      completion.implementationId === "user32.set-window-pos.v4" &&
      includesSetWindowPosRepeatActivation(
        this.#configuration.capabilityProfile,
      )
    ) {
      this.#setWindowPosCompletionCount += 1;
      if (this.#setWindowPosCompletionCount === 5) {
        const entry = this.#setWindowPosRepeatActivationCallbackEntry;
        const window = metadata.window;
        const [
          windowHandle,
          insertAfter,
          x,
          y,
          width,
          height,
          flags,
        ] = call.arguments;
        invariant(
          metadata.kind === "callback-complete" &&
            metadata.remainingDepth === 0 &&
            reservedSequence === 108_718 &&
            callbackResult === 0 &&
            entry !== null &&
            entry.sequence === reservedSequence &&
            entry.frameId === metadata.frameId &&
            entry.instructionCount === 28_273_977 &&
            entry.target === 0x1009_2550 &&
            entry.allocationStart === 0x006f_f5cc &&
            entry.stackPointer === 0x006f_f5cc &&
            entry.scratchAddress === 0x0050_1000 &&
            entry.scratchBytes === 28 &&
            entry.frameBytes === 20 &&
            entry.returnSentinel === 0x7f01_0000 &&
            entry.arguments.length === 4 &&
            entry.arguments[0] === 0x4801_0001 &&
            entry.arguments[1] === 0x0046 &&
            entry.arguments[2] === 0 &&
            entry.arguments[3] === 0x0050_1000 &&
            this.#report.instructions === 28_274_079 &&
            call.arguments.length === 7 &&
            windowHandle === 0x4801_0001 &&
            insertAfter === 0xffff_fffe &&
            x === 0 &&
            y === 0 &&
            width === 1_286 &&
            height === 749 &&
            flags === 266 &&
            completion.eax.action === "set" &&
            completion.eax.value === 1 &&
            completion.instructionPointer === 0x1005_cb4b &&
            frame.stackPointer === 0x006f_f5e0 &&
            completion.stackPointer === 0x006f_f600 &&
            completion.lastError === 0 &&
            guestWrites.length === 0 &&
            window !== undefined &&
            window.handle === windowHandle &&
            window.geometry.x === 15_360 &&
            window.geometry.y === 8_640 &&
            window.geometry.width === 1_286 &&
            window.geometry.height === 749 &&
            window.geometry.clientX === 15_363 &&
            window.geometry.clientY === 8_666 &&
            window.geometry.clientWidth === 1_280 &&
            window.geometry.clientHeight === 720,
          "SetWindowPos repeat activation ABI, callback, or authority state changed",
        );
        const repeatCall: RuntimeSetWindowPosRepeatActivationCallSnapshot = {
          sequence: reservedSequence,
          module: endpoint.module,
          symbol: "SetWindowPos",
          occurrence: 5,
          trapInstructionCount: entry.instructionCount,
          arguments: [
            windowHandle,
            insertAfter,
            x,
            y,
            width,
            height,
            flags,
          ],
          result: { action: "set", value: 1 },
          implementationId: "user32.set-window-pos.v4",
          returnAddress: completion.instructionPointer,
          stackPointerBefore: frame.stackPointer,
          stackPointerAfter: completion.stackPointer,
          lastError: completion.lastError,
          guestWrites: [],
          window: structuredClone(window),
          callback: {
            frameId: entry.frameId,
            target: entry.target,
            allocationStart: entry.allocationStart,
            stackPointer: entry.stackPointer,
            scratchAddress: entry.scratchAddress,
            scratchBytes: entry.scratchBytes,
            frameBytes: entry.frameBytes,
            returnSentinel: entry.returnSentinel,
            returnInstructionCount: this.#report.instructions,
            result: callbackResult,
          },
        };
        this.#report.setWindowPosRepeatActivation = {
          callCount: 5,
          repeatCall,
          callbackEvents: [
            {
              action: "enter",
              sequence: entry.sequence,
              frameId: entry.frameId,
              instructionCount: entry.instructionCount,
              target: entry.target,
              stackPointer: entry.stackPointer,
              arguments: entry.arguments,
            },
            {
              action: "return",
              sequence: entry.sequence,
              frameId: entry.frameId,
              instructionCount: this.#report.instructions,
              result: callbackResult,
            },
          ],
          windowAuthority: "rust-runtime-user32-logical-window",
          relationAuthority:
            "rust-runtime-user32-thread-and-process-window-relations",
          callbackDisposition: "validate-only",
          mutation: false,
          browserResourceCreated: false,
        };
      }
    }
    if (
      completion.implementationId ===
      "shell32.drag-accept-files.logical-window-style-v1"
    ) {
      invariant(
        this.#report.dragAcceptFiles === null,
        "DragAcceptFiles completed more than once",
      );
      invariant(
        endpoint.module.toLowerCase() === "shell32.dll" &&
          endpoint.symbol.kind === "name" &&
          endpoint.symbol.name === "DragAcceptFiles",
        "DragAcceptFiles endpoint identity changed",
      );
      const [windowHandle, rawAccept] = call.arguments;
      invariant(
        call.arguments.length === 2 &&
          typeof windowHandle === "number" &&
          Number.isInteger(windowHandle) &&
          typeof rawAccept === "number" &&
          Number.isInteger(rawAccept),
        "DragAcceptFiles arguments are malformed",
      );
      invariant(
        completion.eax.action === "preserve" &&
          completion.lastError === 0 &&
          guestWrites.length === 0,
        "DragAcceptFiles ABI completion changed",
      );
      const window = metadata.window;
      invariant(
        window !== undefined &&
          window.handle === windowHandle &&
          window.lifecycle.state === "created",
        "DragAcceptFiles omitted its created logical window",
      );
      invariant(
        ((window.effectiveExtendedStyle & 0x0000_0010) !== 0) ===
          (rawAccept !== 0),
        "DragAcceptFiles effective window style disagrees with its BOOL argument",
      );
      this.#report.dragAcceptFiles = {
        call: {
          sequence,
          module: endpoint.module,
          symbol: "DragAcceptFiles",
          occurrence: 1,
          trapInstructionCount: this.#report.instructions,
          arguments: [windowHandle, rawAccept],
          result: { action: "preserve" },
          implementationId:
            "shell32.drag-accept-files.logical-window-style-v1",
          returnAddress: completion.instructionPointer,
          stackPointerBefore: frame.stackPointer,
          stackPointerAfter: completion.stackPointer,
          lastError: 0,
          guestWrites: [],
          window: structuredClone(window),
        },
        windowAuthority: "runtime-user32-logical-window",
        acceptanceAuthority: "effective-extended-style-ws-ex-acceptfiles",
        browserResourceCreated: false,
        wmDropFilesImplemented: false,
      };
    }
    if (
      completion.implementationId ===
      "kernel32.sleep-zero.yield-current-thread.v1"
    ) {
      invariant(
        (this.#configuration.capabilityProfile === "kernel32-sleep-zero" ||
          this.#configuration.capabilityProfile ===
            "kernel32-global-memory-status" ||
          this.#configuration.capabilityProfile ===
            "kernel32-get-file-attributes-a" ||
          this.#configuration.capabilityProfile ===
            "kernel32-create-directory-a" ||
          includesSetWindowPosRepeatActivation(
            this.#configuration.capabilityProfile,
          )) &&
          endpoint.module.toLowerCase() === "kernel32.dll" &&
          endpoint.symbol.kind === "name" &&
          endpoint.symbol.name === "Sleep",
        "Sleep(0) completed outside its capability profile",
      );
      invariant(
        call.arguments.length === 1 &&
          call.arguments[0] === 0 &&
          completion.eax.action === "preserve" &&
          completion.lastError === 0 &&
          guestWrites.length === 0 &&
          completion.executionBoundary === "yield-current-thread",
        "Sleep(0) ABI or execution-boundary completion changed",
      );
      const occurrence = (this.#report.sleepZero?.callCount ?? 0) + 1;
      const sleepCall = {
        sequence,
        module: endpoint.module,
        symbol: "Sleep" as const,
        occurrence,
        trapInstructionCount: this.#report.instructions,
        arguments: [0] as [0],
        result: { action: "preserve" as const },
        implementationId:
          "kernel32.sleep-zero.yield-current-thread.v1" as const,
        returnAddress: completion.instructionPointer,
        stackPointerBefore: frame.stackPointer,
        stackPointerAfter: completion.stackPointer,
        lastError: completion.lastError,
        guestWrites: [] as [],
        executionBoundary: "yield-current-thread" as const,
      };
      if (this.#report.sleepZero === null) {
        this.#report.sleepZero = {
          callCount: 1,
          firstCall: sleepCall,
          lastCall: sleepCall,
          executionBoundary: "yield-current-thread",
          schedulerAuthority: "rust-runtime-scheduler",
          schedulerSelections: {
            selectionCount: 0,
            firstSelection: null,
            lastSelection: null,
          },
          noCompetitorCompletions: 0,
          hostWaitCreated: false,
        };
      } else {
        this.#report.sleepZero.callCount = occurrence;
        this.#report.sleepZero.lastCall = sleepCall;
      }
    }
    if (
      completion.implementationId ===
      "kernel32.global-memory-status.x86-deterministic.v1"
    ) {
      invariant(
        (this.#configuration.capabilityProfile ===
          "kernel32-global-memory-status" ||
          this.#configuration.capabilityProfile ===
            "kernel32-get-file-attributes-a" ||
          this.#configuration.capabilityProfile ===
            "kernel32-create-directory-a" ||
          includesSetWindowPosRepeatActivation(
            this.#configuration.capabilityProfile,
          )) &&
          this.#report.globalMemoryStatus === null &&
          endpoint.module.toLowerCase() === "kernel32.dll" &&
          endpoint.symbol.kind === "name" &&
          endpoint.symbol.name === "GlobalMemoryStatus",
        "GlobalMemoryStatus completed outside its capability profile or more than once",
      );
      const [output] = call.arguments;
      const expectedBytes = [
        0x20, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0xff, 0x7f,
        0xff, 0xff, 0xff, 0x7f,
        0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff,
        0x00, 0x00, 0xfe, 0x7f,
        0x00, 0x00, 0x00, 0x60,
      ];
      invariant(
        call.arguments.length === 1 &&
          typeof output === "number" &&
          Number.isInteger(output) &&
          output !== 0 &&
          completion.eax.action === "preserve" &&
          completion.lastError === 0 &&
          guestWrites.length === 1 &&
          guestWrites[0]?.write.address === output &&
          guestWrites[0].write.bytes.length === expectedBytes.length &&
          guestWrites[0].write.bytes.every(
            (byte, index) => byte === expectedBytes[index],
          ),
        "GlobalMemoryStatus ABI or deterministic output changed",
      );
      this.#report.globalMemoryStatus = {
        call: {
          sequence,
          module: endpoint.module,
          symbol: "GlobalMemoryStatus",
          occurrence: 1,
          trapInstructionCount: this.#report.instructions,
          arguments: [output],
          result: { action: "preserve" },
          implementationId:
            "kernel32.global-memory-status.x86-deterministic.v1",
          returnAddress: completion.instructionPointer,
          stackPointerBefore: frame.stackPointer,
          stackPointerAfter: completion.stackPointer,
          lastError: 0,
          guestWrites: [{ address: output, bytes: [...expectedBytes] }],
        },
        stateAuthority: "kernel32-compatibility-policy",
        virtualSpanAuthority: "kernel32-system-info-policy",
        hostMemoryImported: false,
        browserMemoryImported: false,
        allocatorQuotaAuthority: false,
      };
    }
    if (
      completion.implementationId ===
      "kernel32.create-directory-a.runtime-vfs.v1"
    ) {
      const [pathAddress, securityAttributes] = call.arguments;
      const directoryCreate = metadata.writableDirectoryCreate;
      invariant(
        (this.#configuration.capabilityProfile ===
          "kernel32-create-directory-a" ||
          includesSetWindowPosRepeatActivation(
            this.#configuration.capabilityProfile,
          )) &&
          this.#report.createDirectoryA === null &&
          endpoint.module.toLowerCase() === "kernel32.dll" &&
          endpoint.symbol.kind === "name" &&
          endpoint.symbol.name === "CreateDirectoryA" &&
          call.arguments.length === 2 &&
          typeof pathAddress === "number" &&
          Number.isInteger(pathAddress) &&
          pathAddress !== 0 &&
          securityAttributes === 0 &&
          completion.eax.action === "set" &&
          completion.eax.value === 1 &&
          Number.isInteger(completion.lastError) &&
          completion.lastError >= 0 &&
          completion.lastError <= 0xffff_ffff &&
          guestWrites.length === 0 &&
          directoryCreate !== undefined &&
          directoryCreate.request.requestId > 0 &&
          directoryCreate.request.directoryId === 1 &&
          directoryCreate.request.baseGeneration === 0 &&
          directoryCreate.request.relativePathA.length === 8 &&
          directoryCreate.request.relativePathA.every(
            (byte, index) => byte === [85, 115, 101, 114, 68, 97, 116, 97][index],
          ) &&
          directoryCreate.succeeded &&
          directoryCreate.committedGeneration === 1 &&
          directoryCreate.win32Error === 0,
        "CreateDirectoryA ABI or durable VFS transaction changed",
      );
      const directoryCall: RuntimeCreateDirectoryACallSnapshot = {
        sequence,
        module: endpoint.module,
        symbol: "CreateDirectoryA",
        occurrence: 1,
        trapInstructionCount: this.#report.instructions,
        arguments: [pathAddress, 0],
        result: { action: "set", value: 1 },
        implementationId: "kernel32.create-directory-a.runtime-vfs.v1",
        returnAddress: completion.instructionPointer,
        stackPointerBefore: frame.stackPointer,
        stackPointerAfter: completion.stackPointer,
        lastError: completion.lastError,
        guestWrites: [],
        writableDirectoryCreate: structuredClone(directoryCreate),
      };
      this.#report.createDirectoryA = {
        call: directoryCall,
        directoryAuthority: "rust-runtime-vfs-writable-profile",
        durableAuthority: "sqlite-writable-profile-directory-generation",
        transaction: "validate-plan-host-publish-targeted-commit",
        sourceMutation: false,
        hostPathCreated: false,
        browserFileSystemImported: false,
      };
    }
    if (
      completion.implementationId ===
      "kernel32.get-file-attributes-a.runtime-vfs.v1"
    ) {
      const [pathAddress] = call.arguments;
      invariant(
        (this.#configuration.capabilityProfile ===
          "kernel32-get-file-attributes-a" ||
          this.#configuration.capabilityProfile ===
            "kernel32-create-directory-a" ||
          includesSetWindowPosRepeatActivation(
            this.#configuration.capabilityProfile,
          )) &&
          endpoint.module.toLowerCase() === "kernel32.dll" &&
          endpoint.symbol.kind === "name" &&
          endpoint.symbol.name === "GetFileAttributesA" &&
          call.arguments.length === 1 &&
          typeof pathAddress === "number" &&
          Number.isInteger(pathAddress) &&
          pathAddress !== 0 &&
          completion.eax.action === "set" &&
          completion.eax.value === 0xffff_ffff &&
          (completion.lastError === 2 || completion.lastError === 3) &&
          guestWrites.length === 0,
        "GetFileAttributesA ABI or VFS missing-path result changed",
      );
      const occurrence = (this.#report.getFileAttributesA?.callCount ?? 0) + 1;
      const attributeCall: RuntimeGetFileAttributesACallSnapshot = {
        sequence,
        module: endpoint.module,
        symbol: "GetFileAttributesA",
        occurrence,
        trapInstructionCount: this.#report.instructions,
        arguments: [pathAddress],
        result: { action: "set", value: 0xffff_ffff },
        implementationId: "kernel32.get-file-attributes-a.runtime-vfs.v1",
        returnAddress: completion.instructionPointer,
        stackPointerBefore: frame.stackPointer,
        stackPointerAfter: completion.stackPointer,
        lastError: completion.lastError,
        guestWrites: [],
      };
      if (this.#report.getFileAttributesA === null) {
        this.#report.getFileAttributesA = {
          callCount: 1,
          firstCall: attributeCall,
          lastCall: attributeCall,
          pathAuthority: "rust-runtime-vfs",
          fileExistenceAuthority: "registered-source-and-writable-manifests",
          directoryExistence:
            this.#configuration.capabilityProfile ===
              "kernel32-create-directory-a" ||
            includesSetWindowPosRepeatActivation(
              this.#configuration.capabilityProfile,
            )
              ? "registered-path-ancestors-and-writable-directory-state"
              : "registered-path-ancestor-derivation",
          hostFileSystemImported: false,
          browserFileSystemImported: false,
          mutation: false,
        };
      } else {
        this.#report.getFileAttributesA.callCount = occurrence;
        this.#report.getFileAttributesA.lastCall = attributeCall;
      }
    }
    this.#semanticCallDigest.update(call);
    this.#report.supportedCallCount += 1;
    if (this.#configuration.evidenceLevel === "full") {
      this.#report.supportedCalls.push(call);
    }
    if (
      completion.implementationId ===
        "kernel32.wait-for-single-object-ex.waitable-timer.v1" &&
      this.#configuration.capabilityProfile ===
        "kernel32-wait-for-single-object-ex"
    ) {
      this.#waitForSingleObjectExCompletions += 1;
      if (this.#waitForSingleObjectExCompletions === 3) {
        this.#report.checkpointCompletion = {
          capability: "kernel32-wait-for-single-object-ex",
          implementationId:
            "kernel32.wait-for-single-object-ex.waitable-timer.v1",
          requiredCompletions: 3,
          completionCount: 3,
          lastCompletionSequence: sequence,
          lastCompletionInstructionCount: this.#report.instructions,
          instructionCount: 0,
          nextEndpoint: { id: 0, address: 0 },
          memoryContentSha256: "",
        };
        this.#bgiCheckpointPending = true;
      }
    }
    if (this.#configuration.capabilityProfile === "kernel32-enter-critical-section") {
      if (completion.implementationId === "kernel32.enter-critical-section.v1") {
        this.#enterCriticalSectionCompletions += 1;
        if (
          this.#enterCriticalSectionCompletions >= 32 &&
          this.#enterCriticalSectionCompletions <= 34
        ) {
          invariant(
            this.#enterCriticalSectionAcquisition === null,
            "primary critical-section checkpoint nested an acquisition",
          );
          const criticalSectionAddress = call.arguments[0];
          invariant(
            typeof criticalSectionAddress === "number" &&
              Number.isInteger(criticalSectionAddress) &&
              criticalSectionAddress > 0,
            "EnterCriticalSection checkpoint omitted its section address",
          );
          this.#enterCriticalSectionAcquisition = {
            sequence,
            occurrence: this.#enterCriticalSectionCompletions,
            instructionCount: this.#report.instructions,
            criticalSectionAddress,
          };
        }
      } else if (completion.implementationId === "kernel32.leave-critical-section.v1") {
        this.#leaveCriticalSectionCompletions += 1;
        const acquisition = this.#enterCriticalSectionAcquisition;
        if (
          acquisition !== null &&
          this.#leaveCriticalSectionCompletions === acquisition.occurrence - 1
        ) {
          invariant(
            call.arguments[0] === acquisition.criticalSectionAddress,
            "primary EnterCriticalSection continuation released a different section",
          );
          this.#enterCriticalSectionRoundtrips.push({
            criticalSectionAddress: acquisition.criticalSectionAddress,
            acquisition: {
              implementationId: "kernel32.enter-critical-section.v1",
              sequence: acquisition.sequence,
              occurrence: acquisition.occurrence,
              instructionCount: acquisition.instructionCount,
            },
            release: {
              implementationId: "kernel32.leave-critical-section.v1",
              sequence,
              occurrence: this.#leaveCriticalSectionCompletions,
              instructionCount: this.#report.instructions,
            },
          });
          this.#enterCriticalSectionAcquisition = null;
          if (this.#enterCriticalSectionRoundtrips.length === 3) {
            const lastRoundtrip = this.#enterCriticalSectionRoundtrips.at(-1);
            invariant(lastRoundtrip !== undefined, "critical-section cycle is empty");
            this.#report.checkpointCompletion = {
              capability: "kernel32-enter-critical-section",
              implementationId: "kernel32.enter-critical-section.v1",
              requiredCompletions: 3,
              completionCount: 3,
              lastCompletionSequence: lastRoundtrip.acquisition.sequence,
              lastCompletionInstructionCount:
                lastRoundtrip.acquisition.instructionCount,
              roundtrips: structuredClone(this.#enterCriticalSectionRoundtrips),
              instructionCount: 0,
              nextEndpoint: { id: 0, address: 0 },
              memoryContentSha256: "",
            };
            this.#bgiCheckpointPending = true;
          }
        }
      }
    }
    if (
      this.#configuration.capabilityProfile === "kernel32-leave-critical-section" &&
      completion.implementationId === "kernel32.leave-critical-section.v1" &&
      this.#mainLoopThreadSelections.length === 2 &&
      this.#currentThreadId === this.#initialThreadId &&
      sequence === (this.#mainLoopThreadSelections.at(-1)?.sequence ?? -1) + 1
    ) {
      const criticalSectionAddress = call.arguments[0];
      invariant(
        typeof criticalSectionAddress === "number" &&
          Number.isInteger(criticalSectionAddress) &&
          criticalSectionAddress > 0,
        "LeaveCriticalSection checkpoint omitted its section address",
      );
      this.#report.checkpointCompletion = {
        capability: "kernel32-leave-critical-section",
        implementationId: "kernel32.leave-critical-section.v1",
        requiredCompletions: 1,
        completionCount: 1,
        lastCompletionSequence: sequence,
        lastCompletionInstructionCount: this.#report.instructions,
        criticalSectionAddress,
        releaseThreadId: this.#currentThreadId,
        lastError: completion.lastError,
        instructionCount: 0,
        nextEndpoint: { id: 0, address: 0 },
        memoryContentSha256: "",
      };
      this.#bgiCheckpointPending = true;
    }
    if (
      (this.#configuration.capabilityProfile ===
        "kernel32-enter-critical-section-continuation" ||
        this.#configuration.capabilityProfile ===
          "kernel32-leave-critical-section-continuation") &&
      completion.implementationId === "kernel32.leave-critical-section.v1" &&
      this.#mainLoopThreadSelections.length === 2 &&
      this.#currentThreadId === this.#initialThreadId &&
      sequence ===
        (this.#mainLoopThreadSelections.at(-1)?.sequence ?? -1) + 1
    ) {
      const criticalSectionAddress = call.arguments[0];
      invariant(
        typeof criticalSectionAddress === "number" &&
          Number.isInteger(criticalSectionAddress) &&
          criticalSectionAddress > 0,
        "EnterCriticalSection continuation release omitted its section address",
      );
      this.#enterCriticalSectionContinuationRelease = {
        sequence,
        criticalSectionAddress,
      };
    }
    if (
      this.#configuration.capabilityProfile ===
        "kernel32-enter-critical-section-continuation" &&
      completion.implementationId === "kernel32.enter-critical-section.v1" &&
      this.#enterCriticalSectionContinuationRelease !== null &&
      sequence === this.#enterCriticalSectionContinuationRelease.sequence + 1 &&
      this.#currentThreadId === this.#initialThreadId
    ) {
      const criticalSectionAddress = call.arguments[0];
      invariant(
        criticalSectionAddress ===
          this.#enterCriticalSectionContinuationRelease.criticalSectionAddress,
        "EnterCriticalSection continuation acquired a different section",
      );
      invariant(
        completion.lastError === 0,
        "EnterCriticalSection continuation changed LastError",
      );
      this.#report.checkpointCompletion = {
        capability: "kernel32-enter-critical-section-continuation",
        implementationId: "kernel32.enter-critical-section.v1",
        requiredCompletions: 1,
        completionCount: 1,
        lastCompletionSequence: sequence,
        lastCompletionInstructionCount: this.#report.instructions,
        criticalSectionAddress,
        acquisitionThreadId: this.#currentThreadId,
        lastError: completion.lastError,
        instructionCount: 0,
        nextEndpoint: { id: 0, address: 0 },
        memoryContentSha256: "",
      };
      this.#bgiCheckpointPending = true;
    }
    if (
      this.#configuration.capabilityProfile ===
        "kernel32-leave-critical-section-continuation" &&
      completion.implementationId === "kernel32.enter-critical-section.v1" &&
      this.#enterCriticalSectionContinuationRelease !== null &&
      sequence === this.#enterCriticalSectionContinuationRelease.sequence + 1 &&
      this.#currentThreadId === this.#initialThreadId
    ) {
      const criticalSectionAddress = call.arguments[0];
      invariant(
        criticalSectionAddress ===
          this.#enterCriticalSectionContinuationRelease.criticalSectionAddress,
        "LeaveCriticalSection continuation acquisition changed section",
      );
      invariant(
        completion.lastError === 0,
        "LeaveCriticalSection continuation acquisition changed LastError",
      );
      this.#leaveCriticalSectionContinuationAcquisition = {
        sequence,
        criticalSectionAddress,
      };
    }
    if (
      this.#configuration.capabilityProfile ===
        "kernel32-leave-critical-section-continuation" &&
      completion.implementationId === "kernel32.leave-critical-section.v1" &&
      this.#leaveCriticalSectionContinuationAcquisition !== null &&
      sequence ===
        this.#leaveCriticalSectionContinuationAcquisition.sequence + 1 &&
      this.#currentThreadId === this.#initialThreadId
    ) {
      const criticalSectionAddress = call.arguments[0];
      invariant(
        criticalSectionAddress ===
          this.#leaveCriticalSectionContinuationAcquisition
            .criticalSectionAddress,
        "LeaveCriticalSection continuation released a different section",
      );
      invariant(
        completion.lastError === 0,
        "LeaveCriticalSection continuation changed LastError",
      );
      this.#report.checkpointCompletion = {
        capability: "kernel32-leave-critical-section-continuation",
        implementationId: "kernel32.leave-critical-section.v1",
        requiredCompletions: 1,
        completionCount: 1,
        lastCompletionSequence: sequence,
        lastCompletionInstructionCount: this.#report.instructions,
        criticalSectionAddress,
        releaseThreadId: this.#currentThreadId,
        lastError: completion.lastError,
        instructionCount: 0,
        nextEndpoint: { id: 0, address: 0 },
        memoryContentSha256: "",
      };
      this.#bgiCheckpointPending = true;
    }
  }

  #recordD3d9RecreatedTextureCall(call: RuntimeSupportedCallSnapshot): void {
    if (
      this.#configuration.capabilityProfile !== "d3d9-recreated-textures" ||
      call.sequence < 108_735 ||
      call.sequence > 108_743
    ) {
      return;
    }
    if (this.#report.d3d9RecreatedTextures === null) {
      invariant(
        call.sequence === 108_735,
        "D3D9 recreated texture evidence did not start at the first texture",
      );
      this.#report.d3d9RecreatedTextures = {
        calls: [],
        compatibilityPolicyId: "d3d9-primary-virtual-adapter-v17",
        createTextureProfileId:
          "primary-initial-and-recreated-x8r8g8b8-textures-v2",
        getSurfaceLevelProfileId:
          "primary-system-memory-texture-level-zero-surfaces-v2",
        textureAuthority: "rust-runtime-graphics-logical-texture",
        comAuthority: "rust-runtime-com-object-and-interface",
        browserRendererResourceCreated: false,
        mutation: "cross-owner-preflight-transactional-commit",
      };
    }
    invariant(
      call.sequence === 108_735 + this.#report.d3d9RecreatedTextures.calls.length,
      "D3D9 recreated texture evidence sequence is not contiguous",
    );
    this.#report.d3d9RecreatedTextures.calls.push(structuredClone(call));
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
    this.#cancelAndFinish(
      "structured-stop",
      "Structured stop retired the private runtime",
      null,
    );
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
    this.#cancelAndFinish(
      "structured-stop",
      "Execution stop retired the private runtime",
      null,
    );
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
        invariant(
          this.#setWindowPosRepeatActivationCallbackEntry?.frameId !==
            metadata.replacedFrameId,
          "SetWindowPos repeat activation unexpectedly replaced its window callback",
        );
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
      this.#recordBgiCompletion(metadata, sequence, result);
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
    executionBoundary: ApiCompletion["executionBoundary"],
  ): void {
    if (executionBoundary === "yield-current-thread") {
      const sleepZero = this.#report.sleepZero;
      invariant(
        (this.#configuration.capabilityProfile === "kernel32-sleep-zero" ||
          this.#configuration.capabilityProfile ===
            "kernel32-global-memory-status" ||
          this.#configuration.capabilityProfile ===
            "kernel32-get-file-attributes-a" ||
          this.#configuration.capabilityProfile ===
            "kernel32-create-directory-a" ||
          includesSetWindowPosRepeatActivation(
            this.#configuration.capabilityProfile,
          )) &&
          sleepZero !== null &&
          sleepZero.callCount ===
            sleepZero.schedulerSelections.selectionCount +
              sleepZero.noCompetitorCompletions +
              1,
        "Sleep(0) execution boundary is out of sequence",
      );
      if (selection === undefined) {
        sleepZero.noCompetitorCompletions += 1;
      }
    }
    if (selection === undefined) {
      this.#execution.transition(callerResumeState, {
        cause:
          executionBoundary === "yield-current-thread"
            ? "synchronous-api-explicit-yield-no-competitor"
            : "synchronous-api-complete",
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
    if (
      this.#configuration.capabilityProfile === "primary-main-loop-progression" ||
      this.#configuration.capabilityProfile === "kernel32-leave-critical-section" ||
      this.#configuration.capabilityProfile ===
        "kernel32-enter-critical-section-continuation" ||
      this.#configuration.capabilityProfile ===
        "kernel32-leave-critical-section-continuation" ||
      this.#configuration.capabilityProfile === "shell32-drag-accept-files" ||
      this.#configuration.capabilityProfile === "kernel32-archive-source-read" ||
      this.#configuration.capabilityProfile === "kernel32-sleep-zero" ||
      this.#configuration.capabilityProfile === "kernel32-global-memory-status" ||
      this.#configuration.capabilityProfile === "kernel32-get-file-attributes-a" ||
      this.#configuration.capabilityProfile === "kernel32-create-directory-a" ||
      includesSetWindowPosRepeatActivation(
        this.#configuration.capabilityProfile,
      )
    ) {
      const selectionSnapshot = {
        sequence: this.#nextBgiCallSequence,
        instructionCount: this.#report.instructions,
        previousThreadId: selection.previousThreadId,
        threadId: selection.threadId,
        state: structuredClone(selection.state),
        ...(executionBoundary === undefined ? {} : { executionBoundary }),
      };
      this.#mainLoopThreadSelections.push(selectionSnapshot);
      if (executionBoundary === "yield-current-thread") {
        const sleepZero = this.#report.sleepZero ??
          failInvariant("Sleep(0) scheduler evidence is missing");
        sleepZero.schedulerSelections.selectionCount += 1;
        sleepZero.schedulerSelections.firstSelection ??=
          structuredClone(selectionSnapshot);
        sleepZero.schedulerSelections.lastSelection =
          structuredClone(selectionSnapshot);
      }
      if (
        this.#configuration.capabilityProfile === "primary-main-loop-progression" &&
        this.#mainLoopThreadSelections.length === 2 &&
        selection.threadId === this.#initialThreadId
      ) {
        this.#report.checkpointCompletion = {
          capability: "primary-main-loop-progression",
          implementationId: "runtime.scheduler.api-boundary-round-robin.v1",
          requiredCompletions: 2,
          completionCount: 2,
          lastCompletionSequence: this.#nextBgiCallSequence,
          lastCompletionInstructionCount: this.#report.instructions,
          threadSelections: structuredClone(this.#mainLoopThreadSelections),
          instructionCount: 0,
          nextEndpoint: { id: 0, address: 0 },
          memoryContentSha256: "",
        };
        this.#bgiCheckpointPending = true;
      }
    }
    this.#saveLiveThreadContext();
    this.#execution.transition("thread-switch", {
      cause:
        executionBoundary === "yield-current-thread"
          ? "api-boundary-explicit-yield"
          : "api-boundary-quantum-expired",
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
    if (metadata.service === "runtime.timer-delay.v1") {
      invariant(metadata.arguments.length === 13, "timer pacing request must contain thirteen words");
      const [
        version,
        delayMilliseconds,
        registrationLow,
        registrationHigh,
        timerLow,
        timerHigh,
        targetLow,
        targetHigh,
        clockGenerationLow,
        clockGenerationHigh,
        armGenerationLow,
        armGenerationHigh,
        outcome,
      ] = metadata.arguments;
      invariant(version === 1, "timer pacing request version mismatch");
      invariant(
        Number.isInteger(delayMilliseconds) &&
          delayMilliseconds !== undefined &&
          delayMilliseconds > 0 &&
          delayMilliseconds <= 10,
        "timer pacing delay exceeds the runtime policy",
      );
      invariant(
        registrationLow !== undefined &&
          registrationHigh !== undefined &&
          (registrationLow !== 0 || registrationHigh !== 0),
        "timer pacing wait registration identity is zero",
      );
      invariant(
        timerLow !== undefined && timerHigh !== undefined && (timerLow !== 0 || timerHigh !== 0),
        "timer pacing timer identity is zero",
      );
      invariant(
        targetLow !== undefined && targetHigh !== undefined && (targetLow !== 0 || targetHigh !== 0),
        "timer pacing logical target is zero",
      );
      invariant(
        clockGenerationLow !== undefined && clockGenerationHigh !== undefined,
        "timer pacing clock generation is missing",
      );
      invariant(
        armGenerationLow !== undefined &&
          armGenerationHigh !== undefined &&
          (outcome !== 0 || armGenerationLow !== 0 || armGenerationHigh !== 0),
        "timer pacing arm generation does not match its outcome",
      );
      invariant(outcome === 0 || outcome === 1, "timer pacing outcome is invalid");
      if (this.#configuration.mode === "success") {
        this.#timerPacing.schedule({
          requestId: metadata.requestId,
          threadId: metadata.threadId,
          delayMilliseconds,
          complete: () => {
            this.completeHost({
              type: "host-completion",
              protocolVersion,
              sessionId: this.#sessionId,
              requestId: metadata.requestId,
              threadId: metadata.threadId,
              result: 1,
            });
          },
        });
      }
    } else if (metadata.service === "storage.source-read.v1") {
      if (this.#configuration.mode === "success") {
        void this.#serviceSourceRead(metadata).catch((error: unknown) => {
          if (!this.#terminal && !this.#stopping) {
            this.#fail(error instanceof Error ? error : new Error(String(error)));
          }
        });
      }
    } else if (metadata.service === "storage.profile-read.v1") {
      if (this.#configuration.mode === "success") {
        void this.#serviceProfileRead(metadata).catch((error: unknown) => {
          if (!this.#terminal && !this.#stopping) {
            this.#fail(error instanceof Error ? error : new Error(String(error)));
          }
        });
      }
    } else if (metadata.service === "storage.profile-commit.v1") {
      if (this.#configuration.mode === "success") {
        const task = this.#serviceWritableCommit(metadata);
        this.#writableMutationTasks.add(task);
        void task.then(
          () => {
            this.#writableMutationTasks.delete(task);
          },
          (error: unknown) => {
            this.#writableMutationTasks.delete(task);
            if (!this.#terminal && !this.#stopping) {
              this.#fail(error instanceof Error ? error : new Error(String(error)));
            }
          },
        );
      }
    } else if (metadata.service === "storage.profile-directory-create.v1") {
      if (this.#configuration.mode === "success") {
        const task = this.#serviceWritableDirectoryCreate(metadata);
        this.#writableMutationTasks.add(task);
        void task.then(
          () => {
            this.#writableMutationTasks.delete(task);
          },
          (error: unknown) => {
            this.#writableMutationTasks.delete(task);
            if (!this.#terminal && !this.#stopping) {
              this.#fail(error instanceof Error ? error : new Error(String(error)));
            }
          },
        );
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

  async #scheduleAfterBlock(
    blockedThreadId: number,
    requestId: string | null,
    waitForHostBeforeScheduling = false,
  ): Promise<void> {
    this.#saveLiveThreadContext();
    if (waitForHostBeforeScheduling) {
      this.#liveThreadId = null;
      const pending =
        requestId === null ? null : this.#pendingHostRequests.pending(requestId);
      invariant(
        pending !== null && pending.threadId === blockedThreadId,
        "synchronous host wait has no matching delivery record",
      );
      invariant(
        this.#pendingHostRequests.size() === 1,
        "synchronous host wait overlaps another pending host request",
      );
      invariant(
        this.#hostBarrierRequestId === null,
        "synchronous host completion barrier is already armed",
      );
      this.#hostBarrierRequestId = pending.requestId;
      this.#execution.transition("waiting-host", {
        cause: "synchronous-host-execution-barrier",
        threadId: blockedThreadId,
        requestId,
      });
      try {
        await this.#waitForRunnable();
      } finally {
        this.#hostBarrierRequestId = null;
      }
      if (this.#terminal || this.#stopping) {
        throw new SessionStopped("session ended during a synchronous host wait");
      }
      invariant(
        this.#execution.state() === "host-completion",
        "synchronous host wake did not originate from its host completion",
      );
      const resume = this.#hostBarrierResume;
      invariant(
        resume?.requestId === requestId && resume.threadId === blockedThreadId,
        "synchronous host completion omitted its Rust-owned owner resume",
      );
      this.#hostBarrierResume = null;
      this.#execution.transition("thread-switch", {
        cause: "rust-synchronous-host-owner-resume",
        threadId: resume.threadId,
        requestId,
      });
      this.#activateThread(resume.threadId, resume.scheduler);
      this.#execution.transition(
        resume.scheduler.state === "in-callback" ? "guest-callback" : "ready",
        {
          cause: "synchronous-host-owner-context-restored",
          threadId: resume.threadId,
          requestId,
        },
      );
      return;
    }
    while (!this.#terminal && !this.#stopping) {
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
      if (this.#terminal || this.#stopping) {
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
    if (this.#terminal || this.#stopping) {
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
    this.#cancelAndFinish(outcome, detail, detail);
  }

  #fail(error: Error): void {
    this.#cancelAndFinish("fault", error.message, error.message);
  }

  #cancelAndFinish(
    outcome:
      | "cancelled"
      | "watchdog"
      | "fault"
      | "structured-stop"
      | "checkpoint-complete",
    detail: string,
    reportedError: string | null,
  ): void {
    if (this.#terminal || this.#stopping) {
      return;
    }
    this.#stopping = true;
    this.#timerPacing.cancelAll();
    this.#hostBarrierResume = null;
    this.#hostBarrierRequestId = null;
    this.#deferredHostCompletions.clear();
    this.#abortSourceReads();
    this.#abortProfileTransports();
    this.#terminalTask = (async () => {
      let pendingHostCalls = this.#pendingHostRequests.size();
      let pendingCallbacks = 0;
      const cleanupErrors: string[] = [];
      try {
        await this.#settleProfileLeaseAcquisition();
      } catch (cleanupError) {
        cleanupErrors.push(`profile lease acquisition: ${String(cleanupError)}`);
      }
      try {
        await this.#drainProfileTransports();
      } catch (cleanupError) {
        cleanupErrors.push(`profile read transport: ${String(cleanupError)}`);
      }
      try {
        await this.#drainWritableMutations();
      } catch (cleanupError) {
        cleanupErrors.push(`writable commit transport: ${String(cleanupError)}`);
      }
      try {
        await this.#revokeWritableProfileLease();
      } catch (cleanupError) {
        cleanupErrors.push(`profile lease revoke: ${String(cleanupError)}`);
      }
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
            invariant(
              metadata.dllLifecycle === "process-detached",
              "BGI cancellation did not detach",
            );
          }
        } catch (cleanupError) {
          cleanupErrors.push(`Rust cancellation: ${String(cleanupError)}`);
        }
      }
      this.#pendingHostRequests.rejectAll(new SessionStopped(detail));
      this.#finishNow({
        outcome,
        exitCode: null,
        pendingHostCalls,
        pendingCallbacks,
        error:
          cleanupErrors.length === 0
            ? reportedError
            : `${reportedError ?? detail}; cleanup failed: ${cleanupErrors.join("; ")}`,
      });
    })();
  }

  #abortSourceReads(): void {
    for (const controller of this.#sourceReadControllers.values()) {
      controller.abort();
    }
    this.#sourceReadControllers.clear();
  }

  #abortProfileTransports(): void {
    for (const controller of this.#profileTransportControllers) {
      controller.abort();
    }
  }

  #finish(
    terminal: Pick<
      SessionReport,
      "outcome" | "exitCode" | "pendingHostCalls" | "pendingCallbacks" | "error"
    >,
  ): void {
    if (this.#terminal || this.#stopping) {
      return;
    }
    this.#stopping = true;
    this.#abortSourceReads();
    this.#abortProfileTransports();
    this.#terminalTask = (async () => {
      let final = terminal;
      try {
        await this.#settleProfileLeaseAcquisition();
        await this.#drainProfileTransports();
        await this.#drainWritableMutations();
        await this.#revokeWritableProfileLease();
      } catch (cleanupError) {
        final = {
          outcome: "fault",
          exitCode: null,
          pendingHostCalls: this.#pendingHostRequests.size(),
          pendingCallbacks: terminal.pendingCallbacks,
          error: `terminal storage barrier failed: ${String(cleanupError)}`,
        };
      }
      this.#finishNow(final);
    })();
  }

  async #settleProfileLeaseAcquisition(): Promise<void> {
    const acquisition = this.#profileLeaseAcquisitionTask;
    if (acquisition !== null) {
      await acquisition;
    }
  }

  async #drainProfileTransports(): Promise<void> {
    if (this.#profileTransportTasks.size > 0) {
      await Promise.allSettled([...this.#profileTransportTasks]);
    }
  }

  async #drainWritableMutations(): Promise<void> {
    if (this.#writableMutationTasks.size === 0) {
      return;
    }
    await Promise.allSettled([...this.#writableMutationTasks]);
  }

  async #revokeWritableProfileLease(): Promise<void> {
    const lease = this.#profileLease;
    if (lease === null) {
      return;
    }
    const base = this.#sameOriginStorageUrl(
      this.#configuration.writableProfileApiUrl,
      "/api/profile/v3/",
      "writable profile API URL",
    );
    if (!base.pathname.endsWith("/")) {
      base.pathname += "/";
    }
    const url = new URL(`sessions/${lease.epoch}`, base);
    const response = await fetch(url, {
      method: "DELETE",
      cache: "no-store",
      headers: { "X-Vine-Session-Key": lease.sessionKey },
    });
    invariant(response.status === 204, `writable profile revoke failed with ${response.status}`);
    invariant(
      response.headers.get("x-vine-title-fingerprint") === lease.titleFingerprint &&
        response.headers.get("x-vine-profile-id") === lease.profileId &&
        response.headers.get("x-vine-session-key") === lease.sessionKey &&
        response.headers.get("x-vine-session-epoch") === String(lease.epoch),
      "writable profile revoke response identity mismatch",
    );
    this.#profileLease = null;
  }

  #finishNow(
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
    if (
      (this.#configuration.capabilityProfile === "kernel32-sleep-zero" ||
        this.#configuration.capabilityProfile ===
          "kernel32-global-memory-status" ||
        this.#configuration.capabilityProfile ===
          "kernel32-get-file-attributes-a" ||
        this.#configuration.capabilityProfile ===
          "kernel32-create-directory-a" ||
        includesSetWindowPosRepeatActivation(
          this.#configuration.capabilityProfile,
        )) &&
      terminal.outcome === "structured-stop"
    ) {
      const sleepZero = this.#report.sleepZero;
      invariant(
        sleepZero !== null &&
          sleepZero.callCount > 0 &&
          sleepZero.callCount ===
            sleepZero.schedulerSelections.selectionCount +
              sleepZero.noCompetitorCompletions,
        "Sleep(0) report omitted an execution-boundary outcome",
      );
    }
    if (
      (this.#configuration.capabilityProfile === "kernel32-global-memory-status" ||
        this.#configuration.capabilityProfile ===
          "kernel32-get-file-attributes-a" ||
        this.#configuration.capabilityProfile ===
          "kernel32-create-directory-a" ||
        includesSetWindowPosRepeatActivation(
          this.#configuration.capabilityProfile,
        )) &&
      terminal.outcome === "structured-stop"
    ) {
      invariant(
        this.#report.globalMemoryStatus !== null,
        "GlobalMemoryStatus report omitted its deterministic completion",
      );
    }
    if (
      (this.#configuration.capabilityProfile ===
        "kernel32-get-file-attributes-a" ||
        this.#configuration.capabilityProfile ===
          "kernel32-create-directory-a" ||
        includesSetWindowPosRepeatActivation(
          this.#configuration.capabilityProfile,
        )) &&
      terminal.outcome === "structured-stop"
    ) {
      invariant(
        this.#report.getFileAttributesA?.callCount === 23,
        "GetFileAttributesA report omitted a VFS path-query completion",
      );
    }
    if (
      (this.#configuration.capabilityProfile === "kernel32-create-directory-a" ||
        includesSetWindowPosRepeatActivation(
          this.#configuration.capabilityProfile,
        )) &&
      terminal.outcome === "structured-stop"
    ) {
      invariant(
        this.#report.createDirectoryA?.call.writableDirectoryCreate.succeeded === true &&
          this.#report.writableDirectoryCreates.length === 1,
        "CreateDirectoryA report omitted its durable VFS transaction",
      );
    }
    if (
      includesSetWindowPosRepeatActivation(
        this.#configuration.capabilityProfile,
      ) &&
      terminal.outcome === "structured-stop"
    ) {
      invariant(
        this.#setWindowPosCompletionCount === 5 &&
          this.#report.setWindowPosRepeatActivation?.repeatCall.occurrence === 5 &&
          this.#report.setWindowPosRepeatActivation.callbackEvents.length === 2,
        "SetWindowPos report omitted its repeat activation callback completion",
      );
    }
    if (
      this.#configuration.capabilityProfile === "d3d9-recreated-textures" &&
      terminal.outcome === "structured-stop"
    ) {
      const recreated = this.#report.d3d9RecreatedTextures;
      invariant(
        recreated !== null &&
          recreated.calls.length === 9 &&
          recreated.calls[0]?.sequence === 108_735 &&
          recreated.calls.at(-1)?.sequence === 108_743 &&
          recreated.calls.filter(
            (call) => call.symbol === "IDirect3DDevice9.CreateTexture",
          ).length === 3,
        "D3D9 recreated texture report omitted its transactional call sequence",
      );
    }
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
        timerPacing: this.#timerPacing.metrics(),
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
    if (!this.#terminal && !this.#stopping) {
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
            timerPacing: {
              requests: 0,
              completions: 0,
              cancellations: 0,
              freezes: 0,
              resumes: 0,
              staleCallbacks: 0,
              boundedCatchUps: 0,
              pending: 0,
            },
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
            writableDirectoryCreates: [],
            dragAcceptFiles: null,
            sleepZero: null,
            globalMemoryStatus: null,
            getFileAttributesA: null,
            createDirectoryA: null,
            setWindowPosRepeatActivation: null,
            d3d9RecreatedTextures: null,
            firstUnsupported: null,
            checkpointCompletion: null,
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
    case "page-lifecycle":
      activeSession?.updatePageLifecycle(message);
      break;
  }
});
