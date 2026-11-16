import {
  cpuWritebackControlByteLength,
  type DirtyMirrorPage,
  encodeCpuWritebackControlInto,
  memoryPageSize,
  type MemoryRegionSnapshot,
} from "./memory-transport.js";
import type { ExecutionBackendSnapshot, ExecutionCpuContext } from "./execution-engine-contract.js";
import type { RuntimeKeyboardInputEvent, RuntimePointerInputEvent } from "./protocol.js";

export interface ExecutablePreparedMetadata {
  kind: "prepared";
  entryPoint: number;
  codeEntries: number[];
  codeEntriesLimited: boolean;
  fsBase: number;
  initialStackPointer: number;
  entrySentinel: number;
  callbackSentinel: number;
  dllAttach: { target: number; stackPointer: number } | null;
  maximumPhysicalPages: number;
  regions: MemoryRegionSnapshot[];
  runtime: { threadId: number };
}

export interface BridgeOutput<T = Record<string, unknown>> {
  metadata: T;
  control: Uint8Array;
  payload: Uint8Array;
  presentation: Uint8Array;
  audio: Uint8Array;
  mediaBytes: Uint8Array;
}

export interface RuntimeStopContext {
  kind: "stop-context";
  threadId: number | null;
  instructionPointer: number;
  lastError: number | null;
  module: string | null;
  symbol: string | null;
}

interface CpuSnapshotReceipt {
  kind: "cpu-snapshot" | "cpu-snapshot-stored";
  threadId: number;
  revisionLow: number;
  revisionHigh: number;
  backendTag: number;
  abiVersion: number;
  byteLength: number;
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
  vine_bridge_presentation_ptr(): number;
  vine_bridge_presentation_len(): number;
  vine_bridge_audio_ptr(): number;
  vine_bridge_audio_len(): number;
  vine_bridge_media_ptr(): number;
  vine_bridge_media_len(): number;
  vine_bridge_read_com_media(low: number, high: number, offsetLow: number, offsetHigh: number, length: number): number;
  vine_bridge_complete_com_media(low: number, high: number, metadataLength: number, payloadLength: number): number;
  vine_bridge_complete_com_audio(low: number, high: number, accepted: number): number;
  vine_bridge_observe_audio_end(low: number, high: number): number;
  vine_bridge_inspect_executable(executableLength: number, pathLength: number): number;
  vine_bridge_prepare_executable(executableLength: number, pathLength: number, cookie: number): number;
  vine_bridge_ready_object_wait_request_id(): bigint;
  vine_bridge_register_source_dll(imageLength: number, pathLength: number): number;
  vine_bridge_complete_source_lookup(length: number): number;
  vine_bridge_synchronize_forward_clock(low: number, high: number, elapsedLow: number, elapsedHigh: number): number;
  vine_bridge_read_timestamp_counter(low: number, high: number, elapsedLow: number, elapsedHigh: number): number;
  vine_bridge_acknowledge_fast(low: number, high: number): number;
  vine_bridge_reconcile_writeback(controlLength: number, payloadLength: number): number;
  vine_bridge_writeback_generation_low(): number;
  vine_bridge_writeback_generation_high(): number;
  vine_bridge_service_page_fault(address: number, errorCode: number): number;
  vine_bridge_ingest_keyboard_event(low: number, high: number, kind: number, virtualKey: number, repeat: number): number;
  vine_bridge_ingest_pointer_event(low: number, high: number, kind: number, button: number, x: number, y: number): number;
  vine_bridge_dispatch(
    endpoint: number, stackPointer: number, returnAddress: number, low: number, high: number,
    eax: number, ecx: number, edx: number, ebx: number, esp: number, ebp: number,
    esi: number, edi: number, eip: number, eflags: number,
    es: number, cs: number, ss: number, ds: number, fs: number, gs: number,
  ): number;
  vine_bridge_api_completion_ptr(): number;
  vine_bridge_complete_host(low: number, high: number, result: number): number;
  vine_bridge_complete_opfs_open(low: number, high: number, observation: number, sizeLow: number, sizeHigh: number): number;
  vine_bridge_complete_opfs_write(low: number, high: number, observation: number): number;
  vine_bridge_complete_source_read(low: number, high: number, controlLength: number, payloadLength: number): number;
  vine_bridge_complete_gdi_text_raster(low: number, high: number, payloadLength: number): number;
  vine_bridge_schedule_next_thread(): number;
  vine_bridge_finish_thread(exitCode: number): number;
  vine_bridge_cpu_snapshot_put(threadId: number, revisionLow: number, revisionHigh: number,
    backendTag: number, abiVersion: number, inputLength: number): number;
  vine_bridge_cpu_snapshot_get(threadId: number): number;
  vine_bridge_complete_callback(sentinel: number, result: number): number;
  vine_bridge_stop_context(instructionPointer: number): number;
  vine_bridge_stop_context_ptr(): number;
  vine_bridge_stop_context_len(): number;
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function splitU64(value: bigint): [number, number] {
  invariant(value >= 0n && value <= 0xffff_ffff_ffff_ffffn, "64-bit bridge value is out of range");
  return [Number(value & 0xffff_ffffn), Number(value >> 32n)];
}

function instructionWords(value: number): [number, number] {
  invariant(Number.isSafeInteger(value) && value >= 0, "Invalid retired instruction cursor");
  return splitU64(BigInt(value));
}

function pointerCookie(): number {
  const words = new Uint32Array(1);
  do { crypto.getRandomValues(words); } while (words[0] === 0);
  return words[0]!;
}

const emptyBytes = new Uint8Array(0);
const inputLimit = 16 * 1024 * 1024;

export class RuntimeBridge {
  readonly #exports: RuntimeBridgeExports;
  readonly #decoder = new TextDecoder();
  readonly #sharedWritebackOffsets: number[] = [];

