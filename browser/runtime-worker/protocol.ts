import type { ExecutionState, ExecutionTransition } from "./execution-state.js";
import type { ExecutionEngineIdentity } from "./execution-engine-contract.js";

export const protocolVersion = 49 as const;

export type SessionMode = "success" | "cancel-pending" | "watchdog";
export type PageLifecycleState = "active" | "frozen";
export type BgiCapabilityProfile =
  | "kernel32-wait-for-single-object-ex"
  | "kernel32-enter-critical-section"
  | "primary-main-loop-progression"
  | "kernel32-leave-critical-section"
  | "kernel32-enter-critical-section-continuation"
  | "kernel32-leave-critical-section-continuation"
  | "shell32-drag-accept-files"
  | "kernel32-archive-source-read"
  | "kernel32-sleep-zero"
  | "kernel32-global-memory-status"
  | "kernel32-get-file-attributes-a"
  | "kernel32-create-directory-a"
  | "user32-set-window-pos-repeat-activation"
  | "d3d9-recreated-textures"
  | "user32-set-window-pos-frame-change-move"
  | "user32-show-window-normal"
  | "user32-update-window"
  | "d3d9-clear"
  | "user32-set-rect"
  | "d3d9-add-dirty-rect"
  | "d3d9-update-texture"
  | "d3d9-begin-scene"
  | "d3d9-initial-sampler-states"
  | "d3d9-initial-texture-stage-states"
  | "d3d9-set-texture"
  | "d3d9-set-fvf"
  | "d3d9-set-stream-source"
  | "d3d9-draw-primitive"
  | "d3d9-end-scene"
  | "d3d9-post-end-scene-raster-status"
  | "d3d9-post-raster-status-waitable-timer"
  | "d3d9-post-scene-present-gateway"
  | "d3d9-first-present"
  | "d3d9-post-present-continuation"
  | "user32-async-key-state-input"
  | "user32-async-key-state-input-continuation"
  | "user32-async-key-state-initial-scan"
  | "user32-async-key-state-initial-scan-continuation";

export type RuntimeKeyboardSourceEventKind =
  | "focus-gained"
  | "focus-lost"
  | "key-down"
  | "key-up";

export interface RuntimeKeyboardInputEvent {
  sequence: string;
  kind: RuntimeKeyboardSourceEventKind;
  virtualKey: number | null;
  repeat: boolean;
}

export interface RuntimeKeyboardEventReceipt {
  sequence: number;
  kind: RuntimeKeyboardSourceEventKind;
  virtualKey: number | null;
  repeat: boolean;
  sourceFocused: boolean;
  generation: number;
  affectedKeys: Array<{
    virtualKey: number;
    down: boolean;
    pressedSinceLastQuery: boolean;
  }>;
}

export interface RuntimeKeyboardInputEvidence {
  receivedMessages: number;
  queuedBeforePrepare: number;
  receipts: RuntimeKeyboardEventReceipt[];
}

export interface RuntimeCursorPositionSnapshot {
  profileId: string;
  screenX: number;
  screenY: number;
  generation: number;
}

export type RuntimeGraphicsTextureDirtyRegionSnapshot =
  | { kind: "clean" }
  | {
      kind: "rect";
      rect: { left: number; top: number; right: number; bottom: number };
    }
  | { kind: "whole-texture" };

export interface RuntimeGraphicsBackBufferSnapshot {
  contentGeneration: number;
  uniformClear?: {
    sequence: number;
    rect: { left: number; top: number; right: number; bottom: number };
    colorArgb: number;
  };
  drawSubmissions?: Array<{
    sequence: number;
    sceneSequence: number;
    primitiveTopology: "triangle-strip";
    startVertex: number;
    primitiveCount: number;
    vertexCount: number;
    flexibleVertexFormat: "xyz-rhw-diffuse-texture1";
    streamSource: {
      vertexBufferId: number;
      offsetInBytes: number;
      stride: number;
    };
    vertexBufferContentGeneration: number;
    vertexData: number[];
    renderStates: RuntimeGraphicsDeviceSnapshot["renderStates"];
    samplerStates: NonNullable<RuntimeGraphicsDeviceSnapshot["samplerStates"]>;
    textureStageStates: NonNullable<
      RuntimeGraphicsDeviceSnapshot["textureStageStates"]
    >;
    texture: {
      textureId: number;
      descriptor: RuntimeGraphicsTextureSnapshot["descriptor"];
      generation: number;
      content: NonNullable<RuntimeGraphicsTextureSnapshot["content"]>;
    };
  }>;
}

export interface RuntimeGraphicsDeviceSnapshot {
  id: number;
  namespace: "vine.logical-graphics-device.v1";
  frontend: "d3d9";
  windowId: number;
  windowHandle: number;
  ownerThreadId: number;
  presentation: {
    width: number;
    height: number;
    colorFormat: "b8g8r8x8-unorm";
    backBufferCount: number;
    sampleCount: number;
    presentMode: "discard";
    windowed: boolean;
  };
  comObjectId: string;
  renderStates: {
    cullMode?: "none";
    depthTestEnabled?: boolean;
    lightingEnabled?: boolean;
    alphaBlendEnabled?: boolean;
  };
  samplerStates?: {
    samplerZero: {
      minFilter?: "linear";
      magFilter?: "linear";
    };
  };
  textureStageStates?: {
    stageZero: {
      colorOperation?: "modulate";
      colorArgumentOne?: "texture";
      colorArgumentTwo?: "diffuse";
      alphaOperation?: "select-argument-one";
      alphaArgumentOne?: "texture";
      alphaArgumentTwo?: "diffuse";
    };
  };
  textureBindings?: {
    stageZero: number;
  };
  flexibleVertexFormat?: "xyz-rhw-diffuse-texture1";
  streamSources?: {
    streamZero: {
      vertexBufferId: number;
      offsetInBytes: number;
      stride: number;
    };
  };
  backBuffer: RuntimeGraphicsBackBufferSnapshot;
  presentedFrame?: {
    sequence: number;
    backBuffer: RuntimeGraphicsBackBufferSnapshot;
  };
  activeScene?: {
    sequence: number;
  };
  renderer?: {
    namespace: "vine.browser-renderer-resource.v1";
    resourceId: number;
    ownership: "non-owning";
  };
  generation: number;
}

export interface RuntimeGraphicsTextureSnapshot {
  id: number;
  namespace: "vine.logical-graphics-texture.v1";
  deviceId: number;
  descriptor: {
    width: number;
    height: number;
    levels: number;
    usage: "none" | "dynamic";
    colorFormat: "b8g8r8x8-unorm";
    pool: "default" | "system-memory";
  };
  comObjectId: string;
  dirtyRegion: RuntimeGraphicsTextureDirtyRegionSnapshot;
  content?: {
    contentGeneration: number;
    fills: Array<{
      sequence: number;
      rect: { left: number; top: number; right: number; bottom: number };
      colorRef: number;
    }>;
    lastUpdate?: {
      sequence: number;
      sourceTextureId: number;
      sourceContentGeneration: number;
      dirtyRegion: RuntimeGraphicsTextureDirtyRegionSnapshot;
    };
  };
  generation: number;
}

export interface RuntimeGraphicsTextureUpdateSnapshot {
  source: RuntimeGraphicsTextureSnapshot;
  destination: RuntimeGraphicsTextureSnapshot;
}

