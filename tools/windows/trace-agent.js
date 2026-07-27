"use strict";

const schema = "vine.win32-trace.v2";
const agentOptions = globalThis.__vineTraceOptions ?? {
  vtables: [],
  observations: [],
};
const selectedVtables = new Set(agentOptions.vtables ?? []);
const selectedObservations = new Set(agentOptions.observations ?? []);
const focusedWindowDcObservation =
  selectedObservations.size === 1 && selectedObservations.has("window-dc");
const focusedScreenDcObservation =
  selectedObservations.size === 1 && selectedObservations.has("screen-dc");
const focusedFontObjectObservation =
  selectedObservations.size === 1 && selectedObservations.has("font-object");
const focusedGdiplusStartupObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("gdiplus-startup");
const focusedVirtualAllocObservation =
  selectedObservations.size === 1 && selectedObservations.has("virtual-alloc");
const focusedImeContextObservation =
  selectedObservations.size === 1 && selectedObservations.has("ime-context");
const focusedSetWindowLongAObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("set-window-long-a");
const focusedSetWindowPosSizeObservation =
  selectedObservations.size === 1 &&
  (selectedObservations.has("set-window-pos-size") ||
    selectedObservations.has("set-window-pos-resize"));
const focusedSetWindowPosFrameChangeMoveObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("set-window-pos-frame-change-move");
const focusedShowWindowNormalObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("show-window-normal");
const focusedUpdateWindowObservation =
  selectedObservations.size === 1 && selectedObservations.has("update-window");
const focusedSetWindowPosStateObservation =
  focusedSetWindowPosSizeObservation ||
  focusedSetWindowPosFrameChangeMoveObservation;
const focusedDragAcceptFilesObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("drag-accept-files");
const focusedSetTimerObservation =
  selectedObservations.size === 1 && selectedObservations.has("set-timer");
const focusedCreateThreadObservation =
  selectedObservations.size === 1 && selectedObservations.has("create-thread");
const focusedSetThreadPriorityObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("set-thread-priority");
const focusedGuestThreadObservation =
  focusedCreateThreadObservation || focusedSetThreadPriorityObservation;
const focusedD3d9CreateDeviceObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("d3d9-create-device");
const focusedD3d9GetSurfaceLevelObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("d3d9-get-surface-level");
const focusedD3d9SurfaceGetDcObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("d3d9-surface-get-dc");
const focusedD3d9InitialTexturesObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("d3d9-initial-textures");
const focusedD3d9RecreatedTexturesObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("d3d9-recreated-textures");
const focusedD3d9InitialVertexBufferObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("d3d9-initial-vertex-buffer");
const focusedD3d9InitialRenderStatesObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("d3d9-initial-render-states");
const focusedD3d9RasterStatusObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("d3d9-raster-status");
const focusedDinput8RootObservation =
  selectedObservations.size === 1 && selectedObservations.has("dinput8-root");
const focusedDinput8EnumDevicesObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("dinput8-enum-devices");
const focusedDinput8Observation =
  focusedDinput8RootObservation || focusedDinput8EnumDevicesObservation;
const focusedDsoundRootObservation =
  selectedObservations.size === 1 && selectedObservations.has("dsound-root");
const focusedDsoundCreateBufferObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("dsound-create-sound-buffer");
const focusedDsoundBufferLockUnlockPlayObservation =
  selectedObservations.size === 1 &&
  selectedObservations.has("dsound-buffer-lock-unlock-play");
const focusedDsoundObservation =
  focusedDsoundRootObservation ||
  focusedDsoundCreateBufferObservation ||
  focusedDsoundBufferLockUnlockPlayObservation;
const focusedD3d9ResourceObservation =
  focusedD3d9GetSurfaceLevelObservation || focusedD3d9SurfaceGetDcObservation;
const focusedD3d9DeviceObservation =
  focusedD3d9CreateDeviceObservation ||
  focusedD3d9ResourceObservation ||
  focusedD3d9InitialTexturesObservation ||
  focusedD3d9RecreatedTexturesObservation ||
  focusedD3d9InitialVertexBufferObservation ||
  focusedD3d9InitialRenderStatesObservation ||
  focusedD3d9RasterStatusObservation;
const stopOnEvent = agentOptions.stopOnEvent ?? null;
let sequence = 0;
let nextCallId = 1;
let focusedD3d9DeviceMethodCaptured = false;
let focusedD3d9TextureMethodCaptured = false;
let focusedD3d9SurfaceMethodCaptured = false;
let focusedD3d9SurfaceObject = null;
let focusedD3d9SurfaceDcComplete = false;
const focusedD3d9SurfaceDcCalls = [];
let focusedD3d9InitialTextureDevice = null;
let focusedD3d9InitialTexturesComplete = false;
const focusedD3d9InitialTextureCalls = [];
let focusedD3d9RecreatedTextureDevice = null;
let focusedD3d9RecreatedTexturesStarted = false;
let focusedD3d9RecreatedTexturesComplete = false;
const focusedD3d9RecreatedTextureCalls = [];
let focusedD3d9InitialVertexBufferDevice = null;
let focusedD3d9InitialVertexBufferObject = null;
let focusedD3d9InitialVertexBufferData = null;
let focusedD3d9InitialVertexBufferLength = null;
let focusedD3d9InitialVertexBufferComplete = false;
const focusedD3d9InitialVertexBufferCalls = [];
let focusedD3d9InitialRenderStateDevice = null;
let focusedD3d9InitialRenderStatesComplete = false;
const focusedD3d9InitialRenderStateCalls = [];
let focusedD3d9RasterStatusDevice = null;
let focusedD3d9RasterStatusComplete = false;
const focusedD3d9RasterStatusCalls = [];
let focusedDinput8RootObject = null;
let focusedDinput8RootComplete = false;
let focusedDinput8FactoryCall = null;
const focusedDinput8RootCalls = [];
const activeDinput8EnumDevicesByThread = new Map();
const attachedDinput8EnumDevicesCallbacks = new Set();
let focusedDsoundRootObject = null;
let focusedDsoundRootComplete = false;
let focusedDsoundFactoryCall = null;
const focusedDsoundRootCalls = [];
let focusedDsoundCreateBufferComplete = false;
let focusedDsoundBufferLockUnlockPlayComplete = false;
const focusedDsoundCreateBufferCalls = [];
const focusedDsoundBufferObjects = new Set();
const focusedDsoundBufferCalls = [];
let focusedDsoundActiveLock = null;
const attached = new Set();
const attachedVtableMethods = new Set();
const observedVtables = new Set();
const activeCallsByThread = new Map();
let criticalSectionOracleScheduled = false;
let criticalSectionOracleSentinel = null;
let criticalSectionOracleLastError = null;
let initializeCriticalSectionConsumersRecorded = 0;
let commandLineOracleScheduled = false;
const commandLineFirstPointers = new Map();
const commandLineLatestPointers = new Map();
let environmentStringsOracleScheduled = false;
let environmentStringsOracleSentinel = null;
let environmentStringsOracleLastError = null;
let wideCharOracleScheduled = false;
let wideCharOracleSentinel = null;
let wideCharOracleLastError = null;
let multiByteOracleScheduled = false;
let multiByteOracleSentinel = null;
let multiByteOracleLastError = null;
const multiByteConsumerKeys = new Set();
let setCurrentDirectoryOracleScheduled = false;
let setCurrentDirectoryOracleSentinel = null;
let setCurrentDirectoryOracleResult = null;
let setCurrentDirectoryOracleLastError = null;
let setCurrentDirectoryConsumersRecorded = 0;
let createMutexOracleScheduled = false;
let createMutexOracleSentinel = null;
let createMutexOracleResult = null;
let createMutexOracleLastError = null;
let createMutexConsumersRecorded = 0;
let mutexLifecycleConsumersRecorded = 0;
const trackedMutexHandles = new Map();
let getStringTypeOracleScheduled = false;
let getStringTypeOracleSentinel = null;
let getStringTypeOracleResult = null;
let getStringTypeOracleLastError = null;
let getStringTypeConsumerRecorded = false;
let lcMapStringOracleScheduled = false;
let lcMapStringOracleSentinel = null;
let lcMapStringOracleResult = null;
let lcMapStringOracleLastError = null;
let lcMapStringConsumersRecorded = 0;
let interlockedDecrementOracleScheduled = false;
let interlockedDecrementOracleSentinel = null;
let interlockedDecrementOracleResult = null;
let interlockedDecrementOracleLastError = null;
let interlockedDecrementConsumersRecorded = 0;
let getModuleFileNameAOracleScheduled = false;
let getModuleFileNameAOracleSentinel = null;
let getModuleFileNameAOracleResult = null;
let getModuleFileNameAOracleLastError = null;
let getModuleFileNameAConsumersRecorded = 0;
let tlsLifecycleOracleScheduled = false;
let tlsGetValueConsumerRecorded = false;
let flsLifecycleOracleScheduled = false;
let flsGetValueConsumerRecorded = false;
let getLastErrorOracleScheduled = false;
let getLastErrorOracleSentinel = null;
let getLastErrorOracleResult = null;
let getLastErrorOracleLastError = null;
let setLastErrorOracleScheduled = false;
let setLastErrorOracleActive = false;
let setLastErrorOracleArgument = null;
let setLastErrorOracleEaxBefore = null;
let setLastErrorOracleEaxAfter = null;
let setLastErrorOracleLastErrorAfter = null;
let setLastErrorConsumerRecorded = false;
let getAcpOracleScheduled = false;
let getAcpOracleSentinel = null;
let getAcpOracleResult = null;
let getAcpOracleLastError = null;
let getAcpConsumerRecorded = false;
let systemDefaultLangIdOracleScheduled = false;
let systemDefaultLangIdOracleSentinel = null;
let systemDefaultLangIdOracleResult = null;
let systemDefaultLangIdOracleLastError = null;
let systemDefaultLangIdConsumerRecorded = false;
let driveTypeOracleScheduled = false;
let driveTypeOracleSentinel = null;
let driveTypeOracleResult = null;
let driveTypeOracleLastError = null;
let driveTypeConsumerRecorded = false;
let driveTypeScanCompletionRecorded = false;
let driveTypeMainModuleCallCount = 0;
let swapMouseButtonStateScheduled = false;
let swapMouseButtonInitialState = null;
let swapMouseButtonGetSystemMetrics = null;
let swapMouseButtonRestore = null;
let swapMouseButtonRestoreActive = false;
let swapMouseButtonConsumerRecorded = false;
let swapMouseButtonCompletionRecorded = false;
const swapMouseButtonMainModuleCalls = [];
let dragAcceptFilesCompletion = null;
let dragAcceptFilesCompletionRecorded = false;
let timeGetDevCapsOracleScheduled = false;
let timeGetDevCapsOracleCall = null;
let timePeriodOracleCall = null;
let timeGetDevCapsOracleComplete = false;
let timeGetDevCapsOracleEvidence = null;
let timePeriodOracleEvidence = null;
let timeGetDevCapsConsumerRecorded = false;
let timeGetDevCapsCompletionRecorded = false;
let timeGetDevCapsDrainCheckScheduled = false;
const timeGetDevCapsMainModuleCalls = [];
const timePeriodMainModuleCalls = [];
let timeGetTimeOracleScheduled = false;
let timeGetTimeOracleCall = null;
let timeGetTimeOracleComplete = false;
let timeGetTimeOracleEvidence = null;
let timeGetTimeCompletionRecorded = false;
const timeGetTimeMainModuleCalls = [];
let queryPerformanceFrequencyOracleScheduled = false;
let queryPerformanceFrequencyOracleCall = null;
let queryPerformanceFrequencyOracleComplete = false;
let queryPerformanceFrequencyOracleEvidence = null;
let queryPerformanceFrequencyCompletionRecorded = false;
const queryPerformanceFrequencyMainModuleCalls = [];
let queryPerformanceCounterOracleScheduled = false;
let queryPerformanceCounterOracleCall = null;
let queryPerformanceCounterOracleComplete = false;
let queryPerformanceCounterOracleEvidence = null;
let queryPerformanceCounterCompletionRecorded = false;
const queryPerformanceCounterMainModuleCalls = [];
let getVersionExAOracleScheduled = false;
let getVersionExAOracleCall = null;
let getVersionExAOracleComplete = false;
let getVersionExAOracleEvidence = null;
let getVersionExACompletionRecorded = false;
let getVersionExADrainCheckScheduled = false;
const getVersionExAMainModuleCalls = [];
let createWaitableTimerAOracleScheduled = false;
let createWaitableTimerAOracleCall = null;
let createWaitableTimerAOracleComplete = false;
let createWaitableTimerAOracleEvidence = null;
let createWaitableTimerACompletionRecorded = false;
let createWaitableTimerADrainCheckScheduled = false;
const createWaitableTimerAMainModuleCalls = [];
let setWaitableTimerOracleScheduled = false;
let setWaitableTimerOracleCall = null;
let setWaitableTimerOracleComplete = false;
let setWaitableTimerOracleEvidence = null;
let setWaitableTimerCompletionRecorded = false;
let setWaitableTimerDrainCheckScheduled = false;
let setWaitableTimerMainCallClaimed = false;
const setWaitableTimerMainModuleCalls = [];
let waitForSingleObjectExOracleScheduled = false;
let waitForSingleObjectExOracleCall = null;
let waitForSingleObjectExOracleComplete = false;
let waitForSingleObjectExOracleEvidence = null;
let waitForSingleObjectExCompletionRecorded = false;
let waitForSingleObjectExDrainCheckScheduled = false;
let waitForSingleObjectExMainCallClaimed = false;
const waitForSingleObjectExMainModuleCalls = [];
let setTimerCompletionRecorded = false;
let setTimerDrainCheckScheduled = false;
const setTimerMainModuleCalls = [];
const setTimerCallbackCalls = [];
const killTimerMainModuleCalls = [];
const attachedTimerProcedures = new Set();
let createThreadCompletionRecorded = false;
let createThreadDrainCheckScheduled = false;
const createThreadMainModuleCalls = [];
const createThreadLifecycleCalls = [];
const createThreadStartCalls = [];
const trackedCreateThreadHandles = new Map();
const attachedThreadProcedures = new Set();
let loadIconAOracleScheduled = false;
let loadIconAOracleCall = null;
let loadIconAOracleComplete = false;
let loadIconAOracleEvidence = null;
let loadIconACompletionRecorded = false;
let loadIconADrainCheckScheduled = false;
const loadIconAMainModuleCalls = [];
let loadImageAOracleScheduled = false;
let loadImageAOracleCall = null;
let loadImageAOracleComplete = false;
let loadImageAOracleEvidence = null;
let loadImageACompletionRecorded = false;
let loadImageADrainCheckScheduled = false;
const loadImageAMainModuleCalls = [];
let sendMessageACompletionRecorded = false;
let sendMessageADrainCheckScheduled = false;
const sendMessageAMainModuleCalls = [];
const sendMessageAWindowProcedureCalls = [];
let setWindowPosCompletionRecorded = false;
const setWindowPosMainModuleCalls = [];
const setWindowPosWindowProcedureCalls = [];
const setWindowPosDefaultProcedureCalls = [];
const activeSetWindowPosCallsByThread = new Map();
let setWindowPosWindowStateSnapshot = null;
let setWindowPosStateOracleScheduled = false;
let setWindowPosStateOracleComplete = false;
let setWindowPosStateOracleEvidence = null;
let showWindowNormalCompletionRecorded = false;
const showWindowNormalMainModuleCalls = [];
const showWindowNormalWindowProcedureCalls = [];
const activeShowWindowNormalCallsByThread = new Map();
let showWindowNormalOracleScheduled = false;
let showWindowNormalOracleComplete = false;
let showWindowNormalOracleEvidence = null;
let updateWindowCompletionRecorded = false;
const updateWindowMainModuleCalls = [];
const updateWindowWindowProcedureCalls = [];
const updateWindowPaintCalls = [];
const activeUpdateWindowCallsByThread = new Map();
let updateWindowOracleScheduled = false;
let updateWindowOracleComplete = false;
let updateWindowOracleEvidence = null;
let loadCursorAOracleScheduled = false;
let loadCursorAOracleCall = null;
let loadCursorAOracleComplete = false;
let loadCursorAOracleEvidence = null;
let loadCursorACompletionRecorded = false;
let loadCursorADrainCheckScheduled = false;
const loadCursorAMainModuleCalls = [];
let registerClassExAOracleScheduled = false;
let registerClassExAOracleCall = null;
let registerClassExAOracleComplete = false;
let registerClassExAOracleEvidence = null;
let registerClassExACompletionRecorded = false;
let registerClassExADrainCheckScheduled = false;
const registerClassExAMainModuleCalls = [];
let createWindowExACompletionRecorded = false;
let createWindowExADrainCheckScheduled = false;
const createWindowExAMainModuleCalls = [];
const createWindowClassRegistrations = new Map();
const activeCreateWindowCallsByThread = new Map();
const attachedCreateWindowClassDiscovery = new Set();
const attachedCreateWindowProcedures = new Set();
let getSystemMetricsOracleScheduled = false;
let getSystemMetricsOracleCall = null;
let getSystemMetricsOracleComplete = false;
let getSystemMetricsOracleEvidence = null;
let getSystemMetricsCompletionRecorded = false;
const getSystemMetricsMainModuleCalls = [];
let getClientRectOracleScheduled = false;
let getClientRectOracleCall = null;
let getClientRectOracleComplete = false;
let getClientRectOracleEvidence = null;
let getClientRectCompletionRecorded = false;
const getClientRectMainModuleCalls = [];
let setRectOracleScheduled = false;
let setRectOracleCall = null;
let setRectOracleComplete = false;
let setRectOracleEvidence = null;
let setRectCompletionRecorded = false;
const setRectMainModuleCalls = [];
let getMonitorInfoAOracleScheduled = false;
let getMonitorInfoAOracleCall = null;
let getMonitorInfoAOracleComplete = false;
let getMonitorInfoAOracleEvidence = null;
let getMonitorInfoACompletionRecorded = false;
let getMonitorInfoAQuiescentNextCallId = null;
const getMonitorInfoAMainModuleCalls = [];
let windowDcOracleScheduled = false;
let windowDcOracleCall = null;
let windowDcOracleComplete = false;
let windowDcOracleEvidence = null;
let windowDcCompletionRecorded = false;
const windowDcMainModuleCalls = [];
let screenDcOracleScheduled = false;
let screenDcOracleCall = null;
let screenDcOracleComplete = false;
let screenDcOracleEvidence = null;
let screenDcCompletionRecorded = false;
let screenDcTargetHandle = null;
const screenDcMainModuleCalls = [];
const activeScreenDcFontEnumerationsByThread = new Map();
const attachedScreenDcFontCallbacks = new Set();
const screenDcFontOracleCallbacks = [];
let fontObjectOracleScheduled = false;
let fontObjectOracleCall = null;
let fontObjectOracleComplete = false;
let fontObjectOracleEvidence = null;
let fontObjectCompletionRecorded = false;
let fontObjectActivityGeneration = 0;
let fontObjectPrimaryStarted = false;
const fontObjectMainModuleCalls = [];
const fontObjectPrimaryHandles = new Set();
const fontObjectSelectedDcHandles = new Map();
const fontObjectDibSectionsByHandle = new Map();
const fontObjectSelectedBitmapByDc = new Map();
let gdiplusStartupCompletionRecorded = false;
let gdiplusStartupActivityGeneration = 0;
let virtualAllocCompletionRecorded = false;
let virtualAllocActivityGeneration = 0;
const virtualAllocMainModuleCalls = [];
let virtualQueryFunction = null;
let imeContextCompletionRecorded = false;
let imeContextDrainCheckScheduled = false;
let imeContextOracleScheduled = false;
let imeContextOracleComplete = false;
let imeContextOracleEvidence = null;
const imeGetDefaultWindowCalls = [];
const imeAssociateContextCalls = [];
const imeControlMessageCalls = [];
let imeWindowInspector = null;
const gdiplusMainModuleCalls = [];
let setWindowLongAOracleScheduled = false;
let setWindowLongAOracleCall = null;
let setWindowLongAOracleComplete = false;
let setWindowLongAOracleEvidence = null;
let setWindowLongACompletionRecorded = false;
const setWindowLongAMainModuleCalls = [];
let getCpInfoOracleScheduled = false;
let getCpInfoOracleSentinel = null;
let getCpInfoOracleResult = null;
let getCpInfoOracleLastError = null;
let getCpInfoConsumerRecorded = false;
let processorFeatureOracleScheduled = false;
let processorFeatureOracleSentinel = null;
let processorFeatureOracleResult = null;
let processorFeatureOracleLastError = null;
let processorFeatureConsumersRecorded = 0;
let unhandledExceptionFilterOracleScheduled = false;
let unhandledExceptionFilterOracleSentinel = null;
let unhandledExceptionFilterOracleResult = null;
let unhandledExceptionFilterOracleLastError = null;
let unhandledExceptionFilterConsumersRecorded = 0;
const unhandledExceptionFilterCallbacks = [];
let heapSizeOracleScheduled = false;
let heapSizeOracleSentinel = null;
let heapSizeOracleResult = null;
let heapSizeOracleLastError = null;
let heapSizeConsumersRecorded = 0;
const observedMainModuleHeapAllocations = new Map();
const largeHeapAccessMinimumBytes = 16 * 1024 * 1024;
let activeLargeHeapAccessMonitor = null;
let largeHeapAllocationCompletionCandidate = null;
let largeHeapAllocationCompletionRecorded = false;
const observedSourceFileHandles = new Map();
let sourceFileReadCompletionRecorded = false;
let getFileAttributesAOracleScheduled = false;
let getFileAttributesAOracleCall = null;
let getFileAttributesAConsumerRecorded = false;
let createDirectoryAOracleScheduled = false;
let createDirectoryAOracleCall = null;
let createDirectoryAConsumerRecorded = false;
let getCurrentThreadOracleScheduled = false;
let setThreadAffinityMaskOracleScheduled = false;
let setThreadAffinityMaskOracleSentinel = null;
let setThreadAffinityMaskOracleResult = null;
let setThreadAffinityMaskOracleLastError = null;
let setThreadAffinityMaskConsumersRecorded = 0;
let setThreadAffinityMaskPrimaryCalls = 0;
let setThreadAffinityCalibration = null;
let getSystemInfoOracleScheduled = false;
let getSystemInfoOracleSentinel = null;
let getSystemInfoOracleLastError = null;
let getSystemInfoConsumerRecorded = false;
let globalMemoryStatusOracleScheduled = false;
let globalMemoryStatusOracleSentinel = null;
let globalMemoryStatusOracleLastError = null;
let globalMemoryStatusConsumerRecorded = false;
let enumDisplayMonitorsOracleScheduled = false;
let enumDisplayMonitorsOracleSentinel = null;
let enumDisplayMonitorsOracleResult = null;
let enumDisplayMonitorsOracleLastError = null;
let enumDisplayMonitorsConsumerRecorded = false;
const enumDisplayMonitorsOracleCallbacks = [];
const activeEnumDisplayMonitorsByThread = new Map();
const attachedEnumDisplayMonitorCallbacks = new Set();
let comInitializationOracleScheduled = false;
let comInitializationOracleCall = null;
let comInitializationConsumerRecorded = false;

function snapshotNulTerminatedString(pointer, encoding) {
  const maximumBytes = 4096;
  const unitBytes = encoding === "utf16" ? 2 : 1;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      encoding,
      maximumBytes,
      error: "null pointer",
    };
  }
  try {
    let byteLengthWithNul = null;
    for (
      let offset = 0;
      offset + unitBytes <= maximumBytes;
      offset += unitBytes
    ) {
      const terminator =
        unitBytes === 1
          ? pointer.add(offset).readU8() === 0
          : pointer.add(offset).readU16() === 0;
      if (terminator) {
        byteLengthWithNul = offset + unitBytes;
        break;
      }
    }
    if (byteLengthWithNul === null) {
      return {
        pointer: pointer.toString(),
        encoding,
        maximumBytes,
        error: "terminator not found within bounded observation",
      };
    }
    const buffer = pointer.readByteArray(byteLengthWithNul);
    const bytes = [...new Uint8Array(buffer)];
    const api = encoding === "utf16" ? "GetCommandLineW" : "GetCommandLineA";
    const firstPointer =
      commandLineFirstPointers.get(api) ?? pointer.toString();
    commandLineFirstPointers.set(api, firstPointer);
    commandLineLatestPointers.set(api, pointer.toString());
    const otherApi =
      api === "GetCommandLineA" ? "GetCommandLineW" : "GetCommandLineA";
    const otherPointer = commandLineLatestPointers.get(otherApi) ?? null;
    return {
      pointer: pointer.toString(),
      encoding,
      maximumBytes,
      byteLengthWithNul,
      bytes,
      string: readString(pointer, encoding),
      equalsFirstPointer: pointer.toString() === firstPointer,
      otherApi,
      otherPointer,
      equalsOtherEncodingPointer:
        otherPointer === null ? null : pointer.toString() === otherPointer,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      encoding,
      maximumBytes,
      error: error.message,
    };
  }
}

function snapshotOptionalU32(pointer) {
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), isNull: true };
  }
  try {
    return {
      pointer: pointer.toString(),
      isNull: false,
      value: pointer.readU32(),
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      isNull: false,
      error: error.message,
    };
  }
}

function compactPageRuns(pageIndexes, requestedBytes) {
  const pageSize = Process.pageSize;
  const sorted = [...pageIndexes].sort((left, right) => left - right);
  const runs = [];
  let first = null;
  let last = null;
  const append = () => {
    if (first === null || last === null) {
      return;
    }
    runs.push({
      firstPage: first,
      lastPage: last,
      pageCount: last - first + 1,
      startOffset: first * pageSize,
      endOffsetExclusive: Math.min(requestedBytes, (last + 1) * pageSize),
    });
  };
  for (const pageIndex of sorted) {
    if (first === null) {
      first = pageIndex;
      last = pageIndex;
    } else if (pageIndex === last + 1) {
      last = pageIndex;
    } else {
      append();
      first = pageIndex;
      last = pageIndex;
    }
  }
  append();
  return runs;
}

function describeLargeHeapAccess(access) {
  if (access === null) {
    return null;
  }
  return {
    operation: access.operation,
    allocationOffset: access.allocationOffset,
    pageIndex: access.pageIndex,
    address: access.address,
    source: describeCaller(ptr(access.source)),
    sourceInstructions: snapshotInstructions(ptr(access.source), 8),
    timestampMs: access.timestampMs,
  };
}

function beginLargeHeapAccessObservation(
  pointer,
  requestedBytes,
  allocationCallId,
) {
  if (requestedBytes < largeHeapAccessMinimumBytes) {
    return null;
  }
  if (activeLargeHeapAccessMonitor !== null) {
    emit("large-heap-access-monitor-error", {
      error: "a second large allocation overlapped the active observation",
      activeAllocationCallId: activeLargeHeapAccessMonitor.allocationCallId,
      overlappingAllocationCallId: allocationCallId,
    });
    return {
      enabled: false,
      error: "large allocation monitor already active",
    };
  }
  if (typeof MemoryAccessMonitor === "undefined") {
    emit("large-heap-access-monitor-error", {
      error: "MemoryAccessMonitor is unavailable",
      allocationCallId,
    });
    return {
      enabled: false,
      error: "MemoryAccessMonitor is unavailable",
    };
  }
  const pageSize = Process.pageSize;
  const pointerValue = pointer.toUInt32();
  const end = pointerValue + requestedBytes;
  if (!Number.isSafeInteger(end) || end > 0x1_0000_0000) {
    emit("large-heap-access-monitor-error", {
      error: "large allocation range crosses the x86 address limit",
      allocationCallId,
      pointer: pointer.toString(),
      requestedBytes,
    });
    return {
      enabled: false,
      error: "large allocation range is invalid",
    };
  }
  const monitoredBase = Math.floor(pointerValue / pageSize) * pageSize;
  const monitoredEnd = Math.ceil(end / pageSize) * pageSize;
  const state = {
    allocationCallId,
    pointer: pointer.toString().toLowerCase(),
    pointerValue,
    requestedBytes,
    requestedPages: Math.ceil(requestedBytes / pageSize),
    monitoredBase,
    monitoredBytes: monitoredEnd - monitoredBase,
    startedAtMs: Date.now(),
    pages: new Map(),
    operationCounts: { read: 0, write: 0, execute: 0, other: 0 },
    firstAccess: null,
    lastAccess: null,
  };
  activeLargeHeapAccessMonitor = state;
  try {
    MemoryAccessMonitor.enable(
      [{ base: ptr(monitoredBase), size: state.monitoredBytes }],
      {
        onAccess(details) {
          const address = details.address.toUInt32();
          if (address < pointerValue || address >= end) {
            return;
          }
          const allocationOffset = address - pointerValue;
          const pageIndex = Math.floor(allocationOffset / pageSize);
          if (state.pages.has(pageIndex)) {
            return;
          }
          const operation = String(details.operation);
          const operationKey = Object.prototype.hasOwnProperty.call(
            state.operationCounts,
            operation,
          )
            ? operation
            : "other";
          const access = {
            operation,
            allocationOffset,
            pageIndex,
            address: details.address.toString(),
            source: details.from.toString(),
            timestampMs: Date.now(),
          };
          state.pages.set(pageIndex, access);
          state.operationCounts[operationKey] += 1;
          state.firstAccess ??= access;
          state.lastAccess = access;
        },
      },
    );
    return {
      enabled: true,
      allocationCallId,
      pointer: state.pointer,
      requestedBytes,
      requestedPages: state.requestedPages,
      monitoredBase: ptr(monitoredBase).toString(),
      monitoredBytes: state.monitoredBytes,
    };
  } catch (error) {
    activeLargeHeapAccessMonitor = null;
    emit("large-heap-access-monitor-error", {
      error: error.message,
      allocationCallId,
      pointer: pointer.toString(),
      requestedBytes,
    });
    return {
      enabled: false,
      error: error.message,
    };
  }
}

function finishLargeHeapAccessObservation(pointer, reason) {
  const state = activeLargeHeapAccessMonitor;
  if (state === null || state.pointer !== pointer.toLowerCase()) {
    return null;
  }
  MemoryAccessMonitor.disable();
  activeLargeHeapAccessMonitor = null;
  const pageIndexes = new Set(state.pages.keys());
  return {
    enabled: true,
    reason,
    allocationCallId: state.allocationCallId,
    pointer: state.pointer,
    requestedBytes: state.requestedBytes,
    requestedPages: state.requestedPages,
    monitoredBase: ptr(state.monitoredBase).toString(),
    monitoredBytes: state.monitoredBytes,
    lifetimeMilliseconds: Math.max(0, Date.now() - state.startedAtMs),
    accessedPages: pageIndexes.size,
    untouchedPages: state.requestedPages - pageIndexes.size,
    firstAccessOperationCounts: state.operationCounts,
    accessedPageRuns: compactPageRuns(pageIndexes, state.requestedBytes),
    firstAccess: describeLargeHeapAccess(state.firstAccess),
    lastAccess: describeLargeHeapAccess(state.lastAccess),
  };
}

function maybeCompleteLargeHeapAllocationObservation() {
  if (
    largeHeapAllocationCompletionRecorded ||
    largeHeapAllocationCompletionCandidate === null
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteLargeHeapAllocationObservation, 10);
    return;
  }
  largeHeapAllocationCompletionRecorded = true;
  emit(
    "large-heap-allocation-complete",
    largeHeapAllocationCompletionCandidate,
  );
  if (stopOnEvent === "large-heap-allocation-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function fnv1aByte(hash, byte) {
  return Math.imul((hash ^ byte) >>> 0, 0x01000193) >>> 0;
}

function signed32(value) {
  const unsigned = value.toUInt32();
  return unsigned > 0x7fffffff ? unsigned - 0x100000000 : unsigned;
}

function snapshotWideCharSource(pointer, requestedUnits) {
  const maximumUnits = 512 * 1024;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      error: "null pointer",
    };
  }
  if (requestedUnits === 0 || requestedUnits < -1) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      error: "invalid UTF-16 unit count",
    };
  }
  try {
    let observedUnits = requestedUnits;
    if (requestedUnits === -1) {
      observedUnits = null;
      for (let index = 0; index < maximumUnits; index += 1) {
        if (pointer.add(index * 2).readU16() === 0) {
          observedUnits = index + 1;
          break;
        }
      }
      if (observedUnits === null) {
        return {
          pointer: pointer.toString(),
          requestedUnits,
          maximumUnits,
          error: "UTF-16 NUL not found within bounded observation",
        };
      }
    }
    if (observedUnits > maximumUnits) {
      return {
        pointer: pointer.toString(),
        requestedUnits,
        observedUnits,
        maximumUnits,
        error: "UTF-16 input exceeds bounded observation",
      };
    }
    let hash = 0x811c9dc5;
    let nulUnits = 0;
    let asciiUnits = 0;
    let nonAsciiBmpUnits = 0;
    let highSurrogates = 0;
    let lowSurrogates = 0;
    let lastUnit = null;
    for (let index = 0; index < observedUnits; index += 1) {
      const unit = pointer.add(index * 2).readU16();
      hash = fnv1aByte(hash, unit & 0xff);
      hash = fnv1aByte(hash, unit >>> 8);
      lastUnit = unit;
      if (unit === 0) {
        nulUnits += 1;
      } else if (unit <= 0x7f) {
        asciiUnits += 1;
      } else if (unit >= 0xd800 && unit <= 0xdbff) {
        highSurrogates += 1;
      } else if (unit >= 0xdc00 && unit <= 0xdfff) {
        lowSurrogates += 1;
      } else {
        nonAsciiBmpUnits += 1;
      }
    }
    return {
      pointer: pointer.toString(),
      requestedUnits,
      observedUnits,
      maximumUnits,
      byteLength: observedUnits * 2,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      nulUnits,
      asciiUnits,
      nonAsciiBmpUnits,
      highSurrogates,
      lowSurrogates,
      endsWithNul: lastUnit === 0,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      error: error.message,
    };
  }
}

function snapshotMultiByteSource(pointer, requestedBytes) {
  const maximumBytes = 1024 * 1024;
  const maximumRecordedBytes = 4096;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "null pointer",
    };
  }
  if (requestedBytes === 0 || requestedBytes < -1) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "invalid multibyte byte count",
    };
  }
  try {
    let observedBytes = requestedBytes;
    if (requestedBytes === -1) {
      observedBytes = null;
      for (let index = 0; index < maximumBytes; index += 1) {
        if (pointer.add(index).readU8() === 0) {
          observedBytes = index + 1;
          break;
        }
      }
      if (observedBytes === null) {
        return {
          pointer: pointer.toString(),
          requestedBytes,
          maximumBytes,
          error: "multibyte NUL not found within bounded observation",
        };
      }
    }
    if (observedBytes > maximumBytes) {
      return {
        pointer: pointer.toString(),
        requestedBytes,
        observedBytes,
        maximumBytes,
        error: "multibyte input exceeds bounded observation",
      };
    }
    const buffer = pointer.readByteArray(observedBytes);
    const bytes = [...new Uint8Array(buffer)];
    let hash = 0x811c9dc5;
    let nulBytes = 0;
    let asciiBytes = 0;
    for (const byte of bytes) {
      hash = fnv1aByte(hash, byte);
      if (byte === 0) {
        nulBytes += 1;
      }
      if (byte <= 0x7f) {
        asciiBytes += 1;
      }
    }
    return {
      pointer: pointer.toString(),
      requestedBytes,
      observedBytes,
      maximumBytes,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      nulBytes,
      asciiBytes,
      bytes:
        observedBytes <= maximumRecordedBytes
          ? bytes
          : bytes.slice(0, maximumRecordedBytes),
      bytesTruncated: observedBytes > maximumRecordedBytes,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: error.message,
    };
  }
}

function snapshotCurrentDirectoryA(module) {
  const capacity = 32 * 1024;
  try {
    const address = findExport(module, "GetCurrentDirectoryA");
    if (address === null) {
      return { capacity, error: "GetCurrentDirectoryA export is unavailable" };
    }
    const getCurrentDirectoryA = new SystemFunction(
      address,
      "uint32",
      ["uint32", "pointer"],
      "stdcall",
    );
    const buffer = Memory.alloc(capacity);
    buffer.writeByteArray(Array(capacity).fill(0xa5));
    const outcome = getCurrentDirectoryA(capacity, buffer);
    const result = Number(outcome.value) >>> 0;
    const systemLastError = Number(outcome.lastError) >>> 0;
    return {
      capacity,
      result,
      systemLastError,
      path:
        result > 0 && result < capacity
          ? snapshotMultiByteSource(buffer, -1)
          : null,
    };
  } catch (error) {
    return { capacity, error: error.message };
  }
}

function snapshotByteBuffer(pointer, requestedBytes) {
  const maximumBytes = 1024 * 1024;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      isNull: true,
    };
  }
  if (requestedBytes < 0 || requestedBytes > maximumBytes) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "byte buffer length is outside the bounded observation",
    };
  }
  try {
    const buffer = pointer.readByteArray(requestedBytes);
    const bytes = new Uint8Array(buffer);
    let hash = 0x811c9dc5;
    let nulBytes = 0;
    for (const byte of bytes) {
      hash = fnv1aByte(hash, byte);
      if (byte === 0) {
        nulBytes += 1;
      }
    }
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      isNull: false,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      nulBytes,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: error.message,
    };
  }
}

function snapshotWideCharBuffer(pointer, requestedUnits) {
  const maximumUnits = 512 * 1024;
  const maximumRecordedUnits = 4096;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      isNull: true,
    };
  }
  if (requestedUnits < 0 || requestedUnits > maximumUnits) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      error: "UTF-16 buffer length is outside the bounded observation",
    };
  }
  try {
    const units = Array.from({ length: requestedUnits }, (_, index) =>
      pointer.add(index * 2).readU16(),
    );
    let hash = 0x811c9dc5;
    let nulUnits = 0;
    for (const unit of units) {
      hash = fnv1aByte(hash, unit & 0xff);
      hash = fnv1aByte(hash, unit >>> 8);
      if (unit === 0) {
        nulUnits += 1;
      }
    }
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      isNull: false,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      nulUnits,
      units:
        requestedUnits <= maximumRecordedUnits
          ? units
          : units.slice(0, maximumRecordedUnits),
      unitsTruncated: requestedUnits > maximumRecordedUnits,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedUnits,
      maximumUnits,
      error: error.message,
    };
  }
}

function snapshotWordBuffer(pointer, requestedWords) {
  const maximumWords = 512 * 1024;
  const maximumRecordedWords = 4096;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedWords,
      maximumWords,
      isNull: true,
    };
  }
  if (requestedWords < 0 || requestedWords > maximumWords) {
    return {
      pointer: pointer.toString(),
      requestedWords,
      maximumWords,
      error: "WORD buffer length is outside the bounded observation",
    };
  }
  try {
    const words = Array.from({ length: requestedWords }, (_, index) =>
      pointer.add(index * 2).readU16(),
    );
    let hash = 0x811c9dc5;
    let zeroWords = 0;
    let bitwiseOr = 0;
    let bitwiseAnd = 0xffff;
    for (const word of words) {
      hash = fnv1aByte(hash, word & 0xff);
      hash = fnv1aByte(hash, word >>> 8);
      if (word === 0) {
        zeroWords += 1;
      }
      bitwiseOr |= word;
      bitwiseAnd &= word;
    }
    return {
      pointer: pointer.toString(),
      requestedWords,
      maximumWords,
      isNull: false,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      zeroWords,
      bitwiseOr,
      bitwiseAnd,
      words:
        requestedWords <= maximumRecordedWords
          ? words
          : words.slice(0, maximumRecordedWords),
      wordsTruncated: requestedWords > maximumRecordedWords,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedWords,
      maximumWords,
      error: error.message,
    };
  }
}

function snapshotBoundedRawBytes(pointer, requestedBytes, maximumBytes = 64) {
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "null pointer",
    };
  }
  if (
    !Number.isInteger(requestedBytes) ||
    requestedBytes < 0 ||
    requestedBytes > maximumBytes
  ) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "raw byte length is outside the bounded observation",
    };
  }
  try {
    const buffer = pointer.readByteArray(requestedBytes);
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      bytes: [...new Uint8Array(buffer)],
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: error.message,
    };
  }
}

function snapshotMeasuredCpInfo(pointer) {
  const measuredBytes = 18;
  const raw = snapshotBoundedRawBytes(pointer, measuredBytes);
  if (raw.error !== undefined) {
    return {
      profile: "measured-x86-cpinfo-fields-v1",
      measuredBytes,
      raw,
    };
  }
  try {
    const leadByte = Array.from({ length: 12 }, (_, index) =>
      pointer.add(6 + index).readU8(),
    );
    const leadBytePairs = Array.from({ length: 6 }, (_, index) => [
      leadByte[index * 2],
      leadByte[index * 2 + 1],
    ]);
    return {
      profile: "measured-x86-cpinfo-fields-v1",
      measuredBytes,
      raw,
      fields: {
        maxCharSize: pointer.readU32(),
        defaultChar: [pointer.add(4).readU8(), pointer.add(5).readU8()],
        leadByte,
        leadBytePairs,
      },
    };
  } catch (error) {
    return {
      profile: "measured-x86-cpinfo-fields-v1",
      measuredBytes,
      raw,
      error: error.message,
    };
  }
}

function snapshotSystemInfo(pointer) {
  const measuredBytes = 36;
  const raw = snapshotBoundedRawBytes(pointer, measuredBytes);
  if (raw.error !== undefined) {
    return {
      profile: "x86-system-info-layout-v1",
      measuredBytes,
      raw,
    };
  }
  if (Process.pointerSize !== 4) {
    return {
      profile: "x86-system-info-layout-v1",
      measuredBytes,
      raw,
      error: `x86 SYSTEM_INFO observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  try {
    return {
      profile: "x86-system-info-layout-v1",
      measuredBytes,
      raw,
      fields: {
        processorArchitecture: pointer.readU16(),
        reserved: pointer.add(2).readU16(),
        pageSize: pointer.add(4).readU32(),
        minimumApplicationAddress: pointer.add(8).readU32(),
        maximumApplicationAddress: pointer.add(12).readU32(),
        activeProcessorMask: pointer.add(16).readU32(),
        numberOfProcessors: pointer.add(20).readU32(),
        processorType: pointer.add(24).readU32(),
        allocationGranularity: pointer.add(28).readU32(),
        processorLevel: pointer.add(32).readU16(),
        processorRevision: pointer.add(34).readU16(),
      },
    };
  } catch (error) {
    return {
      profile: "x86-system-info-layout-v1",
      measuredBytes,
      raw,
      error: error.message,
    };
  }
}

function snapshotMemoryStatus(pointer) {
  const measuredBytes = 32;
  const raw = snapshotBoundedRawBytes(pointer, measuredBytes);
  if (raw.error !== undefined) {
    return {
      profile: "x86-memory-status-layout-v1",
      measuredBytes,
      raw,
    };
  }
  if (Process.pointerSize !== 4) {
    return {
      profile: "x86-memory-status-layout-v1",
      measuredBytes,
      raw,
      error: `x86 MEMORYSTATUS observation requires 4-byte fields, got ${Process.pointerSize}-byte pointers`,
    };
  }
  try {
    return {
      profile: "x86-memory-status-layout-v1",
      measuredBytes,
      raw,
      fields: {
        length: pointer.readU32(),
        memoryLoadPercent: pointer.add(4).readU32(),
        totalPhysicalBytes: pointer.add(8).readU32(),
        availablePhysicalBytes: pointer.add(12).readU32(),
        totalPageFileBytes: pointer.add(16).readU32(),
        availablePageFileBytes: pointer.add(20).readU32(),
        totalVirtualBytes: pointer.add(24).readU32(),
        availableVirtualBytes: pointer.add(28).readU32(),
      },
    };
  } catch (error) {
    return {
      profile: "x86-memory-status-layout-v1",
      measuredBytes,
      raw,
      error: error.message,
    };
  }
}

function snapshotTimeCaps(pointer) {
  const measuredBytes = 8;
  const raw = snapshotBoundedRawBytes(pointer, measuredBytes);
  if (raw.error !== undefined) {
    return {
      profile: "x86-timecaps-layout-v1",
      measuredBytes,
      raw,
    };
  }
  try {
    return {
      profile: "x86-timecaps-layout-v1",
      measuredBytes,
      raw,
      fields: {
        periodMinimumMilliseconds: pointer.readU32(),
        periodMaximumMilliseconds: pointer.add(4).readU32(),
      },
    };
  } catch (error) {
    return {
      profile: "x86-timecaps-layout-v1",
      measuredBytes,
      raw,
      error: error.message,
    };
  }
}

function snapshotOsVersionInfoA(pointer) {
  const profile = "x86-os-version-info-a-layout-v1";
  const baseBytes = 148;
  const extendedBytes = 156;
  if (pointer.isNull()) {
    return {
      profile,
      baseBytes,
      extendedBytes,
      error: "null pointer",
    };
  }
  try {
    const declaredBytes = pointer.readU32();
    const measuredBytes =
      declaredBytes >= extendedBytes
        ? extendedBytes
        : declaredBytes >= baseBytes
          ? baseBytes
          : 4;
    const raw = snapshotBoundedRawBytes(pointer, measuredBytes, extendedBytes);
    if (declaredBytes < baseBytes || raw.error !== undefined) {
      return {
        profile,
        baseBytes,
        extendedBytes,
        declaredBytes,
        measuredBytes,
        raw,
      };
    }
    const servicePackBytes = raw.bytes.slice(20, baseBytes);
    const terminator = servicePackBytes.indexOf(0);
    const servicePackBytesWithoutNul =
      terminator < 0 ? servicePackBytes : servicePackBytes.slice(0, terminator);
    return {
      profile,
      baseBytes,
      extendedBytes,
      declaredBytes,
      measuredBytes,
      raw,
      fields: {
        majorVersion: pointer.add(4).readU32(),
        minorVersion: pointer.add(8).readU32(),
        buildNumber: pointer.add(12).readU32(),
        platformId: pointer.add(16).readU32(),
        servicePackBytes: servicePackBytesWithoutNul,
        servicePackTerminated: terminator >= 0,
        ...(declaredBytes < extendedBytes
          ? {}
          : {
              servicePackMajor: pointer.add(148).readU16(),
              servicePackMinor: pointer.add(150).readU16(),
              suiteMask: pointer.add(152).readU16(),
              productType: pointer.add(154).readU8(),
              reserved: pointer.add(155).readU8(),
            }),
      },
    };
  } catch (error) {
    return {
      profile,
      baseBytes,
      extendedBytes,
      error: error.message,
    };
  }
}

function snapshotRect(pointer) {
  const raw = snapshotBoundedRawBytes(pointer, 16);
  if (raw.error !== undefined) {
    return { profile: "x86-rect-v1", raw };
  }
  try {
    return {
      profile: "x86-rect-v1",
      raw,
      fields: {
        left: pointer.readS32(),
        top: pointer.add(4).readS32(),
        right: pointer.add(8).readS32(),
        bottom: pointer.add(12).readS32(),
      },
    };
  } catch (error) {
    return { profile: "x86-rect-v1", raw, error: error.message };
  }
}

function snapshotPaintStruct(pointer) {
  const raw = snapshotBoundedRawBytes(pointer, 64, 64);
  if (raw.error !== undefined) {
    return { profile: "x86-paintstruct-v1", raw };
  }
  try {
    return {
      profile: "x86-paintstruct-v1",
      raw,
      fields: {
        deviceContext: pointer.readPointer().toString(),
        erase: pointer.add(4).readU32(),
        paintRect: {
          left: pointer.add(8).readS32(),
          top: pointer.add(12).readS32(),
          right: pointer.add(16).readS32(),
          bottom: pointer.add(20).readS32(),
        },
        restore: pointer.add(24).readU32(),
        incrementalUpdate: pointer.add(28).readU32(),
        reserved: [...new Uint8Array(pointer.add(32).readByteArray(32))],
      },
    };
  } catch (error) {
    return {
      profile: "x86-paintstruct-v1",
      raw,
      error: error.message,
    };
  }
}

function attachEnumDisplayMonitorCallback(callback) {
  if (callback.isNull()) {
    return;
  }
  const key = callback.toString().toLowerCase();
  if (attachedEnumDisplayMonitorCallbacks.has(key)) {
    return;
  }
  attachedEnumDisplayMonitorCallbacks.add(key);
  Interceptor.attach(callback, {
    onEnter(args) {
      const threadId = Process.getCurrentThreadId();
      const active = activeEnumDisplayMonitorsByThread.get(threadId) ?? null;
      this.enumDisplayMonitorsActive = active;
      this.enumDisplayMonitorsRect = args[2];
      if (active === null) {
        this.enumDisplayMonitorsCallback = null;
        return;
      }
      const record = {
        callback: callback.toString(),
        caller: describeCaller(this.returnAddress),
        monitor: args[0].toString(),
        monitorDc: args[1].toString(),
        rectBefore: snapshotRect(args[2]),
        data: args[3].toString(),
        lastErrorBefore: Number(this.lastError) >>> 0,
      };
      this.enumDisplayMonitorsCallback = record;
      active.callbacks.push(record);
    },
    onLeave(result) {
      const record = this.enumDisplayMonitorsCallback;
      if (record === null) {
        return;
      }
      record.result = result.toUInt32();
      record.rectAfter = snapshotRect(this.enumDisplayMonitorsRect);
      record.lastErrorAfter = Number(this.lastError) >>> 0;
    },
  });
}

function snapshotOptionalBool(pointer) {
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), isNull: true };
  }
  try {
    return {
      pointer: pointer.toString(),
      isNull: false,
      value: pointer.readU32(),
    };
  } catch (error) {
    return { pointer: pointer.toString(), isNull: false, error: error.message };
  }
}

function snapshotInstructions(address, maximumInstructions) {
  try {
    const instructions = [];
    let cursor = address;
    for (let index = 0; index < maximumInstructions; index += 1) {
      const instruction = Instruction.parse(cursor);
      instructions.push({
        address: instruction.address.toString(),
        mnemonic: instruction.mnemonic,
        opStr: instruction.opStr,
        size: instruction.size,
      });
      cursor = instruction.next;
    }
    return {
      address: address.toString(),
      maximumInstructions,
      instructions,
    };
  } catch (error) {
    return {
      address: address.toString(),
      maximumInstructions,
      error: error.message,
    };
  }
}

function snapshotInstructionsFollowingDirectJump(address, maximumInstructions) {
  const entry = snapshotInstructions(address, maximumInstructions);
  try {
    const instruction = Instruction.parse(address);
    const operand = instruction.operands?.[0] ?? null;
    if (instruction.mnemonic !== "jmp" || operand?.type !== "imm") {
      return { entry, directJumpTarget: null };
    }
    const target = ptr(operand.value);
    return {
      entry,
      directJumpTarget: snapshotInstructions(target, maximumInstructions),
    };
  } catch (error) {
    return {
      entry,
      directJumpTarget: { error: error.message },
    };
  }
}

function snapshotX86FrameCallerContinuation(context, maximumInstructions) {
  if (Process.arch !== "ia32" || context.ebp === undefined) {
    return { error: "x86 EBP frame is unavailable" };
  }
  try {
    const framePointer = ptr(context.ebp);
    const address = framePointer.add(Process.pointerSize).readPointer();
    return {
      framePointer: framePointer.toString(),
      ...snapshotInstructions(address, maximumInstructions),
    };
  } catch (error) {
    return { error: error.message };
  }
}

function snapshotX86FrameCallerContinuations(
  context,
  maximumFrames,
  maximumInstructions,
) {
  if (Process.arch !== "ia32" || context.ebp === undefined) {
    return { error: "x86 EBP frame is unavailable" };
  }
  const frames = [];
  let framePointer = ptr(context.ebp);
  for (let depth = 0; depth < maximumFrames; depth += 1) {
    try {
      const address = framePointer.add(Process.pointerSize).readPointer();
      frames.push({
        depth,
        framePointer: framePointer.toString(),
        caller: describeCaller(address),
        ...snapshotInstructions(address, maximumInstructions),
      });
      const nextFramePointer = framePointer.readPointer();
      if (nextFramePointer.isNull() || nextFramePointer.equals(framePointer)) {
        break;
      }
      framePointer = nextFramePointer;
    } catch (error) {
      return {
        maximumFrames,
        maximumInstructions,
        frames,
        error: error.message,
      };
    }
  }
  return { maximumFrames, maximumInstructions, frames };
}

function snapshotX86FrameCallerContinuationsFollowingDirectJump(
  context,
  maximumFrames,
  maximumInstructions,
) {
  if (Process.arch !== "ia32" || context.ebp === undefined) {
    return { error: "x86 EBP frame is unavailable" };
  }
  const frames = [];
  let framePointer = ptr(context.ebp);
  for (let depth = 0; depth < maximumFrames; depth += 1) {
    try {
      const address = framePointer.add(Process.pointerSize).readPointer();
      frames.push({
        depth,
        framePointer: framePointer.toString(),
        caller: describeCaller(address),
        continuation: snapshotInstructionsFollowingDirectJump(
          address,
          maximumInstructions,
        ),
      });
      const nextFramePointer = framePointer.readPointer();
      if (nextFramePointer.isNull() || nextFramePointer.equals(framePointer)) {
        break;
      }
      framePointer = nextFramePointer;
    } catch (error) {
      return {
        maximumFrames,
        maximumInstructions,
        frames,
        error: error.message,
      };
    }
  }
  return { maximumFrames, maximumInstructions, frames };
}

function snapshotTlsGetValueCall(caller, returnAddress, lastErrorBefore) {
  const recordsConsumer =
    !tlsGetValueConsumerRecorded && caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    tlsGetValueConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    immediateConsumer: recordsConsumer
      ? snapshotInstructions(returnAddress, 6)
      : null,
  };
}

function snapshotFlsGetValueCall(caller, returnAddress, lastErrorBefore) {
  const recordsConsumer =
    !flsGetValueConsumerRecorded && caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    flsGetValueConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    immediateConsumer: recordsConsumer
      ? snapshotInstructions(returnAddress, 6)
      : null,
  };
}

function snapshotSetLastErrorCall(
  caller,
  returnAddress,
  lastErrorBefore,
  eaxBefore,
) {
  const recordsConsumer =
    !setLastErrorConsumerRecorded && caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    setLastErrorConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    eaxBefore: eaxBefore.toUInt32(),
    immediateConsumer: recordsConsumer
      ? snapshotInstructions(returnAddress, 6)
      : null,
  };
}

function snapshotGetAcpCall(caller, returnAddress, lastErrorBefore, context) {
  const recordsConsumer =
    !getAcpConsumerRecorded && caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    getAcpConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    immediateConsumer: recordsConsumer
      ? {
          apiContinuation: snapshotInstructionsFollowingDirectJump(
            returnAddress,
            32,
          ),
          frameCallerContinuation: snapshotX86FrameCallerContinuation(
            context,
            8,
          ),
        }
      : null,
  };
}

function snapshotGetCpInfoCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  codePage,
  output,
) {
  const recordsConsumer =
    !getCpInfoConsumerRecorded &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    getCpInfoConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    getCpInfo: {
      codePage: codePage.toUInt32(),
      outputBefore: snapshotBoundedRawBytes(output, 32),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              4,
              12,
            ),
          }
        : null,
    },
  };
}

function snapshotGetSystemInfoCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  output,
) {
  const recordsConsumer =
    !getSystemInfoConsumerRecorded &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    getSystemInfoConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    getSystemInfo: {
      outputBefore: snapshotSystemInfo(output),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              64,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              6,
              16,
            ),
          }
        : null,
    },
  };
}

function snapshotGlobalMemoryStatusCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  output,
) {
  const recordsConsumer =
    !globalMemoryStatusConsumerRecorded &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    globalMemoryStatusConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    globalMemoryStatus: {
      outputBefore: snapshotMemoryStatus(output),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              96,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              6,
              24,
            ),
          }
        : null,
    },
  };
}

function snapshotProcessorFeatureCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  feature,
) {
  const recordsConsumer =
    processorFeatureConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    processorFeatureConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    processorFeature: {
      index: feature.toUInt32(),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              4,
              12,
            ),
          }
        : null,
    },
  };
}

function snapshotSetThreadAffinityMaskCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  threadHandle,
  affinityMask,
) {
  const recordsConsumer =
    setThreadAffinityMaskConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    setThreadAffinityMaskConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    setThreadAffinityMask: {
      threadHandle: threadHandle.toString(),
      affinityMask: affinityMask.toUInt32(),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              8,
              12,
            ),
          }
        : null,
    },
  };
}

function snapshotUnhandledExceptionFilterCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  filter,
) {
  const recordsConsumer =
    unhandledExceptionFilterConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    unhandledExceptionFilterConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    unhandledExceptionFilter: {
      filter: filter.toString(),
      filterIdentity: filter.isNull() ? null : describeCaller(filter),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              4,
              12,
            ),
          }
        : null,
    },
  };
}

function snapshotMultiByteToWideCharCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  codePage,
  flags,
  source,
  sourceBytes,
  destination,
  destinationUnits,
) {
  const consumerKey =
    caller.module !== null && caller.module.toLowerCase() === "bgi.exe"
      ? `${caller.module.toLowerCase()}:${caller.rva ?? caller.address}`
      : null;
  const recordsConsumer =
    consumerKey !== null && !multiByteConsumerKeys.has(consumerKey);
  if (recordsConsumer) {
    multiByteConsumerKeys.add(consumerKey);
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    multiByteToWideChar: {
      codePage: codePage.toUInt32(),
      flags: flags.toUInt32(),
      source: snapshotMultiByteSource(source, sourceBytes),
      destinationBefore: snapshotWideCharBuffer(
        destination,
        Math.max(0, destinationUnits),
      ),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              context,
              4,
              12,
            ),
          }
        : null,
    },
  };
}

function snapshotSetCurrentDirectoryACall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  path,
  currentDirectoryBefore,
) {
  const recordsConsumer =
    setCurrentDirectoryConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    setCurrentDirectoryConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    setCurrentDirectoryA: {
      pathBefore: snapshotMultiByteSource(path, -1),
      currentDirectoryBefore,
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function snapshotCreateMutexACall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  securityAttributes,
  initialOwner,
  name,
) {
  const recordsConsumer =
    createMutexConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    createMutexConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    createMutexA: {
      securityAttributes: securityAttributes.toString(),
      initialOwner: initialOwner.toUInt32(),
      nameBefore: snapshotMultiByteSource(name, -1),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function snapshotGetStringTypeWCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  infoType,
  source,
  sourceUnits,
  output,
) {
  const recordsConsumer =
    !getStringTypeConsumerRecorded &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    getStringTypeConsumerRecorded = true;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    getStringTypeW: {
      infoType: infoType.toUInt32(),
      sourceCount: sourceUnits,
      source: snapshotWideCharSource(
        source,
        sourceUnits < 0 ? -1 : sourceUnits,
      ),
      sourceContent: snapshotWordBuffer(source, Math.max(0, sourceUnits)),
      outputBefore: snapshotWordBuffer(output, Math.max(0, sourceUnits)),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function snapshotLCMapStringWCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  locale,
  flags,
  source,
  sourceUnits,
  destination,
  destinationUnits,
) {
  const recordsConsumer =
    lcMapStringConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    lcMapStringConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    lcMapStringW: {
      locale: locale.toUInt32(),
      flags: flags.toUInt32(),
      sourceCount: sourceUnits,
      destinationCount: destinationUnits,
      source: snapshotWideCharSource(
        source,
        sourceUnits < 0 ? -1 : sourceUnits,
      ),
      sourceContent: snapshotWordBuffer(source, Math.max(0, sourceUnits)),
      destinationBefore: snapshotWideCharBuffer(
        destination,
        Math.max(0, destinationUnits),
      ),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function snapshotInterlockedDecrementCall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  target,
) {
  const recordsConsumer =
    interlockedDecrementConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    interlockedDecrementConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    interlockedDecrement: {
      targetBefore: snapshotLong(target),
      targetAlignment: target.toUInt32() % 4,
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function snapshotAnsiOutputBuffer(pointer, requestedBytes) {
  const maximumBytes = 32 * 1024;
  const maximumRecordedBytes = 4096;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      isNull: true,
    };
  }
  if (
    !Number.isInteger(requestedBytes) ||
    requestedBytes < 0 ||
    requestedBytes > maximumBytes
  ) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: "ANSI buffer length is outside the bounded observation",
    };
  }
  try {
    const buffer = pointer.readByteArray(requestedBytes);
    const bytes = [...new Uint8Array(buffer)];
    let hash = 0x811c9dc5;
    let nulBytes = 0;
    let firstNulOffset = null;
    for (const [index, byte] of bytes.entries()) {
      hash = fnv1aByte(hash, byte);
      if (byte === 0) {
        nulBytes += 1;
        firstNulOffset ??= index;
      }
    }
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      isNull: false,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      nulBytes,
      firstNulOffset,
      bytes:
        requestedBytes <= maximumRecordedBytes
          ? bytes
          : bytes.slice(0, maximumRecordedBytes),
      bytesTruncated: requestedBytes > maximumRecordedBytes,
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      maximumBytes,
      error: error.message,
    };
  }
}

function snapshotGetModuleFileNameACall(
  caller,
  returnAddress,
  lastErrorBefore,
  context,
  moduleHandle,
  output,
  capacity,
) {
  const recordsConsumer =
    getModuleFileNameAConsumersRecorded < 2 &&
    caller.module !== null &&
    caller.module.toLowerCase() === "bgi.exe";
  if (recordsConsumer) {
    getModuleFileNameAConsumersRecorded += 1;
  }
  return {
    lastErrorBefore: Number(lastErrorBefore) >>> 0,
    getModuleFileNameA: {
      moduleHandle: moduleHandle.toString(),
      capacity,
      outputBefore: snapshotAnsiOutputBuffer(output, capacity),
      immediateConsumer: recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              returnAddress,
              32,
            ),
            frameCallerContinuations:
              snapshotX86FrameCallerContinuationsFollowingDirectJump(
                context,
                4,
                32,
              ),
          }
        : null,
    },
  };
}

function readProcessParametersEnvironmentPointer() {
  try {
    const ntdll = Process.getModuleByName("ntdll.dll");
    const address = findExport(ntdll, "RtlGetCurrentPeb");
    if (address === null) {
      return { error: "RtlGetCurrentPeb export is unavailable" };
    }
    const peb = new NativeFunction(address, "pointer", [], "stdcall")();
    const processParameters = peb.add(0x10).readPointer();
    const environment = processParameters.add(0x48).readPointer();
    return {
      peb: peb.toString(),
      processParameters: processParameters.toString(),
      environment: environment.toString(),
    };
  } catch (error) {
    return { error: error.message };
  }
}

function snapshotUtf16EnvironmentBlock(pointer, processEnvironmentPointer) {
  const maximumBytes = 1024 * 1024;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      encoding: "utf16-environment-block",
      maximumBytes,
      error: "null pointer",
    };
  }
  try {
    const entryUtf16UnitLengths = [];
    const foldedNames = [];
    let pseudoDriveEntryCount = 0;
    let entriesMissingAssignmentSeparator = 0;
    let current = [];
    let previousWasNul = false;
    let byteLengthWithDoubleNul = null;
    let hash = 0x811c9dc5;
    for (let offset = 0; offset + 2 <= maximumBytes; offset += 2) {
      const unit = pointer.add(offset).readU16();
      hash = fnv1aByte(hash, unit & 0xff);
      hash = fnv1aByte(hash, unit >>> 8);
      if (unit === 0) {
        if (previousWasNul) {
          byteLengthWithDoubleNul = offset + 2;
          break;
        }
        if (current.length > 0) {
          entryUtf16UnitLengths.push(current.length);
          const assignmentStart = current[0] === 0x3d ? 1 : 0;
          const assignmentIndex = current.indexOf(0x3d, assignmentStart);
          if (assignmentIndex < 0) {
            entriesMissingAssignmentSeparator += 1;
          } else {
            if (current[0] === 0x3d) {
              pseudoDriveEntryCount += 1;
            }
            foldedNames.push(
              String.fromCharCode(
                ...current.slice(0, assignmentIndex),
              ).toUpperCase(),
            );
          }
        }
        current = [];
        previousWasNul = true;
      } else {
        current.push(unit);
        previousWasNul = false;
      }
    }
    if (byteLengthWithDoubleNul === null) {
      return {
        pointer: pointer.toString(),
        encoding: "utf16-environment-block",
        maximumBytes,
        error: "double-NUL terminator not found within bounded observation",
      };
    }
    const names = new Set();
    let duplicateCaseInsensitiveNameCount = 0;
    for (const name of foldedNames) {
      if (names.has(name)) {
        duplicateCaseInsensitiveNameCount += 1;
      }
      names.add(name);
    }
    const range = Process.findRangeByAddress(pointer);
    return {
      pointer: pointer.toString(),
      encoding: "utf16-environment-block",
      maximumBytes,
      byteLengthWithDoubleNul,
      entryCount: entryUtf16UnitLengths.length,
      entryUtf16UnitLengths,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      pseudoDriveEntryCount,
      entriesMissingAssignmentSeparator,
      duplicateCaseInsensitiveNameCount,
      caseFoldedNameOrderNondecreasing: foldedNames.every(
        (name, index) => index === 0 || foldedNames[index - 1] <= name,
      ),
      processParametersEnvironmentPointer:
        processEnvironmentPointer?.environment ?? null,
      equalsProcessParametersEnvironmentPointer:
        processEnvironmentPointer?.environment === pointer.toString(),
      range:
        range === null
          ? null
          : {
              base: range.base.toString(),
              size: range.size,
              protection: range.protection,
              pointerOffset: pointer.sub(range.base).toUInt32(),
            },
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      encoding: "utf16-environment-block",
      maximumBytes,
      error: error.message,
    };
  }
}

function emit(kind, fields = {}) {
  send({
    schema,
    sequence: sequence++,
    timestampMs: Date.now(),
    threadId: Process.getCurrentThreadId(),
    kind,
    ...fields,
  });
}

function readString(pointer, encoding) {
  if (pointer.isNull()) {
    return null;
  }
  try {
    return encoding === "utf16"
      ? pointer.readUtf16String()
      : pointer.readAnsiString();
  } catch (error) {
    return `<unreadable:${error.message}>`;
  }
}

function decodeArgument(argument, decoder) {
  if (decoder === "ansi") {
    return readString(argument, "ansi");
  }
  if (decoder === "utf16") {
    return readString(argument, "utf16");
  }
  if (decoder === "proc") {
    const value = argument.toUInt32();
    return value <= 0xffff ? `#${value}` : readString(argument, "ansi");
  }
  return argument.toString();
}

function readOptionalUtf16Address(address) {
  if (address === 0) {
    return null;
  }
  try {
    return ptr(address).readUtf16String();
  } catch (error) {
    return `<unreadable:${error.message}>`;
  }
}

function snapshotStartupInfoW(pointer, decodeStrings = false) {
  const size = 68;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 STARTUPINFOW observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    const bytes = [...new Uint8Array(buffer)];
    const view = new DataView(buffer);
    const fields = {
      cb: view.getUint32(0, true),
      lpReserved: view.getUint32(4, true),
      lpDesktop: view.getUint32(8, true),
      lpTitle: view.getUint32(12, true),
      dwX: view.getUint32(16, true),
      dwY: view.getUint32(20, true),
      dwXSize: view.getUint32(24, true),
      dwYSize: view.getUint32(28, true),
      dwXCountChars: view.getUint32(32, true),
      dwYCountChars: view.getUint32(36, true),
      dwFillAttribute: view.getUint32(40, true),
      dwFlags: view.getUint32(44, true),
      wShowWindow: view.getUint16(48, true),
      cbReserved2: view.getUint16(50, true),
      lpReserved2: view.getUint32(52, true),
      hStdInput: view.getUint32(56, true),
      hStdOutput: view.getUint32(60, true),
      hStdError: view.getUint32(64, true),
    };
    return {
      pointer: pointer.toString(),
      size,
      bytes,
      fields,
      ...(decodeStrings
        ? {
            strings: {
              reserved: readOptionalUtf16Address(fields.lpReserved),
              desktop: readOptionalUtf16Address(fields.lpDesktop),
              title: readOptionalUtf16Address(fields.lpTitle),
            },
          }
        : {}),
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotCriticalSection(pointer) {
  const size = 24;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 CRITICAL_SECTION observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    const bytes = [...new Uint8Array(buffer)];
    const view = new DataView(buffer);
    return {
      pointer: pointer.toString(),
      size,
      bytes,
      fields: {
        debugInfo: view.getUint32(0, true),
        lockCount: view.getInt32(4, true),
        recursionCount: view.getInt32(8, true),
        owningThread: view.getUint32(12, true),
        lockSemaphore: view.getUint32(16, true),
        spinCount: view.getUint32(20, true),
      },
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotD3d9DisplayMode(pointer) {
  const size = 16;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 D3DDISPLAYMODE observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    const view = new DataView(buffer);
    return {
      pointer: pointer.toString(),
      size,
      bytes: [...new Uint8Array(buffer)],
      fields: {
        width: view.getUint32(0, true),
        height: view.getUint32(4, true),
        refreshRate: view.getUint32(8, true),
        format: view.getUint32(12, true),
      },
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotD3d9AdapterIdentifier(pointer) {
  const size = 1100;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 D3DADAPTER_IDENTIFIER9 observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    return {
      pointer: pointer.toString(),
      size,
      bytes: [...new Uint8Array(buffer)],
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotD3d9DeviceCaps(pointer) {
  const size = 304;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 D3DCAPS9 observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    return {
      pointer: pointer.toString(),
      size,
      bytes: [...new Uint8Array(buffer)],
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotD3d9PresentParameters(pointer) {
  const size = 56;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 D3DPRESENT_PARAMETERS observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    const view = new DataView(buffer);
    return {
      pointer: pointer.toString(),
      size,
      bytes: [...new Uint8Array(buffer)],
      fields: {
        backBufferWidth: view.getUint32(0, true),
        backBufferHeight: view.getUint32(4, true),
        backBufferFormat: view.getUint32(8, true),
        backBufferCount: view.getUint32(12, true),
        multiSampleType: view.getUint32(16, true),
        multiSampleQuality: view.getUint32(20, true),
        swapEffect: view.getUint32(24, true),
        deviceWindow: view.getUint32(28, true),
        windowed: view.getUint32(32, true),
        enableAutoDepthStencil: view.getUint32(36, true),
        autoDepthStencilFormat: view.getUint32(40, true),
        flags: view.getUint32(44, true),
        fullScreenRefreshRateInHz: view.getUint32(48, true),
        presentationInterval: view.getUint32(52, true),
      },
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotD3d9RasterStatus(pointer) {
  const size = 8;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 D3DRASTER_STATUS observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    const view = new DataView(buffer);
    return {
      pointer: pointer.toString(),
      size,
      bytes: [...new Uint8Array(buffer)],
      fields: {
        inVBlank: view.getUint32(0, true),
        scanLine: view.getUint32(4, true),
      },
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotBoundedAnsiBytes(pointer, maximumBytes = 32) {
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      maximumBytes,
      bytesWithNul: [],
      bytes: [],
      string: null,
    };
  }
  try {
    const bytesWithNul = [];
    for (let offset = 0; offset < maximumBytes; offset += 1) {
      const value = pointer.add(offset).readU8();
      bytesWithNul.push(value);
      if (value === 0) {
        return {
          pointer: pointer.toString(),
          maximumBytes,
          bytesWithNul,
          bytes: bytesWithNul.slice(0, -1),
          string: readString(pointer, "ansi"),
        };
      }
    }
    return {
      pointer: pointer.toString(),
      maximumBytes,
      bytesWithNul,
      bytes: bytesWithNul,
      error: "terminator not found within bounded observation",
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      maximumBytes,
      error: error.message,
    };
  }
}

function snapshotDsoundBufferContent(pointer, byteLength) {
  const maximumBytes = 1024 * 1024;
  const edgeBytes = 64;
  if (
    !Number.isInteger(byteLength) ||
    byteLength < 0 ||
    byteLength > maximumBytes
  ) {
    return {
      pointer: pointer.toString(),
      byteLength,
      maximumBytes,
      error: "DirectSound buffer length is outside the bounded observation",
    };
  }
  if (byteLength === 0) {
    return {
      pointer: pointer.toString(),
      byteLength,
      maximumBytes,
      isNull: pointer.isNull(),
      contentFnv1a32: "0x811c9dc5",
      zeroBytes: 0,
      prefix: [],
      suffix: [],
    };
  }
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      byteLength,
      maximumBytes,
      isNull: true,
      error: "nonempty DirectSound segment has a null pointer",
    };
  }
  try {
    const buffer = pointer.readByteArray(byteLength);
    const bytes = new Uint8Array(buffer);
    let hash = 0x811c9dc5;
    let zeroBytes = 0;
    for (const byte of bytes) {
      hash = fnv1aByte(hash, byte);
      zeroBytes += Number(byte === 0);
    }
    const prefixLength = Math.min(edgeBytes, byteLength);
    const suffixOffset = Math.max(0, byteLength - edgeBytes);
    return {
      pointer: pointer.toString(),
      byteLength,
      maximumBytes,
      isNull: false,
      contentFnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
      zeroBytes,
      prefix: [...bytes.slice(0, prefixLength)],
      suffix: [...bytes.slice(suffixOffset)],
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      byteLength,
      maximumBytes,
      isNull: false,
      error: error.message,
    };
  }
}

function snapshotWaveFormatEx(pointer) {
  const minimumSize = 16;
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      size: minimumSize,
      error: "null pointer",
    };
  }
  try {
    const prefix = pointer.readByteArray(minimumSize);
    const prefixView = new DataView(prefix);
    const formatTag = prefixView.getUint16(0, true);
    const size = formatTag === 1 ? minimumSize : 18;
    const buffer = pointer.readByteArray(size);
    const view = new DataView(buffer);
    return {
      pointer: pointer.toString(),
      size,
      bytes: [...new Uint8Array(buffer)],
      fields: {
        formatTag,
        channels: view.getUint16(2, true),
        samplesPerSecond: view.getUint32(4, true),
        averageBytesPerSecond: view.getUint32(8, true),
        blockAlign: view.getUint16(12, true),
        bitsPerSample: view.getUint16(14, true),
        extraBytes: size === 18 ? view.getUint16(16, true) : 0,
      },
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      size: minimumSize,
      error: error.message,
    };
  }
}

function snapshotDirectSoundBufferDesc(pointer) {
  const size = 36;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 DSBUFFERDESC observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    const view = new DataView(buffer);
    const formatAddress = view.getUint32(16, true);
    return {
      pointer: pointer.toString(),
      size,
      bytes: [...new Uint8Array(buffer)],
      fields: {
        size: view.getUint32(0, true),
        flags: view.getUint32(4, true),
        bufferBytes: view.getUint32(8, true),
        reserved: view.getUint32(12, true),
        format: formatAddress,
        algorithmGuid: [...new Uint8Array(buffer.slice(20, 36))],
      },
      format:
        formatAddress === 0 ? null : snapshotWaveFormatEx(ptr(formatAddress)),
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotFixedAnsiBuffer(pointer, maximumBytes) {
  const raw = snapshotBoundedRawBytes(pointer, maximumBytes, maximumBytes);
  if (!Array.isArray(raw.bytes)) {
    return { raw };
  }
  const terminator = raw.bytes.indexOf(0);
  const byteLength = terminator === -1 ? maximumBytes : terminator;
  return {
    raw,
    byteLength,
    terminated: terminator !== -1,
    codeUnits: raw.bytes.slice(0, byteLength),
  };
}

function snapshotDinput8DeviceInstanceA(pointer) {
  const byteLength = 580;
  const raw = snapshotBoundedRawBytes(pointer, byteLength, byteLength);
  if (!Array.isArray(raw.bytes)) {
    return { profile: "x86-dideviceinstancea-v1", byteLength, raw };
  }
  try {
    return {
      profile: "x86-dideviceinstancea-v1",
      byteLength,
      raw,
      fields: {
        size: pointer.readU32(),
        instanceGuid: snapshotBoundedRawBytes(pointer.add(4), 16, 16),
        productGuid: snapshotBoundedRawBytes(pointer.add(20), 16, 16),
        deviceType: pointer.add(36).readU32(),
        instanceName: snapshotFixedAnsiBuffer(pointer.add(40), 260),
        productName: snapshotFixedAnsiBuffer(pointer.add(300), 260),
        forceFeedbackDriverGuid: snapshotBoundedRawBytes(
          pointer.add(560),
          16,
          16,
        ),
        usagePage: pointer.add(576).readU16(),
        usage: pointer.add(578).readU16(),
      },
    };
  } catch (error) {
    return {
      profile: "x86-dideviceinstancea-v1",
      byteLength,
      raw,
      error: error.message,
    };
  }
}

function attachDinput8EnumDevicesCallback(callback) {
  if (callback.isNull()) {
    return;
  }
  const key = callback.toString().toLowerCase();
  if (attachedDinput8EnumDevicesCallbacks.has(key)) {
    return;
  }
  attachedDinput8EnumDevicesCallbacks.add(key);
  Interceptor.attach(callback, {
    onEnter(args) {
      const threadId = Process.getCurrentThreadId();
      const active = activeDinput8EnumDevicesByThread.get(threadId) ?? null;
      this.dinput8EnumDevicesActive = active;
      this.dinput8EnumDevicesInstance = args[0];
      if (active === null) {
        this.dinput8EnumDevicesCallback = null;
        return;
      }
      const record = {
        callback: callback.toString(),
        callbackIdentity: describeCaller(callback),
        caller: describeCaller(this.returnAddress),
        deviceInstancePointer: args[0].toString(),
        deviceInstanceBefore: snapshotDinput8DeviceInstanceA(args[0]),
        reference: args[1].toString(),
        lastErrorBefore: Number(this.lastError) >>> 0,
      };
      this.dinput8EnumDevicesCallback = record;
      active.callbacks.push(record);
    },
    onLeave(result) {
      const record = this.dinput8EnumDevicesCallback;
      if (record === null) {
        return;
      }
      record.result = result.toUInt32();
      record.deviceInstanceAfter = snapshotDinput8DeviceInstanceA(
        this.dinput8EnumDevicesInstance,
      );
      record.lastErrorAfter = Number(this.lastError) >>> 0;
    },
  });
}

function snapshotMonitorInfoA(pointer) {
  const profile = "x86-monitor-info-a-v1";
  const header = snapshotBoundedRawBytes(pointer, 4);
  if (header.error !== undefined) {
    return { profile, header };
  }
  try {
    const cbSize = pointer.readU32();
    const capturedBytes = Math.min(Math.max(cbSize, 4), 72);
    const raw = snapshotBoundedRawBytes(pointer, capturedBytes, 72);
    const result = { profile, cbSize, capturedBytes, raw };
    if (raw.error !== undefined || capturedBytes < 40) {
      return result;
    }
    result.fields = {
      monitor: snapshotRect(pointer.add(4)),
      work: snapshotRect(pointer.add(20)),
      flags: pointer.add(36).readU32(),
    };
    if (capturedBytes >= 72) {
      const deviceBytes = [
        ...new Uint8Array(pointer.add(40).readByteArray(32)),
      ];
      const terminator = deviceBytes.indexOf(0);
      result.fields.deviceBytes = deviceBytes;
      result.fields.deviceBytesWithoutNul =
        terminator < 0 ? deviceBytes : deviceBytes.slice(0, terminator);
      result.fields.deviceTerminated = terminator >= 0;
    }
    return result;
  } catch (error) {
    return { profile, header, error: error.message };
  }
}

function snapshotUnsignedLargeInteger(pointer) {
  const size = 8;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 LARGE_INTEGER observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    const view = new DataView(buffer);
    const low = view.getUint32(0, true);
    const high = view.getUint32(4, true);
    return {
      pointer: pointer.toString(),
      size,
      bytes: [...new Uint8Array(buffer)],
      low,
      high,
      value: ((BigInt(high) << 32n) | BigInt(low)).toString(),
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotSignedLargeInteger(pointer) {
  const snapshot = snapshotUnsignedLargeInteger(pointer);
  if (snapshot.error !== undefined) {
    return snapshot;
  }
  const unsigned = BigInt(snapshot.value);
  const signed =
    unsigned >= 0x8000000000000000n
      ? unsigned - 0x10000000000000000n
      : unsigned;
  return {
    ...snapshot,
    signedValue: signed.toString(),
    interpretation: signed < 0n ? "relative-100ns" : "absolute-file-time",
  };
}

function snapshotPointerSlot(pointer) {
  const size = 4;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 pointer-slot observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    return {
      pointer: pointer.toString(),
      size,
      bytes: [...new Uint8Array(buffer)],
      value: pointer.readPointer().toString(),
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotLong(pointer) {
  const size = 4;
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    return {
      pointer: pointer.toString(),
      size,
      bytes: [...new Uint8Array(buffer)],
      value: new DataView(buffer).getInt32(0, true),
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function describeCaller(address) {
  const module = Process.findModuleByAddress(address);
  return {
    address: address.toString(),
    module: module === null ? null : module.name,
    rva: module === null ? null : address.sub(module.base).toUInt32(),
  };
}

function beginCall(fields, returnAddress) {
  const threadId = Process.getCurrentThreadId();
  const stack = activeCallsByThread.get(threadId) ?? [];
  activeCallsByThread.set(threadId, stack);
  const frame = {
    callId: nextCallId++,
    parentCallId: stack.at(-1)?.callId ?? null,
    caller: describeCaller(returnAddress),
    startedAtMs: Date.now(),
    threadId,
  };
  stack.push(frame);
  emit("api-enter", {
    threadId,
    callId: frame.callId,
    parentCallId: frame.parentCallId,
    caller: frame.caller,
    ...fields,
  });
  return frame;
}

function finishCall(frame, fields, lastError) {
  const finishedAtMs = Date.now();
  const stack = activeCallsByThread.get(frame.threadId) ?? [];
  if (stack.at(-1) === frame) {
    stack.pop();
  } else {
    emit("call-stack-error", {
      threadId: frame.threadId,
      callId: frame.callId,
      expectedCallId: stack.at(-1)?.callId ?? null,
    });
    const index = stack.lastIndexOf(frame);
    if (index !== -1) {
      stack.splice(index, 1);
    }
  }
  if (stack.length === 0) {
    activeCallsByThread.delete(frame.threadId);
  }
  emit("api-leave", {
    threadId: frame.threadId,
    callId: frame.callId,
    parentCallId: frame.parentCallId,
    caller: frame.caller,
    lastError: Number(lastError) >>> 0,
    durationMs: Math.max(0, finishedAtMs - frame.startedAtMs),
    ...fields,
  });
}

const gdiplusImportedFunctions = [
  ["GdiplusShutdown", 1],
  ["GdipGetImageHeight", 2],
  ["GdipCreateBitmapFromGdiDib", 3],
  ["GdipGetImagePixelFormat", 2],
  ["GdipFree", 1],
  ["GdipAlloc", 1],
  ["GdipGetImageEncodersSize", 2],
  ["GdipDisposeImage", 1],
  ["GdipCreateBitmapFromFile", 2],
  ["GdipBitmapUnlockBits", 2],
  ["GdiplusStartup", 3],
  ["GdipGetImageWidth", 2],
  ["GdipCloneImage", 2],
  ["GdipGetImageEncoders", 3],
  ["GdipSaveImageToFile", 4],
  ["GdipBitmapLockBits", 5],
];

const hooks = {
  "kernel32.dll": [
    ["LoadLibraryA", ["ansi"]],
    ["LoadLibraryW", ["utf16"]],
    ["GetProcAddress", [null, "proc"]],
    ["CreateFileA", ["ansi"]],
    ["CreateFileW", ["utf16"]],
    ["CreateThread", []],
  ],
  "user32.dll": [
    ["CreateWindowExA", [null, "ansi", "ansi"]],
    ["CreateWindowExW", [null, "utf16", "utf16"]],
  ],
  "shell32.dll": [],
  "imm32.dll": [],
  "gdi32.dll": [
    ["BitBlt", []],
    ["StretchDIBits", []],
  ],
  "gdiplus.dll": [["GdiplusStartup", []]],
  "d3d9.dll": [["Direct3DCreate9", [null]]],
  "dinput8.dll": [["DirectInput8Create", [null, null, null, null, null]]],
  "dsound.dll": [
    ["DirectSoundCreate", []],
    ["DirectSoundCreate8", [null, null, null]],
  ],
  "winmm.dll": [
    ["mciSendCommandA", [null, null, null, null]],
    ["mciSendCommandW", [null, null, null, null]],
    ["waveOutOpen", []],
  ],
  "ole32.dll": [
    ["CoInitialize", [null]],
    ["CoInitializeEx", [null, null]],
    ["CoCreateInstance", []],
  ],
};

if (focusedGuestThreadObservation) {
  hooks["kernel32.dll"] = hooks["kernel32.dll"].filter(
    ([name]) => name !== "CreateThread",
  );
}

function snapshotGdiplusStartupInput(pointer) {
  const size = 16;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 GdiplusStartupInput observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    return {
      pointer: pointer.toString(),
      size,
      raw: snapshotBoundedRawBytes(pointer, size, size),
      fields: {
        gdiplusVersion: pointer.readU32(),
        debugEventCallback: pointer.add(4).readPointer().toString(),
        suppressBackgroundThread: pointer.add(8).readU32(),
        suppressExternalCodecs: pointer.add(12).readU32(),
      },
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function snapshotGdiplusStartupOutput(pointer) {
  const size = 8;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 GdiplusStartupOutput observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, isNull: true };
  }
  try {
    return {
      pointer: pointer.toString(),
      size,
      isNull: false,
      raw: snapshotBoundedRawBytes(pointer, size, size),
      fields: {
        notificationHook: pointer.readPointer().toString(),
        notificationUnhook: pointer.add(4).readPointer().toString(),
      },
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      size,
      isNull: false,
      error: error.message,
    };
  }
}

if (selectedObservations.has("create-window-ex-a")) {
  hooks["user32.dll"] = hooks["user32.dll"].filter(
    ([name]) => name !== "CreateWindowExA",
  );
  hooks["user32.dll"].push(["DefWindowProcA", [null, null, null, null]]);
}

if (selectedObservations.has("get-system-menu")) {
  hooks["user32.dll"].push(
    ["GetSystemMenu", [null, null]],
    ["EnableMenuItem", [null, null, null]],
  );
}

if (selectedObservations.has("startup-info")) {
  hooks["kernel32.dll"].push(["GetStartupInfoW", [null]]);
}
if (selectedObservations.has("heap-set-information")) {
  hooks["kernel32.dll"].push(["HeapSetInformation", [null, null, null, null]]);
}
if (selectedObservations.has("heap-lifecycle")) {
  hooks["kernel32.dll"].push(
    ["HeapCreate", [null, null, null]],
    ["HeapAlloc", [null, null, null]],
    ["HeapReAlloc", [null, null, null, null]],
    ["HeapFree", [null, null, null]],
    ["HeapSize", [null, null, null]],
    ["HeapDestroy", [null]],
  );
}
if (selectedObservations.has("heap-allocation")) {
  hooks["kernel32.dll"].push(
    ["HeapAlloc", [null, null, null]],
    ["HeapFree", [null, null, null]],
  );
}
if (selectedObservations.has("heap-size")) {
  hooks["kernel32.dll"].push(
    ["HeapAlloc", [null, null, null]],
    ["HeapFree", [null, null, null]],
    ["HeapSize", [null, null, null]],
  );
}
if (selectedObservations.has("module-handle")) {
  hooks["kernel32.dll"].push(
    ["GetModuleHandleA", ["ansi"]],
    ["GetModuleHandleW", ["utf16"]],
  );
}
if (selectedObservations.has("tls-lifecycle")) {
  hooks["kernel32.dll"].push(
    ["TlsAlloc", []],
    ["TlsFree", [null]],
    ["TlsGetValue", [null]],
    ["TlsSetValue", [null, null]],
  );
}
if (selectedObservations.has("fls-lifecycle")) {
  hooks["kernel32.dll"].push(
    ["FlsAlloc", [null]],
    ["FlsFree", [null]],
    ["FlsGetValue", [null]],
    ["FlsSetValue", [null, null]],
  );
}
if (selectedObservations.has("pointer-encoding")) {
  hooks["kernel32.dll"].push(
    ["EncodePointer", [null]],
    ["DecodePointer", [null]],
  );
}
if (selectedObservations.has("critical-section")) {
  hooks["kernel32.dll"].push(
    ["InitializeCriticalSection", [null]],
    ["InitializeCriticalSectionAndSpinCount", [null, null]],
    ["InitializeCriticalSectionEx", [null, null, null]],
    ["EnterCriticalSection", [null]],
    ["TryEnterCriticalSection", [null]],
    ["LeaveCriticalSection", [null]],
    ["SetCriticalSectionSpinCount", [null, null]],
    ["DeleteCriticalSection", [null]],
  );
}
if (selectedObservations.has("interlocked-increment")) {
  hooks["kernel32.dll"].push(["InterlockedIncrement", [null]]);
}
if (selectedObservations.has("interlocked-decrement")) {
  hooks["kernel32.dll"].push(["InterlockedDecrement", [null]]);
}
if (selectedObservations.has("get-module-file-name-a")) {
  hooks["kernel32.dll"].push(["GetModuleFileNameA", [null, null, null]]);
}
if (selectedObservations.has("standard-handle")) {
  hooks["kernel32.dll"].push(["GetStdHandle", [null]]);
}
if (selectedObservations.has("file-type")) {
  hooks["kernel32.dll"].push(["GetFileType", [null]]);
}
if (selectedObservations.has("set-handle-count")) {
  hooks["kernel32.dll"].push(["SetHandleCount", [null]]);
}
if (selectedObservations.has("command-line-a")) {
  hooks["kernel32.dll"].push(["GetCommandLineA", []], ["GetCommandLineW", []]);
}
if (selectedObservations.has("environment-strings-w")) {
  hooks["kernel32.dll"].push(
    ["GetEnvironmentStringsW", []],
    ["FreeEnvironmentStringsW", [null]],
  );
}
if (selectedObservations.has("wide-char-to-multi-byte")) {
  hooks["kernel32.dll"].push([
    "WideCharToMultiByte",
    [null, null, null, null, null, null, null, null],
  ]);
}
if (selectedObservations.has("multi-byte-to-wide-char")) {
  hooks["kernel32.dll"].push([
    "MultiByteToWideChar",
    [null, null, null, null, null, null],
  ]);
}
if (selectedObservations.has("set-current-directory-a")) {
  hooks["kernel32.dll"].push(["SetCurrentDirectoryA", ["ansi"]]);
}
if (selectedObservations.has("source-file-read")) {
  const createFileA = hooks["kernel32.dll"].findIndex(
    ([name]) => name === "CreateFileA",
  );
  const createFileW = hooks["kernel32.dll"].findIndex(
    ([name]) => name === "CreateFileW",
  );
  hooks["kernel32.dll"][createFileA] = [
    "CreateFileA",
    ["ansi", null, null, null, null, null, null],
  ];
  hooks["kernel32.dll"][createFileW] = [
    "CreateFileW",
    ["utf16", null, null, null, null, null, null],
  ];
  hooks["kernel32.dll"].push(
    ["ReadFile", [null, null, null, null, null]],
    ["WriteFile", [null, null, null, null, null]],
    ["SetFilePointer", [null, null, null, null]],
    ["GetFileSize", [null, null]],
    ["FlushFileBuffers", [null]],
    ["CloseHandle", [null]],
  );
}
if (selectedObservations.has("create-mutex-a")) {
  hooks["kernel32.dll"].push(
    ["CreateMutexA", [null, null, "ansi"]],
    ["WaitForSingleObject", [null, null]],
    ["ReleaseMutex", [null]],
    ["CloseHandle", [null]],
  );
}
if (selectedObservations.has("get-string-type-w")) {
  hooks["kernel32.dll"].push(["GetStringTypeW", [null, null, null, null]]);
}
if (selectedObservations.has("lc-map-string-w")) {
  hooks["kernel32.dll"].push([
    "LCMapStringW",
    [null, null, null, null, null, null],
  ]);
}
if (selectedObservations.has("get-last-error")) {
  hooks["kernel32.dll"].push(["GetLastError", []]);
}
if (selectedObservations.has("set-last-error")) {
  hooks["kernel32.dll"].push(["SetLastError", [null]]);
}
if (selectedObservations.has("get-acp")) {
  hooks["kernel32.dll"].push(["GetACP", []]);
}
if (selectedObservations.has("system-default-lang-id")) {
  hooks["kernel32.dll"].push(["GetSystemDefaultLangID", []]);
}
if (selectedObservations.has("drive-type")) {
  hooks["kernel32.dll"].push(["GetDriveTypeA", ["ansi"]]);
}
if (selectedObservations.has("get-cp-info")) {
  hooks["kernel32.dll"].push(["GetCPInfo", [null, null]]);
}
if (selectedObservations.has("processor-feature")) {
  hooks["kernel32.dll"].push(["IsProcessorFeaturePresent", [null]]);
}
if (selectedObservations.has("unhandled-exception-filter")) {
  hooks["kernel32.dll"].push(["SetUnhandledExceptionFilter", [null]]);
}
if (selectedObservations.has("set-thread-affinity-mask")) {
  hooks["kernel32.dll"].push(["SetThreadAffinityMask", [null, null]]);
}
if (selectedObservations.has("get-system-info")) {
  hooks["kernel32.dll"].push(["GetSystemInfo", [null]]);
}
if (selectedObservations.has("global-memory-status")) {
  hooks["kernel32.dll"].push(["GlobalMemoryStatus", [null]]);
}
if (selectedObservations.has("enum-display-monitors")) {
  hooks["user32.dll"].push(["EnumDisplayMonitors", [null, null, null, null]]);
}
if (selectedObservations.has("swap-mouse-button")) {
  hooks["user32.dll"].push(["SwapMouseButton", [null]]);
}
if (selectedObservations.has("drag-accept-files")) {
  hooks["shell32.dll"].push(["DragAcceptFiles", [null, null]]);
}
if (selectedObservations.has("time-get-dev-caps")) {
  hooks["winmm.dll"].push(
    ["timeGetDevCaps", [null, null]],
    ["timeBeginPeriod", [null]],
    ["timeEndPeriod", [null]],
  );
}
if (selectedObservations.has("com-initialization")) {
  hooks["ole32.dll"].push(["CoUninitialize", []]);
}
if (focusedDragAcceptFilesObservation) {
  for (const moduleName of Object.keys(hooks)) {
    if (moduleName !== "shell32.dll") {
      hooks[moduleName] = [];
    }
  }
}
const idirect3d9Methods = [
  [0, "QueryInterface", 2],
  [1, "AddRef", 0],
  [2, "Release", 0],
  [3, "RegisterSoftwareDevice", 1],
  [4, "GetAdapterCount", 0],
  [5, "GetAdapterIdentifier", 3],
  [6, "GetAdapterModeCount", 2],
  [7, "EnumAdapterModes", 4],
  [8, "GetAdapterDisplayMode", 2],
  [9, "CheckDeviceType", 5],
  [10, "CheckDeviceFormat", 6],
  [11, "CheckDeviceMultiSampleType", 6],
  [12, "CheckDepthStencilMatch", 5],
  [13, "CheckDeviceFormatConversion", 4],
  [14, "GetDeviceCaps", 3],
  [15, "GetAdapterMonitor", 1],
  [16, "CreateDevice", 6],
];

const idirectinput8aMethods = [
  [0, "QueryInterface", 2],
  [1, "AddRef", 0],
  [2, "Release", 0],
  [3, "CreateDevice", 3],
  [4, "EnumDevices", 4],
  [5, "GetDeviceStatus", 1],
  [6, "RunControlPanel", 2],
  [7, "Initialize", 2],
  [8, "FindDevice", 3],
  [9, "EnumDevicesBySemantics", 5],
  [10, "ConfigureDevices", 4],
];

const idirectsound8Methods = [
  [0, "QueryInterface", 2],
  [1, "AddRef", 0],
  [2, "Release", 0],
  [3, "CreateSoundBuffer", 3],
  [4, "GetCaps", 1],
  [5, "DuplicateSoundBuffer", 2],
  [6, "SetCooperativeLevel", 2],
  [7, "Compact", 0],
  [8, "GetSpeakerConfig", 1],
  [9, "SetSpeakerConfig", 1],
  [10, "Initialize", 1],
  [11, "VerifyCertification", 1],
];

const idirectsoundbufferMethods = [
  [0, "QueryInterface", 2],
  [1, "AddRef", 0],
  [2, "Release", 0],
  [3, "GetCaps", 1],
  [4, "GetCurrentPosition", 2],
  [5, "GetFormat", 3],
  [6, "GetVolume", 1],
  [7, "GetPan", 1],
  [8, "GetFrequency", 1],
  [9, "GetStatus", 1],
  [10, "Initialize", 2],
  [11, "Lock", 7],
  [12, "Play", 3],
  [13, "SetCurrentPosition", 1],
  [14, "SetFormat", 1],
  [15, "SetVolume", 1],
  [16, "SetPan", 1],
  [17, "SetFrequency", 1],
  [18, "Stop", 0],
  [19, "Unlock", 4],
  [20, "Restore", 0],
];

const idirect3ddevice9Methods = [
  [0, "QueryInterface", 2],
  [1, "AddRef", 0],
  [2, "Release", 0],
  [3, "TestCooperativeLevel", 0],
  [4, "GetAvailableTextureMem", 0],
  [5, "EvictManagedResources", 0],
  [6, "GetDirect3D", 1],
  [7, "GetDeviceCaps", 1],
  [8, "GetDisplayMode", 2],
  [9, "GetCreationParameters", 1],
  [10, "SetCursorProperties", 3],
  [11, "SetCursorPosition", 3],
  [12, "ShowCursor", 1],
  [13, "CreateAdditionalSwapChain", 2],
  [14, "GetSwapChain", 2],
  [15, "GetNumberOfSwapChains", 0],
  [16, "Reset", 1],
  [17, "Present", 4],
  [18, "GetBackBuffer", 4],
  [19, "GetRasterStatus", 2],
  [20, "SetDialogBoxMode", 1],
  [21, "SetGammaRamp", 3],
  [22, "GetGammaRamp", 2],
  [23, "CreateTexture", 8],
  [24, "CreateVolumeTexture", 9],
  [25, "CreateCubeTexture", 7],
  [26, "CreateVertexBuffer", 6],
  [27, "CreateIndexBuffer", 6],
  [28, "CreateRenderTarget", 8],
  [29, "CreateDepthStencilSurface", 8],
  [30, "UpdateSurface", 4],
  [31, "UpdateTexture", 2],
  [32, "GetRenderTargetData", 2],
  [33, "GetFrontBufferData", 2],
  [34, "StretchRect", 5],
  [35, "ColorFill", 3],
  [36, "CreateOffscreenPlainSurface", 6],
  [37, "SetRenderTarget", 2],
  [38, "GetRenderTarget", 2],
  [39, "SetDepthStencilSurface", 1],
  [40, "GetDepthStencilSurface", 1],
  [41, "BeginScene", 0],
  [42, "EndScene", 0],
  [43, "Clear", 6],
  [44, "SetTransform", 2],
  [45, "GetTransform", 2],
  [46, "MultiplyTransform", 2],
  [47, "SetViewport", 1],
  [48, "GetViewport", 1],
  [49, "SetMaterial", 1],
  [50, "GetMaterial", 1],
  [51, "SetLight", 2],
  [52, "GetLight", 2],
  [53, "LightEnable", 2],
  [54, "GetLightEnable", 2],
  [55, "SetClipPlane", 2],
  [56, "GetClipPlane", 2],
  [57, "SetRenderState", 2],
  [58, "GetRenderState", 2],
  [59, "CreateStateBlock", 2],
  [60, "BeginStateBlock", 0],
  [61, "EndStateBlock", 1],
  [62, "SetClipStatus", 1],
  [63, "GetClipStatus", 1],
  [64, "GetTexture", 2],
  [65, "SetTexture", 2],
  [66, "GetTextureStageState", 3],
  [67, "SetTextureStageState", 3],
  [68, "GetSamplerState", 3],
  [69, "SetSamplerState", 3],
  [70, "ValidateDevice", 1],
  [71, "SetPaletteEntries", 2],
  [72, "GetPaletteEntries", 2],
  [73, "SetCurrentTexturePalette", 1],
  [74, "GetCurrentTexturePalette", 1],
  [75, "SetScissorRect", 1],
  [76, "GetScissorRect", 1],
  [77, "SetSoftwareVertexProcessing", 1],
  [78, "GetSoftwareVertexProcessing", 0],
  [79, "SetNPatchMode", 1],
  [80, "GetNPatchMode", 0],
  [81, "DrawPrimitive", 3],
  [82, "DrawIndexedPrimitive", 6],
  [83, "DrawPrimitiveUP", 4],
  [84, "DrawIndexedPrimitiveUP", 8],
  [85, "ProcessVertices", 6],
  [86, "CreateVertexDeclaration", 2],
  [87, "SetVertexDeclaration", 1],
  [88, "GetVertexDeclaration", 1],
  [89, "SetFVF", 1],
  [90, "GetFVF", 1],
  [91, "CreateVertexShader", 2],
  [92, "SetVertexShader", 1],
  [93, "GetVertexShader", 1],
  [94, "SetVertexShaderConstantF", 3],
  [95, "GetVertexShaderConstantF", 3],
  [96, "SetVertexShaderConstantI", 3],
  [97, "GetVertexShaderConstantI", 3],
  [98, "SetVertexShaderConstantB", 3],
  [99, "GetVertexShaderConstantB", 3],
  [100, "SetStreamSource", 4],
  [101, "GetStreamSource", 4],
  [102, "SetStreamSourceFreq", 2],
  [103, "GetStreamSourceFreq", 2],
  [104, "SetIndices", 1],
  [105, "GetIndices", 1],
  [106, "CreatePixelShader", 2],
  [107, "SetPixelShader", 1],
  [108, "GetPixelShader", 1],
  [109, "SetPixelShaderConstantF", 3],
  [110, "GetPixelShaderConstantF", 3],
  [111, "SetPixelShaderConstantI", 3],
  [112, "GetPixelShaderConstantI", 3],
  [113, "SetPixelShaderConstantB", 3],
  [114, "GetPixelShaderConstantB", 3],
  [115, "DrawRectPatch", 3],
  [116, "DrawTriPatch", 3],
  [117, "DeletePatch", 1],
  [118, "CreateQuery", 2],
];

const idirect3dtexture9Methods = [
  [0, "QueryInterface", 2],
  [1, "AddRef", 0],
  [2, "Release", 0],
  [3, "GetDevice", 1],
  [4, "SetPrivateData", 4],
  [5, "GetPrivateData", 3],
  [6, "FreePrivateData", 1],
  [7, "SetPriority", 1],
  [8, "GetPriority", 0],
  [9, "PreLoad", 0],
  [10, "GetType", 0],
  [11, "SetLOD", 1],
  [12, "GetLOD", 0],
  [13, "GetLevelCount", 0],
  [14, "SetAutoGenFilterType", 1],
  [15, "GetAutoGenFilterType", 0],
  [16, "GenerateMipSubLevels", 0],
  [17, "GetLevelDesc", 2],
  [18, "GetSurfaceLevel", 2],
  [19, "LockRect", 4],
  [20, "UnlockRect", 1],
  [21, "AddDirtyRect", 1],
];

const idirect3dsurface9Methods = [
  [0, "QueryInterface", 2],
  [1, "AddRef", 0],
  [2, "Release", 0],
  [3, "GetDevice", 1],
  [4, "SetPrivateData", 4],
  [5, "GetPrivateData", 3],
  [6, "FreePrivateData", 1],
  [7, "SetPriority", 1],
  [8, "GetPriority", 0],
  [9, "PreLoad", 0],
  [10, "GetType", 0],
  [11, "GetContainer", 2],
  [12, "GetDesc", 1],
  [13, "LockRect", 3],
  [14, "UnlockRect", 0],
  [15, "GetDC", 1],
  [16, "ReleaseDC", 1],
];

const idirect3dvertexbuffer9Methods = [
  [0, "QueryInterface", 2],
  [1, "AddRef", 0],
  [2, "Release", 0],
  [3, "GetDevice", 1],
  [4, "SetPrivateData", 4],
  [5, "GetPrivateData", 3],
  [6, "FreePrivateData", 1],
  [7, "SetPriority", 1],
  [8, "GetPriority", 0],
  [9, "PreLoad", 0],
  [10, "GetType", 0],
  [11, "Lock", 4],
  [12, "Unlock", 0],
  [13, "GetDesc", 1],
];

function findExport(module, name) {
  if (typeof module.findExportByName === "function") {
    return module.findExportByName(name);
  }
  if (typeof Module.findExportByName === "function") {
    return Module.findExportByName(module.name, name);
  }
  return null;
}

function attachGetCurrentThreadCallSites(module) {
  const target = findExport(module, "GetCurrentThread");
  if (target === null) {
    throw new Error("GetCurrentThread export is unavailable");
  }
  const mainModule = Process.mainModule;
  const callSites = [];
  for (const range of mainModule.enumerateRanges("r-x")) {
    for (const match of Memory.scanSync(range.base, range.size, "ff 15")) {
      try {
        const slot = ptr(match.address.add(2).readU32());
        const resolved = slot.readPointer();
        if (resolved.equals(target)) {
          callSites.push({ address: match.address, slot });
        }
      } catch (_) {
        // Ignore unreadable false-positive instruction patterns.
      }
    }
  }
  if (callSites.length === 0) {
    throw new Error("BGI GetCurrentThread IAT call site was not found");
  }
  emit("get-current-thread-callsite-observed", {
    module: mainModule.name,
    target: target.toString(),
    callSites: callSites.map(({ address, slot }) => ({
      callSite: describeCaller(address),
      returnSite: describeCaller(address.add(6)),
      iatAddress: slot.toString(),
      callInstruction: snapshotInstructions(address, 1),
      immediateConsumer: snapshotInstructionsFollowingDirectJump(
        address.add(6),
        16,
      ),
    })),
  });
}

function attachThreadAffinityTickCountObserver(module) {
  const address = findExport(module, "GetTickCount");
  if (address === null) {
    throw new Error("GetTickCount export is unavailable");
  }
  const key = `thread-affinity-tick-count:${address}`;
  if (attached.has(key)) {
    return;
  }
  attached.add(key);
  Interceptor.attach(address, {
    onEnter() {
      const caller = describeCaller(this.returnAddress);
      this.traceThreadAffinityTick =
        setThreadAffinityCalibration?.active === true &&
        caller.module !== null &&
        caller.module.toLowerCase() === "bgi.exe";
      this.traceThreadAffinityTickCaller = caller;
      this.traceThreadAffinityTickReturnAddress = this.returnAddress;
    },
    onLeave(result) {
      if (!this.traceThreadAffinityTick) {
        return;
      }
      const calibration = setThreadAffinityCalibration;
      if (calibration === null || !calibration.active) {
        return;
      }
      const value = result.toUInt32();
      const sample = {
        index: calibration.tickCountCalls + 1,
        value,
        lastError: Number(this.lastError) >>> 0,
      };
      if (calibration.tickCountCalls === 0) {
        sample.caller = this.traceThreadAffinityTickCaller;
        sample.immediateConsumer = snapshotInstructionsFollowingDirectJump(
          this.traceThreadAffinityTickReturnAddress,
          16,
        );
        calibration.firstValue = value;
      } else {
        const delta = (value - calibration.lastValue) >>> 0;
        if (delta !== 0) {
          calibration.valueChanges += 1;
        }
        if (delta > 0x7fff_ffff) {
          calibration.regressions += 1;
        }
      }
      calibration.tickCountCalls += 1;
      calibration.lastValue = value;
      if (calibration.firstSamples.length < 8) {
        calibration.firstSamples.push(sample);
      }
      calibration.lastSamples.push(sample);
      if (calibration.lastSamples.length > 8) {
        calibration.lastSamples.shift();
      }
    },
  });
}

function attachIdirect3dsurface9(object) {
  if (object.isNull()) {
    return;
  }
  try {
    const vtable = object.readPointer();
    const vtableKey = `IDirect3DSurface9:${vtable}`;
    if (observedVtables.has(vtableKey)) {
      return;
    }
    observedVtables.add(vtableKey);
    const slots = [];
    for (const [slot, name, argumentCount] of idirect3dsurface9Methods) {
      const address = vtable.add(slot * Process.pointerSize).readPointer();
      slots.push({ slot, name, address: address.toString() });
      const key = `IDirect3DSurface9:${slot}:${address}`;
      if (attachedVtableMethods.has(key)) {
        continue;
      }
      attachedVtableMethods.add(key);
      const implementationModule = Process.findModuleByAddress(address);
      Interceptor.attach(address, {
        onEnter(args) {
          if (
            focusedD3d9GetSurfaceLevelObservation &&
            focusedD3d9SurfaceMethodCaptured
          ) {
            this.traceFrame = null;
            return;
          }
          const objectAddress = args[0].toString();
          if (focusedD3d9SurfaceGetDcObservation) {
            const caller = describeCaller(this.returnAddress);
            const expectedMethod =
              focusedD3d9SurfaceDcCalls.length === 0
                ? "GetDC"
                : focusedD3d9SurfaceDcCalls.length === 3
                  ? "ReleaseDC"
                  : focusedD3d9SurfaceDcCalls.length === 4
                    ? "Release"
                    : null;
            if (
              focusedD3d9SurfaceDcComplete ||
              objectAddress.toLowerCase() !== focusedD3d9SurfaceObject ||
              caller.module?.toLowerCase() !==
                Process.mainModule.name.toLowerCase() ||
              name !== expectedMethod
            ) {
              this.traceFrame = null;
              return;
            }
          }
          const arguments_ = Array.from({ length: argumentCount }, (_, index) =>
            args[index + 1].toString(),
          );
          this.traceObject = objectAddress;
          this.traceArguments = arguments_;
          this.traceSurfaceDcOutput =
            focusedD3d9SurfaceGetDcObservation && slot === 15 ? args[1] : null;
          this.traceSurfaceGetDc =
            this.traceSurfaceDcOutput === null
              ? null
              : {
                  output: {
                    before: snapshotPointerSlot(this.traceSurfaceDcOutput),
                  },
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    128,
                  ),
                };
          this.traceFrame = beginCall(
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3DSurface9",
              vtableSlot: slot,
              object: objectAddress,
              arguments: arguments_,
            },
            this.returnAddress,
          );
          if (focusedD3d9GetSurfaceLevelObservation) {
            focusedD3d9SurfaceMethodCaptured = true;
          }
        },
        onLeave(result) {
          if (this.traceFrame === null) {
            return;
          }
          const d3d9SurfaceGetDc =
            this.traceSurfaceGetDc === null
              ? null
              : {
                  ...this.traceSurfaceGetDc,
                  output: {
                    ...this.traceSurfaceGetDc.output,
                    after: snapshotPointerSlot(this.traceSurfaceDcOutput),
                  },
                };
          const lastError = Number(this.lastError) >>> 0;
          finishCall(
            this.traceFrame,
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3DSurface9",
              vtableSlot: slot,
              object: this.traceObject,
              result: result.toString(),
              ...(d3d9SurfaceGetDc === null ? {} : { d3d9SurfaceGetDc }),
            },
            this.lastError,
          );
          if (focusedD3d9GetSurfaceLevelObservation) {
            emit("d3d9-get-surface-level-complete", {
              interface: "IDirect3DSurface9",
              vtableSlot: slot,
              method: name,
              result: result.toString(),
            });
          }
          if (focusedD3d9SurfaceGetDcObservation) {
            focusedD3d9SurfaceDcCalls.push({
              api: name,
              interface: "IDirect3DSurface9",
              vtableSlot: slot,
              object: this.traceObject,
              arguments: this.traceArguments,
              result: result.toString(),
              lastError,
              ...(d3d9SurfaceGetDc === null ? {} : { d3d9SurfaceGetDc }),
              caller: this.traceFrame.caller,
            });
            maybeCompleteFocusedD3d9SurfaceDcObservation();
          }
        },
      });
    }
    emit("vtable-observed", {
      interface: "IDirect3DSurface9",
      object: object.toString(),
      vtable: vtable.toString(),
      slots,
    });
  } catch (error) {
    emit("vtable-error", {
      interface: "IDirect3DSurface9",
      object: object.toString(),
      error: error.message,
    });
  }
}

function attachD3d9SurfaceDcGdiHook(module, name) {
  const address = findExport(module, name);
  if (address === null) {
    throw new Error(`${name} export is unavailable`);
  }
  const key = `d3d9-surface-dc:${name}:${address}`;
  if (attached.has(key)) {
    return;
  }
  attached.add(key);
  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const expectedApi =
        focusedD3d9SurfaceDcCalls.length === 1
          ? "GetStockObject"
          : focusedD3d9SurfaceDcCalls.length === 2
            ? "FillRect"
            : null;
      if (
        !focusedD3d9SurfaceGetDcObservation ||
        focusedD3d9SurfaceDcComplete ||
        caller.module?.toLowerCase() !==
          Process.mainModule.name.toLowerCase() ||
        name !== expectedApi
      ) {
        this.traceFrame = null;
        return;
      }
      this.traceArguments =
        name === "GetStockObject"
          ? [signed32(args[0])]
          : [args[0].toString(), args[1].toString(), args[2].toString()];
      this.traceRect = name === "FillRect" ? snapshotRect(args[1]) : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: this.traceArguments,
          d3d9SurfaceDcGdi: {
            rect: this.traceRect,
            immediateConsumer: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              64,
            ),
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          d3d9SurfaceDcGdi: { rect: this.traceRect },
        },
        this.lastError,
      );
      focusedD3d9SurfaceDcCalls.push({
        api: name,
        module: module.name,
        arguments: this.traceArguments,
        result: result.toString(),
        lastError,
        rect: this.traceRect,
        caller: this.traceFrame.caller,
      });
      maybeCompleteFocusedD3d9SurfaceDcObservation();
    },
  });
}

function maybeCompleteFocusedD3d9SurfaceDcObservation() {
  if (focusedD3d9SurfaceDcComplete) {
    return;
  }
  const expected = [
    "GetDC",
    "GetStockObject",
    "FillRect",
    "ReleaseDC",
    "Release",
  ];
  if (
    focusedD3d9SurfaceDcCalls.length !== expected.length ||
    !expected.every(
      (api, index) => focusedD3d9SurfaceDcCalls[index]?.api === api,
    )
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteFocusedD3d9SurfaceDcObservation, 10);
    return;
  }
  focusedD3d9SurfaceDcComplete = true;
  emit("d3d9-surface-get-dc-complete", {
    profile: "x86-bgi-system-memory-surface-dc-fill-v1",
    surface: focusedD3d9SurfaceObject,
    calls: focusedD3d9SurfaceDcCalls,
  });
  if (stopOnEvent === "d3d9-surface-get-dc-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function attachIdirect3dtexture9(object) {
  if (object.isNull()) {
    return;
  }
  try {
    const vtable = object.readPointer();
    const vtableKey = `IDirect3DTexture9:${vtable}`;
    if (observedVtables.has(vtableKey)) {
      return;
    }
    observedVtables.add(vtableKey);
    const slots = [];
    for (const [slot, name, argumentCount] of idirect3dtexture9Methods) {
      const address = vtable.add(slot * Process.pointerSize).readPointer();
      slots.push({ slot, name, address: address.toString() });
      const key = `IDirect3DTexture9:${slot}:${address}`;
      if (attachedVtableMethods.has(key)) {
        continue;
      }
      attachedVtableMethods.add(key);
      const implementationModule = Process.findModuleByAddress(address);
      Interceptor.attach(address, {
        onEnter(args) {
          if (
            focusedD3d9ResourceObservation &&
            focusedD3d9TextureMethodCaptured
          ) {
            this.traceFrame = null;
            return;
          }
          const objectAddress = args[0].toString();
          const arguments_ = Array.from({ length: argumentCount }, (_, index) =>
            args[index + 1].toString(),
          );
          this.traceObject = objectAddress;
          this.traceD3d9SurfaceLevelOutput = slot === 18 ? args[2] : null;
          this.traceD3d9GetSurfaceLevel =
            this.traceD3d9SurfaceLevelOutput === null
              ? null
              : {
                  level: args[1].toUInt32(),
                  surfaceOutput: {
                    before: snapshotPointerSlot(
                      this.traceD3d9SurfaceLevelOutput,
                    ),
                  },
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    128,
                  ),
                };
          this.traceFrame = beginCall(
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3DTexture9",
              vtableSlot: slot,
              object: objectAddress,
              arguments: arguments_,
            },
            this.returnAddress,
          );
        },
        onLeave(result) {
          if (this.traceFrame === null) {
            return;
          }
          const d3d9GetSurfaceLevel =
            this.traceD3d9GetSurfaceLevel === null
              ? null
              : {
                  ...this.traceD3d9GetSurfaceLevel,
                  surfaceOutput: {
                    ...this.traceD3d9GetSurfaceLevel.surfaceOutput,
                    after: snapshotPointerSlot(
                      this.traceD3d9SurfaceLevelOutput,
                    ),
                  },
                };
          finishCall(
            this.traceFrame,
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3DTexture9",
              vtableSlot: slot,
              object: this.traceObject,
              result: result.toString(),
              ...(d3d9GetSurfaceLevel === null ? {} : { d3d9GetSurfaceLevel }),
            },
            this.lastError,
          );
          if (
            d3d9GetSurfaceLevel !== null &&
            result.toInt32() >= 0 &&
            typeof d3d9GetSurfaceLevel.surfaceOutput.after.value === "string" &&
            d3d9GetSurfaceLevel.surfaceOutput.after.value !== "0x0"
          ) {
            if (focusedD3d9SurfaceGetDcObservation) {
              focusedD3d9SurfaceObject =
                d3d9GetSurfaceLevel.surfaceOutput.after.value.toLowerCase();
            }
            attachIdirect3dsurface9(
              ptr(d3d9GetSurfaceLevel.surfaceOutput.after.value),
            );
          }
          if (focusedD3d9ResourceObservation) {
            focusedD3d9TextureMethodCaptured = true;
          }
        },
      });
    }
    emit("vtable-observed", {
      interface: "IDirect3DTexture9",
      object: object.toString(),
      vtable: vtable.toString(),
      slots,
    });
  } catch (error) {
    emit("vtable-error", {
      interface: "IDirect3DTexture9",
      object: object.toString(),
      error: error.message,
    });
  }
}

function attachIdirect3dvertexbuffer9(object) {
  if (object.isNull()) {
    return;
  }
  try {
    const vtable = object.readPointer();
    const vtableKey = `IDirect3DVertexBuffer9:${vtable}`;
    if (observedVtables.has(vtableKey)) {
      return;
    }
    observedVtables.add(vtableKey);
    const slots = [];
    for (const [slot, name, argumentCount] of idirect3dvertexbuffer9Methods) {
      const address = vtable.add(slot * Process.pointerSize).readPointer();
      slots.push({ slot, name, address: address.toString() });
      const key = `IDirect3DVertexBuffer9:${slot}:${address}`;
      if (attachedVtableMethods.has(key)) {
        continue;
      }
      attachedVtableMethods.add(key);
      const implementationModule = Process.findModuleByAddress(address);
      Interceptor.attach(address, {
        onEnter(args) {
          const caller = describeCaller(this.returnAddress);
          const objectAddress = args[0].toString().toLowerCase();
          const expectedMethod =
            focusedD3d9InitialVertexBufferCalls.length === 1
              ? "Lock"
              : focusedD3d9InitialVertexBufferCalls.length === 2
                ? "Unlock"
                : null;
          if (
            !focusedD3d9InitialVertexBufferObservation ||
            focusedD3d9InitialVertexBufferComplete ||
            objectAddress !== focusedD3d9InitialVertexBufferObject ||
            caller.module?.toLowerCase() !==
              Process.mainModule.name.toLowerCase() ||
            name !== expectedMethod
          ) {
            this.traceFrame = null;
            return;
          }
          const arguments_ = Array.from({ length: argumentCount }, (_, index) =>
            args[index + 1].toString(),
          );
          this.traceObject = objectAddress;
          this.traceArguments = arguments_;
          this.traceVertexBufferLockOutput = slot === 11 ? args[3] : null;
          this.traceD3d9VertexBufferLock =
            this.traceVertexBufferLockOutput === null
              ? null
              : {
                  dataOutput: {
                    before: snapshotPointerSlot(
                      this.traceVertexBufferLockOutput,
                    ),
                  },
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    256,
                  ),
                };
          this.traceD3d9VertexBufferUnlock =
            slot === 12
              ? {
                  dataAtUnlock: snapshotBoundedRawBytes(
                    ptr(focusedD3d9InitialVertexBufferData),
                    focusedD3d9InitialVertexBufferLength,
                    112,
                  ),
                }
              : null;
          this.traceFrame = beginCall(
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3DVertexBuffer9",
              vtableSlot: slot,
              object: objectAddress,
              arguments: arguments_,
            },
            this.returnAddress,
          );
        },
        onLeave(result) {
          if (this.traceFrame === null) {
            return;
          }
          const d3d9VertexBufferLock =
            this.traceD3d9VertexBufferLock === null
              ? null
              : {
                  ...this.traceD3d9VertexBufferLock,
                  dataOutput: {
                    ...this.traceD3d9VertexBufferLock.dataOutput,
                    after: snapshotPointerSlot(
                      this.traceVertexBufferLockOutput,
                    ),
                  },
                };
          if (
            d3d9VertexBufferLock !== null &&
            result.toInt32() >= 0 &&
            typeof d3d9VertexBufferLock.dataOutput.after.value === "string" &&
            d3d9VertexBufferLock.dataOutput.after.value !== "0x0"
          ) {
            focusedD3d9InitialVertexBufferData =
              d3d9VertexBufferLock.dataOutput.after.value.toLowerCase();
            d3d9VertexBufferLock.dataBefore = snapshotBoundedRawBytes(
              ptr(focusedD3d9InitialVertexBufferData),
              focusedD3d9InitialVertexBufferLength,
              112,
            );
          }
          const lastError = Number(this.lastError) >>> 0;
          finishCall(
            this.traceFrame,
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3DVertexBuffer9",
              vtableSlot: slot,
              object: this.traceObject,
              result: result.toString(),
              ...(d3d9VertexBufferLock === null
                ? {}
                : { d3d9VertexBufferLock }),
              ...(this.traceD3d9VertexBufferUnlock === null
                ? {}
                : {
                    d3d9VertexBufferUnlock: this.traceD3d9VertexBufferUnlock,
                  }),
            },
            this.lastError,
          );
          focusedD3d9InitialVertexBufferCalls.push({
            callId: this.traceFrame.callId,
            api: name,
            interface: "IDirect3DVertexBuffer9",
            vtableSlot: slot,
            object: this.traceObject,
            arguments: this.traceArguments,
            result: result.toString(),
            lastError,
            ...(d3d9VertexBufferLock === null ? {} : { d3d9VertexBufferLock }),
            ...(this.traceD3d9VertexBufferUnlock === null
              ? {}
              : {
                  d3d9VertexBufferUnlock: this.traceD3d9VertexBufferUnlock,
                }),
            caller: this.traceFrame.caller,
          });
          maybeCompleteFocusedD3d9InitialVertexBufferObservation();
        },
      });
    }
    emit("vtable-observed", {
      interface: "IDirect3DVertexBuffer9",
      object: object.toString(),
      vtable: vtable.toString(),
      slots,
    });
  } catch (error) {
    emit("vtable-error", {
      interface: "IDirect3DVertexBuffer9",
      object: object.toString(),
      error: error.message,
    });
  }
}

function attachIdirect3ddevice9(object) {
  if (object.isNull()) {
    return;
  }
  try {
    const vtable = object.readPointer();
    const vtableKey = `IDirect3DDevice9:${vtable}`;
    if (observedVtables.has(vtableKey)) {
      return;
    }
    observedVtables.add(vtableKey);
    const slots = [];
    for (const [slot, name, argumentCount] of idirect3ddevice9Methods) {
      const address = vtable.add(slot * Process.pointerSize).readPointer();
      slots.push({ slot, name, address: address.toString() });
      const key = `IDirect3DDevice9:${slot}:${address}`;
      if (attachedVtableMethods.has(key)) {
        continue;
      }
      attachedVtableMethods.add(key);
      const implementationModule = Process.findModuleByAddress(address);
      Interceptor.attach(address, {
        onEnter(args) {
          if (focusedD3d9RasterStatusObservation) {
            const caller = describeCaller(this.returnAddress);
            const objectAddress = args[0].toString().toLowerCase();
            if (
              focusedD3d9RasterStatusComplete ||
              focusedD3d9RasterStatusCalls.length !== 0 ||
              slot !== 19 ||
              caller.module?.toLowerCase() !==
                Process.mainModule.name.toLowerCase() ||
              (focusedD3d9RasterStatusDevice !== null &&
                objectAddress !== focusedD3d9RasterStatusDevice)
            ) {
              this.traceFrame = null;
              return;
            }
            focusedD3d9RasterStatusDevice ??= objectAddress;
          } else if (focusedD3d9InitialRenderStatesObservation) {
            const caller = describeCaller(this.returnAddress);
            const objectAddress = args[0].toString().toLowerCase();
            if (
              focusedD3d9InitialRenderStatesComplete ||
              focusedD3d9InitialRenderStateCalls.length >= 4 ||
              slot !== 57 ||
              caller.module?.toLowerCase() !==
                Process.mainModule.name.toLowerCase() ||
              (focusedD3d9InitialRenderStateDevice !== null &&
                objectAddress !== focusedD3d9InitialRenderStateDevice)
            ) {
              this.traceFrame = null;
              return;
            }
            focusedD3d9InitialRenderStateDevice ??= objectAddress;
          } else if (focusedD3d9InitialVertexBufferObservation) {
            const caller = describeCaller(this.returnAddress);
            const objectAddress = args[0].toString().toLowerCase();
            if (
              focusedD3d9InitialVertexBufferComplete ||
              focusedD3d9InitialVertexBufferCalls.length !== 0 ||
              slot !== 26 ||
              caller.module?.toLowerCase() !==
                Process.mainModule.name.toLowerCase() ||
              (focusedD3d9InitialVertexBufferDevice !== null &&
                objectAddress !== focusedD3d9InitialVertexBufferDevice)
            ) {
              this.traceFrame = null;
              return;
            }
            focusedD3d9InitialVertexBufferDevice ??= objectAddress;
          } else if (focusedD3d9InitialTexturesObservation) {
            const caller = describeCaller(this.returnAddress);
            const objectAddress = args[0].toString().toLowerCase();
            if (
              focusedD3d9InitialTexturesComplete ||
              focusedD3d9InitialTextureCalls.length >= 3 ||
              slot !== 23 ||
              caller.module?.toLowerCase() !==
                Process.mainModule.name.toLowerCase() ||
              (focusedD3d9InitialTextureDevice !== null &&
                objectAddress !== focusedD3d9InitialTextureDevice)
            ) {
              this.traceFrame = null;
              return;
            }
            focusedD3d9InitialTextureDevice ??= objectAddress;
          } else if (focusedD3d9RecreatedTexturesObservation) {
            const caller = describeCaller(this.returnAddress);
            const objectAddress = args[0].toString().toLowerCase();
            const startsRecreatedSequence =
              slot === 23 &&
              caller.module?.toLowerCase() ===
                Process.mainModule.name.toLowerCase() &&
              caller.rva === 0x5a715 &&
              args[1].toUInt32() === 2_048 &&
              args[2].toUInt32() === 1_024 &&
              args[3].toUInt32() === 1 &&
              args[4].toUInt32() === 0 &&
              args[5].toUInt32() === 22 &&
              args[6].toUInt32() === 2 &&
              args[8].isNull();
            if (
              focusedD3d9RecreatedTexturesComplete ||
              focusedD3d9RecreatedTextureCalls.length >= 3 ||
              slot !== 23 ||
              caller.module?.toLowerCase() !==
                Process.mainModule.name.toLowerCase() ||
              (!focusedD3d9RecreatedTexturesStarted &&
                !startsRecreatedSequence) ||
              (focusedD3d9RecreatedTextureDevice !== null &&
                objectAddress !== focusedD3d9RecreatedTextureDevice)
            ) {
              this.traceFrame = null;
              return;
            }
            focusedD3d9RecreatedTexturesStarted = true;
            focusedD3d9RecreatedTextureDevice ??= objectAddress;
          } else if (
            focusedD3d9DeviceObservation &&
            focusedD3d9DeviceMethodCaptured
          ) {
            this.traceFrame = null;
            return;
          }
          const objectAddress = args[0].toString();
          const arguments_ = Array.from({ length: argumentCount }, (_, index) =>
            args[index + 1].toString(),
          );
          this.traceArguments = arguments_;
          this.traceD3d9CreateTextureOutput =
            (focusedD3d9ResourceObservation ||
              focusedD3d9InitialTexturesObservation ||
              focusedD3d9RecreatedTexturesObservation) &&
            slot === 23
              ? args[7]
              : null;
          this.traceD3d9CreateTexture =
            this.traceD3d9CreateTextureOutput === null
              ? null
              : {
                  textureOutput: {
                    before: snapshotPointerSlot(
                      this.traceD3d9CreateTextureOutput,
                    ),
                  },
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    128,
                  ),
                };
          this.traceD3d9CreateVertexBufferOutput =
            focusedD3d9InitialVertexBufferObservation && slot === 26
              ? args[5]
              : null;
          this.traceD3d9CreateVertexBuffer =
            this.traceD3d9CreateVertexBufferOutput === null
              ? null
              : {
                  vertexBufferOutput: {
                    before: snapshotPointerSlot(
                      this.traceD3d9CreateVertexBufferOutput,
                    ),
                  },
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    256,
                  ),
                };
          this.traceD3d9RasterStatusOutput =
            focusedD3d9RasterStatusObservation && slot === 19 ? args[2] : null;
          this.traceD3d9RasterStatus =
            this.traceD3d9RasterStatusOutput === null
              ? null
              : {
                  swapChain: args[1].toUInt32(),
                  before: snapshotD3d9RasterStatus(
                    this.traceD3d9RasterStatusOutput,
                  ),
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    256,
                  ),
                  outerConsumer: snapshotInstructionsFollowingDirectJump(
                    this.context.ebp.add(Process.pointerSize).readPointer(),
                    128,
                  ),
                };
          this.traceObject = objectAddress;
          this.traceFrame = beginCall(
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3DDevice9",
              vtableSlot: slot,
              object: objectAddress,
              arguments: arguments_,
            },
            this.returnAddress,
          );
        },
        onLeave(result) {
          if (this.traceFrame === null) {
            return;
          }
          const d3d9CreateTexture =
            this.traceD3d9CreateTexture === null
              ? null
              : {
                  ...this.traceD3d9CreateTexture,
                  textureOutput: {
                    ...this.traceD3d9CreateTexture.textureOutput,
                    after: snapshotPointerSlot(
                      this.traceD3d9CreateTextureOutput,
                    ),
                  },
                };
          const d3d9CreateVertexBuffer =
            this.traceD3d9CreateVertexBuffer === null
              ? null
              : {
                  ...this.traceD3d9CreateVertexBuffer,
                  vertexBufferOutput: {
                    ...this.traceD3d9CreateVertexBuffer.vertexBufferOutput,
                    after: snapshotPointerSlot(
                      this.traceD3d9CreateVertexBufferOutput,
                    ),
                  },
                };
          const d3d9RasterStatus =
            this.traceD3d9RasterStatus === null
              ? null
              : {
                  ...this.traceD3d9RasterStatus,
                  after: snapshotD3d9RasterStatus(
                    this.traceD3d9RasterStatusOutput,
                  ),
                };
          const lastError = Number(this.lastError) >>> 0;
          finishCall(
            this.traceFrame,
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3DDevice9",
              vtableSlot: slot,
              object: this.traceObject,
              result: result.toString(),
              ...(d3d9CreateTexture === null ? {} : { d3d9CreateTexture }),
              ...(d3d9CreateVertexBuffer === null
                ? {}
                : { d3d9CreateVertexBuffer }),
              ...(d3d9RasterStatus === null ? {} : { d3d9RasterStatus }),
            },
            this.lastError,
          );
          if (focusedD3d9InitialTexturesObservation) {
            focusedD3d9InitialTextureCalls.push({
              callId: this.traceFrame.callId,
              api: name,
              interface: "IDirect3DDevice9",
              vtableSlot: slot,
              object: this.traceObject,
              arguments: this.traceArguments,
              result: result.toString(),
              lastError,
              d3d9CreateTexture,
              caller: this.traceFrame.caller,
            });
            maybeCompleteFocusedD3d9InitialTexturesObservation();
          }
          if (focusedD3d9RecreatedTexturesObservation) {
            focusedD3d9RecreatedTextureCalls.push({
              callId: this.traceFrame.callId,
              api: name,
              interface: "IDirect3DDevice9",
              vtableSlot: slot,
              object: this.traceObject,
              arguments: this.traceArguments,
              result: result.toString(),
              lastError,
              d3d9CreateTexture,
              caller: this.traceFrame.caller,
            });
            maybeCompleteFocusedD3d9RecreatedTexturesObservation();
          }
          if (focusedD3d9InitialVertexBufferObservation) {
            focusedD3d9InitialVertexBufferCalls.push({
              callId: this.traceFrame.callId,
              api: name,
              interface: "IDirect3DDevice9",
              vtableSlot: slot,
              object: this.traceObject,
              arguments: this.traceArguments,
              result: result.toString(),
              lastError,
              d3d9CreateVertexBuffer,
              caller: this.traceFrame.caller,
            });
            if (
              d3d9CreateVertexBuffer !== null &&
              result.toInt32() >= 0 &&
              typeof d3d9CreateVertexBuffer.vertexBufferOutput.after.value ===
                "string" &&
              d3d9CreateVertexBuffer.vertexBufferOutput.after.value !== "0x0"
            ) {
              focusedD3d9InitialVertexBufferObject =
                d3d9CreateVertexBuffer.vertexBufferOutput.after.value.toLowerCase();
              focusedD3d9InitialVertexBufferLength = Number(
                BigInt(this.traceArguments[0]),
              );
              attachIdirect3dvertexbuffer9(
                ptr(focusedD3d9InitialVertexBufferObject),
              );
            }
          }
          if (focusedD3d9InitialRenderStatesObservation) {
            focusedD3d9InitialRenderStateCalls.push({
              callId: this.traceFrame.callId,
              api: name,
              interface: "IDirect3DDevice9",
              vtableSlot: slot,
              object: this.traceObject,
              arguments: this.traceArguments,
              result: result.toString(),
              lastError,
              caller: this.traceFrame.caller,
            });
            maybeCompleteFocusedD3d9InitialRenderStatesObservation();
          }
          if (focusedD3d9RasterStatusObservation) {
            focusedD3d9RasterStatusCalls.push({
              callId: this.traceFrame.callId,
              api: name,
              interface: "IDirect3DDevice9",
              vtableSlot: slot,
              object: this.traceObject,
              arguments: this.traceArguments,
              result: result.toString(),
              lastError,
              d3d9RasterStatus,
              caller: this.traceFrame.caller,
            });
            maybeCompleteFocusedD3d9RasterStatusObservation();
          }
          if (
            d3d9CreateTexture !== null &&
            !focusedD3d9InitialTexturesObservation &&
            result.toInt32() >= 0 &&
            typeof d3d9CreateTexture.textureOutput.after.value === "string" &&
            d3d9CreateTexture.textureOutput.after.value !== "0x0"
          ) {
            attachIdirect3dtexture9(
              ptr(d3d9CreateTexture.textureOutput.after.value),
            );
          }
          if (
            focusedD3d9DeviceObservation &&
            !focusedD3d9InitialTexturesObservation &&
            !focusedD3d9RecreatedTexturesObservation &&
            !focusedD3d9InitialVertexBufferObservation &&
            !focusedD3d9InitialRenderStatesObservation &&
            !focusedD3d9RasterStatusObservation
          ) {
            focusedD3d9DeviceMethodCaptured = true;
          }
        },
      });
    }
    emit("vtable-observed", {
      interface: "IDirect3DDevice9",
      object: object.toString(),
      vtable: vtable.toString(),
      slots,
    });
  } catch (error) {
    emit("vtable-error", {
      interface: "IDirect3DDevice9",
      object: object.toString(),
      error: error.message,
    });
  }
}

function attachIdirectsoundbuffer(object) {
  if (
    object.isNull() ||
    (!focusedDsoundCreateBufferObservation &&
      !focusedDsoundBufferLockUnlockPlayObservation)
  ) {
    return;
  }
  try {
    const objectAddress = object.toString().toLowerCase();
    const vtable = object.readPointer();
    const vtableKey = `IDirectSoundBuffer:${vtable}`;
    if (observedVtables.has(vtableKey)) {
      focusedDsoundBufferObjects.add(objectAddress);
      return;
    }
    observedVtables.add(vtableKey);
    focusedDsoundBufferObjects.add(objectAddress);
    const slots = [];
    for (const [slot, name, argumentCount] of idirectsoundbufferMethods) {
      const address = vtable.add(slot * Process.pointerSize).readPointer();
      slots.push({ slot, name, address: address.toString() });
      const key = `IDirectSoundBuffer:${slot}:${address}`;
      if (attachedVtableMethods.has(key)) {
        continue;
      }
      attachedVtableMethods.add(key);
      const implementationModule = Process.findModuleByAddress(address);
      Interceptor.attach(address, {
        onEnter(args) {
          const caller = describeCaller(this.returnAddress);
          const calledObject = args[0].toString().toLowerCase();
          const maximumBufferCalls =
            focusedDsoundBufferLockUnlockPlayObservation ? 7 : 5;
          if (
            (focusedDsoundCreateBufferObservation &&
              focusedDsoundCreateBufferComplete) ||
            (focusedDsoundBufferLockUnlockPlayObservation &&
              focusedDsoundBufferLockUnlockPlayComplete) ||
            focusedDsoundBufferCalls.length >= maximumBufferCalls ||
            !focusedDsoundBufferObjects.has(calledObject) ||
            caller.module?.toLowerCase() !==
              Process.mainModule.name.toLowerCase()
          ) {
            this.traceFrame = null;
            return;
          }
          this.traceObject = calledObject;
          this.traceArguments = Array.from(
            { length: argumentCount },
            (_, index) => args[index + 1].toString(),
          );
          this.traceDsoundBufferQueryOutput = slot === 0 ? args[2] : null;
          this.traceDsoundBufferLockOutputs =
            slot === 11
              ? {
                  data1: args[3],
                  bytes1: args[4],
                  data2: args[5],
                  bytes2: args[6],
                }
              : null;
          this.traceDsoundBufferUnlock =
            slot === 19
              ? {
                  data1: args[1].toString(),
                  bytes1: args[2].toUInt32(),
                  data2: args[3].toString(),
                  bytes2: args[4].toUInt32(),
                  activeLock: focusedDsoundActiveLock,
                  firstSegmentContent: snapshotDsoundBufferContent(
                    args[1],
                    args[2].toUInt32(),
                  ),
                  secondSegmentContent: snapshotDsoundBufferContent(
                    args[3],
                    args[4].toUInt32(),
                  ),
                }
              : null;
          this.traceDsoundBufferMethod = {
            ...(slot === 0
              ? {
                  queryInterface: {
                    iid: snapshotBoundedRawBytes(args[1], 16, 16),
                    output: {
                      address: args[2].toString(),
                      before: snapshotPointerSlot(args[2]),
                    },
                  },
                }
              : {}),
            ...(slot === 14
              ? { waveFormat: snapshotWaveFormatEx(args[1]) }
              : {}),
            ...(this.traceDsoundBufferLockOutputs === null
              ? {}
              : {
                  lock: {
                    offset: args[1].toUInt32(),
                    requestedBytes: args[2].toUInt32(),
                    flags: args[7].toUInt32(),
                    outputsBefore: {
                      data1: snapshotPointerSlot(args[3]),
                      bytes1: snapshotPointerSlot(args[4]),
                      data2: snapshotPointerSlot(args[5]),
                      bytes2: snapshotPointerSlot(args[6]),
                    },
                  },
                }),
            ...(this.traceDsoundBufferUnlock === null
              ? {}
              : { unlock: this.traceDsoundBufferUnlock }),
            immediateConsumer: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              96,
            ),
          };
          this.traceFrame = beginCall(
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirectSoundBuffer",
              vtableSlot: slot,
              object: this.traceObject,
              arguments: this.traceArguments,
            },
            this.returnAddress,
          );
        },
        onLeave(result) {
          if (this.traceFrame === null) {
            return;
          }
          const lastError = Number(this.lastError) >>> 0;
          let directSoundBufferMethod =
            this.traceDsoundBufferQueryOutput === null
              ? this.traceDsoundBufferMethod
              : {
                  ...this.traceDsoundBufferMethod,
                  queryInterface: {
                    ...this.traceDsoundBufferMethod.queryInterface,
                    output: {
                      ...this.traceDsoundBufferMethod.queryInterface.output,
                      after: snapshotPointerSlot(
                        this.traceDsoundBufferQueryOutput,
                      ),
                    },
                  },
                };
          if (this.traceDsoundBufferLockOutputs !== null) {
            directSoundBufferMethod = {
              ...directSoundBufferMethod,
              lock: {
                ...directSoundBufferMethod.lock,
                outputsAfter: {
                  data1: snapshotPointerSlot(
                    this.traceDsoundBufferLockOutputs.data1,
                  ),
                  bytes1: snapshotPointerSlot(
                    this.traceDsoundBufferLockOutputs.bytes1,
                  ),
                  data2: snapshotPointerSlot(
                    this.traceDsoundBufferLockOutputs.data2,
                  ),
                  bytes2: snapshotPointerSlot(
                    this.traceDsoundBufferLockOutputs.bytes2,
                  ),
                },
              },
            };
          }
          if (
            this.traceDsoundBufferLockOutputs !== null &&
            result.toInt32() >= 0
          ) {
            const outputs = directSoundBufferMethod.lock.outputsAfter;
            focusedDsoundActiveLock = {
              object: this.traceObject,
              data1: outputs.data1.value?.toLowerCase() ?? null,
              bytes1:
                outputs.bytes1.value === undefined
                  ? null
                  : ptr(outputs.bytes1.value).toUInt32(),
              data2: outputs.data2.value?.toLowerCase() ?? null,
              bytes2:
                outputs.bytes2.value === undefined
                  ? null
                  : ptr(outputs.bytes2.value).toUInt32(),
            };
          }
          finishCall(
            this.traceFrame,
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirectSoundBuffer",
              vtableSlot: slot,
              object: this.traceObject,
              result: result.toString(),
              directSoundBufferMethod,
            },
            lastError,
          );
          focusedDsoundBufferCalls.push({
            callId: this.traceFrame.callId,
            caller: this.traceFrame.caller,
            returnAddress: this.returnAddress.toString(),
            api: name,
            interface: "IDirectSoundBuffer",
            vtableSlot: slot,
            object: this.traceObject,
            arguments: this.traceArguments,
            result: result.toString(),
            signedResult: result.toInt32(),
            lastError,
            directSoundBufferMethod,
          });
          const queryOutput =
            directSoundBufferMethod.queryInterface?.output?.after?.value;
          if (
            slot === 0 &&
            result.toInt32() >= 0 &&
            typeof queryOutput === "string" &&
            queryOutput !== "0x0"
          ) {
            attachIdirectsoundbuffer(ptr(queryOutput));
          }
          setImmediate(maybeCompleteFocusedDsoundCreateBufferObservation);
          setImmediate(
            maybeCompleteFocusedDsoundBufferLockUnlockPlayObservation,
          );
        },
      });
    }
    emit("vtable-observed", {
      interface: "IDirectSoundBuffer",
      object: objectAddress,
      vtable: vtable.toString(),
      slots,
    });
  } catch (error) {
    emit("vtable-error", {
      interface: "IDirectSoundBuffer",
      object: object.toString(),
      error: error.message,
    });
  }
}

function attachIdirectsound8(object) {
  if (object.isNull()) {
    return;
  }
  try {
    const vtable = object.readPointer();
    const vtableKey = `IDirectSound8:${vtable}`;
    if (observedVtables.has(vtableKey)) {
      return;
    }
    observedVtables.add(vtableKey);
    const slots = [];
    for (const [slot, name, argumentCount] of idirectsound8Methods) {
      const address = vtable.add(slot * Process.pointerSize).readPointer();
      slots.push({ slot, name, address: address.toString() });
      const key = `IDirectSound8:${slot}:${address}`;
      if (attachedVtableMethods.has(key)) {
        continue;
      }
      attachedVtableMethods.add(key);
      const implementationModule = Process.findModuleByAddress(address);
      Interceptor.attach(address, {
        onEnter(args) {
          const caller = describeCaller(this.returnAddress);
          if (
            focusedDsoundObservation &&
            (caller.module?.toLowerCase() !==
              Process.mainModule.name.toLowerCase() ||
              (focusedDsoundRootObservation && focusedDsoundRootComplete) ||
              (focusedDsoundCreateBufferObservation &&
                (focusedDsoundCreateBufferComplete ||
                  slot !== 3 ||
                  focusedDsoundCreateBufferCalls.length >= 2)) ||
              (focusedDsoundBufferLockUnlockPlayObservation &&
                (focusedDsoundBufferLockUnlockPlayComplete ||
                  slot !== 3 ||
                  focusedDsoundCreateBufferCalls.length >= 2)))
          ) {
            this.traceFrame = null;
            return;
          }
          this.traceObject = args[0].toString();
          this.traceArguments = Array.from(
            { length: argumentCount },
            (_, index) => args[index + 1].toString(),
          );
          this.traceDsoundSetCooperativeLevel =
            slot === 6
              ? {
                  window: args[1].toString(),
                  level: args[2].toUInt32(),
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    96,
                  ),
                }
              : null;
          this.traceDsoundCreateBufferOutput =
            (focusedDsoundCreateBufferObservation ||
              focusedDsoundBufferLockUnlockPlayObservation) &&
            slot === 3
              ? args[2]
              : null;
          this.traceDsoundCreateBuffer =
            this.traceDsoundCreateBufferOutput === null
              ? null
              : {
                  descriptor: snapshotDirectSoundBufferDesc(args[1]),
                  output: {
                    address: args[2].toString(),
                    before: snapshotPointerSlot(args[2]),
                  },
                  outerUnknown: args[3].toString(),
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    128,
                  ),
                };
          this.traceFrame = beginCall(
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirectSound8",
              vtableSlot: slot,
              object: this.traceObject,
              arguments: this.traceArguments,
            },
            this.returnAddress,
          );
        },
        onLeave(result) {
          if (this.traceFrame === null) {
            return;
          }
          const lastError = Number(this.lastError) >>> 0;
          const directSoundCreateBuffer =
            this.traceDsoundCreateBuffer === null
              ? null
              : {
                  ...this.traceDsoundCreateBuffer,
                  output: {
                    ...this.traceDsoundCreateBuffer.output,
                    after: snapshotPointerSlot(
                      this.traceDsoundCreateBufferOutput,
                    ),
                  },
                };
          finishCall(
            this.traceFrame,
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirectSound8",
              vtableSlot: slot,
              object: this.traceObject,
              result: result.toString(),
              ...(this.traceDsoundSetCooperativeLevel === null
                ? {}
                : {
                    directSoundSetCooperativeLevel:
                      this.traceDsoundSetCooperativeLevel,
                  }),
              ...(directSoundCreateBuffer === null
                ? {}
                : { directSoundCreateBuffer }),
            },
            lastError,
          );
          if (
            focusedDsoundRootObservation &&
            focusedDsoundRootCalls.length === 0
          ) {
            focusedDsoundRootCalls.push({
              callId: this.traceFrame.callId,
              caller: this.traceFrame.caller,
              returnAddress: this.returnAddress.toString(),
              api: name,
              interface: "IDirectSound8",
              vtableSlot: slot,
              object: this.traceObject,
              arguments: this.traceArguments,
              result: result.toString(),
              signedResult: result.toInt32(),
              lastError,
              ...(this.traceDsoundSetCooperativeLevel === null
                ? {}
                : {
                    directSoundSetCooperativeLevel:
                      this.traceDsoundSetCooperativeLevel,
                  }),
            });
            setImmediate(maybeCompleteFocusedDsoundRootObservation);
          }
          if (
            (focusedDsoundCreateBufferObservation ||
              focusedDsoundBufferLockUnlockPlayObservation) &&
            directSoundCreateBuffer !== null &&
            focusedDsoundCreateBufferCalls.length < 2
          ) {
            const call = {
              callId: this.traceFrame.callId,
              caller: this.traceFrame.caller,
              returnAddress: this.returnAddress.toString(),
              api: name,
              interface: "IDirectSound8",
              vtableSlot: slot,
              object: this.traceObject,
              arguments: this.traceArguments,
              result: result.toString(),
              signedResult: result.toInt32(),
              lastError,
              directSoundCreateBuffer,
            };
            focusedDsoundCreateBufferCalls.push(call);
            const output = directSoundCreateBuffer.output.after.value;
            if (
              result.toInt32() >= 0 &&
              typeof output === "string" &&
              output !== "0x0"
            ) {
              attachIdirectsoundbuffer(ptr(output));
            }
            setImmediate(maybeCompleteFocusedDsoundCreateBufferObservation);
            setImmediate(
              maybeCompleteFocusedDsoundBufferLockUnlockPlayObservation,
            );
          }
        },
      });
    }
    emit("vtable-observed", {
      interface: "IDirectSound8",
      object: object.toString(),
      vtable: vtable.toString(),
      slots,
    });
  } catch (error) {
    emit("vtable-error", {
      interface: "IDirectSound8",
      object: object.toString(),
      error: error.message,
    });
  }
}

function maybeCompleteFocusedDsoundRootObservation() {
  if (
    focusedDsoundRootComplete ||
    focusedDsoundFactoryCall === null ||
    focusedDsoundRootCalls.length !== 1
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteFocusedDsoundRootObservation, 10);
    return;
  }
  focusedDsoundRootComplete = true;
  emit("dsound-root-complete", {
    profile: "x86-bgi-direct-sound-8-root-v1",
    rootObject: focusedDsoundRootObject,
    factory: focusedDsoundFactoryCall,
    firstRootCall: focusedDsoundRootCalls[0],
  });
  if (stopOnEvent === "dsound-root-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteFocusedDsoundCreateBufferObservation() {
  if (
    !focusedDsoundCreateBufferObservation ||
    focusedDsoundCreateBufferComplete ||
    focusedDsoundFactoryCall === null ||
    focusedDsoundCreateBufferCalls.length !== 2 ||
    focusedDsoundBufferCalls.length !== 5
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteFocusedDsoundCreateBufferObservation, 10);
    return;
  }
  focusedDsoundCreateBufferComplete = true;
  emit("dsound-create-sound-buffer-complete", {
    profile: "x86-bgi-direct-sound-create-buffer-v1",
    rootObject: focusedDsoundRootObject,
    factory: focusedDsoundFactoryCall,
    createCalls: focusedDsoundCreateBufferCalls,
    bufferCalls: focusedDsoundBufferCalls,
  });
  if (stopOnEvent === "dsound-create-sound-buffer-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteFocusedDsoundBufferLockUnlockPlayObservation() {
  const expectedBufferMethods = [
    "SetFormat",
    "Play",
    "QueryInterface",
    "Release",
    "Lock",
    "Unlock",
    "Play",
  ];
  if (
    !focusedDsoundBufferLockUnlockPlayObservation ||
    focusedDsoundBufferLockUnlockPlayComplete ||
    focusedDsoundFactoryCall === null ||
    focusedDsoundCreateBufferCalls.length !== 2 ||
    focusedDsoundBufferCalls.length !== expectedBufferMethods.length ||
    !focusedDsoundBufferCalls.every(
      (call, index) => call.api === expectedBufferMethods[index],
    )
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteFocusedDsoundBufferLockUnlockPlayObservation, 10);
    return;
  }
  focusedDsoundBufferLockUnlockPlayComplete = true;
  emit("dsound-buffer-lock-unlock-play-complete", {
    profile: "x86-bgi-direct-sound-buffer-lock-unlock-play-v1",
    rootObject: focusedDsoundRootObject,
    factory: focusedDsoundFactoryCall,
    createCalls: focusedDsoundCreateBufferCalls,
    bufferCalls: focusedDsoundBufferCalls,
  });
  if (stopOnEvent === "dsound-buffer-lock-unlock-play-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function attachIdirectinput8a(object) {
  if (object.isNull()) {
    return;
  }
  try {
    const vtable = object.readPointer();
    const vtableKey = `IDirectInput8A:${vtable}`;
    if (observedVtables.has(vtableKey)) {
      return;
    }
    observedVtables.add(vtableKey);
    const slots = [];
    for (const [slot, name, argumentCount] of idirectinput8aMethods) {
      const address = vtable.add(slot * Process.pointerSize).readPointer();
      slots.push({ slot, name, address: address.toString() });
      const key = `IDirectInput8A:${slot}:${address}`;
      if (attachedVtableMethods.has(key)) {
        continue;
      }
      attachedVtableMethods.add(key);
      const implementationModule = Process.findModuleByAddress(address);
      Interceptor.attach(address, {
        onEnter(args) {
          const caller = describeCaller(this.returnAddress);
          if (
            focusedDinput8Observation &&
            (focusedDinput8RootComplete ||
              caller.module?.toLowerCase() !==
                Process.mainModule.name.toLowerCase())
          ) {
            this.traceFrame = null;
            return;
          }
          const objectAddress = args[0].toString();
          const arguments_ = Array.from({ length: argumentCount }, (_, index) =>
            args[index + 1].toString(),
          );
          this.traceObject = objectAddress;
          this.traceArguments = arguments_;
          this.traceDinput8CreateDevice =
            slot === 3
              ? {
                  deviceGuid: snapshotBoundedRawBytes(args[1], 16, 16),
                  deviceOutput: {
                    before: snapshotPointerSlot(args[2]),
                  },
                  outerUnknown: args[3].toString(),
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    96,
                  ),
                }
              : null;
          this.traceDinput8CreateDeviceOutput = slot === 3 ? args[2] : null;
          this.traceDinput8EnumDevices =
            slot === 4 && focusedDinput8EnumDevicesObservation
              ? {
                  deviceType: args[1].toUInt32(),
                  callback: args[2].toString(),
                  callbackIdentity: args[2].isNull()
                    ? null
                    : describeCaller(args[2]),
                  callbackInstructions: args[2].isNull()
                    ? null
                    : snapshotInstructions(args[2], 96),
                  reference: args[3].toString(),
                  flags: args[4].toUInt32(),
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    96,
                  ),
                  callbacks: [],
                }
              : null;
          if (this.traceDinput8EnumDevices !== null) {
            attachDinput8EnumDevicesCallback(args[2]);
          }
          this.traceFrame = beginCall(
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirectInput8A",
              vtableSlot: slot,
              object: objectAddress,
              arguments: arguments_,
            },
            this.returnAddress,
          );
          if (this.traceDinput8EnumDevices !== null) {
            activeDinput8EnumDevicesByThread.set(
              this.traceFrame.threadId,
              this.traceDinput8EnumDevices,
            );
          }
        },
        onLeave(result) {
          if (this.traceFrame === null) {
            return;
          }
          const lastError = this.lastError;
          if (this.traceDinput8EnumDevices !== null) {
            activeDinput8EnumDevicesByThread.delete(this.traceFrame.threadId);
          }
          const dinput8CreateDevice =
            this.traceDinput8CreateDevice === null
              ? null
              : {
                  ...this.traceDinput8CreateDevice,
                  deviceOutput: {
                    ...this.traceDinput8CreateDevice.deviceOutput,
                    after: snapshotPointerSlot(
                      this.traceDinput8CreateDeviceOutput,
                    ),
                  },
                };
          finishCall(
            this.traceFrame,
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirectInput8A",
              vtableSlot: slot,
              object: this.traceObject,
              result: result.toString(),
              ...(dinput8CreateDevice === null ? {} : { dinput8CreateDevice }),
              ...(this.traceDinput8EnumDevices === null
                ? {}
                : { dinput8EnumDevices: this.traceDinput8EnumDevices }),
            },
            lastError,
          );
          if (
            focusedDinput8Observation &&
            focusedDinput8RootCalls.length === 0
          ) {
            focusedDinput8RootCalls.push({
              callId: this.traceFrame.callId,
              caller: this.traceFrame.caller,
              returnAddress: this.returnAddress.toString(),
              api: name,
              interface: "IDirectInput8A",
              vtableSlot: slot,
              object: this.traceObject,
              arguments: this.traceArguments,
              result: result.toString(),
              signedResult: result.toInt32(),
              lastError: Number(lastError) >>> 0,
              ...(dinput8CreateDevice === null ? {} : { dinput8CreateDevice }),
              ...(this.traceDinput8EnumDevices === null
                ? {}
                : { dinput8EnumDevices: this.traceDinput8EnumDevices }),
            });
            setImmediate(maybeCompleteFocusedDinput8RootObservation);
          }
        },
      });
    }
    emit("vtable-observed", {
      interface: "IDirectInput8A",
      object: object.toString(),
      vtable: vtable.toString(),
      slots,
    });
  } catch (error) {
    emit("vtable-error", {
      interface: "IDirectInput8A",
      object: object.toString(),
      error: error.message,
    });
  }
}

function maybeCompleteFocusedDinput8RootObservation() {
  if (
    focusedDinput8RootComplete ||
    focusedDinput8FactoryCall === null ||
    focusedDinput8RootCalls.length !== 1
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteFocusedDinput8RootObservation, 10);
    return;
  }
  focusedDinput8RootComplete = true;
  const completionKind = focusedDinput8EnumDevicesObservation
    ? "dinput8-enum-devices-complete"
    : "dinput8-root-complete";
  emit(completionKind, {
    profile: focusedDinput8EnumDevicesObservation
      ? "x86-bgi-direct-input-8-enum-devices-v1"
      : "x86-bgi-direct-input-8-root-v1",
    rootObject: focusedDinput8RootObject,
    factory: focusedDinput8FactoryCall,
    firstRootCall: focusedDinput8RootCalls[0],
  });
  if (stopOnEvent === completionKind) {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteFocusedD3d9InitialTexturesObservation() {
  if (
    focusedD3d9InitialTexturesComplete ||
    focusedD3d9InitialTextureCalls.length !== 3
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteFocusedD3d9InitialTexturesObservation, 10);
    return;
  }
  focusedD3d9InitialTexturesComplete = true;
  emit("d3d9-initial-textures-complete", {
    profile: "x86-bgi-initial-textures-v1",
    device: focusedD3d9InitialTextureDevice,
    calls: focusedD3d9InitialTextureCalls,
  });
  if (stopOnEvent === "d3d9-initial-textures-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteFocusedD3d9RecreatedTexturesObservation() {
  if (
    focusedD3d9RecreatedTexturesComplete ||
    focusedD3d9RecreatedTextureCalls.length !== 3
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteFocusedD3d9RecreatedTexturesObservation, 10);
    return;
  }
  focusedD3d9RecreatedTexturesComplete = true;
  emit("d3d9-recreated-textures-complete", {
    profile: "x86-bgi-recreated-textures-v1",
    device: focusedD3d9RecreatedTextureDevice,
    calls: focusedD3d9RecreatedTextureCalls,
  });
  if (stopOnEvent === "d3d9-recreated-textures-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteFocusedD3d9InitialVertexBufferObservation() {
  if (
    focusedD3d9InitialVertexBufferComplete ||
    focusedD3d9InitialVertexBufferCalls.length !== 3 ||
    focusedD3d9InitialVertexBufferCalls[0]?.api !== "CreateVertexBuffer" ||
    focusedD3d9InitialVertexBufferCalls[1]?.api !== "Lock" ||
    focusedD3d9InitialVertexBufferCalls[2]?.api !== "Unlock"
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteFocusedD3d9InitialVertexBufferObservation, 10);
    return;
  }
  focusedD3d9InitialVertexBufferComplete = true;
  emit("d3d9-initial-vertex-buffer-complete", {
    profile: "x86-bgi-initial-vertex-buffer-v1",
    device: focusedD3d9InitialVertexBufferDevice,
    vertexBuffer: focusedD3d9InitialVertexBufferObject,
    calls: focusedD3d9InitialVertexBufferCalls,
  });
  if (stopOnEvent === "d3d9-initial-vertex-buffer-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteFocusedD3d9InitialRenderStatesObservation() {
  if (
    focusedD3d9InitialRenderStatesComplete ||
    focusedD3d9InitialRenderStateCalls.length !== 4 ||
    !focusedD3d9InitialRenderStateCalls.every(
      (call) => call.api === "SetRenderState",
    )
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteFocusedD3d9InitialRenderStatesObservation, 10);
    return;
  }
  focusedD3d9InitialRenderStatesComplete = true;
  emit("d3d9-initial-render-states-complete", {
    profile: "x86-bgi-initial-render-states-v1",
    device: focusedD3d9InitialRenderStateDevice,
    calls: focusedD3d9InitialRenderStateCalls,
  });
  if (stopOnEvent === "d3d9-initial-render-states-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteFocusedD3d9RasterStatusObservation() {
  if (
    focusedD3d9RasterStatusComplete ||
    focusedD3d9RasterStatusCalls.length !== 1
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteFocusedD3d9RasterStatusObservation, 10);
    return;
  }
  focusedD3d9RasterStatusComplete = true;
  emit("d3d9-raster-status-complete", {
    profile: "x86-bgi-first-raster-status-v1",
    device: focusedD3d9RasterStatusDevice,
    calls: focusedD3d9RasterStatusCalls,
  });
  if (stopOnEvent === "d3d9-raster-status-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function attachIdirect3d9(object) {
  if (object.isNull()) {
    return;
  }
  try {
    const vtable = object.readPointer();
    const vtableKey = `IDirect3D9:${vtable}`;
    if (observedVtables.has(vtableKey)) {
      return;
    }
    observedVtables.add(vtableKey);
    const slots = [];
    for (const [slot, name, argumentCount] of idirect3d9Methods) {
      const address = vtable.add(slot * Process.pointerSize).readPointer();
      slots.push({ slot, name, address: address.toString() });
      const key = `IDirect3D9:${slot}:${address}`;
      if (attachedVtableMethods.has(key)) {
        continue;
      }
      attachedVtableMethods.add(key);
      const implementationModule = Process.findModuleByAddress(address);
      Interceptor.attach(address, {
        onEnter(args) {
          const objectAddress = args[0].toString();
          const arguments_ = Array.from({ length: argumentCount }, (_, index) =>
            args[index + 1].toString(),
          );
          this.traceObject = objectAddress;
          this.traceD3d9DisplayModeOutput = slot === 8 ? args[2] : null;
          this.traceD3d9DisplayMode =
            this.traceD3d9DisplayModeOutput === null
              ? null
              : {
                  adapter: args[1].toUInt32(),
                  before: snapshotD3d9DisplayMode(
                    this.traceD3d9DisplayModeOutput,
                  ),
                  immediateConsumer: snapshotInstructions(
                    this.returnAddress,
                    8,
                  ),
                };
          this.traceD3d9AdapterIdentifierOutput = slot === 5 ? args[3] : null;
          this.traceD3d9AdapterIdentifier =
            this.traceD3d9AdapterIdentifierOutput === null
              ? null
              : {
                  adapter: args[1].toUInt32(),
                  flags: args[2].toUInt32(),
                  before: snapshotD3d9AdapterIdentifier(
                    this.traceD3d9AdapterIdentifierOutput,
                  ),
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    128,
                  ),
                };
          this.traceD3d9DeviceCapsOutput = slot === 14 ? args[3] : null;
          this.traceD3d9DeviceCaps =
            this.traceD3d9DeviceCapsOutput === null
              ? null
              : {
                  adapter: args[1].toUInt32(),
                  deviceType: args[2].toUInt32(),
                  callerStackPointerAfterReturn: this.context.esp
                    .add((argumentCount + 2) * Process.pointerSize)
                    .toString(),
                  before: snapshotD3d9DeviceCaps(
                    this.traceD3d9DeviceCapsOutput,
                  ),
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    256,
                  ),
                };
          this.traceD3d9CreateDevicePresentationOutput =
            slot === 16 ? args[5] : null;
          this.traceD3d9CreateDeviceOutput = slot === 16 ? args[6] : null;
          this.traceD3d9CreateDevice =
            this.traceD3d9CreateDevicePresentationOutput === null
              ? null
              : {
                  adapter: args[1].toUInt32(),
                  deviceType: args[2].toUInt32(),
                  focusWindow: args[3].toString(),
                  behaviorFlags: args[4].toUInt32(),
                  presentationParameters: {
                    before: snapshotD3d9PresentParameters(
                      this.traceD3d9CreateDevicePresentationOutput,
                    ),
                  },
                  deviceOutput: {
                    before: snapshotPointerSlot(
                      this.traceD3d9CreateDeviceOutput,
                    ),
                  },
                  immediateConsumer: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    128,
                  ),
                };
          this.traceFrame = beginCall(
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3D9",
              vtableSlot: slot,
              object: objectAddress,
              arguments: arguments_,
            },
            this.returnAddress,
          );
        },
        onLeave(result) {
          const lastError = this.lastError;
          const d3d9DisplayMode =
            this.traceD3d9DisplayMode === null
              ? null
              : {
                  ...this.traceD3d9DisplayMode,
                  after: snapshotD3d9DisplayMode(
                    this.traceD3d9DisplayModeOutput,
                  ),
                };
          const d3d9AdapterIdentifier =
            this.traceD3d9AdapterIdentifier === null
              ? null
              : {
                  ...this.traceD3d9AdapterIdentifier,
                  after: snapshotD3d9AdapterIdentifier(
                    this.traceD3d9AdapterIdentifierOutput,
                  ),
                };
          const d3d9DeviceCaps =
            this.traceD3d9DeviceCaps === null
              ? null
              : {
                  ...this.traceD3d9DeviceCaps,
                  after: snapshotD3d9DeviceCaps(this.traceD3d9DeviceCapsOutput),
                };
          const d3d9CreateDevice =
            this.traceD3d9CreateDevice === null
              ? null
              : {
                  ...this.traceD3d9CreateDevice,
                  presentationParameters: {
                    ...this.traceD3d9CreateDevice.presentationParameters,
                    after: snapshotD3d9PresentParameters(
                      this.traceD3d9CreateDevicePresentationOutput,
                    ),
                  },
                  deviceOutput: {
                    ...this.traceD3d9CreateDevice.deviceOutput,
                    after: snapshotPointerSlot(
                      this.traceD3d9CreateDeviceOutput,
                    ),
                  },
                };
          finishCall(
            this.traceFrame,
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3D9",
              vtableSlot: slot,
              object: this.traceObject,
              result: result.toString(),
              ...(d3d9DisplayMode === null ? {} : { d3d9DisplayMode }),
              ...(d3d9AdapterIdentifier === null
                ? {}
                : { d3d9AdapterIdentifier }),
              ...(d3d9DeviceCaps === null ? {} : { d3d9DeviceCaps }),
              ...(d3d9CreateDevice === null ? {} : { d3d9CreateDevice }),
            },
            lastError,
          );
          if (
            d3d9CreateDevice !== null &&
            result.toInt32() >= 0 &&
            typeof d3d9CreateDevice.deviceOutput.after.value === "string" &&
            d3d9CreateDevice.deviceOutput.after.value !== "0x0"
          ) {
            attachIdirect3ddevice9(
              ptr(d3d9CreateDevice.deviceOutput.after.value),
            );
          }
        },
      });
    }
    emit("vtable-observed", {
      interface: "IDirect3D9",
      object: object.toString(),
      vtable: vtable.toString(),
      slots,
    });
  } catch (error) {
    emit("vtable-error", {
      interface: "IDirect3D9",
      object: object.toString(),
      error: error.message,
    });
  }
}

function attachGetVersionExAHook(module) {
  const name = "GetVersionExA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" && getVersionExAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.output = args[0];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = getVersionExAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.outputBefore = snapshotOsVersionInfoA(this.output);
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              64,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              4,
              16,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.output.toString()],
          lastErrorBefore: this.lastErrorBefore,
          getVersionExA: {
            outputBefore: this.outputBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const rawResult = result.toUInt32();
      const outputAfter = snapshotOsVersionInfoA(this.output);
      if (this.oracleCaller) {
        getVersionExAOracleCall.hookResult = rawResult;
        getVersionExAOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          getVersionExA: {
            result: rawResult,
            outputAfter,
          },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && getVersionExAMainModuleCalls.length === 0) {
        getVersionExAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          outputAddress: this.output.toString(),
          lastErrorBefore: this.lastErrorBefore,
          outputBefore: this.outputBefore,
          outputAfter,
          result: rawResult,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        setImmediate(maybeCompleteGetVersionExAObservation);
      }
    },
  });
}

function attachCreateWaitableTimerAHook(module) {
  const name = "CreateWaitableTimerA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        createWaitableTimerAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.securityAttributes = args[0];
      this.manualReset = args[1].toUInt32();
      this.nameAddress = args[2];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = createWaitableTimerAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              12,
              16,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [
            this.securityAttributes.toString(),
            this.manualReset,
            this.nameAddress.isNull()
              ? null
              : snapshotNulTerminatedString(this.nameAddress, "ansi"),
          ],
          lastErrorBefore: this.lastErrorBefore,
          createWaitableTimerA: {
            securityAttributes: this.securityAttributes.toString(),
            manualReset: this.manualReset,
            nameAddress: this.nameAddress.toString(),
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      if (this.oracleCaller) {
        createWaitableTimerAOracleCall.hookResult = resultValue;
        createWaitableTimerAOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          createWaitableTimerA: {
            result: resultValue,
            resultNonNull: !result.isNull(),
          },
        },
        this.lastError,
      );
      if (
        this.mainModuleCaller &&
        createWaitableTimerAMainModuleCalls.length === 0
      ) {
        createWaitableTimerAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          securityAttributes: this.securityAttributes.toString(),
          manualReset: this.manualReset,
          nameAddress: this.nameAddress.toString(),
          result: resultValue,
          resultNonNull: !result.isNull(),
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        setImmediate(maybeCompleteCreateWaitableTimerAObservation);
      }
    },
  });
}

function attachGetFileAttributesAHook(module) {
  const name = "GetFileAttributesA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        getFileAttributesAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.path = args[0];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = getFileAttributesAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.pathBefore = snapshotNulTerminatedString(this.path, "ansi");
      const recordsConsumer =
        isMainModuleCaller && !getFileAttributesAConsumerRecorded;
      if (recordsConsumer) {
        getFileAttributesAConsumerRecorded = true;
      }
      this.immediateConsumer = recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              4,
              16,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.path.toString()],
          lastErrorBefore: this.lastErrorBefore,
          getFileAttributesA: {
            pathBefore: this.pathBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const attributes = result.toUInt32();
      if (this.oracleCaller) {
        getFileAttributesAOracleCall.hookResult = attributes;
        getFileAttributesAOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          getFileAttributesA: {
            attributes,
            isInvalid: attributes === 0xffff_ffff,
            isDirectory:
              attributes !== 0xffff_ffff && (attributes & 0x10) !== 0,
          },
        },
        this.lastError,
      );
    },
  });
}

function attachCreateDirectoryAHook(module) {
  const name = "CreateDirectoryA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        createDirectoryAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.path = args[0];
      this.securityAttributes = args[1];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = createDirectoryAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.pathBefore = snapshotNulTerminatedString(this.path, "ansi");
      const recordsConsumer =
        isMainModuleCaller && !createDirectoryAConsumerRecorded;
      if (recordsConsumer) {
        createDirectoryAConsumerRecorded = true;
      }
      this.immediateConsumer = recordsConsumer
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              32,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              4,
              16,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.path.toString(), this.securityAttributes.toString()],
          lastErrorBefore: this.lastErrorBefore,
          createDirectoryA: {
            pathBefore: this.pathBefore,
            securityAttributes: this.securityAttributes.toString(),
            securityAttributesNull: this.securityAttributes.isNull(),
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const succeeded = result.toUInt32() !== 0;
      if (this.oracleCaller) {
        createDirectoryAOracleCall.hookResult = succeeded;
        createDirectoryAOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          createDirectoryA: {
            succeeded,
          },
        },
        this.lastError,
      );
    },
  });
}

function attachSetWaitableTimerHook(module) {
  const name = "SetWaitableTimer";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        setWaitableTimerOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      if (isMainModuleCaller) {
        if (setWaitableTimerMainCallClaimed) {
          this.traceFrame = null;
          return;
        }
        setWaitableTimerMainCallClaimed = true;
      }
      this.handle = args[0];
      this.dueTimeAddress = args[1];
      this.periodMilliseconds = args[2].toInt32();
      this.completionRoutine = args[3];
      this.completionContext = args[4];
      this.resume = args[5].toUInt32();
      this.dueTime = snapshotSignedLargeInteger(this.dueTimeAddress);
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = setWaitableTimerOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              48,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              24,
              16,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [
            this.handle.toString(),
            this.dueTimeAddress.toString(),
            this.periodMilliseconds,
            this.completionRoutine.toString(),
            this.completionContext.toString(),
            this.resume,
          ],
          lastErrorBefore: this.lastErrorBefore,
          setWaitableTimer: {
            handle: this.handle.toString(),
            dueTime: this.dueTime,
            periodMilliseconds: this.periodMilliseconds,
            completionRoutine: this.completionRoutine.toString(),
            completionContext: this.completionContext.toString(),
            resume: this.resume,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const rawResult = result.toUInt32();
      if (this.oracleCaller) {
        setWaitableTimerOracleCall.hookResult = rawResult;
        setWaitableTimerOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          setWaitableTimer: {
            result: rawResult,
          },
        },
        this.lastError,
      );
      if (
        this.mainModuleCaller &&
        setWaitableTimerMainModuleCalls.length === 0
      ) {
        setWaitableTimerMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          handle: this.handle.toString(),
          dueTimeAddress: this.dueTimeAddress.toString(),
          dueTime: this.dueTime,
          periodMilliseconds: this.periodMilliseconds,
          completionRoutine: this.completionRoutine.toString(),
          completionContext: this.completionContext.toString(),
          resume: this.resume,
          result: rawResult,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        setImmediate(maybeCompleteSetWaitableTimerObservation);
      }
    },
  });
}

function attachWaitForSingleObjectExHook(module) {
  const name = "WaitForSingleObjectEx";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        waitForSingleObjectExOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      if (isMainModuleCaller) {
        if (waitForSingleObjectExMainCallClaimed) {
          this.traceFrame = null;
          return;
        }
        waitForSingleObjectExMainCallClaimed = true;
      }
      this.handle = args[0];
      this.timeoutMilliseconds = args[1].toUInt32();
      this.alertable = args[2].toUInt32();
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = waitForSingleObjectExOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              48,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              24,
              16,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [
            this.handle.toString(),
            this.timeoutMilliseconds,
            this.alertable,
          ],
          lastErrorBefore: this.lastErrorBefore,
          waitForSingleObjectEx: {
            handle: this.handle.toString(),
            timeoutMilliseconds: this.timeoutMilliseconds,
            alertable: this.alertable,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const rawResult = result.toUInt32();
      const lastError = Number(this.lastError) >>> 0;
      if (this.oracleCaller) {
        waitForSingleObjectExOracleCall.hookResult = rawResult;
        waitForSingleObjectExOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          waitForSingleObjectEx: {
            result: rawResult,
          },
        },
        this.lastError,
      );
      if (
        this.mainModuleCaller &&
        waitForSingleObjectExMainModuleCalls.length === 0
      ) {
        waitForSingleObjectExMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          handle: this.handle.toString(),
          timeoutMilliseconds: this.timeoutMilliseconds,
          alertable: this.alertable,
          result: rawResult,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        setImmediate(maybeCompleteWaitForSingleObjectExObservation);
      }
    },
  });
}

function snapshotResourceNameA(address) {
  const value = address.toUInt32();
  if (value <= 0xffff) {
    return {
      address: address.toString(),
      kind: "integer",
      id: value,
    };
  }
  return {
    address: address.toString(),
    kind: "string",
    value: snapshotNulTerminatedString(address, "ansi"),
  };
}

function snapshotClassNameA(address) {
  if (address.isNull()) {
    return {
      address: address.toString(),
      kind: "null",
    };
  }
  const value = address.toUInt32();
  if (value <= 0xffff) {
    return {
      address: address.toString(),
      kind: "integer",
      atom: value,
    };
  }
  return {
    address: address.toString(),
    kind: "string",
    value: snapshotNulTerminatedString(address, "ansi"),
  };
}

function snapshotAddressIdentity(address) {
  if (address.isNull()) {
    return {
      address: address.toString(),
      module: null,
      rva: null,
    };
  }
  const module = Process.findModuleByAddress(address);
  return {
    address: address.toString(),
    module: module?.name ?? null,
    moduleBase: module?.base.toString() ?? null,
    rva: module === null ? null : address.sub(module.base).toUInt32(),
  };
}

function snapshotWndClassExA(pointer) {
  const profile = "x86-wndclassexa-v1";
  const measuredBytes = 48;
  const raw = snapshotBoundedRawBytes(pointer, measuredBytes);
  if (raw.error !== undefined) {
    return { profile, measuredBytes, raw };
  }
  if (Process.pointerSize !== 4) {
    return {
      profile,
      measuredBytes,
      raw,
      error: `x86 WNDCLASSEXA observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  try {
    const wndProc = pointer.add(8).readPointer();
    const instance = pointer.add(20).readPointer();
    const icon = pointer.add(24).readPointer();
    const cursor = pointer.add(28).readPointer();
    const backgroundBrush = pointer.add(32).readPointer();
    const menuName = pointer.add(36).readPointer();
    const className = pointer.add(40).readPointer();
    const smallIcon = pointer.add(44).readPointer();
    return {
      profile,
      measuredBytes,
      raw,
      fields: {
        cbSize: pointer.readU32(),
        style: pointer.add(4).readU32(),
        wndProc: snapshotAddressIdentity(wndProc),
        classExtraBytes: pointer.add(12).readS32(),
        windowExtraBytes: pointer.add(16).readS32(),
        instance: snapshotAddressIdentity(instance),
        icon: icon.toString(),
        cursor: cursor.toString(),
        backgroundBrush: backgroundBrush.toString(),
        menuName: snapshotClassNameA(menuName),
        className: snapshotClassNameA(className),
        smallIcon: smallIcon.toString(),
      },
    };
  } catch (error) {
    return { profile, measuredBytes, raw, error: error.message };
  }
}

function snapshotCreateStructA(pointer) {
  const profile = "x86-createstructa-v1";
  const measuredBytes = 48;
  const raw = snapshotBoundedRawBytes(pointer, measuredBytes);
  if (raw.error !== undefined) {
    return { profile, measuredBytes, raw };
  }
  if (Process.pointerSize !== 4) {
    return {
      profile,
      measuredBytes,
      raw,
      error: `x86 CREATESTRUCTA observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  try {
    const createParams = pointer.readPointer();
    const instance = pointer.add(4).readPointer();
    const menu = pointer.add(8).readPointer();
    const parent = pointer.add(12).readPointer();
    const windowName = pointer.add(36).readPointer();
    const className = pointer.add(40).readPointer();
    return {
      profile,
      measuredBytes,
      raw,
      fields: {
        createParams: createParams.toString(),
        instance: snapshotAddressIdentity(instance),
        menu: menu.toString(),
        parent: parent.toString(),
        height: pointer.add(16).readS32(),
        width: pointer.add(20).readS32(),
        y: pointer.add(24).readS32(),
        x: pointer.add(28).readS32(),
        style: pointer.add(32).readU32(),
        windowName: snapshotClassNameA(windowName),
        className: snapshotClassNameA(className),
        extendedStyle: pointer.add(44).readU32(),
      },
    };
  } catch (error) {
    return { profile, measuredBytes, raw, error: error.message };
  }
}

function createWindowClassIdentity(snapshot) {
  if (
    snapshot?.kind !== "string" ||
    typeof snapshot.value?.string !== "string"
  ) {
    return null;
  }
  return snapshot.value.string.toLowerCase();
}

function createWindowMessageName(message) {
  switch (message) {
    case 0x0001:
      return "WM_CREATE";
    case 0x0003:
      return "WM_MOVE";
    case 0x0005:
      return "WM_SIZE";
    case 0x0018:
      return "WM_SHOWWINDOW";
    case 0x0046:
      return "WM_WINDOWPOSCHANGING";
    case 0x0047:
      return "WM_WINDOWPOSCHANGED";
    case 0x0081:
      return "WM_NCCREATE";
    case 0x0083:
      return "WM_NCCALCSIZE";
    case 0x0085:
      return "WM_NCPAINT";
    case 0x0086:
      return "WM_NCACTIVATE";
    case 0x0129:
      return "WM_QUERYUISTATE";
    default:
      return null;
  }
}

function snapshotCreateWindowMessagePayload(message, wParam, lParam) {
  if (message === 0x0083 && wParam.toUInt32() === 0 && !lParam.isNull()) {
    const raw = snapshotBoundedRawBytes(lParam, 16);
    let fields = null;
    if (raw.error === undefined) {
      try {
        fields = {
          left: lParam.readS32(),
          top: lParam.add(4).readS32(),
          right: lParam.add(8).readS32(),
          bottom: lParam.add(12).readS32(),
        };
      } catch {}
    }
    return { kind: "rect", profile: "x86-rect-v1", raw, fields };
  }
  if (message === 0x0005 || message === 0x0003) {
    const packed = lParam.toUInt32();
    const low = packed & 0xffff;
    const high = (packed >>> 16) & 0xffff;
    const signed16 = (value) => (value & 0x8000 ? value - 0x10000 : value);
    return message === 0x0005
      ? {
          kind: "size",
          profile: "x86-wm-size-v1",
          width: low,
          height: high,
        }
      : {
          kind: "move",
          profile: "x86-wm-move-v1",
          x: signed16(low),
          y: signed16(high),
        };
  }
  return null;
}

function attachCreateWindowProcedureObserver(address, registration) {
  if (address.isNull()) {
    return;
  }
  const key = address.toString();
  if (attachedCreateWindowProcedures.has(key)) {
    return;
  }
  attachedCreateWindowProcedures.add(key);
  Interceptor.attach(address, {
    onEnter(args) {
      const threadId = Process.getCurrentThreadId();
      const calls = activeCreateWindowCallsByThread.get(threadId) ?? [];
      const active = calls.at(-1) ?? null;
      if (active === null || active.callbacks.length >= 128) {
        this.callback = null;
        return;
      }
      const message = args[1].toUInt32();
      const lParam = args[3];
      const callback = {
        sequence: active.callbacks.length + 1,
        threadId,
        procedure: snapshotAddressIdentity(address),
        registeredClass: registration,
        hwnd: args[0].toString(),
        message,
        messageName: createWindowMessageName(message),
        wParam: args[2].toString(),
        lParam: lParam.toString(),
        messagePayloadBefore: snapshotCreateWindowMessagePayload(
          message,
          args[2],
          lParam,
        ),
        messagePayloadAfter: null,
        createStruct:
          (message === 0x0081 || message === 0x0001) && !lParam.isNull()
            ? snapshotCreateStructA(lParam)
            : null,
        result: null,
        lastError: null,
      };
      active.callbacks.push(callback);
      this.callback = callback;
    },
    onLeave(result) {
      if (this.callback === null) {
        return;
      }
      this.callback.messagePayloadAfter = snapshotCreateWindowMessagePayload(
        this.callback.message,
        ptr(this.callback.wParam),
        ptr(this.callback.lParam),
      );
      this.callback.result = result.toString();
      this.callback.lastError = Number(this.lastError) >>> 0;
    },
  });
}

function attachCreateWindowClassDiscoveryHook(module) {
  const address = findExport(module, "RegisterClassExA");
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attachedCreateWindowClassDiscovery.has(key)) {
    return;
  }
  attachedCreateWindowClassDiscovery.add(key);
  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      if (
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase()
      ) {
        this.registration = null;
        return;
      }
      const windowClassAddress = args[0];
      const windowClass = snapshotWndClassExA(windowClassAddress);
      let procedure = null;
      try {
        procedure = windowClassAddress.add(8).readPointer();
      } catch {}
      this.registration = {
        returnAddress: this.returnAddress.toString(),
        windowClassAddress: windowClassAddress.toString(),
        windowClass,
        procedure,
      };
    },
    onLeave(result) {
      if (this.registration === null || result.toUInt32() === 0) {
        return;
      }
      const className = this.registration.windowClass.fields?.className ?? null;
      const identity = createWindowClassIdentity(className);
      const registration = {
        returnAddress: this.registration.returnAddress,
        atom: result.toUInt32(),
        windowClass: this.registration.windowClass,
      };
      if (identity !== null) {
        createWindowClassRegistrations.set(identity, registration);
      }
      if (this.registration.procedure !== null) {
        attachCreateWindowProcedureObserver(
          this.registration.procedure,
          registration,
        );
      }
      emit("create-window-class-discovered", registration);
    },
  });
}

function attachCreateWindowExAHook(module) {
  const name = "CreateWindowExA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);
  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      if (
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase()
      ) {
        this.traceFrame = null;
        return;
      }
      const className = snapshotClassNameA(args[1]);
      const windowName = snapshotClassNameA(args[2]);
      const classIdentity = createWindowClassIdentity(className);
      const call = {
        threadId: Process.getCurrentThreadId(),
        returnAddress: this.returnAddress.toString(),
        extendedStyle: args[0].toUInt32(),
        classNameAddress: args[1],
        className,
        windowNameAddress: args[2],
        windowName,
        style: args[3].toUInt32(),
        x: signed32(args[4]),
        y: signed32(args[5]),
        width: signed32(args[6]),
        height: signed32(args[7]),
        parent: args[8].toString(),
        menu: args[9].toString(),
        instance: snapshotAddressIdentity(args[10]),
        createParams: args[11].toString(),
        registeredClass:
          classIdentity === null
            ? null
            : (createWindowClassRegistrations.get(classIdentity) ?? null),
        lastErrorBefore: Number(this.lastError) >>> 0,
        immediateConsumer: {
          apiContinuation: snapshotInstructionsFollowingDirectJump(
            this.returnAddress,
            96,
          ),
          frameCallerContinuations: snapshotX86FrameCallerContinuations(
            this.context,
            8,
            24,
          ),
        },
        callbacks: [],
      };
      const calls = activeCreateWindowCallsByThread.get(call.threadId) ?? [];
      calls.push(call);
      activeCreateWindowCallsByThread.set(call.threadId, calls);
      this.windowCall = call;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [
            call.extendedStyle,
            call.className,
            call.windowName,
            call.style,
            call.x,
            call.y,
            call.width,
            call.height,
            call.parent,
            call.menu,
            call.instance,
            call.createParams,
          ],
          lastErrorBefore: call.lastErrorBefore,
          createWindowExA: {
            registeredClass: call.registeredClass,
            immediateConsumer: call.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const calls =
        activeCreateWindowCallsByThread.get(this.windowCall.threadId) ?? [];
      if (calls.at(-1) === this.windowCall) {
        calls.pop();
      } else {
        const index = calls.lastIndexOf(this.windowCall);
        if (index !== -1) {
          calls.splice(index, 1);
        }
      }
      if (calls.length === 0) {
        activeCreateWindowCallsByThread.delete(this.windowCall.threadId);
      }
      const lastError = Number(this.lastError) >>> 0;
      const classNameAfter = snapshotClassNameA(
        this.windowCall.classNameAddress,
      );
      const windowNameAfter = snapshotClassNameA(
        this.windowCall.windowNameAddress,
      );
      const resultValue = result.toString();
      const completed = {
        callId: this.traceFrame.callId,
        returnAddress: this.windowCall.returnAddress,
        extendedStyle: this.windowCall.extendedStyle,
        className: this.windowCall.className,
        classNameAfter,
        windowName: this.windowCall.windowName,
        windowNameAfter,
        style: this.windowCall.style,
        x: this.windowCall.x,
        y: this.windowCall.y,
        width: this.windowCall.width,
        height: this.windowCall.height,
        parent: this.windowCall.parent,
        menu: this.windowCall.menu,
        instance: this.windowCall.instance,
        createParams: this.windowCall.createParams,
        registeredClass: this.windowCall.registeredClass,
        result: resultValue,
        resultNonNull: !result.isNull(),
        lastErrorBefore: this.windowCall.lastErrorBefore,
        lastError,
        immediateConsumer: this.windowCall.immediateConsumer,
        callbacks: this.windowCall.callbacks,
        inputsPreserved:
          JSON.stringify(this.windowCall.className) ===
            JSON.stringify(classNameAfter) &&
          JSON.stringify(this.windowCall.windowName) ===
            JSON.stringify(windowNameAfter),
      };
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          createWindowExA: completed,
        },
        this.lastError,
      );
      if (!createWindowExACompletionRecorded) {
        createWindowExAMainModuleCalls.push(completed);
        setImmediate(maybeCompleteCreateWindowExAObservation);
      }
    },
  });
}

function attachBgiMainWindowProcedureHook() {
  const procedureRva = 0x00092550;
  if (procedureRva >= Process.mainModule.size) {
    throw new Error("BGI main window procedure RVA is outside the main module");
  }
  const address = Process.mainModule.base.add(procedureRva);
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const threadId = Process.getCurrentThreadId();
      this.message = args[1].toUInt32();
      this.recordSendMessage =
        selectedObservations.has("send-message-a") && this.message === 0x0080;
      this.setWindowPosCallId =
        activeSetWindowPosCallsByThread.get(threadId)?.at(-1) ?? null;
      this.recordSetWindowPos = this.setWindowPosCallId !== null;
      this.showWindowNormalCallId =
        activeShowWindowNormalCallsByThread.get(threadId)?.at(-1) ?? null;
      this.recordShowWindowNormal = this.showWindowNormalCallId !== null;
      this.updateWindowCallId =
        activeUpdateWindowCallsByThread.get(threadId)?.at(-1) ?? null;
      this.recordUpdateWindow = this.updateWindowCallId !== null;
      if (
        !this.recordSendMessage &&
        !this.recordSetWindowPos &&
        !this.recordShowWindowNormal &&
        !this.recordUpdateWindow
      ) {
        this.traceFrame = null;
        return;
      }
      this.windowHandlePointer = args[0];
      this.windowHandle = this.windowHandlePointer.toString();
      this.wParam = args[2].toUInt32();
      this.lParam = args[3].toUInt32();
      this.windowPosition =
        (this.message === 0x0046 || this.message === 0x0047) &&
        this.lParam !== 0
          ? snapshotWindowPosition(ptr(this.lParam))
          : null;
      this.nccalcSizeBefore =
        this.message === 0x0083 && this.wParam === 1 && this.lParam !== 0
          ? snapshotNccalcSizeParams(ptr(this.lParam))
          : null;
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.stateAtEntry =
        (this.recordSetWindowPos ||
          this.recordShowWindowNormal ||
          this.recordUpdateWindow) &&
        setWindowPosWindowStateSnapshot !== null
          ? setWindowPosWindowStateSnapshot(this.windowHandlePointer)
          : null;
      this.lastError = this.lastErrorBefore;
      this.traceFrame = beginCall(
        {
          module: Process.mainModule.name,
          api: "MainWindowProc",
          arguments: [
            this.windowHandle,
            this.message,
            this.wParam,
            this.lParam,
          ],
          lastErrorBefore: this.lastErrorBefore,
          windowProcedure: {
            profile: "bgi-main-window-procedure-rva-00092550-v1",
            procedureRva,
            windowHandle: this.windowHandle,
            message: this.message,
            wParam: this.wParam,
            lParam: this.lParam,
            windowPosition: this.windowPosition,
            nccalcSizeBefore: this.nccalcSizeBefore,
            stateAtEntry: this.stateAtEntry,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      const resultU32 = result.toUInt32();
      const nccalcSizeAfter =
        this.nccalcSizeBefore === null
          ? null
          : snapshotNccalcSizeParams(ptr(this.lParam));
      finishCall(
        this.traceFrame,
        {
          module: Process.mainModule.name,
          api: "MainWindowProc",
          result: resultValue,
          windowProcedure: { result: resultValue, resultU32, nccalcSizeAfter },
        },
        this.lastError,
      );
      const completed = {
        callId: this.traceFrame.callId,
        parentCallId: this.traceFrame.parentCallId,
        returnAddress: this.returnAddress.toString(),
        windowHandle: this.windowHandle,
        message: this.message,
        wParam: this.wParam,
        lParam: this.lParam,
        windowPosition: this.windowPosition,
        nccalcSizeBefore: this.nccalcSizeBefore,
        nccalcSizeAfter,
        stateAtEntry: this.stateAtEntry,
        result: resultValue,
        resultU32,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
      };
      if (this.recordSendMessage) {
        sendMessageAWindowProcedureCalls.push(completed);
      }
      if (this.recordSetWindowPos) {
        setWindowPosWindowProcedureCalls.push({
          ...completed,
          setWindowPosCallId: this.setWindowPosCallId,
        });
      }
      if (this.recordShowWindowNormal) {
        showWindowNormalWindowProcedureCalls.push({
          ...completed,
          showWindowCallId: this.showWindowNormalCallId,
        });
      }
      if (this.recordUpdateWindow) {
        updateWindowWindowProcedureCalls.push({
          ...completed,
          updateWindowCallId: this.updateWindowCallId,
        });
      }
    },
  });
}

function snapshotWindowPosition(address) {
  try {
    return {
      address: address.toString(),
      windowHandle: address.readPointer().toString(),
      insertAfter: address.add(4).readPointer().toString(),
      x: address.add(8).readS32(),
      y: address.add(12).readS32(),
      width: address.add(16).readS32(),
      height: address.add(20).readS32(),
      flags: address.add(24).readU32(),
    };
  } catch (error) {
    return { address: address.toString(), error: error.message };
  }
}

function attachSetWindowPosDefaultProcedureHook(module) {
  const name = "DefWindowProcA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const threadId = Process.getCurrentThreadId();
      this.setWindowPosCallId =
        activeSetWindowPosCallsByThread.get(threadId)?.at(-1) ?? null;
      const caller = describeCaller(this.returnAddress);
      if (
        this.setWindowPosCallId === null ||
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase()
      ) {
        this.traceFrame = null;
        return;
      }
      this.windowHandlePointer = args[0];
      this.windowHandle = this.windowHandlePointer.toString();
      this.message = args[1].toUInt32();
      this.wParam = args[2].toUInt32();
      this.lParam = args[3].toUInt32();
      this.nccalcSizeBefore =
        this.message === 0x0083 && this.wParam === 1 && this.lParam !== 0
          ? snapshotNccalcSizeParams(ptr(this.lParam))
          : null;
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.stateAtEntry =
        setWindowPosWindowStateSnapshot === null
          ? null
          : setWindowPosWindowStateSnapshot(this.windowHandlePointer);
      this.lastError = this.lastErrorBefore;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [
            this.windowHandle,
            this.message,
            this.wParam,
            this.lParam,
          ],
          lastErrorBefore: this.lastErrorBefore,
          setWindowPosDefaultProcedure: {
            setWindowPosCallId: this.setWindowPosCallId,
            windowHandle: this.windowHandle,
            message: this.message,
            wParam: this.wParam,
            lParam: this.lParam,
            nccalcSizeBefore: this.nccalcSizeBefore,
            stateAtEntry: this.stateAtEntry,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      const resultU32 = result.toUInt32();
      const nccalcSizeAfter =
        this.nccalcSizeBefore === null
          ? null
          : snapshotNccalcSizeParams(ptr(this.lParam));
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          setWindowPosDefaultProcedure: {
            result: resultValue,
            resultU32,
            nccalcSizeAfter,
          },
        },
        this.lastError,
      );
      setWindowPosDefaultProcedureCalls.push({
        callId: this.traceFrame.callId,
        parentCallId: this.traceFrame.parentCallId,
        setWindowPosCallId: this.setWindowPosCallId,
        returnAddress: this.returnAddress.toString(),
        windowHandle: this.windowHandle,
        message: this.message,
        wParam: this.wParam,
        lParam: this.lParam,
        nccalcSizeBefore: this.nccalcSizeBefore,
        nccalcSizeAfter,
        stateAtEntry: this.stateAtEntry,
        result: resultValue,
        resultU32,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
      });
    },
  });
}

function attachSetWindowPosHook(module) {
  const name = "SetWindowPos";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);
  attachBgiMainWindowProcedureHook();
  attachSetWindowPosDefaultProcedureHook(module);

  const stateQueryExports = {
    getWindowRect: findExport(module, "GetWindowRect"),
    getClientRect: findExport(module, "GetClientRect"),
    clientToScreen: findExport(module, "ClientToScreen"),
    getWindowLongA: findExport(module, "GetWindowLongA"),
    isWindowVisible: findExport(module, "IsWindowVisible"),
    getUpdateRect: findExport(module, "GetUpdateRect"),
    getActiveWindow: findExport(module, "GetActiveWindow"),
    getFocus: findExport(module, "GetFocus"),
    getForegroundWindow: findExport(module, "GetForegroundWindow"),
  };
  if (
    focusedSetWindowPosStateObservation &&
    Object.values(stateQueryExports).some(
      (exportAddress) => exportAddress === null,
    )
  ) {
    throw new Error("SetWindowPos state-query exports are unavailable");
  }
  const getWindowRect =
    stateQueryExports.getWindowRect === null
      ? null
      : new NativeFunction(
          stateQueryExports.getWindowRect,
          "int32",
          ["pointer", "pointer"],
          "stdcall",
        );
  const getClientRect =
    stateQueryExports.getClientRect === null
      ? null
      : new NativeFunction(
          stateQueryExports.getClientRect,
          "int32",
          ["pointer", "pointer"],
          "stdcall",
        );
  const clientToScreen =
    stateQueryExports.clientToScreen === null
      ? null
      : new NativeFunction(
          stateQueryExports.clientToScreen,
          "int32",
          ["pointer", "pointer"],
          "stdcall",
        );
  const getWindowLongA =
    stateQueryExports.getWindowLongA === null
      ? null
      : new NativeFunction(
          stateQueryExports.getWindowLongA,
          "int32",
          ["pointer", "int32"],
          "stdcall",
        );
  const isWindowVisible =
    stateQueryExports.isWindowVisible === null
      ? null
      : new NativeFunction(
          stateQueryExports.isWindowVisible,
          "int32",
          ["pointer"],
          "stdcall",
        );
  const getUpdateRect =
    stateQueryExports.getUpdateRect === null
      ? null
      : new NativeFunction(
          stateQueryExports.getUpdateRect,
          "int32",
          ["pointer", "pointer", "int32"],
          "stdcall",
        );
  const getActiveWindow =
    stateQueryExports.getActiveWindow === null
      ? null
      : new NativeFunction(
          stateQueryExports.getActiveWindow,
          "pointer",
          [],
          "stdcall",
        );
  const getFocus =
    stateQueryExports.getFocus === null
      ? null
      : new NativeFunction(
          stateQueryExports.getFocus,
          "pointer",
          [],
          "stdcall",
        );
  const getForegroundWindow =
    stateQueryExports.getForegroundWindow === null
      ? null
      : new NativeFunction(
          stateQueryExports.getForegroundWindow,
          "pointer",
          [],
          "stdcall",
        );
  const windowRect = Memory.alloc(16);
  const clientRect = Memory.alloc(16);
  const clientOrigin = Memory.alloc(8);
  const updateRect = Memory.alloc(16);
  const rectFields = (address) => ({
    left: address.readS32(),
    top: address.add(4).readS32(),
    right: address.add(8).readS32(),
    bottom: address.add(12).readS32(),
  });
  const snapshotWindowState = (windowHandle) => {
    if (
      getWindowRect === null ||
      getClientRect === null ||
      clientToScreen === null ||
      getWindowLongA === null ||
      isWindowVisible === null ||
      getUpdateRect === null ||
      getActiveWindow === null ||
      getFocus === null ||
      getForegroundWindow === null
    ) {
      return null;
    }
    windowRect.writeByteArray(Array(16).fill(0));
    clientRect.writeByteArray(Array(16).fill(0));
    clientOrigin.writeByteArray(Array(8).fill(0));
    updateRect.writeByteArray(Array(16).fill(0));
    const windowRectResult =
      Number(getWindowRect(windowHandle, windowRect)) >>> 0;
    const clientRectResult =
      Number(getClientRect(windowHandle, clientRect)) >>> 0;
    const clientOriginResult =
      Number(clientToScreen(windowHandle, clientOrigin)) >>> 0;
    const updateRectResult =
      Number(getUpdateRect(windowHandle, updateRect, 0)) >>> 0;
    const windowRelation = (relatedWindow) => ({
      windowHandle: relatedWindow.toString(),
      isNull: relatedWindow.isNull(),
      isTarget: relatedWindow.equals(windowHandle),
    });
    return {
      style: Number(getWindowLongA(windowHandle, -16)) >>> 0,
      extendedStyle: Number(getWindowLongA(windowHandle, -20)) >>> 0,
      visible: Number(isWindowVisible(windowHandle)) !== 0,
      windowRectResult,
      windowRect: windowRectResult === 0 ? null : rectFields(windowRect),
      clientRectResult,
      clientRect: clientRectResult === 0 ? null : rectFields(clientRect),
      clientOriginResult,
      clientOrigin:
        clientOriginResult === 0
          ? null
          : { x: clientOrigin.readS32(), y: clientOrigin.add(4).readS32() },
      updateRectResult,
      updateRect: updateRectResult === 0 ? null : rectFields(updateRect),
      activeWindow: windowRelation(getActiveWindow()),
      focusWindow: windowRelation(getFocus()),
      foregroundWindow: windowRelation(getForegroundWindow()),
    };
  };
  setWindowPosWindowStateSnapshot = snapshotWindowState;

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      if (
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase()
      ) {
        this.traceFrame = null;
        return;
      }
      this.callerRva = caller.rva;
      this.windowHandlePointer = args[0];
      this.windowHandle = this.windowHandlePointer.toString();
      this.insertAfter = args[1].toString();
      this.x = signed32(args[2]);
      this.y = signed32(args[3]);
      this.width = signed32(args[4]);
      this.height = signed32(args[5]);
      this.flags = args[6].toUInt32();
      const initialPlacementCall =
        selectedObservations.has("set-window-pos") &&
        (caller.rva === 0x00067f88 || caller.rva === 0x00067f9f);
      const nonzeroSizeCall =
        focusedSetWindowPosSizeObservation &&
        this.width > 0 &&
        this.height > 0 &&
        (this.flags & 0x0000_0001) === 0;
      const sizedStateBefore = nonzeroSizeCall
        ? snapshotWindowState(this.windowHandlePointer)
        : null;
      const requestedResize =
        sizedStateBefore?.windowRect != null &&
        (sizedStateBefore.windowRect.right -
          sizedStateBefore.windowRect.left !==
          this.width ||
          sizedStateBefore.windowRect.bottom -
            sizedStateBefore.windowRect.top !==
            this.height);
      const sizedCall =
        nonzeroSizeCall &&
        (selectedObservations.has("set-window-pos-size") || requestedResize);
      const frameChangeMoveCall =
        focusedSetWindowPosFrameChangeMoveObservation &&
        caller.rva === 0x000938c2 &&
        this.insertAfter === "0x0" &&
        this.x === 317 &&
        this.y === 154 &&
        this.width === 0 &&
        this.height === 0 &&
        this.flags === 0x0000_0025;
      if (!initialPlacementCall && !sizedCall && !frameChangeMoveCall) {
        this.traceFrame = null;
        return;
      }
      this.observation = frameChangeMoveCall
        ? "set-window-pos-frame-change-move"
        : sizedCall
          ? selectedObservations.has("set-window-pos-resize")
            ? "set-window-pos-resize"
            : "set-window-pos-size"
          : "set-window-pos";
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.stateBefore = frameChangeMoveCall
        ? snapshotWindowState(this.windowHandlePointer)
        : sizedCall
          ? sizedStateBefore
          : null;
      this.lastError = this.lastErrorBefore;
      this.immediateConsumer = {
        apiContinuation: snapshotInstructionsFollowingDirectJump(
          this.returnAddress,
          64,
        ),
        frameCallerContinuations: snapshotX86FrameCallerContinuations(
          this.context,
          6,
          24,
        ),
      };
      const setWindowPos = {
        windowHandle: this.windowHandle,
        insertAfter: this.insertAfter,
        x: this.x,
        y: this.y,
        width: this.width,
        height: this.height,
        flags: this.flags,
        immediateConsumer: this.immediateConsumer,
      };
      if (this.stateBefore !== null) {
        setWindowPos.stateBefore = this.stateBefore;
      }
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [
            this.windowHandle,
            this.insertAfter,
            this.x,
            this.y,
            this.width,
            this.height,
            this.flags,
          ],
          lastErrorBefore: this.lastErrorBefore,
          setWindowPos,
        },
        this.returnAddress,
      );
      const stack =
        activeSetWindowPosCallsByThread.get(this.traceFrame.threadId) ?? [];
      stack.push(this.traceFrame.callId);
      activeSetWindowPosCallsByThread.set(this.traceFrame.threadId, stack);
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const stack =
        activeSetWindowPosCallsByThread.get(this.traceFrame.threadId) ?? [];
      if (stack.at(-1) === this.traceFrame.callId) {
        stack.pop();
      }
      if (stack.length === 0) {
        activeSetWindowPosCallsByThread.delete(this.traceFrame.threadId);
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      const resultU32 = result.toUInt32();
      this.stateAfter =
        this.observation === "set-window-pos-size" ||
        this.observation === "set-window-pos-resize" ||
        this.observation === "set-window-pos-frame-change-move"
          ? snapshotWindowState(this.windowHandlePointer)
          : null;
      this.lastError = lastError;
      const setWindowPos = { result: resultValue, resultU32 };
      if (this.stateAfter !== null) {
        setWindowPos.stateAfter = this.stateAfter;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          setWindowPos,
        },
        this.lastError,
      );
      if (!setWindowPosCompletionRecorded) {
        setWindowPosMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          callerRva: this.callerRva,
          windowHandle: this.windowHandle,
          insertAfter: this.insertAfter,
          x: this.x,
          y: this.y,
          width: this.width,
          height: this.height,
          flags: this.flags,
          result: resultValue,
          resultU32,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          observation: this.observation,
          stateBefore: this.stateBefore,
          stateAfter: this.stateAfter,
          immediateConsumer: this.immediateConsumer,
        });
        if (
          this.observation === "set-window-pos-size" ||
          this.observation === "set-window-pos-resize" ||
          this.observation === "set-window-pos-frame-change-move"
        ) {
          scheduleSetWindowPosStateOracle(
            module,
            {
              insertAfter: this.insertAfter,
              x: this.x,
              y: this.y,
              width: this.width,
              height: this.height,
              flags: this.flags,
            },
            this.observation,
          );
        }
        setImmediate(maybeCompleteSetWindowPosObservation);
      }
    },
  });
}

function attachShowWindowNormalHook(module) {
  const name = "ShowWindow";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  attachSetWindowPosHook(module);
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const windowHandlePointer = args[0];
      const command = args[1].toUInt32();
      if (
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase() ||
        caller.rva !== 0x000938da ||
        command !== 1
      ) {
        this.traceFrame = null;
        return;
      }
      if (setWindowPosWindowStateSnapshot === null) {
        throw new Error("ShowWindow state snapshot authority is unavailable");
      }
      this.callerRva = caller.rva;
      this.windowHandlePointer = windowHandlePointer;
      this.windowHandle = windowHandlePointer.toString();
      this.command = command;
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.stateBefore = setWindowPosWindowStateSnapshot(windowHandlePointer);
      this.immediateConsumer = {
        apiContinuation: snapshotInstructionsFollowingDirectJump(
          this.returnAddress,
          64,
        ),
        frameCallerContinuations: snapshotX86FrameCallerContinuations(
          this.context,
          6,
          24,
        ),
      };
      this.lastError = this.lastErrorBefore;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.windowHandle, this.command],
          lastErrorBefore: this.lastErrorBefore,
          showWindow: {
            profile: "x86-bgi-show-window-normal-v1",
            windowHandle: this.windowHandle,
            command: this.command,
            stateBefore: this.stateBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
      const stack =
        activeShowWindowNormalCallsByThread.get(this.traceFrame.threadId) ?? [];
      stack.push(this.traceFrame.callId);
      activeShowWindowNormalCallsByThread.set(this.traceFrame.threadId, stack);
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const stack =
        activeShowWindowNormalCallsByThread.get(this.traceFrame.threadId) ?? [];
      if (stack.at(-1) === this.traceFrame.callId) {
        stack.pop();
      }
      if (stack.length === 0) {
        activeShowWindowNormalCallsByThread.delete(this.traceFrame.threadId);
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      const resultU32 = result.toUInt32();
      const stateAfter = setWindowPosWindowStateSnapshot(
        this.windowHandlePointer,
      );
      this.lastError = lastError;
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          showWindow: { result: resultValue, resultU32, stateAfter },
        },
        this.lastError,
      );
      if (!showWindowNormalCompletionRecorded) {
        showWindowNormalMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          callerRva: this.callerRva,
          windowHandle: this.windowHandle,
          command: this.command,
          result: resultValue,
          resultU32,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          stateBefore: this.stateBefore,
          stateAfter,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleShowWindowNormalOracle(module, this.command);
        setImmediate(maybeCompleteShowWindowNormalObservation);
      }
    },
  });
}

function scheduleShowWindowNormalOracle(module, command) {
  if (showWindowNormalOracleScheduled) {
    return;
  }
  showWindowNormalOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "ShowWindow");
      const kernel32 = Process.getModuleByName("kernel32.dll");
      const setLastErrorAddress = findExport(kernel32, "SetLastError");
      if (address === null || setLastErrorAddress === null) {
        throw new Error("ShowWindow oracle exports are unavailable");
      }
      const showWindow = new SystemFunction(
        address,
        "int32",
        ["pointer", "int32"],
        "stdcall",
      );
      const setLastError = new SystemFunction(
        setLastErrorAddress,
        "void",
        ["uint32"],
        "stdcall",
      );
      const sentinel = 0x55667788;
      setLastError(sentinel);
      const outcome = showWindow(ptr("0x1234"), command);
      showWindowNormalOracleEvidence = {
        profile: "x86-show-window-normal-invalid-window-v1",
        call: {
          windowHandle: "0x1234",
          command,
          sentinel,
          result: Number(outcome.value) >>> 0,
          lastError: Number(outcome.lastError) >>> 0,
        },
      };
      showWindowNormalOracleComplete = true;
      emit("show-window-normal-oracle", showWindowNormalOracleEvidence);
      maybeCompleteShowWindowNormalObservation();
    } catch (error) {
      emit("show-window-normal-oracle-error", { error: error.message });
    }
  });
}

function attachUpdateWindowHook(module) {
  const name = "UpdateWindow";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  attachSetWindowPosHook(module);
  attachUpdateWindowPaintHooks(module);
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      if (
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase() ||
        caller.rva !== 0x000938e7
      ) {
        this.traceFrame = null;
        return;
      }
      if (setWindowPosWindowStateSnapshot === null) {
        throw new Error("UpdateWindow state snapshot authority is unavailable");
      }
      this.callerRva = caller.rva;
      this.windowHandlePointer = args[0];
      this.windowHandle = this.windowHandlePointer.toString();
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.stateBefore = setWindowPosWindowStateSnapshot(
        this.windowHandlePointer,
      );
      this.immediateConsumer = {
        apiContinuation: snapshotInstructionsFollowingDirectJump(
          this.returnAddress,
          64,
        ),
        frameCallerContinuations: snapshotX86FrameCallerContinuations(
          this.context,
          6,
          24,
        ),
      };
      this.lastError = this.lastErrorBefore;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.windowHandle],
          lastErrorBefore: this.lastErrorBefore,
          updateWindow: {
            profile: "x86-bgi-update-window-full-client-paint-v1",
            windowHandle: this.windowHandle,
            stateBefore: this.stateBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
      const stack =
        activeUpdateWindowCallsByThread.get(this.traceFrame.threadId) ?? [];
      stack.push(this.traceFrame.callId);
      activeUpdateWindowCallsByThread.set(this.traceFrame.threadId, stack);
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const stack =
        activeUpdateWindowCallsByThread.get(this.traceFrame.threadId) ?? [];
      if (stack.at(-1) === this.traceFrame.callId) {
        stack.pop();
      }
      if (stack.length === 0) {
        activeUpdateWindowCallsByThread.delete(this.traceFrame.threadId);
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      const resultU32 = result.toUInt32();
      const stateAfter = setWindowPosWindowStateSnapshot(
        this.windowHandlePointer,
      );
      this.lastError = lastError;
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          updateWindow: { result: resultValue, resultU32, stateAfter },
        },
        this.lastError,
      );
      if (!updateWindowCompletionRecorded) {
        updateWindowMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          callerRva: this.callerRva,
          windowHandle: this.windowHandle,
          result: resultValue,
          resultU32,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          stateBefore: this.stateBefore,
          stateAfter,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleUpdateWindowOracle(module);
        setImmediate(maybeCompleteUpdateWindowObservation);
      }
    },
  });
}

function attachUpdateWindowPaintHooks(module) {
  for (const name of ["BeginPaint", "EndPaint"]) {
    const address = findExport(module, name);
    if (address === null) {
      throw new Error(`${name} export is unavailable`);
    }
    const key = address.toString();
    if (attached.has(key)) {
      continue;
    }
    attached.add(key);
    Interceptor.attach(address, {
      onEnter(args) {
        const threadId = Process.getCurrentThreadId();
        this.updateWindowCallId =
          activeUpdateWindowCallsByThread.get(threadId)?.at(-1) ?? null;
        if (this.updateWindowCallId === null) {
          this.traceFrame = null;
          return;
        }
        this.name = name;
        this.windowHandlePointer = args[0];
        this.windowHandle = this.windowHandlePointer.toString();
        this.paintStructPointer = args[1];
        this.paintStructAddress = this.paintStructPointer.toString();
        this.lastErrorBefore = Number(this.lastError) >>> 0;
        this.stateBefore = setWindowPosWindowStateSnapshot(
          this.windowHandlePointer,
        );
        this.paintStructBefore = snapshotPaintStruct(this.paintStructPointer);
        this.lastError = this.lastErrorBefore;
        this.traceFrame = beginCall(
          {
            module: module.name,
            api: name,
            arguments: [this.windowHandle, this.paintStructAddress],
            lastErrorBefore: this.lastErrorBefore,
            updateWindowPaint: {
              profile: "x86-bgi-update-window-paint-api-v1",
              updateWindowCallId: this.updateWindowCallId,
              stateBefore: this.stateBefore,
              paintStructBefore: this.paintStructBefore,
            },
          },
          this.returnAddress,
        );
      },
      onLeave(result) {
        if (this.traceFrame === null) {
          return;
        }
        const lastError = Number(this.lastError) >>> 0;
        const resultValue = result.toString();
        const resultU32 = result.toUInt32();
        const stateAfter = setWindowPosWindowStateSnapshot(
          this.windowHandlePointer,
        );
        const paintStructAfter = snapshotPaintStruct(this.paintStructPointer);
        this.lastError = lastError;
        finishCall(
          this.traceFrame,
          {
            module: module.name,
            api: this.name,
            result: resultValue,
            updateWindowPaint: {
              result: resultValue,
              resultU32,
              stateAfter,
              paintStructAfter,
            },
          },
          this.lastError,
        );
        updateWindowPaintCalls.push({
          callId: this.traceFrame.callId,
          parentCallId: this.traceFrame.parentCallId,
          updateWindowCallId: this.updateWindowCallId,
          api: this.name,
          returnAddress: this.returnAddress.toString(),
          windowHandle: this.windowHandle,
          paintStructAddress: this.paintStructAddress,
          result: resultValue,
          resultU32,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          stateBefore: this.stateBefore,
          stateAfter,
          paintStructBefore: this.paintStructBefore,
          paintStructAfter,
        });
      },
    });
  }
}

function scheduleUpdateWindowOracle(module) {
  if (updateWindowOracleScheduled) {
    return;
  }
  updateWindowOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "UpdateWindow");
      const beginPaintAddress = findExport(module, "BeginPaint");
      const endPaintAddress = findExport(module, "EndPaint");
      const kernel32 = Process.getModuleByName("kernel32.dll");
      const setLastErrorAddress = findExport(kernel32, "SetLastError");
      if (
        address === null ||
        beginPaintAddress === null ||
        endPaintAddress === null ||
        setLastErrorAddress === null
      ) {
        throw new Error("UpdateWindow oracle exports are unavailable");
      }
      const updateWindow = new SystemFunction(
        address,
        "int32",
        ["pointer"],
        "stdcall",
      );
      const setLastError = new SystemFunction(
        setLastErrorAddress,
        "void",
        ["uint32"],
        "stdcall",
      );
      const beginPaint = new SystemFunction(
        beginPaintAddress,
        "pointer",
        ["pointer", "pointer"],
        "stdcall",
      );
      const endPaint = new SystemFunction(
        endPaintAddress,
        "int32",
        ["pointer", "pointer"],
        "stdcall",
      );
      const sentinel = 0x55667788;
      setLastError(sentinel);
      const outcome = updateWindow(ptr("0x1234"));
      const paintStruct = Memory.alloc(64);
      paintStruct.writeByteArray(Array(64).fill(0xa5));
      const paintStructBefore = snapshotPaintStruct(paintStruct);
      setLastError(sentinel);
      const beginPaintOutcome = beginPaint(ptr("0x1234"), paintStruct);
      const paintStructAfterBeginPaint = snapshotPaintStruct(paintStruct);
      paintStruct.writeByteArray(Array(64).fill(0x5a));
      const paintStructBeforeEndPaint = snapshotPaintStruct(paintStruct);
      setLastError(sentinel);
      const endPaintOutcome = endPaint(ptr("0x1234"), paintStruct);
      const paintStructAfterEndPaint = snapshotPaintStruct(paintStruct);
      updateWindowOracleEvidence = {
        profile: "x86-update-window-paint-invalid-window-v1",
        call: {
          windowHandle: "0x1234",
          sentinel,
          result: Number(outcome.value) >>> 0,
          lastError: Number(outcome.lastError) >>> 0,
        },
        beginPaint: {
          windowHandle: "0x1234",
          sentinel,
          paintStructBefore,
          result: beginPaintOutcome.value.toString(),
          lastError: Number(beginPaintOutcome.lastError) >>> 0,
          paintStructAfter: paintStructAfterBeginPaint,
        },
        endPaint: {
          windowHandle: "0x1234",
          sentinel,
          paintStructBefore: paintStructBeforeEndPaint,
          result: Number(endPaintOutcome.value) >>> 0,
          lastError: Number(endPaintOutcome.lastError) >>> 0,
          paintStructAfter: paintStructAfterEndPaint,
        },
      };
      updateWindowOracleComplete = true;
      emit("update-window-oracle", updateWindowOracleEvidence);
      maybeCompleteUpdateWindowObservation();
    } catch (error) {
      emit("update-window-oracle-error", { error: error.message });
    }
  });
}

function scheduleSetWindowPosStateOracle(module, reachedCall, observation) {
  if (setWindowPosStateOracleScheduled) {
    return;
  }
  setWindowPosStateOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "SetWindowPos");
      const kernel32 = Process.getModuleByName("kernel32.dll");
      const setLastErrorAddress = findExport(kernel32, "SetLastError");
      if (address === null || setLastErrorAddress === null) {
        throw new Error("SetWindowPos state oracle exports are unavailable");
      }
      const setWindowPos = new SystemFunction(
        address,
        "int32",
        ["pointer", "pointer", "int32", "int32", "int32", "int32", "uint32"],
        "stdcall",
      );
      const setLastError = new SystemFunction(
        setLastErrorAddress,
        "void",
        ["uint32"],
        "stdcall",
      );
      const sentinel = 0x55667788;
      setLastError(sentinel);
      const outcome = setWindowPos(
        ptr("0x1234"),
        ptr(reachedCall.insertAfter),
        reachedCall.x,
        reachedCall.y,
        reachedCall.width,
        reachedCall.height,
        reachedCall.flags,
      );
      const frameChangeMove =
        observation === "set-window-pos-frame-change-move";
      setWindowPosStateOracleEvidence = {
        profile: frameChangeMove
          ? "x86-set-window-pos-frame-change-move-invalid-window-v1"
          : "x86-set-window-pos-size-invalid-window-v1",
        call: {
          windowHandle: "0x1234",
          ...reachedCall,
          sentinel,
          result: Number(outcome.value) >>> 0,
          lastError: Number(outcome.lastError) >>> 0,
        },
      };
      setWindowPosStateOracleComplete = true;
      emit(
        frameChangeMove
          ? "set-window-pos-frame-change-move-oracle"
          : "set-window-pos-size-oracle",
        setWindowPosStateOracleEvidence,
      );
      maybeCompleteSetWindowPosObservation();
    } catch (error) {
      emit(
        observation === "set-window-pos-frame-change-move"
          ? "set-window-pos-frame-change-move-oracle-error"
          : "set-window-pos-size-oracle-error",
        { error: error.message },
      );
    }
  });
}

function attachSendMessageAHook(module) {
  const name = "SendMessageA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);
  attachBgiMainWindowProcedureHook();

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      if (
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase()
      ) {
        this.traceFrame = null;
        return;
      }
      this.recordSendMessageObservation =
        selectedObservations.has("send-message-a");
      this.recordImeControl =
        focusedImeContextObservation && args[1].toUInt32() === 0x0283;
      if (!this.recordSendMessageObservation && !this.recordImeControl) {
        this.traceFrame = null;
        return;
      }
      this.windowHandle = args[0].toString();
      this.message = args[1].toUInt32();
      this.wParam = args[2].toUInt32();
      this.lParam = args[3].toUInt32();
      this.callerRva = caller.rva;
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = {
        apiContinuation: snapshotInstructionsFollowingDirectJump(
          this.returnAddress,
          48,
        ),
        frameCallerContinuations: snapshotX86FrameCallerContinuations(
          this.context,
          6,
          20,
        ),
      };
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [
            this.windowHandle,
            this.message,
            this.wParam,
            this.lParam,
          ],
          lastErrorBefore: this.lastErrorBefore,
          sendMessageA: {
            windowHandle: this.windowHandle,
            message: this.message,
            wParam: this.wParam,
            lParam: this.lParam,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      const resultU32 = result.toUInt32();
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          sendMessageA: {
            result: resultValue,
            resultU32,
          },
        },
        this.lastError,
      );
      if (
        this.recordSendMessageObservation &&
        !sendMessageACompletionRecorded
      ) {
        sendMessageAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          windowHandle: this.windowHandle,
          message: this.message,
          wParam: this.wParam,
          lParam: this.lParam,
          result: resultValue,
          resultU32,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        setImmediate(maybeCompleteSendMessageAObservation);
      }
      if (this.recordImeControl && !imeContextCompletionRecorded) {
        imeControlMessageCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          callerRva: this.callerRva,
          windowHandle: this.windowHandle,
          message: this.message,
          wParam: this.wParam,
          lParam: this.lParam,
          result: resultValue,
          resultU32,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        setImmediate(maybeCompleteImeContextObservation);
      }
    },
  });
}

function attachLoadIconAHook(module) {
  const name = "LoadIconA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" && loadIconAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.instance = args[0];
      this.resourceName = args[1];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = loadIconAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.resource = snapshotResourceNameA(this.resourceName);
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              48,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              6,
              20,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.instance.toString(), this.resource],
          lastErrorBefore: this.lastErrorBefore,
          loadIconA: {
            instance: this.instance.toString(),
            instanceIsMainModule:
              this.instance.toString() === Process.mainModule.base.toString(),
            resource: this.resource,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      if (this.oracleCaller) {
        loadIconAOracleCall.hookResult = resultValue;
        loadIconAOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          loadIconA: {
            result: resultValue,
            resultNonNull: !result.isNull(),
          },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && !loadIconACompletionRecorded) {
        loadIconAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          instance: this.instance.toString(),
          instanceIsMainModule:
            this.instance.toString() === Process.mainModule.base.toString(),
          resource: this.resource,
          result: resultValue,
          resultNonNull: !result.isNull(),
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleLoadIconAOracle(module);
        setImmediate(maybeCompleteLoadIconAObservation);
      }
    },
  });
}

function attachLoadImageAHook(module) {
  const name = "LoadImageA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" && loadImageAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.instance = args[0];
      this.resourceName = args[1];
      this.imageType = args[2].toUInt32();
      this.width = args[3].toUInt32();
      this.height = args[4].toUInt32();
      this.flags = args[5].toUInt32();
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = loadImageAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.resource = snapshotResourceNameA(this.resourceName);
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              48,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              6,
              20,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [
            this.instance.toString(),
            this.resource,
            this.imageType,
            this.width,
            this.height,
            this.flags,
          ],
          lastErrorBefore: this.lastErrorBefore,
          loadImageA: {
            instance: this.instance.toString(),
            instanceIsMainModule:
              this.instance.toString() === Process.mainModule.base.toString(),
            resource: this.resource,
            imageType: this.imageType,
            width: this.width,
            height: this.height,
            flags: this.flags,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      if (this.oracleCaller) {
        loadImageAOracleCall.hookResult = resultValue;
        loadImageAOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          loadImageA: {
            result: resultValue,
            resultNonNull: !result.isNull(),
          },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && !loadImageACompletionRecorded) {
        loadImageAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          instance: this.instance.toString(),
          instanceIsMainModule:
            this.instance.toString() === Process.mainModule.base.toString(),
          resource: this.resource,
          imageType: this.imageType,
          width: this.width,
          height: this.height,
          flags: this.flags,
          result: resultValue,
          resultNonNull: !result.isNull(),
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleLoadImageAOracle(module);
        setImmediate(maybeCompleteLoadImageAObservation);
      }
    },
  });
}

function attachLoadCursorAHook(module) {
  const name = "LoadCursorA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" && loadCursorAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.instance = args[0];
      this.resourceName = args[1];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = loadCursorAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.resource = snapshotResourceNameA(this.resourceName);
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              48,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              6,
              20,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.instance.toString(), this.resource],
          lastErrorBefore: this.lastErrorBefore,
          loadCursorA: {
            instance: this.instance.toString(),
            instanceIsNull: this.instance.isNull(),
            resource: this.resource,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toString();
      if (this.oracleCaller) {
        loadCursorAOracleCall.hookResult = resultValue;
        loadCursorAOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: resultValue,
          loadCursorA: {
            result: resultValue,
            resultNonNull: !result.isNull(),
          },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && !loadCursorACompletionRecorded) {
        loadCursorAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          instance: this.instance.toString(),
          instanceIsNull: this.instance.isNull(),
          resource: this.resource,
          result: resultValue,
          resultNonNull: !result.isNull(),
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleLoadCursorAOracle(module);
        setImmediate(maybeCompleteLoadCursorAObservation);
      }
    },
  });
}

function attachRegisterClassExAHook(module) {
  const name = "RegisterClassExA";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        registerClassExAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.windowClassAddress = args[0];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = registerClassExAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.windowClassBefore = snapshotWndClassExA(this.windowClassAddress);
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              64,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              6,
              20,
            ),
          }
        : null;
      if (isOracleCaller) {
        registerClassExAOracleCall.hookWindowClassBefore =
          this.windowClassBefore;
      }
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.windowClassAddress.toString()],
          lastErrorBefore: this.lastErrorBefore,
          registerClassExA: {
            windowClassBefore: this.windowClassBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const atom = result.toUInt32();
      const windowClassAfter = snapshotWndClassExA(this.windowClassAddress);
      if (this.oracleCaller) {
        registerClassExAOracleCall.hookResult = atom;
        registerClassExAOracleCall.hookLastErrorAfter = lastError;
        registerClassExAOracleCall.hookWindowClassAfter = windowClassAfter;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          registerClassExA: {
            atom,
            succeeded: atom !== 0,
            windowClassAfter,
            inputPreserved:
              JSON.stringify(this.windowClassBefore.raw?.bytes ?? null) ===
              JSON.stringify(windowClassAfter.raw?.bytes ?? null),
          },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && !registerClassExACompletionRecorded) {
        registerClassExAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          windowClassAddress: this.windowClassAddress.toString(),
          windowClassBefore: this.windowClassBefore,
          windowClassAfter,
          atom,
          succeeded: atom !== 0,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleRegisterClassExAOracle(module);
        setImmediate(maybeCompleteRegisterClassExAObservation);
      }
    },
  });
}

function attachGetSystemMetricsHook(module) {
  const name = "GetSystemMetrics";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        getSystemMetricsOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.metricIndex = signed32(args[0]);
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = getSystemMetricsOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer =
        isMainModuleCaller && getSystemMetricsMainModuleCalls.length === 0
          ? {
              apiContinuation: snapshotInstructionsFollowingDirectJump(
                this.returnAddress,
                96,
              ),
              frameCallerContinuations: snapshotX86FrameCallerContinuations(
                this.context,
                4,
                20,
              ),
            }
          : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.metricIndex],
          lastErrorBefore: this.lastErrorBefore,
          getSystemMetrics: {
            metricIndex: this.metricIndex,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const metricValue = signed32(result);
      const lastError = Number(this.lastError) >>> 0;
      if (this.oracleCaller) {
        getSystemMetricsOracleCall.hookResult = metricValue;
        getSystemMetricsOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          getSystemMetrics: {
            metricIndex: this.metricIndex,
            metricValue,
          },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && !getSystemMetricsCompletionRecorded) {
        getSystemMetricsMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          metricIndex: this.metricIndex,
          metricValue,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleGetSystemMetricsOracle(module);
        setImmediate(maybeCompleteGetSystemMetricsObservation);
      }
    },
  });
}

function attachGetClientRectHook(module) {
  const name = "GetClientRect";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" && getClientRectOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.windowHandle = args[0];
      this.output = args[1];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = getClientRectOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.outputBefore = snapshotRect(this.output);
      this.immediateConsumer =
        isMainModuleCaller && getClientRectMainModuleCalls.length === 0
          ? snapshotInstructions(this.returnAddress, 96)
          : null;
      if (isOracleCaller) {
        getClientRectOracleCall.hookOutputBefore = this.outputBefore;
      }
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.windowHandle.toString(), this.output.toString()],
          lastErrorBefore: this.lastErrorBefore,
          getClientRect: {
            windowHandle: this.windowHandle.toString(),
            outputBefore: this.outputBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const resultValue = result.toUInt32();
      const lastError = Number(this.lastError) >>> 0;
      const outputAfter = snapshotRect(this.output);
      if (this.oracleCaller) {
        getClientRectOracleCall.hookResult = resultValue;
        getClientRectOracleCall.hookLastErrorAfter = lastError;
        getClientRectOracleCall.hookOutputAfter = outputAfter;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          getClientRect: {
            result: resultValue,
            succeeded: resultValue !== 0,
            outputAfter,
          },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && !getClientRectCompletionRecorded) {
        getClientRectMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          callerRva: this.traceFrame.caller.rva,
          windowHandle: this.windowHandle.toString(),
          outputBefore: this.outputBefore,
          outputAfter,
          result: resultValue,
          succeeded: resultValue !== 0,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleGetClientRectOracle(module, this.windowHandle);
        setImmediate(maybeCompleteGetClientRectObservation);
      }
    },
  });
}

function scheduleGetClientRectOracle(module, liveWindowHandle) {
  if (getClientRectOracleScheduled) {
    return;
  }
  getClientRectOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetClientRect");
      if (address === null) {
        throw new Error("GetClientRect oracle export is unavailable");
      }
      const getClientRect = new SystemFunction(
        address,
        "int32",
        ["pointer", "pointer"],
        "stdcall",
      );
      const invoke = ({ id, windowHandle, nullOutput, sentinel }) => {
        const output = nullOutput ? ptr(0) : Memory.alloc(16);
        if (!nullOutput) {
          output.writeByteArray(Array(16).fill(0xa5));
        }
        const outputBefore = snapshotRect(output);
        getClientRectOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
          hookOutputBefore: null,
          hookOutputAfter: null,
        };
        try {
          const outcome = getClientRect(windowHandle, output);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getClientRectOracleCall.hookResult === null ||
            getClientRectOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              "GetClientRect oracle did not pass through the hook",
            );
          }
          const outputAfter = snapshotRect(output);
          return {
            id,
            windowHandle: windowHandle.toString(),
            output: output.toString(),
            sentinel,
            systemResult,
            succeeded: systemResult !== 0,
            systemLastError,
            outputBefore,
            outputAfter,
            hookResult: getClientRectOracleCall.hookResult,
            hookLastErrorAfter: getClientRectOracleCall.hookLastErrorAfter,
            hookOutputBefore: getClientRectOracleCall.hookOutputBefore,
            hookOutputAfter: getClientRectOracleCall.hookOutputAfter,
            hookResultMatches:
              getClientRectOracleCall.hookResult === systemResult,
            hookAndSystemLastErrorMatch:
              getClientRectOracleCall.hookLastErrorAfter === systemLastError,
          };
        } finally {
          getClientRectOracleCall = null;
        }
      };
      const calls = [
        {
          id: "live-window",
          windowHandle: liveWindowHandle,
          nullOutput: false,
          sentinel: 0x11223344,
        },
        {
          id: "null-window",
          windowHandle: ptr(0),
          nullOutput: false,
          sentinel: 0x55667788,
        },
        {
          id: "invalid-window",
          windowHandle: ptr("0x1234"),
          nullOutput: false,
          sentinel: 0x7a6b5c4d,
        },
      ].map(invoke);
      getClientRectOracleEvidence = {
        profile: "x86-live-window-and-invalid-arguments-v1",
        calls,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      getClientRectOracleComplete = true;
      emit("get-client-rect-oracle", getClientRectOracleEvidence);
      maybeCompleteGetClientRectObservation();
    } catch (error) {
      emit("get-client-rect-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteGetClientRectObservation() {
  if (
    getClientRectCompletionRecorded ||
    !getClientRectOracleComplete ||
    getClientRectMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteGetClientRectObservation, 10);
    return;
  }
  getClientRectCompletionRecorded = true;
  emit("get-client-rect-complete", {
    profile: "x86-bgi-first-client-rect-v1",
    mainModuleCall: getClientRectMainModuleCalls[0],
    oracle: getClientRectOracleEvidence,
  });
  if (stopOnEvent === "get-client-rect-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function attachSetRectHook(module) {
  const name = "SetRect";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" && setRectOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.output = args[0];
      this.coordinates = [
        args[1].toInt32(),
        args[2].toInt32(),
        args[3].toInt32(),
        args[4].toInt32(),
      ];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = setRectOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.outputBefore = snapshotRect(this.output);
      this.immediateConsumer =
        isMainModuleCaller && setRectMainModuleCalls.length === 0
          ? snapshotInstructions(this.returnAddress, 96)
          : null;
      if (isOracleCaller) {
        setRectOracleCall.hookOutputBefore = this.outputBefore;
      }
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.output.toString(), ...this.coordinates],
          lastErrorBefore: this.lastErrorBefore,
          setRect: {
            output: this.output.toString(),
            coordinates: this.coordinates,
            outputBefore: this.outputBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const resultValue = result.toUInt32();
      const lastError = Number(this.lastError) >>> 0;
      const outputAfter = snapshotRect(this.output);
      if (this.oracleCaller) {
        setRectOracleCall.hookResult = resultValue;
        setRectOracleCall.hookLastErrorAfter = lastError;
        setRectOracleCall.hookOutputAfter = outputAfter;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          setRect: {
            result: resultValue,
            succeeded: resultValue !== 0,
            outputAfter,
          },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && !setRectCompletionRecorded) {
        setRectMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          callerRva: this.traceFrame.caller.rva,
          output: this.output.toString(),
          coordinates: this.coordinates,
          outputBefore: this.outputBefore,
          outputAfter,
          result: resultValue,
          succeeded: resultValue !== 0,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleSetRectOracle(module);
        setImmediate(maybeCompleteSetRectObservation);
      }
    },
  });
}

function scheduleSetRectOracle(module) {
  if (setRectOracleScheduled) {
    return;
  }
  setRectOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "SetRect");
      if (address === null) {
        throw new Error("SetRect oracle export is unavailable");
      }
      const setRect = new SystemFunction(
        address,
        "int32",
        ["pointer", "int32", "int32", "int32", "int32"],
        "stdcall",
      );
      const invoke = ({ id, coordinates, nullOutput, sentinel }) => {
        const output = nullOutput ? ptr(0) : Memory.alloc(16);
        if (!nullOutput) {
          output.writeByteArray(Array(16).fill(0xa5));
        }
        const outputBefore = snapshotRect(output);
        setRectOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
          hookOutputBefore: null,
          hookOutputAfter: null,
        };
        try {
          const outcome = setRect(output, ...coordinates);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            setRectOracleCall.hookResult === null ||
            setRectOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error("SetRect oracle did not pass through the hook");
          }
          const outputAfter = snapshotRect(output);
          return {
            id,
            output: output.toString(),
            coordinates,
            sentinel,
            systemResult,
            succeeded: systemResult !== 0,
            systemLastError,
            outputBefore,
            outputAfter,
            hookResult: setRectOracleCall.hookResult,
            hookLastErrorAfter: setRectOracleCall.hookLastErrorAfter,
            hookOutputBefore: setRectOracleCall.hookOutputBefore,
            hookOutputAfter: setRectOracleCall.hookOutputAfter,
            hookResultMatches: setRectOracleCall.hookResult === systemResult,
            hookAndSystemLastErrorMatch:
              setRectOracleCall.hookLastErrorAfter === systemLastError,
          };
        } finally {
          setRectOracleCall = null;
        }
      };
      const calls = [
        {
          id: "primary-full-presentation",
          coordinates: [0, 0, 1280, 720],
          nullOutput: false,
          sentinel: 0x11223344,
        },
        {
          id: "signed-coordinates",
          coordinates: [-7, 9, 123, -456],
          nullOutput: false,
          sentinel: 0x55667788,
        },
        {
          id: "null-output",
          coordinates: [1, 2, 3, 4],
          nullOutput: true,
          sentinel: 0x7a6b5c4d,
        },
      ].map(invoke);
      setRectOracleEvidence = {
        profile: "x86-primary-signed-and-null-output-v1",
        calls,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      setRectOracleComplete = true;
      emit("set-rect-oracle", setRectOracleEvidence);
      maybeCompleteSetRectObservation();
    } catch (error) {
      emit("set-rect-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteSetRectObservation() {
  if (
    setRectCompletionRecorded ||
    !setRectOracleComplete ||
    setRectMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteSetRectObservation, 10);
    return;
  }
  setRectCompletionRecorded = true;
  emit("set-rect-complete", {
    profile: "x86-bgi-first-set-rect-full-presentation-v1",
    mainModuleCall: setRectMainModuleCalls[0],
    oracle: setRectOracleEvidence,
  });
  if (stopOnEvent === "set-rect-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function attachTimerProcedure(callback) {
  if (callback.isNull()) {
    return;
  }
  const key = callback.toString().toLowerCase();
  if (attachedTimerProcedures.has(key)) {
    return;
  }
  attachedTimerProcedures.add(key);
  Interceptor.attach(callback, {
    onEnter(args) {
      if (setTimerCallbackCalls.length >= 16) {
        this.timerProcedureCall = null;
        return;
      }
      const record = {
        callback: callback.toString(),
        callbackIdentity: describeCaller(callback),
        caller: describeCaller(this.returnAddress),
        threadId: Process.getCurrentThreadId(),
        window: args[0].toString(),
        message: args[1].toUInt32(),
        timerId: args[2].toUInt32(),
        tickCount: args[3].toUInt32(),
        lastErrorBefore: Number(this.lastError) >>> 0,
      };
      this.timerProcedureCall = record;
      setTimerCallbackCalls.push(record);
    },
    onLeave() {
      const record = this.timerProcedureCall;
      if (record === null) {
        return;
      }
      record.lastErrorAfter = Number(this.lastError) >>> 0;
      setImmediate(maybeCompleteSetTimerObservation);
    },
  });
}

function attachSetTimerHook(module) {
  const name = "SetTimer";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      if (
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase()
      ) {
        this.traceFrame = null;
        return;
      }
      this.window = args[0];
      this.timerId = args[1].toUInt32();
      this.intervalMilliseconds = args[2].toUInt32();
      this.timerProcedure = args[3];
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = {
        apiContinuation: snapshotInstructionsFollowingDirectJump(
          this.returnAddress,
          32,
        ),
        frameCallerContinuations: snapshotX86FrameCallerContinuations(
          this.context,
          12,
          16,
        ),
      };
      attachTimerProcedure(this.timerProcedure);
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [
            this.window.toString(),
            this.timerId,
            this.intervalMilliseconds,
            this.timerProcedure.toString(),
          ],
          lastErrorBefore: this.lastErrorBefore,
          setTimer: {
            window: this.window.toString(),
            timerId: this.timerId,
            intervalMilliseconds: this.intervalMilliseconds,
            timerProcedure: this.timerProcedure.toString(),
            timerProcedureIdentity: describeCaller(this.timerProcedure),
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toUInt32();
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          setTimer: {
            result: resultValue,
            resultNonzero: resultValue !== 0,
            resultMatchesRequestedId: resultValue === this.timerId,
          },
        },
        this.lastError,
      );
      setTimerMainModuleCalls.push({
        callId: this.traceFrame.callId,
        returnAddress: this.returnAddress.toString(),
        caller: this.traceFrame.caller,
        threadId: this.traceFrame.threadId,
        window: this.window.toString(),
        timerId: this.timerId,
        intervalMilliseconds: this.intervalMilliseconds,
        timerProcedure: this.timerProcedure.toString(),
        timerProcedureIdentity: describeCaller(this.timerProcedure),
        result: resultValue,
        resultNonzero: resultValue !== 0,
        resultMatchesRequestedId: resultValue === this.timerId,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        immediateConsumer: this.immediateConsumer,
      });
      setImmediate(maybeCompleteSetTimerObservation);
    },
  });
}

function attachKillTimerHook(module) {
  const name = "KillTimer";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      if (
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase()
      ) {
        this.traceFrame = null;
        return;
      }
      this.window = args[0];
      this.timerId = args[1].toUInt32();
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.window.toString(), this.timerId],
          lastErrorBefore: this.lastErrorBefore,
          killTimer: {
            window: this.window.toString(),
            timerId: this.timerId,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue = result.toUInt32();
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          killTimer: { result: resultValue },
        },
        this.lastError,
      );
      killTimerMainModuleCalls.push({
        callId: this.traceFrame.callId,
        returnAddress: this.returnAddress.toString(),
        caller: this.traceFrame.caller,
        threadId: this.traceFrame.threadId,
        window: this.window.toString(),
        timerId: this.timerId,
        result: resultValue,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
      });
      setImmediate(maybeCompleteSetTimerObservation);
    },
  });
}

function attachCreatedThreadProcedure(startRoutine) {
  if (startRoutine.isNull()) {
    return;
  }
  const key = startRoutine.toString().toLowerCase();
  if (attachedThreadProcedures.has(key)) {
    return;
  }
  attachedThreadProcedures.add(key);
  Interceptor.attach(startRoutine, {
    onEnter(args) {
      const threadId = Process.getCurrentThreadId();
      const relatedCall = createThreadMainModuleCalls.find(
        (call) =>
          call.createdThreadId === threadId &&
          call.startRoutine.toLowerCase() === key,
      );
      const record = {
        startRoutine: startRoutine.toString(),
        startRoutineIdentity: describeCaller(startRoutine),
        caller: describeCaller(this.returnAddress),
        threadId,
        parameter: args[0].toString(),
        parameterBytes: snapshotBoundedRawBytes(args[0], 32, 32),
        relatedCreateThreadCallId: relatedCall?.callId ?? null,
        lastErrorBefore: Number(this.lastError) >>> 0,
      };
      this.createdThreadProcedureCall = record;
      createThreadStartCalls.push(record);
      setImmediate(maybeCompleteCreateThreadObservation);
    },
    onLeave(result) {
      const record = this.createdThreadProcedureCall;
      if (record === null || record === undefined) {
        return;
      }
      record.result = result.toUInt32();
      record.lastErrorAfter = Number(this.lastError) >>> 0;
      setImmediate(maybeCompleteCreateThreadObservation);
    },
  });
}

function attachCreateThreadHook(module) {
  const name = "CreateThread";
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      if (
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase()
      ) {
        this.traceFrame = null;
        return;
      }
      this.securityAttributes = args[0];
      this.stackSize = args[1].toUInt32();
      this.startRoutine = args[2];
      this.parameter = args[3];
      this.creationFlags = args[4].toUInt32();
      this.threadIdOutput = args[5];
      this.threadIdBefore = snapshotOptionalU32(this.threadIdOutput);
      this.parameterBytesBefore = snapshotBoundedRawBytes(
        this.parameter,
        32,
        32,
      );
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = {
        apiContinuation: snapshotInstructionsFollowingDirectJump(
          this.returnAddress,
          48,
        ),
        frameCallerContinuations: snapshotX86FrameCallerContinuations(
          this.context,
          12,
          20,
        ),
      };
      attachCreatedThreadProcedure(this.startRoutine);
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [
            this.securityAttributes.toString(),
            this.stackSize,
            this.startRoutine.toString(),
            this.parameter.toString(),
            this.creationFlags,
            this.threadIdOutput.toString(),
          ],
          lastErrorBefore: this.lastErrorBefore,
          createThread: {
            securityAttributes: this.securityAttributes.toString(),
            stackSize: this.stackSize,
            startRoutine: this.startRoutine.toString(),
            startRoutineIdentity: describeCaller(this.startRoutine),
            parameter: this.parameter.toString(),
            parameterBytesBefore: this.parameterBytesBefore,
            creationFlags: this.creationFlags,
            threadIdOutput: this.threadIdOutput.toString(),
            threadIdBefore: this.threadIdBefore,
            parameterAliasesThreadIdOutput: this.parameter.equals(
              this.threadIdOutput,
            ),
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const threadIdAfter = snapshotOptionalU32(this.threadIdOutput);
      const parameterBytesAfter = snapshotBoundedRawBytes(
        this.parameter,
        32,
        32,
      );
      const handle = result.toString();
      const call = {
        callId: this.traceFrame.callId,
        returnAddress: this.returnAddress.toString(),
        caller: this.traceFrame.caller,
        threadId: this.traceFrame.threadId,
        securityAttributes: this.securityAttributes.toString(),
        stackSize: this.stackSize,
        startRoutine: this.startRoutine.toString(),
        startRoutineIdentity: describeCaller(this.startRoutine),
        parameter: this.parameter.toString(),
        parameterBytesBefore: this.parameterBytesBefore,
        parameterBytesAfter,
        creationFlags: this.creationFlags,
        threadIdOutput: this.threadIdOutput.toString(),
        threadIdBefore: this.threadIdBefore,
        threadIdAfter,
        parameterAliasesThreadIdOutput: this.parameter.equals(
          this.threadIdOutput,
        ),
        handle,
        handleNonzero: !result.isNull(),
        createdThreadId:
          threadIdAfter.isNull || threadIdAfter.error !== undefined
            ? null
            : threadIdAfter.value,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        immediateConsumer: this.immediateConsumer,
      };
      createThreadMainModuleCalls.push(call);
      if (!result.isNull()) {
        trackedCreateThreadHandles.set(handle.toLowerCase(), call);
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: handle,
          createThread: call,
        },
        this.lastError,
      );
      setImmediate(maybeCompleteCreateThreadObservation);
    },
  });
}

function attachCreatedThreadLifecycleHook(module, name) {
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const handle = args[0].toString().toLowerCase();
      const relatedCall = trackedCreateThreadHandles.get(handle) ?? null;
      if (relatedCall === null) {
        this.traceFrame = null;
        return;
      }
      this.handle = handle;
      this.relatedCall = relatedCall;
      this.output = name === "GetExitCodeThread" ? args[1] : null;
      this.outputBefore =
        this.output === null ? null : snapshotOptionalU32(this.output);
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = {
        apiContinuation: snapshotInstructionsFollowingDirectJump(
          this.returnAddress,
          32,
        ),
        frameCallerContinuations: snapshotX86FrameCallerContinuations(
          this.context,
          8,
          16,
        ),
      };
      const extraArgument =
        name === "SetThreadAffinityMask" ||
        name === "SetThreadPriority" ||
        name === "WaitForSingleObject"
          ? args[1].toUInt32()
          : null;
      this.extraArgument = extraArgument;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments:
            extraArgument === null ? [handle] : [handle, extraArgument],
          lastErrorBefore: this.lastErrorBefore,
          createdThreadLifecycle: {
            relatedCreateThreadCallId: relatedCall.callId,
            handle,
            extraArgument,
            outputBefore: this.outputBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const record = {
        callId: this.traceFrame.callId,
        api: name,
        caller: this.traceFrame.caller,
        threadId: this.traceFrame.threadId,
        relatedCreateThreadCallId: this.relatedCall.callId,
        handle: this.handle,
        extraArgument: this.extraArgument,
        outputBefore: this.outputBefore,
        outputAfter:
          this.output === null ? null : snapshotOptionalU32(this.output),
        result: result.toUInt32(),
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        immediateConsumer: this.immediateConsumer,
      };
      createThreadLifecycleCalls.push(record);
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          createdThreadLifecycle: record,
        },
        this.lastError,
      );
      setImmediate(maybeCompleteCreateThreadObservation);
    },
  });
}

function maybeCompleteCreateThreadObservation() {
  if (createThreadCompletionRecorded) {
    return;
  }
  const targetCall = createThreadMainModuleCalls.find(
    (call) => call.caller.rva === 0x000a1fad,
  );
  if (targetCall === undefined || targetCall.createdThreadId === null) {
    return;
  }
  const targetLifecycleCalls = createThreadLifecycleCalls.filter(
    (call) => call.relatedCreateThreadCallId === targetCall.callId,
  );
  const targetStartCalls = createThreadStartCalls.filter(
    (call) =>
      call.relatedCreateThreadCallId === targetCall.callId ||
      (call.threadId === targetCall.createdThreadId &&
        call.startRoutine.toLowerCase() ===
          targetCall.startRoutine.toLowerCase()),
  );
  if (focusedSetThreadPriorityObservation) {
    const targetPriorityCall = targetLifecycleCalls.find(
      (call) =>
        call.api === "SetThreadPriority" && call.caller.rva === 0x00091b77,
    );
    if (targetPriorityCall === undefined) {
      return;
    }
    if (createThreadDrainCheckScheduled) {
      return;
    }
    createThreadDrainCheckScheduled = true;
    const expectedLifecycleCount = createThreadLifecycleCalls.length;
    const expectedStartCount = createThreadStartCalls.length;
    setTimeout(() => {
      createThreadDrainCheckScheduled = false;
      if (
        createThreadCompletionRecorded ||
        createThreadLifecycleCalls.length !== expectedLifecycleCount ||
        createThreadStartCalls.length !== expectedStartCount
      ) {
        maybeCompleteCreateThreadObservation();
        return;
      }
      createThreadCompletionRecorded = true;
      emit("set-thread-priority-complete", {
        profile: "x86-bgi-created-thread-priority-v1",
        createThreadCall: targetCall,
        targetCall: targetPriorityCall,
        mainModuleCalls: createThreadMainModuleCalls.map((call) => ({
          ...call,
        })),
        lifecycleCalls: createThreadLifecycleCalls.map((call) => ({ ...call })),
        startCalls: createThreadStartCalls.map((call) => ({ ...call })),
      });
      if (stopOnEvent === "set-thread-priority-complete") {
        recv("vine-trace-stop", () => {}).wait();
      }
    }, 100);
    return;
  }
  if (
    !targetLifecycleCalls.some((call) => call.api === "ResumeThread") ||
    targetStartCalls.length === 0
  ) {
    return;
  }
  if (createThreadDrainCheckScheduled) {
    return;
  }
  createThreadDrainCheckScheduled = true;
  const expectedLifecycleCount = createThreadLifecycleCalls.length;
  const expectedStartCount = createThreadStartCalls.length;
  setTimeout(() => {
    createThreadDrainCheckScheduled = false;
    if (
      createThreadCompletionRecorded ||
      createThreadLifecycleCalls.length !== expectedLifecycleCount ||
      createThreadStartCalls.length !== expectedStartCount
    ) {
      maybeCompleteCreateThreadObservation();
      return;
    }
    createThreadCompletionRecorded = true;
    emit("create-thread-complete", {
      profile: "x86-bgi-create-suspended-thread-v1",
      targetCall,
      mainModuleCalls: createThreadMainModuleCalls.map((call) => ({ ...call })),
      lifecycleCalls: createThreadLifecycleCalls.map((call) => ({ ...call })),
      startCalls: createThreadStartCalls.map((call) => ({ ...call })),
    });
    if (stopOnEvent === "create-thread-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 100);
}

function attachGetMonitorInfoAHook(module) {
  const name = "GetMonitorInfoA";
  const address = findExport(module, name);
  if (address === null) {
    throw new Error("GetMonitorInfoA export is unavailable");
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        getMonitorInfoAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.monitor = args[0];
      this.output = args[1];
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = getMonitorInfoAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.outputBefore = snapshotMonitorInfoA(this.output);
      this.immediateConsumer = isMainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              128,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              6,
              48,
            ),
          }
        : null;
      if (isOracleCaller) {
        getMonitorInfoAOracleCall.hookOutputBefore = this.outputBefore;
      }
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.monitor.toString(), this.output.toString()],
          lastErrorBefore: this.lastErrorBefore,
          getMonitorInfoA: {
            monitor: this.monitor.toString(),
            outputBefore: this.outputBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const resultValue = result.toUInt32();
      const lastError = Number(this.lastError) >>> 0;
      const outputAfter = snapshotMonitorInfoA(this.output);
      if (this.oracleCaller) {
        getMonitorInfoAOracleCall.hookResult = resultValue;
        getMonitorInfoAOracleCall.hookLastErrorAfter = lastError;
        getMonitorInfoAOracleCall.hookOutputAfter = outputAfter;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          getMonitorInfoA: {
            result: resultValue,
            succeeded: resultValue !== 0,
            outputAfter,
          },
        },
        this.lastError,
      );
      if (
        this.mainModuleCaller &&
        getMonitorInfoAMainModuleCalls.length === 0
      ) {
        getMonitorInfoAMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          callerRva: this.traceFrame.caller.rva,
          monitor: this.monitor.toString(),
          outputPointer: this.output.toString(),
          outputBefore: this.outputBefore,
          outputAfter,
          result: resultValue,
          succeeded: resultValue !== 0,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        scheduleGetMonitorInfoAOracle(module, this.monitor);
        setImmediate(maybeCompleteGetMonitorInfoAObservation);
      }
    },
  });
}

function scheduleGetMonitorInfoAOracle(module, liveMonitor) {
  if (getMonitorInfoAOracleScheduled) {
    return;
  }
  getMonitorInfoAOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetMonitorInfoA");
      if (address === null) {
        throw new Error("GetMonitorInfoA oracle export is unavailable");
      }
      const getMonitorInfoA = new SystemFunction(
        address,
        "int32",
        ["pointer", "pointer"],
        "stdcall",
      );
      const allocationBytes = 80;
      const invoke = ({ id, monitor, cbSize, sentinel }) => {
        const output = Memory.alloc(allocationBytes);
        output.writeByteArray(Array(allocationBytes).fill(0xa5));
        output.writeU32(cbSize);
        const outputBefore = snapshotMonitorInfoA(output);
        const outputRawBefore = snapshotBoundedRawBytes(
          output,
          allocationBytes,
          allocationBytes,
        );
        getMonitorInfoAOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
          hookOutputBefore: null,
          hookOutputAfter: null,
        };
        try {
          const outcome = getMonitorInfoA(monitor, output);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getMonitorInfoAOracleCall.hookResult === null ||
            getMonitorInfoAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              "GetMonitorInfoA oracle did not pass through the hook",
            );
          }
          const outputAfter = snapshotMonitorInfoA(output);
          const outputRawAfter = snapshotBoundedRawBytes(
            output,
            allocationBytes,
            allocationBytes,
          );
          return {
            id,
            monitor: monitor.toString(),
            output: output.toString(),
            allocationBytes,
            cbSize,
            sentinel,
            systemResult,
            succeeded: systemResult !== 0,
            systemLastError,
            outputBefore,
            outputAfter,
            outputRawBefore,
            outputRawAfter,
            hookResult: getMonitorInfoAOracleCall.hookResult,
            hookLastErrorAfter: getMonitorInfoAOracleCall.hookLastErrorAfter,
            hookOutputBefore: getMonitorInfoAOracleCall.hookOutputBefore,
            hookOutputAfter: getMonitorInfoAOracleCall.hookOutputAfter,
            hookResultMatches:
              getMonitorInfoAOracleCall.hookResult === systemResult,
            hookAndSystemLastErrorMatch:
              getMonitorInfoAOracleCall.hookLastErrorAfter === systemLastError,
          };
        } finally {
          getMonitorInfoAOracleCall = null;
        }
      };
      const calls = [
        {
          id: "live-monitor-info",
          monitor: liveMonitor,
          cbSize: 40,
          sentinel: 0x11223344,
        },
        {
          id: "live-monitor-info-ex-a",
          monitor: liveMonitor,
          cbSize: 72,
          sentinel: 0x22334455,
        },
        {
          id: "live-size-zero",
          monitor: liveMonitor,
          cbSize: 0,
          sentinel: 0x33445566,
        },
        {
          id: "live-size-39",
          monitor: liveMonitor,
          cbSize: 39,
          sentinel: 0x44556677,
        },
        {
          id: "live-size-41",
          monitor: liveMonitor,
          cbSize: 41,
          sentinel: 0x55667788,
        },
        {
          id: "live-size-71",
          monitor: liveMonitor,
          cbSize: 71,
          sentinel: 0x66778899,
        },
        {
          id: "live-size-73",
          monitor: liveMonitor,
          cbSize: 73,
          sentinel: 0x778899aa,
        },
        {
          id: "null-monitor",
          monitor: ptr(0),
          cbSize: 40,
          sentinel: 0x8899aabb,
        },
        {
          id: "invalid-monitor",
          monitor: ptr("0x1234"),
          cbSize: 40,
          sentinel: 0x99aabbcc,
        },
        {
          id: "null-monitor-size-zero",
          monitor: ptr(0),
          cbSize: 0,
          sentinel: 0xaabbccdd,
        },
        {
          id: "invalid-monitor-size-zero",
          monitor: ptr("0x1234"),
          cbSize: 0,
          sentinel: 0xbbccddee,
        },
      ].map(invoke);
      getMonitorInfoAOracleEvidence = {
        profile: "x86-live-monitor-size-and-handle-matrix-v1",
        calls,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      getMonitorInfoAOracleComplete = true;
      emit("get-monitor-info-a-oracle", getMonitorInfoAOracleEvidence);
      maybeCompleteGetMonitorInfoAObservation();
    } catch (error) {
      emit("get-monitor-info-a-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteGetMonitorInfoAObservation() {
  if (
    getMonitorInfoACompletionRecorded ||
    !getMonitorInfoAOracleComplete ||
    getMonitorInfoAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    getMonitorInfoAQuiescentNextCallId = null;
    setTimeout(maybeCompleteGetMonitorInfoAObservation, 10);
    return;
  }
  if (getMonitorInfoAQuiescentNextCallId !== nextCallId) {
    getMonitorInfoAQuiescentNextCallId = nextCallId;
    setTimeout(maybeCompleteGetMonitorInfoAObservation, 1000);
    return;
  }
  getMonitorInfoACompletionRecorded = true;
  emit("get-monitor-info-a-complete", {
    profile: "x86-bgi-first-monitor-info-a-v1",
    mainModuleCall: getMonitorInfoAMainModuleCalls[0],
    oracle: getMonitorInfoAOracleEvidence,
  });
  if (stopOnEvent === "get-monitor-info-a-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function snapshotNccalcSizeParams(address) {
  const raw = snapshotBoundedRawBytes(address, 52);
  if (raw.error !== undefined) {
    return {
      address: address.toString(),
      profile: "x86-nccalcsize-params-v1",
      raw,
    };
  }
  try {
    const windowPositionPointer = address.add(48).readPointer();
    return {
      address: address.toString(),
      profile: "x86-nccalcsize-params-v1",
      raw,
      rectangles: [0, 16, 32].map((offset) =>
        snapshotRect(address.add(offset)),
      ),
      windowPositionPointer: windowPositionPointer.toString(),
      windowPosition: windowPositionPointer.isNull()
        ? null
        : snapshotWindowPosition(windowPositionPointer),
    };
  } catch (error) {
    return {
      address: address.toString(),
      profile: "x86-nccalcsize-params-v1",
      raw,
      error: error.message,
    };
  }
}

function attachWindowDcHook(module, name) {
  const address = findExport(module, name);
  if (address === null) {
    throw new Error(`${name} export is unavailable`);
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        windowDcOracleCall !== null &&
        windowDcOracleCall.api === name;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }

      this.api = name;
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = windowDcOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.windowHandle = null;
      this.dc = null;
      this.object = null;
      this.objectIndex = null;
      this.rect = null;
      switch (name) {
        case "GetDC":
          this.windowHandle = args[0];
          this.arguments = [this.windowHandle.toString()];
          break;
        case "ReleaseDC":
          this.windowHandle = args[0];
          this.dc = args[1];
          this.arguments = [this.windowHandle.toString(), this.dc.toString()];
          break;
        case "FillRect":
          this.dc = args[0];
          this.rectPointer = args[1];
          this.object = args[2];
          this.rect = snapshotRect(this.rectPointer);
          this.arguments = [
            this.dc.toString(),
            this.rectPointer.toString(),
            this.object.toString(),
          ];
          break;
        case "GetStockObject":
          this.objectIndex = signed32(args[0]);
          this.arguments = [this.objectIndex];
          break;
        default:
          throw new Error(`unsupported window DC hook: ${name}`);
      }
      this.immediateConsumer =
        isMainModuleCaller && windowDcMainModuleCalls.length < 4
          ? snapshotInstructions(this.returnAddress, 48)
          : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: this.arguments,
          lastErrorBefore: this.lastErrorBefore,
          windowDc: {
            rect: this.rect,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const resultValue =
        this.api === "GetDC" || this.api === "GetStockObject"
          ? result.toString()
          : signed32(result);
      const lastError = Number(this.lastError) >>> 0;
      if (this.oracleCaller) {
        windowDcOracleCall.hookResult = resultValue;
        windowDcOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: this.api,
          result: result.toString(),
          windowDc: {
            result: resultValue,
            rect: this.rect,
          },
        },
        this.lastError,
      );
      if (!this.mainModuleCaller || windowDcCompletionRecorded) {
        return;
      }
      windowDcMainModuleCalls.push({
        callId: this.traceFrame.callId,
        api: this.api,
        returnAddress: this.returnAddress.toString(),
        callerRva: this.traceFrame.caller.rva,
        arguments: this.arguments,
        result: resultValue,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        rect: this.rect,
        immediateConsumer: this.immediateConsumer,
      });
      const firstGetDc = windowDcMainModuleCalls.find(
        (call) => call.api === "GetDC",
      );
      const requiredApis = ["GetDC", "GetStockObject", "FillRect", "ReleaseDC"];
      if (
        this.api === "ReleaseDC" &&
        firstGetDc !== undefined &&
        requiredApis.every((api) =>
          windowDcMainModuleCalls.some((call) => call.api === api),
        )
      ) {
        scheduleWindowDcOracle(ptr(firstGetDc.arguments[0]));
        setImmediate(maybeCompleteWindowDcObservation);
      }
    },
  });
}

function scheduleWindowDcOracle(liveWindowHandle) {
  if (windowDcOracleScheduled) {
    return;
  }
  windowDcOracleScheduled = true;
  setImmediate(() => {
    try {
      const user32 = Process.getModuleByName("user32.dll");
      const gdi32 = Process.getModuleByName("gdi32.dll");
      const getDcAddress = findExport(user32, "GetDC");
      const releaseDcAddress = findExport(user32, "ReleaseDC");
      const fillRectAddress = findExport(user32, "FillRect");
      const getStockObjectAddress = findExport(gdi32, "GetStockObject");
      if (
        getDcAddress === null ||
        releaseDcAddress === null ||
        fillRectAddress === null ||
        getStockObjectAddress === null
      ) {
        throw new Error("window DC oracle export is unavailable");
      }
      const getDc = new SystemFunction(
        getDcAddress,
        "pointer",
        ["pointer"],
        "stdcall",
      );
      const releaseDc = new SystemFunction(
        releaseDcAddress,
        "int32",
        ["pointer", "pointer"],
        "stdcall",
      );
      const fillRect = new SystemFunction(
        fillRectAddress,
        "int32",
        ["pointer", "pointer", "pointer"],
        "stdcall",
      );
      const getStockObject = new SystemFunction(
        getStockObjectAddress,
        "pointer",
        ["int32"],
        "stdcall",
      );
      const invoke = (id, api, sentinel, callback) => {
        windowDcOracleCall = {
          api,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = callback();
          const systemResult =
            api === "GetDC" || api === "GetStockObject"
              ? outcome.value.toString()
              : Number(outcome.value);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            windowDcOracleCall.hookResult === null ||
            windowDcOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(`${api} oracle did not pass through the hook`);
          }
          return {
            id,
            api,
            sentinel,
            systemResult,
            systemLastError,
            hookResult: windowDcOracleCall.hookResult,
            hookLastErrorAfter: windowDcOracleCall.hookLastErrorAfter,
            hookResultMatches: windowDcOracleCall.hookResult === systemResult,
            hookAndSystemLastErrorMatch:
              windowDcOracleCall.hookLastErrorAfter === systemLastError,
          };
        } finally {
          windowDcOracleCall = null;
        }
      };

      const calls = [];
      const firstGetDc = invoke("live-get-dc-first", "GetDC", 0x11112222, () =>
        getDc(liveWindowHandle),
      );
      calls.push(firstGetDc);
      const secondGetDc = invoke(
        "live-get-dc-second",
        "GetDC",
        0x22223333,
        () => getDc(liveWindowHandle),
      );
      calls.push(secondGetDc);
      if (
        firstGetDc.systemResult === "0x0" ||
        secondGetDc.systemResult === "0x0"
      ) {
        throw new Error("live GetDC oracle returned NULL");
      }
      const firstStockObject = invoke(
        "black-brush-first",
        "GetStockObject",
        0x33334444,
        () => getStockObject(4),
      );
      calls.push(firstStockObject);
      const secondStockObject = invoke(
        "black-brush-second",
        "GetStockObject",
        0x44445555,
        () => getStockObject(4),
      );
      calls.push(secondStockObject);
      if (firstStockObject.systemResult === "0x0") {
        throw new Error("BLACK_BRUSH oracle returned NULL");
      }
      const emptyRect = Memory.alloc(16);
      emptyRect.writeByteArray(Array(16).fill(0));
      calls.push(
        invoke("empty-fill", "FillRect", 0x55556666, () =>
          fillRect(
            ptr(firstGetDc.systemResult),
            emptyRect,
            ptr(firstStockObject.systemResult),
          ),
        ),
      );
      calls.push(
        invoke("release-first", "ReleaseDC", 0x66667777, () =>
          releaseDc(liveWindowHandle, ptr(firstGetDc.systemResult)),
        ),
      );
      calls.push(
        invoke("release-second", "ReleaseDC", 0x77778888, () =>
          releaseDc(liveWindowHandle, ptr(secondGetDc.systemResult)),
        ),
      );
      const afterReleaseGetDc = invoke(
        "live-get-dc-after-release",
        "GetDC",
        0x13572468,
        () => getDc(liveWindowHandle),
      );
      calls.push(afterReleaseGetDc);
      if (afterReleaseGetDc.systemResult !== "0x0") {
        calls.push(
          invoke("release-after-reacquire", "ReleaseDC", 0x24681357, () =>
            releaseDc(liveWindowHandle, ptr(afterReleaseGetDc.systemResult)),
          ),
        );
      }
      calls.push(
        invoke("invalid-window-get-dc", "GetDC", 0x7a6b5c4d, () =>
          getDc(ptr("0x1234")),
        ),
      );
      calls.push(
        invoke("invalid-hdc-fill", "FillRect", 0x10293847, () =>
          fillRect(
            ptr("0x1234"),
            emptyRect,
            ptr(firstStockObject.systemResult),
          ),
        ),
      );
      calls.push(
        invoke("invalid-hdc-release", "ReleaseDC", 0x56473829, () =>
          releaseDc(liveWindowHandle, ptr("0x1234")),
        ),
      );
      calls.push(
        invoke("invalid-stock-object", "GetStockObject", 0x6a5b4c3d, () =>
          getStockObject(-1),
        ),
      );

      windowDcOracleEvidence = {
        profile: "x86-live-cs-owndc-lifetime-v1",
        liveWindowHandle: liveWindowHandle.toString(),
        stockObjectIndex: 4,
        calls,
        relations: {
          repeatedGetDcSameIdentity:
            firstGetDc.systemResult === secondGetDc.systemResult,
          afterReleaseSameIdentity:
            firstGetDc.systemResult === afterReleaseGetDc.systemResult,
          repeatedStockObjectSameIdentity:
            firstStockObject.systemResult === secondStockObject.systemResult,
        },
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      windowDcOracleComplete = true;
      emit("window-dc-oracle", windowDcOracleEvidence);
      maybeCompleteWindowDcObservation();
    } catch (error) {
      emit("window-dc-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteWindowDcObservation() {
  if (
    windowDcCompletionRecorded ||
    !windowDcOracleComplete ||
    !["GetDC", "GetStockObject", "FillRect", "ReleaseDC"].every((api) =>
      windowDcMainModuleCalls.some((call) => call.api === api),
    )
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteWindowDcObservation, 10);
    return;
  }
  windowDcCompletionRecorded = true;
  emit("window-dc-complete", {
    profile: "x86-bgi-first-window-dc-fill-v1",
    mainModuleCalls: windowDcMainModuleCalls,
    oracle: windowDcOracleEvidence,
  });
  if (stopOnEvent === "window-dc-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function snapshotEnumLogFontExA(pointer) {
  const raw = snapshotBoundedRawBytes(pointer, 188, 188);
  if (raw.error !== undefined) {
    return { profile: "x86-enumlogfontexa-v1", raw };
  }
  const bytes = raw.bytes;
  const fixedAnsiField = (offset, capacity) => {
    const storage = bytes.slice(offset, offset + capacity);
    const terminator = storage.indexOf(0);
    return {
      bytes: terminator === -1 ? storage : storage.slice(0, terminator),
      terminated: terminator !== -1,
    };
  };
  const numericFields = {
    height: pointer.readS32(),
    width: pointer.add(4).readS32(),
    escapement: pointer.add(8).readS32(),
    orientation: pointer.add(12).readS32(),
    weight: pointer.add(16).readS32(),
    italic: pointer.add(20).readU8(),
    underline: pointer.add(21).readU8(),
    strikeOut: pointer.add(22).readU8(),
    charSet: pointer.add(23).readU8(),
    outPrecision: pointer.add(24).readU8(),
    clipPrecision: pointer.add(25).readU8(),
    quality: pointer.add(26).readU8(),
    pitchAndFamily: pointer.add(27).readU8(),
  };
  return {
    profile: "x86-enumlogfontexa-v1",
    raw,
    fields: {
      ...numericFields,
      faceNameBytes: bytes.slice(28, 60),
      fullNameBytes: bytes.slice(60, 124),
      styleBytes: bytes.slice(124, 156),
      scriptBytes: bytes.slice(156, 188),
    },
    semantic: {
      ...numericFields,
      faceName: fixedAnsiField(28, 32),
      fullName: fixedAnsiField(60, 64),
      style: fixedAnsiField(124, 32),
      script: fixedAnsiField(156, 32),
    },
  };
}

function snapshotNewTextMetricExA(pointer) {
  const raw = snapshotBoundedRawBytes(pointer, 96, 96);
  if (raw.error !== undefined) {
    return { profile: "x86-newtextmetricexa-v1", raw };
  }
  return {
    profile: "x86-newtextmetricexa-v1",
    raw,
    fields: {
      height: pointer.readS32(),
      ascent: pointer.add(4).readS32(),
      descent: pointer.add(8).readS32(),
      internalLeading: pointer.add(12).readS32(),
      externalLeading: pointer.add(16).readS32(),
      averageCharacterWidth: pointer.add(20).readS32(),
      maximumCharacterWidth: pointer.add(24).readS32(),
      weight: pointer.add(28).readS32(),
      overhang: pointer.add(32).readS32(),
      digitizedAspectX: pointer.add(36).readS32(),
      digitizedAspectY: pointer.add(40).readS32(),
      firstCharacter: pointer.add(44).readU8(),
      lastCharacter: pointer.add(45).readU8(),
      defaultCharacter: pointer.add(46).readU8(),
      breakCharacter: pointer.add(47).readU8(),
      italic: pointer.add(48).readU8(),
      underlined: pointer.add(49).readU8(),
      struckOut: pointer.add(50).readU8(),
      pitchAndFamily: pointer.add(51).readU8(),
      charSet: pointer.add(52).readU8(),
      flags: pointer.add(56).readU32(),
      sizeEm: pointer.add(60).readU32(),
      cellHeight: pointer.add(64).readU32(),
      averageWidth: pointer.add(68).readU32(),
      fontSignatureBytes: raw.bytes.slice(72, 96),
    },
  };
}

function attachScreenDcFontCallback(callback) {
  if (callback.isNull()) {
    return;
  }
  const key = callback.toString().toLowerCase();
  if (attachedScreenDcFontCallbacks.has(key)) {
    return;
  }
  attachedScreenDcFontCallbacks.add(key);
  Interceptor.attach(callback, {
    onEnter(args) {
      const threadId = Process.getCurrentThreadId();
      const active =
        activeScreenDcFontEnumerationsByThread.get(threadId) ?? null;
      this.screenDcFontCallback = null;
      if (active === null) {
        return;
      }
      const record = {
        callback: callback.toString(),
        caller: describeCaller(this.returnAddress),
        enumLogFontPointer: args[0].toString(),
        textMetricPointer: args[1].toString(),
        fontType: args[2].toUInt32(),
        callbackParameter: args[3].toString(),
        enumLogFontBefore: snapshotEnumLogFontExA(args[0]),
        textMetricBefore: snapshotNewTextMetricExA(args[1]),
        lastErrorBefore: Number(this.lastError) >>> 0,
      };
      this.screenDcFontCallback = record;
      this.enumLogFontPointer = args[0];
      this.textMetricPointer = args[1];
      active.callbacks.push(record);
    },
    onLeave(result) {
      const record = this.screenDcFontCallback;
      if (record === null) {
        return;
      }
      record.result = signed32(result);
      record.enumLogFontAfter = snapshotEnumLogFontExA(this.enumLogFontPointer);
      record.textMetricAfter = snapshotNewTextMetricExA(this.textMetricPointer);
      record.lastErrorAfter = Number(this.lastError) >>> 0;
    },
  });
}

function attachScreenDcHook(module, name) {
  const address = findExport(module, name);
  if (address === null) {
    throw new Error(`${name} export is unavailable`);
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        screenDcOracleCall !== null &&
        screenDcOracleCall.api === name;
      this.windowHandle =
        name === "GetDC" || name === "ReleaseDC" ? args[0] : null;
      this.dc =
        name === "EnumFontFamiliesExA"
          ? args[0]
          : name === "ReleaseDC"
            ? args[1]
            : null;
      this.logFontPointer = name === "EnumFontFamiliesExA" ? args[1] : null;
      this.fontCallback = name === "EnumFontFamiliesExA" ? args[2] : null;
      this.callbackParameter = name === "EnumFontFamiliesExA" ? args[3] : null;
      this.flags = name === "EnumFontFamiliesExA" ? args[4].toUInt32() : null;
      this.logFont =
        name === "EnumFontFamiliesExA"
          ? snapshotBoundedRawBytes(this.logFontPointer, 60, 60)
          : null;
      this.fontCallbacks = [];
      this.fontCallbackInstructions =
        name === "EnumFontFamiliesExA" && isMainModuleCaller
          ? snapshotInstructions(this.fontCallback, 96)
          : null;
      this.fontEnumerationThreadId = null;
      this.previousFontEnumeration = null;
      this.fontEnumeration = null;
      const relevantMainCall =
        isMainModuleCaller &&
        !screenDcCompletionRecorded &&
        ((name === "GetDC" &&
          this.windowHandle.isNull() &&
          caller.rva === 0x0002c155) ||
          (name === "EnumFontFamiliesExA" &&
            screenDcTargetHandle !== null &&
            this.dc.toString() === screenDcTargetHandle) ||
          (name === "ReleaseDC" &&
            this.windowHandle.isNull() &&
            screenDcTargetHandle !== null &&
            this.dc.toString() === screenDcTargetHandle));
      if (!relevantMainCall && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }

      this.api = name;
      this.mainModuleCaller = relevantMainCall;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = screenDcOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.arguments =
        name === "GetDC"
          ? [this.windowHandle.toString()]
          : name === "ReleaseDC"
            ? [this.windowHandle.toString(), this.dc.toString()]
            : [
                this.dc.toString(),
                this.logFontPointer.toString(),
                this.fontCallback.toString(),
                this.callbackParameter.toString(),
                this.flags,
              ];
      this.immediateConsumer = relevantMainCall
        ? snapshotInstructions(this.returnAddress, 48)
        : null;
      if (name === "EnumFontFamiliesExA") {
        attachScreenDcFontCallback(this.fontCallback);
        this.fontEnumerationThreadId = Process.getCurrentThreadId();
        this.previousFontEnumeration =
          activeScreenDcFontEnumerationsByThread.get(
            this.fontEnumerationThreadId,
          ) ?? null;
        this.fontEnumeration = {
          callback: this.fontCallback.toString(),
          callbackParameter: this.callbackParameter.toString(),
          callbacks: this.fontCallbacks,
        };
        activeScreenDcFontEnumerationsByThread.set(
          this.fontEnumerationThreadId,
          this.fontEnumeration,
        );
      }
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: this.arguments,
          lastErrorBefore: this.lastErrorBefore,
          screenDc: {
            immediateConsumer: this.immediateConsumer,
            logFont: this.logFont,
            fontCallbackInstructions: this.fontCallbackInstructions,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.fontEnumerationThreadId !== null) {
        const active = activeScreenDcFontEnumerationsByThread.get(
          this.fontEnumerationThreadId,
        );
        if (active === this.fontEnumeration) {
          if (this.previousFontEnumeration === null) {
            activeScreenDcFontEnumerationsByThread.delete(
              this.fontEnumerationThreadId,
            );
          } else {
            activeScreenDcFontEnumerationsByThread.set(
              this.fontEnumerationThreadId,
              this.previousFontEnumeration,
            );
          }
        }
      }
      if (this.traceFrame === null) {
        return;
      }
      const resultValue =
        this.api === "GetDC" ? result.toString() : signed32(result);
      const lastError = Number(this.lastError) >>> 0;
      if (this.oracleCaller) {
        screenDcOracleCall.hookResult = resultValue;
        screenDcOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: this.api,
          result: result.toString(),
          screenDc: {
            result: resultValue,
            logFont: this.logFont,
            fontCallbacks: this.fontCallbacks,
          },
        },
        this.lastError,
      );
      if (!this.mainModuleCaller) {
        return;
      }
      if (this.api === "GetDC" && resultValue !== "0x0") {
        screenDcTargetHandle = resultValue;
      }
      screenDcMainModuleCalls.push({
        callId: this.traceFrame.callId,
        api: this.api,
        returnAddress: this.returnAddress.toString(),
        callerRva: this.traceFrame.caller.rva,
        arguments: this.arguments,
        result: resultValue,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        logFont: this.logFont,
        fontCallbackInstructions: this.fontCallbackInstructions,
        fontCallbacks: this.fontCallbacks,
        immediateConsumer: this.immediateConsumer,
      });
      if (this.api === "ReleaseDC" && resultValue === 1) {
        scheduleScreenDcOracle();
        setImmediate(maybeCompleteScreenDcObservation);
      }
    },
  });
}

function scheduleScreenDcOracle() {
  if (screenDcOracleScheduled) {
    return;
  }
  screenDcOracleScheduled = true;
  setImmediate(() => {
    try {
      const user32 = Process.getModuleByName("user32.dll");
      const getDcAddress = findExport(user32, "GetDC");
      const releaseDcAddress = findExport(user32, "ReleaseDC");
      if (getDcAddress === null || releaseDcAddress === null) {
        throw new Error("screen DC oracle export is unavailable");
      }
      const getDc = new SystemFunction(
        getDcAddress,
        "pointer",
        ["pointer"],
        "stdcall",
      );
      const releaseDc = new SystemFunction(
        releaseDcAddress,
        "int32",
        ["pointer", "pointer"],
        "stdcall",
      );
      const gdi32 = Process.getModuleByName("gdi32.dll");
      const enumFontFamiliesExAAddress = findExport(
        gdi32,
        "EnumFontFamiliesExA",
      );
      if (enumFontFamiliesExAAddress === null) {
        throw new Error("EnumFontFamiliesExA oracle export is unavailable");
      }
      const enumFontFamiliesExA = new SystemFunction(
        enumFontFamiliesExAAddress,
        "int32",
        ["pointer", "pointer", "pointer", "pointer", "uint32"],
        "stdcall",
      );
      const invoke = (id, api, sentinel, callback) => {
        screenDcOracleCall = {
          api,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = callback();
          const systemResult =
            api === "GetDC" ? outcome.value.toString() : Number(outcome.value);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            screenDcOracleCall.hookResult === null ||
            screenDcOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              `${api} screen oracle did not pass through the hook`,
            );
          }
          return {
            id,
            api,
            sentinel,
            systemResult,
            systemLastError,
            hookResult: screenDcOracleCall.hookResult,
            hookLastErrorAfter: screenDcOracleCall.hookLastErrorAfter,
            hookResultMatches: screenDcOracleCall.hookResult === systemResult,
            hookAndSystemLastErrorMatch:
              screenDcOracleCall.hookLastErrorAfter === systemLastError,
          };
        } finally {
          screenDcOracleCall = null;
        }
      };

      const calls = [];
      const first = invoke("screen-get-dc-first", "GetDC", 0x11112222, () =>
        getDc(ptr(0)),
      );
      calls.push(first);
      const second = invoke("screen-get-dc-second", "GetDC", 0x22223333, () =>
        getDc(ptr(0)),
      );
      calls.push(second);
      if (first.systemResult === "0x0" || second.systemResult === "0x0") {
        throw new Error("screen GetDC oracle returned NULL");
      }
      calls.push(
        invoke("screen-release-first", "ReleaseDC", 0x33334444, () =>
          releaseDc(ptr(0), ptr(first.systemResult)),
        ),
      );
      calls.push(
        invoke("screen-release-repeat", "ReleaseDC", 0x44445555, () =>
          releaseDc(ptr(0), ptr(first.systemResult)),
        ),
      );
      calls.push(
        invoke("screen-release-second", "ReleaseDC", 0x55556666, () =>
          releaseDc(ptr(0), ptr(second.systemResult)),
        ),
      );

      const fontInputStorage = Memory.alloc(68);
      fontInputStorage.writeByteArray(Array(68).fill(0xa5));
      const fontInput = fontInputStorage.add(4);
      fontInput.writeByteArray(Array(60).fill(0));
      fontInput.add(23).writeU8(128);
      fontInput
        .add(28)
        .writeByteArray([
          0x82, 0x6c, 0x82, 0x72, 0x20, 0x83, 0x53, 0x83, 0x56, 0x83, 0x62,
          0x83, 0x4e, 0,
        ]);
      const fontDcOutcome = getDc(ptr(0));
      const fontDc = fontDcOutcome.value;
      if (fontDc.isNull()) {
        throw new Error("font enumeration screen GetDC oracle returned NULL");
      }
      const invokeFont = (id, callbackResult, sentinel, callbackData) => {
        const callbacks = [];
        const callback = new NativeCallback(
          (enumLogFont, textMetric, fontType, data) => {
            callbacks.push({
              threadId: Process.getCurrentThreadId(),
              enumLogFont: snapshotEnumLogFontExA(enumLogFont),
              textMetric: snapshotNewTextMetricExA(textMetric),
              fontType: Number(fontType) >>> 0,
              callbackData: data.toString(),
              result: callbackResult,
            });
            return callbackResult;
          },
          "int32",
          ["pointer", "pointer", "uint32", "pointer"],
          "stdcall",
        );
        screenDcFontOracleCallbacks.push(callback);
        const inputStorageBefore = snapshotBoundedRawBytes(
          fontInputStorage,
          68,
          68,
        );
        screenDcOracleCall = {
          api: "EnumFontFamiliesExA",
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = enumFontFamiliesExA(
            fontDc,
            fontInput,
            callback,
            callbackData,
            0,
          );
          const systemResult = Number(outcome.value);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            screenDcOracleCall.hookResult === null ||
            screenDcOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              `EnumFontFamiliesExA ${id} oracle did not pass through the hook`,
            );
          }
          const inputStorageAfter = snapshotBoundedRawBytes(
            fontInputStorage,
            68,
            68,
          );
          const afterBytes = inputStorageAfter.bytes ?? [];
          return {
            id,
            callbackResult,
            sentinel,
            hdc: fontDc.toString(),
            input: snapshotBoundedRawBytes(fontInput, 60, 60),
            callbackData: callbackData.toString(),
            flags: 0,
            hookResult: screenDcOracleCall.hookResult,
            hookLastErrorAfter: screenDcOracleCall.hookLastErrorAfter,
            systemResult,
            systemLastError,
            callbacks,
            inputStorageBefore,
            inputStorageAfter,
            hookResultMatches: screenDcOracleCall.hookResult === systemResult,
            hookPreservesLastError:
              screenDcOracleCall.hookLastErrorAfter === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
            inputUnchanged:
              JSON.stringify(inputStorageBefore.bytes) ===
              JSON.stringify(inputStorageAfter.bytes),
            prefixGuardUnchanged:
              afterBytes.length === 68 &&
              afterBytes.slice(0, 4).every((value) => value === 0xa5),
            suffixGuardUnchanged:
              afterBytes.length === 68 &&
              afterBytes.slice(64).every((value) => value === 0xa5),
            allCallbackDataRoundTrips: callbacks.every(
              (invocation) =>
                invocation.callbackData === callbackData.toString(),
            ),
          };
        } finally {
          screenDcOracleCall = null;
        }
      };
      let fontEnumerationCalls;
      try {
        fontEnumerationCalls = [
          invokeFont("stop", 0, 0x66667777, ptr("0x12345678")),
          invokeFont("continue", 1, 0x77778888, ptr("0x87654321")),
        ];
      } finally {
        releaseDc(ptr(0), fontDc);
      }

      screenDcOracleEvidence = {
        profile: "x86-screen-dc-and-font-enumeration-v2",
        calls,
        relations: {
          concurrentGetDcSameIdentity:
            first.systemResult === second.systemResult,
        },
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
        fontEnumeration: {
          profile: "x86-cp932-ms-gothic-callback-enumeration-v1",
          calls: fontEnumerationCalls,
          stopCallbackCount: fontEnumerationCalls[0].callbacks.length,
          continueCallbackCount: fontEnumerationCalls[1].callbacks.length,
          stopHaltsAfterFirstCallback:
            fontEnumerationCalls[0].callbacks.length === 1,
          expectedResultsObserved:
            fontEnumerationCalls[0].systemResult === 0 &&
            fontEnumerationCalls[1].systemResult === 1 &&
            fontEnumerationCalls.every((call) => call.hookResultMatches),
          allInputsAndGuardsUnchanged: fontEnumerationCalls.every(
            (call) =>
              call.inputUnchanged &&
              call.prefixGuardUnchanged &&
              call.suffixGuardUnchanged,
          ),
          allCallbackDataRoundTrips: fontEnumerationCalls.every(
            (call) => call.allCallbackDataRoundTrips,
          ),
          allPreserveLastError: fontEnumerationCalls.every(
            (call) =>
              call.hookPreservesLastError && call.systemPreservesLastError,
          ),
          firstEnumLogFontRawStable:
            fontEnumerationCalls.every((call) => call.callbacks.length > 0) &&
            JSON.stringify(
              fontEnumerationCalls[0].callbacks[0].enumLogFont.raw.bytes,
            ) ===
              JSON.stringify(
                fontEnumerationCalls[1].callbacks[0].enumLogFont.raw.bytes,
              ),
          firstEnumLogFontSemanticsStable:
            fontEnumerationCalls.every((call) => call.callbacks.length > 0) &&
            JSON.stringify(
              fontEnumerationCalls[0].callbacks[0].enumLogFont.semantic,
            ) ===
              JSON.stringify(
                fontEnumerationCalls[1].callbacks[0].enumLogFont.semantic,
              ),
          firstTextMetricSemanticsStable:
            fontEnumerationCalls.every((call) => call.callbacks.length > 0) &&
            JSON.stringify(
              fontEnumerationCalls[0].callbacks[0].textMetric.fields,
            ) ===
              JSON.stringify(
                fontEnumerationCalls[1].callbacks[0].textMetric.fields,
              ),
        },
      };
      screenDcOracleComplete = true;
      emit("screen-dc-oracle", screenDcOracleEvidence);
      maybeCompleteScreenDcObservation();
    } catch (error) {
      emit("screen-dc-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteScreenDcObservation() {
  if (
    screenDcCompletionRecorded ||
    !screenDcOracleComplete ||
    !screenDcMainModuleCalls.some((call) => call.api === "GetDC") ||
    !screenDcMainModuleCalls.some(
      (call) => call.api === "EnumFontFamiliesExA",
    ) ||
    !screenDcMainModuleCalls.some((call) => call.api === "ReleaseDC")
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteScreenDcObservation, 10);
    return;
  }
  screenDcCompletionRecorded = true;
  emit("screen-dc-complete", {
    profile: "x86-bgi-screen-dc-font-enumeration-callback-v2",
    mainModuleCalls: screenDcMainModuleCalls,
    oracle: screenDcOracleEvidence,
  });
  if (stopOnEvent === "screen-dc-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function snapshotCreateFontAArguments(args) {
  return {
    height: signed32(args[0]),
    width: signed32(args[1]),
    escapement: signed32(args[2]),
    orientation: signed32(args[3]),
    weight: signed32(args[4]),
    italic: args[5].toUInt32(),
    underline: args[6].toUInt32(),
    strikeOut: args[7].toUInt32(),
    characterSet: args[8].toUInt32(),
    outputPrecision: args[9].toUInt32(),
    clipPrecision: args[10].toUInt32(),
    quality: args[11].toUInt32(),
    pitchAndFamily: args[12].toUInt32(),
    face: snapshotBoundedAnsiBytes(args[13], 32),
  };
}

function snapshotDibBitmapInfo(pointer) {
  const maximumBitmapInfoBytes = 4096;
  try {
    const headerSize = pointer.readU32();
    if (headerSize < 40 || headerSize > maximumBitmapInfoBytes) {
      return {
        pointer: pointer.toString(),
        headerSize,
        maximumBitmapInfoBytes,
        error: "unsupported BITMAPINFO header size",
      };
    }
    const width = pointer.add(4).readS32();
    const height = pointer.add(8).readS32();
    const planes = pointer.add(12).readU16();
    const bitCount = pointer.add(14).readU16();
    const compression = pointer.add(16).readU32();
    const sizeImage = pointer.add(20).readU32();
    const xPixelsPerMeter = pointer.add(24).readS32();
    const yPixelsPerMeter = pointer.add(28).readS32();
    const colorsUsed = pointer.add(32).readU32();
    const colorsImportant = pointer.add(36).readU32();
    const defaultColorCount = bitCount <= 8 ? 2 ** bitCount : 0;
    const colorCount = colorsUsed !== 0 ? colorsUsed : defaultColorCount;
    const bitmapInfoBytes = headerSize + colorCount * 4;
    const absoluteWidth = Math.abs(width);
    const absoluteHeight = Math.abs(height);
    const strideBytes = Math.floor((absoluteWidth * bitCount + 31) / 32) * 4;
    const pixelBytes = strideBytes * absoluteHeight;
    return {
      pointer: pointer.toString(),
      headerSize,
      width,
      height,
      topDown: height < 0,
      planes,
      bitCount,
      compression,
      sizeImage,
      xPixelsPerMeter,
      yPixelsPerMeter,
      colorsUsed,
      colorsImportant,
      colorCount,
      bitmapInfoBytes,
      strideBytes,
      pixelBytes,
      raw: snapshotBoundedRawBytes(
        pointer,
        bitmapInfoBytes,
        maximumBitmapInfoBytes,
      ),
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      maximumBitmapInfoBytes,
      error: error.message,
    };
  }
}

function snapshotDibPixelPlane(dibSection) {
  const maximumPixelBytes = 1024 * 1024;
  const recordedPixelBytes = 64 * 1024;
  const byteLength = dibSection.bitmapInfo.pixelBytes;
  if (
    !Number.isInteger(byteLength) ||
    byteLength < 0 ||
    byteLength > maximumPixelBytes
  ) {
    return {
      evidence: {
        bitsPointer: dibSection.bits.toString(),
        byteLength,
        maximumPixelBytes,
        error: "DIB pixel plane is outside the bounded observation",
      },
      bytes: null,
    };
  }
  try {
    const bytes = [
      ...new Uint8Array(dibSection.bits.readByteArray(byteLength)),
    ];
    const histogram = Array(256).fill(0);
    let hash = 0x811c9dc5;
    let minimumByte = 255;
    let maximumByte = 0;
    for (const byte of bytes) {
      histogram[byte] += 1;
      hash = fnv1aByte(hash, byte);
      minimumByte = Math.min(minimumByte, byte);
      maximumByte = Math.max(maximumByte, byte);
    }
    return {
      evidence: {
        bitsPointer: dibSection.bits.toString(),
        byteLength,
        maximumPixelBytes,
        fnv1a32: `0x${hash.toString(16).padStart(8, "0")}`,
        minimumByte: bytes.length === 0 ? null : minimumByte,
        maximumByte: bytes.length === 0 ? null : maximumByte,
        uniqueByteCount: histogram.filter((count) => count !== 0).length,
        histogram,
        bytes: byteLength <= recordedPixelBytes ? bytes : null,
        bytesTruncated: byteLength > recordedPixelBytes,
      },
      bytes,
    };
  } catch (error) {
    return {
      evidence: {
        bitsPointer: dibSection.bits.toString(),
        byteLength,
        maximumPixelBytes,
        error: error.message,
      },
      bytes: null,
    };
  }
}

function diffDibPixelPlanes(dibSection, before, after) {
  if (before === null || after === null || before.length !== after.length) {
    return {
      error: "DIB pixel planes are unavailable or have different lengths",
    };
  }
  const maximumRecordedChanges = 64 * 1024;
  const strideBytes = dibSection.bitmapInfo.strideBytes;
  const absoluteHeight = Math.abs(dibSection.bitmapInfo.height);
  let changedByteCount = 0;
  let minimumXByte = null;
  let maximumXByte = null;
  let minimumStorageRow = null;
  let maximumStorageRow = null;
  let minimumLogicalY = null;
  let maximumLogicalY = null;
  const changes = [];
  for (let offset = 0; offset < before.length; offset += 1) {
    if (before[offset] === after[offset]) {
      continue;
    }
    changedByteCount += 1;
    const storageRow = Math.floor(offset / strideBytes);
    const xByte = offset % strideBytes;
    const logicalY = dibSection.bitmapInfo.topDown
      ? storageRow
      : absoluteHeight - storageRow - 1;
    minimumXByte =
      minimumXByte === null ? xByte : Math.min(minimumXByte, xByte);
    maximumXByte =
      maximumXByte === null ? xByte : Math.max(maximumXByte, xByte);
    minimumStorageRow =
      minimumStorageRow === null
        ? storageRow
        : Math.min(minimumStorageRow, storageRow);
    maximumStorageRow =
      maximumStorageRow === null
        ? storageRow
        : Math.max(maximumStorageRow, storageRow);
    minimumLogicalY =
      minimumLogicalY === null ? logicalY : Math.min(minimumLogicalY, logicalY);
    maximumLogicalY =
      maximumLogicalY === null ? logicalY : Math.max(maximumLogicalY, logicalY);
    if (changes.length < maximumRecordedChanges) {
      changes.push({
        offset,
        xByte,
        storageRow,
        logicalY,
        before: before[offset],
        after: after[offset],
      });
    }
  }
  return {
    changedByteCount,
    byteBounds:
      changedByteCount === 0
        ? null
        : {
            minimumXByte,
            maximumXByte,
            minimumStorageRow,
            maximumStorageRow,
            minimumLogicalY,
            maximumLogicalY,
          },
    changes,
    changesTruncated: changedByteCount > maximumRecordedChanges,
  };
}

function fontObjectResult(name, result) {
  return name === "CreateFontA" || name === "SelectObject"
    ? result.toString()
    : signed32(result);
}

function scheduleFontObjectCompletion() {
  const generation = ++fontObjectActivityGeneration;
  setTimeout(() => {
    if (generation !== fontObjectActivityGeneration) {
      return;
    }
    maybeCompleteFontObjectObservation(generation);
  }, 250);
}

function maybeCompleteFontObjectObservation(generation) {
  if (
    fontObjectCompletionRecorded ||
    generation !== fontObjectActivityGeneration ||
    !fontObjectOracleComplete ||
    !fontObjectMainModuleCalls.some((call) => call.api === "CreateFontA")
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(() => maybeCompleteFontObjectObservation(generation), 10);
    return;
  }
  fontObjectCompletionRecorded = true;
  emit("font-object-complete", {
    profile: "x86-bgi-gdi-font-object-lifecycle-v5",
    mainModuleCalls: fontObjectMainModuleCalls,
    oracle: fontObjectOracleEvidence,
  });
  if (stopOnEvent === "font-object-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function attachFontObjectHook(module, name) {
  const address = findExport(module, name);
  if (address === null) {
    throw new Error(`${name} export is unavailable`);
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        fontObjectOracleCall !== null &&
        fontObjectOracleCall.api === name;
      this.handle =
        name === "GetObjectA" || name === "DeleteObject" ? args[0] : null;
      this.deviceContext =
        name === "SelectObject" ||
        name === "GetTextMetricsA" ||
        name === "GetTextExtentPoint32A"
          ? args[0]
          : null;
      this.object = name === "SelectObject" ? args[1] : null;
      const handleKey = this.handle?.toString() ?? null;
      const dcKey = this.deviceContext?.toString() ?? null;
      const objectKey = this.object?.toString() ?? null;
      const mainRelated =
        isMainModuleCaller &&
        (name === "CreateFontA" ||
          fontObjectPrimaryStarted ||
          (handleKey !== null && fontObjectPrimaryHandles.has(handleKey)) ||
          (name === "SelectObject" &&
            (fontObjectPrimaryHandles.has(objectKey) ||
              fontObjectSelectedDcHandles.has(dcKey))));
      if (!mainRelated && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }

      this.api = name;
      this.mainModuleCaller = mainRelated;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = fontObjectOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = mainRelated
        ? snapshotInstructions(this.returnAddress, 96)
        : null;
      this.createFont =
        name === "CreateFontA" ? snapshotCreateFontAArguments(args) : null;
      this.outputPointer =
        name === "GetObjectA" ||
        name === "GetTextMetricsA" ||
        name === "GetTextExtentPoint32A"
          ? name === "GetObjectA"
            ? args[2]
            : name === "GetTextMetricsA"
              ? args[1]
              : args[3]
          : null;
      this.outputBytes =
        name === "GetObjectA"
          ? signed32(args[1])
          : name === "GetTextMetricsA"
            ? 56
            : name === "GetTextExtentPoint32A"
              ? 8
              : 0;
      this.text =
        name === "GetTextExtentPoint32A"
          ? snapshotBoundedRawBytes(
              args[1],
              Math.min(args[2].toUInt32(), 256),
              256,
            )
          : null;
      this.arguments =
        name === "CreateFontA"
          ? [
              this.createFont.height,
              this.createFont.width,
              this.createFont.escapement,
              this.createFont.orientation,
              this.createFont.weight,
              this.createFont.italic,
              this.createFont.underline,
              this.createFont.strikeOut,
              this.createFont.characterSet,
              this.createFont.outputPrecision,
              this.createFont.clipPrecision,
              this.createFont.quality,
              this.createFont.pitchAndFamily,
              this.createFont.face.pointer,
            ]
          : name === "SelectObject"
            ? [this.deviceContext.toString(), this.object.toString()]
            : name === "GetObjectA"
              ? [
                  this.handle.toString(),
                  this.outputBytes,
                  this.outputPointer.toString(),
                ]
              : name === "DeleteObject"
                ? [this.handle.toString()]
                : name === "GetTextMetricsA"
                  ? [
                      this.deviceContext.toString(),
                      this.outputPointer.toString(),
                    ]
                  : [
                      this.deviceContext.toString(),
                      args[1].toString(),
                      args[2].toUInt32(),
                      this.outputPointer.toString(),
                    ];
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: this.arguments,
          lastErrorBefore: this.lastErrorBefore,
          fontObject: {
            createFont: this.createFont,
            text: this.text,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const resultValue = fontObjectResult(this.api, result);
      const lastError = Number(this.lastError) >>> 0;
      const output =
        this.outputPointer !== null &&
        this.outputBytes >= 0 &&
        this.outputBytes <= 256
          ? snapshotBoundedRawBytes(this.outputPointer, this.outputBytes, 256)
          : null;
      if (this.oracleCaller) {
        fontObjectOracleCall.hookResult = resultValue;
        fontObjectOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: this.api,
          result: result.toString(),
          fontObject: {
            result: resultValue,
            output,
          },
        },
        this.lastError,
      );
      if (!this.mainModuleCaller) {
        return;
      }

      if (this.api === "CreateFontA" && resultValue !== "0x0") {
        fontObjectPrimaryStarted = true;
        fontObjectPrimaryHandles.add(resultValue);
      } else if (this.api === "SelectObject" && resultValue !== "0x0") {
        const dcKey = this.deviceContext.toString();
        const objectKey = this.object.toString();
        if (fontObjectPrimaryHandles.has(objectKey)) {
          fontObjectSelectedDcHandles.set(dcKey, objectKey);
        } else if (fontObjectPrimaryHandles.has(resultValue)) {
          fontObjectSelectedDcHandles.delete(dcKey);
        }
        if (fontObjectDibSectionsByHandle.has(objectKey)) {
          fontObjectSelectedBitmapByDc.set(dcKey, objectKey);
        } else if (fontObjectDibSectionsByHandle.has(resultValue)) {
          fontObjectSelectedBitmapByDc.delete(dcKey);
        }
      } else if (this.api === "DeleteObject" && resultValue !== 0) {
        const handleKey = this.handle.toString();
        fontObjectPrimaryHandles.delete(handleKey);
        fontObjectDibSectionsByHandle.delete(handleKey);
        for (const [dcKey, fontKey] of fontObjectSelectedDcHandles) {
          if (fontKey === handleKey) {
            fontObjectSelectedDcHandles.delete(dcKey);
          }
        }
        for (const [dcKey, bitmapKey] of fontObjectSelectedBitmapByDc) {
          if (bitmapKey === handleKey) {
            fontObjectSelectedBitmapByDc.delete(dcKey);
          }
        }
      }
      fontObjectMainModuleCalls.push({
        callId: this.traceFrame.callId,
        api: this.api,
        returnAddress: this.returnAddress.toString(),
        callerRva: this.traceFrame.caller.rva,
        arguments: this.arguments,
        createFont: this.createFont,
        text: this.text,
        result: resultValue,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        output,
        immediateConsumer: this.immediateConsumer,
      });
      if (this.api === "CreateFontA") {
        scheduleFontObjectOracle(module);
      }
      scheduleFontObjectCompletion();
    },
  });
}

function attachFontPipelineHook(module, name) {
  const address = findExport(module, name);
  if (address === null) {
    throw new Error(`${name} export is unavailable`);
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);
  const argumentCounts = {
    CreateCompatibleDC: 1,
    CreateDIBSection: 6,
    DeleteDC: 1,
    GetOutlineTextMetricsA: 3,
    GetGlyphOutlineA: 7,
    TextOutA: 5,
    TextOutW: 5,
    SetBkMode: 2,
    SetTextColor: 2,
    SelectPalette: 3,
    CreatePalette: 1,
  };

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase() &&
        fontObjectPrimaryStarted;
      const isOracleCaller =
        callerModule === "frida-agent.dll" &&
        fontObjectOracleCall !== null &&
        fontObjectOracleCall.api === name;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.api = name;
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = fontObjectOracleCall.sentinel;
      }
      this.argumentPointers = [];
      for (let index = 0; index < argumentCounts[name]; index += 1) {
        this.argumentPointers.push(args[index]);
      }
      this.arguments = this.argumentPointers.map((argument) =>
        argument.toString(),
      );
      this.bitmapInfo =
        name === "CreateDIBSection" ? snapshotDibBitmapInfo(args[1]) : null;
      this.deviceContextKey =
        name === "TextOutA" || name === "TextOutW" ? args[0].toString() : null;
      this.dibSectionKey =
        this.deviceContextKey === null
          ? null
          : (fontObjectSelectedBitmapByDc.get(this.deviceContextKey) ?? null);
      this.dibSection =
        this.dibSectionKey === null
          ? null
          : (fontObjectDibSectionsByHandle.get(this.dibSectionKey) ?? null);
      this.pixelPlaneBefore =
        this.dibSection === null
          ? null
          : snapshotDibPixelPlane(this.dibSection);
      this.text =
        name === "TextOutA"
          ? snapshotBoundedRawBytes(
              args[3],
              Math.min(args[4].toUInt32(), 512),
              512,
            )
          : name === "TextOutW"
            ? snapshotBoundedRawBytes(
                args[3],
                Math.min(args[4].toUInt32() * 2, 1024),
                1024,
              )
            : null;
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = isMainModuleCaller
        ? snapshotInstructions(this.returnAddress, 96)
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: this.arguments,
          lastErrorBefore: this.lastErrorBefore,
          fontPipeline: {
            bitmapInfo: this.bitmapInfo,
            text: this.text,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const resultValue =
        this.api === "CreateCompatibleDC" || this.api === "CreateDIBSection"
          ? result.toString()
          : signed32(result);
      const lastError = Number(this.lastError) >>> 0;
      let output = null;
      if (this.api === "CreateDIBSection") {
        try {
          const bits = this.argumentPointers[3].readPointer();
          output = {
            bitsPointer: bits.toString(),
          };
          if (
            resultValue !== "0x0" &&
            !bits.isNull() &&
            this.bitmapInfo.error === undefined
          ) {
            const dibSection = {
              handle: result,
              bits,
              bitmapInfo: this.bitmapInfo,
            };
            fontObjectDibSectionsByHandle.set(resultValue, dibSection);
            output.pixelPlane = snapshotDibPixelPlane(dibSection).evidence;
          }
        } catch (error) {
          output = { error: error.message };
        }
      } else if (this.api === "GetOutlineTextMetricsA") {
        const requestedBytes = Math.min(
          this.argumentPointers[1].toUInt32(),
          4096,
        );
        output = snapshotBoundedRawBytes(
          this.argumentPointers[2],
          requestedBytes,
          4096,
        );
      } else if (this.api === "GetGlyphOutlineA") {
        output = {
          glyphMetrics: snapshotBoundedRawBytes(
            this.argumentPointers[3],
            20,
            20,
          ),
          buffer: snapshotBoundedRawBytes(
            this.argumentPointers[5],
            Math.min(this.argumentPointers[4].toUInt32(), 4096),
            4096,
          ),
          transform: snapshotBoundedRawBytes(this.argumentPointers[6], 16, 16),
        };
      } else if (this.api === "TextOutA" || this.api === "TextOutW") {
        const pixelPlaneAfter =
          this.dibSection === null
            ? null
            : snapshotDibPixelPlane(this.dibSection);
        output = {
          dibSectionHandle: this.dibSectionKey,
          pixelPlaneBefore: this.pixelPlaneBefore?.evidence ?? null,
          pixelPlaneAfter: pixelPlaneAfter?.evidence ?? null,
          pixelDiff:
            this.dibSection === null
              ? null
              : diffDibPixelPlanes(
                  this.dibSection,
                  this.pixelPlaneBefore?.bytes ?? null,
                  pixelPlaneAfter?.bytes ?? null,
                ),
        };
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: this.api,
          result: result.toString(),
          fontPipeline: { result: resultValue, output },
        },
        this.lastError,
      );
      if (this.oracleCaller) {
        fontObjectOracleCall.hookResult = resultValue;
        fontObjectOracleCall.hookLastErrorAfter = lastError;
      }
      if (!this.mainModuleCaller) {
        return;
      }
      fontObjectMainModuleCalls.push({
        callId: this.traceFrame.callId,
        api: this.api,
        returnAddress: this.returnAddress.toString(),
        callerRva: this.traceFrame.caller.rva,
        arguments: this.arguments,
        bitmapInfo: this.bitmapInfo,
        text: this.text,
        result: resultValue,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        output,
        immediateConsumer: this.immediateConsumer,
      });
      scheduleFontObjectCompletion();
    },
  });
}

function scheduleGdiplusStartupCompletion() {
  const generation = ++gdiplusStartupActivityGeneration;
  setTimeout(() => {
    if (generation !== gdiplusStartupActivityGeneration) {
      return;
    }
    maybeCompleteGdiplusStartupObservation(generation);
  }, 250);
}

function maybeCompleteGdiplusStartupObservation(generation) {
  if (
    gdiplusStartupCompletionRecorded ||
    generation !== gdiplusStartupActivityGeneration ||
    !gdiplusMainModuleCalls.some((call) => call.api === "GdiplusStartup")
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(() => maybeCompleteGdiplusStartupObservation(generation), 10);
    return;
  }
  gdiplusStartupCompletionRecorded = true;
  emit("gdiplus-startup-complete", {
    profile: "x86-bgi-gdiplus-startup-v1",
    mainModuleCalls: gdiplusMainModuleCalls,
  });
  if (stopOnEvent === "gdiplus-startup-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function attachGdiplusHook(module, name, argumentCount) {
  const address = findExport(module, name);
  if (address === null) {
    throw new Error(`${name} export is unavailable`);
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const isMainModuleCaller =
        caller.module?.toLowerCase() === Process.mainModule.name.toLowerCase();
      if (!isMainModuleCaller) {
        this.traceFrame = null;
        return;
      }

      this.api = name;
      this.arguments = Array.from({ length: argumentCount }, (_, index) =>
        args[index].toString(),
      );
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.token = name === "GdiplusStartup" ? args[0] : null;
      this.input =
        name === "GdiplusStartup" ? snapshotGdiplusStartupInput(args[1]) : null;
      this.output =
        name === "GdiplusStartup"
          ? snapshotGdiplusStartupOutput(args[2])
          : null;
      this.tokenBefore =
        this.token === null ? null : snapshotPointerSlot(this.token);
      this.immediateConsumer = {
        apiContinuation: snapshotInstructions(this.returnAddress, 96),
        frameCallerContinuations: snapshotX86FrameCallerContinuations(
          this.context,
          8,
          24,
        ),
      };
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: this.arguments,
          lastErrorBefore: this.lastErrorBefore,
          gdiplus: {
            startupInput: this.input,
            startupOutput: this.output,
            tokenBefore: this.tokenBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const isVoid = this.api === "GdiplusShutdown" || this.api === "GdipFree";
      const resultValue = isVoid
        ? null
        : this.api === "GdipAlloc"
          ? result.toString()
          : signed32(result);
      const tokenAfter =
        this.token === null ? null : snapshotPointerSlot(this.token);
      const lastError = Number(this.lastError) >>> 0;
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: this.api,
          result: resultValue,
          gdiplus: {
            tokenAfter,
          },
        },
        this.lastError,
      );
      gdiplusMainModuleCalls.push({
        callId: this.traceFrame.callId,
        api: this.api,
        returnAddress: this.returnAddress.toString(),
        callerRva: this.traceFrame.caller.rva,
        arguments: this.arguments,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        result: resultValue,
        startupInput: this.input,
        startupOutput: this.output,
        tokenBefore: this.tokenBefore,
        tokenAfter,
        immediateConsumer: this.immediateConsumer,
      });
      scheduleGdiplusStartupCompletion();
    },
  });
}

function snapshotVirtualAllocation(pointer, requestedBytes) {
  if (pointer.isNull()) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      range: null,
    };
  }
  try {
    const first = Process.findRangeByAddress(pointer);
    const lastAddress = pointer.add(Math.max(requestedBytes - 1, 0));
    const last = Process.findRangeByAddress(lastAddress);
    let memoryBasicInformation = null;
    if (virtualQueryFunction !== null && Process.pointerSize === 4) {
      const output = Memory.alloc(28);
      const written = virtualQueryFunction(pointer, output, 28);
      memoryBasicInformation =
        written === 28
          ? {
              baseAddress: output.readPointer().toString(),
              allocationBase: output.add(4).readPointer().toString(),
              allocationProtect: output.add(8).readU32(),
              regionSize: output.add(12).readU32(),
              state: output.add(16).readU32(),
              protect: output.add(20).readU32(),
              type: output.add(24).readU32(),
            }
          : { error: `VirtualQuery returned ${written}` };
    }
    return {
      pointer: pointer.toString(),
      requestedBytes,
      memoryBasicInformation,
      range:
        first === null
          ? null
          : {
              base: first.base.toString(),
              size: first.size,
              protection: first.protection,
              coversRequestedRange:
                last !== null &&
                last.base.equals(first.base) &&
                last.size === first.size,
            },
    };
  } catch (error) {
    return {
      pointer: pointer.toString(),
      requestedBytes,
      error: error.message,
    };
  }
}

function scheduleVirtualAllocCompletion() {
  const generation = ++virtualAllocActivityGeneration;
  setTimeout(() => {
    if (generation !== virtualAllocActivityGeneration) {
      return;
    }
    maybeCompleteVirtualAllocObservation(generation);
  }, 250);
}

function maybeCompleteVirtualAllocObservation(generation) {
  if (
    virtualAllocCompletionRecorded ||
    generation !== virtualAllocActivityGeneration ||
    !virtualAllocMainModuleCalls.some((call) => call.api === "VirtualAlloc")
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(() => maybeCompleteVirtualAllocObservation(generation), 10);
    return;
  }
  virtualAllocCompletionRecorded = true;
  emit("virtual-alloc-complete", {
    profile: "x86-bgi-virtual-alloc-reserve-top-down-v1",
    mainModuleCalls: virtualAllocMainModuleCalls,
  });
  if (stopOnEvent === "virtual-alloc-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function attachVirtualAllocHook(module, name) {
  if (virtualQueryFunction === null) {
    const virtualQueryAddress = findExport(module, "VirtualQuery");
    if (virtualQueryAddress === null) {
      throw new Error("VirtualQuery export is unavailable");
    }
    virtualQueryFunction = new NativeFunction(virtualQueryAddress, "uint32", [
      "pointer",
      "pointer",
      "uint32",
    ]);
  }
  const address = findExport(module, name);
  if (address === null) {
    throw new Error(`${name} export is unavailable`);
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const isMainModuleCaller =
        caller.module?.toLowerCase() === Process.mainModule.name.toLowerCase();
      if (!isMainModuleCaller) {
        this.traceFrame = null;
        return;
      }
      this.api = name;
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.arguments =
        name === "VirtualAlloc"
          ? {
              preferredAddress: args[0].toString(),
              byteLength: args[1].toUInt32(),
              allocationType: args[2].toUInt32(),
              protection: args[3].toUInt32(),
            }
          : {
              address: args[0].toString(),
              byteLength: args[1].toUInt32(),
              freeType: args[2].toUInt32(),
            };
      this.immediateConsumer = {
        apiContinuation: snapshotInstructions(this.returnAddress, 96),
        frameCallerContinuations: snapshotX86FrameCallerContinuations(
          this.context,
          8,
          24,
        ),
      };
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: this.arguments,
          lastErrorBefore: this.lastErrorBefore,
          immediateConsumer: this.immediateConsumer,
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultValue =
        this.api === "VirtualAlloc" ? result.toString() : result.toUInt32();
      const allocation =
        this.api === "VirtualAlloc"
          ? snapshotVirtualAllocation(result, this.arguments.byteLength)
          : null;
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: this.api,
          result: resultValue,
          allocation,
        },
        this.lastError,
      );
      virtualAllocMainModuleCalls.push({
        callId: this.traceFrame.callId,
        api: this.api,
        returnAddress: this.returnAddress.toString(),
        callerRva: this.traceFrame.caller.rva,
        arguments: this.arguments,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        result: resultValue,
        allocation,
        immediateConsumer: this.immediateConsumer,
      });
      scheduleVirtualAllocCompletion();
    },
  });
}

function requireImeWindowInspector() {
  if (imeWindowInspector !== null) {
    return imeWindowInspector;
  }
  const user32 = Process.getModuleByName("user32.dll");
  const isWindowAddress = findExport(user32, "IsWindow");
  const getWindowThreadProcessIdAddress = findExport(
    user32,
    "GetWindowThreadProcessId",
  );
  const getClassNameAAddress = findExport(user32, "GetClassNameA");
  if (
    isWindowAddress === null ||
    getWindowThreadProcessIdAddress === null ||
    getClassNameAAddress === null
  ) {
    throw new Error("User32 window identity exports are unavailable");
  }
  imeWindowInspector = {
    isWindow: new NativeFunction(
      isWindowAddress,
      "int32",
      ["pointer"],
      "stdcall",
    ),
    getWindowThreadProcessId: new NativeFunction(
      getWindowThreadProcessIdAddress,
      "uint32",
      ["pointer", "pointer"],
      "stdcall",
    ),
    getClassNameA: new NativeFunction(
      getClassNameAAddress,
      "int32",
      ["pointer", "pointer", "int32"],
      "stdcall",
    ),
  };
  return imeWindowInspector;
}

function snapshotImeWindowIdentity(window) {
  const handle = window.toString();
  if (window.isNull()) {
    return {
      handle,
      nonNull: false,
      isWindow: false,
      threadId: 0,
      processId: 0,
      className: null,
    };
  }
  try {
    const inspector = requireImeWindowInspector();
    const processId = Memory.alloc(4);
    processId.writeU32(0);
    const threadId = inspector.getWindowThreadProcessId(window, processId);
    const classNameBuffer = Memory.alloc(64);
    classNameBuffer.writeByteArray(Array(64).fill(0));
    const classNameLength = inspector.getClassNameA(
      window,
      classNameBuffer,
      64,
    );
    return {
      handle,
      nonNull: true,
      isWindow: inspector.isWindow(window) !== 0,
      threadId,
      processId: processId.readU32(),
      className:
        classNameLength > 0
          ? classNameBuffer.readAnsiString(classNameLength)
          : null,
    };
  } catch (error) {
    return {
      handle,
      nonNull: true,
      error: error.message,
    };
  }
}

function scheduleImeContextOracle(module, reachedWindow) {
  if (imeContextOracleScheduled) {
    return;
  }
  imeContextOracleScheduled = true;
  setImmediate(() => {
    try {
      const getDefaultImeWindowAddress = findExport(
        module,
        "ImmGetDefaultIMEWnd",
      );
      const kernel32 = Process.getModuleByName("kernel32.dll");
      const setLastErrorAddress = findExport(kernel32, "SetLastError");
      if (getDefaultImeWindowAddress === null || setLastErrorAddress === null) {
        throw new Error("IME query oracle exports are unavailable");
      }
      const getDefaultImeWindow = new SystemFunction(
        getDefaultImeWindowAddress,
        "pointer",
        ["pointer"],
        "stdcall",
      );
      const setLastError = new SystemFunction(
        setLastErrorAddress,
        "void",
        ["uint32"],
        "stdcall",
      );
      const invoke = (variant, window, sentinel) => {
        setLastError(sentinel);
        const outcome = getDefaultImeWindow(window);
        return {
          variant,
          window: window.toString(),
          sentinel,
          result: outcome.value.toString(),
          lastError: Number(outcome.lastError) >>> 0,
          resultIdentity: snapshotImeWindowIdentity(outcome.value),
        };
      };
      const reached = ptr(reachedWindow);
      imeContextOracleEvidence = {
        profile: "x86-imm-get-default-ime-window-query-v1",
        calls: [
          invoke("reached-repeat-first", reached, 0x5a5a_0101),
          invoke("reached-repeat-second", reached, 0x5a5a_0102),
          invoke("null-window", ptr(0), 0x5a5a_0103),
          invoke("invalid-window", ptr(0x1234), 0x5a5a_0104),
        ],
      };
      imeContextOracleComplete = true;
      emit("ime-context-oracle", imeContextOracleEvidence);
      maybeCompleteImeContextObservation();
    } catch (error) {
      emit("ime-context-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteImeContextObservation() {
  if (imeContextCompletionRecorded || !imeContextOracleComplete) {
    return;
  }
  const primaryGet = imeGetDefaultWindowCalls.find(
    (call) => call.callerRva === 0x0008_73da,
  );
  const detach = imeAssociateContextCalls.find(
    (call) => call.callerRva === 0x0008_73fe,
  );
  const restore = imeAssociateContextCalls.find(
    (call) => call.callerRva === 0x0008_7413,
  );
  const before = imeControlMessageCalls.find(
    (call) => call.callerRva === 0x0008_73ee,
  );
  const after = imeControlMessageCalls.find(
    (call) => call.callerRva === 0x0008_741f,
  );
  if (
    primaryGet === undefined ||
    detach === undefined ||
    before === undefined
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteImeContextObservation, 10);
    return;
  }
  if (imeContextDrainCheckScheduled) {
    return;
  }
  imeContextDrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    imeContextDrainCheckScheduled = false;
    if (imeContextCompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteImeContextObservation();
      return;
    }
    imeContextCompletionRecorded = true;
    emit("ime-context-complete", {
      profile: "x86-bgi-default-ime-window-context-detach-prefix-v1",
      quiescenceMilliseconds: 250,
      restoreReached: restore !== undefined,
      reopenStatusWindowReached: after !== undefined,
      getDefaultImeWindowCalls: imeGetDefaultWindowCalls.map((call) => ({
        ...call,
      })),
      associateContextCalls: imeAssociateContextCalls.map((call) => ({
        ...call,
      })),
      controlMessageCalls: imeControlMessageCalls.map((call) => ({ ...call })),
      oracle: imeContextOracleEvidence,
    });
    if (stopOnEvent === "ime-context-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function attachImmGetDefaultImeWindowHook(module) {
  const name = "ImmGetDefaultIMEWnd";
  const address = findExport(module, name);
  if (address === null) {
    throw new Error(`${name} export is unavailable`);
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);
  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      if (
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase()
      ) {
        this.traceFrame = null;
        return;
      }
      this.window = args[0];
      this.windowHandle = this.window.toString();
      this.callerRva = caller.rva;
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = {
        apiContinuation: snapshotInstructions(this.returnAddress, 96),
        frameCallerContinuations: snapshotX86FrameCallerContinuations(
          this.context,
          8,
          24,
        ),
      };
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.windowHandle],
          lastErrorBefore: this.lastErrorBefore,
          immediateConsumer: this.immediateConsumer,
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      const resultIdentity = snapshotImeWindowIdentity(result);
      const inputIdentity = snapshotImeWindowIdentity(this.window);
      this.lastError = lastError;
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          imeWindow: resultIdentity,
        },
        this.lastError,
      );
      imeGetDefaultWindowCalls.push({
        callId: this.traceFrame.callId,
        returnAddress: this.returnAddress.toString(),
        callerRva: this.callerRva,
        windowHandle: this.windowHandle,
        inputIdentity,
        result: result.toString(),
        resultIdentity,
        resultDistinctFromInput: !result.equals(this.window),
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        immediateConsumer: this.immediateConsumer,
      });
      scheduleImeContextOracle(module, this.windowHandle);
      setImmediate(maybeCompleteImeContextObservation);
    },
  });
}

function attachImmAssociateContextHook(module) {
  const name = "ImmAssociateContext";
  const address = findExport(module, name);
  if (address === null) {
    throw new Error(`${name} export is unavailable`);
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);
  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      if (
        caller.module?.toLowerCase() !== Process.mainModule.name.toLowerCase()
      ) {
        this.traceFrame = null;
        return;
      }
      this.windowHandle = args[0].toString();
      this.contextHandle = args[1].toString();
      this.callerRva = caller.rva;
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = {
        apiContinuation: snapshotInstructions(this.returnAddress, 64),
        frameCallerContinuations: snapshotX86FrameCallerContinuations(
          this.context,
          8,
          24,
        ),
      };
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.windowHandle, this.contextHandle],
          lastErrorBefore: this.lastErrorBefore,
          immediateConsumer: this.immediateConsumer,
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const lastError = Number(this.lastError) >>> 0;
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
        },
        this.lastError,
      );
      imeAssociateContextCalls.push({
        callId: this.traceFrame.callId,
        returnAddress: this.returnAddress.toString(),
        callerRva: this.callerRva,
        windowHandle: this.windowHandle,
        contextHandle: this.contextHandle,
        result: result.toString(),
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        immediateConsumer: this.immediateConsumer,
      });
      setImmediate(maybeCompleteImeContextObservation);
    },
  });
}

function scheduleFontObjectOracle(module) {
  if (fontObjectOracleScheduled) {
    return;
  }
  fontObjectOracleScheduled = true;
  setImmediate(() => {
    try {
      const createFontAAddress = findExport(module, "CreateFontA");
      const getObjectAAddress = findExport(module, "GetObjectA");
      const getOutlineTextMetricsAAddress = findExport(
        module,
        "GetOutlineTextMetricsA",
      );
      const textOutAAddress = findExport(module, "TextOutA");
      const selectObjectAddress = findExport(module, "SelectObject");
      const deleteObjectAddress = findExport(module, "DeleteObject");
      const createCompatibleDcAddress = findExport(
        module,
        "CreateCompatibleDC",
      );
      const createDibSectionAddress = findExport(module, "CreateDIBSection");
      const deleteDcAddress = findExport(module, "DeleteDC");
      const getCurrentObjectAddress = findExport(module, "GetCurrentObject");
      const getObjectTypeAddress = findExport(module, "GetObjectType");
      const user32 = Process.getModuleByName("user32.dll");
      const getDcAddress = findExport(user32, "GetDC");
      const releaseDcAddress = findExport(user32, "ReleaseDC");
      if (
        createFontAAddress === null ||
        getObjectAAddress === null ||
        getOutlineTextMetricsAAddress === null ||
        textOutAAddress === null ||
        selectObjectAddress === null ||
        deleteObjectAddress === null ||
        createCompatibleDcAddress === null ||
        createDibSectionAddress === null ||
        deleteDcAddress === null ||
        getCurrentObjectAddress === null ||
        getObjectTypeAddress === null ||
        getDcAddress === null ||
        releaseDcAddress === null
      ) {
        throw new Error("font object oracle export is unavailable");
      }
      const createFontA = new SystemFunction(
        createFontAAddress,
        "pointer",
        [
          "int32",
          "int32",
          "int32",
          "int32",
          "int32",
          "uint32",
          "uint32",
          "uint32",
          "uint32",
          "uint32",
          "uint32",
          "uint32",
          "uint32",
          "pointer",
        ],
        "stdcall",
      );
      const getObjectA = new SystemFunction(
        getObjectAAddress,
        "int32",
        ["pointer", "int32", "pointer"],
        "stdcall",
      );
      const getOutlineTextMetricsA = new SystemFunction(
        getOutlineTextMetricsAAddress,
        "uint32",
        ["pointer", "uint32", "pointer"],
        "stdcall",
      );
      const textOutA = new SystemFunction(
        textOutAAddress,
        "int32",
        ["pointer", "int32", "int32", "pointer", "int32"],
        "stdcall",
      );
      const selectObject = new SystemFunction(
        selectObjectAddress,
        "pointer",
        ["pointer", "pointer"],
        "stdcall",
      );
      const deleteObject = new SystemFunction(
        deleteObjectAddress,
        "int32",
        ["pointer"],
        "stdcall",
      );
      const createCompatibleDc = new SystemFunction(
        createCompatibleDcAddress,
        "pointer",
        ["pointer"],
        "stdcall",
      );
      const createDibSection = new SystemFunction(
        createDibSectionAddress,
        "pointer",
        ["pointer", "pointer", "uint32", "pointer", "pointer", "uint32"],
        "stdcall",
      );
      const deleteDc = new SystemFunction(
        deleteDcAddress,
        "int32",
        ["pointer"],
        "stdcall",
      );
      const getCurrentObject = new SystemFunction(
        getCurrentObjectAddress,
        "pointer",
        ["pointer", "uint32"],
        "stdcall",
      );
      const getObjectType = new SystemFunction(
        getObjectTypeAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const getDc = new SystemFunction(
        getDcAddress,
        "pointer",
        ["pointer"],
        "stdcall",
      );
      const releaseDc = new SystemFunction(
        releaseDcAddress,
        "int32",
        ["pointer", "pointer"],
        "stdcall",
      );
      const invoke = (id, api, sentinel, callback) => {
        fontObjectOracleCall = {
          api,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = callback();
          const systemResult =
            api === "CreateFontA" ||
            api === "CreateCompatibleDC" ||
            api === "CreateDIBSection" ||
            api === "SelectObject"
              ? outcome.value.toString()
              : Number(outcome.value);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            fontObjectOracleCall.hookResult === null ||
            fontObjectOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              `${api} ${id} oracle did not pass through the hook`,
            );
          }
          return {
            id,
            api,
            sentinel,
            systemResult,
            systemLastError,
            hookResult: fontObjectOracleCall.hookResult,
            hookLastErrorAfter: fontObjectOracleCall.hookLastErrorAfter,
            hookResultMatches: fontObjectOracleCall.hookResult === systemResult,
            hookAndSystemLastErrorMatch:
              fontObjectOracleCall.hookLastErrorAfter === systemLastError,
          };
        } finally {
          fontObjectOracleCall = null;
        }
      };

      const faceBytes = [
        0x82, 0x6c, 0x82, 0x72, 0x20, 0x83, 0x53, 0x83, 0x56, 0x83, 0x62, 0x83,
        0x4e, 0,
      ];
      const face = Memory.alloc(faceBytes.length);
      face.writeByteArray(faceBytes);
      const created = invoke("create", "CreateFontA", 0x11112222, () =>
        createFontA(64, 32, 0, 0, 100, 0, 0, 0, 128, 4, 0, 2, 0, face),
      );
      if (created.systemResult === "0x0") {
        throw new Error("font object oracle CreateFontA returned NULL");
      }
      const font = ptr(created.systemResult);
      const objectStorage = Memory.alloc(68);
      objectStorage.writeByteArray(Array(68).fill(0xa5));
      const object = objectStorage.add(4);
      const getObject = invoke("get-object", "GetObjectA", 0x22223333, () =>
        getObjectA(font, 60, object),
      );
      const objectStorageAfter = snapshotBoundedRawBytes(objectStorage, 68, 68);

      const dcOutcome = getDc(ptr(0));
      const dc = dcOutcome.value;
      if (dc.isNull()) {
        throw new Error("font object oracle GetDC returned NULL");
      }
      let selected;
      let restored;
      let release;
      let outlineTextMetricsEvidence;
      try {
        selected = invoke("select", "SelectObject", 0x33334444, () =>
          selectObject(dc, font),
        );
        if (
          selected.systemResult === "0x0" ||
          selected.systemResult === "0xffffffff"
        ) {
          throw new Error(
            "font object oracle SelectObject did not return a prior object",
          );
        }
        const outlineSize = invoke(
          "outline-size",
          "GetOutlineTextMetricsA",
          0x34343434,
          () => getOutlineTextMetricsA(dc, 0, ptr(0)),
        );
        if (outlineSize.systemResult <= 0 || outlineSize.systemResult > 4096) {
          throw new Error(
            "outline text metrics oracle returned an invalid byte length",
          );
        }
        const outlineByteLength = outlineSize.systemResult;
        const invokeOutlineBufferCase = (id, sentinel, requestedBytes) => {
          const storage = Memory.alloc(outlineByteLength + 24);
          storage.writeByteArray(Array(outlineByteLength + 24).fill(0xa5));
          return {
            call: invoke(id, "GetOutlineTextMetricsA", sentinel, () =>
              getOutlineTextMetricsA(dc, requestedBytes, storage.add(4)),
            ),
            requestedBytes,
            storage: snapshotBoundedRawBytes(
              storage,
              outlineByteLength + 24,
              outlineByteLength + 24,
            ),
          };
        };
        const exact = invokeOutlineBufferCase(
          "outline-exact",
          0x35353535,
          outlineByteLength,
        );
        const insufficient = [
          ["outline-one-byte", 0x36363636, 1],
          ["outline-two-bytes", 0x36464646, 2],
          ["outline-three-bytes", 0x36565656, 3],
          ["outline-four-bytes", 0x36666666, 4],
          ["outline-fixed-minus-one", 0x37373737, 211],
          ["outline-fixed", 0x38383838, 212],
          ["outline-fixed-plus-one", 0x39393939, 213],
          ["outline-short", 0x3a3a3a3a, outlineByteLength - 1],
        ].map(([id, sentinel, requestedBytes]) =>
          invokeOutlineBufferCase(id, sentinel, requestedBytes),
        );
        const oversized = invokeOutlineBufferCase(
          "outline-oversized",
          0x3b3b3b3b,
          outlineByteLength + 16,
        );
        outlineTextMetricsEvidence = {
          profile: "x86-get-outline-text-metrics-a-cp932-v1",
          requiredBytes: outlineByteLength,
          calls: [
            outlineSize,
            exact.call,
            ...insufficient.map((entry) => entry.call),
            oversized.call,
          ],
          exact,
          insufficient,
          oversized,
        };
        restored = invoke("restore", "SelectObject", 0x44445555, () =>
          selectObject(dc, ptr(selected.systemResult)),
        );
      } finally {
        release = releaseDc(ptr(0), dc);
      }
      const deleted = invoke("delete", "DeleteObject", 0x55556666, () =>
        deleteObject(font),
      );
      const repeatedDelete = invoke(
        "delete-repeat",
        "DeleteObject",
        0x66667777,
        () => deleteObject(font),
      );
      objectStorage.writeByteArray(Array(68).fill(0xa5));
      const staleGetObject = invoke(
        "get-object-stale",
        "GetObjectA",
        0x77778888,
        () => getObjectA(font, 60, object),
      );
      const staleObjectStorageAfter = snapshotBoundedRawBytes(
        objectStorage,
        68,
        68,
      );

      const firstCompatibleDc = invoke(
        "create-compatible-first",
        "CreateCompatibleDC",
        0x88889999,
        () => createCompatibleDc(ptr(0)),
      );
      const secondCompatibleDc = invoke(
        "create-compatible-second",
        "CreateCompatibleDC",
        0x9999aaaa,
        () => createCompatibleDc(ptr(0)),
      );
      const thirdCompatibleDc = invoke(
        "create-compatible-third",
        "CreateCompatibleDC",
        0xaaaabbbb,
        () => createCompatibleDc(ptr(0)),
      );
      if (
        firstCompatibleDc.systemResult === "0x0" ||
        secondCompatibleDc.systemResult === "0x0" ||
        thirdCompatibleDc.systemResult === "0x0"
      ) {
        throw new Error(
          "compatible DC oracle CreateCompatibleDC returned NULL",
        );
      }
      const firstMemoryDc = ptr(firstCompatibleDc.systemResult);
      const secondMemoryDc = ptr(secondCompatibleDc.systemResult);
      const thirdMemoryDc = ptr(thirdCompatibleDc.systemResult);
      const snapshotCompatibleDc = (deviceContext) => {
        const currentFont = getCurrentObject(deviceContext, 6);
        const currentBitmap = getCurrentObject(deviceContext, 7);
        return {
          objectType: Number(getObjectType(deviceContext).value),
          currentFont: currentFont.value.toString(),
          currentFontType: Number(getObjectType(currentFont.value).value),
          currentBitmap: currentBitmap.value.toString(),
          currentBitmapType: Number(getObjectType(currentBitmap.value).value),
        };
      };
      const firstInitialState = snapshotCompatibleDc(firstMemoryDc);
      const secondInitialState = snapshotCompatibleDc(secondMemoryDc);
      const wrongRelease = releaseDc(ptr(0), firstMemoryDc);
      const firstTypeAfterWrongRelease = Number(
        getObjectType(firstMemoryDc).value,
      );
      const firstDeleteDc = invoke(
        "delete-dc-first",
        "DeleteDC",
        0xbbbccccc,
        () => deleteDc(firstMemoryDc),
      );
      const outlineDeletedDc = invoke(
        "outline-deleted-dc",
        "GetOutlineTextMetricsA",
        0xbcbcbcbc,
        () => getOutlineTextMetricsA(firstMemoryDc, 0, ptr(0)),
      );
      const repeatedDeleteDc = invoke(
        "delete-dc-repeat",
        "DeleteDC",
        0xcccddddd,
        () => deleteDc(firstMemoryDc),
      );
      const firstTypeAfterDelete = Number(getObjectType(firstMemoryDc).value);
      const secondDeleteDc = invoke(
        "delete-dc-second",
        "DeleteDC",
        0xdddeeeee,
        () => deleteDc(secondMemoryDc),
      );
      const deleteObjectVariant = invoke(
        "delete-object-memory-dc",
        "DeleteObject",
        0xeeefffff,
        () => deleteObject(thirdMemoryDc),
      );
      const thirdTypeAfterDeleteObject = Number(
        getObjectType(thirdMemoryDc).value,
      );
      const compatibleDcCalls = [
        firstCompatibleDc,
        secondCompatibleDc,
        thirdCompatibleDc,
        firstDeleteDc,
        repeatedDeleteDc,
        secondDeleteDc,
        deleteObjectVariant,
      ];

      const dibMemoryDcCreated = invoke(
        "dib-create-compatible-dc",
        "CreateCompatibleDC",
        0xf0f1f2f3,
        () => createCompatibleDc(ptr(0)),
      );
      if (dibMemoryDcCreated.systemResult === "0x0") {
        throw new Error("DIB section oracle CreateCompatibleDC returned NULL");
      }
      const dibMemoryDc = ptr(dibMemoryDcCreated.systemResult);
      const dibInfoBytes = 40 + 256 * 4;
      const dibInfo = Memory.alloc(dibInfoBytes);
      dibInfo.writeU32(40);
      dibInfo.add(4).writeS32(128);
      dibInfo.add(8).writeS32(-96);
      dibInfo.add(12).writeU16(1);
      dibInfo.add(14).writeU16(8);
      dibInfo.add(16).writeU32(0);
      dibInfo.add(20).writeU32(0);
      dibInfo.add(24).writeS32(0);
      dibInfo.add(28).writeS32(0);
      dibInfo.add(32).writeU32(256);
      dibInfo.add(36).writeU32(0);
      const paletteBytes = [];
      for (let index = 0; index < 256; index += 1) {
        const gray = 255 - index;
        paletteBytes.push(gray, gray, gray, 0);
      }
      dibInfo.add(40).writeByteArray(paletteBytes);

      const snapshotAddressRange = (address, byteLength) => {
        const range = Process.findRangeByAddress(address);
        if (range === null) {
          return {
            address: address.toString(),
            mapped: false,
          };
        }
        return {
          address: address.toString(),
          mapped: true,
          rangeBase: range.base.toString(),
          rangeSize: range.size,
          protection: range.protection,
          bytes: snapshotBoundedRawBytes(address, Math.min(byteLength, 64), 64),
        };
      };
      const dibBitsOutput = Memory.alloc(Process.pointerSize);
      const dibBitsSentinel = ptr("0x13572468");
      dibBitsOutput.writePointer(dibBitsSentinel);
      const dibCreated = invoke(
        "dib-create",
        "CreateDIBSection",
        0x10111213,
        () =>
          createDibSection(dibMemoryDc, dibInfo, 0, dibBitsOutput, ptr(0), 0),
      );
      const dibBitsAfterCreate = dibBitsOutput.readPointer();
      if (dibCreated.systemResult === "0x0" || dibBitsAfterCreate.isNull()) {
        throw new Error("DIB section oracle CreateDIBSection returned NULL");
      }
      const dib = ptr(dibCreated.systemResult);
      const dibSnapshot = {
        handle: dib,
        bits: dibBitsAfterCreate,
        bitmapInfo: snapshotDibBitmapInfo(dibInfo),
      };
      const dibPixelPlaneAfterCreate =
        snapshotDibPixelPlane(dibSnapshot).evidence;
      const dibBackingAfterCreate = snapshotAddressRange(
        dibBitsAfterCreate,
        64,
      );

      const textFontCreated = invoke(
        "text-create-font",
        "CreateFontA",
        0x14151617,
        () => createFontA(64, 32, 0, 0, 100, 0, 0, 0, 128, 4, 0, 2, 0, face),
      );
      if (textFontCreated.systemResult === "0x0") {
        throw new Error("text raster oracle CreateFontA returned NULL");
      }
      const textFont = ptr(textFontCreated.systemResult);
      const textFontSelected = invoke(
        "text-select-font",
        "SelectObject",
        0x18191a1b,
        () => selectObject(dibMemoryDc, textFont),
      );
      if (
        textFontSelected.systemResult === "0x0" ||
        textFontSelected.systemResult === "0xffffffff"
      ) {
        throw new Error(
          "text raster oracle SelectObject did not return a prior font",
        );
      }

      const dibObjectStorage = Memory.alloc(32);
      dibObjectStorage.writeByteArray(Array(32).fill(0xa5));
      const dibObject = dibObjectStorage.add(4);
      const dibGetObject = invoke(
        "dib-get-object",
        "GetObjectA",
        0x20212223,
        () => getObjectA(dib, 24, dibObject),
      );
      const dibObjectStorageAfter = snapshotBoundedRawBytes(
        dibObjectStorage,
        32,
        32,
      );
      const dibSelected = invoke("dib-select", "SelectObject", 0x30313233, () =>
        selectObject(dibMemoryDc, dib),
      );
      if (
        dibSelected.systemResult === "0x0" ||
        dibSelected.systemResult === "0xffffffff"
      ) {
        throw new Error(
          "DIB section oracle SelectObject did not return a prior object",
        );
      }
      const textBytes = Memory.alloc(1);
      textBytes.writeU8(0x61);
      const textPlaneBefore = snapshotDibPixelPlane(dibSnapshot);
      const textFirst = invoke("text-out-first", "TextOutA", 0x34353637, () =>
        textOutA(dibMemoryDc, 0, 0, textBytes, 1),
      );
      const textPlaneAfterFirst = snapshotDibPixelPlane(dibSnapshot);
      const textFirstDiff = diffDibPixelPlanes(
        dibSnapshot,
        textPlaneBefore.bytes,
        textPlaneAfterFirst.bytes,
      );
      const textRepeat = invoke("text-out-repeat", "TextOutA", 0x38393a3b, () =>
        textOutA(dibMemoryDc, 0, 0, textBytes, 1),
      );
      const textPlaneAfterRepeat = snapshotDibPixelPlane(dibSnapshot);
      const textRepeatDiff = diffDibPixelPlanes(
        dibSnapshot,
        textPlaneAfterFirst.bytes,
        textPlaneAfterRepeat.bytes,
      );
      const textZeroLength = invoke(
        "text-out-zero-length",
        "TextOutA",
        0x3c3d3e3f,
        () => textOutA(dibMemoryDc, 0, 0, ptr(0), 0),
      );
      const textPlaneAfterZeroLength = snapshotDibPixelPlane(dibSnapshot);
      const textZeroLengthDiff = diffDibPixelPlanes(
        dibSnapshot,
        textPlaneAfterRepeat.bytes,
        textPlaneAfterZeroLength.bytes,
      );
      const dibDeleteWhileSelected = invoke(
        "dib-delete-selected",
        "DeleteObject",
        0x40414243,
        () => deleteObject(dib),
      );
      const dibTypeAfterSelectedDelete = Number(getObjectType(dib).value);
      const dibBackingAfterSelectedDelete = snapshotAddressRange(
        dibBitsAfterCreate,
        64,
      );
      const dibRestored = invoke(
        "dib-restore",
        "SelectObject",
        0x50515253,
        () => selectObject(dibMemoryDc, ptr(dibSelected.systemResult)),
      );
      const dibTypeAfterRestore = Number(getObjectType(dib).value);
      const dibBackingAfterRestore = snapshotAddressRange(
        dibBitsAfterCreate,
        64,
      );
      const textFontRestored = invoke(
        "text-restore-font",
        "SelectObject",
        0x54555657,
        () => selectObject(dibMemoryDc, ptr(textFontSelected.systemResult)),
      );
      const textFontDeleted = invoke(
        "text-delete-font",
        "DeleteObject",
        0x58595a5b,
        () => deleteObject(textFont),
      );
      const dibDeletedAfterRestore = invoke(
        "dib-delete-restored",
        "DeleteObject",
        0x60616263,
        () => deleteObject(dib),
      );
      const dibRepeatedDelete = invoke(
        "dib-delete-repeat",
        "DeleteObject",
        0x70717273,
        () => deleteObject(dib),
      );
      const dibTypeAfterDelete = Number(getObjectType(dib).value);
      const dibBackingAfterDelete = snapshotAddressRange(
        dibBitsAfterCreate,
        64,
      );
      dibObjectStorage.writeByteArray(Array(32).fill(0xa5));
      const dibStaleGetObject = invoke(
        "dib-get-object-stale",
        "GetObjectA",
        0x80818283,
        () => getObjectA(dib, 24, dibObject),
      );
      const dibStaleObjectStorageAfter = snapshotBoundedRawBytes(
        dibObjectStorage,
        32,
        32,
      );

      const invalidHdcBitsOutput = Memory.alloc(Process.pointerSize);
      const invalidHdcBitsSentinel = ptr("0x24681357");
      invalidHdcBitsOutput.writePointer(invalidHdcBitsSentinel);
      const dibInvalidHdc = invoke(
        "dib-create-deleted-hdc",
        "CreateDIBSection",
        0x90919293,
        () =>
          createDibSection(
            firstMemoryDc,
            dibInfo,
            0,
            invalidHdcBitsOutput,
            ptr(0),
            0,
          ),
      );
      const invalidHdcBitsAfter = invalidHdcBitsOutput.readPointer();
      let invalidHdcCleanup = null;
      if (dibInvalidHdc.systemResult !== "0x0") {
        const cleanup = deleteObject(ptr(dibInvalidHdc.systemResult));
        invalidHdcCleanup = {
          result: Number(cleanup.value),
          lastError: Number(cleanup.lastError) >>> 0,
        };
      }

      const malformedDibInfo = Memory.alloc(40);
      malformedDibInfo.writeByteArray(Array(40).fill(0));
      const malformedBitsOutput = Memory.alloc(Process.pointerSize);
      const malformedBitsSentinel = ptr("0x369c258b");
      malformedBitsOutput.writePointer(malformedBitsSentinel);
      const dibMalformedHeader = invoke(
        "dib-create-malformed-header",
        "CreateDIBSection",
        0xa0a1a2a3,
        () =>
          createDibSection(
            dibMemoryDc,
            malformedDibInfo,
            0,
            malformedBitsOutput,
            ptr(0),
            0,
          ),
      );
      const malformedBitsAfter = malformedBitsOutput.readPointer();
      let malformedCleanup = null;
      if (dibMalformedHeader.systemResult !== "0x0") {
        const cleanup = deleteObject(ptr(dibMalformedHeader.systemResult));
        malformedCleanup = {
          result: Number(cleanup.value),
          lastError: Number(cleanup.lastError) >>> 0,
        };
      }
      const dibMemoryDcDeleted = invoke(
        "dib-delete-compatible-dc",
        "DeleteDC",
        0xb0b1b2b3,
        () => deleteDc(dibMemoryDc),
      );
      const textDeletedDc = invoke(
        "text-out-deleted-dc",
        "TextOutA",
        0xb4b5b6b7,
        () => textOutA(dibMemoryDc, 0, 0, textBytes, 1),
      );
      const dibCalls = [
        dibMemoryDcCreated,
        dibCreated,
        textFontCreated,
        textFontSelected,
        dibGetObject,
        dibSelected,
        textFirst,
        textRepeat,
        textZeroLength,
        dibDeleteWhileSelected,
        dibRestored,
        textFontRestored,
        textFontDeleted,
        dibDeletedAfterRestore,
        dibRepeatedDelete,
        dibStaleGetObject,
        dibInvalidHdc,
        dibMalformedHeader,
        dibMemoryDcDeleted,
        textDeletedDc,
      ];

      fontObjectOracleEvidence = {
        profile: "x86-create-font-a-object-lifecycle-v5",
        arguments: {
          height: 64,
          width: 32,
          escapement: 0,
          orientation: 0,
          weight: 100,
          italic: 0,
          underline: 0,
          strikeOut: 0,
          characterSet: 128,
          outputPrecision: 4,
          clipPrecision: 0,
          quality: 2,
          pitchAndFamily: 0,
          faceBytes,
        },
        calls: [
          created,
          getObject,
          selected,
          restored,
          deleted,
          repeatedDelete,
          staleGetObject,
        ],
        objectStorageAfter,
        releaseDcResult: Number(release.value),
        releaseDcLastError: Number(release.lastError) >>> 0,
        staleObjectStorageAfter,
        createdHandle: created.systemResult,
        selectedPreviousObject: selected.systemResult,
        restoredFontObject: restored.systemResult,
        outlineTextMetrics: {
          ...outlineTextMetricsEvidence,
          deletedDcCall: outlineDeletedDc,
          allHookResultsMatch: [
            ...outlineTextMetricsEvidence.calls,
            outlineDeletedDc,
          ].every((call) => call.hookResultMatches),
          allHookAndSystemLastErrorsMatch: [
            ...outlineTextMetricsEvidence.calls,
            outlineDeletedDc,
          ].every((call) => call.hookAndSystemLastErrorMatch),
        },
        allHookResultsMatch: [
          created,
          getObject,
          selected,
          restored,
          deleted,
          repeatedDelete,
          staleGetObject,
        ].every((call) => call.hookResultMatches),
        allHookAndSystemLastErrorsMatch: [
          created,
          getObject,
          selected,
          restored,
          deleted,
          repeatedDelete,
          staleGetObject,
        ].every((call) => call.hookAndSystemLastErrorMatch),
        compatibleDc: {
          profile: "x86-create-compatible-dc-null-lifecycle-v1",
          calls: compatibleDcCalls,
          handlesDistinct:
            firstCompatibleDc.systemResult !== secondCompatibleDc.systemResult,
          firstInitialState,
          secondInitialState,
          initialFontShared:
            firstInitialState.currentFont === secondInitialState.currentFont,
          initialBitmapShared:
            firstInitialState.currentBitmap ===
            secondInitialState.currentBitmap,
          wrongReleaseResult: Number(wrongRelease.value),
          wrongReleaseLastError: Number(wrongRelease.lastError) >>> 0,
          firstTypeAfterWrongRelease,
          firstTypeAfterDelete,
          deleteObjectVariantResult: deleteObjectVariant.systemResult,
          thirdTypeAfterDeleteObject,
          allHookResultsMatch: compatibleDcCalls.every(
            (call) => call.hookResultMatches,
          ),
          allHookAndSystemLastErrorsMatch: compatibleDcCalls.every(
            (call) => call.hookAndSystemLastErrorMatch,
          ),
        },
        dibSection: {
          profile: "x86-create-dib-section-8bpp-lifecycle-v2",
          bitmapInfo: snapshotDibBitmapInfo(dibInfo),
          calls: dibCalls,
          bitsOutputSentinel: dibBitsSentinel.toString(),
          bitsOutputAfterCreate: dibBitsAfterCreate.toString(),
          pixelPlaneAfterCreate: dibPixelPlaneAfterCreate,
          backingAfterCreate: dibBackingAfterCreate,
          objectStorageAfter: dibObjectStorageAfter,
          selectedPreviousObject: dibSelected.systemResult,
          deleteWhileSelectedResult: dibDeleteWhileSelected.systemResult,
          typeAfterSelectedDelete: dibTypeAfterSelectedDelete,
          backingAfterSelectedDelete: dibBackingAfterSelectedDelete,
          restoredDibObject: dibRestored.systemResult,
          typeAfterRestore: dibTypeAfterRestore,
          backingAfterRestore: dibBackingAfterRestore,
          deleteAfterRestoreResult: dibDeletedAfterRestore.systemResult,
          repeatedDeleteResult: dibRepeatedDelete.systemResult,
          typeAfterDelete: dibTypeAfterDelete,
          backingAfterDelete: dibBackingAfterDelete,
          staleObjectStorageAfter: dibStaleObjectStorageAfter,
          invalidHdcBitsOutputSentinel: invalidHdcBitsSentinel.toString(),
          invalidHdcBitsOutputAfter: invalidHdcBitsAfter.toString(),
          invalidHdcCleanup,
          malformedBitsOutputSentinel: malformedBitsSentinel.toString(),
          malformedBitsOutputAfter: malformedBitsAfter.toString(),
          malformedCleanup,
          textOutA: {
            profile: "x86-text-out-a-8bpp-raster-v1",
            textBytes: [0x61],
            origin: { x: 0, y: 0 },
            calls: [textFirst, textRepeat, textZeroLength, textDeletedDc],
            selectedFont: textFont.toString(),
            selectedDib: dib.toString(),
            planeBefore: textPlaneBefore.evidence,
            planeAfterFirst: textPlaneAfterFirst.evidence,
            firstDiff: textFirstDiff,
            planeAfterRepeat: textPlaneAfterRepeat.evidence,
            repeatDiff: textRepeatDiff,
            planeAfterZeroLength: textPlaneAfterZeroLength.evidence,
            zeroLengthDiff: textZeroLengthDiff,
            restoredFont: textFontRestored.systemResult,
            deletedFontResult: textFontDeleted.systemResult,
            allHookResultsMatch: [
              textFirst,
              textRepeat,
              textZeroLength,
              textDeletedDc,
            ].every((call) => call.hookResultMatches),
            allHookAndSystemLastErrorsMatch: [
              textFirst,
              textRepeat,
              textZeroLength,
              textDeletedDc,
            ].every((call) => call.hookAndSystemLastErrorMatch),
          },
          allHookResultsMatch: dibCalls.every((call) => call.hookResultMatches),
          allHookAndSystemLastErrorsMatch: dibCalls.every(
            (call) => call.hookAndSystemLastErrorMatch,
          ),
        },
      };
      fontObjectOracleComplete = true;
      emit("font-object-oracle", fontObjectOracleEvidence);
      scheduleFontObjectCompletion();
    } catch (error) {
      emit("font-object-oracle-error", { error: error.message });
    }
  });
}

function attachSetWindowLongAHook(module) {
  const name = "SetWindowLongA";
  const address = findExport(module, name);
  const getWindowLongAddress = findExport(module, "GetWindowLongA");
  const isWindowVisibleAddress = findExport(module, "IsWindowVisible");
  if (
    address === null ||
    getWindowLongAddress === null ||
    isWindowVisibleAddress === null
  ) {
    throw new Error("SetWindowLongA state-query exports are unavailable");
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  const getWindowLongA = new NativeFunction(
    getWindowLongAddress,
    "int32",
    ["pointer", "int32"],
    "stdcall",
  );
  const isWindowVisible = new NativeFunction(
    isWindowVisibleAddress,
    "int32",
    ["pointer"],
    "stdcall",
  );
  const snapshotWindowState = (windowHandle) => ({
    style: Number(getWindowLongA(windowHandle, -16)) >>> 0,
    visible: Number(isWindowVisible(windowHandle)) !== 0,
  });

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      const isMainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      const isOracleCaller =
        callerModule === "frida-agent.dll" && setWindowLongAOracleCall !== null;
      if (!isMainModuleCaller && !isOracleCaller) {
        this.traceFrame = null;
        return;
      }
      this.windowHandle = args[0];
      this.index = signed32(args[1]);
      this.newValue = args[2].toUInt32();
      this.mainModuleCaller = isMainModuleCaller;
      this.oracleCaller = isOracleCaller;
      if (isOracleCaller) {
        this.lastError = setWindowLongAOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.stateBefore = snapshotWindowState(this.windowHandle);
      this.lastError = this.lastErrorBefore;
      this.immediateConsumer = isMainModuleCaller
        ? snapshotInstructions(this.returnAddress, 96)
        : null;
      if (isOracleCaller) {
        setWindowLongAOracleCall.hookStateBefore = this.stateBefore;
      }
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.windowHandle.toString(), this.index, this.newValue],
          lastErrorBefore: this.lastErrorBefore,
          setWindowLongA: {
            stateBefore: this.stateBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const resultUnsigned = result.toUInt32();
      const lastError = Number(this.lastError) >>> 0;
      const stateAfter = snapshotWindowState(this.windowHandle);
      this.lastError = lastError;
      if (this.oracleCaller) {
        setWindowLongAOracleCall.hookResultUnsigned = resultUnsigned;
        setWindowLongAOracleCall.hookLastErrorAfter = lastError;
        setWindowLongAOracleCall.hookStateAfter = stateAfter;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          setWindowLongA: {
            resultUnsigned,
            stateAfter,
          },
        },
        this.lastError,
      );
      if (!this.mainModuleCaller || setWindowLongACompletionRecorded) {
        return;
      }
      setWindowLongAMainModuleCalls.push({
        callId: this.traceFrame.callId,
        returnAddress: this.returnAddress.toString(),
        callerRva: this.traceFrame.caller.rva,
        windowHandle: this.windowHandle.toString(),
        index: this.index,
        newValue: this.newValue,
        resultUnsigned,
        lastErrorBefore: this.lastErrorBefore,
        lastError,
        stateBefore: this.stateBefore,
        stateAfter,
        immediateConsumer: this.immediateConsumer,
      });
      scheduleSetWindowLongAOracle(module, this.windowHandle, stateAfter.style);
      setImmediate(maybeCompleteSetWindowLongAObservation);
    },
  });
}

function scheduleSetWindowLongAOracle(module, liveWindowHandle, liveStyle) {
  if (setWindowLongAOracleScheduled) {
    return;
  }
  setWindowLongAOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "SetWindowLongA");
      if (address === null) {
        throw new Error("SetWindowLongA oracle export is unavailable");
      }
      const setWindowLongA = new SystemFunction(
        address,
        "int32",
        ["pointer", "int32", "int32"],
        "stdcall",
      );
      const invoke = ({ id, windowHandle, index, newValue, sentinel }) => {
        setWindowLongAOracleCall = {
          sentinel,
          hookResultUnsigned: null,
          hookLastErrorAfter: null,
          hookStateBefore: null,
          hookStateAfter: null,
        };
        try {
          const outcome = setWindowLongA(windowHandle, index, newValue | 0);
          const systemResultUnsigned = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            setWindowLongAOracleCall.hookResultUnsigned === null ||
            setWindowLongAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              "SetWindowLongA oracle did not pass through the hook",
            );
          }
          return {
            id,
            windowHandle: windowHandle.toString(),
            index,
            newValue,
            sentinel,
            systemResultUnsigned,
            systemLastError,
            hookResultUnsigned: setWindowLongAOracleCall.hookResultUnsigned,
            hookLastErrorAfter: setWindowLongAOracleCall.hookLastErrorAfter,
            hookStateBefore: setWindowLongAOracleCall.hookStateBefore,
            hookStateAfter: setWindowLongAOracleCall.hookStateAfter,
            hookResultMatches:
              setWindowLongAOracleCall.hookResultUnsigned ===
              systemResultUnsigned,
            hookAndSystemLastErrorMatch:
              setWindowLongAOracleCall.hookLastErrorAfter === systemLastError,
          };
        } finally {
          setWindowLongAOracleCall = null;
        }
      };
      const calls = [
        {
          id: "live-same-style",
          windowHandle: liveWindowHandle,
          index: -16,
          newValue: liveStyle,
          sentinel: 0x11223344,
        },
        {
          id: "invalid-window",
          windowHandle: ptr("0x1234"),
          index: -16,
          newValue: liveStyle,
          sentinel: 0x55667788,
        },
        {
          id: "invalid-index",
          windowHandle: liveWindowHandle,
          index: -100,
          newValue: liveStyle,
          sentinel: 0x7a6b5c4d,
        },
      ].map(invoke);
      setWindowLongAOracleEvidence = {
        profile: "x86-live-gwl-style-and-invalid-arguments-v1",
        liveWindowHandle: liveWindowHandle.toString(),
        liveStyle,
        calls,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      setWindowLongAOracleComplete = true;
      emit("set-window-long-a-oracle", setWindowLongAOracleEvidence);
      maybeCompleteSetWindowLongAObservation();
    } catch (error) {
      emit("set-window-long-a-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteSetWindowLongAObservation() {
  if (
    setWindowLongACompletionRecorded ||
    !setWindowLongAOracleComplete ||
    setWindowLongAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteSetWindowLongAObservation, 10);
    return;
  }
  setWindowLongACompletionRecorded = true;
  emit("set-window-long-a-complete", {
    profile: "x86-bgi-first-gwl-style-update-v1",
    mainModuleCalls: setWindowLongAMainModuleCalls,
    oracle: setWindowLongAOracleEvidence,
  });
  if (stopOnEvent === "set-window-long-a-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteDragAcceptFilesObservation() {
  if (dragAcceptFilesCompletionRecorded || dragAcceptFilesCompletion === null) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteDragAcceptFilesObservation, 10);
    return;
  }
  try {
    const shell32 = Process.getModuleByName("SHELL32.dll");
    const user32 = Process.getModuleByName("USER32.dll");
    const kernel32 = Process.getModuleByName("KERNEL32.dll");
    const dragAddress = findExport(shell32, "DragAcceptFiles");
    const getWindowLongAddress = findExport(user32, "GetWindowLongA");
    const setLastErrorAddress = findExport(kernel32, "SetLastError");
    if (
      dragAddress === null ||
      getWindowLongAddress === null ||
      setLastErrorAddress === null
    ) {
      throw new Error("DragAcceptFiles oracle exports are unavailable");
    }
    const dragAcceptFiles = new SystemFunction(
      dragAddress,
      "void",
      ["pointer", "uint32"],
      "stdcall",
    );
    const getWindowLongA = new NativeFunction(
      getWindowLongAddress,
      "int32",
      ["pointer", "int32"],
      "stdcall",
    );
    const setLastError = new NativeFunction(
      setLastErrorAddress,
      "void",
      ["uint32"],
      "stdcall",
    );
    const window = ptr(dragAcceptFilesCompletion.window);
    const acceptsFiles = () =>
      ((getWindowLongA(window, -20) >>> 0) & 0x0000_0010) !== 0;
    const initialState = acceptsFiles();
    const invoke = (accept, sentinel) => {
      setLastError(sentinel);
      const outcome = dragAcceptFiles(window, accept ? 1 : 0);
      return {
        accept,
        sentinel,
        lastError: Number(outcome.lastError) >>> 0,
        stateAfter: acceptsFiles(),
      };
    };
    const calls = [invoke(true, 0x5566_7788), invoke(false, 0x7a6b_5c4d)];
    if (acceptsFiles() !== initialState) {
      calls.push(invoke(initialState, 0x1234_5678));
    }
    dragAcceptFilesCompletion.oracle = {
      profile: "x86-live-window-drag-accept-files-v1",
      initialState,
      calls,
      finalState: acceptsFiles(),
      allLastErrorsPreserved: calls.every(
        (call) => call.lastError === call.sentinel,
      ),
    };
  } catch (error) {
    dragAcceptFilesCompletion.oracle = { error: error.message };
    emit("drag-accept-files-oracle-error", { error: error.message });
  }
  dragAcceptFilesCompletionRecorded = true;
  emit("drag-accept-files-complete", dragAcceptFilesCompletion);
}

function attachHook(module, name, decoders) {
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const isFocusedHeapApi = name === "HeapAlloc" || name === "HeapFree";
      const isMainModuleCaller =
        caller.module !== null &&
        caller.module.toLowerCase() === Process.mainModule.name.toLowerCase();
      const tracesDinput8Factory =
        (focusedDinput8Observation || selectedVtables.has("idirectinput8a")) &&
        (!focusedDinput8Observation ||
          (!focusedDinput8RootComplete &&
            focusedDinput8FactoryCall === null)) &&
        name === "DirectInput8Create" &&
        isMainModuleCaller;
      const tracesDsoundFactory =
        focusedDsoundObservation &&
        !focusedDsoundRootComplete &&
        !focusedDsoundCreateBufferComplete &&
        !focusedDsoundBufferLockUnlockPlayComplete &&
        focusedDsoundFactoryCall === null &&
        name === "DirectSoundCreate8" &&
        isMainModuleCaller;
      const recordsInitializeCriticalSectionConsumer =
        selectedObservations.has("critical-section") &&
        name === "InitializeCriticalSection" &&
        isMainModuleCaller &&
        initializeCriticalSectionConsumersRecorded < 2;
      if (recordsInitializeCriticalSectionConsumer) {
        initializeCriticalSectionConsumersRecorded += 1;
      }
      const focusedHeapObservation =
        selectedObservations.has("heap-allocation") &&
        isFocusedHeapApi &&
        isMainModuleCaller;
      const heapSizeAllocationTracking =
        selectedObservations.has("heap-size") &&
        !selectedObservations.has("heap-allocation") &&
        isFocusedHeapApi &&
        isMainModuleCaller;
      const focusedGetCpInfoObservation =
        selectedObservations.has("get-cp-info") &&
        name === "GetCPInfo" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          caller.module.toLowerCase() === "frida-agent.dll");
      const focusedProcessorFeatureObservation =
        selectedObservations.has("processor-feature") &&
        name === "IsProcessorFeaturePresent" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          caller.module.toLowerCase() === "frida-agent.dll");
      const focusedSetThreadAffinityMaskObservation =
        selectedObservations.has("set-thread-affinity-mask") &&
        name === "SetThreadAffinityMask" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            setThreadAffinityMaskOracleSentinel !== null));
      const focusedGetSystemInfoObservation =
        selectedObservations.has("get-system-info") &&
        name === "GetSystemInfo" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            getSystemInfoOracleSentinel !== null));
      const focusedGlobalMemoryStatusObservation =
        selectedObservations.has("global-memory-status") &&
        name === "GlobalMemoryStatus" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            globalMemoryStatusOracleSentinel !== null));
      const focusedEnumDisplayMonitorsObservation =
        selectedObservations.has("enum-display-monitors") &&
        name === "EnumDisplayMonitors" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            enumDisplayMonitorsOracleSentinel !== null));
      const focusedSwapMouseButtonObservation =
        selectedObservations.has("swap-mouse-button") &&
        name === "SwapMouseButton" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            swapMouseButtonRestoreActive));
      const tracesDragAcceptFiles =
        selectedObservations.has("drag-accept-files") &&
        name === "DragAcceptFiles" &&
        isMainModuleCaller;
      const focusedTimeGetDevCapsObservation =
        selectedObservations.has("time-get-dev-caps") &&
        name === "timeGetDevCaps" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            timeGetDevCapsOracleCall !== null));
      const focusedTimePeriodObservation =
        selectedObservations.has("time-get-dev-caps") &&
        (name === "timeBeginPeriod" || name === "timeEndPeriod") &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            timePeriodOracleCall !== null));
      const focusedUnhandledExceptionFilterObservation =
        selectedObservations.has("unhandled-exception-filter") &&
        name === "SetUnhandledExceptionFilter" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          caller.module.toLowerCase() === "frida-agent.dll");
      const focusedHeapSizeObservation =
        selectedObservations.has("heap-size") &&
        name === "HeapSize" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          caller.module.toLowerCase() === "frida-agent.dll");
      const focusedMultiByteObservation =
        selectedObservations.has("multi-byte-to-wide-char") &&
        name === "MultiByteToWideChar" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          caller.module.toLowerCase() === "frida-agent.dll");
      const focusedSetCurrentDirectoryAObservation =
        selectedObservations.has("set-current-directory-a") &&
        name === "SetCurrentDirectoryA" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            setCurrentDirectoryOracleSentinel !== null));
      const focusedSourceFileCreateObservation =
        selectedObservations.has("source-file-read") &&
        name === "CreateFileA" &&
        isMainModuleCaller;
      const sourceFileHandleKey =
        selectedObservations.has("source-file-read") &&
        [
          "ReadFile",
          "WriteFile",
          "SetFilePointer",
          "GetFileSize",
          "FlushFileBuffers",
          "CloseHandle",
        ].includes(name)
          ? args[0].toString().toLowerCase()
          : null;
      const focusedSourceFileOperationObservation =
        sourceFileHandleKey !== null &&
        observedSourceFileHandles.has(sourceFileHandleKey);
      const isCreateMutexA = name === "CreateMutexA";
      const isMutexLifecycleApi =
        name === "WaitForSingleObject" ||
        name === "ReleaseMutex" ||
        name === "CloseHandle";
      const mutexHandleKey = isMutexLifecycleApi
        ? args[0].toString().toLowerCase()
        : null;
      const focusedCreateMutexAObservation =
        selectedObservations.has("create-mutex-a") &&
        isCreateMutexA &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            createMutexOracleSentinel !== null));
      const focusedMutexLifecycleObservation =
        selectedObservations.has("create-mutex-a") &&
        isMutexLifecycleApi &&
        mutexHandleKey !== null &&
        trackedMutexHandles.has(mutexHandleKey);
      const focusedGetStringTypeObservation =
        selectedObservations.has("get-string-type-w") &&
        name === "GetStringTypeW" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          caller.module.toLowerCase() === "frida-agent.dll");
      const focusedLCMapStringObservation =
        selectedObservations.has("lc-map-string-w") &&
        name === "LCMapStringW" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          caller.module.toLowerCase() === "frida-agent.dll");
      const focusedInterlockedDecrementObservation =
        selectedObservations.has("interlocked-decrement") &&
        name === "InterlockedDecrement" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          caller.module.toLowerCase() === "frida-agent.dll");
      const focusedGetModuleFileNameAObservation =
        selectedObservations.has("get-module-file-name-a") &&
        name === "GetModuleFileNameA" &&
        caller.module !== null &&
        (isMainModuleCaller ||
          (caller.module.toLowerCase() === "frida-agent.dll" &&
            getModuleFileNameAOracleSentinel !== null));
      if (heapSizeAllocationTracking) {
        this.traceFrame = null;
        this.trackHeapSizeAllocation =
          name === "HeapAlloc"
            ? {
                requestedBytes: args[2].toUInt32(),
              }
            : null;
        this.trackHeapSizeFree =
          name === "HeapFree" ? args[2].toString().toLowerCase() : null;
        return;
      }
      if (
        selectedObservations.has("heap-size") &&
        !selectedObservations.has("heap-allocation") &&
        !selectedObservations.has("heap-lifecycle") &&
        isFocusedHeapApi
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("heap-allocation") &&
        !selectedObservations.has("heap-lifecycle") &&
        isFocusedHeapApi &&
        !focusedHeapObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("get-cp-info") &&
        name === "GetCPInfo" &&
        !focusedGetCpInfoObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("processor-feature") &&
        name === "IsProcessorFeaturePresent" &&
        !focusedProcessorFeatureObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("set-thread-affinity-mask") &&
        name === "SetThreadAffinityMask" &&
        !focusedSetThreadAffinityMaskObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("get-system-info") &&
        name === "GetSystemInfo" &&
        !focusedGetSystemInfoObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("global-memory-status") &&
        name === "GlobalMemoryStatus" &&
        !focusedGlobalMemoryStatusObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("enum-display-monitors") &&
        name === "EnumDisplayMonitors" &&
        !focusedEnumDisplayMonitorsObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("swap-mouse-button") &&
        name === "SwapMouseButton" &&
        !focusedSwapMouseButtonObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("drag-accept-files") &&
        name === "DragAcceptFiles" &&
        !tracesDragAcceptFiles
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("time-get-dev-caps") &&
        name === "timeGetDevCaps" &&
        !focusedTimeGetDevCapsObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("time-get-dev-caps") &&
        (name === "timeBeginPeriod" || name === "timeEndPeriod") &&
        !focusedTimePeriodObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("unhandled-exception-filter") &&
        name === "SetUnhandledExceptionFilter" &&
        !focusedUnhandledExceptionFilterObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("heap-size") &&
        name === "HeapSize" &&
        !focusedHeapSizeObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("multi-byte-to-wide-char") &&
        name === "MultiByteToWideChar" &&
        !focusedMultiByteObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("set-current-directory-a") &&
        name === "SetCurrentDirectoryA" &&
        !focusedSetCurrentDirectoryAObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("source-file-read") &&
        name === "CreateFileA" &&
        !focusedSourceFileCreateObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("source-file-read") &&
        [
          "ReadFile",
          "WriteFile",
          "SetFilePointer",
          "GetFileSize",
          "FlushFileBuffers",
          "CloseHandle",
        ].includes(name) &&
        !focusedSourceFileOperationObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("create-mutex-a") &&
        (isCreateMutexA || isMutexLifecycleApi) &&
        !focusedCreateMutexAObservation &&
        !focusedMutexLifecycleObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("get-string-type-w") &&
        name === "GetStringTypeW" &&
        !focusedGetStringTypeObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("lc-map-string-w") &&
        name === "LCMapStringW" &&
        !focusedLCMapStringObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("interlocked-decrement") &&
        name === "InterlockedDecrement" &&
        !focusedInterlockedDecrementObservation
      ) {
        this.traceFrame = null;
        return;
      }
      if (
        selectedObservations.has("get-module-file-name-a") &&
        name === "GetModuleFileNameA" &&
        !focusedGetModuleFileNameAObservation
      ) {
        this.traceFrame = null;
        return;
      }
      this.traceCriticalSectionOracle =
        criticalSectionOracleSentinel !== null &&
        selectedObservations.has("critical-section") &&
        name.includes("CriticalSection") &&
        caller.module === "frida-agent.dll";
      if (this.traceCriticalSectionOracle) {
        this.lastError = criticalSectionOracleSentinel;
      }
      this.traceEnvironmentStringsOracle =
        environmentStringsOracleSentinel !== null &&
        selectedObservations.has("environment-strings-w") &&
        (name === "GetEnvironmentStringsW" ||
          name === "FreeEnvironmentStringsW") &&
        caller.module === "frida-agent.dll";
      if (this.traceEnvironmentStringsOracle) {
        this.lastError = environmentStringsOracleSentinel;
      }
      this.traceWideCharOracle =
        wideCharOracleSentinel !== null &&
        selectedObservations.has("wide-char-to-multi-byte") &&
        name === "WideCharToMultiByte" &&
        caller.module === "frida-agent.dll";
      if (this.traceWideCharOracle) {
        this.lastError = wideCharOracleSentinel;
      }
      this.traceMultiByteOracle =
        multiByteOracleSentinel !== null &&
        selectedObservations.has("multi-byte-to-wide-char") &&
        name === "MultiByteToWideChar" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceMultiByteOracle) {
        this.lastError = multiByteOracleSentinel;
      }
      this.traceSetCurrentDirectoryOracle =
        setCurrentDirectoryOracleSentinel !== null &&
        selectedObservations.has("set-current-directory-a") &&
        name === "SetCurrentDirectoryA" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceSetCurrentDirectoryOracle) {
        this.lastError = setCurrentDirectoryOracleSentinel;
      }
      this.traceCreateMutexOracle =
        createMutexOracleSentinel !== null &&
        selectedObservations.has("create-mutex-a") &&
        (isCreateMutexA || isMutexLifecycleApi) &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceCreateMutexOracle) {
        this.lastError = createMutexOracleSentinel;
      }
      this.traceGetStringTypeOracle =
        getStringTypeOracleSentinel !== null &&
        selectedObservations.has("get-string-type-w") &&
        name === "GetStringTypeW" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceGetStringTypeOracle) {
        this.lastError = getStringTypeOracleSentinel;
      }
      this.traceLCMapStringOracle =
        lcMapStringOracleSentinel !== null &&
        selectedObservations.has("lc-map-string-w") &&
        name === "LCMapStringW" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceLCMapStringOracle) {
        this.lastError = lcMapStringOracleSentinel;
      }
      this.traceInterlockedDecrementOracle =
        interlockedDecrementOracleSentinel !== null &&
        selectedObservations.has("interlocked-decrement") &&
        name === "InterlockedDecrement" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceInterlockedDecrementOracle) {
        this.lastError = interlockedDecrementOracleSentinel;
      }
      this.traceGetModuleFileNameAOracle =
        getModuleFileNameAOracleSentinel !== null &&
        selectedObservations.has("get-module-file-name-a") &&
        name === "GetModuleFileNameA" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceGetModuleFileNameAOracle) {
        this.lastError = getModuleFileNameAOracleSentinel;
      }
      this.traceGetLastErrorOracle =
        getLastErrorOracleSentinel !== null &&
        selectedObservations.has("get-last-error") &&
        name === "GetLastError" &&
        caller.module === "frida-agent.dll";
      if (this.traceGetLastErrorOracle) {
        this.lastError = getLastErrorOracleSentinel;
      }
      this.traceSetLastErrorOracle =
        setLastErrorOracleActive &&
        selectedObservations.has("set-last-error") &&
        name === "SetLastError" &&
        caller.module === "frida-agent.dll";
      if (this.traceSetLastErrorOracle) {
        setLastErrorOracleArgument = args[0].toUInt32();
        setLastErrorOracleEaxBefore = this.context.eax.toUInt32();
      }
      this.traceGetAcpOracle =
        getAcpOracleSentinel !== null &&
        selectedObservations.has("get-acp") &&
        name === "GetACP" &&
        caller.module === "frida-agent.dll";
      if (this.traceGetAcpOracle) {
        this.lastError = getAcpOracleSentinel;
      }
      this.traceSystemDefaultLangIdOracle =
        systemDefaultLangIdOracleSentinel !== null &&
        selectedObservations.has("system-default-lang-id") &&
        name === "GetSystemDefaultLangID" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceSystemDefaultLangIdOracle) {
        this.lastError = systemDefaultLangIdOracleSentinel;
      }
      this.traceDriveTypeOracle =
        driveTypeOracleSentinel !== null &&
        selectedObservations.has("drive-type") &&
        name === "GetDriveTypeA" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceDriveTypeOracle) {
        this.lastError = driveTypeOracleSentinel;
      }
      this.traceGetCpInfoOracle =
        getCpInfoOracleSentinel !== null &&
        selectedObservations.has("get-cp-info") &&
        name === "GetCPInfo" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceGetCpInfoOracle) {
        this.lastError = getCpInfoOracleSentinel;
      }
      this.traceProcessorFeatureOracle =
        processorFeatureOracleSentinel !== null &&
        selectedObservations.has("processor-feature") &&
        name === "IsProcessorFeaturePresent" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceProcessorFeatureOracle) {
        this.lastError = processorFeatureOracleSentinel;
      }
      this.traceSetThreadAffinityMaskOracle =
        setThreadAffinityMaskOracleSentinel !== null &&
        selectedObservations.has("set-thread-affinity-mask") &&
        name === "SetThreadAffinityMask" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceSetThreadAffinityMaskOracle) {
        this.lastError = setThreadAffinityMaskOracleSentinel;
      }
      this.traceGetSystemInfoOracle =
        getSystemInfoOracleSentinel !== null &&
        selectedObservations.has("get-system-info") &&
        name === "GetSystemInfo" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceGetSystemInfoOracle) {
        this.lastError = getSystemInfoOracleSentinel;
      }
      this.traceGlobalMemoryStatusOracle =
        globalMemoryStatusOracleSentinel !== null &&
        selectedObservations.has("global-memory-status") &&
        name === "GlobalMemoryStatus" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceGlobalMemoryStatusOracle) {
        this.lastError = globalMemoryStatusOracleSentinel;
      }
      this.traceEnumDisplayMonitorsOracle =
        enumDisplayMonitorsOracleSentinel !== null &&
        selectedObservations.has("enum-display-monitors") &&
        name === "EnumDisplayMonitors" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceEnumDisplayMonitorsOracle) {
        this.lastError = enumDisplayMonitorsOracleSentinel;
      }
      this.traceTimeGetDevCapsOracle =
        timeGetDevCapsOracleCall !== null &&
        selectedObservations.has("time-get-dev-caps") &&
        name === "timeGetDevCaps" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceTimeGetDevCapsOracle) {
        this.lastError = timeGetDevCapsOracleCall.sentinel;
      }
      this.traceTimePeriodOracle =
        timePeriodOracleCall !== null &&
        selectedObservations.has("time-get-dev-caps") &&
        name === timePeriodOracleCall.api &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceTimePeriodOracle) {
        this.lastError = timePeriodOracleCall.sentinel;
      }
      this.traceComInitializationOracle =
        comInitializationOracleCall !== null &&
        selectedObservations.has("com-initialization") &&
        name === comInitializationOracleCall.api &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceComInitializationOracle) {
        this.lastError = comInitializationOracleCall.sentinel;
      }
      this.traceUnhandledExceptionFilterOracle =
        unhandledExceptionFilterOracleSentinel !== null &&
        selectedObservations.has("unhandled-exception-filter") &&
        name === "SetUnhandledExceptionFilter" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceUnhandledExceptionFilterOracle) {
        this.lastError = unhandledExceptionFilterOracleSentinel;
      }
      this.traceHeapSizeOracle =
        heapSizeOracleSentinel !== null &&
        selectedObservations.has("heap-size") &&
        name === "HeapSize" &&
        caller.module !== null &&
        caller.module.toLowerCase() === "frida-agent.dll";
      if (this.traceHeapSizeOracle) {
        this.lastError = heapSizeOracleSentinel;
      }
      const arguments_ = decoders.map((decoder, index) =>
        decoder === null
          ? args[index].toString()
          : decodeArgument(args[index], decoder),
      );
      this.traceDinput8Factory = tracesDinput8Factory
        ? {
            instance: args[0].toString(),
            version: args[1].toUInt32(),
            iid: snapshotBoundedRawBytes(args[2], 16, 16),
            output: {
              address: args[3].toString(),
              before: snapshotPointerSlot(args[3]),
            },
            outerUnknown: args[4].toString(),
            lastErrorBefore: Number(this.lastError) >>> 0,
            immediateConsumer: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              96,
            ),
          }
        : null;
      this.traceDinput8FactoryOutput = tracesDinput8Factory ? args[3] : null;
      this.traceDinput8FactoryArguments = tracesDinput8Factory
        ? arguments_
        : null;
      this.traceDsoundFactory = tracesDsoundFactory
        ? {
            deviceGuid: args[0].isNull()
              ? null
              : snapshotBoundedRawBytes(args[0], 16, 16),
            output: {
              address: args[1].toString(),
              before: snapshotPointerSlot(args[1]),
            },
            outerUnknown: args[2].toString(),
            lastErrorBefore: Number(this.lastError) >>> 0,
            immediateConsumer: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              96,
            ),
          }
        : null;
      this.traceDsoundFactoryOutput = tracesDsoundFactory ? args[1] : null;
      this.traceDsoundFactoryArguments = tracesDsoundFactory
        ? arguments_
        : null;
      this.traceThreadAffinityPrimaryCall =
        focusedSetThreadAffinityMaskObservation && isMainModuleCaller
          ? ++setThreadAffinityMaskPrimaryCalls
          : null;
      this.traceThreadAffinityRequestedMask =
        this.traceThreadAffinityPrimaryCall === null
          ? null
          : args[1].toUInt32();
      this.traceHeapAllocation =
        focusedHeapObservation && name === "HeapAlloc"
          ? {
              heap: args[0].toString(),
              flags: args[1].toUInt32(),
              requestedBytes: args[2].toUInt32(),
              lastErrorBefore: Number(this.lastError) >>> 0,
              immediateConsumer: {
                apiContinuation: snapshotInstructionsFollowingDirectJump(
                  this.returnAddress,
                  16,
                ),
                frameCallerContinuations: snapshotX86FrameCallerContinuations(
                  this.context,
                  4,
                  12,
                ),
              },
            }
          : null;
      const heapFreePointer =
        focusedHeapObservation && name === "HeapFree"
          ? args[2].toString().toLowerCase()
          : null;
      const relatedHeapAllocation =
        heapFreePointer === null
          ? null
          : (observedMainModuleHeapAllocations.get(heapFreePointer) ?? null);
      const largeHeapAccess =
        relatedHeapAllocation === null
          ? null
          : finishLargeHeapAccessObservation(
              heapFreePointer,
              "matching-heap-free-enter",
            );
      this.traceHeapFree =
        heapFreePointer === null
          ? null
          : {
              heap: args[0].toString().toLowerCase(),
              flags: args[1].toUInt32(),
              pointer: heapFreePointer,
              relatedAllocationCallId: relatedHeapAllocation?.callId ?? null,
              requestedBytes: relatedHeapAllocation?.requestedBytes ?? null,
              relatedAllocation: relatedHeapAllocation,
              largeHeapAccess,
              bufferBeforeFree:
                relatedHeapAllocation === null
                  ? null
                  : snapshotByteBuffer(
                      args[2],
                      relatedHeapAllocation.requestedBytes,
                    ),
            };
      const heapSizePointer = focusedHeapSizeObservation
        ? args[2].toString().toLowerCase()
        : null;
      const relatedHeapSizeAllocation =
        heapSizePointer === null
          ? null
          : (observedMainModuleHeapAllocations.get(heapSizePointer) ?? null);
      const recordsHeapSizeConsumer =
        focusedHeapSizeObservation &&
        isMainModuleCaller &&
        heapSizeConsumersRecorded < 2;
      if (recordsHeapSizeConsumer) {
        heapSizeConsumersRecorded += 1;
      }
      this.traceHeapSize =
        heapSizePointer === null
          ? null
          : {
              heap: args[0].toString().toLowerCase(),
              flags: args[1].toUInt32(),
              pointer: heapSizePointer,
              relatedAllocationCallId:
                relatedHeapSizeAllocation?.callId ?? null,
              requestedBytes: relatedHeapSizeAllocation?.requestedBytes ?? null,
              immediateConsumer: recordsHeapSizeConsumer
                ? {
                    apiContinuation: snapshotInstructionsFollowingDirectJump(
                      this.returnAddress,
                      24,
                    ),
                    frameCallerContinuations:
                      snapshotX86FrameCallerContinuations(this.context, 4, 12),
                  }
                : null,
            };
      this.traceWideCharConversion =
        name === "WideCharToMultiByte" &&
        selectedObservations.has("wide-char-to-multi-byte")
          ? {
              source: args[2],
              sourceUnits: signed32(args[3]),
              destination: args[4],
              destinationBytes: signed32(args[5]),
              usedDefaultChar: args[7],
            }
          : null;
      this.traceMultiByteConversion = focusedMultiByteObservation
        ? {
            codePage: args[0],
            flags: args[1],
            source: args[2],
            sourceBytes: signed32(args[3]),
            destination: args[4],
            destinationUnits: signed32(args[5]),
          }
        : null;
      this.traceSetCurrentDirectoryA = focusedSetCurrentDirectoryAObservation
        ? {
            path: args[0],
            currentDirectoryBefore: (() => {
              const lastErrorBefore = Number(this.lastError) >>> 0;
              const currentDirectory = snapshotCurrentDirectoryA(module);
              this.lastError = lastErrorBefore;
              return currentDirectory;
            })(),
          }
        : null;
      this.traceSourceFileCreate = focusedSourceFileCreateObservation
        ? {
            path: snapshotMultiByteSource(args[0], -1),
            desiredAccess: args[1].toUInt32(),
            shareMode: args[2].toUInt32(),
            securityAttributes: args[3].toString(),
            creationDisposition: args[4].toUInt32(),
            flagsAndAttributes: args[5].toUInt32(),
            templateFile: args[6].toString(),
            lastErrorBefore: Number(this.lastError) >>> 0,
            immediateConsumer: {
              apiContinuation: snapshotInstructionsFollowingDirectJump(
                this.returnAddress,
                32,
              ),
              frameCallerContinuations: snapshotX86FrameCallerContinuations(
                this.context,
                4,
                16,
              ),
            },
          }
        : null;
      const sourceFileRecord =
        sourceFileHandleKey === null
          ? null
          : (observedSourceFileHandles.get(sourceFileHandleKey) ?? null);
      this.traceSourceFileOperation =
        sourceFileRecord === null
          ? null
          : {
              api: name,
              handle: sourceFileHandleKey,
              record: sourceFileRecord,
              lastErrorBefore: Number(this.lastError) >>> 0,
              immediateConsumer: {
                apiContinuation: snapshotInstructionsFollowingDirectJump(
                  this.returnAddress,
                  32,
                ),
                frameCallerContinuations: snapshotX86FrameCallerContinuations(
                  this.context,
                  4,
                  16,
                ),
              },
              ...(name === "ReadFile"
                ? {
                    buffer: args[1],
                    requestedBytes: args[2].toUInt32(),
                    bytesRead: args[3],
                    bytesReadBefore: snapshotOptionalU32(args[3]),
                    overlapped: args[4].toString(),
                  }
                : {}),
              ...(name === "WriteFile"
                ? {
                    buffer: args[1],
                    requestedBytes: args[2].toUInt32(),
                    bytesWritten: args[3],
                    bytesWrittenBefore: snapshotOptionalU32(args[3]),
                    overlapped: args[4].toString(),
                    inputBefore: snapshotByteBuffer(
                      args[1],
                      args[2].toUInt32(),
                    ),
                  }
                : {}),
              ...(name === "SetFilePointer"
                ? {
                    distanceLow: signed32(args[1]),
                    distanceHigh: args[2],
                    distanceHighBefore: snapshotOptionalU32(args[2]),
                    moveMethod: args[3].toUInt32(),
                  }
                : {}),
              ...(name === "GetFileSize"
                ? {
                    sizeHigh: args[1],
                    sizeHighBefore: snapshotOptionalU32(args[1]),
                  }
                : {}),
            };
      this.traceCreateMutexA = focusedCreateMutexAObservation
        ? {
            securityAttributes: args[0],
            initialOwner: args[1],
            name: args[2],
            nameBefore: snapshotMultiByteSource(args[2], -1),
          }
        : null;
      const recordsMutexLifecycleConsumer =
        focusedMutexLifecycleObservation &&
        isMainModuleCaller &&
        mutexLifecycleConsumersRecorded < 6;
      if (recordsMutexLifecycleConsumer) {
        mutexLifecycleConsumersRecorded += 1;
      }
      this.traceMutexLifecycle = focusedMutexLifecycleObservation
        ? {
            operation: name,
            handle: mutexHandleKey,
            trackedObject: trackedMutexHandles.get(mutexHandleKey),
            timeoutMilliseconds:
              name === "WaitForSingleObject" ? args[1].toUInt32() : null,
            immediateConsumer: recordsMutexLifecycleConsumer
              ? snapshotInstructionsFollowingDirectJump(this.returnAddress, 24)
              : null,
          }
        : null;
      this.traceGetStringTypeW = focusedGetStringTypeObservation
        ? {
            infoType: args[0],
            source: args[1],
            sourceUnits: signed32(args[2]),
            output: args[3],
          }
        : null;
      this.traceLCMapStringW = focusedLCMapStringObservation
        ? {
            locale: args[0],
            flags: args[1],
            source: args[2],
            sourceUnits: signed32(args[3]),
            destination: args[4],
            destinationUnits: signed32(args[5]),
          }
        : null;
      this.traceInterlockedDecrement = focusedInterlockedDecrementObservation
        ? { target: args[0] }
        : null;
      this.traceGetModuleFileNameA = focusedGetModuleFileNameAObservation
        ? {
            moduleHandle: args[0],
            output: args[1],
            capacity: args[2].toUInt32(),
          }
        : null;
      this.traceGetCpInfo = focusedGetCpInfoObservation
        ? { codePage: args[0], output: args[1] }
        : null;
      this.traceGetSystemInfo = focusedGetSystemInfoObservation
        ? { output: args[0] }
        : null;
      this.traceGlobalMemoryStatus = focusedGlobalMemoryStatusObservation
        ? { output: args[0] }
        : null;
      const recordsSystemDefaultLangIdConsumer =
        selectedObservations.has("system-default-lang-id") &&
        name === "GetSystemDefaultLangID" &&
        isMainModuleCaller &&
        !systemDefaultLangIdConsumerRecorded;
      if (recordsSystemDefaultLangIdConsumer) {
        systemDefaultLangIdConsumerRecorded = true;
      }
      this.traceSystemDefaultLangId =
        selectedObservations.has("system-default-lang-id") &&
        name === "GetSystemDefaultLangID"
          ? {
              immediateConsumer: recordsSystemDefaultLangIdConsumer
                ? {
                    apiContinuation: snapshotInstructionsFollowingDirectJump(
                      this.returnAddress,
                      24,
                    ),
                    frameCallerContinuations:
                      snapshotX86FrameCallerContinuations(this.context, 0, 12),
                  }
                : null,
            }
          : null;
      const recordsDriveTypeConsumer =
        selectedObservations.has("drive-type") &&
        name === "GetDriveTypeA" &&
        isMainModuleCaller &&
        !driveTypeConsumerRecorded;
      if (recordsDriveTypeConsumer) {
        driveTypeConsumerRecorded = true;
      }
      this.traceDriveType =
        selectedObservations.has("drive-type") && name === "GetDriveTypeA"
          ? {
              rootPathBefore: snapshotNulTerminatedString(args[0], "ansi"),
              immediateConsumer: recordsDriveTypeConsumer
                ? {
                    apiContinuation: snapshotInstructionsFollowingDirectJump(
                      this.returnAddress,
                      32,
                    ),
                    frameCallerContinuations:
                      snapshotX86FrameCallerContinuations(this.context, 4, 16),
                  }
                : null,
            }
          : null;
      this.traceDriveTypeMainModuleCaller =
        this.traceDriveType !== null && isMainModuleCaller;
      const recordsEnumDisplayMonitorsConsumer =
        focusedEnumDisplayMonitorsObservation &&
        isMainModuleCaller &&
        !enumDisplayMonitorsConsumerRecorded;
      if (recordsEnumDisplayMonitorsConsumer) {
        enumDisplayMonitorsConsumerRecorded = true;
      }
      this.traceEnumDisplayMonitors = focusedEnumDisplayMonitorsObservation
        ? {
            hdc: args[0].toString(),
            clipRect: args[1].toString(),
            clipRectBefore: args[1].isNull() ? null : snapshotRect(args[1]),
            callback: args[2].toString(),
            callbackInstructions: args[2].isNull()
              ? null
              : snapshotInstructions(args[2], 24),
            data: args[3].toString(),
            immediateConsumer: recordsEnumDisplayMonitorsConsumer
              ? {
                  apiContinuation: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    24,
                  ),
                  frameCallerContinuations: snapshotX86FrameCallerContinuations(
                    this.context,
                    4,
                    12,
                  ),
                }
              : null,
            callbacks: [],
          }
        : null;
      if (this.traceEnumDisplayMonitors !== null) {
        attachEnumDisplayMonitorCallback(args[2]);
      }
      const recordsSwapMouseButtonConsumer =
        focusedSwapMouseButtonObservation &&
        isMainModuleCaller &&
        !swapMouseButtonConsumerRecorded;
      if (recordsSwapMouseButtonConsumer) {
        swapMouseButtonConsumerRecorded = true;
      }
      this.traceSwapMouseButton = focusedSwapMouseButtonObservation
        ? {
            swap: args[0].toUInt32(),
            stateBefore:
              swapMouseButtonGetSystemMetrics === null
                ? null
                : swapMouseButtonGetSystemMetrics(23) !== 0,
            immediateConsumer: recordsSwapMouseButtonConsumer
              ? {
                  apiContinuation: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    24,
                  ),
                  frameCallerContinuations: snapshotX86FrameCallerContinuations(
                    this.context,
                    4,
                    16,
                  ),
                }
              : null,
          }
        : null;
      this.traceSwapMouseButtonMainModuleCaller =
        this.traceSwapMouseButton !== null && isMainModuleCaller;
      this.traceDragAcceptFiles = tracesDragAcceptFiles
        ? {
            window: args[0].toString(),
            accept: args[1].toUInt32(),
            lastErrorBefore: Number(this.lastError) >>> 0,
            immediateConsumer: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              24,
            ),
          }
        : null;
      const recordsTimeGetDevCapsConsumer =
        focusedTimeGetDevCapsObservation &&
        isMainModuleCaller &&
        !timeGetDevCapsConsumerRecorded;
      if (recordsTimeGetDevCapsConsumer) {
        timeGetDevCapsConsumerRecorded = true;
      }
      this.traceTimeGetDevCaps = focusedTimeGetDevCapsObservation
        ? {
            output: args[0],
            outputAddress: args[0].toString(),
            size: args[1].toUInt32(),
            outputBefore: snapshotTimeCaps(args[0]),
            immediateConsumer: recordsTimeGetDevCapsConsumer
              ? {
                  apiContinuation: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    48,
                  ),
                  frameCallerContinuations: snapshotX86FrameCallerContinuations(
                    this.context,
                    8,
                    16,
                  ),
                }
              : null,
          }
        : null;
      this.traceTimeGetDevCapsMainModuleCaller =
        this.traceTimeGetDevCaps !== null && isMainModuleCaller;
      this.traceTimePeriod = focusedTimePeriodObservation
        ? {
            api: name,
            periodMilliseconds: args[0].toUInt32(),
            immediateConsumer: isMainModuleCaller
              ? {
                  apiContinuation: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    24,
                  ),
                  frameCallerContinuations: snapshotX86FrameCallerContinuations(
                    this.context,
                    4,
                    12,
                  ),
                }
              : null,
          }
        : null;
      this.traceTimePeriodMainModuleCaller =
        this.traceTimePeriod !== null && isMainModuleCaller;
      const tracesComInitialization =
        selectedObservations.has("com-initialization") &&
        (name === "CoInitialize" ||
          name === "CoInitializeEx" ||
          name === "CoUninitialize");
      const recordsComInitializationConsumer =
        tracesComInitialization &&
        !comInitializationConsumerRecorded &&
        name === "CoInitialize" &&
        isMainModuleCaller;
      if (recordsComInitializationConsumer) {
        comInitializationConsumerRecorded = true;
      }
      this.traceComInitialization = tracesComInitialization
        ? {
            api: name,
            reserved:
              name === "CoInitialize" || name === "CoInitializeEx"
                ? args[0].toString()
                : null,
            concurrency: name === "CoInitializeEx" ? args[1].toUInt32() : null,
            immediateConsumer: recordsComInitializationConsumer
              ? {
                  apiContinuation: snapshotInstructionsFollowingDirectJump(
                    this.returnAddress,
                    24,
                  ),
                  frameCallerContinuations: snapshotX86FrameCallerContinuations(
                    this.context,
                    name === "CoInitializeEx"
                      ? 8
                      : name === "CoInitialize"
                        ? 4
                        : 0,
                    12,
                  ),
                }
              : null,
          }
        : null;
      const observation =
        this.traceDinput8Factory !== null
          ? {
              lastErrorBefore: this.traceDinput8Factory.lastErrorBefore,
              directInput8Create: this.traceDinput8Factory,
            }
          : this.traceDsoundFactory !== null
            ? {
                lastErrorBefore: this.traceDsoundFactory.lastErrorBefore,
                directSoundCreate8: this.traceDsoundFactory,
              }
            : this.traceHeapAllocation !== null
              ? {
                  lastErrorBefore: Number(this.lastError) >>> 0,
                  heapAllocation: this.traceHeapAllocation,
                }
              : this.traceHeapFree !== null
                ? {
                    lastErrorBefore: Number(this.lastError) >>> 0,
                    heapFree: this.traceHeapFree,
                  }
                : this.traceHeapSize !== null
                  ? {
                      lastErrorBefore: Number(this.lastError) >>> 0,
                      heapSize: this.traceHeapSize,
                    }
                  : name === "SetLastError" &&
                      selectedObservations.has("set-last-error")
                    ? snapshotSetLastErrorCall(
                        caller,
                        this.returnAddress,
                        this.lastError,
                        this.context.eax,
                      )
                    : name === "FlsGetValue" &&
                        selectedObservations.has("fls-lifecycle")
                      ? snapshotFlsGetValueCall(
                          caller,
                          this.returnAddress,
                          this.lastError,
                        )
                      : name === "TlsGetValue" &&
                          selectedObservations.has("tls-lifecycle")
                        ? snapshotTlsGetValueCall(
                            caller,
                            this.returnAddress,
                            this.lastError,
                          )
                        : name === "GetLastError" &&
                            selectedObservations.has("get-last-error")
                          ? {
                              lastErrorBefore: Number(this.lastError) >>> 0,
                              immediateConsumer:
                                caller.module.toLowerCase() === "bgi.exe"
                                  ? snapshotInstructions(this.returnAddress, 4)
                                  : null,
                            }
                          : name === "GetSystemMenu" &&
                              selectedObservations.has("get-system-menu")
                            ? {
                                lastErrorBefore: Number(this.lastError) >>> 0,
                                getSystemMenu: {
                                  window: args[0].toString(),
                                  revert: args[1].toUInt32(),
                                  immediateConsumer: {
                                    apiContinuation:
                                      snapshotInstructionsFollowingDirectJump(
                                        this.returnAddress,
                                        24,
                                      ),
                                    frameCallerContinuations:
                                      snapshotX86FrameCallerContinuations(
                                        this.context,
                                        8,
                                        12,
                                      ),
                                  },
                                },
                              }
                            : name === "GetACP" &&
                                selectedObservations.has("get-acp")
                              ? snapshotGetAcpCall(
                                  caller,
                                  this.returnAddress,
                                  this.lastError,
                                  this.context,
                                )
                              : this.traceSystemDefaultLangId !== null
                                ? {
                                    lastErrorBefore:
                                      Number(this.lastError) >>> 0,
                                    systemDefaultLangId:
                                      this.traceSystemDefaultLangId,
                                  }
                                : this.traceDriveType !== null
                                  ? {
                                      lastErrorBefore:
                                        Number(this.lastError) >>> 0,
                                      driveType: this.traceDriveType,
                                    }
                                  : this.traceGetCpInfo !== null
                                    ? snapshotGetCpInfoCall(
                                        caller,
                                        this.returnAddress,
                                        this.lastError,
                                        this.context,
                                        this.traceGetCpInfo.codePage,
                                        this.traceGetCpInfo.output,
                                      )
                                    : this.traceGetSystemInfo !== null
                                      ? snapshotGetSystemInfoCall(
                                          caller,
                                          this.returnAddress,
                                          this.lastError,
                                          this.context,
                                          this.traceGetSystemInfo.output,
                                        )
                                      : this.traceGlobalMemoryStatus !== null
                                        ? snapshotGlobalMemoryStatusCall(
                                            caller,
                                            this.returnAddress,
                                            this.lastError,
                                            this.context,
                                            this.traceGlobalMemoryStatus.output,
                                          )
                                        : this.traceEnumDisplayMonitors !== null
                                          ? {
                                              lastErrorBefore:
                                                Number(this.lastError) >>> 0,
                                              enumDisplayMonitors: {
                                                hdc: this
                                                  .traceEnumDisplayMonitors.hdc,
                                                clipRect:
                                                  this.traceEnumDisplayMonitors
                                                    .clipRect,
                                                clipRectBefore:
                                                  this.traceEnumDisplayMonitors
                                                    .clipRectBefore,
                                                callback:
                                                  this.traceEnumDisplayMonitors
                                                    .callback,
                                                callbackInstructions:
                                                  this.traceEnumDisplayMonitors
                                                    .callbackInstructions,
                                                data: this
                                                  .traceEnumDisplayMonitors
                                                  .data,
                                                immediateConsumer:
                                                  this.traceEnumDisplayMonitors
                                                    .immediateConsumer,
                                              },
                                            }
                                          : this.traceSwapMouseButton !== null
                                            ? {
                                                lastErrorBefore:
                                                  Number(this.lastError) >>> 0,
                                                swapMouseButton:
                                                  this.traceSwapMouseButton,
                                              }
                                            : this.traceDragAcceptFiles !== null
                                              ? {
                                                  lastErrorBefore:
                                                    this.traceDragAcceptFiles
                                                      .lastErrorBefore,
                                                  dragAcceptFiles:
                                                    this.traceDragAcceptFiles,
                                                }
                                              : this.traceTimeGetDevCaps !==
                                                  null
                                                ? {
                                                    lastErrorBefore:
                                                      Number(this.lastError) >>>
                                                      0,
                                                    timeGetDevCaps: {
                                                      outputAddress:
                                                        this.traceTimeGetDevCaps
                                                          .outputAddress,
                                                      size: this
                                                        .traceTimeGetDevCaps
                                                        .size,
                                                      outputBefore:
                                                        this.traceTimeGetDevCaps
                                                          .outputBefore,
                                                      immediateConsumer:
                                                        this.traceTimeGetDevCaps
                                                          .immediateConsumer,
                                                    },
                                                  }
                                                : this.traceTimePeriod !== null
                                                  ? {
                                                      lastErrorBefore:
                                                        Number(
                                                          this.lastError,
                                                        ) >>> 0,
                                                      timePeriod:
                                                        this.traceTimePeriod,
                                                    }
                                                  : this
                                                        .traceComInitialization !==
                                                      null
                                                    ? {
                                                        lastErrorBefore:
                                                          Number(
                                                            this.lastError,
                                                          ) >>> 0,
                                                        comInitialization:
                                                          this
                                                            .traceComInitialization,
                                                      }
                                                    : focusedProcessorFeatureObservation
                                                      ? snapshotProcessorFeatureCall(
                                                          caller,
                                                          this.returnAddress,
                                                          this.lastError,
                                                          this.context,
                                                          args[0],
                                                        )
                                                      : focusedSetThreadAffinityMaskObservation
                                                        ? snapshotSetThreadAffinityMaskCall(
                                                            caller,
                                                            this.returnAddress,
                                                            this.lastError,
                                                            this.context,
                                                            args[0],
                                                            args[1],
                                                          )
                                                        : focusedUnhandledExceptionFilterObservation
                                                          ? snapshotUnhandledExceptionFilterCall(
                                                              caller,
                                                              this
                                                                .returnAddress,
                                                              this.lastError,
                                                              this.context,
                                                              args[0],
                                                            )
                                                          : this
                                                                .traceMultiByteConversion !==
                                                              null
                                                            ? snapshotMultiByteToWideCharCall(
                                                                caller,
                                                                this
                                                                  .returnAddress,
                                                                this.lastError,
                                                                this.context,
                                                                this
                                                                  .traceMultiByteConversion
                                                                  .codePage,
                                                                this
                                                                  .traceMultiByteConversion
                                                                  .flags,
                                                                this
                                                                  .traceMultiByteConversion
                                                                  .source,
                                                                this
                                                                  .traceMultiByteConversion
                                                                  .sourceBytes,
                                                                this
                                                                  .traceMultiByteConversion
                                                                  .destination,
                                                                this
                                                                  .traceMultiByteConversion
                                                                  .destinationUnits,
                                                              )
                                                            : this
                                                                  .traceSetCurrentDirectoryA !==
                                                                null
                                                              ? snapshotSetCurrentDirectoryACall(
                                                                  caller,
                                                                  this
                                                                    .returnAddress,
                                                                  this
                                                                    .lastError,
                                                                  this.context,
                                                                  this
                                                                    .traceSetCurrentDirectoryA
                                                                    .path,
                                                                  this
                                                                    .traceSetCurrentDirectoryA
                                                                    .currentDirectoryBefore,
                                                                )
                                                              : this
                                                                    .traceCreateMutexA !==
                                                                  null
                                                                ? snapshotCreateMutexACall(
                                                                    caller,
                                                                    this
                                                                      .returnAddress,
                                                                    this
                                                                      .lastError,
                                                                    this
                                                                      .context,
                                                                    this
                                                                      .traceCreateMutexA
                                                                      .securityAttributes,
                                                                    this
                                                                      .traceCreateMutexA
                                                                      .initialOwner,
                                                                    this
                                                                      .traceCreateMutexA
                                                                      .name,
                                                                  )
                                                                : this
                                                                      .traceMutexLifecycle !==
                                                                    null
                                                                  ? {
                                                                      lastErrorBefore:
                                                                        Number(
                                                                          this
                                                                            .lastError,
                                                                        ) >>> 0,
                                                                      mutexLifecycle:
                                                                        this
                                                                          .traceMutexLifecycle,
                                                                    }
                                                                  : this
                                                                        .traceGetStringTypeW !==
                                                                      null
                                                                    ? snapshotGetStringTypeWCall(
                                                                        caller,
                                                                        this
                                                                          .returnAddress,
                                                                        this
                                                                          .lastError,
                                                                        this
                                                                          .context,
                                                                        this
                                                                          .traceGetStringTypeW
                                                                          .infoType,
                                                                        this
                                                                          .traceGetStringTypeW
                                                                          .source,
                                                                        this
                                                                          .traceGetStringTypeW
                                                                          .sourceUnits,
                                                                        this
                                                                          .traceGetStringTypeW
                                                                          .output,
                                                                      )
                                                                    : this
                                                                          .traceLCMapStringW !==
                                                                        null
                                                                      ? snapshotLCMapStringWCall(
                                                                          caller,
                                                                          this
                                                                            .returnAddress,
                                                                          this
                                                                            .lastError,
                                                                          this
                                                                            .context,
                                                                          this
                                                                            .traceLCMapStringW
                                                                            .locale,
                                                                          this
                                                                            .traceLCMapStringW
                                                                            .flags,
                                                                          this
                                                                            .traceLCMapStringW
                                                                            .source,
                                                                          this
                                                                            .traceLCMapStringW
                                                                            .sourceUnits,
                                                                          this
                                                                            .traceLCMapStringW
                                                                            .destination,
                                                                          this
                                                                            .traceLCMapStringW
                                                                            .destinationUnits,
                                                                        )
                                                                      : this
                                                                            .traceInterlockedDecrement !==
                                                                          null
                                                                        ? snapshotInterlockedDecrementCall(
                                                                            caller,
                                                                            this
                                                                              .returnAddress,
                                                                            this
                                                                              .lastError,
                                                                            this
                                                                              .context,
                                                                            this
                                                                              .traceInterlockedDecrement
                                                                              .target,
                                                                          )
                                                                        : this
                                                                              .traceGetModuleFileNameA !==
                                                                            null
                                                                          ? snapshotGetModuleFileNameACall(
                                                                              caller,
                                                                              this
                                                                                .returnAddress,
                                                                              this
                                                                                .lastError,
                                                                              this
                                                                                .context,
                                                                              this
                                                                                .traceGetModuleFileNameA
                                                                                .moduleHandle,
                                                                              this
                                                                                .traceGetModuleFileNameA
                                                                                .output,
                                                                              this
                                                                                .traceGetModuleFileNameA
                                                                                .capacity,
                                                                            )
                                                                          : name ===
                                                                              "GetStartupInfoW"
                                                                            ? {
                                                                                startupInfoBefore:
                                                                                  snapshotStartupInfoW(
                                                                                    args[0],
                                                                                  ),
                                                                              }
                                                                            : selectedObservations.has(
                                                                                  "critical-section",
                                                                                ) &&
                                                                                name.includes(
                                                                                  "CriticalSection",
                                                                                )
                                                                              ? {
                                                                                  lastErrorBefore:
                                                                                    Number(
                                                                                      this
                                                                                        .lastError,
                                                                                    ) >>>
                                                                                    0,
                                                                                  criticalSectionBefore:
                                                                                    snapshotCriticalSection(
                                                                                      args[0],
                                                                                    ),
                                                                                  initializeCriticalSectionConsumer:
                                                                                    recordsInitializeCriticalSectionConsumer
                                                                                      ? {
                                                                                          apiContinuation:
                                                                                            snapshotInstructions(
                                                                                              this
                                                                                                .returnAddress,
                                                                                              16,
                                                                                            ),
                                                                                          frameCallerContinuations:
                                                                                            snapshotX86FrameCallerContinuations(
                                                                                              this
                                                                                                .context,
                                                                                              4,
                                                                                              12,
                                                                                            ),
                                                                                        }
                                                                                      : null,
                                                                                }
                                                                              : name ===
                                                                                    "InterlockedIncrement" &&
                                                                                  selectedObservations.has(
                                                                                    "interlocked-increment",
                                                                                  )
                                                                                ? {
                                                                                    lastErrorBefore:
                                                                                      Number(
                                                                                        this
                                                                                          .lastError,
                                                                                      ) >>>
                                                                                      0,
                                                                                    interlockedTargetBefore:
                                                                                      snapshotLong(
                                                                                        args[0],
                                                                                      ),
                                                                                  }
                                                                                : name ===
                                                                                      "GetStdHandle" &&
                                                                                    selectedObservations.has(
                                                                                      "standard-handle",
                                                                                    )
                                                                                  ? {
                                                                                      lastErrorBefore:
                                                                                        Number(
                                                                                          this
                                                                                            .lastError,
                                                                                        ) >>>
                                                                                        0,
                                                                                    }
                                                                                  : name ===
                                                                                        "GetFileType" &&
                                                                                      selectedObservations.has(
                                                                                        "file-type",
                                                                                      )
                                                                                    ? {
                                                                                        lastErrorBefore:
                                                                                          Number(
                                                                                            this
                                                                                              .lastError,
                                                                                          ) >>>
                                                                                          0,
                                                                                      }
                                                                                    : name ===
                                                                                          "SetHandleCount" &&
                                                                                        selectedObservations.has(
                                                                                          "set-handle-count",
                                                                                        )
                                                                                      ? {
                                                                                          lastErrorBefore:
                                                                                            Number(
                                                                                              this
                                                                                                .lastError,
                                                                                            ) >>>
                                                                                            0,
                                                                                        }
                                                                                      : (name ===
                                                                                            "GetCommandLineA" ||
                                                                                            name ===
                                                                                              "GetCommandLineW") &&
                                                                                          selectedObservations.has(
                                                                                            "command-line-a",
                                                                                          )
                                                                                        ? {
                                                                                            lastErrorBefore:
                                                                                              Number(
                                                                                                this
                                                                                                  .lastError,
                                                                                              ) >>>
                                                                                              0,
                                                                                          }
                                                                                        : (name ===
                                                                                              "GetEnvironmentStringsW" ||
                                                                                              name ===
                                                                                                "FreeEnvironmentStringsW") &&
                                                                                            selectedObservations.has(
                                                                                              "environment-strings-w",
                                                                                            )
                                                                                          ? {
                                                                                              lastErrorBefore:
                                                                                                Number(
                                                                                                  this
                                                                                                    .lastError,
                                                                                                ) >>>
                                                                                                0,
                                                                                              ...(name ===
                                                                                              "FreeEnvironmentStringsW"
                                                                                                ? {
                                                                                                    environmentBlockBeforeFree:
                                                                                                      snapshotUtf16EnvironmentBlock(
                                                                                                        args[0],
                                                                                                        readProcessParametersEnvironmentPointer(),
                                                                                                      ),
                                                                                                  }
                                                                                                : {}),
                                                                                            }
                                                                                          : this
                                                                                                .traceWideCharConversion !==
                                                                                              null
                                                                                            ? {
                                                                                                lastErrorBefore:
                                                                                                  Number(
                                                                                                    this
                                                                                                      .lastError,
                                                                                                  ) >>>
                                                                                                  0,
                                                                                                codePage:
                                                                                                  args[0].toUInt32(),
                                                                                                flags:
                                                                                                  args[1].toUInt32(),
                                                                                                source:
                                                                                                  snapshotWideCharSource(
                                                                                                    this
                                                                                                      .traceWideCharConversion
                                                                                                      .source,
                                                                                                    this
                                                                                                      .traceWideCharConversion
                                                                                                      .sourceUnits,
                                                                                                  ),
                                                                                                destinationBefore:
                                                                                                  snapshotByteBuffer(
                                                                                                    this
                                                                                                      .traceWideCharConversion
                                                                                                      .destination,
                                                                                                    Math.max(
                                                                                                      0,
                                                                                                      this
                                                                                                        .traceWideCharConversion
                                                                                                        .destinationBytes,
                                                                                                    ),
                                                                                                  ),
                                                                                                defaultCharPointer:
                                                                                                  args[6].toString(),
                                                                                                usedDefaultCharBefore:
                                                                                                  snapshotOptionalBool(
                                                                                                    this
                                                                                                      .traceWideCharConversion
                                                                                                      .usedDefaultChar,
                                                                                                  ),
                                                                                              }
                                                                                            : name.startsWith(
                                                                                                  "Heap",
                                                                                                ) ||
                                                                                                name.startsWith(
                                                                                                  "GetModuleHandle",
                                                                                                ) ||
                                                                                                (name.startsWith(
                                                                                                  "Tls",
                                                                                                ) &&
                                                                                                  selectedObservations.has(
                                                                                                    "tls-lifecycle",
                                                                                                  )) ||
                                                                                                (name.startsWith(
                                                                                                  "Fls",
                                                                                                ) &&
                                                                                                  selectedObservations.has(
                                                                                                    "fls-lifecycle",
                                                                                                  )) ||
                                                                                                ((name ===
                                                                                                  "EncodePointer" ||
                                                                                                  name ===
                                                                                                    "DecodePointer") &&
                                                                                                  selectedObservations.has(
                                                                                                    "pointer-encoding",
                                                                                                  )) ||
                                                                                                (name ===
                                                                                                  "GetProcAddress" &&
                                                                                                  selectedObservations.has(
                                                                                                    "dynamic-export",
                                                                                                  ))
                                                                                              ? {
                                                                                                  lastErrorBefore:
                                                                                                    Number(
                                                                                                      this
                                                                                                        .lastError,
                                                                                                    ) >>>
                                                                                                    0,
                                                                                                }
                                                                                              : {};
      this.traceStartupInfoPointer =
        name === "GetStartupInfoW" ? args[0] : null;
      this.traceCriticalSectionPointer =
        selectedObservations.has("critical-section") &&
        name.includes("CriticalSection")
          ? args[0]
          : null;
      this.traceInterlockedPointer =
        name === "InterlockedIncrement" &&
        selectedObservations.has("interlocked-increment")
          ? args[0]
          : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: arguments_,
          ...observation,
        },
        this.returnAddress,
      );
      if (this.traceEnumDisplayMonitors !== null) {
        activeEnumDisplayMonitorsByThread.set(
          this.traceFrame.threadId,
          this.traceEnumDisplayMonitors,
        );
      }
    },
    onLeave(result) {
      if (this.trackHeapSizeAllocation != null && !result.isNull()) {
        observedMainModuleHeapAllocations.set(result.toString().toLowerCase(), {
          callId: null,
          requestedBytes: this.trackHeapSizeAllocation.requestedBytes,
        });
      }
      if (this.trackHeapSizeFree != null && result.toUInt32() !== 0) {
        observedMainModuleHeapAllocations.delete(this.trackHeapSizeFree);
      }
      if (this.traceFrame === null) {
        return;
      }
      const lastError = this.lastError;
      if (this.traceCriticalSectionOracle) {
        criticalSectionOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceEnvironmentStringsOracle) {
        environmentStringsOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceWideCharOracle) {
        wideCharOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceMultiByteOracle) {
        multiByteOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceSetCurrentDirectoryOracle) {
        setCurrentDirectoryOracleResult = result.toUInt32();
        setCurrentDirectoryOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceCreateMutexOracle && name === "CreateMutexA") {
        createMutexOracleResult = result.toString();
        createMutexOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceGetStringTypeOracle) {
        getStringTypeOracleResult = result.toUInt32();
        getStringTypeOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceLCMapStringOracle) {
        lcMapStringOracleResult = signed32(result);
        lcMapStringOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceInterlockedDecrementOracle) {
        interlockedDecrementOracleResult = signed32(result);
        interlockedDecrementOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceGetModuleFileNameAOracle) {
        getModuleFileNameAOracleResult = result.toUInt32();
        getModuleFileNameAOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceGetLastErrorOracle) {
        getLastErrorOracleResult = result.toUInt32();
        getLastErrorOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceSetLastErrorOracle) {
        setLastErrorOracleEaxAfter = result.toUInt32();
        setLastErrorOracleLastErrorAfter = Number(lastError) >>> 0;
      }
      if (this.traceGetAcpOracle) {
        getAcpOracleResult = result.toUInt32();
        getAcpOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceSystemDefaultLangIdOracle) {
        systemDefaultLangIdOracleResult = result.toUInt32();
        systemDefaultLangIdOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceDriveTypeOracle) {
        driveTypeOracleResult = result.toUInt32();
        driveTypeOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceGetCpInfoOracle) {
        getCpInfoOracleResult = result.toUInt32();
        getCpInfoOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceProcessorFeatureOracle) {
        processorFeatureOracleResult = result.toUInt32();
        processorFeatureOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceSetThreadAffinityMaskOracle) {
        setThreadAffinityMaskOracleResult = result.toUInt32();
        setThreadAffinityMaskOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceGetSystemInfoOracle) {
        getSystemInfoOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceGlobalMemoryStatusOracle) {
        globalMemoryStatusOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceEnumDisplayMonitorsOracle) {
        enumDisplayMonitorsOracleResult = result.toUInt32();
        enumDisplayMonitorsOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceTimeGetDevCapsOracle) {
        timeGetDevCapsOracleCall.hookResult = result.toUInt32();
        timeGetDevCapsOracleCall.hookLastErrorAfter = Number(lastError) >>> 0;
      }
      if (this.traceTimePeriodOracle) {
        timePeriodOracleCall.hookResult = result.toUInt32();
        timePeriodOracleCall.hookLastErrorAfter = Number(lastError) >>> 0;
      }
      if (this.traceComInitializationOracle) {
        comInitializationOracleCall.hookResult = result.toUInt32();
        comInitializationOracleCall.hookLastErrorAfter =
          Number(lastError) >>> 0;
      }
      if (
        this.traceThreadAffinityPrimaryCall === 1 &&
        result.toUInt32() !== 0
      ) {
        setThreadAffinityCalibration = {
          active: true,
          startedAtMs: Date.now(),
          initialRequestedMask: this.traceThreadAffinityRequestedMask,
          initialPreviousMask: result.toUInt32(),
          tickCountCalls: 0,
          valueChanges: 0,
          regressions: 0,
          firstValue: null,
          lastValue: null,
          firstSamples: [],
          lastSamples: [],
        };
      } else if (this.traceThreadAffinityPrimaryCall === 2) {
        const calibration = setThreadAffinityCalibration;
        if (calibration !== null) {
          calibration.active = false;
          emit("set-thread-affinity-mask-calibration", {
            profile: "bgi-get-tick-count-affinity-calibration-v1",
            initialRequestedMask: calibration.initialRequestedMask,
            initialPreviousMask: calibration.initialPreviousMask,
            restoreRequestedMask: this.traceThreadAffinityRequestedMask,
            restorePreviousMask: result.toUInt32(),
            tickCountCalls: calibration.tickCountCalls,
            valueChanges: calibration.valueChanges,
            regressions: calibration.regressions,
            firstValue: calibration.firstValue,
            lastValue: calibration.lastValue,
            elapsedModulo32:
              calibration.firstValue === null || calibration.lastValue === null
                ? null
                : (calibration.lastValue - calibration.firstValue) >>> 0,
            observedDurationMs: Math.max(
              0,
              Date.now() - calibration.startedAtMs,
            ),
            firstSamples: calibration.firstSamples,
            lastSamples: calibration.lastSamples,
          });
          setThreadAffinityCalibration = null;
        }
      }
      if (this.traceUnhandledExceptionFilterOracle) {
        unhandledExceptionFilterOracleResult = result.toString();
        unhandledExceptionFilterOracleLastError = Number(lastError) >>> 0;
      }
      if (this.traceHeapSizeOracle) {
        heapSizeOracleResult = result.toUInt32();
        heapSizeOracleLastError = Number(lastError) >>> 0;
      }
      const wideCharResult =
        this.traceWideCharConversion === null ? null : signed32(result);
      const multiByteResult =
        this.traceMultiByteConversion === null ? null : signed32(result);
      const lcMapStringResult =
        this.traceLCMapStringW === null ? null : signed32(result);
      const setCurrentDirectoryAfter =
        this.traceSetCurrentDirectoryA === null
          ? null
          : (() => {
              const currentDirectory = snapshotCurrentDirectoryA(module);
              this.lastError = lastError;
              return currentDirectory;
            })();
      const directInput8Create =
        this.traceDinput8Factory === null
          ? null
          : {
              ...this.traceDinput8Factory,
              output: {
                ...this.traceDinput8Factory.output,
                after: snapshotPointerSlot(this.traceDinput8FactoryOutput),
              },
            };
      const directSoundCreate8 =
        this.traceDsoundFactory === null
          ? null
          : {
              ...this.traceDsoundFactory,
              output: {
                ...this.traceDsoundFactory.output,
                after: snapshotPointerSlot(this.traceDsoundFactoryOutput),
              },
            };
      const observation = {
        ...(directInput8Create === null ? {} : { directInput8Create }),
        ...(directSoundCreate8 === null ? {} : { directSoundCreate8 }),
        ...(this.traceHeapAllocation === null
          ? {}
          : {
              heapAllocation: {
                heap: this.traceHeapAllocation.heap,
                flags: this.traceHeapAllocation.flags,
                requestedBytes: this.traceHeapAllocation.requestedBytes,
                bufferAfterReturn: snapshotByteBuffer(
                  result,
                  this.traceHeapAllocation.requestedBytes,
                ),
              },
            }),
        ...(this.traceHeapFree === null
          ? {}
          : {
              heapFree: {
                heap: this.traceHeapFree.heap,
                flags: this.traceHeapFree.flags,
                pointer: this.traceHeapFree.pointer,
                relatedAllocationCallId:
                  this.traceHeapFree.relatedAllocationCallId,
                requestedBytes: this.traceHeapFree.requestedBytes,
                largeHeapAccess: this.traceHeapFree.largeHeapAccess,
              },
            }),
        ...(this.traceHeapSize === null
          ? {}
          : {
              heapSize: {
                heap: this.traceHeapSize.heap,
                flags: this.traceHeapSize.flags,
                pointer: this.traceHeapSize.pointer,
                relatedAllocationCallId:
                  this.traceHeapSize.relatedAllocationCallId,
                requestedBytes: this.traceHeapSize.requestedBytes,
                result: result.toUInt32(),
              },
            }),
        ...(this.traceStartupInfoPointer === null
          ? {}
          : {
              startupInfoAfter: snapshotStartupInfoW(
                this.traceStartupInfoPointer,
                true,
              ),
            }),
        ...(this.traceCriticalSectionPointer === null
          ? {}
          : {
              criticalSectionAfter: snapshotCriticalSection(
                this.traceCriticalSectionPointer,
              ),
            }),
        ...(this.traceInterlockedPointer === null
          ? {}
          : {
              interlockedTargetAfter: snapshotLong(
                this.traceInterlockedPointer,
              ),
            }),
        ...(this.traceInterlockedDecrement === null
          ? {}
          : {
              interlockedDecrement: {
                result: signed32(result),
                targetAfter: snapshotLong(
                  this.traceInterlockedDecrement.target,
                ),
              },
            }),
        ...(this.traceGetModuleFileNameA === null
          ? {}
          : {
              getModuleFileNameA: {
                result: result.toUInt32(),
                outputAfter: snapshotAnsiOutputBuffer(
                  this.traceGetModuleFileNameA.output,
                  this.traceGetModuleFileNameA.capacity,
                ),
              },
            }),
        ...(name === "SetLastError" &&
        selectedObservations.has("set-last-error")
          ? { eaxAfter: result.toUInt32() }
          : {}),
        ...((name === "GetCommandLineA" || name === "GetCommandLineW") &&
        selectedObservations.has("command-line-a")
          ? {
              commandLine: snapshotNulTerminatedString(
                result,
                name === "GetCommandLineW" ? "utf16" : "ansi",
              ),
            }
          : {}),
        ...(name === "GetEnvironmentStringsW" &&
        selectedObservations.has("environment-strings-w")
          ? {
              environmentBlock: snapshotUtf16EnvironmentBlock(
                result,
                readProcessParametersEnvironmentPointer(),
              ),
            }
          : {}),
        ...(this.traceWideCharConversion === null
          ? {}
          : {
              wideCharConversion: {
                result: wideCharResult,
                destinationAfter: snapshotByteBuffer(
                  this.traceWideCharConversion.destination,
                  wideCharResult > 0
                    ? wideCharResult
                    : Math.max(
                        0,
                        this.traceWideCharConversion.destinationBytes,
                      ),
                ),
                usedDefaultCharAfter: snapshotOptionalBool(
                  this.traceWideCharConversion.usedDefaultChar,
                ),
              },
            }),
        ...(this.traceMultiByteConversion === null
          ? {}
          : {
              multiByteToWideChar: {
                result: multiByteResult,
                destinationAfter: snapshotWideCharBuffer(
                  this.traceMultiByteConversion.destination,
                  multiByteResult > 0
                    ? multiByteResult
                    : Math.max(
                        0,
                        this.traceMultiByteConversion.destinationUnits,
                      ),
                ),
              },
            }),
        ...(this.traceSetCurrentDirectoryA === null
          ? {}
          : {
              setCurrentDirectoryA: {
                result: result.toUInt32(),
                pathAfter: snapshotMultiByteSource(
                  this.traceSetCurrentDirectoryA.path,
                  -1,
                ),
                currentDirectoryAfter: setCurrentDirectoryAfter,
              },
            }),
        ...(this.traceSourceFileCreate === null
          ? {}
          : {
              sourceFileCreate: {
                ...this.traceSourceFileCreate,
                handle: result.toString().toLowerCase(),
                succeeded: result.toUInt32() !== 0xffffffff,
              },
            }),
        ...(this.traceSourceFileOperation === null
          ? {}
          : {
              sourceFileOperation: {
                api: this.traceSourceFileOperation.api,
                handle: this.traceSourceFileOperation.handle,
                result: result.toString(),
                ...(this.traceSourceFileOperation.api === "ReadFile"
                  ? (() => {
                      const bytesReadAfter = snapshotOptionalU32(
                        this.traceSourceFileOperation.bytesRead,
                      );
                      const transferredBytes = bytesReadAfter.value ?? 0;
                      return {
                        requestedBytes:
                          this.traceSourceFileOperation.requestedBytes,
                        bytesReadBefore:
                          this.traceSourceFileOperation.bytesReadBefore,
                        bytesReadAfter,
                        overlapped: this.traceSourceFileOperation.overlapped,
                        outputAfter: snapshotByteBuffer(
                          this.traceSourceFileOperation.buffer,
                          transferredBytes,
                        ),
                      };
                    })()
                  : {}),
                ...(this.traceSourceFileOperation.api === "WriteFile"
                  ? {
                      requestedBytes:
                        this.traceSourceFileOperation.requestedBytes,
                      bytesWrittenBefore:
                        this.traceSourceFileOperation.bytesWrittenBefore,
                      bytesWrittenAfter: snapshotOptionalU32(
                        this.traceSourceFileOperation.bytesWritten,
                      ),
                      overlapped: this.traceSourceFileOperation.overlapped,
                      inputBefore: this.traceSourceFileOperation.inputBefore,
                    }
                  : {}),
                ...(this.traceSourceFileOperation.api === "SetFilePointer"
                  ? {
                      distanceLow: this.traceSourceFileOperation.distanceLow,
                      distanceHighBefore:
                        this.traceSourceFileOperation.distanceHighBefore,
                      distanceHighAfter: snapshotOptionalU32(
                        this.traceSourceFileOperation.distanceHigh,
                      ),
                      moveMethod: this.traceSourceFileOperation.moveMethod,
                    }
                  : {}),
                ...(this.traceSourceFileOperation.api === "GetFileSize"
                  ? {
                      sizeHighBefore:
                        this.traceSourceFileOperation.sizeHighBefore,
                      sizeHighAfter: snapshotOptionalU32(
                        this.traceSourceFileOperation.sizeHigh,
                      ),
                    }
                  : {}),
                immediateConsumer:
                  this.traceSourceFileOperation.immediateConsumer,
              },
            }),
        ...(this.traceCreateMutexA === null
          ? {}
          : {
              createMutexA: {
                result: result.toString(),
                nameAfter: snapshotMultiByteSource(
                  this.traceCreateMutexA.name,
                  -1,
                ),
              },
            }),
        ...(this.traceMutexLifecycle === null
          ? {}
          : {
              mutexLifecycle: {
                ...this.traceMutexLifecycle,
                result: result.toString(),
              },
            }),
        ...(this.traceGetStringTypeW === null
          ? {}
          : {
              getStringTypeW: {
                result: result.toUInt32(),
                outputAfter: snapshotWordBuffer(
                  this.traceGetStringTypeW.output,
                  Math.max(0, this.traceGetStringTypeW.sourceUnits),
                ),
              },
            }),
        ...(this.traceLCMapStringW === null
          ? {}
          : {
              lcMapStringW: {
                result: lcMapStringResult,
                destinationAfter: snapshotWideCharBuffer(
                  this.traceLCMapStringW.destination,
                  lcMapStringResult > 0
                    ? lcMapStringResult
                    : Math.max(0, this.traceLCMapStringW.destinationUnits),
                ),
              },
            }),
        ...(this.traceGetCpInfo === null
          ? {}
          : {
              getCpInfo: {
                codePage: this.traceGetCpInfo.codePage.toUInt32(),
                outputAfter: snapshotBoundedRawBytes(
                  this.traceGetCpInfo.output,
                  32,
                ),
                measuredOutput: snapshotMeasuredCpInfo(
                  this.traceGetCpInfo.output,
                ),
              },
            }),
        ...(this.traceGetSystemInfo === null
          ? {}
          : {
              getSystemInfo: {
                outputAfter: snapshotSystemInfo(this.traceGetSystemInfo.output),
              },
            }),
        ...(this.traceGlobalMemoryStatus === null
          ? {}
          : {
              globalMemoryStatus: {
                outputAfter: snapshotMemoryStatus(
                  this.traceGlobalMemoryStatus.output,
                ),
              },
            }),
        ...(this.traceSystemDefaultLangId === null
          ? {}
          : {
              systemDefaultLangId: {
                ...this.traceSystemDefaultLangId,
                langId: result.toUInt32() & 0xffff,
                rawEax: result.toUInt32(),
              },
            }),
        ...(this.traceDriveType === null
          ? {}
          : {
              driveType: {
                ...this.traceDriveType,
                result: result.toUInt32(),
              },
            }),
        ...(this.traceEnumDisplayMonitors === null
          ? {}
          : {
              enumDisplayMonitors: {
                result: result.toUInt32(),
                hdc: this.traceEnumDisplayMonitors.hdc,
                clipRect: this.traceEnumDisplayMonitors.clipRect,
                clipRectAfter:
                  this.traceEnumDisplayMonitors.clipRect === ptr(0).toString()
                    ? null
                    : snapshotRect(ptr(this.traceEnumDisplayMonitors.clipRect)),
                callback: this.traceEnumDisplayMonitors.callback,
                data: this.traceEnumDisplayMonitors.data,
                callbacks: this.traceEnumDisplayMonitors.callbacks,
              },
            }),
        ...(this.traceSwapMouseButton === null
          ? {}
          : {
              swapMouseButton: {
                ...this.traceSwapMouseButton,
                result: result.toUInt32(),
                stateAfter:
                  swapMouseButtonGetSystemMetrics === null
                    ? null
                    : swapMouseButtonGetSystemMetrics(23) !== 0,
              },
            }),
        ...(this.traceDragAcceptFiles === null
          ? {}
          : {
              dragAcceptFiles: {
                ...this.traceDragAcceptFiles,
                result: null,
              },
            }),
        ...(this.traceTimeGetDevCaps === null
          ? {}
          : {
              timeGetDevCaps: {
                result: result.toUInt32(),
                outputAddress: this.traceTimeGetDevCaps.outputAddress,
                size: this.traceTimeGetDevCaps.size,
                outputAfter: snapshotTimeCaps(this.traceTimeGetDevCaps.output),
              },
            }),
        ...(this.traceTimePeriod === null
          ? {}
          : {
              timePeriod: {
                ...this.traceTimePeriod,
                result: result.toUInt32(),
              },
            }),
        ...(this.traceComInitialization === null
          ? {}
          : {
              comInitialization: {
                ...this.traceComInitialization,
                result: name === "CoUninitialize" ? null : result.toUInt32(),
                signedResult:
                  name === "CoUninitialize" ? null : signed32(result),
              },
            }),
      };
      if (this.traceEnumDisplayMonitors !== null) {
        const active = activeEnumDisplayMonitorsByThread.get(
          this.traceFrame.threadId,
        );
        if (active === this.traceEnumDisplayMonitors) {
          activeEnumDisplayMonitorsByThread.delete(this.traceFrame.threadId);
        }
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          ...observation,
        },
        lastError,
      );
      if (this.traceDragAcceptFiles !== null) {
        dragAcceptFilesCompletion = {
          callId: this.traceFrame.callId,
          caller: this.traceFrame.caller,
          window: this.traceDragAcceptFiles.window,
          accept: this.traceDragAcceptFiles.accept,
          lastErrorBefore: this.traceDragAcceptFiles.lastErrorBefore,
          lastError: Number(lastError) >>> 0,
          immediateConsumer: this.traceDragAcceptFiles.immediateConsumer,
        };
        setImmediate(maybeCompleteDragAcceptFilesObservation);
      }
      if (
        directInput8Create !== null &&
        result.toInt32() >= 0 &&
        typeof directInput8Create.output.after.value === "string" &&
        directInput8Create.output.after.value !== "0x0"
      ) {
        const rootObject = directInput8Create.output.after.value.toLowerCase();
        if (focusedDinput8Observation) {
          focusedDinput8RootObject = rootObject;
          focusedDinput8FactoryCall = {
            callId: this.traceFrame.callId,
            caller: this.traceFrame.caller,
            returnAddress: this.returnAddress.toString(),
            arguments: this.traceDinput8FactoryArguments,
            result: result.toString(),
            signedResult: result.toInt32(),
            lastErrorBefore: directInput8Create.lastErrorBefore,
            lastError: Number(lastError) >>> 0,
            directInput8Create,
          };
        }
        attachIdirectinput8a(ptr(rootObject));
        if (focusedDinput8Observation) {
          setImmediate(maybeCompleteFocusedDinput8RootObservation);
        }
      }
      if (
        directSoundCreate8 !== null &&
        result.toInt32() >= 0 &&
        typeof directSoundCreate8.output.after.value === "string" &&
        directSoundCreate8.output.after.value !== "0x0"
      ) {
        const rootObject = directSoundCreate8.output.after.value.toLowerCase();
        focusedDsoundRootObject = rootObject;
        focusedDsoundFactoryCall = {
          callId: this.traceFrame.callId,
          caller: this.traceFrame.caller,
          returnAddress: this.returnAddress.toString(),
          arguments: this.traceDsoundFactoryArguments,
          result: result.toString(),
          signedResult: result.toInt32(),
          lastErrorBefore: directSoundCreate8.lastErrorBefore,
          lastError: Number(lastError) >>> 0,
          directSoundCreate8,
        };
        attachIdirectsound8(ptr(rootObject));
        if (focusedDsoundRootObservation) {
          setImmediate(maybeCompleteFocusedDsoundRootObservation);
        } else if (focusedDsoundCreateBufferObservation) {
          setImmediate(maybeCompleteFocusedDsoundCreateBufferObservation);
        } else {
          setImmediate(
            maybeCompleteFocusedDsoundBufferLockUnlockPlayObservation,
          );
        }
      }
      if (
        this.traceSourceFileCreate !== null &&
        result.toUInt32() !== 0xffffffff
      ) {
        const handle = result.toString().toLowerCase();
        observedSourceFileHandles.set(handle, {
          profile: "main-module-source-file-read-v1",
          open: {
            callId: this.traceFrame.callId,
            caller: this.traceFrame.caller,
            returnAddress: this.returnAddress.toString().toLowerCase(),
            ...this.traceSourceFileCreate,
            handle,
            lastError: Number(lastError) >>> 0,
          },
          operations: [],
        });
      }
      if (this.traceSourceFileOperation !== null) {
        const operation = {
          callId: this.traceFrame.callId,
          caller: this.traceFrame.caller,
          returnAddress: this.returnAddress.toString().toLowerCase(),
          api: this.traceSourceFileOperation.api,
          result: result.toString(),
          lastErrorBefore: this.traceSourceFileOperation.lastErrorBefore,
          lastError: Number(lastError) >>> 0,
          immediateConsumer: this.traceSourceFileOperation.immediateConsumer,
          ...(this.traceSourceFileOperation.api === "ReadFile"
            ? (() => {
                const bytesRead = snapshotOptionalU32(
                  this.traceSourceFileOperation.bytesRead,
                );
                return {
                  requestedBytes: this.traceSourceFileOperation.requestedBytes,
                  bytesRead,
                  overlapped: this.traceSourceFileOperation.overlapped,
                  output: snapshotByteBuffer(
                    this.traceSourceFileOperation.buffer,
                    bytesRead.value ?? 0,
                  ),
                };
              })()
            : {}),
          ...(this.traceSourceFileOperation.api === "WriteFile"
            ? {
                requestedBytes: this.traceSourceFileOperation.requestedBytes,
                bytesWritten: snapshotOptionalU32(
                  this.traceSourceFileOperation.bytesWritten,
                ),
                overlapped: this.traceSourceFileOperation.overlapped,
                input: this.traceSourceFileOperation.inputBefore,
              }
            : {}),
          ...(this.traceSourceFileOperation.api === "SetFilePointer"
            ? {
                distanceLow: this.traceSourceFileOperation.distanceLow,
                distanceHigh: snapshotOptionalU32(
                  this.traceSourceFileOperation.distanceHigh,
                ),
                moveMethod: this.traceSourceFileOperation.moveMethod,
              }
            : {}),
          ...(this.traceSourceFileOperation.api === "GetFileSize"
            ? {
                sizeHigh: snapshotOptionalU32(
                  this.traceSourceFileOperation.sizeHigh,
                ),
              }
            : {}),
        };
        this.traceSourceFileOperation.record.operations.push(operation);
        if (
          !sourceFileReadCompletionRecorded &&
          this.traceSourceFileOperation.api === "CloseHandle" &&
          result.toUInt32() !== 0
        ) {
          sourceFileReadCompletionRecorded = true;
          observedSourceFileHandles.delete(
            this.traceSourceFileOperation.handle,
          );
          emit("source-file-read-complete", {
            ...this.traceSourceFileOperation.record,
            close: operation,
          });
          if (stopOnEvent === "source-file-read-complete") {
            recv("vine-trace-stop", () => {}).wait();
          }
        }
      }
      if (
        this.traceTimeGetDevCaps !== null &&
        this.traceTimeGetDevCapsMainModuleCaller &&
        timeGetDevCapsMainModuleCalls.length === 0
      ) {
        timeGetDevCapsMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          outputAddress: this.traceTimeGetDevCaps.outputAddress,
          size: this.traceTimeGetDevCaps.size,
          outputBefore: this.traceTimeGetDevCaps.outputBefore,
          outputAfter: snapshotTimeCaps(this.traceTimeGetDevCaps.output),
          result: result.toUInt32(),
          lastError: Number(lastError) >>> 0,
          immediateConsumer: this.traceTimeGetDevCaps.immediateConsumer,
        });
        setImmediate(maybeCompleteTimeGetDevCapsObservation);
      }
      if (
        this.traceTimePeriod !== null &&
        this.traceTimePeriodMainModuleCaller
      ) {
        timePeriodMainModuleCalls.push({
          callId: this.traceFrame.callId,
          api: this.traceTimePeriod.api,
          returnAddress: this.returnAddress.toString(),
          periodMilliseconds: this.traceTimePeriod.periodMilliseconds,
          result: result.toUInt32(),
          lastError: Number(lastError) >>> 0,
          immediateConsumer: this.traceTimePeriod.immediateConsumer,
        });
        setImmediate(maybeCompleteTimeGetDevCapsObservation);
      }
      if (
        this.traceSwapMouseButton !== null &&
        this.traceSwapMouseButtonMainModuleCaller &&
        !swapMouseButtonCompletionRecorded
      ) {
        swapMouseButtonMainModuleCalls.push({
          callId: this.traceFrame.callId,
          argument: this.traceSwapMouseButton.swap,
          stateBefore: this.traceSwapMouseButton.stateBefore,
          result: result.toUInt32(),
          lastError: Number(lastError) >>> 0,
          stateAfter:
            swapMouseButtonGetSystemMetrics === null
              ? null
              : swapMouseButtonGetSystemMetrics(23) !== 0,
        });
        if (swapMouseButtonMainModuleCalls.length === 2) {
          swapMouseButtonCompletionRecorded = true;
          const calls = swapMouseButtonMainModuleCalls.map((call) => ({
            ...call,
          }));
          setImmediate(() => {
            completeSwapMouseButtonObservation(calls);
          });
        }
      }
      if (this.traceDriveType !== null && this.traceDriveTypeMainModuleCaller) {
        driveTypeMainModuleCallCount += 1;
        if (
          !driveTypeScanCompletionRecorded &&
          driveTypeMainModuleCallCount === 26
        ) {
          driveTypeScanCompletionRecorded = true;
          emit("drive-type-scan-complete", {
            profile: "x86-drive-letter-scan-v1",
            callId: this.traceFrame.callId,
            callCount: driveTypeMainModuleCallCount,
            rootPath: this.traceDriveType.rootPathBefore,
            result: result.toUInt32(),
            lastError: Number(lastError) >>> 0,
          });
          if (stopOnEvent === "drive-type-scan-complete") {
            recv("vine-trace-stop", () => {}).wait();
          }
        }
      }
      if (this.traceHeapAllocation !== null && !result.isNull()) {
        const allocationRecord = {
          callId: this.traceFrame.callId,
          heap: this.traceHeapAllocation.heap.toLowerCase(),
          flags: this.traceHeapAllocation.flags,
          requestedBytes: this.traceHeapAllocation.requestedBytes,
          pointer: result.toString().toLowerCase(),
          returnAddress: this.returnAddress.toString().toLowerCase(),
          lastErrorBefore: this.traceHeapAllocation.lastErrorBefore,
          lastErrorAfter: Number(lastError) >>> 0,
        };
        observedMainModuleHeapAllocations.set(
          allocationRecord.pointer,
          allocationRecord,
        );
        allocationRecord.accessMonitor = beginLargeHeapAccessObservation(
          result,
          allocationRecord.requestedBytes,
          allocationRecord.callId,
        );
      }
      if (this.traceHeapFree !== null && result.toUInt32() !== 0) {
        observedMainModuleHeapAllocations.delete(this.traceHeapFree.pointer);
        if (
          !largeHeapAllocationCompletionRecorded &&
          this.traceHeapFree.largeHeapAccess !== null
        ) {
          largeHeapAllocationCompletionCandidate = {
            profile: "main-module-large-heap-allocation-lifetime-v1",
            allocation: this.traceHeapFree.relatedAllocation,
            free: {
              callId: this.traceFrame.callId,
              heap: this.traceHeapFree.heap,
              flags: this.traceHeapFree.flags,
              pointer: this.traceHeapFree.pointer,
              returnAddress: this.returnAddress.toString().toLowerCase(),
              result: result.toUInt32(),
              lastError: Number(lastError) >>> 0,
            },
            access: this.traceHeapFree.largeHeapAccess,
          };
          setImmediate(maybeCompleteLargeHeapAllocationObservation);
        }
      }
      if (this.traceCreateMutexA !== null && !result.isNull()) {
        trackedMutexHandles.set(result.toString().toLowerCase(), {
          createdByCallId: this.traceFrame.callId,
          initialOwner: this.traceCreateMutexA.initialOwner.toUInt32(),
          name: this.traceCreateMutexA.nameBefore,
        });
      }
      if (
        this.traceMutexLifecycle !== null &&
        name === "CloseHandle" &&
        result.toUInt32() !== 0
      ) {
        trackedMutexHandles.delete(this.traceMutexLifecycle.handle);
      }
      if (name === "Direct3DCreate9" && selectedVtables.has("idirect3d9")) {
        attachIdirect3d9(result);
      }
    },
  });
}

function scheduleSetCurrentDirectoryAOracle(module) {
  if (setCurrentDirectoryOracleScheduled) {
    return;
  }
  setCurrentDirectoryOracleScheduled = true;
  setImmediate(() => {
    let setCurrentDirectoryA = null;
    let originalPointer = null;
    let restored = false;
    try {
      const address = findExport(module, "SetCurrentDirectoryA");
      if (address === null) {
        throw new Error("SetCurrentDirectoryA export is unavailable");
      }
      setCurrentDirectoryA = new SystemFunction(
        address,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const original = snapshotCurrentDirectoryA(module);
      const originalBytes = original.path?.bytes ?? null;
      if (
        original.error !== undefined ||
        originalBytes === null ||
        originalBytes.length < 2 ||
        originalBytes.at(-1) !== 0
      ) {
        throw new Error(
          "current-directory oracle could not snapshot its original path",
        );
      }

      const allocatePath = (bytes) => {
        const pointer = Memory.alloc(Math.max(1, bytes.length));
        pointer.writeByteArray(bytes.length === 0 ? [0] : bytes);
        return pointer;
      };
      originalPointer = allocatePath(originalBytes);
      const invoke = (id, sentinel, pathBytes) => {
        const pointer = allocatePath(pathBytes);
        setCurrentDirectoryOracleSentinel = sentinel;
        setCurrentDirectoryOracleResult = null;
        setCurrentDirectoryOracleLastError = null;
        try {
          const outcome = setCurrentDirectoryA(pointer);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            setCurrentDirectoryOracleResult === null ||
            setCurrentDirectoryOracleLastError === null
          ) {
            throw new Error(
              `${id} did not pass through the current-directory hook`,
            );
          }
          return {
            record: {
              id,
              sentinel,
              pathBytes,
              result,
              hookResult: setCurrentDirectoryOracleResult,
              hookLastErrorAfter: setCurrentDirectoryOracleLastError,
              systemLastError,
              currentDirectoryAfter: snapshotCurrentDirectoryA(module),
            },
            pointer,
          };
        } finally {
          setCurrentDirectoryOracleSentinel = null;
          setCurrentDirectoryOracleResult = null;
          setCurrentDirectoryOracleLastError = null;
        }
      };

      const parent = invoke("relative-parent", 87, [0x2e, 0x2e, 0]);
      parent.pointer.writeByteArray([0x58, 0x58, 0]);
      const currentDirectoryAfterInputMutation =
        snapshotCurrentDirectoryA(module);
      const self = invoke("relative-self", 88, [0x2e, 0]);

      const originalWithoutNul = originalBytes.slice(0, -1);
      const separator = originalWithoutNul.at(-1) === 0x5c ? [] : [0x5c];
      const moduleNameBytes = [...Process.mainModule.name].map((character) =>
        character.charCodeAt(0),
      );
      const filePathBytes = [
        ...originalWithoutNul,
        ...separator,
        ...moduleNameBytes,
        0,
      ];
      const missingPathBytes = [
        ...originalWithoutNul,
        ...separator,
        ..."__vine_missing_current_directory_oracle_31__",
      ].map((value) =>
        typeof value === "number" ? value : value.charCodeAt(0),
      );
      missingPathBytes.push(0);
      const file = invoke("existing-file-not-directory", 89, filePathBytes);
      const missing = invoke("missing-directory", 90, missingPathBytes);
      const empty = invoke("empty-path", 91, [0]);
      const restore = invoke("restore-original", 92, originalBytes);
      restored = restore.record.result !== 0;

      emit("set-current-directory-a-oracle", {
        profile: "x86-process-directory-copy-relative-failure-v1",
        original,
        inputCopiedBeforeMutation:
          parent.record.result !== 0 &&
          currentDirectoryAfterInputMutation.path?.contentFnv1a32 ===
            parent.record.currentDirectoryAfter.path?.contentFnv1a32,
        currentDirectoryAfterInputMutation,
        calls: [
          parent.record,
          self.record,
          file.record,
          missing.record,
          empty.record,
          restore.record,
        ],
        restored,
      });
    } catch (error) {
      emit("set-current-directory-a-oracle-error", { error: error.message });
    } finally {
      setCurrentDirectoryOracleSentinel = null;
      setCurrentDirectoryOracleResult = null;
      setCurrentDirectoryOracleLastError = null;
      if (
        !restored &&
        setCurrentDirectoryA !== null &&
        originalPointer !== null
      ) {
        try {
          setCurrentDirectoryA(originalPointer);
        } catch (error) {
          emit("set-current-directory-a-oracle-restore-error", {
            error: error.message,
          });
        }
      }
    }
  });
}

function scheduleCreateMutexAOracle(module) {
  if (createMutexOracleScheduled) {
    return;
  }
  createMutexOracleScheduled = true;
  setImmediate(() => {
    const handles = [];
    let closeHandle = null;
    try {
      const address = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const createMutexA = new SystemFunction(
        address("CreateMutexA"),
        "pointer",
        ["pointer", "uint32", "pointer"],
        "stdcall",
      );
      const waitForSingleObject = new SystemFunction(
        address("WaitForSingleObject"),
        "uint32",
        ["pointer", "uint32"],
        "stdcall",
      );
      const releaseMutex = new SystemFunction(
        address("ReleaseMutex"),
        "uint32",
        ["pointer"],
        "stdcall",
      );
      closeHandle = new SystemFunction(
        address("CloseHandle"),
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const nameBytes = [...`Local\\VineCreateMutexAOracle-${Process.id}`].map(
        (character) => character.charCodeAt(0),
      );
      nameBytes.push(0);
      const allocateName = () => {
        const pointer = Memory.alloc(nameBytes.length);
        pointer.writeByteArray(nameBytes);
        return pointer;
      };
      const invokeCreate = (id, sentinel, initialOwner, name) => {
        createMutexOracleSentinel = sentinel;
        createMutexOracleResult = null;
        createMutexOracleLastError = null;
        try {
          const outcome = createMutexA(ptr(0), initialOwner, name);
          const result = outcome.value.toString();
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            createMutexOracleResult === null ||
            createMutexOracleLastError === null
          ) {
            throw new Error(`${id} did not pass through the CreateMutexA hook`);
          }
          if (result !== "0x0") {
            handles.push(outcome.value);
          }
          return {
            id,
            sentinel,
            initialOwner,
            name: snapshotMultiByteSource(name, -1),
            result,
            hookResult: createMutexOracleResult,
            hookLastErrorAfter: createMutexOracleLastError,
            systemLastError,
          };
        } finally {
          createMutexOracleSentinel = null;
          createMutexOracleResult = null;
          createMutexOracleLastError = null;
        }
      };
      const invokeUint32 = (api, id, sentinel, operation) => {
        createMutexOracleSentinel = sentinel;
        try {
          const outcome = operation();
          return {
            api,
            id,
            sentinel,
            result: Number(outcome.value) >>> 0,
            systemLastError: Number(outcome.lastError) >>> 0,
          };
        } finally {
          createMutexOracleSentinel = null;
        }
      };

      const firstName = allocateName();
      const first = invokeCreate("create-initial-owner", 87, 1, firstName);
      firstName.writeU8(0x58);
      const firstInputAfterMutation = snapshotMultiByteSource(firstName, -1);
      const duplicateName = allocateName();
      const duplicate = invokeCreate(
        "create-existing-initial-owner-ignored",
        88,
        1,
        duplicateName,
      );
      const firstHandle = ptr(first.result);
      const duplicateHandle = ptr(duplicate.result);
      const recursiveWait = invokeUint32(
        "WaitForSingleObject",
        "wait-recursive-through-duplicate-handle",
        89,
        () => waitForSingleObject(duplicateHandle, 0),
      );
      const releaseRecursive = invokeUint32(
        "ReleaseMutex",
        "release-recursive-acquisition",
        90,
        () => releaseMutex(duplicateHandle),
      );
      const releaseInitial = invokeUint32(
        "ReleaseMutex",
        "release-initial-ownership",
        91,
        () => releaseMutex(firstHandle),
      );
      const releaseUnowned = invokeUint32(
        "ReleaseMutex",
        "reject-release-while-unowned",
        92,
        () => releaseMutex(firstHandle),
      );
      const waitAfterRelease = invokeUint32(
        "WaitForSingleObject",
        "wait-after-full-release",
        93,
        () => waitForSingleObject(duplicateHandle, 0),
      );
      const releaseAfterWait = invokeUint32(
        "ReleaseMutex",
        "release-after-wait",
        94,
        () => releaseMutex(duplicateHandle),
      );
      const closeFirst = invokeUint32(
        "CloseHandle",
        "close-first-handle",
        95,
        () => closeHandle(firstHandle),
      );
      const waitAfterFirstClose = invokeUint32(
        "WaitForSingleObject",
        "wait-through-remaining-handle",
        96,
        () => waitForSingleObject(duplicateHandle, 0),
      );
      const releaseAfterFirstClose = invokeUint32(
        "ReleaseMutex",
        "release-through-remaining-handle",
        97,
        () => releaseMutex(duplicateHandle),
      );
      const closeDuplicate = invokeUint32(
        "CloseHandle",
        "close-last-handle",
        98,
        () => closeHandle(duplicateHandle),
      );
      const recreatedName = allocateName();
      const recreated = invokeCreate(
        "recreate-after-last-close",
        99,
        0,
        recreatedName,
      );
      const recreatedHandle = ptr(recreated.result);
      const waitRecreated = invokeUint32(
        "WaitForSingleObject",
        "wait-recreated-unowned-mutex",
        100,
        () => waitForSingleObject(recreatedHandle, 0),
      );
      const releaseRecreated = invokeUint32(
        "ReleaseMutex",
        "release-recreated-mutex",
        101,
        () => releaseMutex(recreatedHandle),
      );
      const closeRecreated = invokeUint32(
        "CloseHandle",
        "close-recreated-handle",
        102,
        () => closeHandle(recreatedHandle),
      );

      emit("create-mutex-a-oracle", {
        profile: "x86-named-copy-owner-handle-lifetime-v1",
        constants: {
          errorAlreadyExists: 183,
          errorNotOwner: 288,
          waitObject0: 0,
          zeroTimeoutMilliseconds: 0,
        },
        nameBytes,
        firstInputAfterMutation,
        calls: [
          first,
          duplicate,
          recursiveWait,
          releaseRecursive,
          releaseInitial,
          releaseUnowned,
          waitAfterRelease,
          releaseAfterWait,
          closeFirst,
          waitAfterFirstClose,
          releaseAfterFirstClose,
          closeDuplicate,
          recreated,
          waitRecreated,
          releaseRecreated,
          closeRecreated,
        ],
        relations: {
          nameCopiedBeforeCallerMutation: duplicate.systemLastError === 183,
          sameObjectReturnedThroughDistinctHandles:
            duplicate.systemLastError === 183 &&
            first.result !== duplicate.result,
          newObjectClearsLastError:
            first.systemLastError === 0 && recreated.systemLastError === 0,
          initialOwnerGrantedAndExistingInitialOwnerIgnored:
            recursiveWait.result === 0 &&
            releaseRecursive.result !== 0 &&
            releaseInitial.result !== 0 &&
            releaseUnowned.result === 0 &&
            releaseUnowned.systemLastError === 288,
          remainingHandleKeepsObjectAlive:
            closeFirst.result !== 0 &&
            waitAfterFirstClose.result === 0 &&
            releaseAfterFirstClose.result !== 0,
          lastCloseRetiresNamedObject:
            closeDuplicate.result !== 0 &&
            recreated.result !== "0x0" &&
            recreated.systemLastError === 0,
        },
      });
    } catch (error) {
      emit("create-mutex-a-oracle-error", { error: error.message });
    } finally {
      createMutexOracleSentinel = null;
      createMutexOracleResult = null;
      createMutexOracleLastError = null;
      if (closeHandle !== null) {
        for (const handle of handles) {
          try {
            closeHandle(handle);
          } catch {
            // Best-effort cleanup after a partial oracle sequence.
          }
        }
      }
    }
  });
}

function scheduleCommandLineOracle(module) {
  if (commandLineOracleScheduled) {
    return;
  }
  commandLineOracleScheduled = true;
  setImmediate(() => {
    try {
      const getExport = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const getCommandLineA = new NativeFunction(
        getExport("GetCommandLineA"),
        "pointer",
        [],
        "stdcall",
      );
      const getCommandLineW = new NativeFunction(
        getExport("GetCommandLineW"),
        "pointer",
        [],
        "stdcall",
      );
      const observations = [];
      for (const [api, encoding, operation] of [
        ["GetCommandLineA", "ansi", getCommandLineA],
        ["GetCommandLineA", "ansi", getCommandLineA],
        ["GetCommandLineW", "utf16", getCommandLineW],
        ["GetCommandLineW", "utf16", getCommandLineW],
      ]) {
        const pointer = operation();
        observations.push({
          api,
          commandLine: snapshotNulTerminatedString(pointer, encoding),
        });
      }
      emit("command-line-oracle", {
        profile: "x86-process-lifetime-stability-v1",
        observations,
        ansiPointerStable:
          observations[0].commandLine.pointer ===
          observations[1].commandLine.pointer,
        utf16PointerStable:
          observations[2].commandLine.pointer ===
          observations[3].commandLine.pointer,
        ansiAndUtf16PointersDistinct:
          observations[0].commandLine.pointer !==
          observations[2].commandLine.pointer,
        stringsEqual:
          observations[0].commandLine.string ===
          observations[2].commandLine.string,
      });
    } catch (error) {
      emit("command-line-oracle-error", { error: error.message });
    }
  });
}

function scheduleEnvironmentStringsOracle(module) {
  if (environmentStringsOracleScheduled) {
    return;
  }
  environmentStringsOracleScheduled = true;
  setImmediate(() => {
    try {
      const getExport = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const getEnvironmentStringsW = new NativeFunction(
        getExport("GetEnvironmentStringsW"),
        "pointer",
        [],
        "stdcall",
      );
      const freeEnvironmentStringsW = new NativeFunction(
        getExport("FreeEnvironmentStringsW"),
        "int",
        ["pointer"],
        "stdcall",
      );
      const processEnvironmentPointer =
        readProcessParametersEnvironmentPointer();
      const invoke = (step, sentinel, operation) => {
        environmentStringsOracleSentinel = sentinel;
        environmentStringsOracleLastError = null;
        try {
          const result = operation();
          const lastErrorAfter = environmentStringsOracleLastError;
          if (lastErrorAfter === null) {
            throw new Error(
              `${step} did not pass through the environment hook`,
            );
          }
          return { result, lastErrorAfter };
        } finally {
          environmentStringsOracleSentinel = null;
          environmentStringsOracleLastError = null;
        }
      };
      const acquire = (sentinel) => {
        const outcome = invoke("acquire", sentinel, () =>
          getEnvironmentStringsW(),
        );
        return {
          sentinel,
          lastErrorAfter: outcome.lastErrorAfter,
          block: snapshotUtf16EnvironmentBlock(
            outcome.result,
            processEnvironmentPointer,
          ),
        };
      };
      const release = (acquisition, sentinel) => {
        const outcome = invoke("release", sentinel, () =>
          freeEnvironmentStringsW(ptr(acquisition.block.pointer)),
        );
        return {
          sentinel,
          lastErrorAfter: outcome.lastErrorAfter,
          result: Number(outcome.result),
          pointer: acquisition.block.pointer,
        };
      };
      const acquisitions = [acquire(0), acquire(87)];
      const releases = [
        release(acquisitions[0], 88),
        release(acquisitions[1], 89),
      ];
      emit("environment-strings-oracle", {
        profile: "x86-wide-copy-lifetime-v1",
        processEnvironmentPointer,
        acquisitions,
        releases,
        pointersEqual:
          acquisitions[0].block.pointer === acquisitions[1].block.pointer,
        contentShapesEqual:
          acquisitions[0].block.byteLengthWithDoubleNul ===
            acquisitions[1].block.byteLengthWithDoubleNul &&
          acquisitions[0].block.entryCount ===
            acquisitions[1].block.entryCount &&
          acquisitions[0].block.contentFnv1a32 ===
            acquisitions[1].block.contentFnv1a32,
      });
    } catch (error) {
      emit("environment-strings-oracle-error", { error: error.message });
    }
  });
}

function scheduleWideCharOracle(module) {
  if (wideCharOracleScheduled) {
    return;
  }
  wideCharOracleScheduled = true;
  setImmediate(() => {
    try {
      const getExport = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const wideCharToMultiByte = new NativeFunction(
        getExport("WideCharToMultiByte"),
        "int",
        [
          "uint32",
          "uint32",
          "pointer",
          "int",
          "pointer",
          "int",
          "pointer",
          "pointer",
        ],
        "stdcall",
      );
      const getAcp = new NativeFunction(
        getExport("GetACP"),
        "uint32",
        [],
        "stdcall",
      );
      const allocateUtf16 = (units) => {
        const pointer = Memory.alloc(units.length * 2);
        units.forEach((unit, index) => pointer.add(index * 2).writeU16(unit));
        return pointer;
      };
      const invoke = ({
        id,
        sentinel,
        sourceUnits,
        sourceCount,
        destinationBytes,
        defaultCharBytes = null,
        observeUsedDefault = false,
      }) => {
        const source = allocateUtf16(sourceUnits);
        const destination =
          destinationBytes === 0 ? ptr(0) : Memory.alloc(destinationBytes);
        if (!destination.isNull()) {
          destination.writeByteArray(Array(destinationBytes).fill(0xa5));
        }
        const defaultChar =
          defaultCharBytes === null
            ? ptr(0)
            : Memory.alloc(defaultCharBytes.length);
        if (!defaultChar.isNull()) {
          defaultChar.writeByteArray(defaultCharBytes);
        }
        const usedDefault = observeUsedDefault ? Memory.alloc(4) : ptr(0);
        if (!usedDefault.isNull()) {
          usedDefault.writeU32(0xa5a5a5a5);
        }
        wideCharOracleSentinel = sentinel;
        wideCharOracleLastError = null;
        try {
          const result = Number(
            wideCharToMultiByte(
              0,
              0,
              source,
              sourceCount,
              destination,
              destinationBytes,
              defaultChar,
              usedDefault,
            ),
          );
          if (wideCharOracleLastError === null) {
            throw new Error(`${id} did not pass through the conversion hook`);
          }
          return {
            id,
            sentinel,
            sourceUnits,
            sourceCount,
            destinationCapacity: destinationBytes,
            defaultCharBytes,
            observeUsedDefault,
            result,
            lastErrorAfter: wideCharOracleLastError,
            destinationBytes: destination.isNull()
              ? null
              : [
                  ...new Uint8Array(
                    destination.readByteArray(destinationBytes),
                  ),
                ],
            usedDefaultChar: usedDefault.isNull()
              ? null
              : usedDefault.readU32(),
          };
        } finally {
          wideCharOracleSentinel = null;
          wideCharOracleLastError = null;
        }
      };

      const emptyUnits = [0, 0];
      const japaneseUnits = [0x41, 0x3042, 0];
      const unmappableUnits = [0xd83d, 0xde00, 0];
      const emptyQuery = invoke({
        id: "empty-query",
        sentinel: 87,
        sourceUnits: emptyUnits,
        sourceCount: 1,
        destinationBytes: 0,
      });
      const emptyConversion = invoke({
        id: "empty-conversion",
        sentinel: 88,
        sourceUnits: emptyUnits,
        sourceCount: 1,
        destinationBytes: emptyQuery.result,
      });
      const japaneseQuery = invoke({
        id: "ascii-japanese-query",
        sentinel: 89,
        sourceUnits: japaneseUnits,
        sourceCount: japaneseUnits.length,
        destinationBytes: 0,
        observeUsedDefault: true,
      });
      const japaneseConversion = invoke({
        id: "ascii-japanese-conversion",
        sentinel: 90,
        sourceUnits: japaneseUnits,
        sourceCount: japaneseUnits.length,
        destinationBytes: japaneseQuery.result,
        observeUsedDefault: true,
      });
      const unmappableConversion = invoke({
        id: "unmappable-conversion",
        sentinel: 91,
        sourceUnits: unmappableUnits,
        sourceCount: unmappableUnits.length,
        destinationBytes: 8,
        defaultCharBytes: [0x3f, 0],
        observeUsedDefault: true,
      });
      const insufficientBuffer = invoke({
        id: "insufficient-buffer",
        sentinel: 92,
        sourceUnits: japaneseUnits,
        sourceCount: japaneseUnits.length,
        destinationBytes: 1,
        observeUsedDefault: true,
      });
      const zeroSourceCount = invoke({
        id: "zero-source-count",
        sentinel: 93,
        sourceUnits: emptyUnits,
        sourceCount: 0,
        destinationBytes: 0,
      });
      emit("wide-char-to-multi-byte-oracle", {
        profile: "x86-cp-acp-query-convert-v1",
        activeCodePage: Number(getAcp()),
        calls: [
          emptyQuery,
          emptyConversion,
          japaneseQuery,
          japaneseConversion,
          unmappableConversion,
          insufficientBuffer,
          zeroSourceCount,
        ],
      });
    } catch (error) {
      emit("wide-char-to-multi-byte-oracle-error", { error: error.message });
    }
  });
}

function scheduleMultiByteOracle(module) {
  if (multiByteOracleScheduled) {
    return;
  }
  multiByteOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "MultiByteToWideChar");
      if (address === null) {
        throw new Error("MultiByteToWideChar export is unavailable");
      }
      const multiByteToWideChar = new SystemFunction(
        address,
        "int",
        ["uint32", "uint32", "pointer", "int", "pointer", "int"],
        "stdcall",
      );
      const invoke = ({
        id,
        sentinel,
        sourceBytes,
        sourceCount,
        destinationUnits,
        codePage = 932,
        flags = 1,
      }) => {
        const source = Memory.alloc(Math.max(1, sourceBytes.length));
        source.writeByteArray(sourceBytes.length === 0 ? [0] : sourceBytes);
        const storageBytes = Math.max(16, destinationUnits * 2 + 16);
        const storage = Memory.alloc(storageBytes);
        storage.writeByteArray(Array(storageBytes).fill(0xa5));
        const destination = destinationUnits === 0 ? ptr(0) : storage.add(8);
        const storageBefore = snapshotBoundedRawBytes(
          storage,
          Math.min(64, storageBytes),
        );
        multiByteOracleSentinel = sentinel;
        multiByteOracleLastError = null;
        try {
          const outcome = multiByteToWideChar(
            codePage,
            flags,
            source,
            sourceCount,
            destination,
            destinationUnits,
          );
          const result = Number(outcome.value);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (multiByteOracleLastError === null) {
            throw new Error(`${id} did not pass through the conversion hook`);
          }
          return {
            id,
            sentinel,
            codePage,
            flags,
            sourceBytes,
            sourceCount,
            destinationCapacityUnits: destinationUnits,
            result,
            hookLastErrorAfter: multiByteOracleLastError,
            systemLastError,
            storageOffsetBytes: destinationUnits === 0 ? null : 8,
            storageBefore,
            storageAfter: snapshotBoundedRawBytes(
              storage,
              Math.min(64, storageBytes),
            ),
            destinationAfter: snapshotWideCharBuffer(
              destination,
              result > 0 ? result : destinationUnits,
            ),
          };
        } finally {
          multiByteOracleSentinel = null;
          multiByteOracleLastError = null;
        }
      };

      const asciiJapaneseBytes = [0x41, 0x82, 0xa0, 0];
      const query = invoke({
        id: "ascii-japanese-fixed-query",
        sentinel: 87,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: asciiJapaneseBytes.length,
        destinationUnits: 0,
      });
      const conversion = invoke({
        id: "ascii-japanese-fixed-conversion",
        sentinel: 88,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: asciiJapaneseBytes.length,
        destinationUnits: query.result,
      });
      const nulTerminatedQuery = invoke({
        id: "ascii-japanese-nul-query",
        sentinel: 89,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: -1,
        destinationUnits: 0,
      });
      const insufficientBuffer = invoke({
        id: "insufficient-buffer",
        sentinel: 90,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: asciiJapaneseBytes.length,
        destinationUnits: 1,
      });
      const zeroSourceCount = invoke({
        id: "zero-source-count",
        sentinel: 91,
        sourceBytes: [0],
        sourceCount: 0,
        destinationUnits: 0,
      });
      const activeCodePageQuery = invoke({
        id: "active-code-page-nul-query",
        sentinel: 92,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: -1,
        destinationUnits: 0,
        codePage: 0,
        flags: 0,
      });
      const activeCodePageConversion = invoke({
        id: "active-code-page-nul-conversion",
        sentinel: 93,
        sourceBytes: asciiJapaneseBytes,
        sourceCount: -1,
        destinationUnits: activeCodePageQuery.result,
        codePage: 0,
        flags: 0,
      });
      emit("multi-byte-to-wide-char-oracle", {
        profile: "x86-windows-932-fixed-and-acp-nul-v2",
        calls: [
          query,
          conversion,
          nulTerminatedQuery,
          insufficientBuffer,
          zeroSourceCount,
          activeCodePageQuery,
          activeCodePageConversion,
        ],
      });
    } catch (error) {
      emit("multi-byte-to-wide-char-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetStringTypeOracle(module) {
  if (getStringTypeOracleScheduled) {
    return;
  }
  getStringTypeOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetStringTypeW");
      if (address === null) {
        throw new Error("GetStringTypeW export is unavailable");
      }
      const getStringTypeW = new SystemFunction(
        address,
        "uint32",
        ["uint32", "pointer", "int", "pointer"],
        "stdcall",
      );
      const invoke = ({
        id,
        sentinel,
        sourceWords,
        sourceCount = sourceWords.length,
        infoType = 1,
        outputCapacityWords = Math.max(1, sourceWords.length),
      }) => {
        const source = Memory.alloc(Math.max(2, sourceWords.length * 2));
        for (let index = 0; index < sourceWords.length; index += 1) {
          source.add(index * 2).writeU16(sourceWords[index]);
        }
        const leadingGuardBytes = 8;
        const trailingGuardBytes = 8;
        const storageBytes =
          leadingGuardBytes + outputCapacityWords * 2 + trailingGuardBytes;
        const storage = Memory.alloc(storageBytes);
        storage.writeByteArray(Array(storageBytes).fill(0xa5));
        const output = storage.add(leadingGuardBytes);
        const leadingGuardBefore = snapshotBoundedRawBytes(
          storage,
          leadingGuardBytes,
        );
        const outputBefore = snapshotWordBuffer(output, outputCapacityWords);
        const trailingGuard = output.add(outputCapacityWords * 2);
        const trailingGuardBefore = snapshotBoundedRawBytes(
          trailingGuard,
          trailingGuardBytes,
        );
        getStringTypeOracleSentinel = sentinel;
        getStringTypeOracleResult = null;
        getStringTypeOracleLastError = null;
        try {
          const outcome = getStringTypeW(infoType, source, sourceCount, output);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getStringTypeOracleResult === null ||
            getStringTypeOracleLastError === null
          ) {
            throw new Error(
              `${id} did not pass through the classification hook`,
            );
          }
          return {
            id,
            sentinel,
            infoType,
            sourceWords,
            sourceCount,
            outputCapacityWords,
            result,
            hookResult: getStringTypeOracleResult,
            hookLastErrorAfter: getStringTypeOracleLastError,
            systemLastError,
            leadingGuardBefore,
            leadingGuardAfter: snapshotBoundedRawBytes(
              storage,
              leadingGuardBytes,
            ),
            outputBefore,
            outputAfter: snapshotWordBuffer(output, outputCapacityWords),
            trailingGuardBefore,
            trailingGuardAfter: snapshotBoundedRawBytes(
              trailingGuard,
              trailingGuardBytes,
            ),
          };
        } finally {
          getStringTypeOracleSentinel = null;
          getStringTypeOracleResult = null;
          getStringTypeOracleLastError = null;
        }
      };

      const representativeWords = [
        0x0000, 0x0009, 0x000a, 0x000d, 0x0020, 0x0021, 0x002b, 0x002c, 0x002e,
        0x0030, 0x0039, 0x0041, 0x005a, 0x005f, 0x0061, 0x007a, 0x007f, 0x0080,
        0x00a0, 0x00a5, 0x3000, 0x3042, 0x30a2, 0x30fb, 0x4e00, 0xff01, 0xff10,
        0xff21, 0xff41, 0xff61, 0xff66, 0xff71, 0xff9e, 0xf8f0,
      ];
      const calls = [0, 87, 0x12345678].map((sentinel, index) =>
        invoke({
          id: `representative-ctype1-${index + 1}`,
          sentinel,
          sourceWords: representativeWords,
        }),
      );
      calls.push(
        invoke({
          id: "nul-terminated-ctype1",
          sentinel: 88,
          sourceWords: [0x41, 0],
          sourceCount: -1,
          outputCapacityWords: 2,
        }),
        invoke({
          id: "zero-source-count",
          sentinel: 89,
          sourceWords: [0x41],
          sourceCount: 0,
          outputCapacityWords: 1,
        }),
        invoke({
          id: "negative-nul-terminated-ctype1",
          sentinel: 90,
          sourceWords: [0x41, 0],
          sourceCount: -2,
          outputCapacityWords: 2,
        }),
        invoke({
          id: "invalid-info-type",
          sentinel: 91,
          sourceWords: [0x41],
          infoType: 0,
          outputCapacityWords: 1,
        }),
      );
      emit("get-string-type-w-oracle", {
        profile: "x86-ctype1-representative-v1",
        calls,
      });
    } catch (error) {
      emit("get-string-type-w-oracle-error", { error: error.message });
    }
  });
}

function scheduleLCMapStringOracle(module) {
  if (lcMapStringOracleScheduled) {
    return;
  }
  lcMapStringOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "LCMapStringW");
      if (address === null) {
        throw new Error("LCMapStringW export is unavailable");
      }
      const lcMapStringW = new SystemFunction(
        address,
        "int",
        ["uint32", "uint32", "pointer", "int", "pointer", "int"],
        "stdcall",
      );
      const allocateUtf16 = (words) => {
        const pointer = Memory.alloc(words.length * 2);
        words.forEach((word, index) => pointer.add(index * 2).writeU16(word));
        return pointer;
      };
      const invoke = ({
        id,
        sentinel,
        sourceWords,
        sourceCount = sourceWords.length,
        destinationCapacity = 0,
        locale = 0x0411,
        flags = 0x0100,
        nullDestinationWithCapacity = false,
      }) => {
        const source = allocateUtf16(sourceWords);
        const guardBytes = 16;
        const storage =
          destinationCapacity === 0 || nullDestinationWithCapacity
            ? null
            : Memory.alloc(guardBytes + destinationCapacity * 2 + guardBytes);
        if (storage !== null) {
          storage.writeByteArray(
            Array(guardBytes + destinationCapacity * 2 + guardBytes).fill(0xa5),
          );
        }
        const destination = storage === null ? ptr(0) : storage.add(guardBytes);
        const trailingGuard =
          storage === null ? null : destination.add(destinationCapacity * 2);
        const sourceBefore = snapshotWideCharBuffer(source, sourceWords.length);
        const leadingGuardBefore =
          storage === null
            ? null
            : snapshotBoundedRawBytes(storage, guardBytes);
        const destinationBefore = snapshotWideCharBuffer(
          destination,
          destinationCapacity,
        );
        const trailingGuardBefore =
          trailingGuard === null
            ? null
            : snapshotBoundedRawBytes(trailingGuard, guardBytes);
        lcMapStringOracleSentinel = sentinel;
        lcMapStringOracleResult = null;
        lcMapStringOracleLastError = null;
        try {
          const outcome = lcMapStringW(
            locale,
            flags,
            source,
            sourceCount,
            destination,
            destinationCapacity,
          );
          const result = Number(outcome.value) | 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            lcMapStringOracleResult === null ||
            lcMapStringOracleLastError === null
          ) {
            throw new Error(`${id} did not pass through the mapping hook`);
          }
          return {
            id,
            sentinel,
            locale,
            flags,
            sourceWords,
            sourceCount,
            destinationCapacity,
            nullDestinationWithCapacity,
            result,
            hookResult: lcMapStringOracleResult,
            hookLastErrorAfter: lcMapStringOracleLastError,
            systemLastError,
            sourceBefore,
            sourceAfter: snapshotWideCharBuffer(source, sourceWords.length),
            leadingGuardBefore,
            leadingGuardAfter:
              storage === null
                ? null
                : snapshotBoundedRawBytes(storage, guardBytes),
            destinationBefore,
            destinationAfter: snapshotWideCharBuffer(
              destination,
              destinationCapacity,
            ),
            trailingGuardBefore,
            trailingGuardAfter:
              trailingGuard === null
                ? null
                : snapshotBoundedRawBytes(trailingGuard, guardBytes),
          };
        } finally {
          lcMapStringOracleSentinel = null;
          lcMapStringOracleResult = null;
          lcMapStringOracleLastError = null;
        }
      };

      const representativeWords = [
        0x0041, 0x005a, 0x0061, 0x3042, 0x30a2, 0xff21, 0xff41,
      ];
      const calls = [
        invoke({
          id: "explicit-lowercase-query",
          sentinel: 0,
          sourceWords: representativeWords,
        }),
        invoke({
          id: "explicit-lowercase-conversion",
          sentinel: 87,
          sourceWords: representativeWords,
          destinationCapacity: representativeWords.length,
        }),
        invoke({
          id: "nul-terminated-lowercase",
          sentinel: 88,
          sourceWords: [...representativeWords, 0],
          sourceCount: -1,
          destinationCapacity: representativeWords.length + 1,
        }),
        invoke({
          id: "insufficient-destination",
          sentinel: 89,
          sourceWords: representativeWords,
          destinationCapacity: 2,
        }),
        invoke({
          id: "zero-source-count",
          sentinel: 90,
          sourceWords: [0x0041],
          sourceCount: 0,
          destinationCapacity: 1,
        }),
        invoke({
          id: "negative-two-source-count",
          sentinel: 91,
          sourceWords: [0x0041, 0],
          sourceCount: -2,
          destinationCapacity: 2,
        }),
        invoke({
          id: "invalid-flags",
          sentinel: 92,
          sourceWords: [0x0041],
          flags: 0,
        }),
        invoke({
          id: "null-destination-with-capacity",
          sentinel: 93,
          sourceWords: [0x0041],
          destinationCapacity: 1,
          nullDestinationWithCapacity: true,
        }),
      ];
      emit("lc-map-string-w-oracle", {
        profile: "x86-ja-jp-lowercase-v1",
        calls,
      });
    } catch (error) {
      emit("lc-map-string-w-oracle-error", { error: error.message });
    }
  });
}

function scheduleInterlockedDecrementOracle(module) {
  if (interlockedDecrementOracleScheduled) {
    return;
  }
  interlockedDecrementOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "InterlockedDecrement");
      if (address === null) {
        throw new Error("InterlockedDecrement export is unavailable");
      }
      const interlockedDecrement = new SystemFunction(
        address,
        "int",
        ["pointer"],
        "stdcall",
      );
      const invoke = (id, initialValue, sentinel) => {
        const guardBytes = 16;
        const storage = Memory.alloc(guardBytes + 4 + guardBytes);
        storage.writeByteArray(Array(guardBytes + 4 + guardBytes).fill(0xa5));
        const target = storage.add(guardBytes);
        target.writeS32(initialValue);
        const targetBefore = snapshotLong(target);
        const leadingGuardBefore = snapshotBoundedRawBytes(storage, guardBytes);
        const trailingGuard = target.add(4);
        const trailingGuardBefore = snapshotBoundedRawBytes(
          trailingGuard,
          guardBytes,
        );
        interlockedDecrementOracleSentinel = sentinel;
        interlockedDecrementOracleResult = null;
        interlockedDecrementOracleLastError = null;
        try {
          const outcome = interlockedDecrement(target);
          const result = Number(outcome.value) | 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            interlockedDecrementOracleResult === null ||
            interlockedDecrementOracleLastError === null
          ) {
            throw new Error(`${id} did not pass through the decrement hook`);
          }
          return {
            id,
            sentinel,
            initialValue,
            targetAlignment: target.toUInt32() % 4,
            targetBefore,
            targetAfter: snapshotLong(target),
            result,
            hookResult: interlockedDecrementOracleResult,
            hookLastErrorAfter: interlockedDecrementOracleLastError,
            systemLastError,
            leadingGuardBefore,
            leadingGuardAfter: snapshotBoundedRawBytes(storage, guardBytes),
            trailingGuardBefore,
            trailingGuardAfter: snapshotBoundedRawBytes(
              trailingGuard,
              guardBytes,
            ),
          };
        } finally {
          interlockedDecrementOracleSentinel = null;
          interlockedDecrementOracleResult = null;
          interlockedDecrementOracleLastError = null;
        }
      };
      emit("interlocked-decrement-oracle", {
        profile: "x86-aligned-wrapping-v1",
        calls: [
          invoke("one-to-zero", 1, 0),
          invoke("zero-to-minus-one", 0, 87),
          invoke("minimum-to-maximum", -0x80000000, 0x12345678),
        ],
      });
    } catch (error) {
      emit("interlocked-decrement-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetModuleFileNameAOracle(module) {
  if (getModuleFileNameAOracleScheduled) {
    return;
  }
  getModuleFileNameAOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetModuleFileNameA");
      if (address === null) {
        throw new Error("GetModuleFileNameA export is unavailable");
      }
      const getModuleFileNameA = new SystemFunction(
        address,
        "uint32",
        ["pointer", "pointer", "uint32"],
        "stdcall",
      );
      const invoke = ({
        id,
        sentinel,
        moduleHandle = ptr(0),
        capacity,
        nullOutput = false,
      }) => {
        const guardBytes = 16;
        const storage = nullOutput
          ? null
          : Memory.alloc(guardBytes + capacity + guardBytes);
        if (storage !== null) {
          storage.writeByteArray(
            Array(guardBytes + capacity + guardBytes).fill(0xa5),
          );
        }
        const output = storage === null ? ptr(0) : storage.add(guardBytes);
        const trailingGuard = storage === null ? null : output.add(capacity);
        const leadingGuardBefore =
          storage === null
            ? null
            : snapshotBoundedRawBytes(storage, guardBytes);
        const outputBefore = snapshotAnsiOutputBuffer(output, capacity);
        const trailingGuardBefore =
          trailingGuard === null
            ? null
            : snapshotBoundedRawBytes(trailingGuard, guardBytes);
        getModuleFileNameAOracleSentinel = sentinel;
        getModuleFileNameAOracleResult = null;
        getModuleFileNameAOracleLastError = null;
        try {
          const outcome = getModuleFileNameA(moduleHandle, output, capacity);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getModuleFileNameAOracleResult === null ||
            getModuleFileNameAOracleLastError === null
          ) {
            throw new Error(
              `${id} did not pass through the module-file-name hook`,
            );
          }
          return {
            id,
            sentinel,
            moduleHandle: moduleHandle.toString(),
            capacity,
            nullOutput,
            result,
            hookResult: getModuleFileNameAOracleResult,
            hookLastErrorAfter: getModuleFileNameAOracleLastError,
            systemLastError,
            leadingGuardBefore,
            leadingGuardAfter:
              storage === null
                ? null
                : snapshotBoundedRawBytes(storage, guardBytes),
            outputBefore,
            outputAfter: snapshotAnsiOutputBuffer(output, capacity),
            trailingGuardBefore,
            trailingGuardAfter:
              trailingGuard === null
                ? null
                : snapshotBoundedRawBytes(trailingGuard, guardBytes),
          };
        } finally {
          getModuleFileNameAOracleSentinel = null;
          getModuleFileNameAOracleResult = null;
          getModuleFileNameAOracleLastError = null;
        }
      };

      const generous = invoke({
        id: "main-module-generous",
        sentinel: 0,
        capacity: 1024,
      });
      if (generous.result === 0 || generous.result >= 1024) {
        throw new Error(
          "main-module generous query did not return a bounded path",
        );
      }
      const calls = [
        generous,
        invoke({
          id: "main-module-exact",
          sentinel: 87,
          capacity: generous.result + 1,
        }),
        invoke({
          id: "main-module-insufficient",
          sentinel: 0x12345678,
          capacity: generous.result,
        }),
        invoke({
          id: "main-module-one-byte",
          sentinel: 88,
          capacity: 1,
        }),
        invoke({
          id: "main-module-zero-capacity",
          sentinel: 89,
          capacity: 0,
        }),
        invoke({
          id: "main-module-null-zero-capacity",
          sentinel: 90,
          capacity: 0,
          nullOutput: true,
        }),
        invoke({
          id: "explicit-kernel32-module",
          sentinel: 91,
          moduleHandle: module.base,
          capacity: 1024,
        }),
        invoke({
          id: "invalid-module-handle",
          sentinel: 92,
          moduleHandle: ptr(1),
          capacity: 1024,
        }),
      ];
      emit("get-module-file-name-a-oracle", {
        profile: "x86-main-and-explicit-module-v1",
        mainModule: {
          name: Process.mainModule.name,
          base: Process.mainModule.base.toString(),
          path: Process.mainModule.path,
        },
        calls,
      });
    } catch (error) {
      emit("get-module-file-name-a-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetLastErrorOracle(module) {
  if (getLastErrorOracleScheduled) {
    return;
  }
  getLastErrorOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetLastError");
      if (address === null) {
        throw new Error("GetLastError export is unavailable");
      }
      const getLastError = new NativeFunction(address, "uint32", [], "stdcall");
      const invoke = (sentinel) => {
        getLastErrorOracleSentinel = sentinel;
        getLastErrorOracleResult = null;
        getLastErrorOracleLastError = null;
        try {
          const result = Number(getLastError()) >>> 0;
          if (
            getLastErrorOracleResult === null ||
            getLastErrorOracleLastError === null
          ) {
            throw new Error(
              "GetLastError oracle did not pass through the hook",
            );
          }
          return {
            sentinel,
            result,
            hookResult: getLastErrorOracleResult,
            lastErrorAfter: getLastErrorOracleLastError,
            resultMatchesSentinel: result === sentinel,
            hookResultMatchesSentinel: getLastErrorOracleResult === sentinel,
            preservesLastError: getLastErrorOracleLastError === sentinel,
          };
        } finally {
          getLastErrorOracleSentinel = null;
          getLastErrorOracleResult = null;
          getLastErrorOracleLastError = null;
        }
      };
      const calls = [0, 87, 122, 0x12345678].map(invoke);
      emit("get-last-error-oracle", {
        profile: "x86-thread-last-error-read-v1",
        calls,
        allResultsMatch: calls.every(
          (call) =>
            call.resultMatchesSentinel &&
            call.hookResultMatchesSentinel &&
            call.preservesLastError,
        ),
      });
    } catch (error) {
      emit("get-last-error-oracle-error", { error: error.message });
    }
  });
}

function scheduleSetLastErrorOracle(module) {
  if (setLastErrorOracleScheduled) {
    return;
  }
  setLastErrorOracleScheduled = true;
  setImmediate(() => {
    try {
      const setAddress = findExport(module, "SetLastError");
      if (setAddress === null) {
        throw new Error("SetLastError export is unavailable");
      }
      const setLastError = new SystemFunction(
        setAddress,
        "void",
        ["uint32"],
        "stdcall",
      );
      const invoke = (sentinel) => {
        setLastErrorOracleActive = true;
        setLastErrorOracleArgument = null;
        setLastErrorOracleEaxBefore = null;
        setLastErrorOracleEaxAfter = null;
        setLastErrorOracleLastErrorAfter = null;
        try {
          const outcome = setLastError(sentinel);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            setLastErrorOracleArgument === null ||
            setLastErrorOracleEaxBefore === null ||
            setLastErrorOracleEaxAfter === null ||
            setLastErrorOracleLastErrorAfter === null
          ) {
            throw new Error(
              "SetLastError oracle did not pass through the hook",
            );
          }
          return {
            sentinel,
            hookArgument: setLastErrorOracleArgument,
            eaxBefore: setLastErrorOracleEaxBefore,
            eaxAfter: setLastErrorOracleEaxAfter,
            preservesEax:
              setLastErrorOracleEaxAfter === setLastErrorOracleEaxBefore,
            hookLastErrorAfter: setLastErrorOracleLastErrorAfter,
            systemLastError,
            argumentMatches: setLastErrorOracleArgument === sentinel,
            hookLastErrorMatches: setLastErrorOracleLastErrorAfter === sentinel,
            systemLastErrorMatches: systemLastError === sentinel,
          };
        } finally {
          setLastErrorOracleActive = false;
          setLastErrorOracleArgument = null;
          setLastErrorOracleEaxBefore = null;
          setLastErrorOracleEaxAfter = null;
          setLastErrorOracleLastErrorAfter = null;
        }
      };
      const calls = [0, 87, 122, 0x12345678].map(invoke);
      emit("set-last-error-oracle", {
        profile: "x86-thread-last-error-write-v1",
        calls,
        allStateEffectsMatch: calls.every(
          (call) =>
            call.argumentMatches &&
            call.hookLastErrorMatches &&
            call.systemLastErrorMatches,
        ),
      });
    } catch (error) {
      emit("set-last-error-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetAcpOracle(module) {
  if (getAcpOracleScheduled) {
    return;
  }
  getAcpOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetACP");
      if (address === null) {
        throw new Error("GetACP export is unavailable");
      }
      const getAcp = new SystemFunction(address, "uint32", [], "stdcall");
      const invoke = (sentinel) => {
        getAcpOracleSentinel = sentinel;
        getAcpOracleResult = null;
        getAcpOracleLastError = null;
        try {
          const outcome = getAcp();
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (getAcpOracleResult === null || getAcpOracleLastError === null) {
            throw new Error("GetACP oracle did not pass through the hook");
          }
          return {
            sentinel,
            result,
            hookResult: getAcpOracleResult,
            hookLastErrorAfter: getAcpOracleLastError,
            systemLastError,
            hookResultMatches: getAcpOracleResult === result,
            hookPreservesLastError: getAcpOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          getAcpOracleSentinel = null;
          getAcpOracleResult = null;
          getAcpOracleLastError = null;
        }
      };
      const calls = [0, 87, 122, 0x12345678].map(invoke);
      const activeCodePage = calls[0].result;
      emit("get-acp-oracle", {
        profile: "x86-active-code-page-read-v1",
        activeCodePage,
        calls,
        allResultsStable: calls.every(
          (call) => call.result === activeCodePage && call.hookResultMatches,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("get-acp-oracle-error", { error: error.message });
    }
  });
}

function scheduleSystemDefaultLangIdOracle(module) {
  if (systemDefaultLangIdOracleScheduled) {
    return;
  }
  systemDefaultLangIdOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetSystemDefaultLangID");
      if (address === null) {
        throw new Error("GetSystemDefaultLangID export is unavailable");
      }
      const getSystemDefaultLangId = new SystemFunction(
        address,
        "uint16",
        [],
        "stdcall",
      );
      const invoke = (sentinel) => {
        systemDefaultLangIdOracleSentinel = sentinel;
        systemDefaultLangIdOracleResult = null;
        systemDefaultLangIdOracleLastError = null;
        try {
          const outcome = getSystemDefaultLangId();
          const result = Number(outcome.value) & 0xffff;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            systemDefaultLangIdOracleResult === null ||
            systemDefaultLangIdOracleLastError === null
          ) {
            throw new Error(
              "GetSystemDefaultLangID oracle did not pass through the hook",
            );
          }
          return {
            sentinel,
            result,
            hookRawEax: systemDefaultLangIdOracleResult,
            hookLangId: systemDefaultLangIdOracleResult & 0xffff,
            hookLastErrorAfter: systemDefaultLangIdOracleLastError,
            systemLastError,
            hookResultMatches:
              (systemDefaultLangIdOracleResult & 0xffff) === result,
            hookPreservesLastError:
              systemDefaultLangIdOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          systemDefaultLangIdOracleSentinel = null;
          systemDefaultLangIdOracleResult = null;
          systemDefaultLangIdOracleLastError = null;
        }
      };
      const calls = [0, 87, 122, 0x12345678].map(invoke);
      const langId = calls[0].result;
      emit("system-default-lang-id-oracle", {
        profile: "x86-system-default-lang-id-read-v1",
        langId,
        calls,
        rawEaxValues: calls.map((call) => call.hookRawEax),
        upperEaxBitsContract: "unspecified",
        allResultsStable: calls.every(
          (call) => call.result === langId && call.hookResultMatches,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("system-default-lang-id-oracle-error", { error: error.message });
    }
  });
}

function scheduleDriveTypeOracle(module) {
  if (driveTypeOracleScheduled) {
    return;
  }
  driveTypeOracleScheduled = true;
  setImmediate(() => {
    try {
      const driveTypeAddress = findExport(module, "GetDriveTypeA");
      const currentDirectoryAddress = findExport(
        module,
        "GetCurrentDirectoryA",
      );
      if (driveTypeAddress === null || currentDirectoryAddress === null) {
        throw new Error("drive-type oracle exports are unavailable");
      }
      const getDriveType = new SystemFunction(
        driveTypeAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const getCurrentDirectory = new SystemFunction(
        currentDirectoryAddress,
        "uint32",
        ["uint32", "pointer"],
        "stdcall",
      );
      const currentDirectoryBuffer = Memory.alloc(32768);
      const currentDirectoryOutcome = getCurrentDirectory(
        32768,
        currentDirectoryBuffer,
      );
      const currentDirectoryLength =
        Number(currentDirectoryOutcome.value) >>> 0;
      if (currentDirectoryLength === 0 || currentDirectoryLength >= 32768) {
        throw new Error("GetCurrentDirectoryA did not return a bounded path");
      }
      const currentDirectory = currentDirectoryBuffer.readAnsiString();
      if (
        currentDirectory.length < 3 ||
        currentDirectory[1] !== ":" ||
        currentDirectory[2] !== "\\"
      ) {
        throw new Error(
          "drive-type oracle requires a drive-letter current directory",
        );
      }
      const currentRoot = currentDirectory.slice(0, 2) + "\\";
      const currentRootPointer = Memory.allocUtf8String(currentRoot);
      const currentDirectoryPointer = currentDirectoryBuffer;
      const relativePointer = Memory.allocUtf8String(".");
      const invoke = ({ id, sentinel, rootPath, pointer }) => {
        driveTypeOracleSentinel = sentinel;
        driveTypeOracleResult = null;
        driveTypeOracleLastError = null;
        try {
          const outcome = getDriveType(pointer);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            driveTypeOracleResult === null ||
            driveTypeOracleLastError === null
          ) {
            throw new Error(
              "GetDriveTypeA oracle did not pass through the hook",
            );
          }
          return {
            id,
            sentinel,
            rootPath,
            result,
            hookResult: driveTypeOracleResult,
            hookLastErrorAfter: driveTypeOracleLastError,
            systemLastError,
            hookResultMatches: driveTypeOracleResult === result,
            hookPreservesLastError: driveTypeOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          driveTypeOracleSentinel = null;
          driveTypeOracleResult = null;
          driveTypeOracleLastError = null;
        }
      };
      const calls = [
        invoke({
          id: "current-root",
          sentinel: 0,
          rootPath: currentRoot,
          pointer: currentRootPointer,
        }),
        invoke({
          id: "null-current-drive",
          sentinel: 87,
          rootPath: null,
          pointer: NULL,
        }),
        invoke({
          id: "current-directory-not-root",
          sentinel: 122,
          rootPath: currentDirectory,
          pointer: currentDirectoryPointer,
        }),
        invoke({
          id: "relative-dot",
          sentinel: 0x12345678,
          rootPath: ".",
          pointer: relativePointer,
        }),
      ];
      emit("drive-type-oracle", {
        profile: "x86-current-drive-type-read-v1",
        currentDirectory,
        currentRoot,
        calls,
      });
    } catch (error) {
      emit("drive-type-oracle-error", { error: error.message });
    }
  });
}

function scheduleSwapMouseButtonStateObserver(module) {
  if (swapMouseButtonStateScheduled) {
    return;
  }
  swapMouseButtonStateScheduled = true;
  setImmediate(() => {
    try {
      const swapAddress = findExport(module, "SwapMouseButton");
      const metricsAddress = findExport(module, "GetSystemMetrics");
      if (swapAddress === null || metricsAddress === null) {
        throw new Error(
          "SwapMouseButton state-observer exports are unavailable",
        );
      }
      swapMouseButtonGetSystemMetrics = new NativeFunction(
        metricsAddress,
        "int32",
        ["int32"],
        "stdcall",
      );
      swapMouseButtonRestore = new SystemFunction(
        swapAddress,
        "uint32",
        ["uint32"],
        "stdcall",
      );
      swapMouseButtonInitialState = swapMouseButtonGetSystemMetrics(23) !== 0;
      emit("swap-mouse-button-initial-state", {
        profile: "x86-process-observed-system-button-mapping-v1",
        swapped: swapMouseButtonInitialState,
      });
    } catch (error) {
      emit("swap-mouse-button-state-observer-error", { error: error.message });
    }
  });
}

function completeSwapMouseButtonObservation(calls) {
  try {
    if (
      swapMouseButtonInitialState === null ||
      swapMouseButtonGetSystemMetrics === null ||
      swapMouseButtonRestore === null
    ) {
      throw new Error(
        "SwapMouseButton initial state was not captured before the BGI call",
      );
    }
    if (calls.length !== 2) {
      throw new Error("SwapMouseButton completion requires the two BGI calls");
    }
    const stateAfter = calls[1].stateAfter;
    let restoration = null;
    if (stateAfter !== swapMouseButtonInitialState) {
      swapMouseButtonRestoreActive = true;
      try {
        const outcome = swapMouseButtonRestore(
          swapMouseButtonInitialState ? 1 : 0,
        );
        restoration = {
          requestedState: swapMouseButtonInitialState,
          result: Number(outcome.value) >>> 0,
          lastError: Number(outcome.lastError) >>> 0,
        };
      } finally {
        swapMouseButtonRestoreActive = false;
      }
    }
    const finalState = swapMouseButtonGetSystemMetrics(23) !== 0;
    emit("swap-mouse-button-complete", {
      profile: "x86-bgi-normal-button-mapping-v1",
      initialState: swapMouseButtonInitialState,
      calls,
      firstResultRestoredBySecondArgument:
        calls[1].argument === calls[0].result,
      restoration,
      finalState,
      restored: finalState === swapMouseButtonInitialState,
    });
    if (stopOnEvent === "swap-mouse-button-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  } catch (error) {
    emit("swap-mouse-button-completion-error", { error: error.message, calls });
  }
}

function maybeCompleteTimeGetDevCapsObservation() {
  if (
    timeGetDevCapsCompletionRecorded ||
    !timeGetDevCapsOracleComplete ||
    timeGetDevCapsMainModuleCalls.length === 0 ||
    !timePeriodMainModuleCalls.some((call) => call.api === "timeBeginPeriod")
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteTimeGetDevCapsObservation, 10);
    return;
  }
  if (timeGetDevCapsDrainCheckScheduled) {
    return;
  }
  timeGetDevCapsDrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    timeGetDevCapsDrainCheckScheduled = false;
    if (timeGetDevCapsCompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteTimeGetDevCapsObservation();
      return;
    }
    timeGetDevCapsCompletionRecorded = true;
    emit("time-get-dev-caps-complete", {
      profile: "x86-bgi-time-get-dev-caps-v1",
      oracleProfile: timeGetDevCapsOracleEvidence.profile,
      periodOracleProfile: timePeriodOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: timeGetDevCapsMainModuleCalls.map((call) => ({
        ...call,
      })),
      timePeriodCalls: timePeriodMainModuleCalls.map((call) => ({
        ...call,
      })),
    });
    if (stopOnEvent === "time-get-dev-caps-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function maybeCompleteQueryPerformanceFrequencyObservation() {
  if (
    queryPerformanceFrequencyCompletionRecorded ||
    !queryPerformanceFrequencyOracleComplete ||
    queryPerformanceFrequencyMainModuleCalls.length === 0
  ) {
    return;
  }
  queryPerformanceFrequencyCompletionRecorded = true;
  emit("query-performance-frequency-complete", {
    profile: "x86-bgi-query-performance-frequency-v1",
    oracleProfile: queryPerformanceFrequencyOracleEvidence.profile,
    mainModuleCalls: queryPerformanceFrequencyMainModuleCalls.map((call) => ({
      ...call,
    })),
  });
  if (stopOnEvent === "query-performance-frequency-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteQueryPerformanceCounterObservation() {
  if (
    queryPerformanceCounterCompletionRecorded ||
    !queryPerformanceCounterOracleComplete ||
    queryPerformanceCounterMainModuleCalls.filter(
      (call) => call.callerRva === 0x00092017,
    ).length < 2
  ) {
    return;
  }
  queryPerformanceCounterCompletionRecorded = true;
  emit("query-performance-counter-complete", {
    profile: "x86-bgi-query-performance-counter-progression-v1",
    oracleProfile: queryPerformanceCounterOracleEvidence.profile,
    mainModuleCalls: queryPerformanceCounterMainModuleCalls.map((call) => ({
      ...call,
    })),
  });
  if (stopOnEvent === "query-performance-counter-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function attachQueryPerformanceCounterHook(module) {
  const name = "QueryPerformanceCounter";
  const address = findExport(module, name);
  if (address === null) {
    throw new Error("QueryPerformanceCounter export is unavailable");
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      this.mainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      this.oracleCaller =
        callerModule === "frida-agent.dll" &&
        queryPerformanceCounterOracleCall !== null;
      if (!this.mainModuleCaller && !this.oracleCaller) {
        this.traceFrame = null;
        return;
      }
      if (this.oracleCaller) {
        this.lastError = queryPerformanceCounterOracleCall.sentinel;
      }
      this.caller = caller;
      this.output = args[0];
      this.outputBefore = snapshotUnsignedLargeInteger(this.output);
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = this.mainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              96,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              6,
              32,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.output.toString()],
          lastErrorBefore: this.lastErrorBefore,
          queryPerformanceCounter: {
            outputBefore: this.outputBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const value = result.toUInt32();
      const lastError = Number(this.lastError) >>> 0;
      const outputAfter = snapshotUnsignedLargeInteger(this.output);
      if (this.oracleCaller) {
        queryPerformanceCounterOracleCall.hookResult = value;
        queryPerformanceCounterOracleCall.hookLastErrorAfter = lastError;
        queryPerformanceCounterOracleCall.hookOutput = outputAfter;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          queryPerformanceCounter: {
            result: value,
            outputAfter,
          },
        },
        this.lastError,
      );
      if (
        this.mainModuleCaller &&
        queryPerformanceCounterMainModuleCalls.length < 32
      ) {
        queryPerformanceCounterMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          callerRva: this.caller.rva,
          outputPointer: this.output.toString(),
          result: value,
          outputBefore: this.outputBefore,
          outputAfter,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        setImmediate(maybeCompleteQueryPerformanceCounterObservation);
      }
    },
  });
}

function scheduleQueryPerformanceCounterOracle(module) {
  if (queryPerformanceCounterOracleScheduled) {
    return;
  }
  queryPerformanceCounterOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "QueryPerformanceCounter");
      if (address === null) {
        throw new Error("QueryPerformanceCounter export is unavailable");
      }
      const query = new SystemFunction(
        address,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const invoke = (sentinel) => {
        const guardBytes = 16;
        const outputBytes = 8;
        const storageBytes = guardBytes + outputBytes + guardBytes;
        const storage = Memory.alloc(storageBytes);
        storage.writeByteArray(Array(storageBytes).fill(0xa5));
        const output = storage.add(guardBytes);
        const storageBefore = snapshotBoundedRawBytes(storage, storageBytes);
        queryPerformanceCounterOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
          hookOutput: null,
        };
        try {
          const outcome = query(output);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            queryPerformanceCounterOracleCall.hookResult === null ||
            queryPerformanceCounterOracleCall.hookLastErrorAfter === null ||
            queryPerformanceCounterOracleCall.hookOutput === null
          ) {
            throw new Error(
              "QueryPerformanceCounter oracle did not pass through the hook",
            );
          }
          const storageAfter = snapshotBoundedRawBytes(storage, storageBytes);
          const outputAfter = snapshotUnsignedLargeInteger(output);
          const afterBytes = storageAfter.bytes ?? [];
          return {
            sentinel,
            systemResult,
            hookResult: queryPerformanceCounterOracleCall.hookResult,
            outputAfter,
            hookOutput: queryPerformanceCounterOracleCall.hookOutput,
            hookLastErrorAfter:
              queryPerformanceCounterOracleCall.hookLastErrorAfter,
            systemLastError,
            storageOffset: guardBytes,
            storageBefore,
            storageAfter,
            prefixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes.slice(0, guardBytes).every((byte) => byte === 0xa5),
            suffixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes
                .slice(guardBytes + outputBytes)
                .every((byte) => byte === 0xa5),
            hookResultMatches:
              queryPerformanceCounterOracleCall.hookResult === systemResult,
            hookOutputMatches:
              queryPerformanceCounterOracleCall.hookOutput.value ===
              outputAfter.value,
            hookPreservesLastError:
              queryPerformanceCounterOracleCall.hookLastErrorAfter === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          queryPerformanceCounterOracleCall = null;
        }
      };
      const calls = [0, 87, 0x12345678, 0x87654321].map(invoke);
      const deltas = calls
        .slice(1)
        .map((call, index) =>
          Number(
            BigInt(call.outputAfter.value) -
              BigInt(calls[index].outputAfter.value),
          ),
        );
      queryPerformanceCounterOracleEvidence = {
        profile: "x86-monotonic-performance-counter-v1",
        calls,
        deltas,
        allSucceeded: calls.every(
          (call) =>
            call.systemResult !== 0 &&
            call.hookResultMatches &&
            call.hookOutputMatches,
        ),
        strictlyIncreasing: deltas.every((delta) => delta > 0),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
        allGuardsUnchanged: calls.every(
          (call) => call.prefixGuardUnchanged && call.suffixGuardUnchanged,
        ),
      };
      queryPerformanceCounterOracleComplete = true;
      emit(
        "query-performance-counter-oracle",
        queryPerformanceCounterOracleEvidence,
      );
      maybeCompleteQueryPerformanceCounterObservation();
    } catch (error) {
      emit("query-performance-counter-oracle-error", {
        error: error.message,
      });
    }
  });
}

function attachQueryPerformanceFrequencyHook(module) {
  const name = "QueryPerformanceFrequency";
  const address = findExport(module, name);
  if (address === null) {
    throw new Error("QueryPerformanceFrequency export is unavailable");
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      this.mainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      this.oracleCaller =
        callerModule === "frida-agent.dll" &&
        queryPerformanceFrequencyOracleCall !== null;
      if (!this.mainModuleCaller && !this.oracleCaller) {
        this.traceFrame = null;
        return;
      }
      if (this.oracleCaller) {
        this.lastError = queryPerformanceFrequencyOracleCall.sentinel;
      }
      this.output = args[0];
      this.outputBefore = snapshotUnsignedLargeInteger(this.output);
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = this.mainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              96,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              6,
              32,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [this.output.toString()],
          lastErrorBefore: this.lastErrorBefore,
          queryPerformanceFrequency: {
            outputBefore: this.outputBefore,
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const value = result.toUInt32();
      const lastError = Number(this.lastError) >>> 0;
      const outputAfter = snapshotUnsignedLargeInteger(this.output);
      if (this.oracleCaller) {
        queryPerformanceFrequencyOracleCall.hookResult = value;
        queryPerformanceFrequencyOracleCall.hookLastErrorAfter = lastError;
        queryPerformanceFrequencyOracleCall.hookOutput = outputAfter;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          queryPerformanceFrequency: {
            result: value,
            outputAfter,
          },
        },
        this.lastError,
      );
      if (
        this.mainModuleCaller &&
        queryPerformanceFrequencyMainModuleCalls.length === 0
      ) {
        queryPerformanceFrequencyMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          outputPointer: this.output.toString(),
          result: value,
          outputBefore: this.outputBefore,
          outputAfter,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        setImmediate(maybeCompleteQueryPerformanceFrequencyObservation);
      }
    },
  });
}

function scheduleQueryPerformanceFrequencyOracle(module) {
  if (queryPerformanceFrequencyOracleScheduled) {
    return;
  }
  queryPerformanceFrequencyOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "QueryPerformanceFrequency");
      if (address === null) {
        throw new Error("QueryPerformanceFrequency export is unavailable");
      }
      const query = new SystemFunction(
        address,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const invoke = (sentinel) => {
        const guardBytes = 16;
        const outputBytes = 8;
        const storageBytes = guardBytes + outputBytes + guardBytes;
        const storage = Memory.alloc(storageBytes);
        storage.writeByteArray(Array(storageBytes).fill(0xa5));
        const output = storage.add(guardBytes);
        const storageBefore = snapshotBoundedRawBytes(storage, storageBytes);
        queryPerformanceFrequencyOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
          hookOutput: null,
        };
        try {
          const outcome = query(output);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            queryPerformanceFrequencyOracleCall.hookResult === null ||
            queryPerformanceFrequencyOracleCall.hookLastErrorAfter === null ||
            queryPerformanceFrequencyOracleCall.hookOutput === null
          ) {
            throw new Error(
              "QueryPerformanceFrequency oracle did not pass through the hook",
            );
          }
          const storageAfter = snapshotBoundedRawBytes(storage, storageBytes);
          const outputAfter = snapshotUnsignedLargeInteger(output);
          const afterBytes = storageAfter.bytes ?? [];
          return {
            sentinel,
            systemResult,
            hookResult: queryPerformanceFrequencyOracleCall.hookResult,
            outputAfter,
            hookOutput: queryPerformanceFrequencyOracleCall.hookOutput,
            hookLastErrorAfter:
              queryPerformanceFrequencyOracleCall.hookLastErrorAfter,
            systemLastError,
            storageOffset: guardBytes,
            storageBefore,
            storageAfter,
            prefixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes.slice(0, guardBytes).every((byte) => byte === 0xa5),
            suffixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes
                .slice(guardBytes + outputBytes)
                .every((byte) => byte === 0xa5),
            hookResultMatches:
              queryPerformanceFrequencyOracleCall.hookResult === systemResult,
            hookOutputMatches:
              queryPerformanceFrequencyOracleCall.hookOutput.value ===
              outputAfter.value,
            hookPreservesLastError:
              queryPerformanceFrequencyOracleCall.hookLastErrorAfter ===
              sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          queryPerformanceFrequencyOracleCall = null;
        }
      };
      const calls = [0, 87, 0x12345678].map(invoke);
      queryPerformanceFrequencyOracleEvidence = {
        profile: "x86-stable-performance-frequency-v1",
        calls,
        frequency: calls[0].outputAfter.value,
        allSucceeded: calls.every(
          (call) =>
            call.systemResult !== 0 &&
            call.hookResultMatches &&
            call.hookOutputMatches,
        ),
        allStable: calls.every(
          (call) => call.outputAfter.value === calls[0].outputAfter.value,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
        allGuardsUnchanged: calls.every(
          (call) => call.prefixGuardUnchanged && call.suffixGuardUnchanged,
        ),
      };
      queryPerformanceFrequencyOracleComplete = true;
      emit(
        "query-performance-frequency-oracle",
        queryPerformanceFrequencyOracleEvidence,
      );
      maybeCompleteQueryPerformanceFrequencyObservation();
    } catch (error) {
      emit("query-performance-frequency-oracle-error", {
        error: error.message,
      });
    }
  });
}

function maybeCompleteTimeGetTimeObservation() {
  if (
    timeGetTimeCompletionRecorded ||
    !timeGetTimeOracleComplete ||
    timeGetTimeMainModuleCalls.length === 0
  ) {
    return;
  }
  timeGetTimeCompletionRecorded = true;
  emit("time-get-time-complete", {
    profile: "x86-bgi-time-get-time-v1",
    oracleProfile: timeGetTimeOracleEvidence.profile,
    mainModuleCalls: timeGetTimeMainModuleCalls.map((call) => ({ ...call })),
  });
  if (stopOnEvent === "time-get-time-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function attachTimeGetTimeHook(module) {
  const name = "timeGetTime";
  const address = findExport(module, name);
  if (address === null) {
    throw new Error("timeGetTime export is unavailable");
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter() {
      const caller = describeCaller(this.returnAddress);
      const callerModule = caller.module?.toLowerCase() ?? null;
      this.mainModuleCaller =
        callerModule === Process.mainModule.name.toLowerCase();
      this.oracleCaller =
        callerModule === "frida-agent.dll" && timeGetTimeOracleCall !== null;
      if (!this.mainModuleCaller && !this.oracleCaller) {
        this.traceFrame = null;
        return;
      }
      if (this.oracleCaller) {
        this.lastError = timeGetTimeOracleCall.sentinel;
      }
      this.lastErrorBefore = Number(this.lastError) >>> 0;
      this.immediateConsumer = this.mainModuleCaller
        ? {
            apiContinuation: snapshotInstructionsFollowingDirectJump(
              this.returnAddress,
              96,
            ),
            frameCallerContinuations: snapshotX86FrameCallerContinuations(
              this.context,
              6,
              20,
            ),
          }
        : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: [],
          lastErrorBefore: this.lastErrorBefore,
          timeGetTime: {
            immediateConsumer: this.immediateConsumer,
          },
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      if (this.traceFrame === null) {
        return;
      }
      const value = result.toUInt32();
      const lastError = Number(this.lastError) >>> 0;
      if (this.oracleCaller) {
        timeGetTimeOracleCall.hookResult = value;
        timeGetTimeOracleCall.hookLastErrorAfter = lastError;
      }
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          timeGetTime: { result: value },
        },
        this.lastError,
      );
      if (this.mainModuleCaller && timeGetTimeMainModuleCalls.length === 0) {
        timeGetTimeMainModuleCalls.push({
          callId: this.traceFrame.callId,
          returnAddress: this.returnAddress.toString(),
          result: value,
          lastErrorBefore: this.lastErrorBefore,
          lastError,
          immediateConsumer: this.immediateConsumer,
        });
        setImmediate(maybeCompleteTimeGetTimeObservation);
      }
    },
  });
}

function scheduleTimeGetTimeOracle(module) {
  if (timeGetTimeOracleScheduled) {
    return;
  }
  timeGetTimeOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "timeGetTime");
      const kernel32 = Process.getModuleByName("kernel32.dll");
      const getTickCountAddress = findExport(kernel32, "GetTickCount");
      if (address === null || getTickCountAddress === null) {
        throw new Error("timeGetTime or GetTickCount export is unavailable");
      }
      const timeGetTime = new SystemFunction(address, "uint32", [], "stdcall");
      const getTickCount = new SystemFunction(
        getTickCountAddress,
        "uint32",
        [],
        "stdcall",
      );
      const invoke = (sentinel) => {
        const tickCountBefore = Number(getTickCount().value) >>> 0;
        timeGetTimeOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = timeGetTime();
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          const tickCountAfter = Number(getTickCount().value) >>> 0;
          if (
            timeGetTimeOracleCall.hookResult === null ||
            timeGetTimeOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error("timeGetTime oracle did not pass through the hook");
          }
          const tickCountSpan = (tickCountAfter - tickCountBefore) >>> 0;
          const resultOffset = (systemResult - tickCountBefore) >>> 0;
          return {
            sentinel,
            tickCountBefore,
            systemResult,
            tickCountAfter,
            tickCountSpan,
            resultOffset,
            hookResult: timeGetTimeOracleCall.hookResult,
            hookLastErrorAfter: timeGetTimeOracleCall.hookLastErrorAfter,
            systemLastError,
            resultWithinTickCountBracket: resultOffset <= tickCountSpan,
            hookResultMatches:
              timeGetTimeOracleCall.hookResult === systemResult,
            hookPreservesLastError:
              timeGetTimeOracleCall.hookLastErrorAfter === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          timeGetTimeOracleCall = null;
        }
      };
      const calls = [0, 87, 0x12345678].map(invoke);
      timeGetTimeOracleEvidence = {
        profile: "x86-time-get-time-shared-tick-epoch-v1",
        calls,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
        allResultsWithinTickCountBrackets: calls.every(
          (call) => call.resultWithinTickCountBracket,
        ),
      };
      timeGetTimeOracleComplete = true;
      emit("time-get-time-oracle", timeGetTimeOracleEvidence);
      maybeCompleteTimeGetTimeObservation();
    } catch (error) {
      emit("time-get-time-oracle-error", { error: error.message });
    }
  });
}

function scheduleTimeGetDevCapsOracle(module) {
  if (timeGetDevCapsOracleScheduled) {
    return;
  }
  timeGetDevCapsOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "timeGetDevCaps");
      if (address === null) {
        throw new Error("timeGetDevCaps export is unavailable");
      }
      const timeGetDevCaps = new SystemFunction(
        address,
        "uint32",
        ["pointer", "uint32"],
        "stdcall",
      );
      const invoke = ({ size, sentinel }) => {
        const guardBytes = 16;
        const outputCapacity = 16;
        const storageBytes = guardBytes + outputCapacity + guardBytes;
        const storage = Memory.alloc(storageBytes);
        storage.writeByteArray(Array(storageBytes).fill(0xa5));
        const output = storage.add(guardBytes);
        const storageBefore = snapshotBoundedRawBytes(storage, storageBytes);
        timeGetDevCapsOracleCall = {
          size,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = timeGetDevCaps(output, size);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            timeGetDevCapsOracleCall.hookResult === null ||
            timeGetDevCapsOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              "timeGetDevCaps oracle did not pass through the hook",
            );
          }
          const storageAfter = snapshotBoundedRawBytes(storage, storageBytes);
          const beforeBytes = storageBefore.bytes ?? [];
          const afterBytes = storageAfter.bytes ?? [];
          const changedStorageOffsets =
            beforeBytes.length === storageBytes &&
            afterBytes.length === storageBytes
              ? beforeBytes.flatMap((value, index) =>
                  value === afterBytes[index] ? [] : [index],
                )
              : [];
          const changedOutputOffsets = changedStorageOffsets
            .filter(
              (offset) =>
                offset >= guardBytes && offset < guardBytes + outputCapacity,
            )
            .map((offset) => offset - guardBytes);
          return {
            size,
            sentinel,
            hookResult: timeGetDevCapsOracleCall.hookResult,
            systemResult,
            hookLastErrorAfter: timeGetDevCapsOracleCall.hookLastErrorAfter,
            systemLastError,
            storageOffset: guardBytes,
            outputCapacity,
            storageBefore,
            storageAfter,
            outputAfter: snapshotTimeCaps(output),
            outputBytesAfter: snapshotBoundedRawBytes(output, outputCapacity),
            changedOutputOffsets,
            prefixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes.slice(0, guardBytes).every((value) => value === 0xa5),
            suffixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes
                .slice(guardBytes + outputCapacity)
                .every((value) => value === 0xa5),
            bytesBeyondTimeCapsUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes
                .slice(guardBytes + 8, guardBytes + outputCapacity)
                .every((value) => value === 0xa5),
            hookResultMatches:
              timeGetDevCapsOracleCall.hookResult === systemResult,
            hookPreservesLastError:
              timeGetDevCapsOracleCall.hookLastErrorAfter === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          timeGetDevCapsOracleCall = null;
        }
      };
      const calls = [
        { size: 0, sentinel: 0x10203040 },
        { size: 4, sentinel: 0x11223344 },
        { size: 7, sentinel: 0x55667788 },
        { size: 8, sentinel: 0 },
        { size: 8, sentinel: 87 },
        { size: 8, sentinel: 0x12345678 },
        { size: 12, sentinel: 0x76543210 },
      ].map(invoke);
      const successfulCalls = calls.filter((call) => call.size >= 8);
      const referenceFields = successfulCalls[0]?.outputAfter.fields ?? null;
      if (referenceFields === null) {
        throw new Error(
          "timeGetDevCaps oracle did not produce timer capabilities",
        );
      }
      const beginAddress = findExport(module, "timeBeginPeriod");
      const endAddress = findExport(module, "timeEndPeriod");
      if (beginAddress === null || endAddress === null) {
        throw new Error("time period exports are unavailable");
      }
      const periodFunctions = {
        timeBeginPeriod: new SystemFunction(
          beginAddress,
          "uint32",
          ["uint32"],
          "stdcall",
        ),
        timeEndPeriod: new SystemFunction(
          endAddress,
          "uint32",
          ["uint32"],
          "stdcall",
        ),
      };
      const invokePeriod = ({ api, periodMilliseconds, sentinel }) => {
        timePeriodOracleCall = {
          api,
          periodMilliseconds,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = periodFunctions[api](periodMilliseconds);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            timePeriodOracleCall.hookResult === null ||
            timePeriodOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(`${api} oracle did not pass through the hook`);
          }
          return {
            api,
            periodMilliseconds,
            sentinel,
            hookResult: timePeriodOracleCall.hookResult,
            systemResult,
            hookLastErrorAfter: timePeriodOracleCall.hookLastErrorAfter,
            systemLastError,
            hookResultMatches: timePeriodOracleCall.hookResult === systemResult,
            hookPreservesLastError:
              timePeriodOracleCall.hookLastErrorAfter === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          timePeriodOracleCall = null;
        }
      };
      const minimum = referenceFields.periodMinimumMilliseconds;
      const maximum = referenceFields.periodMaximumMilliseconds;
      const differentValid = minimum < maximum ? minimum + 1 : minimum;
      const aboveMaximum = maximum === 0xffff_ffff ? maximum : maximum + 1;
      const periodCalls = [
        {
          api: "timeEndPeriod",
          periodMilliseconds: minimum,
          sentinel: 0x01020304,
        },
        { api: "timeBeginPeriod", periodMilliseconds: 0, sentinel: 0x11121314 },
        { api: "timeEndPeriod", periodMilliseconds: 0, sentinel: 0x21222324 },
        {
          api: "timeBeginPeriod",
          periodMilliseconds: aboveMaximum,
          sentinel: 0x31323334,
        },
        {
          api: "timeEndPeriod",
          periodMilliseconds: aboveMaximum,
          sentinel: 0x41424344,
        },
        {
          api: "timeBeginPeriod",
          periodMilliseconds: differentValid,
          sentinel: 0x45464748,
        },
        {
          api: "timeEndPeriod",
          periodMilliseconds: differentValid,
          sentinel: 0x494a4b4c,
        },
        {
          api: "timeBeginPeriod",
          periodMilliseconds: 0xffff_ffff,
          sentinel: 0x4d4e4f50,
        },
        {
          api: "timeEndPeriod",
          periodMilliseconds: 0xffff_ffff,
          sentinel: 0x55565758,
        },
        {
          api: "timeBeginPeriod",
          periodMilliseconds: minimum,
          sentinel: 0x51525354,
        },
        {
          api: "timeBeginPeriod",
          periodMilliseconds: minimum,
          sentinel: 0x61626364,
        },
        {
          api: "timeEndPeriod",
          periodMilliseconds: differentValid,
          sentinel: 0x71727374,
        },
        {
          api: "timeEndPeriod",
          periodMilliseconds: minimum,
          sentinel: 0x81828384,
        },
        {
          api: "timeEndPeriod",
          periodMilliseconds: minimum,
          sentinel: 0x91929394,
        },
        {
          api: "timeEndPeriod",
          periodMilliseconds: minimum,
          sentinel: 0xa1a2a3a4,
        },
        {
          api: "timeBeginPeriod",
          periodMilliseconds: maximum,
          sentinel: 0xb1b2b3b4,
        },
        {
          api: "timeEndPeriod",
          periodMilliseconds: maximum,
          sentinel: 0xc1c2c3c4,
        },
      ].map(invokePeriod);
      timePeriodOracleEvidence = {
        profile: "x86-time-period-range-and-pairing-v1",
        periodMinimumMilliseconds: minimum,
        periodMaximumMilliseconds: maximum,
        differentValidPeriodMilliseconds: differentValid,
        aboveMaximumPeriodMilliseconds: aboveMaximum,
        calls: periodCalls,
        allResultsMatchHooks: periodCalls.every(
          (call) => call.hookResultMatches,
        ),
        allPreserveLastError: periodCalls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      };
      timeGetDevCapsOracleEvidence = {
        profile: "x86-time-get-dev-caps-size-and-output-v1",
        calls,
        timeErrorNoError: 0,
        timeErrorNoCanDo: 97,
        shortSizesRejectedWithoutWrites: calls
          .filter((call) => call.size < 8)
          .every(
            (call) =>
              call.systemResult === 97 &&
              call.hookResultMatches &&
              call.changedOutputOffsets.length === 0,
          ),
        supportedSizesSucceeded: successfulCalls.every(
          (call) => call.systemResult === 0 && call.hookResultMatches,
        ),
        successfulOutputsStable: successfulCalls.every(
          (call) =>
            JSON.stringify(call.outputAfter.fields ?? null) ===
            JSON.stringify(referenceFields),
        ),
        allWritesBoundedToTimeCaps: successfulCalls.every(
          (call) =>
            call.prefixGuardUnchanged &&
            call.suffixGuardUnchanged &&
            call.bytesBeyondTimeCapsUnchanged &&
            call.changedOutputOffsets.every((offset) => offset < 8),
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
        periodLifecycle: timePeriodOracleEvidence,
      };
      timeGetDevCapsOracleComplete = true;
      emit("time-get-dev-caps-oracle", timeGetDevCapsOracleEvidence);
      maybeCompleteTimeGetDevCapsObservation();
    } catch (error) {
      emit("time-get-dev-caps-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteCreateWaitableTimerAObservation() {
  if (
    createWaitableTimerACompletionRecorded ||
    !createWaitableTimerAOracleComplete ||
    createWaitableTimerAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteCreateWaitableTimerAObservation, 10);
    return;
  }
  if (createWaitableTimerADrainCheckScheduled) {
    return;
  }
  createWaitableTimerADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    createWaitableTimerADrainCheckScheduled = false;
    if (createWaitableTimerACompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteCreateWaitableTimerAObservation();
      return;
    }
    createWaitableTimerACompletionRecorded = true;
    emit("create-waitable-timer-a-complete", {
      profile: "x86-bgi-create-waitable-timer-a-v1",
      oracleProfile: createWaitableTimerAOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: createWaitableTimerAMainModuleCalls.map((call) => ({
        ...call,
      })),
    });
    if (stopOnEvent === "create-waitable-timer-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleCreateWaitableTimerAOracle(module) {
  if (createWaitableTimerAOracleScheduled) {
    return;
  }
  createWaitableTimerAOracleScheduled = true;
  setImmediate(() => {
    try {
      const createAddress = findExport(module, "CreateWaitableTimerA");
      const waitAddress = findExport(module, "WaitForSingleObject");
      const closeAddress = findExport(module, "CloseHandle");
      if (
        createAddress === null ||
        waitAddress === null ||
        closeAddress === null
      ) {
        throw new Error("waitable timer oracle exports are unavailable");
      }
      const createWaitableTimerA = new SystemFunction(
        createAddress,
        "pointer",
        ["pointer", "uint32", "pointer"],
        "stdcall",
      );
      const waitForSingleObject = new SystemFunction(
        waitAddress,
        "uint32",
        ["pointer", "uint32"],
        "stdcall",
      );
      const closeHandle = new SystemFunction(
        closeAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const invoke = ({ manualReset, sentinel }) => {
        createWaitableTimerAOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const createOutcome = createWaitableTimerA(
            ptr(0),
            manualReset ? 1 : 0,
            ptr(0),
          );
          const handle = createOutcome.value;
          const systemLastError = Number(createOutcome.lastError) >>> 0;
          if (
            createWaitableTimerAOracleCall.hookResult === null ||
            createWaitableTimerAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              "CreateWaitableTimerA oracle did not pass through the hook",
            );
          }
          if (handle.isNull()) {
            throw new Error("CreateWaitableTimerA oracle returned null");
          }
          const waitOutcome = waitForSingleObject(handle, 0);
          const closeOutcome = closeHandle(handle);
          return {
            manualReset,
            sentinel,
            handleNonNull: true,
            hookResult: createWaitableTimerAOracleCall.hookResult,
            systemResult: handle.toString(),
            hookLastErrorAfter:
              createWaitableTimerAOracleCall.hookLastErrorAfter,
            systemLastError,
            initialWaitResult: Number(waitOutcome.value) >>> 0,
            closeResult: Number(closeOutcome.value) >>> 0,
            hookResultMatches:
              createWaitableTimerAOracleCall.hookResult === handle.toString(),
            hookAndSystemLastErrorMatch:
              createWaitableTimerAOracleCall.hookLastErrorAfter ===
              systemLastError,
          };
        } finally {
          createWaitableTimerAOracleCall = null;
        }
      };
      const calls = [
        { manualReset: true, sentinel: 0x11223344 },
        { manualReset: false, sentinel: 0x55667788 },
      ].map(invoke);
      createWaitableTimerAOracleEvidence = {
        profile: "x86-create-waitable-timer-initial-state-v1",
        waitObject0: 0,
        waitTimeout: 258,
        calls,
        allCreated: calls.every((call) => call.handleNonNull),
        allInitiallyNonsignaled: calls.every(
          (call) => call.initialWaitResult === 258,
        ),
        allClosed: calls.every((call) => call.closeResult !== 0),
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      createWaitableTimerAOracleComplete = true;
      emit(
        "create-waitable-timer-a-oracle",
        createWaitableTimerAOracleEvidence,
      );
      maybeCompleteCreateWaitableTimerAObservation();
    } catch (error) {
      emit("create-waitable-timer-a-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteSetWaitableTimerObservation() {
  if (
    setWaitableTimerCompletionRecorded ||
    !setWaitableTimerOracleComplete ||
    setWaitableTimerMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteSetWaitableTimerObservation, 10);
    return;
  }
  if (setWaitableTimerDrainCheckScheduled) {
    return;
  }
  setWaitableTimerDrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    setWaitableTimerDrainCheckScheduled = false;
    if (setWaitableTimerCompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteSetWaitableTimerObservation();
      return;
    }
    setWaitableTimerCompletionRecorded = true;
    emit("set-waitable-timer-complete", {
      profile: "x86-bgi-set-waitable-timer-v1",
      oracleProfile: setWaitableTimerOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: setWaitableTimerMainModuleCalls.map((call) => ({
        ...call,
      })),
    });
    if (stopOnEvent === "set-waitable-timer-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function writeSignedLargeInteger(pointer, value) {
  let encoded = value;
  if (encoded < 0n) {
    encoded += 0x10000000000000000n;
  }
  const bytes = [];
  for (let index = 0n; index < 8n; index += 1n) {
    bytes.push(Number((encoded >> (index * 8n)) & 0xffn));
  }
  pointer.writeByteArray(bytes);
}

function scheduleSetWaitableTimerOracle(module) {
  if (setWaitableTimerOracleScheduled) {
    return;
  }
  setWaitableTimerOracleScheduled = true;
  setImmediate(() => {
    try {
      const createAddress = findExport(module, "CreateWaitableTimerA");
      const setAddress = findExport(module, "SetWaitableTimer");
      const waitAddress = findExport(module, "WaitForSingleObject");
      const closeAddress = findExport(module, "CloseHandle");
      if (
        createAddress === null ||
        setAddress === null ||
        waitAddress === null ||
        closeAddress === null
      ) {
        throw new Error("set waitable timer oracle exports are unavailable");
      }
      const createWaitableTimerA = new SystemFunction(
        createAddress,
        "pointer",
        ["pointer", "uint32", "pointer"],
        "stdcall",
      );
      const setWaitableTimer = new SystemFunction(
        setAddress,
        "uint32",
        ["pointer", "pointer", "int32", "pointer", "pointer", "uint32"],
        "stdcall",
      );
      const waitForSingleObject = new SystemFunction(
        waitAddress,
        "uint32",
        ["pointer", "uint32"],
        "stdcall",
      );
      const closeHandle = new SystemFunction(
        closeAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const relativeDueTime100ns = -10000n;
      const dueTime = Memory.alloc(8);
      writeSignedLargeInteger(dueTime, relativeDueTime100ns);
      const invoke = ({ manualReset, sentinel }) => {
        const createOutcome = createWaitableTimerA(
          ptr(0),
          manualReset ? 1 : 0,
          ptr(0),
        );
        const handle = createOutcome.value;
        if (handle.isNull()) {
          throw new Error("set waitable timer oracle could not create a timer");
        }
        setWaitableTimerOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const setOutcome = setWaitableTimer(
            handle,
            dueTime,
            0,
            ptr(0),
            ptr(0),
            0,
          );
          const firstWait = waitForSingleObject(handle, 1000);
          const secondWait = waitForSingleObject(handle, 0);
          return {
            manualReset,
            sentinel,
            relativeDueTime100ns: relativeDueTime100ns.toString(),
            setResult: Number(setOutcome.value) >>> 0,
            systemLastError: Number(setOutcome.lastError) >>> 0,
            hookResult: setWaitableTimerOracleCall.hookResult,
            hookLastErrorAfter: setWaitableTimerOracleCall.hookLastErrorAfter,
            firstWaitResult: Number(firstWait.value) >>> 0,
            secondWaitResult: Number(secondWait.value) >>> 0,
            closeResult: Number(closeHandle(handle).value) >>> 0,
          };
        } finally {
          setWaitableTimerOracleCall = null;
        }
      };
      const calls = [
        { manualReset: true, sentinel: 0x11223344 },
        { manualReset: false, sentinel: 0x55667788 },
      ].map(invoke);

      setWaitableTimerOracleCall = {
        sentinel: 0x778899aa,
        hookResult: null,
        hookLastErrorAfter: null,
      };
      const invalidOutcome = setWaitableTimer(
        ptr(0x1234),
        dueTime,
        0,
        ptr(0),
        ptr(0),
        0,
      );
      const invalidCall = {
        sentinel: setWaitableTimerOracleCall.sentinel,
        result: Number(invalidOutcome.value) >>> 0,
        systemLastError: Number(invalidOutcome.lastError) >>> 0,
        hookResult: setWaitableTimerOracleCall.hookResult,
        hookLastErrorAfter: setWaitableTimerOracleCall.hookLastErrorAfter,
      };
      setWaitableTimerOracleCall = null;

      setWaitableTimerOracleEvidence = {
        profile: "x86-set-waitable-timer-relative-one-shot-v1",
        relativeDueTime100ns: relativeDueTime100ns.toString(),
        waitObject0: 0,
        waitTimeout: 258,
        calls,
        invalidCall,
        allSetSucceeded: calls.every((call) => call.setResult !== 0),
        manualResetRemainsSignaled:
          calls[0].firstWaitResult === 0 && calls[0].secondWaitResult === 0,
        autoResetConsumesSignal:
          calls[1].firstWaitResult === 0 && calls[1].secondWaitResult === 258,
        allResultsMatchHooks: calls.every(
          (call) => call.hookResult === call.setResult,
        ),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookLastErrorAfter === call.systemLastError,
        ),
        invalidHandleRejected:
          invalidCall.result === 0 && invalidCall.systemLastError === 6,
        invalidResultMatchesHook:
          invalidCall.hookResult === invalidCall.result &&
          invalidCall.hookLastErrorAfter === invalidCall.systemLastError,
      };
      setWaitableTimerOracleComplete = true;
      emit("set-waitable-timer-oracle", setWaitableTimerOracleEvidence);
      maybeCompleteSetWaitableTimerObservation();
    } catch (error) {
      setWaitableTimerOracleCall = null;
      emit("set-waitable-timer-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteWaitForSingleObjectExObservation() {
  if (
    waitForSingleObjectExCompletionRecorded ||
    !waitForSingleObjectExOracleComplete ||
    waitForSingleObjectExMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteWaitForSingleObjectExObservation, 10);
    return;
  }
  if (waitForSingleObjectExDrainCheckScheduled) {
    return;
  }
  waitForSingleObjectExDrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    waitForSingleObjectExDrainCheckScheduled = false;
    if (waitForSingleObjectExCompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteWaitForSingleObjectExObservation();
      return;
    }
    waitForSingleObjectExCompletionRecorded = true;
    emit("wait-for-single-object-ex-complete", {
      profile: "x86-bgi-waitable-timer-wait-v1",
      oracleProfile: waitForSingleObjectExOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: waitForSingleObjectExMainModuleCalls.map((call) => ({
        ...call,
      })),
    });
    if (stopOnEvent === "wait-for-single-object-ex-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleWaitForSingleObjectExOracle(module) {
  if (waitForSingleObjectExOracleScheduled) {
    return;
  }
  waitForSingleObjectExOracleScheduled = true;
  setImmediate(() => {
    try {
      const createAddress = findExport(module, "CreateWaitableTimerA");
      const setAddress = findExport(module, "SetWaitableTimer");
      const waitAddress = findExport(module, "WaitForSingleObjectEx");
      const closeAddress = findExport(module, "CloseHandle");
      if (
        createAddress === null ||
        setAddress === null ||
        waitAddress === null ||
        closeAddress === null
      ) {
        throw new Error(
          "wait for single object ex oracle exports are unavailable",
        );
      }
      const createWaitableTimerA = new SystemFunction(
        createAddress,
        "pointer",
        ["pointer", "uint32", "pointer"],
        "stdcall",
      );
      const setWaitableTimer = new SystemFunction(
        setAddress,
        "uint32",
        ["pointer", "pointer", "int32", "pointer", "pointer", "uint32"],
        "stdcall",
      );
      const waitForSingleObjectEx = new SystemFunction(
        waitAddress,
        "uint32",
        ["pointer", "uint32", "uint32"],
        "stdcall",
      );
      const closeHandle = new SystemFunction(
        closeAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const invokeWait = (handle, timeoutMilliseconds, alertable, sentinel) => {
        waitForSingleObjectExOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = waitForSingleObjectEx(
            handle,
            timeoutMilliseconds,
            alertable ? 1 : 0,
          );
          return {
            handle: handle.toString(),
            timeoutMilliseconds,
            alertable,
            sentinel,
            result: Number(outcome.value) >>> 0,
            systemLastError: Number(outcome.lastError) >>> 0,
            hookResult: waitForSingleObjectExOracleCall.hookResult,
            hookLastErrorAfter:
              waitForSingleObjectExOracleCall.hookLastErrorAfter,
          };
        } finally {
          waitForSingleObjectExOracleCall = null;
        }
      };

      const relativeDueTime100ns = -5000n;
      const dueTime = Memory.alloc(8);
      writeSignedLargeInteger(dueTime, relativeDueTime100ns);
      const armedTimer = createWaitableTimerA(ptr(0), 1, ptr(0)).value;
      if (armedTimer.isNull()) {
        throw new Error(
          "wait for single object ex oracle could not create an armed timer",
        );
      }
      const setOutcome = setWaitableTimer(
        armedTimer,
        dueTime,
        0,
        ptr(0),
        ptr(0),
        0,
      );
      const armedWait = invokeWait(armedTimer, 10, true, 0x11223344);
      const repeatedWait = invokeWait(armedTimer, 0, true, 0x22334455);
      const armedCloseResult = Number(closeHandle(armedTimer).value) >>> 0;

      const nonsignaledTimer = createWaitableTimerA(ptr(0), 1, ptr(0)).value;
      if (nonsignaledTimer.isNull()) {
        throw new Error(
          "wait for single object ex oracle could not create a timeout timer",
        );
      }
      const timeoutWait = invokeWait(nonsignaledTimer, 0, true, 0x33445566);
      const nonsignaledCloseResult =
        Number(closeHandle(nonsignaledTimer).value) >>> 0;
      const invalidWait = invokeWait(ptr(0x1234), 10, true, 0x44556677);

      waitForSingleObjectExOracleEvidence = {
        profile: "x86-wait-for-single-object-ex-timer-v1",
        relativeDueTime100ns: relativeDueTime100ns.toString(),
        setResult: Number(setOutcome.value) >>> 0,
        setLastError: Number(setOutcome.lastError) >>> 0,
        armedWait,
        repeatedWait,
        timeoutWait,
        invalidWait,
        armedCloseResult,
        nonsignaledCloseResult,
        waitObject0: 0,
        waitTimeout: 258,
        waitFailed: 0xffffffff,
        errorInvalidHandle: 6,
        armedTimerSatisfied:
          armedWait.result === 0 && repeatedWait.result === 0,
        zeroTimeoutObserved: timeoutWait.result === 258,
        invalidHandleRejected:
          invalidWait.result === 0xffffffff &&
          invalidWait.systemLastError === 6,
        successAndTimeoutPreserveLastError:
          armedWait.systemLastError === armedWait.sentinel &&
          repeatedWait.systemLastError === repeatedWait.sentinel &&
          timeoutWait.systemLastError === timeoutWait.sentinel,
        allResultsMatchHooks: [
          armedWait,
          repeatedWait,
          timeoutWait,
          invalidWait,
        ].every((call) => call.hookResult === call.result),
        allLastErrorsMatchHooks: [
          armedWait,
          repeatedWait,
          timeoutWait,
          invalidWait,
        ].every((call) => call.hookLastErrorAfter === call.systemLastError),
      };
      waitForSingleObjectExOracleComplete = true;
      emit(
        "wait-for-single-object-ex-oracle",
        waitForSingleObjectExOracleEvidence,
      );
      maybeCompleteWaitForSingleObjectExObservation();
    } catch (error) {
      waitForSingleObjectExOracleCall = null;
      emit("wait-for-single-object-ex-oracle-error", {
        error: error.message,
      });
    }
  });
}

function maybeCompleteSetTimerObservation() {
  if (
    setTimerCompletionRecorded ||
    setTimerMainModuleCalls.length === 0 ||
    setTimerCallbackCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteSetTimerObservation, 10);
    return;
  }
  if (setTimerDrainCheckScheduled) {
    return;
  }
  setTimerDrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  const expectedCallbackCount = setTimerCallbackCalls.length;
  setTimeout(() => {
    setTimerDrainCheckScheduled = false;
    if (setTimerCompletionRecorded) {
      return;
    }
    if (
      activeCallsByThread.size !== 0 ||
      nextCallId !== expectedNextCallId ||
      setTimerCallbackCalls.length !== expectedCallbackCount
    ) {
      maybeCompleteSetTimerObservation();
      return;
    }
    setTimerCompletionRecorded = true;
    emit("set-timer-complete", {
      profile: "x86-bgi-window-timer-registration-and-first-callback-v1",
      quiescenceMilliseconds: 250,
      mainModuleCalls: setTimerMainModuleCalls.map((call) => ({ ...call })),
      callbackCalls: setTimerCallbackCalls.map((call) => ({ ...call })),
      killTimerCalls: killTimerMainModuleCalls.map((call) => ({ ...call })),
    });
    if (stopOnEvent === "set-timer-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function maybeCompleteSendMessageAObservation() {
  if (
    sendMessageACompletionRecorded ||
    sendMessageAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteSendMessageAObservation, 10);
    return;
  }
  if (sendMessageADrainCheckScheduled) {
    return;
  }
  sendMessageADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    sendMessageADrainCheckScheduled = false;
    if (sendMessageACompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteSendMessageAObservation();
      return;
    }
    sendMessageACompletionRecorded = true;
    emit("send-message-a-complete", {
      profile: "x86-bgi-send-message-a-wm-seticon-v1",
      quiescenceMilliseconds: 250,
      mainModuleCalls: sendMessageAMainModuleCalls.map((call) => ({ ...call })),
      windowProcedureCalls: sendMessageAWindowProcedureCalls.map((call) => ({
        ...call,
      })),
    });
    if (stopOnEvent === "send-message-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function maybeCompleteSetWindowPosObservation() {
  const frameChangeMoveObservation = selectedObservations.has(
    "set-window-pos-frame-change-move",
  );
  const resizeObservation = selectedObservations.has("set-window-pos-resize");
  const sizeObservation =
    selectedObservations.has("set-window-pos-size") || resizeObservation;
  const stateObservation = sizeObservation || frameChangeMoveObservation;
  const selectedStateObservation = frameChangeMoveObservation
    ? "set-window-pos-frame-change-move"
    : resizeObservation
      ? "set-window-pos-resize"
      : "set-window-pos-size";
  const selectedMainCalls = stateObservation
    ? setWindowPosMainModuleCalls.filter(
        (call) => call.observation === selectedStateObservation,
      )
    : setWindowPosMainModuleCalls.filter(
        (call) => call.observation === "set-window-pos",
      );
  if (
    setWindowPosCompletionRecorded ||
    (stateObservation
      ? selectedMainCalls.length === 0 || !setWindowPosStateOracleComplete
      : selectedMainCalls.length < 2)
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteSetWindowPosObservation, 10);
    return;
  }
  setWindowPosCompletionRecorded = true;
  const selectedCallIds = new Set(selectedMainCalls.map((call) => call.callId));
  const completionKind = frameChangeMoveObservation
    ? "set-window-pos-frame-change-move-complete"
    : resizeObservation
      ? "set-window-pos-resize-complete"
      : sizeObservation
        ? "set-window-pos-size-complete"
        : "set-window-pos-complete";
  emit(completionKind, {
    profile: frameChangeMoveObservation
      ? "x86-bgi-set-window-pos-frame-change-move-v1"
      : resizeObservation
        ? "x86-bgi-first-set-window-pos-resize-v1"
        : sizeObservation
          ? "x86-bgi-first-set-window-pos-size-v1"
          : "x86-bgi-set-window-pos-initial-placement-v1",
    quiescenceMilliseconds: 0,
    mainModuleCalls: selectedMainCalls.map((call) => ({ ...call })),
    windowProcedureCalls: setWindowPosWindowProcedureCalls
      .filter(
        (call) =>
          !stateObservation || selectedCallIds.has(call.setWindowPosCallId),
      )
      .map((call) => ({ ...call })),
    defaultProcedureCalls: setWindowPosDefaultProcedureCalls
      .filter(
        (call) =>
          !stateObservation || selectedCallIds.has(call.setWindowPosCallId),
      )
      .map((call) => ({ ...call })),
    ...(stateObservation ? { oracle: setWindowPosStateOracleEvidence } : {}),
  });
  if (stopOnEvent === completionKind) {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteShowWindowNormalObservation() {
  if (
    showWindowNormalCompletionRecorded ||
    !showWindowNormalOracleComplete ||
    showWindowNormalMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteShowWindowNormalObservation, 10);
    return;
  }
  showWindowNormalCompletionRecorded = true;
  const selectedCallIds = new Set(
    showWindowNormalMainModuleCalls.map((call) => call.callId),
  );
  emit("show-window-normal-complete", {
    profile: "x86-bgi-show-window-normal-v1",
    quiescenceMilliseconds: 0,
    mainModuleCalls: showWindowNormalMainModuleCalls.map((call) => ({
      ...call,
    })),
    windowProcedureCalls: showWindowNormalWindowProcedureCalls
      .filter((call) => selectedCallIds.has(call.showWindowCallId))
      .map((call) => ({ ...call })),
    oracle: showWindowNormalOracleEvidence,
  });
  if (stopOnEvent === "show-window-normal-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteUpdateWindowObservation() {
  if (
    updateWindowCompletionRecorded ||
    !updateWindowOracleComplete ||
    updateWindowMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteUpdateWindowObservation, 10);
    return;
  }
  updateWindowCompletionRecorded = true;
  const selectedCallIds = new Set(
    updateWindowMainModuleCalls.map((call) => call.callId),
  );
  emit("update-window-complete", {
    profile: "x86-bgi-update-window-full-client-paint-v1",
    quiescenceMilliseconds: 0,
    mainModuleCalls: updateWindowMainModuleCalls.map((call) => ({ ...call })),
    windowProcedureCalls: updateWindowWindowProcedureCalls
      .filter((call) => selectedCallIds.has(call.updateWindowCallId))
      .map((call) => ({ ...call })),
    paintCalls: updateWindowPaintCalls
      .filter((call) => selectedCallIds.has(call.updateWindowCallId))
      .map((call) => ({ ...call })),
    oracle: updateWindowOracleEvidence,
  });
  if (stopOnEvent === "update-window-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function maybeCompleteLoadIconAObservation() {
  if (
    loadIconACompletionRecorded ||
    !loadIconAOracleComplete ||
    loadIconAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteLoadIconAObservation, 10);
    return;
  }
  if (loadIconADrainCheckScheduled) {
    return;
  }
  loadIconADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    loadIconADrainCheckScheduled = false;
    if (loadIconACompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteLoadIconAObservation();
      return;
    }
    loadIconACompletionRecorded = true;
    emit("load-icon-a-complete", {
      profile: "x86-bgi-load-icon-a-v1",
      oracleProfile: loadIconAOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: loadIconAMainModuleCalls.map((call) => ({ ...call })),
    });
    if (stopOnEvent === "load-icon-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleLoadIconAOracle(module) {
  if (loadIconAOracleScheduled) {
    return;
  }
  loadIconAOracleScheduled = true;
  setImmediate(() => {
    try {
      const loadIconAddress = findExport(module, "LoadIconA");
      const getIconInfoAddress = findExport(module, "GetIconInfo");
      const getSystemMetricsAddress = findExport(module, "GetSystemMetrics");
      const gdi32 = Process.getModuleByName("gdi32.dll");
      const getObjectAddress = findExport(gdi32, "GetObjectW");
      const deleteObjectAddress = findExport(gdi32, "DeleteObject");
      if (
        loadIconAddress === null ||
        getIconInfoAddress === null ||
        getSystemMetricsAddress === null ||
        getObjectAddress === null ||
        deleteObjectAddress === null
      ) {
        throw new Error("LoadIconA oracle exports are unavailable");
      }
      const loadIconA = new SystemFunction(
        loadIconAddress,
        "pointer",
        ["pointer", "pointer"],
        "stdcall",
      );
      const getIconInfo = new SystemFunction(
        getIconInfoAddress,
        "uint32",
        ["pointer", "pointer"],
        "stdcall",
      );
      const getSystemMetrics = new NativeFunction(
        getSystemMetricsAddress,
        "int32",
        ["int32"],
        "stdcall",
      );
      const getObjectW = new SystemFunction(
        getObjectAddress,
        "int32",
        ["pointer", "int32", "pointer"],
        "stdcall",
      );
      const deleteObject = new SystemFunction(
        deleteObjectAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );

      const snapshotBitmap = (handle) => {
        if (handle.isNull()) {
          return { handle: handle.toString(), isNull: true };
        }
        const storage = Memory.alloc(24);
        storage.writeByteArray(Array(24).fill(0xa5));
        const outcome = getObjectW(handle, 24, storage);
        const result = Number(outcome.value);
        const bytes = [...new Uint8Array(storage.readByteArray(24))];
        const view = new DataView(Uint8Array.from(bytes).buffer);
        return {
          handle: handle.toString(),
          isNull: false,
          result,
          lastError: Number(outcome.lastError) >>> 0,
          fields:
            result === 0
              ? null
              : {
                  type: view.getInt32(0, true),
                  width: view.getInt32(4, true),
                  height: view.getInt32(8, true),
                  widthBytes: view.getInt32(12, true),
                  planes: view.getUint16(16, true),
                  bitsPixel: view.getUint16(18, true),
                  bits: `0x${view.getUint32(20, true).toString(16)}`,
                },
        };
      };

      const snapshotIcon = (handle) => {
        const storage = Memory.alloc(20);
        storage.writeByteArray(Array(20).fill(0xa5));
        const outcome = getIconInfo(handle, storage);
        const succeeded = Number(outcome.value) !== 0;
        if (!succeeded) {
          return {
            result: Number(outcome.value) >>> 0,
            lastError: Number(outcome.lastError) >>> 0,
          };
        }
        const mask = storage.add(12).readPointer();
        const color = storage.add(16).readPointer();
        let maskSnapshot = null;
        let colorSnapshot = null;
        const deletions = [];
        try {
          maskSnapshot = snapshotBitmap(mask);
          colorSnapshot = snapshotBitmap(color);
        } finally {
          for (const [kind, bitmap] of [
            ["mask", mask],
            ["color", color],
          ]) {
            if (!bitmap.isNull()) {
              const deletion = deleteObject(bitmap);
              deletions.push({
                kind,
                result: Number(deletion.value) >>> 0,
                lastError: Number(deletion.lastError) >>> 0,
              });
            }
          }
        }
        return {
          result: Number(outcome.value) >>> 0,
          lastError: Number(outcome.lastError) >>> 0,
          fIcon: storage.readU32(),
          xHotspot: storage.add(4).readU32(),
          yHotspot: storage.add(8).readU32(),
          mask: maskSnapshot,
          color: colorSnapshot,
          deletions,
        };
      };

      const invoke = ({ id, resourceId, sentinel, inspect }) => {
        loadIconAOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = loadIconA(Process.mainModule.base, ptr(resourceId));
          const handle = outcome.value;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            loadIconAOracleCall.hookResult === null ||
            loadIconAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error("LoadIconA oracle did not pass through the hook");
          }
          return {
            id,
            instance: Process.mainModule.base.toString(),
            resourceId,
            sentinel,
            hookResult: loadIconAOracleCall.hookResult,
            systemResult: handle.toString(),
            resultNonNull: !handle.isNull(),
            hookLastErrorAfter: loadIconAOracleCall.hookLastErrorAfter,
            systemLastError,
            hookResultMatches:
              loadIconAOracleCall.hookResult === handle.toString(),
            hookAndSystemLastErrorMatch:
              loadIconAOracleCall.hookLastErrorAfter === systemLastError,
            icon: inspect && !handle.isNull() ? snapshotIcon(handle) : null,
          };
        } finally {
          loadIconAOracleCall = null;
        }
      };
      const calls = [
        {
          id: "valid-first",
          resourceId: 101,
          sentinel: 0x11223344,
          inspect: true,
        },
        {
          id: "valid-repeated",
          resourceId: 101,
          sentinel: 0x55667788,
          inspect: false,
        },
        {
          id: "missing-resource",
          resourceId: 0xffff,
          sentinel: 0x7a6b5c4d,
          inspect: false,
        },
      ].map(invoke);
      const validCalls = calls.slice(0, 2);
      const missingCall = calls[2];
      const mainResult = loadIconAMainModuleCalls[0].result;
      loadIconAOracleEvidence = {
        profile: "x86-module-group-icon-shared-identity-v1",
        mainModule: {
          name: Process.mainModule.name,
          base: Process.mainModule.base.toString(),
        },
        systemMetrics: {
          iconWidth: getSystemMetrics(11),
          iconHeight: getSystemMetrics(12),
          smallIconWidth: getSystemMetrics(49),
          smallIconHeight: getSystemMetrics(50),
        },
        calls,
        validCallsSucceeded: validCalls.every((call) => call.resultNonNull),
        repeatedValidIdentityShared:
          validCalls[0].systemResult === validCalls[1].systemResult,
        validIdentityMatchesMainCall: validCalls.every(
          (call) => call.systemResult === mainResult,
        ),
        missingResourceRejected: !missingCall.resultNonNull,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      loadIconAOracleComplete = true;
      emit("load-icon-a-oracle", loadIconAOracleEvidence);
      maybeCompleteLoadIconAObservation();
    } catch (error) {
      emit("load-icon-a-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteLoadImageAObservation() {
  if (
    loadImageACompletionRecorded ||
    !loadImageAOracleComplete ||
    loadImageAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteLoadImageAObservation, 10);
    return;
  }
  if (loadImageADrainCheckScheduled) {
    return;
  }
  loadImageADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    loadImageADrainCheckScheduled = false;
    if (loadImageACompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteLoadImageAObservation();
      return;
    }
    loadImageACompletionRecorded = true;
    emit("load-image-a-complete", {
      profile: "x86-bgi-load-image-a-small-icon-v1",
      oracleProfile: loadImageAOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: loadImageAMainModuleCalls.map((call) => ({ ...call })),
    });
    if (stopOnEvent === "load-image-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleLoadImageAOracle(module) {
  if (loadImageAOracleScheduled) {
    return;
  }
  loadImageAOracleScheduled = true;
  setImmediate(() => {
    try {
      const loadImageAddress = findExport(module, "LoadImageA");
      const getIconInfoAddress = findExport(module, "GetIconInfo");
      const gdi32 = Process.getModuleByName("gdi32.dll");
      const getObjectAddress = findExport(gdi32, "GetObjectW");
      const deleteObjectAddress = findExport(gdi32, "DeleteObject");
      if (
        loadImageAddress === null ||
        getIconInfoAddress === null ||
        getObjectAddress === null ||
        deleteObjectAddress === null
      ) {
        throw new Error("LoadImageA oracle exports are unavailable");
      }
      const loadImageA = new SystemFunction(
        loadImageAddress,
        "pointer",
        ["pointer", "pointer", "uint32", "int32", "int32", "uint32"],
        "stdcall",
      );
      const getIconInfo = new SystemFunction(
        getIconInfoAddress,
        "uint32",
        ["pointer", "pointer"],
        "stdcall",
      );
      const getObjectW = new SystemFunction(
        getObjectAddress,
        "int32",
        ["pointer", "int32", "pointer"],
        "stdcall",
      );
      const deleteObject = new SystemFunction(
        deleteObjectAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );

      const snapshotBitmap = (handle) => {
        if (handle.isNull()) {
          return { handle: handle.toString(), isNull: true };
        }
        const storage = Memory.alloc(24);
        storage.writeByteArray(Array(24).fill(0xa5));
        const outcome = getObjectW(handle, 24, storage);
        const result = Number(outcome.value);
        const bytes = [...new Uint8Array(storage.readByteArray(24))];
        const view = new DataView(Uint8Array.from(bytes).buffer);
        return {
          handle: handle.toString(),
          isNull: false,
          result,
          lastError: Number(outcome.lastError) >>> 0,
          fields:
            result === 0
              ? null
              : {
                  type: view.getInt32(0, true),
                  width: view.getInt32(4, true),
                  height: view.getInt32(8, true),
                  widthBytes: view.getInt32(12, true),
                  planes: view.getUint16(16, true),
                  bitsPixel: view.getUint16(18, true),
                  bits: `0x${view.getUint32(20, true).toString(16)}`,
                },
        };
      };

      const snapshotIcon = (handle) => {
        const storage = Memory.alloc(20);
        storage.writeByteArray(Array(20).fill(0xa5));
        const outcome = getIconInfo(handle, storage);
        const succeeded = Number(outcome.value) !== 0;
        if (!succeeded) {
          return {
            result: Number(outcome.value) >>> 0,
            lastError: Number(outcome.lastError) >>> 0,
          };
        }
        const mask = storage.add(12).readPointer();
        const color = storage.add(16).readPointer();
        let maskSnapshot = null;
        let colorSnapshot = null;
        const deletions = [];
        try {
          maskSnapshot = snapshotBitmap(mask);
          colorSnapshot = snapshotBitmap(color);
        } finally {
          for (const [kind, bitmap] of [
            ["mask", mask],
            ["color", color],
          ]) {
            if (!bitmap.isNull()) {
              const deletion = deleteObject(bitmap);
              deletions.push({
                kind,
                result: Number(deletion.value) >>> 0,
                lastError: Number(deletion.lastError) >>> 0,
              });
            }
          }
        }
        return {
          result: Number(outcome.value) >>> 0,
          lastError: Number(outcome.lastError) >>> 0,
          fIcon: storage.readU32(),
          xHotspot: storage.add(4).readU32(),
          yHotspot: storage.add(8).readU32(),
          mask: maskSnapshot,
          color: colorSnapshot,
          deletions,
        };
      };

      const invoke = ({ id, resourceId, sentinel, inspect }) => {
        loadImageAOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = loadImageA(
            Process.mainModule.base,
            ptr(resourceId),
            1,
            16,
            16,
            0x8000,
          );
          const handle = outcome.value;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            loadImageAOracleCall.hookResult === null ||
            loadImageAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error("LoadImageA oracle did not pass through the hook");
          }
          return {
            id,
            instance: Process.mainModule.base.toString(),
            resourceId,
            imageType: 1,
            width: 16,
            height: 16,
            flags: 0x8000,
            sentinel,
            hookResult: loadImageAOracleCall.hookResult,
            systemResult: handle.toString(),
            resultNonNull: !handle.isNull(),
            hookLastErrorAfter: loadImageAOracleCall.hookLastErrorAfter,
            systemLastError,
            successLastErrorPreserved:
              !handle.isNull() && systemLastError === sentinel,
            hookResultMatches:
              loadImageAOracleCall.hookResult === handle.toString(),
            hookAndSystemLastErrorMatch:
              loadImageAOracleCall.hookLastErrorAfter === systemLastError,
            icon: inspect && !handle.isNull() ? snapshotIcon(handle) : null,
          };
        } finally {
          loadImageAOracleCall = null;
        }
      };
      const calls = [
        {
          id: "valid-first",
          resourceId: 101,
          sentinel: 0x11223344,
          inspect: true,
        },
        {
          id: "valid-repeated",
          resourceId: 101,
          sentinel: 0x55667788,
          inspect: false,
        },
        {
          id: "missing-resource",
          resourceId: 0xffff,
          sentinel: 0x7a6b5c4d,
          inspect: false,
        },
      ].map(invoke);
      const validCalls = calls.slice(0, 2);
      const missingCall = calls[2];
      const mainResult = loadImageAMainModuleCalls[0].result;
      loadImageAOracleEvidence = {
        profile: "x86-module-small-icon-load-image-shared-identity-v1",
        mainModule: {
          name: Process.mainModule.name,
          base: Process.mainModule.base.toString(),
        },
        calls,
        validCallsSucceeded: validCalls.every((call) => call.resultNonNull),
        validSuccessLastErrorsPreserved: validCalls.every(
          (call) => call.successLastErrorPreserved,
        ),
        validSuccessLastErrorsResetToZero: validCalls.every(
          (call) => call.systemLastError === 0,
        ),
        validIconIs16x16Color32:
          validCalls[0].icon?.result === 1 &&
          validCalls[0].icon?.fIcon === 1 &&
          validCalls[0].icon?.color?.fields?.width === 16 &&
          validCalls[0].icon?.color?.fields?.height === 16 &&
          validCalls[0].icon?.color?.fields?.planes === 1 &&
          validCalls[0].icon?.color?.fields?.bitsPixel === 32,
        repeatedValidIdentityShared:
          validCalls[0].systemResult === validCalls[1].systemResult,
        validIdentityMatchesMainCall: validCalls.every(
          (call) => call.systemResult === mainResult,
        ),
        missingResourceRejected: !missingCall.resultNonNull,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      loadImageAOracleComplete = true;
      emit("load-image-a-oracle", loadImageAOracleEvidence);
      maybeCompleteLoadImageAObservation();
    } catch (error) {
      emit("load-image-a-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteLoadCursorAObservation() {
  if (
    loadCursorACompletionRecorded ||
    !loadCursorAOracleComplete ||
    loadCursorAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteLoadCursorAObservation, 10);
    return;
  }
  if (loadCursorADrainCheckScheduled) {
    return;
  }
  loadCursorADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    loadCursorADrainCheckScheduled = false;
    if (loadCursorACompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteLoadCursorAObservation();
      return;
    }
    loadCursorACompletionRecorded = true;
    emit("load-cursor-a-complete", {
      profile: "x86-bgi-load-cursor-a-v1",
      oracleProfile: loadCursorAOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: loadCursorAMainModuleCalls.map((call) => ({ ...call })),
    });
    if (stopOnEvent === "load-cursor-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleLoadCursorAOracle(module) {
  if (loadCursorAOracleScheduled) {
    return;
  }
  loadCursorAOracleScheduled = true;
  setImmediate(() => {
    try {
      const loadCursorAddress = findExport(module, "LoadCursorA");
      const getSystemMetricsAddress = findExport(module, "GetSystemMetrics");
      if (loadCursorAddress === null || getSystemMetricsAddress === null) {
        throw new Error("LoadCursorA oracle exports are unavailable");
      }
      const loadCursorA = new SystemFunction(
        loadCursorAddress,
        "pointer",
        ["pointer", "pointer"],
        "stdcall",
      );
      const getSystemMetrics = new NativeFunction(
        getSystemMetricsAddress,
        "int32",
        ["int32"],
        "stdcall",
      );
      const invoke = ({ id, instance, resourceId, sentinel }) => {
        loadCursorAOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = loadCursorA(instance, ptr(resourceId));
          const handle = outcome.value;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            loadCursorAOracleCall.hookResult === null ||
            loadCursorAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error("LoadCursorA oracle did not pass through the hook");
          }
          return {
            id,
            instance: instance.toString(),
            resourceId,
            sentinel,
            hookResult: loadCursorAOracleCall.hookResult,
            systemResult: handle.toString(),
            resultNonNull: !handle.isNull(),
            hookLastErrorAfter: loadCursorAOracleCall.hookLastErrorAfter,
            systemLastError,
            hookResultMatches:
              loadCursorAOracleCall.hookResult === handle.toString(),
            hookAndSystemLastErrorMatch:
              loadCursorAOracleCall.hookLastErrorAfter === systemLastError,
          };
        } finally {
          loadCursorAOracleCall = null;
        }
      };
      const calls = [
        {
          id: "arrow-first",
          instance: ptr(0),
          resourceId: 0x7f00,
          sentinel: 0x11223344,
        },
        {
          id: "arrow-repeated",
          instance: ptr(0),
          resourceId: 0x7f00,
          sentinel: 0x55667788,
        },
        {
          id: "missing-predefined",
          instance: ptr(0),
          resourceId: 0xffff,
          sentinel: 0x7a6b5c4d,
        },
        {
          id: "module-cursor-first",
          instance: Process.mainModule.base,
          resourceId: 106,
          sentinel: 0x13572468,
        },
        {
          id: "module-cursor-repeated",
          instance: Process.mainModule.base,
          resourceId: 106,
          sentinel: 0x24681357,
        },
        {
          id: "module-missing-name",
          instance: Process.mainModule.base,
          resourceId: 0xffff,
          sentinel: 0x31415926,
        },
        {
          id: "module-wrong-type",
          instance: Process.mainModule.base,
          resourceId: 101,
          sentinel: 0x27182818,
        },
      ].map(invoke);
      const validCalls = calls.slice(0, 2);
      const missingCall = calls[2];
      const moduleValidCalls = calls.slice(3, 5);
      const moduleMissingCall = calls[5];
      const moduleWrongTypeCall = calls[6];
      const mainResult = loadCursorAMainModuleCalls[0].result;
      loadCursorAOracleEvidence = {
        profile: "x86-system-arrow-and-module-cursor-shared-identity-v2",
        predefinedResourceId: 0x7f00,
        moduleResourceId: 106,
        systemMetrics: {
          cursorWidth: getSystemMetrics(13),
          cursorHeight: getSystemMetrics(14),
        },
        calls,
        validCallsSucceeded: validCalls.every((call) => call.resultNonNull),
        repeatedValidIdentityShared:
          validCalls[0].systemResult === validCalls[1].systemResult,
        validIdentityMatchesMainCall: validCalls.every(
          (call) => call.systemResult === mainResult,
        ),
        missingPredefinedRejected: !missingCall.resultNonNull,
        moduleValidCallsSucceeded: moduleValidCalls.every(
          (call) => call.resultNonNull,
        ),
        moduleRepeatedIdentityShared:
          moduleValidCalls[0].systemResult === moduleValidCalls[1].systemResult,
        moduleMissingNameRejected: !moduleMissingCall.resultNonNull,
        moduleWrongTypeRejected: !moduleWrongTypeCall.resultNonNull,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
      };
      loadCursorAOracleComplete = true;
      emit("load-cursor-a-oracle", loadCursorAOracleEvidence);
      maybeCompleteLoadCursorAObservation();
    } catch (error) {
      emit("load-cursor-a-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteGetSystemMetricsObservation() {
  if (
    getSystemMetricsCompletionRecorded ||
    !getSystemMetricsOracleComplete ||
    getSystemMetricsMainModuleCalls.length < 5
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteGetSystemMetricsObservation, 10);
    return;
  }
  const initialCalls = getSystemMetricsMainModuleCalls.slice(0, 5);
  const expectedMetricIndexes = [7, 8, 4, 0, 1];
  const observedMetricIndexes = initialCalls.map((call) => call.metricIndex);
  if (
    JSON.stringify(observedMetricIndexes) !==
    JSON.stringify(expectedMetricIndexes)
  ) {
    emit("get-system-metrics-profile-error", {
      expectedMetricIndexes,
      observedMetricIndexes,
    });
    return;
  }
  const oracleByIndex = new Map(
    getSystemMetricsOracleEvidence.calls.map((call) => [
      call.metricIndex,
      call.systemResult,
    ]),
  );
  getSystemMetricsCompletionRecorded = true;
  emit("get-system-metrics-complete", {
    profile: "x86-bgi-initial-window-metrics-v1",
    oracleProfile: getSystemMetricsOracleEvidence.profile,
    expectedMetricIndexes,
    mainModuleCalls: initialCalls,
    allMainResultsMatchOracle: initialCalls.every(
      (call) => oracleByIndex.get(call.metricIndex) === call.metricValue,
    ),
  });
  if (stopOnEvent === "get-system-metrics-complete") {
    recv("vine-trace-stop", () => {}).wait();
  }
}

function scheduleGetSystemMetricsOracle(module) {
  if (getSystemMetricsOracleScheduled) {
    return;
  }
  getSystemMetricsOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetSystemMetrics");
      if (address === null) {
        throw new Error("GetSystemMetrics oracle export is unavailable");
      }
      const getSystemMetrics = new SystemFunction(
        address,
        "int32",
        ["int32"],
        "stdcall",
      );
      const invoke = ({ id, metricIndex, sentinel }) => {
        getSystemMetricsOracleCall = {
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = getSystemMetrics(metricIndex);
          const systemResult = Number(outcome.value);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getSystemMetricsOracleCall.hookResult === null ||
            getSystemMetricsOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              "GetSystemMetrics oracle did not pass through the hook",
            );
          }
          return {
            id,
            metricIndex,
            sentinel,
            hookResult: getSystemMetricsOracleCall.hookResult,
            systemResult,
            hookLastErrorAfter: getSystemMetricsOracleCall.hookLastErrorAfter,
            systemLastError,
            hookResultMatches:
              getSystemMetricsOracleCall.hookResult === systemResult,
            hookAndSystemLastErrorMatch:
              getSystemMetricsOracleCall.hookLastErrorAfter === systemLastError,
            successLastErrorPreserved: systemLastError === sentinel,
          };
        } finally {
          getSystemMetricsOracleCall = null;
        }
      };
      const calls = [
        { id: "fixed-frame-width", metricIndex: 7, sentinel: 0x11223344 },
        { id: "fixed-frame-height", metricIndex: 8, sentinel: 0x55667788 },
        { id: "caption-height", metricIndex: 4, sentinel: 0x7a6b5c4d },
        { id: "primary-screen-width", metricIndex: 0, sentinel: 0x13572468 },
        { id: "primary-screen-height", metricIndex: 1, sentinel: 0x24681357 },
      ].map(invoke);
      getSystemMetricsOracleEvidence = {
        profile: "x86-system-metrics-last-error-v1",
        calls,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
        allSuccessLastErrorsPreserved: calls.every(
          (call) => call.successLastErrorPreserved,
        ),
      };
      getSystemMetricsOracleComplete = true;
      emit("get-system-metrics-oracle", getSystemMetricsOracleEvidence);
      maybeCompleteGetSystemMetricsObservation();
    } catch (error) {
      emit("get-system-metrics-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteCreateWindowExAObservation() {
  if (
    createWindowExACompletionRecorded ||
    createWindowExAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (
    activeCallsByThread.size !== 0 ||
    activeCreateWindowCallsByThread.size !== 0
  ) {
    setTimeout(maybeCompleteCreateWindowExAObservation, 10);
    return;
  }
  if (createWindowExADrainCheckScheduled) {
    return;
  }
  createWindowExADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    createWindowExADrainCheckScheduled = false;
    if (createWindowExACompletionRecorded) {
      return;
    }
    if (
      activeCallsByThread.size !== 0 ||
      activeCreateWindowCallsByThread.size !== 0 ||
      nextCallId !== expectedNextCallId
    ) {
      maybeCompleteCreateWindowExAObservation();
      return;
    }
    createWindowExACompletionRecorded = true;
    emit("create-window-ex-a-complete", {
      profile: "x86-bgi-create-window-ex-a-v1",
      quiescenceMilliseconds: 250,
      mainModuleCalls: createWindowExAMainModuleCalls.map((call) => ({
        ...call,
      })),
    });
    if (stopOnEvent === "create-window-ex-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function maybeCompleteRegisterClassExAObservation() {
  if (
    registerClassExACompletionRecorded ||
    !registerClassExAOracleComplete ||
    registerClassExAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteRegisterClassExAObservation, 10);
    return;
  }
  if (registerClassExADrainCheckScheduled) {
    return;
  }
  registerClassExADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    registerClassExADrainCheckScheduled = false;
    if (registerClassExACompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteRegisterClassExAObservation();
      return;
    }
    registerClassExACompletionRecorded = true;
    emit("register-class-ex-a-complete", {
      profile: "x86-bgi-register-class-ex-a-v1",
      oracleProfile: registerClassExAOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: registerClassExAMainModuleCalls.map((call) => ({
        ...call,
      })),
    });
    if (stopOnEvent === "register-class-ex-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleRegisterClassExAOracle(module) {
  if (registerClassExAOracleScheduled) {
    return;
  }
  registerClassExAOracleScheduled = true;
  setImmediate(() => {
    try {
      if (Process.pointerSize !== 4) {
        throw new Error("RegisterClassExA oracle requires an x86 process");
      }
      const registerClassAddress = findExport(module, "RegisterClassExA");
      const getClassInfoAddress = findExport(module, "GetClassInfoExA");
      const unregisterClassAddress = findExport(module, "UnregisterClassA");
      const defWindowProcAddress = findExport(module, "DefWindowProcA");
      if (
        registerClassAddress === null ||
        getClassInfoAddress === null ||
        unregisterClassAddress === null ||
        defWindowProcAddress === null
      ) {
        throw new Error("RegisterClassExA oracle exports are unavailable");
      }
      const registerClassExA = new SystemFunction(
        registerClassAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const getClassInfoExA = new SystemFunction(
        getClassInfoAddress,
        "int32",
        ["pointer", "pointer", "pointer"],
        "stdcall",
      );
      const unregisterClassA = new SystemFunction(
        unregisterClassAddress,
        "int32",
        ["pointer", "pointer"],
        "stdcall",
      );
      const instance = Process.mainModule.base;
      const zeroWndClass = () => {
        const storage = Memory.alloc(48);
        storage.writeByteArray(new Uint8Array(48).buffer);
        return storage;
      };
      const createWndClass = ({ name, cbSize }) => {
        const nameAddress = Memory.allocUtf8String(name);
        const storage = zeroWndClass();
        storage.writeU32(cbSize);
        storage.add(4).writeU32(3);
        storage.add(8).writePointer(defWindowProcAddress);
        storage.add(12).writeS32(4);
        storage.add(16).writeS32(8);
        storage.add(20).writePointer(instance);
        storage.add(32).writePointer(ptr(6));
        storage.add(40).writePointer(nameAddress);
        return { name, nameAddress, storage };
      };
      const invoke = ({ id, windowClass, sentinel }) => {
        registerClassExAOracleCall = {
          id,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
          hookWindowClassBefore: null,
          hookWindowClassAfter: null,
        };
        try {
          const outcome = registerClassExA(windowClass.storage);
          const atom = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            registerClassExAOracleCall.hookResult === null ||
            registerClassExAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              "RegisterClassExA oracle did not pass through the hook",
            );
          }
          return {
            id,
            name: windowClass.name,
            windowClassAddress: windowClass.storage.toString(),
            sentinel,
            hookResult: registerClassExAOracleCall.hookResult,
            systemResult: atom,
            succeeded: atom !== 0,
            hookLastErrorAfter: registerClassExAOracleCall.hookLastErrorAfter,
            systemLastError,
            hookResultMatches: registerClassExAOracleCall.hookResult === atom,
            hookAndSystemLastErrorMatch:
              registerClassExAOracleCall.hookLastErrorAfter === systemLastError,
            windowClassBefore: registerClassExAOracleCall.hookWindowClassBefore,
            windowClassAfter: registerClassExAOracleCall.hookWindowClassAfter,
          };
        } finally {
          registerClassExAOracleCall = null;
        }
      };
      const query = ({ id, name }) => {
        const output = zeroWndClass();
        output.writeU32(48);
        const outcome = getClassInfoExA(instance, name, output);
        return {
          id,
          argument: snapshotClassNameA(name),
          result: Number(outcome.value) >>> 0,
          lastError: Number(outcome.lastError) >>> 0,
          windowClass: snapshotWndClassExA(output),
        };
      };
      const unregister = ({ id, name }) => {
        const outcome = unregisterClassA(name, instance);
        return {
          id,
          argument: snapshotClassNameA(name),
          result: Number(outcome.value) >>> 0,
          lastError: Number(outcome.lastError) >>> 0,
        };
      };

      const unique = `${Process.id}-${Date.now()}`;
      const firstClass = createWndClass({
        name: `VineRegisterClassExA-${unique}`,
        cbSize: 48,
      });
      const invalidClass = createWndClass({
        name: `VineRegisterClassExAInvalid-${unique}`,
        cbSize: 47,
      });
      const first = invoke({
        id: "valid-first",
        windowClass: firstClass,
        sentinel: 0x11223344,
      });
      const duplicate = invoke({
        id: "duplicate-name-and-module",
        windowClass: firstClass,
        sentinel: 0x55667788,
      });
      const invalidSize = invoke({
        id: "invalid-size",
        windowClass: invalidClass,
        sentinel: 0x7a6b5c4d,
      });
      invalidClass.storage.writeU32(48);
      const correctedAfterInvalid = invoke({
        id: "corrected-after-invalid-size",
        windowClass: invalidClass,
        sentinel: 0x13572468,
      });
      const queryByName = query({
        id: "query-first-by-name",
        name: firstClass.nameAddress,
      });
      const queryByAtom = query({
        id: "query-first-by-atom",
        name: ptr(first.systemResult),
      });
      const cleanup = [
        unregister({
          id: "unregister-first",
          name: firstClass.nameAddress,
        }),
        unregister({
          id: "unregister-corrected",
          name: invalidClass.nameAddress,
        }),
      ];
      const queryAfterCleanup = query({
        id: "query-first-after-cleanup",
        name: firstClass.nameAddress,
      });
      const calls = [first, duplicate, invalidSize, correctedAfterInvalid];
      registerClassExAOracleEvidence = {
        profile: "x86-register-class-ex-a-identity-lifetime-v1",
        mainModule: {
          name: Process.mainModule.name,
          base: Process.mainModule.base.toString(),
        },
        wndProc: snapshotAddressIdentity(defWindowProcAddress),
        calls,
        queryByName,
        queryByAtom,
        cleanup,
        queryAfterCleanup,
        validCallsSucceeded: first.succeeded && correctedAfterInvalid.succeeded,
        validAtomsDistinct:
          first.systemResult !== correctedAfterInvalid.systemResult,
        duplicateRejected: !duplicate.succeeded,
        invalidSizeRejected: !invalidSize.succeeded,
        invalidSizeDidNotReserveIdentity:
          !invalidSize.succeeded && correctedAfterInvalid.succeeded,
        successLastErrorsPreserved:
          first.systemLastError === first.sentinel &&
          correctedAfterInvalid.systemLastError ===
            correctedAfterInvalid.sentinel,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
        allInputsPreserved: calls.every(
          (call) =>
            JSON.stringify(call.windowClassBefore?.raw?.bytes ?? null) ===
            JSON.stringify(call.windowClassAfter?.raw?.bytes ?? null),
        ),
        bothQueriesSucceeded:
          queryByName.result !== 0 && queryByAtom.result !== 0,
        cleanupSucceeded: cleanup.every((call) => call.result !== 0),
        queryAfterCleanupRejected: queryAfterCleanup.result === 0,
      };
      registerClassExAOracleComplete = true;
      emit("register-class-ex-a-oracle", registerClassExAOracleEvidence);
      maybeCompleteRegisterClassExAObservation();
    } catch (error) {
      emit("register-class-ex-a-oracle-error", { error: error.message });
    }
  });
}

function maybeCompleteGetVersionExAObservation() {
  if (
    getVersionExACompletionRecorded ||
    !getVersionExAOracleComplete ||
    getVersionExAMainModuleCalls.length === 0
  ) {
    return;
  }
  if (activeCallsByThread.size !== 0) {
    setTimeout(maybeCompleteGetVersionExAObservation, 10);
    return;
  }
  if (getVersionExADrainCheckScheduled) {
    return;
  }
  getVersionExADrainCheckScheduled = true;
  const expectedNextCallId = nextCallId;
  setTimeout(() => {
    getVersionExADrainCheckScheduled = false;
    if (getVersionExACompletionRecorded) {
      return;
    }
    if (activeCallsByThread.size !== 0 || nextCallId !== expectedNextCallId) {
      maybeCompleteGetVersionExAObservation();
      return;
    }
    getVersionExACompletionRecorded = true;
    emit("get-version-ex-a-complete", {
      profile: "x86-bgi-get-version-ex-a-v1",
      oracleProfile: getVersionExAOracleEvidence.profile,
      quiescenceMilliseconds: 250,
      mainModuleCalls: getVersionExAMainModuleCalls.map((call) => ({
        ...call,
      })),
    });
    if (stopOnEvent === "get-version-ex-a-complete") {
      recv("vine-trace-stop", () => {}).wait();
    }
  }, 250);
}

function scheduleGetFileAttributesAOracle(module) {
  if (getFileAttributesAOracleScheduled) {
    return;
  }
  getFileAttributesAOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetFileAttributesA");
      if (address === null) {
        throw new Error("GetFileAttributesA export is unavailable");
      }
      const getFileAttributesA = new SystemFunction(
        address,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const invoke = ({ id, path, sentinel }) => {
        const pathPointer = Memory.allocUtf8String(path);
        getFileAttributesAOracleCall = {
          id,
          path,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = getFileAttributesA(pathPointer);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getFileAttributesAOracleCall.hookResult === null ||
            getFileAttributesAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              "GetFileAttributesA oracle did not pass through the hook",
            );
          }
          return {
            id,
            path,
            sentinel,
            hookResult: getFileAttributesAOracleCall.hookResult,
            systemResult,
            hookLastErrorAfter: getFileAttributesAOracleCall.hookLastErrorAfter,
            systemLastError,
            hookResultMatches:
              getFileAttributesAOracleCall.hookResult === systemResult,
            hookLastErrorMatchesSystem:
              getFileAttributesAOracleCall.hookLastErrorAfter ===
              systemLastError,
            isInvalid: systemResult === 0xffff_ffff,
            isDirectory:
              systemResult !== 0xffff_ffff && (systemResult & 0x10) !== 0,
          };
        } finally {
          getFileAttributesAOracleCall = null;
        }
      };
      const calls = [
        {
          id: "existing-file",
          path: "BGI.exe",
          sentinel: 0x1122_3344,
        },
        {
          id: "existing-directory",
          path: ".",
          sentinel: 0x5566_7788,
        },
        {
          id: "missing-file",
          path: "vine-get-file-attributes-a-missing.bin",
          sentinel: 0x7a6b_5c4d,
        },
      ].map(invoke);
      emit("get-file-attributes-a-oracle", {
        profile: "ansi-relative-file-directory-and-missing-v1",
        calls,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookLastErrorMatchesSystem,
        ),
        existingFileIsRegular: !calls[0].isInvalid && !calls[0].isDirectory,
        existingDirectoryIsDirectory:
          !calls[1].isInvalid && calls[1].isDirectory,
        missingFileIsInvalid: calls[2].isInvalid,
      });
    } catch (error) {
      emit("get-file-attributes-a-oracle-error", { error: error.message });
    }
  });
}

function scheduleCreateDirectoryAOracle(module) {
  if (createDirectoryAOracleScheduled) {
    return;
  }
  createDirectoryAOracleScheduled = true;
  setImmediate(() => {
    let basePath = null;
    let createdPath = null;
    let filePath = null;
    let removeDirectory = null;
    let deleteFile = null;
    try {
      const createDirectoryAddress = findExport(module, "CreateDirectoryA");
      const removeDirectoryAddress = findExport(module, "RemoveDirectoryA");
      const createFileAddress = findExport(module, "CreateFileA");
      const deleteFileAddress = findExport(module, "DeleteFileA");
      const closeHandleAddress = findExport(module, "CloseHandle");
      const getTempPathAddress = findExport(module, "GetTempPathA");
      if (
        createDirectoryAddress === null ||
        removeDirectoryAddress === null ||
        createFileAddress === null ||
        deleteFileAddress === null ||
        closeHandleAddress === null ||
        getTempPathAddress === null
      ) {
        throw new Error("CreateDirectoryA oracle exports are unavailable");
      }
      const createDirectoryA = new SystemFunction(
        createDirectoryAddress,
        "uint32",
        ["pointer", "pointer"],
        "stdcall",
      );
      const createDirectoryRaw = new NativeFunction(
        createDirectoryAddress,
        "uint32",
        ["pointer", "pointer"],
        "stdcall",
      );
      removeDirectory = new NativeFunction(
        removeDirectoryAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const createFileA = new NativeFunction(
        createFileAddress,
        "pointer",
        [
          "pointer",
          "uint32",
          "uint32",
          "pointer",
          "uint32",
          "uint32",
          "pointer",
        ],
        "stdcall",
      );
      deleteFile = new NativeFunction(
        deleteFileAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const closeHandle = new NativeFunction(
        closeHandleAddress,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const getTempPathA = new SystemFunction(
        getTempPathAddress,
        "uint32",
        ["uint32", "pointer"],
        "stdcall",
      );
      const temporaryBuffer = Memory.alloc(32 * 1024);
      const temporary = getTempPathA(32 * 1024, temporaryBuffer);
      const temporaryLength = Number(temporary.value) >>> 0;
      if (temporaryLength === 0 || temporaryLength >= 32 * 1024) {
        throw new Error("GetTempPathA failed for the CreateDirectoryA oracle");
      }
      const temporaryRoot = temporaryBuffer.readUtf8String(temporaryLength);
      basePath = `${temporaryRoot}vine-create-directory-a-${Process.id}-${Date.now()}`;
      createdPath = `${basePath}\\created`;
      filePath = `${basePath}\\existing-file.bin`;
      const basePointer = Memory.allocUtf8String(basePath);
      if (createDirectoryRaw(basePointer, ptr(0)) === 0) {
        throw new Error(
          "CreateDirectoryA oracle could not create its temporary root",
        );
      }
      const filePointer = Memory.allocUtf8String(filePath);
      const fileHandle = createFileA(
        filePointer,
        0x4000_0000,
        0,
        ptr(0),
        1,
        0x80,
        ptr(0),
      );
      if (fileHandle.equals(ptr("0xffffffff"))) {
        throw new Error(
          "CreateDirectoryA oracle could not create its file conflict",
        );
      }
      closeHandle(fileHandle);

      const invoke = ({ id, path, sentinel }) => {
        const pathPointer = Memory.allocUtf8String(path);
        createDirectoryAOracleCall = {
          id,
          path,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = createDirectoryA(pathPointer, ptr(0));
          const systemResult = Number(outcome.value) >>> 0 !== 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            createDirectoryAOracleCall.hookResult === null ||
            createDirectoryAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              "CreateDirectoryA oracle did not pass through the hook",
            );
          }
          return {
            id,
            path,
            sentinel,
            hookResult: createDirectoryAOracleCall.hookResult,
            systemResult,
            hookLastErrorAfter: createDirectoryAOracleCall.hookLastErrorAfter,
            systemLastError,
            hookResultMatches:
              createDirectoryAOracleCall.hookResult === systemResult,
            hookLastErrorMatchesSystem:
              createDirectoryAOracleCall.hookLastErrorAfter === systemLastError,
          };
        } finally {
          createDirectoryAOracleCall = null;
        }
      };
      const calls = [
        {
          id: "create",
          path: createdPath,
          sentinel: 0x1122_3344,
        },
        {
          id: "duplicate-directory",
          path: createdPath,
          sentinel: 0x5566_7788,
        },
        {
          id: "missing-parent",
          path: `${basePath}\\missing-parent\\child`,
          sentinel: 0x7a6b_5c4d,
        },
        {
          id: "file-conflict",
          path: filePath,
          sentinel: 0x89ab_cdef,
        },
      ].map(invoke);
      const cleanup = {
        fileRemoved: deleteFile(filePointer) !== 0,
        directoryRemoved:
          removeDirectory(Memory.allocUtf8String(createdPath)) !== 0,
        rootRemoved: removeDirectory(basePointer) !== 0,
      };
      filePath = null;
      createdPath = null;
      basePath = null;
      emit("create-directory-a-oracle", {
        profile:
          "ansi-null-security-create-duplicate-parent-and-file-conflict-v1",
        calls,
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allLastErrorsMatchHooks: calls.every(
          (call) => call.hookLastErrorMatchesSystem,
        ),
        cleanup,
      });
      if (stopOnEvent === "create-directory-a-oracle") {
        recv("vine-trace-stop", () => {}).wait();
      }
    } catch (error) {
      emit("create-directory-a-oracle-error", { error: error.message });
    } finally {
      createDirectoryAOracleCall = null;
      if (filePath !== null && deleteFile !== null) {
        deleteFile(Memory.allocUtf8String(filePath));
      }
      if (createdPath !== null && removeDirectory !== null) {
        removeDirectory(Memory.allocUtf8String(createdPath));
      }
      if (basePath !== null && removeDirectory !== null) {
        removeDirectory(Memory.allocUtf8String(basePath));
      }
    }
  });
}

function scheduleGetVersionExAOracle(module) {
  if (getVersionExAOracleScheduled) {
    return;
  }
  getVersionExAOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetVersionExA");
      if (address === null) {
        throw new Error("GetVersionExA export is unavailable");
      }
      const getVersionExA = new SystemFunction(
        address,
        "uint32",
        ["pointer"],
        "stdcall",
      );
      const invoke = ({ declaredBytes, sentinel }) => {
        const guardBytes = 16;
        const outputCapacity = 156;
        const storageBytes = guardBytes + outputCapacity + guardBytes;
        const storage = Memory.alloc(storageBytes);
        storage.writeByteArray(Array(storageBytes).fill(0xa5));
        const output = storage.add(guardBytes);
        output.writeU32(declaredBytes);
        const storageBefore = snapshotBoundedRawBytes(
          storage,
          storageBytes,
          256,
        );
        getVersionExAOracleCall = {
          declaredBytes,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        try {
          const outcome = getVersionExA(output);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getVersionExAOracleCall.hookResult === null ||
            getVersionExAOracleCall.hookLastErrorAfter === null
          ) {
            throw new Error(
              "GetVersionExA oracle did not pass through the hook",
            );
          }
          const storageAfter = snapshotBoundedRawBytes(
            storage,
            storageBytes,
            256,
          );
          const beforeBytes = storageBefore.bytes ?? [];
          const afterBytes = storageAfter.bytes ?? [];
          const changedStorageOffsets =
            beforeBytes.length === storageBytes &&
            afterBytes.length === storageBytes
              ? beforeBytes.flatMap((value, index) =>
                  value === afterBytes[index] ? [] : [index],
                )
              : [];
          const changedOutputOffsets = changedStorageOffsets
            .filter(
              (offset) =>
                offset >= guardBytes && offset < guardBytes + outputCapacity,
            )
            .map((offset) => offset - guardBytes);
          return {
            declaredBytes,
            sentinel,
            hookResult: getVersionExAOracleCall.hookResult,
            systemResult,
            hookLastErrorAfter: getVersionExAOracleCall.hookLastErrorAfter,
            systemLastError,
            storageOffset: guardBytes,
            outputCapacity,
            storageBefore,
            storageAfter,
            outputAfter: snapshotOsVersionInfoA(output),
            changedOutputOffsets,
            prefixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes.slice(0, guardBytes).every((value) => value === 0xa5),
            suffixGuardUnchanged:
              afterBytes.length === storageBytes &&
              afterBytes
                .slice(guardBytes + outputCapacity)
                .every((value) => value === 0xa5),
            bytesBeyondDeclaredSizeUnchanged:
              beforeBytes.length === storageBytes &&
              afterBytes.length === storageBytes &&
              beforeBytes
                .slice(guardBytes + Math.max(4, declaredBytes))
                .every(
                  (value, index) =>
                    value ===
                    afterBytes[guardBytes + Math.max(4, declaredBytes) + index],
                ),
            hookResultMatches:
              getVersionExAOracleCall.hookResult === systemResult,
            hookPreservesLastError:
              getVersionExAOracleCall.hookLastErrorAfter === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          getVersionExAOracleCall = null;
        }
      };
      const calls = [
        { declaredBytes: 0, sentinel: 0x10203040 },
        { declaredBytes: 147, sentinel: 0x11223344 },
        { declaredBytes: 148, sentinel: 0 },
        { declaredBytes: 148, sentinel: 87 },
        { declaredBytes: 148, sentinel: 0x12345678 },
        { declaredBytes: 156, sentinel: 0x76543210 },
      ].map(invoke);
      const baseCalls = calls.filter((call) => call.declaredBytes === 148);
      const referenceFields = baseCalls[0]?.outputAfter.fields ?? null;
      getVersionExAOracleEvidence = {
        profile: "x86-get-version-ex-a-size-and-output-v1",
        baseStructureBytes: 148,
        extendedStructureBytes: 156,
        calls,
        baseSizeSucceeded: baseCalls.every(
          (call) => call.systemResult !== 0 && call.hookResultMatches,
        ),
        baseOutputsStable:
          referenceFields !== null &&
          baseCalls.every(
            (call) =>
              JSON.stringify(call.outputAfter.fields ?? null) ===
              JSON.stringify(referenceFields),
          ),
        allResultsMatchHooks: calls.every((call) => call.hookResultMatches),
        allGuardsUnchanged: calls.every(
          (call) => call.prefixGuardUnchanged && call.suffixGuardUnchanged,
        ),
      };
      getVersionExAOracleComplete = true;
      emit("get-version-ex-a-oracle", getVersionExAOracleEvidence);
      maybeCompleteGetVersionExAObservation();
    } catch (error) {
      emit("get-version-ex-a-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetCpInfoOracle(module) {
  if (getCpInfoOracleScheduled) {
    return;
  }
  getCpInfoOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetCPInfo");
      if (address === null) {
        throw new Error("GetCPInfo export is unavailable");
      }
      const getCpInfo = new SystemFunction(
        address,
        "uint32",
        ["uint32", "pointer"],
        "stdcall",
      );
      const invoke = (sentinel) => {
        const storage = Memory.alloc(64);
        storage.writeByteArray(Array(64).fill(0xa5));
        const output = storage.add(16);
        const storageBefore = snapshotBoundedRawBytes(storage, 64);
        getCpInfoOracleSentinel = sentinel;
        getCpInfoOracleResult = null;
        getCpInfoOracleLastError = null;
        try {
          const outcome = getCpInfo(932, output);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            getCpInfoOracleResult === null ||
            getCpInfoOracleLastError === null
          ) {
            throw new Error("GetCPInfo oracle did not pass through the hook");
          }
          return {
            codePage: 932,
            sentinel,
            result,
            hookResult: getCpInfoOracleResult,
            hookLastErrorAfter: getCpInfoOracleLastError,
            systemLastError,
            storageOffset: 16,
            storageBefore,
            storageAfter: snapshotBoundedRawBytes(storage, 64),
            measuredOutput: snapshotMeasuredCpInfo(output),
            hookResultMatches: getCpInfoOracleResult === result,
            hookPreservesLastError: getCpInfoOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          getCpInfoOracleSentinel = null;
          getCpInfoOracleResult = null;
          getCpInfoOracleLastError = null;
        }
      };
      const calls = [0, 87, 0x12345678].map(invoke);
      emit("get-cp-info-oracle", {
        profile: "x86-code-page-932-output-v1",
        calls,
        allSucceeded: calls.every(
          (call) => call.result === 1 && call.hookResultMatches,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("get-cp-info-oracle-error", { error: error.message });
    }
  });
}

function scheduleGetSystemInfoOracle(module) {
  if (getSystemInfoOracleScheduled) {
    return;
  }
  getSystemInfoOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetSystemInfo");
      if (address === null) {
        throw new Error("GetSystemInfo export is unavailable");
      }
      const getSystemInfo = new SystemFunction(
        address,
        "void",
        ["pointer"],
        "stdcall",
      );
      const invoke = (sentinel) => {
        const storage = Memory.alloc(64);
        storage.writeByteArray(Array(64).fill(0xa5));
        const output = storage.add(16);
        const storageBefore = snapshotBoundedRawBytes(storage, 64);
        getSystemInfoOracleSentinel = sentinel;
        getSystemInfoOracleLastError = null;
        try {
          const outcome = getSystemInfo(output);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (getSystemInfoOracleLastError === null) {
            throw new Error(
              "GetSystemInfo oracle did not pass through the hook",
            );
          }
          const storageAfter = snapshotBoundedRawBytes(storage, 64);
          const bytes = storageAfter.bytes ?? [];
          return {
            sentinel,
            hookLastErrorAfter: getSystemInfoOracleLastError,
            systemLastError,
            storageOffset: 16,
            storageBefore,
            storageAfter,
            measuredOutput: snapshotSystemInfo(output),
            prefixGuardUnchanged:
              bytes.length === 64 &&
              bytes.slice(0, 16).every((value) => value === 0xa5),
            suffixGuardUnchanged:
              bytes.length === 64 &&
              bytes.slice(52).every((value) => value === 0xa5),
            hookPreservesLastError: getSystemInfoOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          getSystemInfoOracleSentinel = null;
          getSystemInfoOracleLastError = null;
        }
      };
      const calls = [0, 87, 0x12345678].map(invoke);
      emit("get-system-info-oracle", {
        profile: "x86-system-info-output-v1",
        calls,
        allOutputsStable: calls.every(
          (call) =>
            JSON.stringify(call.measuredOutput.fields) ===
            JSON.stringify(calls[0].measuredOutput.fields),
        ),
        allGuardsUnchanged: calls.every(
          (call) => call.prefixGuardUnchanged && call.suffixGuardUnchanged,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("get-system-info-oracle-error", { error: error.message });
    }
  });
}

function scheduleGlobalMemoryStatusOracle(module) {
  if (globalMemoryStatusOracleScheduled) {
    return;
  }
  globalMemoryStatusOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GlobalMemoryStatus");
      if (address === null) {
        throw new Error("GlobalMemoryStatus export is unavailable");
      }
      const globalMemoryStatus = new SystemFunction(
        address,
        "void",
        ["pointer"],
        "stdcall",
      );
      const invoke = (sentinel) => {
        const storage = Memory.alloc(64);
        storage.writeByteArray(Array(64).fill(0xa5));
        const output = storage.add(16);
        const storageBefore = snapshotBoundedRawBytes(storage, 64);
        globalMemoryStatusOracleSentinel = sentinel;
        globalMemoryStatusOracleLastError = null;
        try {
          const outcome = globalMemoryStatus(output);
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (globalMemoryStatusOracleLastError === null) {
            throw new Error(
              "GlobalMemoryStatus oracle did not pass through the hook",
            );
          }
          const storageAfter = snapshotBoundedRawBytes(storage, 64);
          const bytes = storageAfter.bytes ?? [];
          const measuredOutput = snapshotMemoryStatus(output);
          const fields = measuredOutput.fields;
          return {
            sentinel,
            hookLastErrorAfter: globalMemoryStatusOracleLastError,
            systemLastError,
            storageOffset: 16,
            storageBefore,
            storageAfter,
            measuredOutput,
            prefixGuardUnchanged:
              bytes.length === 64 &&
              bytes.slice(0, 16).every((value) => value === 0xa5),
            suffixGuardUnchanged:
              bytes.length === 64 &&
              bytes.slice(48).every((value) => value === 0xa5),
            fieldsCoherent:
              fields !== undefined &&
              fields.length === 32 &&
              fields.memoryLoadPercent <= 100 &&
              fields.totalPhysicalBytes > 0 &&
              fields.availablePhysicalBytes <= fields.totalPhysicalBytes &&
              fields.totalPageFileBytes > 0 &&
              fields.availablePageFileBytes <= fields.totalPageFileBytes &&
              fields.totalVirtualBytes > 0 &&
              fields.availableVirtualBytes <= fields.totalVirtualBytes,
            hookPreservesLastError:
              globalMemoryStatusOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          globalMemoryStatusOracleSentinel = null;
          globalMemoryStatusOracleLastError = null;
        }
      };
      const calls = [0, 87, 0x12345678].map(invoke);
      const stableCapacityFields = [
        "totalPhysicalBytes",
        "totalPageFileBytes",
        "totalVirtualBytes",
      ].every((field) =>
        calls.every(
          (call) =>
            call.measuredOutput.fields?.[field] ===
            calls[0].measuredOutput.fields?.[field],
        ),
      );
      emit("global-memory-status-oracle", {
        profile: "x86-memory-status-output-v1",
        calls,
        stableCapacityFields,
        allFieldsCoherent: calls.every((call) => call.fieldsCoherent),
        allGuardsUnchanged: calls.every(
          (call) => call.prefixGuardUnchanged && call.suffixGuardUnchanged,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("global-memory-status-oracle-error", { error: error.message });
    }
  });
}

function scheduleEnumDisplayMonitorsOracle(module) {
  if (enumDisplayMonitorsOracleScheduled) {
    return;
  }
  enumDisplayMonitorsOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "EnumDisplayMonitors");
      if (address === null) {
        throw new Error("EnumDisplayMonitors export is unavailable");
      }
      const enumDisplayMonitors = new SystemFunction(
        address,
        "uint32",
        ["pointer", "pointer", "pointer", "pointer"],
        "stdcall",
      );
      const invoke = (id, callbackResult, sentinel, data) => {
        const callbacks = [];
        const callback = new NativeCallback(
          (monitor, monitorDc, rect, callbackData) => {
            callbacks.push({
              threadId: Process.getCurrentThreadId(),
              monitor: monitor.toString(),
              monitorDc: monitorDc.toString(),
              rect: snapshotRect(rect),
              data: callbackData.toString(),
              result: callbackResult,
            });
            return callbackResult;
          },
          "uint32",
          ["pointer", "pointer", "pointer", "pointer"],
          "stdcall",
        );
        enumDisplayMonitorsOracleCallbacks.push(callback);
        enumDisplayMonitorsOracleSentinel = sentinel;
        enumDisplayMonitorsOracleResult = null;
        enumDisplayMonitorsOracleLastError = null;
        try {
          const outcome = enumDisplayMonitors(ptr(0), ptr(0), callback, data);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            enumDisplayMonitorsOracleResult === null ||
            enumDisplayMonitorsOracleLastError === null
          ) {
            throw new Error(
              `EnumDisplayMonitors ${id} oracle did not pass through the hook`,
            );
          }
          return {
            id,
            callbackResult,
            sentinel,
            hdc: ptr(0).toString(),
            clipRect: ptr(0).toString(),
            data: data.toString(),
            hookResult: enumDisplayMonitorsOracleResult,
            hookLastErrorAfter: enumDisplayMonitorsOracleLastError,
            systemResult,
            systemLastError,
            callbacks,
            hookResultMatches: enumDisplayMonitorsOracleResult === systemResult,
            hookPreservesLastError:
              enumDisplayMonitorsOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
            allCallbackDataRoundTrips: callbacks.every(
              (invocation) => invocation.data === data.toString(),
            ),
            allCallbackDcsNull: callbacks.every(
              (invocation) => invocation.monitorDc === ptr(0).toString(),
            ),
          };
        } finally {
          enumDisplayMonitorsOracleSentinel = null;
          enumDisplayMonitorsOracleResult = null;
          enumDisplayMonitorsOracleLastError = null;
        }
      };
      const calls = [
        invoke("continue", 1, 87, ptr("0x12345678")),
        invoke("stop", 0, 0x12345678, ptr("0x87654321")),
      ];
      emit("enum-display-monitors-oracle", {
        profile: "x86-null-dc-null-clip-callback-enumeration-v1",
        calls,
        continueCallbackCount: calls[0].callbacks.length,
        stopCallbackCount: calls[1].callbacks.length,
        stopHaltsAfterFirstCallback: calls[1].callbacks.length === 1,
        expectedResultsObserved:
          calls[0].systemResult === 1 &&
          calls[1].systemResult === 0 &&
          calls.every((call) => call.hookResultMatches),
        allCallbackDataRoundTrips: calls.every(
          (call) => call.allCallbackDataRoundTrips,
        ),
        allCallbackDcsNull: calls.every((call) => call.allCallbackDcsNull),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
        firstMonitorStable:
          calls.every((call) => call.callbacks.length > 0) &&
          calls[0].callbacks[0].monitor === calls[1].callbacks[0].monitor,
        firstRectStable:
          calls.every((call) => call.callbacks.length > 0) &&
          JSON.stringify(calls[0].callbacks[0].rect.fields) ===
            JSON.stringify(calls[1].callbacks[0].rect.fields),
      });
    } catch (error) {
      emit("enum-display-monitors-oracle-error", { error: error.message });
    }
  });
}

function scheduleComInitializationOracle(module) {
  if (comInitializationOracleScheduled) {
    return;
  }
  comInitializationOracleScheduled = true;
  setImmediate(() => {
    let coUninitialize = null;
    let successfulCalls = 0;
    try {
      const coInitializeAddress = findExport(module, "CoInitialize");
      const coInitializeExAddress = findExport(module, "CoInitializeEx");
      const coUninitializeAddress = findExport(module, "CoUninitialize");
      if (
        coInitializeAddress === null ||
        coInitializeExAddress === null ||
        coUninitializeAddress === null
      ) {
        throw new Error("COM initialization exports are unavailable");
      }
      const coInitialize = new SystemFunction(
        coInitializeAddress,
        "int",
        ["pointer"],
        "stdcall",
      );
      const coInitializeEx = new SystemFunction(
        coInitializeExAddress,
        "int",
        ["pointer", "uint32"],
        "stdcall",
      );
      coUninitialize = new NativeFunction(
        coUninitializeAddress,
        "void",
        [],
        "stdcall",
      );
      const invoke = (id, api, call, args, sentinel) => {
        const observation = {
          id,
          api,
          threadId: Process.getCurrentThreadId(),
          reserved: ptr(0).toString(),
          concurrency: api === "CoInitializeEx" ? args[1] : null,
          sentinel,
          hookResult: null,
          hookLastErrorAfter: null,
        };
        comInitializationOracleCall = observation;
        try {
          const outcome = call(...args);
          const systemResult = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            observation.hookResult === null ||
            observation.hookLastErrorAfter === null
          ) {
            throw new Error(
              `${api} ${id} oracle did not pass through the hook`,
            );
          }
          if ((systemResult & 0x80000000) === 0) {
            successfulCalls += 1;
          }
          return {
            ...observation,
            systemResult,
            signedSystemResult: systemResult | 0,
            systemLastError,
            hookResultMatches: observation.hookResult === systemResult,
            hookAndSystemLastErrorMatch:
              observation.hookLastErrorAfter === systemLastError,
            hookPreservesSentinel: observation.hookLastErrorAfter === sentinel,
            systemPreservesSentinel: systemLastError === sentinel,
          };
        } finally {
          comInitializationOracleCall = null;
        }
      };
      const balanceOne = () => {
        if (successfulCalls === 0) {
          throw new Error("COM oracle attempted an unbalanced CoUninitialize");
        }
        coUninitialize();
        successfulCalls -= 1;
      };

      const calls = [];
      calls.push(
        invoke("sta-first", "CoInitialize", coInitialize, [ptr(0)], 0x11111111),
      );
      calls.push(
        invoke(
          "sta-repeat",
          "CoInitialize",
          coInitialize,
          [ptr(0)],
          0x22222222,
        ),
      );
      calls.push(
        invoke(
          "sta-to-mta-conflict",
          "CoInitializeEx",
          coInitializeEx,
          [ptr(0), 0],
          0x33333333,
        ),
      );
      balanceOne();
      calls.push(
        invoke(
          "sta-count-one-conflict",
          "CoInitializeEx",
          coInitializeEx,
          [ptr(0), 0],
          0x44444444,
        ),
      );
      balanceOne();
      calls.push(
        invoke(
          "mta-first",
          "CoInitializeEx",
          coInitializeEx,
          [ptr(0), 0],
          0x55555555,
        ),
      );
      calls.push(
        invoke(
          "mta-repeat",
          "CoInitializeEx",
          coInitializeEx,
          [ptr(0), 0],
          0x66666666,
        ),
      );
      calls.push(
        invoke(
          "mta-to-sta-conflict",
          "CoInitialize",
          coInitialize,
          [ptr(0)],
          0x77777777,
        ),
      );
      balanceOne();
      balanceOne();
      calls.push(
        invoke(
          "balanced-sta-first",
          "CoInitialize",
          coInitialize,
          [ptr(0)],
          0x88888888,
        ),
      );
      balanceOne();

      const expectedResults = [
        0x00000000, 0x00000001, 0x80010106, 0x80010106, 0x00000000, 0x00000001,
        0x80010106, 0x00000000,
      ];
      emit("com-initialization-oracle", {
        profile: "x86-balanced-agent-thread-com-initialization-v1",
        calls,
        expectedResults,
        expectedResultsObserved: calls.every(
          (call, index) =>
            call.systemResult === expectedResults[index] &&
            call.hookResultMatches,
        ),
        allHookAndSystemLastErrorMatch: calls.every(
          (call) => call.hookAndSystemLastErrorMatch,
        ),
        repeatAndConflictCallsPreserveSentinels: calls
          .filter(
            (call) =>
              call.id.includes("repeat") || call.id.includes("conflict"),
          )
          .every(
            (call) =>
              call.hookPreservesSentinel && call.systemPreservesSentinel,
          ),
        firstSuccessLastErrors: calls
          .filter((call) => call.id.includes("first"))
          .map((call) => ({
            id: call.id,
            hookLastErrorAfter: call.hookLastErrorAfter,
            systemLastError: call.systemLastError,
            sentinel: call.sentinel,
          })),
        successfulCallBalanceZero: successfulCalls === 0,
      });
    } catch (error) {
      emit("com-initialization-oracle-error", { error: error.message });
    } finally {
      comInitializationOracleCall = null;
      if (coUninitialize !== null) {
        while (successfulCalls > 0) {
          coUninitialize();
          successfulCalls -= 1;
        }
      }
    }
  });
}

function scheduleProcessorFeatureOracle(module) {
  if (processorFeatureOracleScheduled) {
    return;
  }
  processorFeatureOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "IsProcessorFeaturePresent");
      if (address === null) {
        throw new Error("IsProcessorFeaturePresent export is unavailable");
      }
      const query = new SystemFunction(
        address,
        "uint32",
        ["uint32"],
        "stdcall",
      );
      const invoke = (feature, sentinel) => {
        processorFeatureOracleSentinel = sentinel;
        processorFeatureOracleResult = null;
        processorFeatureOracleLastError = null;
        try {
          const outcome = query(feature);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            processorFeatureOracleResult === null ||
            processorFeatureOracleLastError === null
          ) {
            throw new Error(
              "processor-feature oracle did not pass through the hook",
            );
          }
          return {
            feature,
            sentinel,
            result,
            hookResult: processorFeatureOracleResult,
            hookLastErrorAfter: processorFeatureOracleLastError,
            systemLastError,
            hookResultMatches: processorFeatureOracleResult === result,
            hookPreservesLastError:
              processorFeatureOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          processorFeatureOracleSentinel = null;
          processorFeatureOracleResult = null;
          processorFeatureOracleLastError = null;
        }
      };
      const calls = [
        invoke(10, 0),
        invoke(10, 87),
        invoke(10, 0x12345678),
        invoke(0xffffffff, 122),
      ];
      emit("processor-feature-oracle", {
        profile: "x86-processor-feature-query-v1",
        calls,
        feature10Stable: calls
          .slice(0, 3)
          .every(
            (call) => call.result === calls[0].result && call.hookResultMatches,
          ),
        invalidFeatureRejected: calls[3].result === 0,
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("processor-feature-oracle-error", { error: error.message });
    }
  });
}

function scheduleUnhandledExceptionFilterOracle(module) {
  if (unhandledExceptionFilterOracleScheduled) {
    return;
  }
  unhandledExceptionFilterOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "SetUnhandledExceptionFilter");
      if (address === null) {
        throw new Error("SetUnhandledExceptionFilter export is unavailable");
      }
      const setFilter = new SystemFunction(
        address,
        "pointer",
        ["pointer"],
        "stdcall",
      );
      const callbackA = new NativeCallback(
        () => 0,
        "int",
        ["pointer"],
        "stdcall",
      );
      const callbackB = new NativeCallback(
        () => 0,
        "int",
        ["pointer"],
        "stdcall",
      );
      unhandledExceptionFilterCallbacks.push(callbackA, callbackB);

      const invoke = (step, filter, sentinel) => {
        unhandledExceptionFilterOracleSentinel = sentinel;
        unhandledExceptionFilterOracleResult = null;
        unhandledExceptionFilterOracleLastError = null;
        try {
          const outcome = setFilter(filter);
          const result = outcome.value.toString().toLowerCase();
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            unhandledExceptionFilterOracleResult === null ||
            unhandledExceptionFilterOracleLastError === null
          ) {
            throw new Error(
              "unhandled-exception-filter oracle did not pass through the hook",
            );
          }
          return {
            step,
            filter: filter.toString().toLowerCase(),
            sentinel,
            result,
            hookResult: unhandledExceptionFilterOracleResult.toLowerCase(),
            hookLastErrorAfter: unhandledExceptionFilterOracleLastError,
            systemLastError,
            hookResultMatches:
              unhandledExceptionFilterOracleResult.toLowerCase() === result,
            hookPreservesLastError:
              unhandledExceptionFilterOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          unhandledExceptionFilterOracleSentinel = null;
          unhandledExceptionFilterOracleResult = null;
          unhandledExceptionFilterOracleLastError = null;
        }
      };

      const installA = invoke("install-a", callbackA, 0);
      const installB = invoke("replace-with-b", callbackB, 87);
      const remove = invoke("remove", NULL, 0x12345678);
      const restore = invoke("restore-original", ptr(installA.result), 122);
      const calls = [installA, installB, remove, restore];
      emit("unhandled-exception-filter-oracle", {
        profile: "x86-process-filter-replacement-v1",
        callbacks: [
          callbackA.toString().toLowerCase(),
          callbackB.toString().toLowerCase(),
        ],
        originalFilter: installA.result,
        calls,
        replacementChainExact:
          installB.result === callbackA.toString().toLowerCase() &&
          remove.result === callbackB.toString().toLowerCase() &&
          restore.result === NULL.toString().toLowerCase(),
        originalFilterRestored: restore.filter === installA.result,
        allHookResultsMatch: calls.every((call) => call.hookResultMatches),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("unhandled-exception-filter-oracle-error", {
        error: error.message,
      });
    }
  });
}

function scheduleHeapSizeOracle(module) {
  if (heapSizeOracleScheduled) {
    return;
  }
  heapSizeOracleScheduled = true;
  setImmediate(() => {
    let heap = NULL;
    let allocation = NULL;
    let allocationLive = false;
    try {
      const requireExport = (name) => {
        const address = findExport(module, name);
        if (address === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return address;
      };
      const heapCreate = new SystemFunction(
        requireExport("HeapCreate"),
        "pointer",
        ["uint", "uint", "uint"],
        "stdcall",
      );
      const heapAlloc = new SystemFunction(
        requireExport("HeapAlloc"),
        "pointer",
        ["pointer", "uint", "uint"],
        "stdcall",
      );
      const heapSize = new SystemFunction(
        requireExport("HeapSize"),
        "uint",
        ["pointer", "uint", "pointer"],
        "stdcall",
      );
      const heapFree = new SystemFunction(
        requireExport("HeapFree"),
        "bool",
        ["pointer", "uint", "pointer"],
        "stdcall",
      );
      const heapDestroy = new SystemFunction(
        requireExport("HeapDestroy"),
        "bool",
        ["pointer"],
        "stdcall",
      );

      heap = heapCreate(0, 0, 0).value;
      if (heap.isNull()) {
        throw new Error("HeapCreate returned NULL");
      }
      allocation = heapAlloc(heap, 0, 128).value;
      if (allocation.isNull()) {
        throw new Error("HeapAlloc returned NULL");
      }
      allocationLive = true;

      const invoke = (step, flags, pointer, sentinel) => {
        heapSizeOracleSentinel = sentinel;
        heapSizeOracleResult = null;
        heapSizeOracleLastError = null;
        try {
          const outcome = heapSize(heap, flags, pointer);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            heapSizeOracleResult === null ||
            heapSizeOracleLastError === null
          ) {
            throw new Error("heap-size oracle did not pass through the hook");
          }
          return {
            step,
            heap: heap.toString().toLowerCase(),
            flags,
            pointer: pointer.toString().toLowerCase(),
            sentinel,
            result,
            hookResult: heapSizeOracleResult,
            hookLastErrorAfter: heapSizeOracleLastError,
            systemLastError,
            hookResultMatches: heapSizeOracleResult === result,
            hookPreservesLastError: heapSizeOracleLastError === sentinel,
            systemPreservesLastError: systemLastError === sentinel,
          };
        } finally {
          heapSizeOracleSentinel = null;
          heapSizeOracleResult = null;
          heapSizeOracleLastError = null;
        }
      };

      const live = invoke("live-flags-zero", 0, allocation, 0);
      const liveRepeat = invoke("live-repeat", 0, allocation, 87);
      const noSerialize = invoke(
        "live-no-serialize",
        1,
        allocation,
        0x12345678,
      );
      const interior = invoke("interior-pointer", 0, allocation.add(1), 122);
      const freeOutcome = heapFree(heap, 0, allocation);
      if (!freeOutcome.value) {
        throw new Error("HeapFree failed in heap-size oracle");
      }
      allocationLive = false;
      const freed = invoke("freed-pointer", 0, allocation, 5);
      const calls = [live, liveRepeat, noSerialize, interior, freed];
      emit("heap-size-oracle", {
        profile: "x86-private-heap-size-query-v1",
        requestedBytes: 128,
        allocation: allocation.toString().toLowerCase(),
        failureSentinel: 0xffffffff,
        calls,
        liveResultStable:
          live.result === liveRepeat.result &&
          live.result === noSerialize.result,
        invalidPointersReturnFailure:
          interior.result === 0xffffffff && freed.result === 0xffffffff,
        allHookResultsMatch: calls.every((call) => call.hookResultMatches),
        successfulCallsPreserveLastError: [live, liveRepeat, noSerialize].every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
        invalidCallsSetErrorInvalidParameter: [interior, freed].every(
          (call) =>
            call.hookLastErrorAfter === 87 && call.systemLastError === 87,
        ),
        allPreserveLastError: calls.every(
          (call) =>
            call.hookPreservesLastError && call.systemPreservesLastError,
        ),
      });
    } catch (error) {
      emit("heap-size-oracle-error", { error: error.message });
    } finally {
      heapSizeOracleSentinel = null;
      heapSizeOracleResult = null;
      heapSizeOracleLastError = null;
      try {
        if (allocationLive && !heap.isNull() && !allocation.isNull()) {
          const address = findExport(module, "HeapFree");
          if (address !== null) {
            new NativeFunction(
              address,
              "bool",
              ["pointer", "uint", "pointer"],
              "stdcall",
            )(heap, 0, allocation);
          }
        }
        if (!heap.isNull()) {
          const address = findExport(module, "HeapDestroy");
          if (address !== null) {
            new NativeFunction(address, "bool", ["pointer"], "stdcall")(heap);
          }
        }
      } catch (cleanupError) {
        emit("heap-size-oracle-cleanup-error", {
          error: cleanupError.message,
        });
      }
    }
  });
}

function scheduleGetCurrentThreadOracle(module) {
  if (getCurrentThreadOracleScheduled) {
    return;
  }
  getCurrentThreadOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = findExport(module, "GetCurrentThread");
      if (address === null) {
        throw new Error("GetCurrentThread export is unavailable");
      }
      const oracleCode = Memory.alloc(Process.pageSize);
      const targetBytes = address.toUInt32().toString(16).padStart(8, "0");
      const targetLittleEndian = [
        Number.parseInt(targetBytes.slice(6, 8), 16),
        Number.parseInt(targetBytes.slice(4, 6), 16),
        Number.parseInt(targetBytes.slice(2, 4), 16),
        Number.parseInt(targetBytes.slice(0, 2), 16),
      ];
      const machineCode = [
        0x8b,
        0x4c,
        0x24,
        0x04, // mov ecx, [esp + 4]
        0x64,
        0x89,
        0x0d,
        0x34,
        0x00,
        0x00,
        0x00, // mov fs:[0x34], ecx
        0xb8,
        ...targetLittleEndian, // mov eax, GetCurrentThread
        0xff,
        0xd0, // call eax
        0x8b,
        0x54,
        0x24,
        0x08, // mov edx, [esp + 8]
        0x89,
        0x02, // mov [edx], eax
        0x64,
        0x8b,
        0x0d,
        0x34,
        0x00,
        0x00,
        0x00, // mov ecx, fs:[0x34]
        0x89,
        0x4a,
        0x04, // mov [edx + 4], ecx
        0xc2,
        0x08,
        0x00, // ret 8
      ];
      Memory.patchCode(oracleCode, machineCode.length, (writable) => {
        writable.writeByteArray(machineCode);
      });
      const invokeNativeOracle = new NativeFunction(
        oracleCode,
        "void",
        ["uint32", "pointer"],
        "stdcall",
      );
      const invoke = (sentinel) => {
        const threadIdBefore = Process.getCurrentThreadId();
        const output = Memory.alloc(8);
        output.writeByteArray(Array(8).fill(0xa5));
        invokeNativeOracle(sentinel, output);
        const threadIdAfter = Process.getCurrentThreadId();
        const result = output.readU32();
        const nativeLastErrorAfter = output.add(4).readU32();
        return {
          sentinel,
          threadIdBefore,
          threadIdAfter,
          result,
          nativeLastErrorAfter,
          sameThread: threadIdBefore === threadIdAfter,
          nativePreservesLastError: nativeLastErrorAfter === sentinel,
        };
      };
      const calls = [0, 87, 122, 0x12345678].map(invoke);
      const pseudoHandle = calls[0].result;
      emit("get-current-thread-oracle", {
        profile: "x86-current-thread-pseudo-handle-v1",
        threadId: Process.getCurrentThreadId(),
        pseudoHandle,
        calls,
        allResultsStable: calls.every(
          (call) => call.result === pseudoHandle && call.sameThread,
        ),
        allPreserveLastError: calls.every(
          (call) => call.nativePreservesLastError,
        ),
      });
    } catch (error) {
      emit("get-current-thread-oracle-error", { error: error.message });
    }
  });
}

function scheduleSetThreadAffinityMaskOracle(module) {
  if (setThreadAffinityMaskOracleScheduled) {
    return;
  }
  setThreadAffinityMaskOracleScheduled = true;
  setImmediate(() => {
    let originalMask = null;
    let currentMask = null;
    let currentThread = null;
    let setAffinity = null;
    const calls = [];
    try {
      const address = (name) => {
        const found = findExport(module, name);
        if (found === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return found;
      };
      const getCurrentThread = new NativeFunction(
        address("GetCurrentThread"),
        "pointer",
        [],
        "stdcall",
      );
      const getCurrentProcess = new NativeFunction(
        address("GetCurrentProcess"),
        "pointer",
        [],
        "stdcall",
      );
      const getProcessAffinityMask = new SystemFunction(
        address("GetProcessAffinityMask"),
        "uint32",
        ["pointer", "pointer", "pointer"],
        "stdcall",
      );
      setAffinity = new SystemFunction(
        address("SetThreadAffinityMask"),
        "uint32",
        ["pointer", "uint32"],
        "stdcall",
      );
      currentThread = getCurrentThread();
      const processMaskStorage = Memory.alloc(4);
      const systemMaskStorage = Memory.alloc(4);
      processMaskStorage.writeU32(0);
      systemMaskStorage.writeU32(0);
      const affinityQuery = getProcessAffinityMask(
        getCurrentProcess(),
        processMaskStorage,
        systemMaskStorage,
      );
      if (Number(affinityQuery.value) >>> 0 === 0) {
        throw new Error(
          `GetProcessAffinityMask failed with ${Number(affinityQuery.lastError) >>> 0}`,
        );
      }
      const processMask = processMaskStorage.readU32();
      const systemMask = systemMaskStorage.readU32();
      const selectedMask = (processMask & -processMask) >>> 0;
      if (selectedMask === 0) {
        throw new Error(
          "process affinity mask has no usable x86 processor bit",
        );
      }

      const invoke = (label, threadHandle, requestedMask, sentinel) => {
        setThreadAffinityMaskOracleSentinel = sentinel;
        setThreadAffinityMaskOracleResult = null;
        setThreadAffinityMaskOracleLastError = null;
        try {
          const outcome = setAffinity(threadHandle, requestedMask);
          const result = Number(outcome.value) >>> 0;
          const systemLastError = Number(outcome.lastError) >>> 0;
          if (
            setThreadAffinityMaskOracleResult === null ||
            setThreadAffinityMaskOracleLastError === null
          ) {
            throw new Error(
              "set-thread-affinity-mask oracle did not pass through the hook",
            );
          }
          const call = {
            label,
            threadHandle: threadHandle.toString(),
            requestedMask,
            sentinel,
            result,
            hookResult: setThreadAffinityMaskOracleResult,
            hookLastErrorAfter: setThreadAffinityMaskOracleLastError,
            systemLastError,
            hookResultMatches: setThreadAffinityMaskOracleResult === result,
          };
          calls.push(call);
          return call;
        } finally {
          setThreadAffinityMaskOracleSentinel = null;
          setThreadAffinityMaskOracleResult = null;
          setThreadAffinityMaskOracleLastError = null;
        }
      };

      const firstSet = invoke(
        "set-selected",
        currentThread,
        selectedMask,
        0x12345678,
      );
      if (firstSet.result === 0) {
        throw new Error(
          `valid affinity set failed with ${firstSet.systemLastError}`,
        );
      }
      originalMask = firstSet.result;
      currentMask = selectedMask;
      const restore = invoke(
        "restore-original",
        currentThread,
        originalMask,
        87,
      );
      if (restore.result !== selectedMask) {
        throw new Error(
          `affinity restore returned ${restore.result} instead of ${selectedMask}`,
        );
      }
      currentMask = originalMask;
      const zeroMask = invoke("reject-zero-mask", currentThread, 0, 122);
      const invalidHandle = invoke(
        "reject-null-handle",
        ptr(0),
        selectedMask,
        0,
      );

      emit("set-thread-affinity-mask-oracle", {
        profile: "x86-current-thread-logical-affinity-v1",
        currentThread: currentThread.toString(),
        processMask,
        systemMask,
        selectedMask,
        originalMask,
        calls,
        successReturnsPreviousMask:
          firstSet.result === originalMask && restore.result === selectedMask,
        successPreservesLastError:
          firstSet.hookLastErrorAfter === firstSet.sentinel &&
          firstSet.systemLastError === firstSet.sentinel &&
          restore.hookLastErrorAfter === restore.sentinel &&
          restore.systemLastError === restore.sentinel,
        zeroMaskRejected:
          zeroMask.result === 0 &&
          zeroMask.hookLastErrorAfter === 87 &&
          zeroMask.systemLastError === 87,
        nullHandleRejected:
          invalidHandle.result === 0 &&
          invalidHandle.hookLastErrorAfter === 6 &&
          invalidHandle.systemLastError === 6,
        restored: currentMask === originalMask,
      });
    } catch (error) {
      emit("set-thread-affinity-mask-oracle-error", { error: error.message });
    } finally {
      if (
        setAffinity !== null &&
        currentThread !== null &&
        originalMask !== null &&
        currentMask !== originalMask
      ) {
        try {
          setThreadAffinityMaskOracleSentinel = 0;
          setAffinity(currentThread, originalMask);
        } catch (cleanupError) {
          emit("set-thread-affinity-mask-oracle-cleanup-error", {
            error: cleanupError.message,
          });
        } finally {
          setThreadAffinityMaskOracleSentinel = null;
          setThreadAffinityMaskOracleResult = null;
          setThreadAffinityMaskOracleLastError = null;
        }
      }
    }
  });
}

function scheduleTlsLifecycleOracle(module) {
  if (tlsLifecycleOracleScheduled) {
    return;
  }
  tlsLifecycleOracleScheduled = true;
  setImmediate(() => {
    const allocatedIndices = [];
    let tlsFree = null;
    let finalized = false;
    const finish = (error, observation = null) => {
      if (finalized) {
        return;
      }
      finalized = true;
      if (error === null) {
        emit("tls-get-value-oracle", observation);
      } else {
        emit("tls-get-value-oracle-error", { error: error.message });
      }
      try {
        if (tlsFree !== null) {
          for (const index of allocatedIndices.reverse()) {
            tlsFree(index);
          }
        }
      } catch (cleanupError) {
        emit("tls-get-value-oracle-cleanup-error", {
          error: cleanupError.message,
        });
      }
    };
    try {
      const address = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const tlsAlloc = new NativeFunction(
        address("TlsAlloc"),
        "uint32",
        [],
        "stdcall",
      );
      tlsFree = new NativeFunction(
        address("TlsFree"),
        "int",
        ["uint32"],
        "stdcall",
      );
      const tlsSetValue = new NativeFunction(
        address("TlsSetValue"),
        "int",
        ["uint32", "pointer"],
        "stdcall",
      );
      const tlsGetValue = new SystemFunction(
        address("TlsGetValue"),
        "pointer",
        ["uint32"],
        "stdcall",
      );
      const setLastError = new NativeFunction(
        address("SetLastError"),
        "void",
        ["uint32"],
        "stdcall",
      );
      const allocate = () => {
        const index = Number(tlsAlloc()) >>> 0;
        if (index === 0xffffffff) {
          throw new Error("TlsAlloc returned TLS_OUT_OF_INDEXES");
        }
        allocatedIndices.push(index);
        return index;
      };
      const read = (id, index, sentinel) => {
        setLastError(sentinel);
        const outcome = tlsGetValue(index);
        return {
          id,
          index,
          sentinel,
          result: outcome.value.toString(),
          lastError: Number(outcome.lastError) >>> 0,
        };
      };

      const mainIndex = allocate();
      const mainValue = ptr("0x12345678");
      const mainThreadId = Process.getCurrentThreadId();
      if (Number(tlsSetValue(mainIndex, mainValue)) === 0) {
        throw new Error("TlsSetValue rejected the main-thread oracle value");
      }
      const mainSetRead = read("main-thread-set-value", mainIndex, 87);

      const unsetIndex = allocate();
      const unsetRead = read("allocated-unset-value", unsetIndex, 122);
      const invalidRead = read("invalid-index", 0xffffffff, 122);
      finish(null, {
        profile: "x86-success-null-and-invalid-v1",
        threadId: mainThreadId,
        setValue: mainValue.toString(),
        calls: [mainSetRead, unsetRead, invalidRead],
      });
    } catch (error) {
      finish(error);
    }
  });
}

function scheduleFlsLifecycleOracle(module) {
  if (flsLifecycleOracleScheduled) {
    return;
  }
  flsLifecycleOracleScheduled = true;
  setImmediate(() => {
    const allocatedIndices = [];
    let flsFree = null;
    let finalized = false;
    const finish = (error, observation = null) => {
      if (finalized) {
        return;
      }
      finalized = true;
      if (error === null) {
        emit("fls-get-value-oracle", observation);
      } else {
        emit("fls-get-value-oracle-error", { error: error.message });
      }
      try {
        if (flsFree !== null) {
          for (const index of allocatedIndices.reverse()) {
            flsFree(index);
          }
        }
      } catch (cleanupError) {
        emit("fls-get-value-oracle-cleanup-error", {
          error: cleanupError.message,
        });
      }
    };
    try {
      const address = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const flsAlloc = new NativeFunction(
        address("FlsAlloc"),
        "uint32",
        ["pointer"],
        "stdcall",
      );
      flsFree = new NativeFunction(
        address("FlsFree"),
        "int",
        ["uint32"],
        "stdcall",
      );
      const flsSetValue = new NativeFunction(
        address("FlsSetValue"),
        "int",
        ["uint32", "pointer"],
        "stdcall",
      );
      const flsGetValue = new SystemFunction(
        address("FlsGetValue"),
        "pointer",
        ["uint32"],
        "stdcall",
      );
      const setLastError = new NativeFunction(
        address("SetLastError"),
        "void",
        ["uint32"],
        "stdcall",
      );
      const allocate = () => {
        const index = Number(flsAlloc(ptr(0))) >>> 0;
        if (index === 0xffffffff) {
          throw new Error("FlsAlloc returned FLS_OUT_OF_INDEXES");
        }
        allocatedIndices.push(index);
        return index;
      };
      const read = (id, index, sentinel) => {
        setLastError(sentinel);
        const outcome = flsGetValue(index);
        return {
          id,
          index,
          sentinel,
          result: outcome.value.toString(),
          lastError: Number(outcome.lastError) >>> 0,
        };
      };

      const mainIndex = allocate();
      const mainValue = ptr("0x12345678");
      const mainThreadId = Process.getCurrentThreadId();
      if (Number(flsSetValue(mainIndex, mainValue)) === 0) {
        throw new Error("FlsSetValue rejected the main-thread oracle value");
      }
      const mainSetRead = read("main-thread-set-value", mainIndex, 87);

      const unsetIndex = allocate();
      const unsetRead = read("allocated-unset-value", unsetIndex, 122);
      const invalidRead = read("invalid-index", 0xffffffff, 122);
      finish(null, {
        profile: "x86-success-null-and-invalid-v1",
        threadId: mainThreadId,
        setValue: mainValue.toString(),
        calls: [mainSetRead, unsetRead, invalidRead],
      });
    } catch (error) {
      finish(error);
    }
  });
}

function scheduleCriticalSectionOracle(module) {
  if (criticalSectionOracleScheduled) {
    return;
  }
  criticalSectionOracleScheduled = true;
  setImmediate(() => {
    try {
      const address = (name) => {
        const result = findExport(module, name);
        if (result === null) {
          throw new Error(`${name} export is unavailable`);
        }
        return result;
      };
      const initialize = new NativeFunction(
        address("InitializeCriticalSectionAndSpinCount"),
        "int",
        ["pointer", "uint32"],
        "stdcall",
      );
      const initializeDefault = new NativeFunction(
        address("InitializeCriticalSection"),
        "void",
        ["pointer"],
        "stdcall",
      );
      const enter = new NativeFunction(
        address("EnterCriticalSection"),
        "void",
        ["pointer"],
        "stdcall",
      );
      const leave = new NativeFunction(
        address("LeaveCriticalSection"),
        "void",
        ["pointer"],
        "stdcall",
      );
      const delete_ = new NativeFunction(
        address("DeleteCriticalSection"),
        "void",
        ["pointer"],
        "stdcall",
      );
      const storage = Memory.alloc(24);
      storage.writeByteArray(Array(24).fill(0xa5));
      const states = [
        { step: "before", snapshot: snapshotCriticalSection(storage) },
      ];
      const defaultStorage = Memory.alloc(24);
      defaultStorage.writeByteArray(Array(24).fill(0xa5));
      const defaultBefore = snapshotCriticalSection(defaultStorage);
      criticalSectionOracleSentinel = 86;
      criticalSectionOracleLastError = null;
      let defaultLastError = null;
      try {
        initializeDefault(defaultStorage);
        defaultLastError = criticalSectionOracleLastError;
        if (defaultLastError === null) {
          throw new Error(
            "initialize-default did not pass through the critical-section hook",
          );
        }
      } finally {
        criticalSectionOracleSentinel = null;
        criticalSectionOracleLastError = null;
      }
      const defaultAfter = snapshotCriticalSection(defaultStorage);
      const invoke = (step, sentinel, operation) => {
        criticalSectionOracleSentinel = sentinel;
        criticalSectionOracleLastError = null;
        try {
          const result = operation();
          const lastError = criticalSectionOracleLastError;
          if (lastError === null) {
            throw new Error(
              `${step} did not pass through the critical-section hook`,
            );
          }
          states.push({
            step,
            sentinel,
            result: result === undefined ? null : Number(result),
            lastError,
            snapshot: snapshotCriticalSection(storage),
          });
        } finally {
          criticalSectionOracleSentinel = null;
          criticalSectionOracleLastError = null;
        }
      };
      invoke("initialize", 87, () => initialize(storage, 4000));
      invoke("enter-first", 88, () => enter(storage));
      invoke("enter-recursive", 89, () => enter(storage));
      invoke("leave-recursive", 90, () => leave(storage));
      invoke("leave-final", 91, () => leave(storage));
      invoke("delete", 92, () => delete_(storage));
      emit("critical-section-oracle", {
        profile: "x86-default-and-uncontended-recursive-v2",
        structureSize: 24,
        spinCount: 4000,
        defaultInitialization: {
          sentinel: 86,
          lastError: defaultLastError,
          exception: null,
          before: defaultBefore,
          after: defaultAfter,
        },
        states,
      });
    } catch (error) {
      emit("critical-section-oracle-error", { error: error.message });
    }
  });
}

Process.attachModuleObserver({
  onAdded(module) {
    emit("module-load", {
      module: module.name,
      base: module.base.toString(),
      size: module.size,
      path: module.path,
    });

    const moduleHooks = hooks[module.name.toLowerCase()];
    if (moduleHooks === undefined) {
      return;
    }
    if (
      !focusedWindowDcObservation &&
      !focusedScreenDcObservation &&
      !focusedFontObjectObservation &&
      !focusedGdiplusStartupObservation &&
      !focusedVirtualAllocObservation &&
      !focusedImeContextObservation &&
      !focusedSetWindowLongAObservation &&
      !focusedSetWindowPosSizeObservation
    ) {
      for (const [name, decoders] of moduleHooks) {
        try {
          attachHook(module, name, decoders);
        } catch (error) {
          emit("hook-error", {
            module: module.name,
            api: name,
            error: error.message,
          });
        }
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("query-performance-frequency")
    ) {
      try {
        attachQueryPerformanceFrequencyHook(module);
        scheduleQueryPerformanceFrequencyOracle(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "QueryPerformanceFrequency",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      focusedVirtualAllocObservation
    ) {
      for (const name of ["VirtualAlloc", "VirtualFree"]) {
        try {
          attachVirtualAllocHook(module, name);
        } catch (error) {
          emit("hook-error", {
            module: module.name,
            api: name,
            error: error.message,
          });
        }
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("query-performance-counter")
    ) {
      try {
        attachQueryPerformanceCounterHook(module);
        scheduleQueryPerformanceCounterOracle(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "QueryPerformanceCounter",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "imm32.dll" &&
      focusedImeContextObservation
    ) {
      for (const [name, attach] of [
        ["ImmGetDefaultIMEWnd", attachImmGetDefaultImeWindowHook],
        ["ImmAssociateContext", attachImmAssociateContextHook],
      ]) {
        try {
          attach(module);
        } catch (error) {
          emit("hook-error", {
            module: module.name,
            api: name,
            error: error.message,
          });
        }
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-file-attributes-a")
    ) {
      try {
        attachGetFileAttributesAHook(module);
        scheduleGetFileAttributesAOracle(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "GetFileAttributesA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("create-directory-a")
    ) {
      try {
        attachCreateDirectoryAHook(module);
        scheduleCreateDirectoryAOracle(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "CreateDirectoryA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-version-ex-a")
    ) {
      try {
        attachGetVersionExAHook(module);
        scheduleGetVersionExAOracle(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "GetVersionExA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("create-waitable-timer-a")
    ) {
      try {
        attachCreateWaitableTimerAHook(module);
        scheduleCreateWaitableTimerAOracle(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "CreateWaitableTimerA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("critical-section")
    ) {
      scheduleCriticalSectionOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("command-line-a")
    ) {
      scheduleCommandLineOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("environment-strings-w")
    ) {
      scheduleEnvironmentStringsOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("wide-char-to-multi-byte")
    ) {
      scheduleWideCharOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("multi-byte-to-wide-char")
    ) {
      scheduleMultiByteOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("set-current-directory-a")
    ) {
      scheduleSetCurrentDirectoryAOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("create-mutex-a")
    ) {
      scheduleCreateMutexAOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-string-type-w")
    ) {
      scheduleGetStringTypeOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("lc-map-string-w")
    ) {
      scheduleLCMapStringOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("interlocked-decrement")
    ) {
      scheduleInterlockedDecrementOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-module-file-name-a")
    ) {
      scheduleGetModuleFileNameAOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-last-error")
    ) {
      scheduleGetLastErrorOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("set-last-error")
    ) {
      scheduleSetLastErrorOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-acp")
    ) {
      scheduleGetAcpOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("system-default-lang-id")
    ) {
      scheduleSystemDefaultLangIdOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("drive-type")
    ) {
      scheduleDriveTypeOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-cp-info")
    ) {
      scheduleGetCpInfoOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-system-info")
    ) {
      scheduleGetSystemInfoOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("global-memory-status")
    ) {
      scheduleGlobalMemoryStatusOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      focusedGuestThreadObservation
    ) {
      for (const [name, attach] of [
        ["CreateThread", attachCreateThreadHook],
        ["ResumeThread", attachCreatedThreadLifecycleHook],
        ["SuspendThread", attachCreatedThreadLifecycleHook],
        ["SetThreadPriority", attachCreatedThreadLifecycleHook],
        ["SetThreadAffinityMask", attachCreatedThreadLifecycleHook],
        ["WaitForSingleObject", attachCreatedThreadLifecycleHook],
        ["GetExitCodeThread", attachCreatedThreadLifecycleHook],
        ["CloseHandle", attachCreatedThreadLifecycleHook],
      ]) {
        try {
          attach(module, name);
        } catch (error) {
          emit("hook-error", {
            module: module.name,
            api: name,
            error: error.message,
          });
        }
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("set-waitable-timer")
    ) {
      try {
        attachSetWaitableTimerHook(module);
        scheduleSetWaitableTimerOracle(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "SetWaitableTimer",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("wait-for-single-object-ex")
    ) {
      try {
        attachWaitForSingleObjectExHook(module);
        scheduleWaitForSingleObjectExOracle(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "WaitForSingleObjectEx",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      (selectedObservations.has("send-message-a") ||
        focusedImeContextObservation)
    ) {
      try {
        attachSendMessageAHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "SendMessageA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      focusedSetTimerObservation
    ) {
      for (const [name, attach] of [
        ["SetTimer", attachSetTimerHook],
        ["KillTimer", attachKillTimerHook],
      ]) {
        try {
          attach(module);
        } catch (error) {
          emit("hook-error", {
            module: module.name,
            api: name,
            error: error.message,
          });
        }
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      (selectedObservations.has("set-window-pos") ||
        selectedObservations.has("set-window-pos-size") ||
        selectedObservations.has("set-window-pos-resize") ||
        selectedObservations.has("set-window-pos-frame-change-move"))
    ) {
      try {
        attachSetWindowPosHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "SetWindowPos",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      focusedShowWindowNormalObservation
    ) {
      try {
        attachShowWindowNormalHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "ShowWindow",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      focusedUpdateWindowObservation
    ) {
      try {
        attachUpdateWindowHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "UpdateWindow",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("load-icon-a")
    ) {
      try {
        attachLoadIconAHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "LoadIconA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("load-image-a")
    ) {
      try {
        attachLoadImageAHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "LoadImageA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("load-cursor-a")
    ) {
      try {
        attachLoadCursorAHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "LoadCursorA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("register-class-ex-a")
    ) {
      try {
        attachRegisterClassExAHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "RegisterClassExA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("create-window-ex-a")
    ) {
      try {
        attachCreateWindowClassDiscoveryHook(module);
        attachCreateWindowExAHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "CreateWindowExA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("get-system-metrics")
    ) {
      try {
        attachGetSystemMetricsHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "GetSystemMetrics",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("get-client-rect")
    ) {
      try {
        attachGetClientRectHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "GetClientRect",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("set-rect")
    ) {
      try {
        attachSetRectHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "SetRect",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("get-monitor-info-a")
    ) {
      try {
        attachGetMonitorInfoAHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "GetMonitorInfoA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("window-dc")
    ) {
      for (const name of ["GetDC", "ReleaseDC", "FillRect"]) {
        try {
          attachWindowDcHook(module, name);
        } catch (error) {
          emit("hook-error", {
            module: module.name,
            api: name,
            error: error.message,
          });
        }
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("screen-dc")
    ) {
      for (const name of ["GetDC", "ReleaseDC"]) {
        try {
          attachScreenDcHook(module, name);
        } catch (error) {
          emit("hook-error", {
            module: module.name,
            api: name,
            error: error.message,
          });
        }
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("d3d9-surface-get-dc")
    ) {
      try {
        attachD3d9SurfaceDcGdiHook(module, "FillRect");
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "FillRect",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("set-window-long-a")
    ) {
      try {
        attachSetWindowLongAHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "SetWindowLongA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "gdi32.dll" &&
      selectedObservations.has("window-dc")
    ) {
      try {
        attachWindowDcHook(module, "GetStockObject");
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "GetStockObject",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "gdi32.dll" &&
      selectedObservations.has("screen-dc")
    ) {
      try {
        attachScreenDcHook(module, "EnumFontFamiliesExA");
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "EnumFontFamiliesExA",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "gdi32.dll" &&
      selectedObservations.has("font-object")
    ) {
      for (const name of [
        "CreateFontA",
        "SelectObject",
        "GetObjectA",
        "DeleteObject",
        "GetTextMetricsA",
        "GetTextExtentPoint32A",
      ]) {
        try {
          attachFontObjectHook(module, name);
        } catch (error) {
          emit("hook-error", {
            module: module.name,
            api: name,
            error: error.message,
          });
        }
      }
      for (const name of [
        "CreateCompatibleDC",
        "CreateDIBSection",
        "DeleteDC",
        "GetOutlineTextMetricsA",
        "GetGlyphOutlineA",
        "TextOutA",
        "TextOutW",
        "SetBkMode",
        "SetTextColor",
        "SelectPalette",
        "CreatePalette",
      ]) {
        try {
          attachFontPipelineHook(module, name);
        } catch (error) {
          emit("hook-error", {
            module: module.name,
            api: name,
            error: error.message,
          });
        }
      }
    }
    if (
      module.name.toLowerCase() === "gdiplus.dll" &&
      selectedObservations.has("gdiplus-startup")
    ) {
      for (const [name, argumentCount] of gdiplusImportedFunctions) {
        try {
          attachGdiplusHook(module, name, argumentCount);
        } catch (error) {
          emit("hook-error", {
            module: module.name,
            api: name,
            error: error.message,
          });
        }
      }
    }
    if (
      module.name.toLowerCase() === "gdi32.dll" &&
      selectedObservations.has("d3d9-surface-get-dc")
    ) {
      try {
        attachD3d9SurfaceDcGdiHook(module, "GetStockObject");
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "GetStockObject",
          error: error.message,
        });
      }
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("enum-display-monitors")
    ) {
      scheduleEnumDisplayMonitorsOracle(module);
    }
    if (
      module.name.toLowerCase() === "user32.dll" &&
      selectedObservations.has("swap-mouse-button")
    ) {
      scheduleSwapMouseButtonStateObserver(module);
    }
    if (
      module.name.toLowerCase() === "winmm.dll" &&
      selectedObservations.has("time-get-dev-caps")
    ) {
      scheduleTimeGetDevCapsOracle(module);
    }
    if (
      module.name.toLowerCase() === "winmm.dll" &&
      selectedObservations.has("time-get-time")
    ) {
      try {
        attachTimeGetTimeHook(module);
      } catch (error) {
        emit("hook-error", {
          module: module.name,
          api: "timeGetTime",
          error: error.message,
        });
      }
      scheduleTimeGetTimeOracle(module);
    }
    if (
      module.name.toLowerCase() === "ole32.dll" &&
      selectedObservations.has("com-initialization")
    ) {
      scheduleComInitializationOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("processor-feature")
    ) {
      scheduleProcessorFeatureOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("unhandled-exception-filter")
    ) {
      scheduleUnhandledExceptionFilterOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("heap-size")
    ) {
      scheduleHeapSizeOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("tls-lifecycle")
    ) {
      scheduleTlsLifecycleOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("fls-lifecycle")
    ) {
      scheduleFlsLifecycleOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("get-current-thread")
    ) {
      try {
        attachGetCurrentThreadCallSites(module);
      } catch (error) {
        emit("get-current-thread-callsite-error", { error: error.message });
      }
      scheduleGetCurrentThreadOracle(module);
    }
    if (
      module.name.toLowerCase() === "kernel32.dll" &&
      selectedObservations.has("set-thread-affinity-mask")
    ) {
      try {
        attachThreadAffinityTickCountObserver(module);
      } catch (error) {
        emit("set-thread-affinity-mask-calibration-error", {
          error: error.message,
        });
      }
      scheduleSetThreadAffinityMaskOracle(module);
    }
  },
});

emit("agent-ready", {
  architecture: Process.arch,
  pointerSize: Process.pointerSize,
  processId: Process.id,
  selectedVtables: [...selectedVtables],
  selectedObservations: [...selectedObservations],
});