  private constructor(exports: RuntimeBridgeExports) {
    this.#exports = exports;
  }

  static async instantiate(url: URL): Promise<RuntimeBridge> {
    const response = await fetch(url);
    invariant(response.ok, `Runtime core fetch failed with ${response.status}`);
    const { instance } = await WebAssembly.instantiate(await response.arrayBuffer(), {});
    return RuntimeBridge.fromExports(instance.exports);
  }

  static fromExports(exports: WebAssembly.Exports): RuntimeBridge {
    return new RuntimeBridge(exports as RuntimeBridgeExports);
  }

  sharedGuestMemory(): import("./memory-transport.js").SharedGuestMemory | undefined {
    const base = this.#exports.vine_bridge_shared_arena_base;
    const offset = this.#exports.vine_bridge_shared_guest_offset;
    const size = this.#exports.vine_bridge_shared_arena_size;
    if (typeof base !== "function" || typeof offset !== "function" || typeof size !== "function") return undefined;
    const physicalBase = base() >>> 0, guestOffset = offset() >>> 0, physicalBytes = size() >>> 0;
    invariant(physicalBase > 0 && guestOffset === 0x800000 && physicalBytes === 0x80800000,
      "shared Runtime has no live guest arena");
    return { memory: this.#exports.memory, guestBase: physicalBase + guestOffset,
      guestBytes: physicalBytes - guestOffset };
  }

  inspectExecutable(executable: Uint8Array, relativePath: string): {
    sha256: string; imageBase: number; entryPoint: number; importModules: number; guestImports: string[];
  } {
    const path = new TextEncoder().encode(relativePath);
    this.#setInput([executable, path]);
    return this.#invoke<{
      sha256: string; imageBase: number; entryPoint: number; importModules: number; guestImports: string[];
    }>(() => this.#exports.vine_bridge_inspect_executable(executable.length, path.length)).metadata;
  }

  registerSourceDll(image: Uint8Array, relativePath: string): {
    sourceId: number; sha256: string; guestImports: string[];
  } {
    const path = new TextEncoder().encode(relativePath);
    this.#setInput([image, path]);
    return this.#invoke<{sourceId: number; sha256: string; guestImports: string[]}>(() =>
      this.#exports.vine_bridge_register_source_dll(image.length, path.length)).metadata;
  }