export interface RuntimeThreadSelectionSnapshot {
  sequence: number;
  instructionCount: number;
  previousThreadId: number;
  threadId: number;
  state: { state: "runnable" } | { state: "in-callback"; depth: number };
  executionBoundary?: "yield-current-thread";
}

export interface RuntimeSleepZeroCallSnapshot {
  sequence: number;
  module: string;
  symbol: "Sleep";
  occurrence: number;
  trapInstructionCount: number;
  arguments: [0];
  result: { action: "preserve" };
  implementationId: "kernel32.sleep-zero.yield-current-thread.v1";
  returnAddress: number;
  stackPointerBefore: number;
  stackPointerAfter: number;
  lastError: number;
  guestWrites: [];
  executionBoundary: "yield-current-thread";
}

export interface RuntimeGlobalMemoryStatusCallSnapshot {
  sequence: number;
  module: string;
  symbol: "GlobalMemoryStatus";
  occurrence: 1;
  trapInstructionCount: number;
  arguments: [number];
  result: { action: "preserve" };
  implementationId: "kernel32.global-memory-status.x86-deterministic.v1";
  returnAddress: number;
  stackPointerBefore: number;
  stackPointerAfter: number;
  lastError: 0;
  guestWrites: [{ address: number; bytes: number[] }];
}

export interface RuntimeGetFileAttributesACallSnapshot {
  sequence: number;
  module: string;
  symbol: "GetFileAttributesA";
  occurrence: number;
  trapInstructionCount: number;
  arguments: [number];
  result: { action: "set"; value: 0xffff_ffff };
  implementationId: "kernel32.get-file-attributes-a.runtime-vfs.v1";
  returnAddress: number;
  stackPointerBefore: number;
  stackPointerAfter: number;
  lastError: 2 | 3;
  guestWrites: [];
}

export interface RuntimeCreateDirectoryACallSnapshot {
  sequence: number;
  module: string;
  symbol: "CreateDirectoryA";
  occurrence: 1;
  trapInstructionCount: number;
  arguments: [number, 0];
  result: { action: "set"; value: 0 | 1 };
  implementationId: "kernel32.create-directory-a.runtime-vfs.v1";
  returnAddress: number;
  stackPointerBefore: number;
  stackPointerAfter: number;
  lastError: number;
  guestWrites: [];
  writableDirectoryCreate: RuntimeWritableDirectoryCreateSnapshot;
}

export interface RuntimeSetWindowPosRepeatActivationCallSnapshot {
  sequence: number;
  module: string;
  symbol: "SetWindowPos";
  occurrence: 5;
  trapInstructionCount: number;
  arguments: [number, number, number, number, number, number, number];
  result: { action: "set"; value: 1 };
  implementationId: "user32.set-window-pos.v5";
  returnAddress: number;
  stackPointerBefore: number;
  stackPointerAfter: number;
  lastError: number;
  guestWrites: [];
  window: RuntimeWindowSnapshot;
  callback: {
    frameId: number;
    target: number;
    allocationStart: number;
    stackPointer: number;
    scratchAddress: number;
    scratchBytes: number;
    frameBytes: number;
    returnSentinel: number;
    returnInstructionCount: number;
    result: number;
  };
}

export interface RuntimeSetWindowPosRepeatActivationCallbackEvent {
  action: "enter" | "return";
  sequence: number;
  frameId: number;
  instructionCount: number;
  target?: number;
  stackPointer?: number;
  arguments?: [number, number, number, number];
  result?: number;
}

export interface RuntimeSetWindowPosFrameChangeMoveCallSnapshot {
  sequence: number;
  module: string;
  symbol: "SetWindowPos";
  occurrence: 6;
  trapInstructionCount: number;
  arguments: [number, 0, 317, 154, 0, 0, 0x25];
  result: { action: "set"; value: 1 };
  implementationId: "user32.set-window-pos.v5";
  returnAddress: number;
  stackPointerBefore: number;
  stackPointerAfter: number;
  lastError: number;
  guestWrites: [];
  window: RuntimeWindowSnapshot;
}

export interface RuntimeSetWindowPosFrameChangeMoveCallbackEvent {
  action: "enter" | "replace" | "return";
  sequence: number;
  frameId: number;
  instructionCount: number;
  target?: number;
  stackPointer?: number;
  arguments?: [number, number, number, number];
  result?: number;
}

export interface RuntimeShowWindowNormalCallSnapshot {
  sequence: number;
  module: string;
  symbol: "ShowWindow";
  occurrence: 1;
  trapInstructionCount: number;
  arguments: [number, 1];
  result: { action: "set"; value: 1 };
  implementationId: "user32.show-window.visible-normal-noop.v1";
  returnAddress: number;
  stackPointerBefore: number;
  stackPointerAfter: number;
  lastError: number;
  guestWrites: [];
}

export interface RuntimeUpdateWindowEntrySnapshot {
  sequence: number;
  module: string;
  symbol: "UpdateWindow";
  occurrence: 1;
  trapInstructionCount: number;
  arguments: [number];
  pendingCallback: true;
}

export interface RuntimeUpdateWindowCallbackEntryEvent {
  action: "enter";
  sequence: number;
  frameId: number;
  instructionCount: number;
  target: number;
  stackPointer: number;
  arguments: [number, 0x000f, 0, 0];
}

export interface RuntimeUpdateWindowCallbackReturnEvent {
  action: "return";
  sequence: number;
  frameId: number;
  instructionCount: number;
  result: number;
}

export type RuntimeUpdateWindowCallbackEvent =
  | RuntimeUpdateWindowCallbackEntryEvent
  | RuntimeUpdateWindowCallbackReturnEvent;

export interface RuntimePaintSessionSnapshot {
  windowId: number;
  windowHandle: number;
  ownerThreadId: number;
  paintGeneration: number;
  updateRect: { left: number; top: number; right: number; bottom: number };
  deviceContextId: number;
  deviceContextHandle: number;
  paintStructAddress: number;
  paintStructBytes: number[];
}

export type RuntimePaintSnapshot =
  | {
      action: "begun";
      window: RuntimeWindowSnapshot;
      session: RuntimePaintSessionSnapshot;
      deviceContext: RuntimeGdiDeviceContextSnapshot;
      livePaintSessions: number;
      liveDeviceContexts: number;
    }
  | {
      action: "ended";
      window: RuntimeWindowSnapshot;
      paintStructAddress: number;
      livePaintSessions: number;
      liveDeviceContexts: number;
    };

export interface RuntimeBeginPaintCallSnapshot {
  sequence: number;
  module: string;
  symbol: "BeginPaint";
  occurrence: 1;
  trapInstructionCount: number;
  arguments: [number, number];
  result: { action: "set"; value: number };
  implementationId: "user32.begin-paint.full-client-logical-dc.v1";
  returnAddress: number;
  stackPointerBefore: number;
  stackPointerAfter: number;
  lastError: number;
  guestWrites: Array<{ address: number; bytes: number[] }>;
  paint: Extract<RuntimePaintSnapshot, { action: "begun" }>;
}

export interface RuntimeEndPaintCallSnapshot {
  sequence: number;
  module: string;
  symbol: "EndPaint";
  occurrence: 1;
  trapInstructionCount: number;
  arguments: [number, number];
  result: { action: "set"; value: 1 };
  implementationId: "user32.end-paint.matching-session.v1";
  returnAddress: number;
  stackPointerBefore: number;
  stackPointerAfter: number;
  lastError: number;
  guestWrites: [];
  paint: Extract<RuntimePaintSnapshot, { action: "ended" }>;
}

