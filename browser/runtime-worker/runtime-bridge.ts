import {
  cpuWritebackControlByteLength,
  type DirtyMirrorPage,
  encodeCpuWritebackControlInto,
  memoryPageSize,
  type MemoryRegionSnapshot,
} from "./memory-transport.js";
import type {
  RuntimeCurrentDirectoryA,
  RuntimeKeyboardInputEvent,
  RuntimePointerButtonInputEvent,
  RuntimePointerPositionInputEvent,
} from "./protocol.js";
import type { ExecutionCpuContext } from "./execution-engine-contract.js";

export type RegisterUpdate =
  | { action: "preserve" }
  | { action: "set"; value: number };

export interface CpuResume {
  instructionPointer: number;
  stackPointer: number;
  eax: RegisterUpdate;
  edx: RegisterUpdate;
  eflags: RegisterUpdate;
}

export interface ApiCompletion extends CpuResume {
  implementationId: string;
  frame?: {
    stackPointer: number;
    arguments: Array<{ words: number[] }>;
  };
  readonly lastError: number;
  executionBoundary?:
    | "yield-current-thread"
    | "block-current-thread"
    | "terminate-current-thread";
  guestWrites?: Array<{
    write:
      | { address: number; bytes: number[] }
      | { address: number; byteLength: number; contentSha256: string };
  }>;
}

export interface RuntimeThreadStackMetadata {
  threadId: number;
  addressAllocationId: number;
  memoryOwner: number;
  reservedRange: { start: number; byteLength: number };
  committedRange: { start: number; byteLength: number };
  guardPage?: { start: number; byteLength: number };
  committedPages: number;
  stackTop: number;
  initialStackPointer: number;
  returnSentinel: number;
}

export interface PreparedMetadata {
  kind: "prepared";
  schemaVersion: 2;
  workload?: "controlled-fixture" | "controlled-source-read" | "private-bgi";
  entryPoint: number;
  fsBase?: number;
  generation: number;
  stackTop: number;
  initialStackPointer: number;
  initialThreadStack?: RuntimeThreadStackMetadata;
  initialStackAddressAllocation?: {
    id: number;
    purpose: { kind: "thread-stack" };
    owner: string;
    range: { base: number; byteLength: number };
    usableByteLength: number;
    alignment: number;
    guardPages: number;
    classification: "stack";
    lifetime: "thread";
    releasePolicy: "explicit";
    executable: boolean;
  };
  callbackSentinel: number;
  entrySentinel: number;
  pageCount: number;
  maximumPhysicalPages: number;
  regions: MemoryRegionSnapshot[];
  runtime: {
    processId: number;
    threadId: number;
    currentDirectoryA: RuntimeCurrentDirectoryA;
    currentDirectoryGeneration: number;
    kernel32Policy?: string;
    user32Policy?: string;
    cursorPosition?: {
      profileId: string;
      screenX: number;
      screenY: number;
      generation: number;
    };
  };
  sourceSnapshot?: {
    titleFingerprint: string;
    snapshotId: string;
    sourceId: number;
    rootPathA: number[];
    files: Record<
      string,
      {
        objectId: number;
        relativePathA: number[];
        byteLength: number;
        sha256: string;
      }
    >;
  };
  writableProfile?: {
    titleFingerprint: string;
    profileId: string;
    rootPathA: number[];
    quotaBytes: number;
    files: Record<
      string,
      {
        objectId: number;
        relativePathA: number[];
        exists: boolean;
        generation: number;
        byteLength: number;
        sha256: string | null;
        dirty: boolean;
      }
    >;
    directories: Record<
      string,
      {
        directoryId: number;
        relativePathA: number[];
        exists: boolean;
        generation: number;
      }
    >;
  };
  endpoints: Array<{
    id: number;
    address: number;
    module?: string;
    symbol?: { kind: "name"; name: string } | { kind: "ordinal"; ordinal: number };
    iatAddress?: number;
  }>;
  additionalThreads?: Array<{
    threadId: number;
    entryPoint: number;
    initialStackPointer: number;
    fsBase: number;
    returnSentinel: number;
    markerAddress: number;
    expectedMarkerValue: number;
    expectedExitCode: number;
  }>;
}

export interface BridgeOutput<T = Record<string, unknown>> {
  metadata: T;
  control: Uint8Array;
  payload: Uint8Array;
}

export type PreparedEndpoint = PreparedMetadata["endpoints"][number];

interface FastApiCompletionMetadata {
  kind: "api-complete";
  endpoint: {
    id: number;
    address: number;
    module: string;
    symbol: { kind: "name"; name: string } | { kind: "ordinal"; ordinal: number };
  };
  completion: ApiCompletion;
  threadSelection?: {
    previousThreadId: number;
    threadId: number;
    state: { state: "runnable" } | { state: "in-callback"; depth: number };
  };
}

