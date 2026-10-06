export class BrowserAudio {
  readonly #context: AudioContext;
  readonly #node: AudioWorkletNode;
  #pending: { requestId: string; resolve: (accepted: boolean) => void } | null = null;
  #closed = false;
  readonly #pendingMedia = new Map<string, (accepted: boolean) => void>();
  readonly #failed: (error: Error) => void;

  private constructor(context: AudioContext, node: AudioWorkletNode, ended: (id: string) => void, failed: (error: Error) => void) {
    this.#failed = failed;
    this.#context = context;
    this.#node = node;
    node.port.onmessage = ({ data }: MessageEvent<unknown>) => {
      if (typeof data !== "object" || data === null) return;
      const message = data as { type?: unknown; requestId?: unknown; accepted?: unknown; resourceId?: unknown };
      if (message.type === "ack" && this.#pending !== null &&
          message.requestId === this.#pending.requestId && typeof message.accepted === "boolean") {
        const pending = this.#pending;
        this.#pending = null;
        pending.resolve(message.accepted);
      } else if (message.type === "media-ack" && typeof message.requestId === "string" &&
                 typeof message.accepted === "boolean") {
        const resolve = this.#pendingMedia.get(message.requestId);
        if (resolve !== undefined) {
          this.#pendingMedia.delete(message.requestId);
          resolve(message.accepted);
        }
      } else if (message.type === "ended" && typeof message.resourceId === "string" &&
                 /^[1-9][0-9]*$/u.test(message.resourceId)) ended(message.resourceId);
    };
    node.onprocessorerror = () => this.#fail(new Error("Chrome audio processor failed"));
    context.onstatechange = () => {
      if (!this.#closed && context.state !== "running") this.#fail(new Error("Chrome audio output stopped unexpectedly"));
    };
  }

  static async create(ended: (id: string) => void, failed: (error: Error) => void): Promise<BrowserAudio> {
    // Construct and resume within the launch click's user activation.
    const context = new AudioContext({ latencyHint: "interactive" });
    try {
      await context.resume();
      await context.audioWorklet.addModule(new URL("./pcm-worklet.js", import.meta.url));
      if (context.state !== "running") throw new Error("Chrome audio output is unavailable");
      const node = new AudioWorkletNode(context, "vine-pcm", { numberOfInputs: 0,
        numberOfOutputs: 1, outputChannelCount: [2] });
      node.connect(context.destination);
      return new BrowserAudio(context, node, ended, failed);
    } catch (error) {
      await context.close();
      throw error;
    }
  }

  submit(requestId: string, bytes: ArrayBuffer): Promise<boolean> {
    if (this.#closed || this.#context.state !== "running") return Promise.resolve(false);
    if (this.#pending !== null) throw new Error("Chrome audio already has a pending operation");
    if (!/^[1-9][0-9]*$/u.test(requestId) || bytes.byteLength < 48 ||
        bytes.byteLength > 16 * 1024 * 1024 + 48) throw new Error("Invalid bounded audio command");
    return new Promise(resolve => {
      this.#pending = { requestId, resolve };
      try { this.#node.port.postMessage({ type: "command", requestId, bytes }, [bytes]); }
      catch { this.#pending = null; resolve(false); }
    });
  }

  retire(resourceId: string): void {
    if (!this.#closed) this.#node.port.postMessage({ type: "retire", resourceId });
  }

  submitMedia(requestId: string, resourceId: string, operation: string, volume: number,
    bytes?: ArrayBuffer): Promise<boolean> {
    if (this.#closed || this.#context.state !== "running") return Promise.resolve(false);
    if (!/^[1-9][0-9]*$/u.test(requestId) || !/^[1-9][0-9]*$/u.test(resourceId) ||
        !["create", "sample", "run", "pause", "stop", "volume", "release", "drain"].includes(operation) ||
        !Number.isInteger(volume) || volume < -10000 || volume > 0 ||
        (bytes !== undefined && (bytes.byteLength === 0 || bytes.byteLength > 512 * 1024 || bytes.byteLength % 8 !== 0))) {
      throw new Error("Invalid bounded decoded audio command");
    }
    if (this.#pendingMedia.has(requestId) || this.#pendingMedia.size >= 32) {
      throw new Error("Chrome media audio already has this pending operation");
    }
    return new Promise(resolve => {
      this.#pendingMedia.set(requestId, resolve);
      try {
        this.#node.port.postMessage({ type: "media-command", requestId, resourceId, operation, volume, bytes },
          bytes === undefined ? [] : [bytes]);
      } catch {
        this.#pendingMedia.delete(requestId);
        resolve(false);
      }
    });
  }

  #fail(error: Error): void {
    if (this.#closed) return;
    this.close();
    // The worker records this fatal observation while it still awaits the false
    // command ACK. It completes the Rust rejection before running or stopping.
    this.#failed(error);
  }

  close(): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#node.disconnect();
    this.#node.port.close();
    this.#pending?.resolve(false);
    this.#pending = null;
    for (const resolve of this.#pendingMedia.values()) resolve(false);
    this.#pendingMedia.clear();
    void this.#context.close();
  }
}
