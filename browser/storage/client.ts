import {
  type BrowserNativeStorageConfiguration,
  type BrowserStorageInitializationResult,
  type BrowserStorageRequest,
  type BrowserStorageResponse,
  type WritableProfileSnapshot,
  browserStorageProtocolVersion,
} from "./protocol.js";

type SuccessfulResult = Extract<BrowserStorageResponse, { type: "success" }>["result"];

interface PendingRequest {
  resolve: (value: SuccessfulResult) => void;
  reject: (error: Error) => void;
  abort?: () => void;
}

interface PublicationWaiter {
  resolve: (generation: number) => void;
  reject: (error: Error) => void;
}

export class BrowserStorageError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "BrowserStorageError";
    this.code = code;
  }
}

export class BrowserStorageClient {
  readonly #worker: Worker;
  readonly #pending = new Map<string, PendingRequest>();
  readonly #publicationWaiters = new Map<string, PublicationWaiter>();
  #nextRequestId = 1;
  #sessionKey: string | null = null;
  #sessionEpoch: number | null = null;
  #closed = false;

  constructor(
    workerUrl = new URL("./browser-storage-worker.js", import.meta.url),
  ) {
    this.#worker = new Worker(workerUrl, { type: "module" });
    this.#worker.addEventListener("message", this.#handleMessage);
    this.#worker.addEventListener("error", this.#handleWorkerError);
    this.#worker.addEventListener("messageerror", this.#handleMessageError);
  }

  async initialize(
    configuration: BrowserNativeStorageConfiguration,
    sessionKey: string = crypto.randomUUID(),
  ): Promise<BrowserStorageInitializationResult> {
    invariant(this.#sessionKey === null, "browser storage is already initialized");
    invariant(!this.#closed, "browser storage client is closed");
    this.#sessionKey = sessionKey;
    try {
      const result = await this.#request({
        type: "initialize",
        protocolVersion: browserStorageProtocolVersion,
        requestId: this.#requestId(),
        sessionKey,
        configuration,
      });
      invariant(result.kind === "initialized", "browser storage initialization response changed");
      this.#sessionEpoch = result.sessionEpoch;
      return result;
    } catch (error) {
      this.#sessionKey = null;
      throw error;
    }
  }