interface RuntimeBridgeExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  vine_bridge_input_resize(length: number): number;
  vine_bridge_input_ptr(): number;
  vine_bridge_metadata_ptr(): number;
  vine_bridge_metadata_len(): number;
  vine_bridge_control_ptr(): number;
  vine_bridge_control_len(): number;
  vine_bridge_payload_ptr(): number;
  vine_bridge_payload_len(): number;
  vine_bridge_prepare(length: number, encodedPointerCookie: number): number;
  vine_bridge_prepare_bgi(
    executableLength: number,
    policyLength: number,
    sourceManifestLength: number,
    writableProfileLength: number,
    d3d9ClearEnabled: number,
    setRectEnabled: number,
    d3d9AddDirtyRectEnabled: number,
    d3d9UpdateTextureEnabled: number,
    d3d9BeginSceneEnabled: number,
    d3d9InitialSamplerStatesEnabled: number,
    d3d9InitialTextureStageStatesEnabled: number,
    d3d9SetTextureEnabled: number,
    d3d9SetFvfEnabled: number,
    d3d9SetStreamSourceEnabled: number,
    d3d9DrawPrimitiveEnabled: number,
    d3d9EndSceneEnabled: number,
    d3d9PresentEnabled: number,
    user32AsyncKeyStateInitialScanEnabled: number,
    heapAlloc24ByteBufferEnabled: number,
    postHeapTimeGetTimeEnabled: number,
    user32WindowTimerCallbackDeliveryEnabled: number,
    user32PointerButtonMessagesEnabled: number,
    user32SetFocusSameWindowEnabled: number,
    user32DefWindowProcAPointerDownEnabled: number,
    hostMonotonicClockEnabled: number,
    encodedPointerCookie: number,
  ): number;
  vine_bridge_synchronize_forward_clock_v1(
    retiredInstructionsLow: number,
    retiredInstructionsHigh: number,
    hostElapsedTime100nsLow: number,
    hostElapsedTime100nsHigh: number,
  ): number;
  vine_bridge_prepare_source_read(
    executableLength: number,
    policyLength: number,
    sourceManifestLength: number,
    encodedPointerCookie: number,
  ): number;
  vine_bridge_acknowledge(low: number, high: number): number;
  vine_bridge_fast_path_abi_version(): number;
  vine_bridge_fast_dispatch_ptr_v3(): number;
  vine_bridge_fast_dispatch_len_v3(): number;
  vine_bridge_acknowledge_fast_v1(low: number, high: number): number;
  vine_bridge_reconcile_writeback(controlLength: number, payloadLength: number): number;
  vine_bridge_reconcile_writeback_fast_v1(
    controlLength: number,
    payloadLength: number,
  ): number;
  vine_bridge_fast_writeback_generation_low_v1(): number;
  vine_bridge_fast_writeback_generation_high_v1(): number;
  vine_bridge_fast_writeback_touched_pages_v1(): number;
  vine_bridge_fast_writeback_changed_pages_v1(): number;
  vine_bridge_advance_mapping_fixture(stage: number): number;
  vine_bridge_current_reflection(): number;
  vine_bridge_current_memory_summary(): number;
  vine_bridge_service_page_fault(faultAddress: number, errorCode: number): number;
  vine_bridge_dispatch(
    endpoint: number,
    stackPointer: number,
    returnAddress: number,
    retiredInstructionsLow: number,
    retiredInstructionsHigh: number,
    eax: number,
    ecx: number,
    edx: number,
    ebx: number,
    esp: number,
    ebp: number,
    esi: number,
    edi: number,
    eip: number,
    eflags: number,
    es: number,
    cs: number,
    ss: number,
    ds: number,
    fs: number,
    gs: number,
  ): number;
  vine_bridge_dispatch_fast_v3(
    endpoint: number,
    stackPointer: number,
    returnAddress: number,
    retiredInstructionsLow: number,
    retiredInstructionsHigh: number,
    eax: number,
    ecx: number,
    edx: number,
    ebx: number,
    esp: number,
    ebp: number,
    esi: number,
    edi: number,
    eip: number,
    eflags: number,
    es: number,
    cs: number,
    ss: number,
    ds: number,
    fs: number,
    gs: number,
  ): number;
  vine_bridge_resolve_com_endpoint(endpoint: number): number;
  vine_bridge_dispatch_com(
    endpoint: number,
    stackPointer: number,
    returnAddress: number,
    retiredInstructionsLow: number,
    retiredInstructionsHigh: number,
  ): number;
  vine_bridge_complete_host(low: number, high: number, result: number): number;
  vine_bridge_complete_source_read(
    low: number,
    high: number,
    controlLength: number,
    payloadLength: number,
  ): number;
  vine_bridge_complete_gdi_text_raster(
    low: number,
    high: number,
    payloadLength: number,
  ): number;
  vine_bridge_writable_commit_payload(low: number, high: number): number;
  vine_bridge_complete_writable_commit(
    low: number,
    high: number,
    controlLength: number,
  ): number;
  vine_bridge_complete_writable_delete(
    low: number,
    high: number,
    controlLength: number,
  ): number;
  vine_bridge_complete_writable_directory_create(
    low: number,
    high: number,
    controlLength: number,
  ): number;
  vine_bridge_hydrate_writable_profile_object(
    objectId: number,
    generationLow: number,
    generationHigh: number,
    inputLength: number,
  ): number;
  vine_bridge_ingest_keyboard_event(
    sequenceLow: number,
    sequenceHigh: number,
    kind: number,
    virtualKey: number,
    repeat: number,
  ): number;
  vine_bridge_ingest_pointer_button_event(
    sequenceLow: number,
    sequenceHigh: number,
    kind: number,
    button: number,
    normalizedX: number,
    normalizedY: number,
  ): number;
  vine_bridge_ingest_pointer_position_event(
    sequenceLow: number,
    sequenceHigh: number,
    normalizedX: number,
    normalizedY: number,
  ): number;
  vine_bridge_schedule_next_thread(): number;
  vine_bridge_finish_thread(exitCode: number): number;
  vine_bridge_complete_callback(sentinel: number, result: number): number;
  vine_bridge_cancel(): number;
  vine_bridge_finish(exitCode: number): number;
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function failFastDispatch(message: string): never {
  throw new Error(message);
}

