import {
  cpuWritebackControlByteLength,
  type DirtyMirrorPage,
  encodeCpuWritebackControlInto,
  memoryPageSize,
  type MemoryRegionSnapshot,
} from "./memory-transport.js";
import type { RuntimeCurrentDirectoryA } from "./protocol.js";

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
  executionBoundary?: "yield-current-thread";
  guestWrites?: Array<{
    write: { address: number; bytes: number[] };
  }>;
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
  initialThreadStack?: {
    threadId: number;
    addressAllocationId: number;
    memoryOwner: number;
    reservedRange: { start: number; byteLength: number };
    committedRange: { start: number; byteLength: number };
    guardPage?: { start: number; byteLength: number };
    committedPages: number;
    stackTop: number;
  };
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
  vine_bridge_prepare(length: number): number;
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
  ): number;
  vine_bridge_prepare_source_read(
    executableLength: number,
    policyLength: number,
    sourceManifestLength: number,
  ): number;
  vine_bridge_acknowledge(low: number, high: number): number;
  vine_bridge_fast_path_abi_version(): number;
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
  vine_bridge_grow_initial_thread_stack(faultAddress: number, errorCode: number): number;
  vine_bridge_service_page_fault(faultAddress: number, errorCode: number): number;
  vine_bridge_dispatch(endpoint: number, stackPointer: number, returnAddress: number): number;
  vine_bridge_resolve_com_endpoint(endpoint: number): number;
  vine_bridge_dispatch_com(endpoint: number, stackPointer: number, returnAddress: number): number;
  vine_bridge_complete_host(low: number, high: number, result: number): number;
  vine_bridge_complete_source_read(
    low: number,
    high: number,
    controlLength: number,
    payloadLength: number,
  ): number;
  vine_bridge_writable_commit_payload(low: number, high: number): number;
  vine_bridge_complete_writable_commit(
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

function splitU64(value: bigint): [number, number] {
  invariant(value >= 0n && value <= 0xffff_ffff_ffff_ffffn, "64-bit bridge value is out of range");
  return [Number(value & 0xffff_ffffn), Number((value >> 32n) & 0xffff_ffffn)];
}

function combineU64(low: number, high: number): bigint {
  return BigInt(low >>> 0) | (BigInt(high >>> 0) << 32n);
}

const emptyBridgeOutput = new Uint8Array(0);

export class RuntimeBridge {
  readonly #exports: RuntimeBridgeExports;
  readonly #metadataDecoder = new TextDecoder();
  #callCount = 0;

  private constructor(exports: RuntimeBridgeExports) {
    this.#exports = exports;
    invariant(
      this.#exports.vine_bridge_fast_path_abi_version() === 1,
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

  prepare(fixture: Uint8Array): BridgeOutput<PreparedMetadata> {
    this.#setInput([fixture]);
    return this.#invoke(() => this.#exports.vine_bridge_prepare(fixture.byteLength));
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
    d3d9BeginSceneEnabled: boolean,
    d3d9InitialSamplerStatesEnabled: boolean,
    d3d9InitialTextureStageStatesEnabled: boolean,
    d3d9SetTextureEnabled: boolean,
  ): BridgeOutput<PreparedMetadata> {
    const source = sourceManifest ?? emptyBridgeOutput;
    this.#setInput([executable, policy, source, writableProfile]);
    return this.#invoke(() =>
      this.#exports.vine_bridge_prepare_bgi(
        executable.byteLength,
        policy.byteLength,
        source.byteLength,
        writableProfile.byteLength,
        Number(d3d9ClearEnabled),
        Number(setRectEnabled),
        Number(d3d9AddDirtyRectEnabled),
        Number(d3d9UpdateTextureEnabled),
        Number(d3d9BeginSceneEnabled),
        Number(d3d9InitialSamplerStatesEnabled),
        Number(d3d9InitialTextureStageStatesEnabled),
        Number(d3d9SetTextureEnabled),
      ),
    );
  }

  prepareSourceRead(
    executable: Uint8Array,
    policy: Uint8Array,
    sourceManifest: Uint8Array,
  ): BridgeOutput<PreparedMetadata> {
    this.#setInput([executable, policy, sourceManifest]);
    return this.#invoke(() =>
      this.#exports.vine_bridge_prepare_source_read(
        executable.byteLength,
        policy.byteLength,
        sourceManifest.byteLength,
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
    const input = this.#resizeInput(controlLength + payloadLength);
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

  growInitialThreadStack<T = Record<string, unknown>>(
    faultAddress: number,
    errorCode: number,
  ): BridgeOutput<T> {
    return this.#invoke(() =>
      this.#exports.vine_bridge_grow_initial_thread_stack(faultAddress, errorCode),
    );
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
  ): BridgeOutput<T> {
    return this.#invoke(() =>
      this.#exports.vine_bridge_dispatch(endpoint, stackPointer, returnAddress),
    );
  }

  resolveComEndpoint<T = Record<string, unknown>>(endpoint: number): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_resolve_com_endpoint(endpoint));
  }

  dispatchCom<T = Record<string, unknown>>(
    endpoint: number,
    stackPointer: number,
    returnAddress: number,
  ): BridgeOutput<T> {
    return this.#invoke(() =>
      this.#exports.vine_bridge_dispatch_com(endpoint, stackPointer, returnAddress),
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
