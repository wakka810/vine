export type RegisterUpdate =
  | { action: "preserve" }
  | { action: "set"; value: number };

export interface ApiCompletion {
  implementationId: string;
  instructionPointer: number;
  stackPointer: number;
  eax: RegisterUpdate;
  edx: RegisterUpdate;
  eflags: RegisterUpdate;
  lastError: number;
}

export interface PreparedMetadata {
  kind: "prepared";
  schemaVersion: 1;
  entryPoint: number;
  generation: number;
  stackTop: number;
  initialStackPointer: number;
  callbackSentinel: number;
  entrySentinel: number;
  pageCount: number;
  endpoints: Array<{ id: number; address: number }>;
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
  vine_bridge_acknowledge(low: number, high: number): number;
  vine_bridge_reconcile_writeback(controlLength: number, payloadLength: number): number;
  vine_bridge_dispatch(endpoint: number, stackPointer: number, returnAddress: number): number;
  vine_bridge_complete_host(low: number, high: number, result: number): number;
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

export class RuntimeBridge {
  readonly #exports: RuntimeBridgeExports;

  private constructor(exports: RuntimeBridgeExports) {
    this.#exports = exports;
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

  acknowledge<T = Record<string, unknown>>(generation: bigint): BridgeOutput<T> {
    const [low, high] = splitU64(generation);
    return this.#invoke(() => this.#exports.vine_bridge_acknowledge(low, high));
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

  dispatch<T = Record<string, unknown>>(
    endpoint: number,
    stackPointer: number,
    returnAddress: number,
  ): BridgeOutput<T> {
    return this.#invoke(() =>
      this.#exports.vine_bridge_dispatch(endpoint, stackPointer, returnAddress),
    );
  }

  completeHost<T = Record<string, unknown>>(requestId: bigint, result: number): BridgeOutput<T> {
    const [low, high] = splitU64(requestId);
    return this.#invoke(() => this.#exports.vine_bridge_complete_host(low, high, result));
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

  #setInput(parts: Uint8Array[]): void {
    const length = parts.reduce((total, part) => total + part.byteLength, 0);
    const status = this.#exports.vine_bridge_input_resize(length);
    if (status !== 0) {
      throw new Error(this.#readMetadataError());
    }
    const pointer = this.#exports.vine_bridge_input_ptr();
    const input = new Uint8Array(this.#exports.memory.buffer, pointer, length);
    let offset = 0;
    for (const part of parts) {
      input.set(part, offset);
      offset += part.byteLength;
    }
  }

  #invoke<T>(operation: () => number): BridgeOutput<T> {
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

  #readMetadataError(): string {
    const metadata = this.#readJson<{ message?: unknown }>();
    return typeof metadata.message === "string" ? metadata.message : "runtime bridge input failed";
  }

  #readJson<T>(): T {
    const bytes = this.#copyOutput(
      this.#exports.vine_bridge_metadata_ptr(),
      this.#exports.vine_bridge_metadata_len(),
    );
    invariant(bytes.byteLength > 0, "runtime bridge returned empty metadata");
    return JSON.parse(new TextDecoder().decode(bytes)) as T;
  }

  #copyOutput(pointer: number, length: number): Uint8Array {
    return Uint8Array.from(new Uint8Array(this.#exports.memory.buffer, pointer, length));
  }
}