function splitU64(value: bigint): [number, number] {
  invariant(value >= 0n && value <= 0xffff_ffff_ffff_ffffn, "64-bit bridge value is out of range");
  return [Number(value & 0xffff_ffffn), Number((value >> 32n) & 0xffff_ffffn)];
}

function splitRetiredInstructions(value: number): [number, number] {
  invariant(
    Number.isSafeInteger(value) && value >= 0,
    "retired instruction cursor must be a non-negative safe integer",
  );
  return splitU64(BigInt(value));
}

function combineU64(low: number, high: number): bigint {
  return BigInt(low >>> 0) | (BigInt(high >>> 0) << 32n);
}

const emptyBridgeOutput = new Uint8Array(0);

function createEncodedPointerCookie(): number {
  const words = new Uint32Array(1);
  let cookie = 0;
  do {
    globalThis.crypto.getRandomValues(words);
    cookie = words[0] ?? 0;
  } while (cookie === 0);
  return cookie >>> 0;
}

export class RuntimeBridge {
  readonly #exports: RuntimeBridgeExports;
  readonly #metadataDecoder = new TextDecoder();
  readonly #sharedWritebackOffsets: number[] = [];
  #callCount = 0;

  private constructor(exports: RuntimeBridgeExports) {
    this.#exports = exports;
    invariant(
      this.#exports.vine_bridge_fast_path_abi_version() === 3,
      "unsupported runtime bridge fast-path ABI",
    );
  }

  static async instantiate(url: URL): Promise<RuntimeBridge> {
    const response = await fetch(url);
    invariant(response.ok, `runtime core fetch failed with ${response.status}`);
    const bytes = await response.arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return new RuntimeBridge(instance.exports as RuntimeBridgeExports);
  }

  static fromExports(exports: WebAssembly.Exports): RuntimeBridge {
    return new RuntimeBridge(exports as RuntimeBridgeExports);
  }