export interface PageLifecycleSnapshot {
  state: PageLifecycleState;
  generation: number;
}
export type SessionWorkload =
  "controlled-fixture" | "controlled-source-read" | "private-bgi";

export interface RuntimeCurrentDirectoryA {
  sourceAuthority: string;
  bytesWithNul: number[];
  encoding: string;
}

export interface RuntimeCurrentDirectorySnapshot {
  directory: RuntimeCurrentDirectoryA;
  generation: number;
}

export interface RuntimeMutexSnapshot {
  handle: number;
  object: {
    id: number;
    nameA?: {
      bytesWithNul: number[];
      utf16UnitsWithNul: number[];
      encoding: string;
    };
    owningThreadId: number | null;
    recursionCount: number;
    abandoned: boolean;
  };
  objectRecord: {
    id: number;
    key: { kind: "mutex"; mutex_id: number };
    kind: "mutex";
    namespace: string;
    closePolicy: "close-handle";
    waitState: null;
    openHandles: number;
  };
  alreadyExists: boolean;
}

export interface RuntimeWaitableTimerSnapshot {
  handle: number;
  timer: {
    id: number;
    manualReset: boolean;
    arm?: {
      relativeDueTime100ns: number;
      armedAtTime100ns: number;
      deadlineTime100ns: number;
      periodMilliseconds: number;
      generation: number;
    };
    lastExpiration?: {
      armGeneration: number;
      deadlineTime100ns: number;
    };
  };
  objectRecord: {
    id: number;
    key: { kind: "waitable-timer"; timer_id: number };
    kind: "waitable-timer";
    namespace: string;
    closePolicy: "close-handle";
    waitState:
      { state: "nonsignaled" } | { state: "signaled"; exit_code: number };
    openHandles: number;
  };
  timerClock?: {
    currentTime100ns: number;
    generation: number;
  };
  nextWaitRegistrationId?: number;
}

export interface RuntimeIconSnapshot {
  id: number;
  handle: number;
  namespace: string;
  shared: boolean;
  resource: {
    moduleId: number;
    moduleLoadBase: number;
    moduleSourceSha256: string;
    groupResourceId: number;
    languageId: number;
    iconResourceId: number;
    groupDataRva: number;
    groupDataSize: number;
    iconDataRva: number;
    iconDataSize: number;
    width: number;
    height: number;
    planes: number;
    bitsPerPixel: number;
  };
}

export interface RuntimeCursorSnapshot {
  id: number;
  handle: number;
  namespace: string;
  shared: boolean;
  source: {
    kind: "system-predefined";
    resourceId: number;
    cursor: "arrow";
  };
}

export interface RuntimeWindowClassSnapshot {
  id: number;
  atom: number;
  atomNamespace: string;
  local: boolean;
  module: {
    moduleId: number;
    loadBase: number;
    sourceSha256: string;
  };
  style: number;
  windowProcedure: {
    moduleId: number;
    address: number;
    rva: number;
    callingConvention: "stdcall";
    argumentBytes: number;
  };
  classExtraBytes: number;
  windowExtraBytes: number;
  icon?: { iconId: number; handle: number };
  cursor?: { cursorId: number; handle: number };
  backgroundBrush: number;
  menuName?: { value: string; encoding: string };
  className: { value: string; encoding: string };
  smallIcon?: { iconId: number; handle: number };
}

export interface RuntimeWindowSnapshot {
  id: number;
  handle: number;
  namespace: string;
  ownerThreadId: number;
  class: { classId: number; atom: number; moduleId: number };
  instance: { moduleId: number; loadBase: number; sourceSha256: string };
  name: { value: string; encoding: string };
  extendedStyle: number;
  effectiveExtendedStyle: number;
  style: number;
  effectiveStyle: number;
  geometry: {
    x: number;
    y: number;
    width: number;
    height: number;
    clientX: number;
    clientY: number;
    clientWidth: number;
    clientHeight: number;
  };
  parent: number;
  menu: number;
  createParameter: number;
  entryLastError: number;
  lifecycle:
    | {
        state: "creating";
        message:
          | "non-client-create"
          | "non-client-calculate"
          | "create"
          | "size"
          | "move";
      }
    | { state: "created" };
  visible: boolean;
  topmost: boolean;
  pendingUpdateRect?: {
    left: number;
    top: number;
    right: number;
    bottom: number;
  };
  paintGeneration: number;
  smallIcon?: { iconId: number; handle: number };
}

export interface RuntimeComApartmentSnapshot {
  model: "single-threaded" | "multi-threaded";
  initializationCount: number;
}

export interface SessionConfiguration {
  mode: SessionMode;
  evidenceLevel: "full" | "summary";
  workload?: SessionWorkload;
  capabilityProfile?: BgiCapabilityProfile;
  executableUrl?: string;
  policyUrl?: string;
  sourceManifestUrl?: string;
  sourceApiUrl?: string;
  writableProfileUrl?: string;
  writableProfileApiUrl?: string;
  instructionBudget: number;
  watchdogInstructionLimit: number;
  watchdogWallMilliseconds: number;
  pageLifecycle: PageLifecycleSnapshot;
}