  async sourceRead(
    sourceId: number,
    objectId: number,
    offset: number,
    requestedBytes: number,
    signal?: AbortSignal,
  ): Promise<Uint8Array> {
    const result = await this.#request(
      this.#sessionRequest({
        type: "source-read",
        sourceId,
        objectId,
        offset,
        requestedBytes,
      }),
      [],
      signal,
    );
    invariant(result.kind === "source-read", "browser source read response changed");
    return result.bytes;
  }

  async profileSnapshot(signal?: AbortSignal): Promise<WritableProfileSnapshot> {
    const result = await this.#request(
      this.#sessionRequest({ type: "profile-snapshot" }),
      [],
      signal,
    );
    invariant(
      result.kind === "profile-snapshot",
      "browser profile snapshot response changed",
    );
    return result.snapshot;
  }

  async profileRead(
    objectId: number,
    generation: number,
    offset: number,
    requestedBytes: number,
    signal?: AbortSignal,
  ): Promise<Uint8Array> {
    const result = await this.#request(
      this.#sessionRequest({
        type: "profile-read",
        objectId,
        generation,
        offset,
        requestedBytes,
      }),
      [],
      signal,
    );
    invariant(result.kind === "profile-read", "browser profile read response changed");
    return result.bytes;
  }

  async profileCommit(input: {
    requestKey: string;
    objectId: number;
    baseGeneration: number;
    contentSha256: string;
    bytes: Uint8Array;
    signal?: AbortSignal;
  }): Promise<{
    objectId: number;
    baseGeneration: number;
    committedGeneration: number;
    duplicate: boolean;
  }> {
    const bytes = ownedBytes(input.bytes);
    const result = await this.#request(
      this.#sessionRequest({
        type: "profile-commit",
        requestKey: input.requestKey,
        objectId: input.objectId,
        baseGeneration: input.baseGeneration,
        contentSha256: input.contentSha256,
        bytes,
      }),
      [bytes.buffer],
      input.signal,
    );
    invariant(result.kind === "profile-commit", "browser profile commit response changed");
    return result;
  }

  async createProfileDirectory(input: {
    requestKey: string;
    directoryId: number;
    baseGeneration: number;
    signal?: AbortSignal;
  }): Promise<{
    directoryId: number;
    baseGeneration: number;
    committedGeneration: number;
    duplicate: boolean;
  }> {
    const result = await this.#request(
      this.#sessionRequest({
        type: "profile-directory-create",
        requestKey: input.requestKey,
        directoryId: input.directoryId,
        baseGeneration: input.baseGeneration,
      }),
      [],
      input.signal,
    );
    invariant(
      result.kind === "profile-directory-create",
      "browser profile directory response changed",
    );
    return result;
  }

  async beginUncommittedMutationForTest(input: {
    objectId: number;
    baseGeneration: number;
    contentSha256: string;
    bytes: Uint8Array;
  }): Promise<void> {
    const bytes = ownedBytes(input.bytes);
    const result = await this.#request(
      this.#sessionRequest({
        type: "test-begin-uncommitted",
        objectId: input.objectId,
        baseGeneration: input.baseGeneration,
        contentSha256: input.contentSha256,
        bytes,
      }),
      [bytes.buffer],
    );
    invariant(
      result.kind === "test-uncommitted-started",
      "browser storage test mutation response changed",
    );
  }

  commitWithoutResponseForTest(input: {
    requestKey: string;
    objectId: number;
    baseGeneration: number;
    contentSha256: string;
    bytes: Uint8Array;
  }): Promise<number> {
    const requestId = this.#requestId();
    const bytes = ownedBytes(input.bytes);
    const request = this.#sessionRequest({
      type: "profile-commit",
      requestId,
      requestKey: input.requestKey,
      objectId: input.objectId,
      baseGeneration: input.baseGeneration,
      contentSha256: input.contentSha256,
      bytes,
      testDropResponse: true,
    });
    return new Promise<number>((resolve, reject) => {
      this.#publicationWaiters.set(requestId, { resolve, reject });
      try {
        this.#worker.postMessage(request, [bytes.buffer]);
      } catch (error) {
        this.#publicationWaiters.delete(requestId);
        reject(asError(error));
      }
    });
  }

  async close(): Promise<void> {
    if (this.#closed) {
      return;
    }
    if (this.#sessionKey !== null && this.#sessionEpoch !== null) {
      const result = await this.#request(this.#sessionRequest({ type: "close" }));
      invariant(result.kind === "closed", "browser storage close response changed");
    }
    this.#terminate(new BrowserStorageError("browser-storage-closed", "browser storage closed"));
  }

  dispose(): void {
    if (!this.#closed) {
      this.#terminate(
        new BrowserStorageError("browser-storage-disposed", "browser storage was disposed"),
      );
    }
  }

  terminateForTest(): void {
    this.#terminate(
      new BrowserStorageError(
        "browser-storage-test-terminated",
        "browser storage worker was terminated by the test",
      ),
    );
  }

  #sessionRequest<T extends Omit<
    Extract<BrowserStorageRequest, { sessionKey: string }>,
    "protocolVersion" | "requestId" | "sessionKey" | "sessionEpoch"
  > & { requestId?: string }>(
    body: T,
  ): Extract<BrowserStorageRequest, { sessionKey: string }> {
    invariant(this.#sessionKey !== null, "browser storage is not initialized");
    invariant(this.#sessionEpoch !== null, "browser storage epoch is unavailable");
    return {
      ...body,
      protocolVersion: browserStorageProtocolVersion,
      requestId: body.requestId ?? this.#requestId(),
      sessionKey: this.#sessionKey,
      sessionEpoch: this.#sessionEpoch,
    } as Extract<BrowserStorageRequest, { sessionKey: string }>;
  }

  #request(
    request: BrowserStorageRequest,
    transfer: Transferable[] = [],
    signal?: AbortSignal,
  ): Promise<SuccessfulResult> {
    invariant(!this.#closed, "browser storage client is closed");
    if (signal?.aborted === true) {
      return Promise.reject(signal.reason ?? new DOMException("Aborted", "AbortError"));
    }
    return new Promise<SuccessfulResult>((resolve, reject) => {
      const pending: PendingRequest = { resolve, reject };
      if (signal !== undefined) {
        const abort = (): void => {
          if (this.#pending.delete(request.requestId)) {
            reject(asError(signal.reason ?? new DOMException("Aborted", "AbortError")));
          }
        };
        signal.addEventListener("abort", abort, { once: true });
        pending.abort = () => signal.removeEventListener("abort", abort);
      }
      this.#pending.set(request.requestId, pending);
      try {
        this.#worker.postMessage(request, transfer);
      } catch (error) {
        this.#pending.delete(request.requestId);
        pending.abort?.();
        reject(asError(error));
      }
    });
  }

  readonly #handleMessage = (event: MessageEvent<BrowserStorageResponse>): void => {
    const response = event.data;
    if (response.protocolVersion !== browserStorageProtocolVersion) {
      this.#terminate(
        new BrowserStorageError(
          "browser-storage-protocol-version",
          "browser storage protocol version changed",
        ),
      );
      return;
    }
    if (response.type === "test-commit-published") {
      const waiter = this.#publicationWaiters.get(response.requestId);
      if (waiter !== undefined) {
        this.#publicationWaiters.delete(response.requestId);
        waiter.resolve(response.committedGeneration);
      }
      return;
    }
    const pending = this.#pending.get(response.requestId);
    if (pending !== undefined) {
      this.#pending.delete(response.requestId);
      pending.abort?.();
      if (response.type === "success") {
        pending.resolve(response.result);
      } else {
        pending.reject(new BrowserStorageError(response.code, response.message));
      }
      return;
    }
    const publication = this.#publicationWaiters.get(response.requestId);
    if (publication !== undefined && response.type === "error") {
      this.#publicationWaiters.delete(response.requestId);
      publication.reject(new BrowserStorageError(response.code, response.message));
    }
  };

  readonly #handleWorkerError = (event: ErrorEvent): void => {
    this.#terminate(
      new BrowserStorageError(
        "browser-storage-worker-crashed",
        event.message || "browser storage worker crashed",
      ),
    );
  };

  readonly #handleMessageError = (): void => {
    this.#terminate(
      new BrowserStorageError(
        "browser-storage-message-invalid",
        "browser storage worker returned an uncloneable message",
      ),
    );
  };

  #terminate(error: Error): void {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    this.#worker.removeEventListener("message", this.#handleMessage);
    this.#worker.removeEventListener("error", this.#handleWorkerError);
    this.#worker.removeEventListener("messageerror", this.#handleMessageError);
    this.#worker.terminate();
    for (const pending of this.#pending.values()) {
      pending.abort?.();
      pending.reject(error);
    }
    this.#pending.clear();
    for (const waiter of this.#publicationWaiters.values()) {
      waiter.reject(error);
    }
    this.#publicationWaiters.clear();
  }

  #requestId(): string {
    const requestId = String(this.#nextRequestId);
    this.#nextRequestId += 1;
    return requestId;
  }
}

function ownedBytes(bytes: Uint8Array): Uint8Array<ArrayBuffer> {
  const owned = new Uint8Array(bytes.byteLength);
  owned.set(bytes);
  return owned;
}

function asError(value: unknown): Error {
  return value instanceof Error ? value : new Error(String(value));
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}
