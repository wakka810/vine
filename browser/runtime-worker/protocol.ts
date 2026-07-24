import type { ExecutionState, ExecutionTransition } from "./execution-state.js";
import type { ExecutionEngineIdentity } from "./execution-engine-contract.js";

export const protocolVersion = 11 as const;

export type SessionMode = "success" | "cancel-pending" | "watchdog";
export type SessionWorkload = "controlled-fixture" | "controlled-source-read" | "private-bgi";

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
  };
  objectRecord: {
    id: number;
    key: { kind: "waitable-timer"; timer_id: number };
    kind: "waitable-timer";
    namespace: string;
    closePolicy: "close-handle";
    waitState: { state: "nonsignaled" };
    openHandles: number;
  };
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
    | { state: "creating"; message: "non-client-create" | "non-client-calculate" | "create" | "size" | "move" }
    | { state: "created" };
  visible: boolean;
  topmost: boolean;
}

export interface RuntimeComApartmentSnapshot {
  model: "single-threaded" | "multi-threaded";
  initializationCount: number;
}

export interface SessionConfiguration {
  mode: SessionMode;
  evidenceLevel: "full" | "summary";
  workload?: SessionWorkload;
  executableUrl?: string;
  policyUrl?: string;
  sourceManifestUrl?: string;
  sourceApiUrl?: string;
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
  evidenceLevel: "full" | "summary";
  outcome: "completed" | "structured-stop" | "cancelled" | "watchdog" | "fault";
  exitCode: number | null;
  elapsedMilliseconds: number;
  instructions: number;
  slices: number;
  budgetYields: number;
  hostRequests: number;
  rustSchedulerTransitions: number;
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
  supportedCalls: Array<{
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
    comApartment?: RuntimeComApartmentSnapshot;
    sourceRead?: RuntimeSourceReadSnapshot;
    gdi?: RuntimeGdiSnapshot;
    gdiplus?: RuntimeGdiplusSnapshot;
    virtualMemory?: RuntimeVirtualMemorySnapshot;
  }>;
  sourceReads: RuntimeSourceReadSnapshot[];
  firstUnsupported: {
    capabilityCluster: string;
    endpoint: unknown;
    fault: unknown;
    instructionCount: number;
    memoryContentSha256: string;
  } | null;
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
  | SessionStatusMessage
  | HostRequestMessage
  | SessionFinishedMessage;

export function assertProtocolVersion(version: number): asserts version is typeof protocolVersion {
  if (version !== protocolVersion) {
    throw new Error(`unsupported browser runtime protocol version ${version}`);
  }
}