  prepareExecutable(executable: Uint8Array, relativePath: string): BridgeOutput<ExecutablePreparedMetadata> {
    const path = new TextEncoder().encode(relativePath);
    this.#setInput([executable, path]);
    return this.#invoke(() => this.#exports.vine_bridge_prepare_executable(
      executable.length, path.length, pointerCookie(),
    ));
  }

  readyObjectWaitRequestId(): bigint {
    const id = this.#exports.vine_bridge_ready_object_wait_request_id();
    invariant(typeof id === "bigint" && id >= -(1n << 63n) && id < (1n << 63n),
      "Runtime returned an invalid object wait request");
    // Wasm exposes an i64 result as a signed BigInt even for Rust u64.
    return BigInt.asUintN(64, id);
  }

  completeSourceLookup(entry: { kind: "file"; byteLength: number } | { kind: "directory" } | null): void {
    const input = new TextEncoder().encode(JSON.stringify(entry));
    this.#setInput([input]);
    const result = this.#invoke<{ kind: string; found: boolean }>(() =>
      this.#exports.vine_bridge_complete_source_lookup(input.length));
    invariant(result.metadata.kind === "source-lookup-complete" &&
      result.metadata.found === (entry !== null), "Source lookup completion disagrees with the browser entry");
  }

  sourceSearchNames(listing: { pathSegments: string[]; entries: Array<{ name: string; kind: "file" | "directory" }> } | null): number[][] | null {
    const input = new TextEncoder().encode(JSON.stringify(listing));
    this.#setInput([input]);
    const result = this.#invoke<{
      kind: "source-search-matches" | "source-search-complete"; entries?: number[][];
    }>(() => this.#exports.vine_bridge_complete_source_lookup(input.length));
    if (result.metadata.kind === "source-search-complete") {
      invariant(listing === null, "Source search completed before enumerating a live directory");
      return null;
    }
    invariant(result.metadata.kind === "source-search-matches" && Array.isArray(result.metadata.entries),
      "Source search did not return selected entries");
    return result.metadata.entries!;
  }

  completeSourceSearch(results: Array<{ kind: "file"; byteLength: number } | { kind: "directory" } | null>): Array<{ objectId: number; path: number[] }> {
    const input = new TextEncoder().encode(JSON.stringify(results));
    this.#setInput([input]);
    const result = this.#invoke<{kind: "source-search-complete"; registered: Array<{ objectId: number; path: number[] }> }>(
      () => this.#exports.vine_bridge_complete_source_lookup(input.length),
    );
    invariant(result.metadata.kind === "source-search-complete" && Array.isArray(result.metadata.registered),
      "Source search did not commit its selected entries");
    return result.metadata.registered;
  }

  synchronizeForwardClock(retiredInstructions: number, elapsedMilliseconds: number): void {
    invariant(Number.isFinite(elapsedMilliseconds) && elapsedMilliseconds >= 0,
      "Forward clock elapsed time must be finite and nonnegative");
    const [low, high] = instructionWords(retiredInstructions);
    const [elapsedLow, elapsedHigh] = splitU64(BigInt(Math.floor(elapsedMilliseconds * 10_000)));
    this.#check(this.#exports.vine_bridge_synchronize_forward_clock(low, high, elapsedLow, elapsedHigh));
  }

  readTimestampCounter(retiredInstructions: number, elapsedMilliseconds: number): { low: number; high: number } {
    invariant(Number.isFinite(elapsedMilliseconds) && elapsedMilliseconds >= 0,
      "Timestamp elapsed time must be finite and nonnegative");
    const [low, high] = instructionWords(retiredInstructions);
    const [elapsedLow, elapsedHigh] = splitU64(BigInt(Math.floor(elapsedMilliseconds * 10_000)));
    const result = this.#invoke<{ kind: string; low: number; high: number }>(() =>
      this.#exports.vine_bridge_read_timestamp_counter(low, high, elapsedLow, elapsedHigh));
    invariant(result.metadata.kind === "timestamp-counter" &&
      Number.isInteger(result.metadata.low) && result.metadata.low >= 0 && result.metadata.low <= 0xffff_ffff &&
      Number.isInteger(result.metadata.high) && result.metadata.high >= 0 && result.metadata.high <= 0xffff_ffff &&
      result.control.length === 0 && result.payload.length === 0,
    "Rust returned an invalid timestamp counter");
    return { low: result.metadata.low, high: result.metadata.high };
  }

  acknowledgeFast(generation: bigint): void {
    const [low, high] = splitU64(generation);
    this.#check(this.#exports.vine_bridge_acknowledge_fast(low, high));
  }

  reconcileWritebackPages(generation: bigint, pages: DirtyMirrorPage[]): bigint {
    const controlLength = cpuWritebackControlByteLength(pages.length);
    const payloadLength = pages.length * memoryPageSize;
    const sourceBuffer = this.#exports.memory.buffer;
    const sharedOffsets = this.#sharedWritebackOffsets;
    let sharedOffsetCount = 0;
    for (const page of pages) {
      sharedOffsets[sharedOffsetCount++] = page.bytes.buffer === sourceBuffer ? page.bytes.byteOffset : -1;
    }
    sharedOffsets.length = sharedOffsetCount;
    const input = this.#resizeInput(controlLength + payloadLength);
    // Resizing Wasm memory can detach CPU page views; rebuild only those views
    // which point into this same linear memory before copying their contents.
    if (sourceBuffer !== this.#exports.memory.buffer) {
      for (let index = 0; index < pages.length; index++) {
        const offset = sharedOffsets[index]!;
        if (offset >= 0) pages[index]!.bytes = this.#view(offset, memoryPageSize);
      }
    }
    encodeCpuWritebackControlInto(input, generation, pages);
    let offset = controlLength;
    for (const page of pages) {
      input.set(page.bytes, offset);
      offset += page.bytes.length;
    }
    this.#check(this.#exports.vine_bridge_reconcile_writeback(controlLength, payloadLength));
    return BigInt(this.#exports.vine_bridge_writeback_generation_low() >>> 0) |
      (BigInt(this.#exports.vine_bridge_writeback_generation_high() >>> 0) << 32n);
  }

  servicePageFault<T = Record<string, unknown>>(address: number, errorCode: number): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_service_page_fault(address, errorCode));
  }

  ingestKeyboardEvent(event: RuntimeKeyboardInputEvent): void {
    invariant(typeof event.sequence === "string" && /^[1-9][0-9]*$/u.test(event.sequence),
      "Invalid keyboard event sequence");
    const sequence = BigInt(event.sequence);
    invariant(sequence <= BigInt(Number.MAX_SAFE_INTEGER), "Keyboard event sequence exceeds the browser range");
    const [low, high] = splitU64(sequence);
    const kind = event.kind === "focus-gained" ? 1 : event.kind === "focus-lost" ? 2 :
      event.kind === "key-down" ? 3 : event.kind === "key-up" ? 4 : 0;
    invariant(kind !== 0 &&
      (event.virtualKey === null || (Number.isInteger(event.virtualKey) && event.virtualKey >= 1 && event.virtualKey <= 0xff)) &&
      typeof event.repeat === "boolean", "Invalid keyboard event");
    const output = this.#invoke<{ kind: string; sequence: number }>(() =>
      this.#exports.vine_bridge_ingest_keyboard_event(
        low, high, kind, event.virtualKey ?? 0xffff_ffff, Number(event.repeat),
      ));
    invariant(output.metadata.kind === "keyboard-event-ingested" &&
      output.metadata.sequence === Number(sequence), "Keyboard event receipt mismatch");
  }

  ingestPointerEvent(event: RuntimePointerInputEvent): void {
    invariant(typeof event.sequence === "string" && /^[1-9][0-9]*$/u.test(event.sequence),
      "Invalid pointer event sequence");
    const sequence = BigInt(event.sequence);
    invariant(sequence <= BigInt(Number.MAX_SAFE_INTEGER), "Pointer event sequence exceeds the browser range");
    const kind = event.kind === "move" ? 1 : event.kind === "down" ? 2 : event.kind === "up" ? 3 : 0;
    const button = event.button === "none" ? 0 : event.button === "primary" ? 1 :
      event.button === "secondary" ? 2 : -1;
    invariant(kind !== 0 && (kind === 1 ? button === 0 : button === 1 || button === 2) &&
      Number.isInteger(event.x) && event.x >= 0 && event.x <= 65535 &&
      Number.isInteger(event.y) && event.y >= 0 && event.y <= 65535,
      "Invalid pointer event");
    const [low, high] = splitU64(sequence);
    const output = this.#invoke<{ kind: string; sequence: number }>(() =>
      this.#exports.vine_bridge_ingest_pointer_event(low, high, kind, button, event.x, event.y));
    invariant((output.metadata.kind === "pointer-event-ingested" ||
      output.metadata.kind === "pointer-event-ignored") &&
      output.metadata.sequence === Number(sequence), "Pointer event receipt mismatch");
  }

  dispatch<T = Record<string, unknown>>(
    endpoint: number, stackPointer: number, returnAddress: number,
    retiredInstructions: number, context: ExecutionCpuContext,
  ): BridgeOutput<T> {
    const [low, high] = instructionWords(retiredInstructions);
    return this.#invoke(() => this.#exports.vine_bridge_dispatch(
      endpoint, stackPointer, returnAddress, low, high,
      context.eax, context.ecx, context.edx, context.ebx, context.esp, context.ebp,
      context.esi, context.edi, context.instructionPointer, context.eflags,
      context.es, context.cs, context.ss, context.ds, context.fs, context.gs,
    ), true);
  }

  readPendingDispatch<T = Record<string, unknown>>(): BridgeOutput<T> {
    invariant(typeof this.#exports.vine_shared_execute_budget === "function", "unified dispatch is unavailable");
    return this.#invoke<T>(() => 0, true);
  }

  completeHost<T = Record<string, unknown>>(requestId: bigint, result: number): BridgeOutput<T> {
    const [low, high] = splitU64(requestId);
    return this.#invoke(() => this.#exports.vine_bridge_complete_host(low, high, result));
  }

  completeComAudio<T = Record<string, unknown>>(requestId: bigint, accepted: boolean): BridgeOutput<T> {
    const [low, high] = splitU64(requestId);
    return this.#invoke(() => this.#exports.vine_bridge_complete_com_audio(low, high, Number(accepted)));
  }

  readComMedia<T = Record<string, unknown>>(requestId: bigint, offset: bigint, length: number): BridgeOutput<T> {
    invariant(Number.isInteger(length) && length > 0 && length <= 2 * 1024 * 1024,
      "Encoded media read exceeds its bounded range");
    const [low, high] = splitU64(requestId);
    const [offsetLow, offsetHigh] = splitU64(offset);
    return this.#invoke(() => this.#exports.vine_bridge_read_com_media(low, high, offsetLow, offsetHigh, length));
  }

  completeComMedia<T = Record<string, unknown>>(
    requestId: bigint, metadata: Record<string, unknown>, payload: Uint8Array = emptyBytes,
  ): BridgeOutput<T> {
    const json = new TextEncoder().encode(JSON.stringify(metadata));
    invariant(json.length > 0 && json.length <= 4096, "Media completion metadata exceeds its bound");
    this.#setInput([json, payload]);
    const [low, high] = splitU64(requestId);
    return this.#invoke(() => this.#exports.vine_bridge_complete_com_media(low, high, json.length, payload.length));
  }

  observeAudioEnd(resourceId: bigint): boolean {
    const [low, high] = splitU64(resourceId);
    const result = this.#invoke<{ kind: string; ended: boolean }>(() => this.#exports.vine_bridge_observe_audio_end(low, high));
    invariant(result.metadata.kind === "audio-end-observed" && typeof result.metadata.ended === "boolean", "Invalid audio end observation");
    return result.metadata.ended;
  }

  completeOpfsWrite<T = Record<string, unknown>>(requestId: bigint, observation: number): BridgeOutput<T> {
    const [low, high] = splitU64(requestId);
    return this.#invoke(() => this.#exports.vine_bridge_complete_opfs_write(low, high, observation));
  }

  completeOpfsOpen<T = Record<string, unknown>>(
    requestId: bigint, observation: number, byteLength: number,
  ): BridgeOutput<T> {
    invariant(Number.isSafeInteger(byteLength) && byteLength >= 0, "Invalid OPFS file length");
    const [low, high] = splitU64(requestId);
    const [sizeLow, sizeHigh] = splitU64(BigInt(byteLength));
    return this.#invoke(() => this.#exports.vine_bridge_complete_opfs_open(low, high, observation, sizeLow, sizeHigh));
  }

  completeSourceRead<T = Record<string, unknown>>(
    requestId: bigint, control: Uint8Array, payload: Uint8Array,
  ): BridgeOutput<T> {
    this.#setInput([control, payload]);
    const [low, high] = splitU64(requestId);
    return this.#invoke(() => this.#exports.vine_bridge_complete_source_read(low, high, control.length, payload.length));
  }

  completeGdiTextRaster<T = Record<string, unknown>>(requestId: bigint, pixels: Uint8Array): BridgeOutput<T> {
    this.#setInput([pixels]);
    const [low, high] = splitU64(requestId);
    return this.#invoke(() => this.#exports.vine_bridge_complete_gdi_text_raster(low, high, pixels.length));
  }

  scheduleNextThread<T = Record<string, unknown>>(): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_schedule_next_thread());
  }

  finishThread<T = Record<string, unknown>>(exitCode: number): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_finish_thread(exitCode));
  }

  // Rust owns parked CPU bytes and their thread lifetime. The browser only
  // transports an opaque backend snapshot and the revision of that handoff.
  storeCpuSnapshot(threadId: number, revision: bigint, snapshot: ExecutionBackendSnapshot): bigint {
    invariant(Number.isInteger(threadId) && threadId > 0 && threadId <= 0xffff_ffff,
      "Invalid CPU snapshot thread");
    invariant(revision < 0xffff_ffff_ffff_ffffn, "CPU snapshot revision exhausted");
    const [low, high] = splitU64(revision);
    const backendTag = snapshot.backend === "v86" ? 1 : snapshot.backend === "recompiler" ? 2 : 0;
    invariant(snapshot.words instanceof Uint32Array &&
      this.#validSnapshotFormat(backendTag, snapshot.abiVersion, snapshot.words.byteLength),
    "CPU snapshot transport ABI mismatch");
    this.#setInput([new Uint8Array(snapshot.words.buffer, snapshot.words.byteOffset, snapshot.words.byteLength)]);
    this.#check(this.#exports.vine_bridge_cpu_snapshot_put(threadId, low, high,
      backendTag, snapshot.abiVersion, snapshot.words.byteLength));
    const receipt = this.#cpuSnapshotReceipt("cpu-snapshot-stored", threadId);
    const next = BigInt(receipt.revisionLow) | BigInt(receipt.revisionHigh) << 32n;
    invariant(next === revision + 1n && receipt.backendTag === backendTag &&
      receipt.abiVersion === snapshot.abiVersion && receipt.byteLength === snapshot.words.byteLength &&
      this.#exports.vine_bridge_payload_len() === 0, "CPU snapshot store receipt mismatch");
    return next;
  }

  loadCpuSnapshot(threadId: number): { snapshot: ExecutionBackendSnapshot; revision: bigint } {
    invariant(Number.isInteger(threadId) && threadId > 0 && threadId <= 0xffff_ffff,
      "Invalid CPU snapshot thread");
    this.#check(this.#exports.vine_bridge_cpu_snapshot_get(threadId));
    const receipt = this.#cpuSnapshotReceipt("cpu-snapshot", threadId);
    invariant(this.#exports.vine_bridge_payload_len() === receipt.byteLength,
      "CPU snapshot payload length mismatch");
    const bytes = this.#copyOutput(this.#exports.vine_bridge_payload_ptr(), receipt.byteLength);
    const words = new Uint32Array(bytes.buffer, bytes.byteOffset, bytes.byteLength / 4);
    const snapshot: ExecutionBackendSnapshot = receipt.backendTag === 1 ?
      { backend: "v86", abiVersion: 1, words } : { backend: "recompiler", abiVersion: 2, words };
    return { snapshot, revision: BigInt(receipt.revisionLow) | BigInt(receipt.revisionHigh) << 32n };
  }

  #validSnapshotFormat(backend: number, version: number, length: number): boolean {
    return backend === 1 && version === 1 && length === 612 ||
      backend === 2 && version === 2 && length === 512;
  }

  #cpuSnapshotReceipt(kind: CpuSnapshotReceipt["kind"], threadId: number): CpuSnapshotReceipt {
    const receipt = this.#readJson<CpuSnapshotReceipt>();
    const word = (value: number) => Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff;
    invariant(receipt !== null && receipt.kind === kind && receipt.threadId === threadId &&
      word(receipt.revisionLow) && word(receipt.revisionHigh) &&
      (receipt.revisionLow !== 0 || receipt.revisionHigh !== 0) &&
      this.#validSnapshotFormat(receipt.backendTag, receipt.abiVersion, receipt.byteLength) &&
      this.#exports.vine_bridge_control_len() === 0 && this.#exports.vine_bridge_presentation_len() === 0 &&
      this.#exports.vine_bridge_audio_len() === 0 && this.#exports.vine_bridge_media_len() === 0,
    "Invalid CPU snapshot receipt");
    return receipt;
  }

  completeCallback<T = Record<string, unknown>>(sentinel: number, result: number): BridgeOutput<T> {
    return this.#invoke(() => this.#exports.vine_bridge_complete_callback(sentinel, result));
  }

  stopContext(instructionPointer: number): RuntimeStopContext {
    const word = (value: unknown): value is number =>
      typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff;
    invariant(word(instructionPointer), "Stop instruction pointer is outside PE32");
    invariant(this.#exports.vine_bridge_stop_context(instructionPointer) === 0,
      "Runtime stop diagnostics are unavailable");
    const length = this.#exports.vine_bridge_stop_context_len();
    invariant(Number.isInteger(length) && length > 0 && length <= 64 * 1024,
      "Runtime stop diagnostics exceed the bounded buffer");
    const context = JSON.parse(this.#decoder.decode(
      this.#view(this.#exports.vine_bridge_stop_context_ptr(), length),
    )) as RuntimeStopContext;
    invariant(context !== null && context.kind === "stop-context" &&
      context.instructionPointer === instructionPointer &&
      (context.threadId === null || word(context.threadId)) &&
      (context.lastError === null || word(context.lastError)) &&
      (context.module === null || typeof context.module === "string") &&
      (context.symbol === null || typeof context.symbol === "string"),
    "Runtime stop diagnostics are invalid");
    return context;
  }

  #setInput(parts: Uint8Array[]): void {
    const input = this.#resizeInput(parts.reduce((sum, part) => sum + part.length, 0));
    let offset = 0;
    for (const part of parts) {
      input.set(part, offset);
      offset += part.length;
    }
  }

  #resizeInput(length: number): Uint8Array {
    invariant(Number.isSafeInteger(length) && length >= 0 && length <= inputLimit,
      "Runtime bridge input exceeds its bounded buffer");
    this.#check(this.#exports.vine_bridge_input_resize(length));
    return this.#view(this.#exports.vine_bridge_input_ptr(), length);
  }

  #view(pointer: number, length: number): Uint8Array {
    invariant(Number.isInteger(pointer) && pointer >= -0x8000_0000 && pointer <= 0xffff_ffff,
      "Runtime bridge buffer is outside Wasm memory");
    pointer >>>= 0;
    const memory = this.#exports.memory.buffer;
    invariant(Number.isSafeInteger(pointer) && pointer >= 0 &&
      Number.isSafeInteger(length) && length >= 0 &&
      pointer <= memory.byteLength && length <= memory.byteLength - pointer,
      "Runtime bridge buffer is outside Wasm memory");
    return new Uint8Array(memory, pointer, length);
  }

  #invoke<T>(operation: () => number, apiDispatch = false): BridgeOutput<T> {
    this.#check(operation());
    const completion = apiDispatch ? this.#exports.vine_bridge_api_completion_ptr() : 0;
    const metadata = completion === 0 ? this.#readJson<T>() : this.#readApiCompletion<T>(completion);
    const receipt = metadata as { kind?: unknown; presentation?: unknown; createdThread?: unknown } | null;
    const borrowPresentation = apiDispatch && completion === 0 && receipt?.kind === "com-complete" &&
      receipt.presentation !== undefined && receipt.createdThread === undefined;
    // The Worker consumes plain deltas and initial COM presentation synchronously
    // before ACK or another core call. Deferred and created-thread outputs stay owned.
    return {
      metadata,
      control: this.#outputBytes(this.#exports.vine_bridge_control_ptr(), this.#exports.vine_bridge_control_len(), completion !== 0),
      payload: this.#outputBytes(this.#exports.vine_bridge_payload_ptr(), this.#exports.vine_bridge_payload_len(), completion !== 0),
      presentation: this.#copyPresentation(borrowPresentation),
      audio: this.#copyAudio(),
      mediaBytes: this.#copyMedia(),
    };
  }

  #readApiCompletion<T>(pointer: number): T {
    const bytes = this.#view(pointer, 16);
    const words = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const hasEax = words.getUint32(12, true);
    invariant(hasEax <= 1, "Invalid API completion register flag");
    return { kind: "api-complete", completion: {
      instructionPointer: words.getUint32(0, true),
      stackPointer: words.getUint32(4, true),
      eax: hasEax === 1 ? words.getUint32(8, true) : null,
    } } as T;
  }

  #check(status: number): void {
    if (status === 0) return;
    const error = this.#readJson<{ message?: unknown }>();
    throw new Error(typeof error.message === "string" ? error.message : "Runtime bridge failed");
  }

  #readJson<T>(): T {
    const length = this.#exports.vine_bridge_metadata_len();
    invariant(length > 0, "Runtime bridge returned empty metadata");
    return JSON.parse(this.#decoder.decode(this.#view(this.#exports.vine_bridge_metadata_ptr(), length))) as T;
  }

  #copyPresentation(borrowed = false): Uint8Array {
    const length = this.#exports.vine_bridge_presentation_len();
    invariant(Number.isInteger(length) && length >= 0 && length <= inputLimit,
      "Presentation exceeds the bounded buffer");
    return this.#outputBytes(this.#exports.vine_bridge_presentation_ptr(), length, borrowed);
  }

  #copyAudio(): Uint8Array {
    const length = this.#exports.vine_bridge_audio_len();
    invariant(Number.isInteger(length) && length >= 0 && length <= inputLimit + 48,
      "Audio command exceeds the bounded PCM buffer");
    return this.#copyOutput(this.#exports.vine_bridge_audio_ptr(), length);
  }

  #copyMedia(): Uint8Array {
    const length = this.#exports.vine_bridge_media_len();
    invariant(Number.isInteger(length) && length >= 0 && length <= 2 * 1024 * 1024,
      "Encoded media exceeds its bounded buffer");
    return this.#copyOutput(this.#exports.vine_bridge_media_ptr(), length);
  }

  #copyOutput(pointer: number, length: number): Uint8Array {
    return length === 0 ? emptyBytes : Uint8Array.from(this.#view(pointer, length));
  }

  #outputBytes(pointer: number, length: number, borrowed: boolean): Uint8Array {
    return borrowed ? (length === 0 ? emptyBytes : this.#view(pointer, length)) : this.#copyOutput(pointer, length);
  }
}