export interface StartSessionMessage {
  type: "start-session";
  protocolVersion: typeof protocolVersion;
  sessionId: number;
  configuration: SessionConfiguration;
  d3d9Canvas?: OffscreenCanvas;
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

export interface PageLifecycleMessage {
  type: "page-lifecycle";
  protocolVersion: typeof protocolVersion;
  sessionId: number;
  lifecycle: PageLifecycleSnapshot;
}

export interface KeyboardInputMessage {
  type: "keyboard-input";
  protocolVersion: typeof protocolVersion;
  sessionId: number;
  event: RuntimeKeyboardInputEvent;
}

export type MainToWorkerMessage =
  | StartSessionMessage
  | CancelSessionMessage
  | HostCompletionMessage
  | PageLifecycleMessage
  | KeyboardInputMessage;

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

export interface RuntimeSupportedCallSnapshot {
  sequence: number;
  module: string;
  symbol: string;
  arguments: number[];
  result:
    | { action: "preserve" }
    | { action: "set"; value: number }
    | { kind: "h-result" | "u32"; value: number };
  implementationId?: string;
  returnAddress: number;
  stackPointerBefore: number;
  stackPointerAfter: number;
  lastError?: number;
  guestWrites: Array<{ address: number; bytes: number[] }>;
  executionBoundary?: "yield-current-thread";
  interface?: string;
  vtableSlot?: number;
  referenceCount?: number;
  objectRetired?: boolean;
  currentDirectory?: RuntimeCurrentDirectorySnapshot;
  mutex?: RuntimeMutexSnapshot;
  waitableTimer?: RuntimeWaitableTimerSnapshot;
  icon?: RuntimeIconSnapshot;
  cursor?: RuntimeCursorSnapshot;
  windowClass?: RuntimeWindowClassSnapshot;
  window?: RuntimeWindowSnapshot;
  paint?: RuntimePaintSnapshot;
  comApartment?: RuntimeComApartmentSnapshot;
  sourceRead?: RuntimeSourceReadSnapshot;
  writableCommit?: RuntimeWritableCommitSnapshot;
  writableDirectoryCreate?: RuntimeWritableDirectoryCreateSnapshot;
  gdi?: RuntimeGdiSnapshot;
  gdiplus?: RuntimeGdiplusSnapshot;
  imm32?: RuntimeImm32Snapshot;
  virtualMemory?: RuntimeVirtualMemorySnapshot;
  graphicsDevice?: RuntimeGraphicsDeviceSnapshot;
  graphicsTexture?: RuntimeGraphicsTextureSnapshot;
  graphicsTextureUpdate?: RuntimeGraphicsTextureUpdateSnapshot;
  browserPresentation?: RuntimeBrowserPresentationReceipt;
}

export interface RuntimeBrowserPresentationReceipt {
  backend: "webgl2-offscreen-canvas";
  resource: {
    id: `browser-resource:${number}`;
    kind: "d3d9-webgl2-device";
    binding: {
      objectId: string;
      interfaceId: string;
      interfaceName: "IDirect3DDevice9";
    };
  };
  browserResourceCount: number;
  deviceId: number;
  deviceGeneration: number;
  frameSequence: number;
  presentedContentGeneration: number;
  discardedBackBufferContentGeneration: number;
  width: number;
  height: number;
  drawSubmissionCount: number;
  uploadedTextureBytes: number;
  uploadedVertexBytes: number;
  centerPixelRgba: [number, number, number, number];
  webGlError: 0;
  capacityPreflightBeforeGuestExecution: true;
  bindingPublishedAfterSnapshotValidation: true;
  renderedBeforeGuestResume: true;
}

export interface SessionReport {
  evidenceLevel: "full" | "summary";
  outcome:
    | "completed"
    | "checkpoint-complete"
    | "structured-stop"
    | "cancelled"
    | "watchdog"
    | "fault";
  exitCode: number | null;
  elapsedMilliseconds: number;
  instructions: number;
  slices: number;
  budgetYields: number;
  hostRequests: number;
  rustSchedulerTransitions: number;
  timerPacing: {
    requests: number;
    completions: number;
    cancellations: number;
    freezes: number;
    resumes: number;
    staleCallbacks: number;
    boundedCatchUps: number;
    pending: number;
  };
  registeredThreadIds: number[];
  executedThreadIds: number[];
  contextSwitches: number;
  hostWaitExecutions: Array<{
    requestId: string;
    waitingThreadId: number;
    executedThreadId: number;
  }>;
  threadExits: Array<{
    threadId: number;
    exitCode: number;
    markerAddress?: number;
    markerValue?: number;
  }>;
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
  stackGrowths: Array<{
    instructionCount: number;
    faultAddress: number;
    pageFaultErrorCode: number;
    addressAllocationId: number;
    memoryOwner: number;
    committedPage: { start: number; byteLength: number };
    previousGuardPage: { start: number; byteLength: number };
    nextGuardPage?: { start: number; byteLength: number };
    committedPages: number;
    fromGeneration: number;
    generation: number;
  }>;
  heapPageMaterializations?: Array<{
    instructionCount: number;
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
    fromGeneration: number;
    generation: number;
  }>;
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
  initialCurrentDirectory: RuntimeCurrentDirectorySnapshot | null;
  finalGeneration: number;
  pendingHostCalls: number;
  pendingCallbacks: number;
  maximumSliceMilliseconds: number;
  wasmBridgeCalls: number;
  runtimeCoreLinearMemoryBytes: number;
  executionEngine: ExecutionEngineIdentity | null;
  executionEngineLinearMemoryBytes: number;
  supportedCallCount: number;
  supportedCallSemanticsAlgorithm: "sha256-canonical-json-v1";
  supportedCallSemanticsSha256: string | null;
  supportedCalls: RuntimeSupportedCallSnapshot[];
  keyboardInput: RuntimeKeyboardInputEvidence;
  sourceReads: RuntimeSourceReadSnapshot[];
  writableCommits: RuntimeWritableCommitSnapshot[];
  writableDirectoryCreates: RuntimeWritableDirectoryCreateSnapshot[];
  dragAcceptFiles: {
    call: {
      sequence: number;
      module: string;
      symbol: "DragAcceptFiles";
      occurrence: 1;
      trapInstructionCount: number;
      arguments: [number, number];
      result: { action: "preserve" };
      implementationId: "shell32.drag-accept-files.logical-window-style-v1";
      returnAddress: number;
      stackPointerBefore: number;
      stackPointerAfter: number;
      lastError: 0;
      guestWrites: [];
      window: RuntimeWindowSnapshot;
    };
    windowAuthority: "runtime-user32-logical-window";
    acceptanceAuthority: "effective-extended-style-ws-ex-acceptfiles";
    browserResourceCreated: false;
    wmDropFilesImplemented: false;
  } | null;
  sleepZero: {
    callCount: number;
    firstCall: RuntimeSleepZeroCallSnapshot;
    lastCall: RuntimeSleepZeroCallSnapshot;
    executionBoundary: "yield-current-thread";
    schedulerAuthority: "rust-runtime-scheduler";
    schedulerSelections: {
      selectionCount: number;
      firstSelection: RuntimeThreadSelectionSnapshot | null;
      lastSelection: RuntimeThreadSelectionSnapshot | null;
    };
    noCompetitorCompletions: number;
    hostWaitCreated: false;
  } | null;
  globalMemoryStatus: {
    call: RuntimeGlobalMemoryStatusCallSnapshot;
    stateAuthority: "kernel32-compatibility-policy";
    virtualSpanAuthority: "kernel32-system-info-policy";
    hostMemoryImported: false;
    browserMemoryImported: false;
    allocatorQuotaAuthority: false;
  } | null;
  getFileAttributesA: {
    callCount: number;
    firstCall: RuntimeGetFileAttributesACallSnapshot;
    lastCall: RuntimeGetFileAttributesACallSnapshot;
    pathAuthority: "rust-runtime-vfs";
    fileExistenceAuthority: "registered-source-and-writable-manifests";
    directoryExistence:
      | "registered-path-ancestor-derivation"
      | "registered-path-ancestors-and-writable-directory-state";
    hostFileSystemImported: false;
    browserFileSystemImported: false;
    mutation: false;
  } | null;
  createDirectoryA: {
    call: RuntimeCreateDirectoryACallSnapshot;
    directoryAuthority: "rust-runtime-vfs-writable-profile";
    durableAuthority: "sqlite-writable-profile-directory-generation";
    transaction: "validate-plan-host-publish-targeted-commit";
    sourceMutation: false;
    hostPathCreated: false;
    browserFileSystemImported: false;
  } | null;
  setWindowPosRepeatActivation: {
    callCount: 5;
    repeatCall: RuntimeSetWindowPosRepeatActivationCallSnapshot;
    callbackEvents: [
      RuntimeSetWindowPosRepeatActivationCallbackEvent,
      RuntimeSetWindowPosRepeatActivationCallbackEvent,
    ];
    windowAuthority: "rust-runtime-user32-logical-window";
    relationAuthority: "rust-runtime-user32-thread-and-process-window-relations";
    callbackDisposition: "validate-only";
    mutation: false;
    browserResourceCreated: false;
  } | null;
  setWindowPosFrameChangeMove: {
    callCount: 6;
    call: RuntimeSetWindowPosFrameChangeMoveCallSnapshot;
    callbackEvents: RuntimeSetWindowPosFrameChangeMoveCallbackEvent[];
    nestedMoveCallbackEvents: RuntimeSetWindowPosFrameChangeMoveCallbackEvent[];
    windowAuthority: "rust-runtime-user32-logical-window";
    relationAuthority: "rust-runtime-user32-thread-and-process-window-relations";
    geometryCommit: "nccalcsize-completion-targeted-commit";
    relationMutation: false;
    gdiResourceMutation: false;
    browserResourceCreated: false;
    nonClientPaintAndEraseModeled: false;
  } | null;
  showWindowNormal: {
    callCount: 1;
    call: RuntimeShowWindowNormalCallSnapshot;
    windowAuthority: "rust-runtime-user32-logical-window";
    relationAuthority: "rust-runtime-user32-thread-and-process-window-relations";
    mutation: false;
    callbackCount: 0;
    browserResourceCreated: false;
  } | null;
  updateWindow: {
    callCount: 1;
    call: RuntimeUpdateWindowEntrySnapshot;
    callbackEvents: RuntimeUpdateWindowCallbackEvent[];
    beginPaint: RuntimeBeginPaintCallSnapshot;
    endPaint: RuntimeEndPaintCallSnapshot;
    windowAuthority: "rust-runtime-user32-logical-window";
    updateRegionAuthority: "rust-runtime-user32-window-update-region";
    updateRegionSource: "set-window-pos-frame-change-nccalcsize-completion";
    updateRegionConsumption: "begin-paint-targeted-commit";
    paintSessionAuthority: "rust-runtime-user32-paint-session";
    deviceContextAuthority: "rust-runtime-gdi-device-context";
    paintSessionRetired: true;
    deviceContextRetired: true;
    failureAtomicity: true;
    callbackActiveAtStop: boolean;
    browserResourceCreated: false;
  } | null;
  postPresentContinuation: {
    timeGetTimeCall: RuntimeSupportedCallSnapshot;
    getForegroundWindowCall: RuntimeSupportedCallSnapshot;
    updateWindowCallbackReturned: true;
    foregroundWindowQueryCompleted: true;
    stateAuthority: "rust-runtime-user32-thread-and-process-window-relations";
    browserResourceCreated: false;
  } | null;
  getAsyncKeyState: {
    call: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "user32-single-monitor-window-frame-change-show-update-paint-input-v28";
    keyboardProfileId: "foreground-focused-virtual-key-state-v1";
    inputEvent: RuntimeKeyboardEventReceipt;
    stateAuthority: "rust-runtime-input-keyboard";
    browserSourceAuthority: "typescript-browser-input-adapter";
    returnEncoding: "signed-short-high-bit-down-low-bit-pressed-since-query";
    modifierAggregation: "side-specific-to-generic";
    pressedBitConsumption: "queried-virtual-key-only";
    sourceFocusRequired: true;
    focusedForegroundWindowRequired: true;
    lastErrorPolicy: "preserve";
    guestMemoryMutation: false;
    failureAtomicity: true;
  } | null;
  getWindowRect: {
    call: RuntimeSupportedCallSnapshot;
    windowAuthority: "rust-runtime-user32-logical-window";
    coordinateSpace: "logical-primary-screen-outer-window";
    browserGeometryImported: false;
    failureAtomicity: true;
  } | null;
  getCursorPos: {
    call: RuntimeSupportedCallSnapshot;
    cursorPosition: RuntimeCursorPositionSnapshot;
    stateAuthority: "rust-runtime-input-cursor-position";
    browserPointerImported: false;
    failureAtomicity: true;
  } | null;
  getAsyncKeyStateInitialScan: {
    leadingCallCount: number;
    scanVirtualKeys: number[];
    firstCall: RuntimeSupportedCallSnapshot;
    lastCall: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "user32-single-monitor-window-frame-change-show-update-paint-input-v28";
    cursorPosition: RuntimeCursorPositionSnapshot;
    cursorStateAuthority: "rust-runtime-input-cursor-position";
    keyboardProfileId: "foreground-focused-virtual-key-state-v1";
    inputEventCount: 0;
    stateAuthority: "rust-runtime-input-keyboard";
    browserSourceAuthority: "typescript-browser-input-adapter";
    returnEncoding: "signed-short-high-bit-down-low-bit-pressed-since-query";
    modifierAggregation: "side-specific-to-generic";
    pressedBitConsumption: "queried-virtual-key-only";
    sourceFocusRequired: true;
    focusedForegroundWindowRequired: true;
    lastErrorPolicy: "preserve";
    guestMemoryMutation: false;
    failureAtomicity: true;
  } | null;
  initialScanContinuation: {
    prerequisiteScanCompletionSequence: number;
    prerequisiteScanCompletionInstructionCount: number;
    calls: RuntimeSupportedCallSnapshot[];
    renderCompletionCounts: {
      clear: number;
      setRect: number;
      addDirtyRect: number;
      updateTexture: number;
      beginScene: number;
      samplerState: number;
      textureStageState: number;
      setTexture: number;
      setFvf: number;
      setStreamSource: number;
      drawPrimitive: number;
      endScene: number;
      present: number;
    };
    nextSubsystemGateway: "d3d9-texture-lock-rect";
  } | null;
  d3d9RecreatedTextures: {
    calls: RuntimeSupportedCallSnapshot[];
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    createTextureProfileId: "primary-initial-and-recreated-x8r8g8b8-textures-v2";
    getSurfaceLevelProfileId: "primary-system-memory-texture-level-zero-surfaces-v2";
    textureAuthority: "rust-runtime-graphics-logical-texture";
    comAuthority: "rust-runtime-com-object-and-interface";
    browserRendererResourceCreated: false;
    mutation: "cross-owner-preflight-transactional-commit";
  } | null;
  d3d9Clear: {
    call: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    clearProfileId: "primary-full-render-target-black-v1";
    targetAuthority: "rust-runtime-graphics-logical-device-back-buffer";
    targetScope: "full-presentation-back-buffer";
    mutation: "validate-plan-preflight-targeted-commit";
    failureAtomicity: true;
    depthStencilCleared: false;
    partialRectsImplemented: false;
    browserRendererResourceCreated: false;
  } | null;
  d3d9AddDirtyRect: {
    call: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    addDirtyRectProfileId: "primary-recreated-system-memory-texture-dirty-rect-v1";
    textureAuthority: "rust-runtime-graphics-logical-texture";
    initialDirtyRegion: "whole-texture";
    mutation: "validate-plan-preflight-idempotent-completion";
    textureGenerationChanged: false;
    failureAtomicity: true;
    dirtyRegionConsumed: false;
    browserRendererResourceCreated: false;
  } | null;
  d3d9UpdateTexture: {
    call: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    updateTextureProfileId: "primary-recreated-system-memory-to-static-default-texture-update-v1";
    contentAuthority: "rust-runtime-graphics-logical-texture";
    sourceDirtyRegionBefore: "whole-texture";
    sourceDirtyRegionAfter: "clean";
    dirtyRegionConsumed: true;
    transferScope: "whole-texture-level-zero-logical-content";
    mutation: "dual-texture-preflight-targeted-commit";
    failureAtomicity: true;
    partialDirtyRegionsImplemented: false;
    dynamicDestinationImplemented: false;
    browserRendererResourceCreated: false;
  } | null;
  d3d9BeginScene: {
    call: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    beginSceneProfileId: "primary-first-frame-scene-begin-v1";
    sceneAuthority: "rust-runtime-graphics-logical-device";
    lifecycle: "inactive-to-active";
    mutation: "validate-plan-preflight-targeted-commit";
    nestedBeginReturnsInvalidCall: true;
    failureAtomicity: true;
    endSceneImplemented: boolean;
    browserRendererResourceCreated: false;
  } | null;
  d3d9InitialSamplerStates: {
    calls: [RuntimeSupportedCallSnapshot, RuntimeSupportedCallSnapshot];
    postSamplerRenderStateRejections: RuntimeSupportedCallSnapshot[];
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    samplerStateProfileId: "primary-first-frame-linear-sampler-states-v1";
    stateAuthority: "rust-runtime-graphics-logical-device-sampler-states";
    mutation: "validate-plan-preflight-targeted-idempotent-commit";
    sameValueGenerationChange: 0;
    unsupportedVariantResult: "0x8876086a";
    failureAtomicity: true;
    browserRendererResourceCreated: false;
  } | null;
  d3d9InitialTextureStageStates: {
    calls: [
      RuntimeSupportedCallSnapshot,
      RuntimeSupportedCallSnapshot,
      RuntimeSupportedCallSnapshot,
      RuntimeSupportedCallSnapshot,
      RuntimeSupportedCallSnapshot,
      RuntimeSupportedCallSnapshot,
    ];
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    textureStageStateProfileId: "primary-first-frame-fixed-function-texture-stage-zero-v1";
    stateAuthority: "rust-runtime-graphics-logical-device-texture-stage-states";
    mutation: "validate-plan-preflight-targeted-idempotent-commit";
    sameValueGenerationChange: 0;
    unsupportedVariantResult: "0x8876086a";
    failureAtomicity: true;
    browserRendererResourceCreated: false;
  } | null;
  d3d9SetTexture: {
    call: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    textureBindingProfileId: "primary-first-frame-static-default-texture-stage-zero-v1";
    stateAuthority: "rust-runtime-graphics-logical-device-texture-bindings";
    bindingOwnership: "non-owning";
    mutation: "validate-plan-preflight-targeted-idempotent-commit";
    sameValueGenerationChange: 0;
    nullUnbindSupported: true;
    unsupportedVariantResult: "0x8876086a";
    failureAtomicity: true;
    browserRendererResourceCreated: false;
  } | null;
  d3d9SetFvf: {
    call: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    flexibleVertexFormatProfileId: "primary-first-frame-transformed-diffuse-textured-fvf-v1";
    stateAuthority: "rust-runtime-graphics-logical-device-flexible-vertex-format";
    mutation: "validate-plan-preflight-targeted-idempotent-commit";
    sameValueGenerationChange: 0;
    unsupportedVariantResult: "0x8876086a";
    failureAtomicity: true;
    browserRendererResourceCreated: false;
  } | null;
  d3d9SetStreamSource: {
    call: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    streamSourceProfileId: "primary-first-frame-transformed-diffuse-textured-stream-zero-v1";
    stateAuthority: "rust-runtime-graphics-logical-device-stream-sources";
    bindingOwnership: "non-owning";
    mutation: "validate-plan-preflight-targeted-idempotent-commit";
    sameValueGenerationChange: 0;
    supportedStream: 0;
    supportedOffsetInBytes: 0;
    supportedStride: 28;
    nullUnbindImplemented: false;
    unsupportedVariantResult: "0x8876086a";
    failureAtomicity: true;
    browserRendererResourceCreated: false;
  } | null;
  d3d9DrawPrimitive: {
    call: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    drawPrimitiveProfileId: "primary-first-frame-triangle-strip-quad-draw-v1";
    stateAuthority: "rust-runtime-graphics-logical-device-back-buffer-draw-submissions";
    vertexByteAuthority: "rust-guest-memory-synchronous-immutable-snapshot";
    mutation: "validate-plan-read-preflight-targeted-commit";
    supportedPrimitiveTopology: "triangle-strip";
    supportedStartVertex: 0;
    supportedPrimitiveCount: 2;
    capturedVertexBytes: 112;
    pendingSubmissionLimit: 4096;
    pendingVertexBytesLimit: 1048576;
    clearDiscardsPendingSubmissions: true;
    failureAtomicity: true;
    browserRendererResourceCreated: false;
    visiblePixelsProduced: false;
  } | null;
  d3d9EndScene: {
    call: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    endSceneProfileId: "primary-first-frame-scene-end-v1";
    sceneAuthority: "rust-runtime-graphics-logical-device-active-scene";
    lifecycle: "active-to-inactive";
    mutation: "validate-plan-preflight-targeted-commit";
    preservesDrawSubmissions: true;
    repeatedEndReturnsInvalidCall: true;
    failureAtomicity: true;
    browserRendererResourceCreated: false;
    visiblePixelsProduced: false;
  } | null;
  d3d9Present: {
    call: RuntimeSupportedCallSnapshot;
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    presentProfileId: "primary-first-frame-full-back-buffer-present-v1";
    frameAuthority: "rust-runtime-graphics-logical-device-presented-frame";
    browserResourceAuthority: "typescript-browser-resource-table";
    mutation: "validate-plan-preflight-targeted-commit";
    presentMode: "discard";
    sourceRectImplemented: false;
    destinationRectImplemented: false;
    destinationWindowOverrideImplemented: false;
    dirtyRegionImplemented: false;
    browserRendererResourceCreated: true;
    browserCapacityPreflightBeforeGuestExecution: true;
    browserBindingPublishedAfterSnapshotValidation: true;
    visiblePixelsProduced: true;
    asynchronousDeviceLossRecoveryImplemented: false;
    failureAtomicity: true;
  } | null;
  d3d9PostEndSceneRasterStatus: {
    call: RuntimeSupportedCallSnapshot;
    cumulativeCompletions: number;
    compatibilityPolicyId: "d3d9-primary-virtual-adapter-v30";
    rasterStatusProfileId: "primary-stable-last-visible-scanline-v2";
    displayProfileId: "single-monitor-1920x1080-work-1920x1032-v2";
    stateAuthority: "rust-d3d9-compatibility-policy";
    outputBytes: 8;
    inVBlank: false;
    scanLine: 1079;
    runtimeStateMutation: false;
    referenceCountMutation: false;
    lastErrorPolicy: "preserve";
    guestWriteAtomic: true;
    hostScanoutImported: false;
    browserScanoutImported: false;
    browserRendererResourceCreated: false;
    visiblePixelsProduced: false;
  } | null;
  setRect: {
    call: RuntimeSupportedCallSnapshot;
    implementationId: "user32.set-rect.v1";
    outputType: "RECT";
    coordinateEncoding: "signed-long32";
    lastErrorPolicy: "preserve";
    mutation: "guest-memory-preflight-atomic-write";
    nullOutputReturnsFalse: true;
    browserOwnsRectState: false;
  } | null;
  firstUnsupported: {
    capabilityCluster: string;
    endpoint: unknown;
    fault: unknown;
    instructionCount: number;
    memoryContentSha256: string;
  } | null;
  checkpointCompletion:
    | {
        capability: "kernel32-wait-for-single-object-ex";
        implementationId: "kernel32.wait-for-single-object-ex.waitable-timer.v1";
        requiredCompletions: number;
        completionCount: number;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "d3d9-end-scene";
        implementationId: "d3d9.end-scene.logical-scene-close.v1";
        requiredCompletions: 1;
        completionCount: 1;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "d3d9-post-end-scene-raster-status";
        implementationId: "d3d9.get-raster-status.deterministic-sample.v1";
        requiredCompletions: 1;
        completionCount: 1;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "d3d9-post-scene-present-gateway";
        implementationId: "d3d9.get-raster-status.last-visible-sample.v2";
        requiredCompletions: 1;
        completionCount: 1;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "d3d9-first-present";
        implementationId: "d3d9.present.discard-snapshot.v1";
        requiredCompletions: 1;
        completionCount: 1;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "user32-async-key-state-input";
        implementationId: "user32.get-async-key-state.v1";
        requiredCompletions: 1;
        completionCount: 1;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        virtualKey: 0x11;
        result: { action: "set"; value: 0xffff_8001 };
        lastError: 0;
        inputEventSequence: 1;
        inputVirtualKey: 0xa2;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "user32-async-key-state-input-continuation";
        implementationId: "user32.get-async-key-state.v1";
        requiredCompletions: 2;
        completionCount: 2;
        firstCompletionSequence: number;
        firstCompletionInstructionCount: number;
        firstResult: { action: "set"; value: 0xffff_8001 };
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        virtualKey: 0x11;
        result: { action: "set"; value: 0xffff_8000 };
        lastError: 0;
        inputEventSequence: 1;
        inputVirtualKey: 0xa2;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "user32-async-key-state-initial-scan";
        implementationId: "user32.get-async-key-state.v1";
        requiredCompletions: 38;
        completionCount: 38;
        leadingCallCount: number;
        firstCompletionSequence: number;
        firstCompletionInstructionCount: number;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        scanVirtualKeys: number[];
        result: { action: "set"; value: 0 };
        lastError: 0;
        guestWriteBytes: 0;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "d3d9-post-raster-status-waitable-timer";
        implementationId: "kernel32.wait-for-single-object-ex.waitable-timer.v1";
        requiredCompletions: 1;
        completionCount: 1;
        armSequence: number;
        armInstructionCount: number;
        armGeneration: number;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "kernel32-enter-critical-section";
        implementationId: "kernel32.enter-critical-section.v1";
        requiredCompletions: 3;
        completionCount: 3;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        roundtrips: Array<{
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
        }>;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "primary-main-loop-progression";
        implementationId: "runtime.scheduler.api-boundary-round-robin.v1";
        requiredCompletions: 2;
        completionCount: 2;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        threadSelections: RuntimeThreadSelectionSnapshot[];
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "kernel32-leave-critical-section";
        implementationId: "kernel32.leave-critical-section.v1";
        requiredCompletions: 1;
        completionCount: 1;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        criticalSectionAddress: number;
        releaseThreadId: number;
        lastError: number;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "kernel32-enter-critical-section-continuation";
        implementationId: "kernel32.enter-critical-section.v1";
        requiredCompletions: 1;
        completionCount: 1;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        criticalSectionAddress: number;
        acquisitionThreadId: number;
        lastError: number;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | {
        capability: "kernel32-leave-critical-section-continuation";
        implementationId: "kernel32.leave-critical-section.v1";
        requiredCompletions: 1;
        completionCount: 1;
        lastCompletionSequence: number;
        lastCompletionInstructionCount: number;
        criticalSectionAddress: number;
        releaseThreadId: number;
        lastError: number;
        instructionCount: number;
        nextEndpoint: {
          id: number;
          address: number;
          module?: string;
          symbol?:
            | { kind: "name"; name: string }
            | { kind: "ordinal"; ordinal: number };
        };
        memoryContentSha256: string;
      }
    | null;
  error: string | null;
}

export interface RuntimeSourceReadSnapshot {
  request: {
    requestId: number;
    fileId: number;
    sourceId: number;
    objectId: number;
    offset: number;
    requestedBytes: number;
  };
  succeeded: boolean;
  transferredBytes: number;
  win32Error: number;
  cursorBefore: number;
  cursorAfter: number;
  contentSha256: string;
}

export interface RuntimeWritableCommitSnapshot {
  request: {
    requestId: number;
    fileId: number;
    objectId: number;
    baseGeneration: number;
    byteLength: number;
    contentSha256: string;
    closeAfterCommit: boolean;
  };
  succeeded: boolean;
  committedGeneration: number;
  win32Error: number;
}

export interface RuntimeWritableDirectoryCreateSnapshot {
  request: {
    requestId: number;
    directoryId: number;
    relativePathA: number[];
    baseGeneration: number;
  };
  succeeded: boolean;
  committedGeneration: number;
  win32Error: number;
}

export type RuntimeGdiplusSnapshot =
  | {
      action: "session-started";
      policyId: string;
      session: {
        id: number;
        tokenNamespace: string;
        tokenId: number;
        token: number;
        startup: {
          profileId: string;
          version: number;
          debugEventCallback: number;
          suppressBackgroundThread: boolean;
          suppressExternalCodecs: boolean;
          notificationOutputRequested: boolean;
        };
        generation: number;
      };
      liveSessions: number;
      browserResourceCreated: false;
      nativeTokenImported: false;
    }
  | {
      action: "session-shutdown";
      token: number;
      liveSessions: number;
    };

export interface RuntimeImeWindowSnapshot {
  id: number;
  handle: number;
  namespace: string;
  ownerThreadId: number;
  className: "IME";
  statusWindowRequest?: "close" | "open";
  generation: number;
}

export interface RuntimeImeContextSnapshot {
  id: number;
  handle: number;
  namespace: string;
  ownerThreadId: number;
  defaultForThread: true;
  generation: number;
}

export interface RuntimeImeContextAssociationSnapshot {
  windowId: number;
  windowHandle: number;
  ownerThreadId: number;
  context?: { contextId: number; handle: number };
  generation: number;
}

export type RuntimeImm32Snapshot =
  | {
      action: "default-window-resolved";
      policyId: string;
      appWindowHandle: number;
      imeWindow: RuntimeImeWindowSnapshot;
      association: RuntimeImeContextAssociationSnapshot;
      context: RuntimeImeContextSnapshot | null;
      nativeHandleImported: false;
      browserResourceCreated: false;
    }
  | {
      action: "status-window-control-requested";
      policyId: string;
      imeWindow: RuntimeImeWindowSnapshot;
      nativeMessageForwarded: false;
      browserResourceCreated: false;
    }
  | {
      action: "context-associated";
      policyId: string;
      appWindowHandle: number;
      requestedContextHandle: number;
      previousContextHandle: number;
      association: RuntimeImeContextAssociationSnapshot;
      context: RuntimeImeContextSnapshot | null;
      nativeHandleImported: false;
      browserResourceCreated: false;
    };

export interface RuntimeVirtualMemorySnapshot {
  allocation: {
    id: number;
    parentId: number | null;
    purpose: { kind: "virtual-memory" };
    owner: string;
    range: { base: number; byteLength: number };
    usableByteLength: number;
    alignment: number;
    guardPages: number;
    classification: "virtual-memory";
    lifetime: "process";
    releasePolicy: "explicit";
    executable: false;
  };
  region: {
    addressAllocationId: number;
    owner: number;
    kind: "anonymous";
    range: { start: number; byteLength: number };
  };
  preferredAddress: number;
  allocationType: number;
  protection: number;
  reserveOnly: true;
  topDown: true;
}

export interface RuntimeGdiDeviceContextSnapshot {
  id: number;
  handle: number;
  namespace: string;
  windowId: number;
  windowHandle: number;
  ownerThreadId: number;
  surfaceId: number;
}

export interface RuntimeGdiSurfaceSnapshot {
  id: number;
  windowId: number;
  width: number;
  height: number;
  fills: Array<{
    sequence: number;
    rect: { left: number; top: number; right: number; bottom: number };
    brushId: number;
    colorRef: number;
  }>;
  generation: number;
}

export interface RuntimeGdiScreenDeviceContextSnapshot {
  id: number;
  handle: number;
  namespace: string;
  ownerThreadId: number;
  targetKind: "screen";
  monitorHandle: number;
  rect: { left: number; top: number; right: number; bottom: number };
}

export interface RuntimeGdiMemoryDeviceContextSnapshot {
  id: number;
  handle: number;
  namespace: string;
  ownerThreadId: number;
  targetKind: "memory";
  compatibleMonitorHandle: number;
  compatibleRect: { left: number; top: number; right: number; bottom: number };
  selectedFontId: number;
  selectedBitmapId: number;
  selectionGeneration: number;
}

export interface RuntimeGdiStockFontSnapshot {
  id: number;
  handle: number;
  namespace: string;
  shared: true;
  object: { kind: "stock-font"; index: number };
}

export interface RuntimeGdiStockBitmapSnapshot {
  id: number;
  handle: number;
  namespace: string;
  shared: true;
  object: {
    kind: "stock-bitmap";
    index: number;
    width: number;
    height: number;
    bitsPerPixel: number;
  };
}

export interface RuntimeGdiFontSnapshot {
  id: number;
  handle: number;
  namespace: string;
  shared: false;
  object: {
    kind: "font-a";
    logFont: {
      height: number;
      width: number;
      escapement: number;
      orientation: number;
      weight: number;
      italic: number;
      underline: number;
      strikeOut: number;
      characterSet: number;
      outputPrecision: number;
      clipPrecision: number;
      quality: number;
      pitchAndFamily: number;
      faceNameWindows932: number[];
    };
  };
}

export interface RuntimeGdiDibSectionSnapshot {
  id: number;
  handle: number;
  namespace: string;
  shared: false;
  object: {
    kind: "dib-section";
    descriptor: {
      width: number;
      height: number;
      planes: number;
      bitsPerPixel: number;
      compression: number;
      strideBytes: number;
      pixelByteLength: number;
      palette: Array<{
        blue: number;
        green: number;
        red: number;
        reserved: number;
      }>;
    };
    backing: {
      addressAllocationId: number;
      memoryOwner: number;
      range: { start: number; byteLength: number };
      contentGeneration: number;
    };
  };
}

export type RuntimeGdiSelectableObjectSnapshot =
  | RuntimeGdiStockFontSnapshot
  | RuntimeGdiStockBitmapSnapshot
  | RuntimeGdiFontSnapshot
  | RuntimeGdiDibSectionSnapshot;

export type RuntimeGdiSnapshot =
  | {
      action: "device-context-acquired";
      deviceContext: RuntimeGdiDeviceContextSnapshot;
      surface: RuntimeGdiSurfaceSnapshot;
      liveDeviceContexts: number;
    }
  | {
      action: "screen-device-context-acquired";
      deviceContext: RuntimeGdiScreenDeviceContextSnapshot;
      liveDeviceContexts: number;
    }
  | {
      action: "font-enumeration-completed";
      deviceContext: RuntimeGdiScreenDeviceContextSnapshot;
      enumeration: {
        profileId: string;
        inputCharacterSet: number;
        inputFaceNameWindows932: number[];
        flags: number;
        record: {
          fontType: number;
          enumLogFontExA: number[];
          newTextMetricExA: number[];
        };
      };
      callbackResult: number;
      liveDeviceContexts: number;
    }
  | {
      action: "font-created";
      object: RuntimeGdiFontSnapshot;
      liveObjects: number;
    }
  | {
      action: "compatible-device-context-created";
      deviceContext: RuntimeGdiMemoryDeviceContextSnapshot;
      initialFont: RuntimeGdiStockFontSnapshot;
      initialBitmap: RuntimeGdiStockBitmapSnapshot;
      liveDeviceContexts: number;
      liveObjects: number;
    }
  | {
      action: "dib-section-created";
      object: RuntimeGdiDibSectionSnapshot;
      addressAllocation: {
        id: number;
        parentId: number;
        purpose: { kind: "gdi-resource" };
        owner: string;
        range: { base: number; byteLength: number };
        usableByteLength: number;
        alignment: number;
        guardPages: number;
        classification: "graphics";
        lifetime: "object";
        releasePolicy: "explicit";
        executable: false;
      };
      liveObjects: number;
    }
  | {
      action: "dib-section-create-rejected";
      liveObjects: number;
    }
  | {
      action: "object-bytes-read";
      handle: number;
      byteLength: number;
      object: RuntimeGdiSelectableObjectSnapshot | null;
      liveObjects: number;
    }
  | {
      action: "outline-text-metrics-read";
      deviceContext: RuntimeGdiMemoryDeviceContextSnapshot;
      selectedFont: RuntimeGdiFontSnapshot;
      profileId: string;
      requestedByteLength: number;
      returnedByteLength: number;
      outputAddress: number;
      writtenByteLength: number;
    }
  | {
      action: "text-raster-written";
      deviceContext: RuntimeGdiMemoryDeviceContextSnapshot;
      selectedFont: RuntimeGdiFontSnapshot;
      selectedDibSection: RuntimeGdiDibSectionSnapshot;
      profileId: string;
      x: number;
      y: number;
      textAddress: number;
      characterCount: number;
      writeAddress: number | null;
      writtenByteLength: number;
      changedByteCount: number;
      contentGeneration: number;
      expectedOutputFnv1a32: number;
    }
  | {
      action: "object-selected";
      deviceContext: RuntimeGdiMemoryDeviceContextSnapshot;
      selectedObject: RuntimeGdiSelectableObjectSnapshot | null;
      previousObject: RuntimeGdiSelectableObjectSnapshot | null;
      succeeded: boolean;
      liveObjects: number;
    }
  | {
      action: "object-deleted";
      handle: number;
      deleted: boolean;
      liveObjects: number;
    }
  | {
      action: "compatible-device-context-deleted";
      handle: number;
      deleted: boolean;
      liveDeviceContexts: number;
      liveObjects: number;
    }
  | {
      action: "stock-object-resolved";
      object: {
        id: number;
        handle: number;
        namespace: string;
        shared: boolean;
        object: { kind: "stock-brush"; index: number; colorRef: number };
      };
      liveObjects: number;
    }
  | {
      action: "surface-filled";
      deviceContext: RuntimeGdiDeviceContextSnapshot;
      surface: RuntimeGdiSurfaceSnapshot;
    }
  | {
      action: "device-context-released";
      handle: number;
      liveDeviceContexts: number;
      liveSurfaces: number;
    };

export interface SessionFinishedMessage {
  type: "session-finished";
  protocolVersion: typeof protocolVersion;
  sessionId: number;
  report: SessionReport;
}

export type WorkerToMainMessage =
  SessionStatusMessage | HostRequestMessage | SessionFinishedMessage;

export function assertProtocolVersion(
  version: number,
): asserts version is typeof protocolVersion {
  if (version !== protocolVersion) {
    throw new Error(`unsupported browser runtime protocol version ${version}`);
  }
}