  prepare(fixture: Uint8Array): BridgeOutput<PreparedMetadata> {
    this.#setInput([fixture]);
    const encodedPointerCookie = createEncodedPointerCookie();
    return this.#invoke(() =>
      this.#exports.vine_bridge_prepare(fixture.byteLength, encodedPointerCookie),
    );
  }

  prepareBgi(
    executable: Uint8Array,
    policy: Uint8Array,
    sourceManifest: Uint8Array | null,
    writableProfile: Uint8Array,
    d3d9ClearEnabled: boolean,
    setRectEnabled: boolean,
    d3d9AddDirtyRectEnabled: boolean,
    d3d9UpdateTextureEnabled: boolean,
    d3d9UpdateTexturePartialDirtyRegionEnabled: boolean,
    d3d9BeginSceneEnabled: boolean,
    d3d9InitialSamplerStatesEnabled: boolean,
    d3d9InitialTextureStageStatesEnabled: boolean,
    d3d9SetTextureEnabled: boolean,
    d3d9SetFvfEnabled: boolean,
    d3d9SetStreamSourceEnabled: boolean,
    d3d9DrawPrimitiveEnabled: boolean,
    d3d9EndSceneEnabled: boolean,
    d3d9PresentEnabled: boolean,
    user32AsyncKeyStateInitialScanEnabled: boolean,
    heapAlloc24ByteBufferEnabled: boolean,
    postHeapTimeGetTimeEnabled: boolean,
    user32WindowTimerCallbackDeliveryEnabled: boolean,
    user32PointerButtonMessagesEnabled: boolean,
    user32SetFocusSameWindowEnabled: boolean,
    user32DefWindowProcAPointerDownEnabled: boolean,
    user32DefWindowProcAPointerReleaseEnabled: boolean,
    hostMonotonicClockEnabled: boolean,
  ): BridgeOutput<PreparedMetadata> {
    const source = sourceManifest ?? emptyBridgeOutput;
    this.#setInput([executable, policy, source, writableProfile]);
    const encodedPointerCookie = createEncodedPointerCookie();
    return this.#invoke(() =>
      this.#exports.vine_bridge_prepare_bgi(
        executable.byteLength,
        policy.byteLength,
        source.byteLength,
        writableProfile.byteLength,
        Number(d3d9ClearEnabled),
        Number(setRectEnabled),
        Number(d3d9AddDirtyRectEnabled),
        d3d9UpdateTexturePartialDirtyRegionEnabled
          ? 2
          : Number(d3d9UpdateTextureEnabled),
        Number(d3d9BeginSceneEnabled),
        Number(d3d9InitialSamplerStatesEnabled),
        Number(d3d9InitialTextureStageStatesEnabled),
        Number(d3d9SetTextureEnabled),
        Number(d3d9SetFvfEnabled),
        Number(d3d9SetStreamSourceEnabled),
        Number(d3d9DrawPrimitiveEnabled),
        Number(d3d9EndSceneEnabled),
        Number(d3d9PresentEnabled),
        Number(user32AsyncKeyStateInitialScanEnabled),
        Number(heapAlloc24ByteBufferEnabled),
        Number(postHeapTimeGetTimeEnabled),
        Number(user32WindowTimerCallbackDeliveryEnabled),
        Number(user32PointerButtonMessagesEnabled),
        Number(user32SetFocusSameWindowEnabled),
        user32DefWindowProcAPointerReleaseEnabled
          ? 2
          : Number(user32DefWindowProcAPointerDownEnabled),
        Number(hostMonotonicClockEnabled),
        encodedPointerCookie,
      ),
    );
  }

  synchronizeForwardClock(retiredInstructions: number, elapsedMilliseconds: number): void {
    invariant(
      Number.isFinite(elapsedMilliseconds) && elapsedMilliseconds >= 0,
      "forward clock elapsed time must be finite and nonnegative",
    );
    const [retiredInstructionsLow, retiredInstructionsHigh] =
      splitRetiredInstructions(retiredInstructions);
    const [elapsedLow, elapsedHigh] = splitU64(
      BigInt(Math.floor(elapsedMilliseconds * 10_000)),
    );
    this.#checkFastResult(
      this.#exports.vine_bridge_synchronize_forward_clock_v1(
        retiredInstructionsLow,
        retiredInstructionsHigh,
        elapsedLow,
        elapsedHigh,
      ),
    );
  }

  prepareSourceRead(
    executable: Uint8Array,
    policy: Uint8Array,
    sourceManifest: Uint8Array,
  ): BridgeOutput<PreparedMetadata> {
    this.#setInput([executable, policy, sourceManifest]);
    const encodedPointerCookie = createEncodedPointerCookie();
    return this.#invoke(() =>
      this.#exports.vine_bridge_prepare_source_read(
        executable.byteLength,
        policy.byteLength,
        sourceManifest.byteLength,
        encodedPointerCookie,
      ),
    );
  }

  acknowledge<T = Record<string, unknown>>(generation: bigint): BridgeOutput<T> {
    const [low, high] = splitU64(generation);
    return this.#invoke(() => this.#exports.vine_bridge_acknowledge(low, high));
  }

  acknowledgeFast(generation: bigint): void {
    invariant(
      generation >= 0n && generation <= 0xffff_ffff_ffff_ffffn,
      "64-bit bridge value is out of range",
    );
    const low = Number(generation & 0xffff_ffffn);
    const high = Number((generation >> 32n) & 0xffff_ffffn);
    this.#checkFastResult(this.#exports.vine_bridge_acknowledge_fast_v1(low, high));
  }

  reconcileWriteback<T = Record<string, unknown>>(
    control: Uint8Array,
    payload: Uint8Array,
  ): BridgeOutput<T> {
    this.#setInput([control, payload]);
    return this.#invoke(() =>
      this.#exports.vine_bridge_reconcile_writeback(control.byteLength, payload.byteLength),
    );
  }

  reconcileWritebackPages(
    generation: bigint,
    pages: DirtyMirrorPage[],
  ): bigint {
    const controlLength = cpuWritebackControlByteLength(pages.length);
    const payloadLength = pages.length * memoryPageSize;
    const sourceBuffer = this.#exports.memory.buffer;
    const sharedOffsets = this.#sharedWritebackOffsets;
    sharedOffsets.length = 0;
    if (pages.every((page) => page.bytes.buffer === sourceBuffer)) {
      for (const page of pages) {
        sharedOffsets.push(page.bytes.byteOffset);
      }
    }
    const input = this.#resizeInput(controlLength + payloadLength);
    if (
      sharedOffsets.length === pages.length &&
      sourceBuffer !== this.#exports.memory.buffer
    ) {
      for (let index = 0; index < pages.length; index += 1) {
        const byteOffset = sharedOffsets[index]!;
        invariant(
          byteOffset + memoryPageSize <= this.#exports.memory.buffer.byteLength,
          "shared writeback page moved outside linear memory",
        );
        pages[index]!.bytes = new Uint8Array(
          this.#exports.memory.buffer,
          byteOffset,
          memoryPageSize,
        );
      }
    }
    encodeCpuWritebackControlInto(input, generation, pages);
    let offset = controlLength;
    for (const page of pages) {
      input.set(page.bytes, offset);
      offset += page.bytes.byteLength;
    }
    this.#checkFastResult(
      this.#exports.vine_bridge_reconcile_writeback_fast_v1(controlLength, payloadLength),
    );
    const touchedPages = this.#exports.vine_bridge_fast_writeback_touched_pages_v1();
    const changedPages = this.#exports.vine_bridge_fast_writeback_changed_pages_v1();
    invariant(touchedPages === pages.length, "writeback page count mismatch");
    invariant(changedPages <= touchedPages, "writeback changed-page count mismatch");
    return combineU64(
      this.#exports.vine_bridge_fast_writeback_generation_low_v1(),
      this.#exports.vine_bridge_fast_writeback_generation_high_v1(),
    );
  }

  advanceMappingFixture<T = Record<string, unknown>>(stage: number): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_advance_mapping_fixture(stage));
  }

  currentReflection<T = Record<string, unknown>>(): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_current_reflection());
  }

  currentMemorySummary<T = Record<string, unknown>>(): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_current_memory_summary());
  }

  servicePageFault<T = Record<string, unknown>>(
    faultAddress: number,
    errorCode: number,
  ): BridgeOutput<T> {
    return this.#invoke(() =>
      this.#exports.vine_bridge_service_page_fault(faultAddress, errorCode),
    );
  }

  dispatch<T = Record<string, unknown>>(
    endpoint: number,
    stackPointer: number,
    returnAddress: number,
    retiredInstructions: number,
    context: ExecutionCpuContext,
  ): BridgeOutput<T> {
    const [retiredInstructionsLow, retiredInstructionsHigh] =
      splitRetiredInstructions(retiredInstructions);
    return this.#invoke(() =>
      this.#exports.vine_bridge_dispatch(
        endpoint,
        stackPointer,
        returnAddress,
        retiredInstructionsLow,
        retiredInstructionsHigh,
        context.eax,
        context.ecx,
        context.edx,
        context.ebx,
        context.esp,
        context.ebp,
        context.esi,
        context.edi,
        context.instructionPointer,
        context.eflags,
        context.es,
        context.cs,
        context.ss,
        context.ds,
        context.fs,
        context.gs,
      ),
    );
  }

  dispatchForward<T = Record<string, unknown>>(
    endpoint: PreparedEndpoint,
    stackPointer: number,
    returnAddress: number,
    retiredInstructions: number,
    context: ExecutionCpuContext,
  ): BridgeOutput<T> {
    const [retiredInstructionsLow, retiredInstructionsHigh] =
      splitRetiredInstructions(retiredInstructions);
    this.#callCount += 1;
    const status = this.#exports.vine_bridge_dispatch_fast_v3(
      endpoint.address,
      stackPointer,
      returnAddress,
      retiredInstructionsLow,
      retiredInstructionsHigh,
      context.eax,
      context.ecx,
      context.edx,
      context.ebx,
      context.esp,
      context.ebp,
      context.esi,
      context.edi,
      context.instructionPointer,
      context.eflags,
      context.es,
      context.cs,
      context.ss,
      context.ds,
      context.fs,
      context.gs,
    );
    if (status !== 0) {
      throw new Error(this.#readMetadataError());
    }
    return this.#readForwardDispatchOutput(endpoint, stackPointer);
  }

  takePendingForward<T = Record<string, unknown>>(
    endpoint: PreparedEndpoint,
    stackPointer: number,
  ): BridgeOutput<T> {
    return this.#readForwardDispatchOutput(endpoint, stackPointer);
  }

  #readForwardDispatchOutput<T>(
    endpoint: PreparedEndpoint,
    stackPointer: number,
  ): BridgeOutput<T> {
    const fastLength = this.#exports.vine_bridge_fast_dispatch_len_v3();
    if (fastLength === 0) {
      return this.#readOutput<T>();
    }
    invariant(fastLength === 36, "runtime bridge returned an invalid fast dispatch record length");
    const pointer = this.#exports.vine_bridge_fast_dispatch_ptr_v3();
    invariant(
      pointer % Uint32Array.BYTES_PER_ELEMENT === 0 &&
        pointer + fastLength * Uint32Array.BYTES_PER_ELEMENT <=
          this.#exports.memory.buffer.byteLength,
      "runtime bridge returned an invalid fast dispatch record pointer",
    );
    const words = new Uint32Array(this.#exports.memory.buffer, pointer, fastLength);
    const metadata = this.#decodeFastDispatch(endpoint, words, stackPointer);
    return {
      metadata: metadata as T,
      control: this.#copyOutput(
        this.#exports.vine_bridge_control_ptr(),
        this.#exports.vine_bridge_control_len(),
      ),
      payload: this.#copyOutput(
        this.#exports.vine_bridge_payload_ptr(),
        this.#exports.vine_bridge_payload_len(),
      ),
    };
  }

  resolveComEndpoint<T = Record<string, unknown>>(endpoint: number): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_resolve_com_endpoint(endpoint));
  }

  dispatchCom<T = Record<string, unknown>>(
    endpoint: number,
    stackPointer: number,
    returnAddress: number,
    retiredInstructions: number,
  ): BridgeOutput<T> {
    const [retiredInstructionsLow, retiredInstructionsHigh] =
      splitRetiredInstructions(retiredInstructions);
    return this.#invoke(() =>
      this.#exports.vine_bridge_dispatch_com(
        endpoint,
        stackPointer,
        returnAddress,
        retiredInstructionsLow,
        retiredInstructionsHigh,
      ),
    );
  }

  completeHost<T = Record<string, unknown>>(requestId: bigint, result: number): BridgeOutput<T> {
    const [low, high] = splitU64(requestId);
    return this.#invoke(() => this.#exports.vine_bridge_complete_host(low, high, result));
  }

  completeSourceRead<T = Record<string, unknown>>(
    requestId: bigint,
    control: Uint8Array,
    payload: Uint8Array,
  ): BridgeOutput<T> {
    this.#setInput([control, payload]);
    const [low, high] = splitU64(requestId);
    return this.#invoke(() =>
      this.#exports.vine_bridge_complete_source_read(
        low,
        high,
        control.byteLength,
        payload.byteLength,
      ),
    );
  }

  completeGdiTextRaster<T = Record<string, unknown>>(
    requestId: bigint,
    pixels: Uint8Array,
  ): BridgeOutput<T> {
    this.#setInput([pixels]);
    const [low, high] = splitU64(requestId);
    return this.#invoke(() =>
      this.#exports.vine_bridge_complete_gdi_text_raster(
        low,
        high,
        pixels.byteLength,
      ),
    );
  }

  writableCommitPayload<T = Record<string, unknown>>(
    requestId: bigint,
  ): BridgeOutput<T> {
    const [low, high] = splitU64(requestId);
    return this.#invoke(() => this.#exports.vine_bridge_writable_commit_payload(low, high));
  }

  completeWritableCommit<T = Record<string, unknown>>(
    requestId: bigint,
    control: Uint8Array,
  ): BridgeOutput<T> {
    this.#setInput([control]);
    const [low, high] = splitU64(requestId);
    return this.#invoke(() =>
      this.#exports.vine_bridge_complete_writable_commit(low, high, control.byteLength),
    );
  }

  completeWritableDelete<T = Record<string, unknown>>(
    requestId: bigint,
    control: Uint8Array,
  ): BridgeOutput<T> {
    this.#setInput([control]);
    const [low, high] = splitU64(requestId);
    return this.#invoke(() =>
      this.#exports.vine_bridge_complete_writable_delete(low, high, control.byteLength),
    );
  }

  completeWritableDirectoryCreate<T = Record<string, unknown>>(
    requestId: bigint,
    control: Uint8Array,
  ): BridgeOutput<T> {
    this.#setInput([control]);
    const [low, high] = splitU64(requestId);
    return this.#invoke(() =>
      this.#exports.vine_bridge_complete_writable_directory_create(
        low,
        high,
        control.byteLength,
      ),
    );
  }

  hydrateWritableProfileObject<T = Record<string, unknown>>(
    objectId: number,
    generation: bigint,
    bytes: Uint8Array,
  ): BridgeOutput<T> {
    this.#setInput([bytes]);
    const [low, high] = splitU64(generation);
    return this.#invoke(() =>
      this.#exports.vine_bridge_hydrate_writable_profile_object(
        objectId,
        low,
        high,
        bytes.byteLength,
      ),
    );
  }

  ingestKeyboardEvent<T = Record<string, unknown>>(
    event: RuntimeKeyboardInputEvent,
  ): BridgeOutput<T> {
    invariant(
      /^(0|[1-9][0-9]*)$/.test(event.sequence),
      "keyboard event sequence is not a canonical unsigned integer",
    );
    const sequence = BigInt(event.sequence);
    const [low, high] = splitU64(sequence);
    const kind = {
      "focus-gained": 1,
      "focus-lost": 2,
      "key-down": 3,
      "key-up": 4,
    }[event.kind];
    invariant(kind !== undefined, "keyboard event kind is outside the bridge ABI");
    invariant(
      event.virtualKey === null ||
        (Number.isInteger(event.virtualKey) &&
          event.virtualKey > 0 &&
          event.virtualKey <= 0xff),
      "keyboard event virtual key is outside the supported range",
    );
    return this.#invoke(() =>
      this.#exports.vine_bridge_ingest_keyboard_event(
        low,
        high,
        kind,
        event.virtualKey ?? 0xffff_ffff,
        Number(event.repeat),
      ),
    );
  }

  ingestPointerButtonEvent<T = Record<string, unknown>>(
    event: RuntimePointerButtonInputEvent,
  ): BridgeOutput<T> {
    invariant(
      /^(0|[1-9][0-9]*)$/.test(event.sequence),
      "pointer event sequence is not a canonical unsigned integer",
    );
    const sequence = BigInt(event.sequence);
    const [low, high] = splitU64(sequence);
    const kind = { "button-down": 1, "button-up": 2 }[event.kind];
    const button = { primary: 1, secondary: 2 }[event.button];
    invariant(kind !== undefined, "pointer event kind is outside the bridge ABI");
    invariant(button !== undefined, "pointer button is outside the bridge ABI");
    invariant(
      Number.isInteger(event.normalizedX) &&
        event.normalizedX >= 0 &&
        event.normalizedX <= 0xffff &&
        Number.isInteger(event.normalizedY) &&
        event.normalizedY >= 0 &&
        event.normalizedY <= 0xffff,
      "pointer normalized coordinates are outside the bridge ABI",
    );
    return this.#invoke(() =>
      this.#exports.vine_bridge_ingest_pointer_button_event(
        low,
        high,
        kind,
        button,
        event.normalizedX,
        event.normalizedY,
      ),
    );
  }

  ingestPointerPositionEvent<T = Record<string, unknown>>(
    event: RuntimePointerPositionInputEvent,
  ): BridgeOutput<T> {
    invariant(
      /^(0|[1-9][0-9]*)$/.test(event.sequence),
      "pointer event sequence is not a canonical unsigned integer",
    );
    const sequence = BigInt(event.sequence);
    const [low, high] = splitU64(sequence);
    invariant(
      event.kind === "pointer-move" &&
        Number.isInteger(event.normalizedX) &&
        event.normalizedX >= 0 &&
        event.normalizedX <= 0xffff &&
        Number.isInteger(event.normalizedY) &&
        event.normalizedY >= 0 &&
        event.normalizedY <= 0xffff,
      "pointer position is outside the bridge ABI",
    );
    return this.#invoke(() =>
      this.#exports.vine_bridge_ingest_pointer_position_event(
        low,
        high,
        event.normalizedX,
        event.normalizedY,
      ),
    );
  }

  scheduleNextThread<T = Record<string, unknown>>(): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_schedule_next_thread());
  }

  finishThread<T = Record<string, unknown>>(exitCode: number): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_finish_thread(exitCode));
  }

  completeCallback<T = Record<string, unknown>>(sentinel: number, result: number): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_complete_callback(sentinel, result));
  }

  cancel<T = Record<string, unknown>>(): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_cancel());
  }

  finish<T = Record<string, unknown>>(exitCode: number): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_finish(exitCode));
  }

  callCount(): number {
    return this.#callCount;
  }

  linearMemoryBytes(): number {
    return this.#exports.memory.buffer.byteLength;
  }

  #setInput(parts: Uint8Array[]): void {
    const length = parts.reduce((total, part) => total + part.byteLength, 0);
    const input = this.#resizeInput(length);
    let offset = 0;
    for (const part of parts) {
      input.set(part, offset);
      offset += part.byteLength;
    }
  }

  #resizeInput(length: number): Uint8Array {
    const status = this.#exports.vine_bridge_input_resize(length);
    if (status !== 0) {
      throw new Error(this.#readMetadataError());
    }
    const pointer = this.#exports.vine_bridge_input_ptr();
    return new Uint8Array(this.#exports.memory.buffer, pointer, length);
  }

  #invoke<T>(operation: () => number): BridgeOutput<T> {
    this.#callCount += 1;
    const status = operation();
    const metadata = this.#readJson<T>();
    if (status !== 0) {
      const error = metadata as { message?: unknown };
      throw new Error(typeof error.message === "string" ? error.message : "runtime bridge failed");
    }
    return this.#readOutput<T>();
  }

  #readOutput<T>(): BridgeOutput<T> {
    const metadata = this.#readJson<T>();
    return {
      metadata,
      control: this.#copyOutput(
        this.#exports.vine_bridge_control_ptr(),
        this.#exports.vine_bridge_control_len(),
      ),
      payload: this.#copyOutput(
        this.#exports.vine_bridge_payload_ptr(),
        this.#exports.vine_bridge_payload_len(),
      ),
    };
  }

  #decodeFastDispatch(
    endpoint: PreparedEndpoint,
    words: Uint32Array,
    expectedStackPointer?: number,
  ): FastApiCompletionMetadata {
    invariant(words[0] === 2, "runtime bridge fast dispatch record version mismatch");
    const implementationId = [
      "",
      "user32.get-async-key-state.v1",
      "user32.get-system-metrics.v1",
      "user32.is-iconic.logical-window-style.v1",
      "kernel32.enter-critical-section.v1",
      "kernel32.leave-critical-section.v1",
      "kernel32.heap-alloc.v2",
      "kernel32.heap-free.v1",
      "winmm.time-get-time.v1",
      "user32.get-cursor-pos.v1",
      "user32.get-window-rect.v1",
      "user32.peek-message-a.current-thread.v1",
      "kernel32.set-waitable-timer.relative-one-shot.v1",
      "kernel32.wait-for-single-object-ex.waitable-timer.v1",
      "kernel32.get-last-error.v1",
      "kernel32.set-last-error.v1",
      "kernel32.tls-get-value.v1",
      "kernel32.fls-get-value.v1",
      "user32.get-cursor-info.current-cursor.v2",
      "user32.translate-message.window-timer-pointer-noop.v2",
    ][words[1] ?? 0];
    invariant(implementationId !== undefined && implementationId.length > 0, "unknown fast API implementation");
    const argumentCount = words[19] ?? 0;
    invariant(
      endpoint.module !== undefined &&
        endpoint.symbol !== undefined &&
        words[2] === endpoint.id &&
        words[3] === endpoint.address &&
        (expectedStackPointer === undefined || words[5] === expectedStackPointer) &&
        argumentCount <= 6,
      "runtime bridge fast dispatch endpoint or frame mismatch",
    );
    const register = (action: number, value: number): RegisterUpdate => {
      invariant(action === 0 || action === 1, "runtime bridge returned an invalid register action");
      if (action === 0) {
        invariant(value === 0, "preserved fast dispatch register has a value");
        return { action: "preserve" };
      }
      return { action: "set", value };
    };
    const executionBoundary =
      words[14] === 0
        ? undefined
        : words[14] === 1
          ? "yield-current-thread" as const
          : words[14] === 2
            ? "block-current-thread" as const
            : failFastDispatch("runtime bridge returned an invalid execution boundary");
    const selectionKind = words[15] ?? 0;
    invariant(selectionKind <= 2, "runtime bridge returned an invalid thread selection kind");
    const threadSelection =
      selectionKind === 0
        ? undefined
        : {
            previousThreadId: words[16]!,
            threadId: words[17]!,
            state:
              selectionKind === 1
                ? { state: "runnable" as const }
                : { state: "in-callback" as const, depth: words[18]! },
          };
    if (threadSelection === undefined) {
      invariant(
        words[16] === 0 && words[17] === 0 && words[18] === 0,
        "empty fast dispatch thread selection retained state",
      );
    }
    const writeCount = words[26] ?? 0;
    invariant(writeCount === 0 || writeCount === 1, "invalid fast dispatch guest-write count");
    const guestWrites = [] as NonNullable<ApiCompletion["guestWrites"]>;
    if (writeCount === 1) {
      const byteLength = words[28] ?? 0;
      invariant(
        byteLength > 0 && byteLength <= 28 && byteLength % 4 === 0,
        "invalid fast dispatch guest-write byte length",
      );
      const bytes: number[] = [];
      for (let index = 29; index < 29 + byteLength / 4; index += 1) {
        const value = words[index]!;
        bytes.push(
          value & 0xff,
          (value >>> 8) & 0xff,
          (value >>> 16) & 0xff,
          value >>> 24,
        );
      }
      guestWrites.push({ write: { address: words[27]!, bytes } });
    } else {
      invariant(
        words.slice(27, 36).every((word) => word === 0),
        "empty fast dispatch guest-write record retained data",
      );
    }
    return {
      kind: "api-complete",
      endpoint: {
        id: endpoint.id,
        address: endpoint.address,
        module: endpoint.module,
        symbol: endpoint.symbol,
      },
      completion: {
        implementationId,
        frame: {
          stackPointer: words[5]!,
          arguments: Array.from({ length: argumentCount }, (_, index) => ({
            words: [words[20 + index]!],
          })),
        },
        instructionPointer: words[4]!,
        stackPointer: words[6]!,
        eax: register(words[8]!, words[9]!),
        edx: register(words[10]!, words[11]!),
        eflags: register(words[12]!, words[13]!),
        lastError: words[7]!,
        guestWrites,
        ...(executionBoundary === undefined ? {} : { executionBoundary }),
      },
      ...(threadSelection === undefined ? {} : { threadSelection }),
    };
  }

  #checkFastResult(status: number): void {
    this.#callCount += 1;
    if (status !== 0) {
      throw new Error(this.#readMetadataError());
    }
  }

  #readMetadataError(): string {
    const metadata = this.#readJson<{ message?: unknown }>();
    return typeof metadata.message === "string" ? metadata.message : "runtime bridge input failed";
  }

  #readJson<T>(): T {
    const pointer = this.#exports.vine_bridge_metadata_ptr();
    const length = this.#exports.vine_bridge_metadata_len();
    invariant(length > 0, "runtime bridge returned empty metadata");
    const bytes = new Uint8Array(this.#exports.memory.buffer, pointer, length);
    return JSON.parse(this.#metadataDecoder.decode(bytes)) as T;
  }

  #copyOutput(pointer: number, length: number): Uint8Array {
    if (length === 0) {
      return emptyBridgeOutput;
    }
    return Uint8Array.from(new Uint8Array(this.#exports.memory.buffer, pointer, length));
  }
}
