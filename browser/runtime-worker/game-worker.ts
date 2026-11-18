import { isDevSource, type SourceDirectory } from "../storage/dev-source-directory.js";
import { browserDelayMilliseconds } from "./browser-delay.js";
import { gameStorageFingerprint } from "../storage/source-directory.js";
import { instantiateExecutionEngine } from "./execution-engine.js";
import { rasterizeGdiText } from "./gdi-text-raster.js";
import { GameSurface, type GamePresentation } from "./game-surface.js";
import { decodeHostDelta, decodeReflection } from "./memory-transport.js";
import { RuntimeBridge } from "./runtime-bridge.js";
import { V86Mirror } from "./v86-mirror.js";
import type { BridgeOutput, RuntimeStopContext } from "./runtime-bridge.js";
import { MediaDecoder, UnsupportedMediaFormat, type DecodedMediaSample } from "./media-decoder.js";
import type { CpuResume, ExecutionEngine } from "./execution-engine-contract.js";
import type { MessageBoxPresentation, RuntimeKeyboardInputEvent, RuntimePointerInputEvent } from "./protocol.js";
import { browseSourceDirectory, inspectSourcePath, listSourceDirectory, openSourceDll, openSourceExecutable, readSourceRange, type VerifiedSourceFile } from "./source-explorer.js";
import { openWritableTree } from "./writable-tree.js";

type GameCompletion = CpuResume & {
  executionBoundary?: "yield-current-thread" | "block-current-thread" | "terminate-current-thread" | "terminate-process";
};
type BrowserHostCompletion =
  | { kind: "host-complete"; threadId: number; completion: GameCompletion }
  | { kind: "host-retry"; threadId: number; requestId: string; service: string; arguments: number[] };
type PendingBrowserCompletion = {
  requestId: bigint; threadId: number; service: string; acknowledgement: number;
  done: boolean; promise: Promise<void>; cancelTimer?: (() => void) | null;
};
type CreatedThread = {
  threadId: number; entryPoint: number; initialStackPointer: number; fsBase: number; returnSentinel: number;
};
type MediaRequest = {
  kind: "media-request"; requestId: string; threadId: number;
  operation: "open" | "next-frame" | "audio-push" | "run" | "pause" | "stop" | "volume" | "release";
  resourceId: string | null; volume: number; delayMilliseconds: number;
  audio?: { start100ns: number; end100ns: number; channels: number; samplesPerSecond: number; byteLength: number };
  source: { kind: "guest-reader"; byteLength: string } |
    { kind: "source-file"; sourceId: number; objectId: number; byteLength: string } |
    { kind: "writable-file"; relativePath: string } | null;
};
type GameDispatch =
  | { kind: "source-library"; relativePathA: number[] }
  | { kind: "source-lookup"; objectId: number; relativePathA: number[] }
  | { kind: "source-search"; relativeDirectoryA: number[] }
  | { kind: "audio-request"; requestId: string; threadId: number }
  | { kind: "com-complete"; threadId?: number; completion: CpuResume; presentation?: GamePresentation; createdThread?: CreatedThread }
  | { kind: "api-complete"; completion: GameCompletion; createdThread?: CreatedThread; threadSelection?: { threadId: number } }
  | { kind: "guest-callback"; threadId?: number; target: number; stackPointer: number; returnSentinel: number }
  | MediaRequest
  | { kind: "media-data"; threadId: number; requestId: string; offset: string; byteLength: number }
  | { kind: "host-request"; requestId: string; threadId: number; service: string; arguments: number[]; relativePath?: string; messageBox?: MessageBoxPresentation };


interface LaunchRequest {
  type: "launch-executable";
  root: SourceDirectory;
  segments: string[];
  name: string;
  sha256: string;
  storageFingerprint: string;
  canvas: OffscreenCanvas;
}

const worker = self as DedicatedWorkerGlobalScope;
const inputEvents: Array<{ type: "keyboard-input"; input: RuntimeKeyboardInputEvent } |
  { type: "pointer-input"; input: RuntimePointerInputEvent }> = [];
let inputOverflow = false;
let inputEventReady: (() => void) | null = null;
let launched = false;
let pendingAudio: { requestId: string; resolve: (accepted: boolean) => void } | null = null;
const pendingMediaAudio = new Map<string, (accepted: boolean) => void>();
const failedMediaAudio = new Set<string>();
const audioEnds: string[] = [];
let audioEventReady: (() => void) | null = null;
let audioBackendError: string | null = null;
let messageBoxError: string | null = null;
let pendingMessageBox: { requestId: string; resolve: (result: number) => void } | null = null;

type RecentEvent = {
  event: "dispatch" | "completion" | "callback-return" | "host-completion" | "thread-switch" | "cpu-stop" | "worker-error";
  threadId: number | null; instructionPointer: number | null; detail: string | null; eax: number | null;
};
type StopContext = {
  threadId: number | null; threadIdSource: "runtime" | "execution-adapter" | "unknown";
  instructionPointer: number | null; eax: number | null; lastError: number | null;
  module: string | null; symbol: string | null;
  stackPointer?: number | null; stackWords?: number[];
};
const recentEvents: RecentEvent[] = [];
let readStopContext: (() => StopContext) | null = null;
const diagnosticWord = (value: unknown): number | null =>
  typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff ? value : null;
const diagnosticText = (value: unknown): string | null =>
  typeof value === "string" ? value.slice(0, 96).replace(/[\u0000-\u001f\u007f]/gu, " ") : null;
function recordRecentEvent(event: RecentEvent["event"], threadId: number | null,
  instructionPointer: number | null, detail: string | null = null, eax: number | null = null): void {
  if (recentEvents.length === 16) recentEvents.shift();
  recentEvents.push({ event, threadId: diagnosticWord(threadId), instructionPointer: diagnosticWord(instructionPointer),
    detail: diagnosticText(detail), eax: diagnosticWord(eax) });
}
function reportGameError(error: unknown): void {
  let context: StopContext = { threadId: null, threadIdSource: "unknown", instructionPointer: null,
    eax: null, lastError: null, module: null, symbol: null };
  try { if (readStopContext !== null) context = readStopContext(); }
  catch { /* Diagnostics must never replace the original fault. */ }
  recordRecentEvent("worker-error", context.threadId, context.instructionPointer);
  const decimal = (value: number | null) => value === null ? "unknown" : String(value);
  const hex = (value: number | null) => value === null ? "unknown" : `0x${value.toString(16).padStart(8, "0")}`;
  const original = error instanceof Error ? error.message : String(error);
  const recent = recentEvents.map(event =>
    `  ${event.event}: tid=${decimal(event.threadId)}, eip=${hex(event.instructionPointer)}` +
    (event.detail === null ? "" : `, ${event.detail}`) +
    (event.eax === null ? "" : `, eax=${hex(event.eax)}`)).join("\n");
  const stack = context.stackPointer == null ? "" :
    `, ESP=${hex(context.stackPointer)}, stack=[${(context.stackWords ?? []).map(hex).join(",")}]`;
  worker.postMessage({ type: "game-error",
    detail: `${original}\nStopped: thread=${decimal(context.threadId)} (${context.threadIdSource}), EIP=${hex(context.instructionPointer)}, EAX=${hex(context.eax)}, module=${context.module ?? "unknown"}, symbol=${context.symbol ?? "unknown"}, LastError=${decimal(context.lastError)}${stack}\nRecent events:\n${recent}`,
  });
}

function submitAudio(requestId: string, bytes: Uint8Array): Promise<boolean> {
  if (pendingAudio !== null) throw new Error("Another audio operation is pending");
  if (!/^[1-9][0-9]*$/u.test(requestId) || bytes.length < 48 ||
      bytes.length > 16 * 1024 * 1024 + 48) throw new Error("Invalid Rust audio command");
  return new Promise(resolve => {
    pendingAudio = { requestId, resolve };
    const buffer = bytes.buffer as ArrayBuffer;
    worker.postMessage({ type: "audio-command", requestId, bytes: buffer }, [buffer]);
  });
}

function submitMediaAudio(requestId: string, resourceId: string, operation: string, volume: number,
  bytes?: Uint8Array): Promise<boolean> {
  if (pendingMediaAudio.has(requestId) || pendingMediaAudio.size >= 32) throw new Error("Excessive media audio handoffs");
  return new Promise(resolve => {
    pendingMediaAudio.set(requestId, resolve);
    const buffer = bytes === undefined ? undefined : Uint8Array.from(bytes).buffer;
    worker.postMessage({ type: "media-audio-command", requestId, resourceId, operation, volume, bytes: buffer },
      buffer === undefined ? [] : [buffer]);
  });
}

worker.addEventListener("message", (event: MessageEvent<LaunchRequest |
  { type: "keyboard-input"; input: RuntimeKeyboardInputEvent } |
  { type: "pointer-input"; input: RuntimePointerInputEvent } |
  { type: "audio-result"; requestId: string; accepted: boolean } |
  { type: "media-audio-result"; requestId: string; accepted: boolean } |
  { type: "message-box-result"; requestId: string; result: number } |
  { type: "message-box-failed"; detail: string } |
  { type: "audio-ended"; resourceId: string } |
  { type: "audio-backend-error"; detail: string }>) => {
  if (event.data.type === "message-box-failed") {
    messageBoxError = typeof event.data.detail === "string" ? event.data.detail : "Chrome message box failed";
    audioEventReady?.(); audioEventReady = null;
    return;
  }
  if (event.data.type === "message-box-result") {
    if (pendingMessageBox === null || event.data.requestId !== pendingMessageBox.requestId ||
        !Number.isInteger(event.data.result) || event.data.result < 1 || event.data.result > 11) {
      messageBoxError = "Message box result disagrees with its pending resource";
      audioEventReady?.(); audioEventReady = null;
      return;
    }
    const pending = pendingMessageBox; pendingMessageBox = null;
    pending.resolve(event.data.result);
    return;
  }
  if (event.data.type === "audio-backend-error") {
    audioBackendError = typeof event.data.detail === "string" ? event.data.detail : "Chrome audio failed";
    for (const id of pendingMediaAudio.keys()) failedMediaAudio.add(id);
    audioEventReady?.(); audioEventReady = null;
    return;
  }
  if (event.data.type === "audio-result") {
    if (pendingAudio !== null && event.data.requestId === pendingAudio.requestId &&
        typeof event.data.accepted === "boolean") {
      const pending = pendingAudio; pendingAudio = null; pending.resolve(event.data.accepted);
    }
    return;
  }
  if (event.data.type === "media-audio-result") {
    const resolve = pendingMediaAudio.get(event.data.requestId);
    if (resolve !== undefined && typeof event.data.accepted === "boolean") {
      pendingMediaAudio.delete(event.data.requestId);
      resolve(event.data.accepted);
    }
    return;
  }
  if (event.data.type === "audio-ended") {
    if (launched && /^[1-9][0-9]*$/u.test(event.data.resourceId)) {
      if (audioEnds.length >= 16384) inputOverflow = true;
      else audioEnds.push(event.data.resourceId);
      audioEventReady?.(); audioEventReady = null;
    }
    return;
  }
  if (event.data.type === "keyboard-input" || event.data.type === "pointer-input") {
    if (!launched) return;
    if (inputEvents.length >= 256) inputOverflow = true;
    else inputEvents.push(event.data);
    inputEventReady?.(); inputEventReady = null;
    return;
  }
  if (event.data.type !== "launch-executable") return;
  if (launched) {
    worker.postMessage({ type: "game-status", detail: "A game is already running" });
    return;
  }
  launched = true;
  void launch(event.data).catch(reportGameError);
});

async function launch(request: LaunchRequest): Promise<void> {
  if (!(request.canvas instanceof OffscreenCanvas)) throw new Error("No game surface was transferred");
  if (request.root?.kind !== "directory" || !Array.isArray(request.segments) ||
      !/^[0-9a-f]{64}$/u.test(request.sha256)) throw new Error("Invalid game selection");
  const surface = new GameSurface(request.canvas, { allowSoftware: isDevSource(request.root) });
  const file = await openSourceExecutable(request.root, request.segments, request.name);
  if (file.size > 16 * 1024 * 1024) throw new Error("Executable exceeds the bridge input limit");
  const executable = new Uint8Array(await file.arrayBuffer());
  const after = await openSourceExecutable(request.root, request.segments, request.name);
  if (executable.length !== file.size || after.size !== file.size ||
      after.lastModified !== file.lastModified ||
      after.sourceVersion !== file.sourceVersion) throw new Error("Executable changed while reading");
  const executionMode = new URL(import.meta.url).searchParams.get("executionMode") ?? "fast";
  if (executionMode !== "strict" && executionMode !== "fast") throw new Error("Unknown execution mode");
  let sharedRuntime;
  if (executionMode === "fast") {
    const response = await fetch(new URL("./runtime-cpu.wasm", import.meta.url));
    if (!response.ok) throw new Error(`Fast Runtime fetch failed: ${response.status}`);
    sharedRuntime = await V86Mirror.instantiateSharedRuntime(await response.arrayBuffer());
  }
  const bridge = sharedRuntime ? RuntimeBridge.fromExports(sharedRuntime.exports)
    : await RuntimeBridge.instantiate(new URL("./runtime-core.wasm", import.meta.url));
  const path = [...request.segments, request.name].join("/");
  const inspected = bridge.inspectExecutable(executable, path);
  if (inspected.sha256 !== request.sha256) throw new Error("Executable changed since inspection");
  const identity = await gameStorageFingerprint(request.root, request.segments, request.name,
    inspected.sha256);
  if (request.storageFingerprint !== identity) throw new Error("Executable storage identity changed");
  const writable = await openWritableTree(identity);
  const sourceFiles = new Map<number, VerifiedSourceFile>();
  sourceFiles.set(1, {
    kind: "file",
    pathSegments: [...request.segments, request.name],
    byteLength: file.size, lastModified: file.lastModified, sourceVersion: file.sourceVersion,
  });
  const pending = [...inspected.guestImports];
  const libraries = new Set<string>();
  while (pending.length !== 0) {
    const imported = pending.shift()!;
    if (!/^[\x20-\x7e]+\.dll$/iu.test(imported) || /[\\/:]/u.test(imported) ||
        imported === "." || imported === "..") throw new Error("Invalid imported DLL filename");
    const key = imported.toLowerCase();
    if (libraries.has(key)) continue;
    if (libraries.size >= 32) throw new Error("Too many imported source DLLs");
    let selected: { segments: string[]; name: string } | undefined;
    for (const segments of [request.segments, []]) {
      const matches = (await browseSourceDirectory(request.root, segments))
        .filter(entry => entry.kind === "file" && entry.name.toLowerCase() === key);
      if (matches.length > 1) throw new Error(`Ambiguous source DLL ${imported}`);
      if (matches.length === 1) {
        selected = { segments: [...segments], name: matches[0]!.name };
        break;
      }
    }
    if (!selected) throw new Error(`Imported source DLL ${imported} is absent`);
    const dll = await openSourceDll(request.root, selected.segments, selected.name);
    if (dll.size > 16 * 1024 * 1024) throw new Error(`Imported source DLL ${imported} exceeds the bridge input limit`);
    const bytes = new Uint8Array(await dll.arrayBuffer());
    const after = await openSourceDll(request.root, selected.segments, selected.name);
    if (bytes.length !== dll.size || after.size !== dll.size || after.lastModified !== dll.lastModified ||
        after.sourceVersion !== dll.sourceVersion) {
      throw new Error(`Imported source DLL ${imported} changed while reading`);
    }
    const relativePath = [...selected.segments, selected.name].join("/");
    const registered = bridge.registerSourceDll(bytes, relativePath);
    if (!Number.isInteger(registered.sourceId) || registered.sourceId < 2 ||
        registered.sourceId > 33 || !/^[0-9a-f]{64}$/u.test(registered.sha256)) {
      throw new Error(`Invalid registered source DLL ${imported}`);
    }
    sourceFiles.set(registered.sourceId, {
      kind: "file", pathSegments: [...selected.segments, selected.name],
      byteLength: dll.size, lastModified: dll.lastModified, sourceVersion: dll.sourceVersion,
    });
    libraries.add(key);
    pending.push(...registered.guestImports);
  }
  const prepared = bridge.prepareExecutable(executable, path);
  if (typeof bridge.registerSourceDirectoryListing === "function") {
    for (const segments of request.segments.length > 0 ? [[], request.segments] : [[]]) {
      try {
        const entries = (await browseSourceDirectory(request.root, segments)).map(entry => ({
          name: entry.name,
          kind: entry.kind === "directory" ? "directory" as const : "file" as const,
        }));
        bridge.registerSourceDirectoryListing({ pathSegments: [...segments], entries });
      } catch {
        // Bounded enumeration failures fall back to on-demand source path inspection.
      }
    }
  }
  if (typeof writable.listEntries === "function" && typeof bridge.registerOpfsSnapshot === "function") {
    const snapshot = await writable.listEntries();
    if (snapshot !== null) bridge.registerOpfsSnapshot(snapshot);
  }
  const readRuntimeStopContext = (instructionPointer: number | null, eax: number | null,
    executionThreadId: number | null = null): StopContext => {
    let native: RuntimeStopContext | null = null;
    // Zero requests thread/LastError only when no CPU address is available. It
    // is never presented as an actual EIP or used to name a module or symbol.
    try { native = bridge.stopContext(instructionPointer ?? 0); }
    catch { /* A bridge diagnostic failure does not hide the original fault. */ }
    const nativeThreadId = diagnosticWord(native?.threadId);
    const adapterThreadId = diagnosticWord(executionThreadId);
    return { threadId: nativeThreadId ?? adapterThreadId,
      threadIdSource: nativeThreadId !== null ? "runtime" : adapterThreadId !== null ? "execution-adapter" : "unknown",
      instructionPointer, eax, lastError: diagnosticWord(native?.lastError),
      module: instructionPointer === null ? null : diagnosticText(native?.module),
      symbol: instructionPointer === null ? null : diagnosticText(native?.symbol) };
  };
  readStopContext = () => readRuntimeStopContext(null, null);
  const engineOptions = {
    maximumPhysicalPages: prepared.metadata.maximumPhysicalPages,
  };
  const engine: ExecutionEngine = sharedRuntime ? V86Mirror.attachSharedRuntime(sharedRuntime, engineOptions)
    : await instantiateExecutionEngine(new URL(import.meta.url), engineOptions);
  const sharedMemory = sharedRuntime ? bridge.sharedGuestMemory() : undefined;
  const reflection = decodeReflection(prepared.control, prepared.payload, sharedMemory);
  engine.initialize(reflection, prepared.metadata);
  if (engine.prepareExecution) {
    worker.postMessage({ type: "game-status", detail: "Preparing executable for launch" });
    const preparation = await engine.prepareExecution(prepared.metadata.codeEntries);
    preparation.limited ||= prepared.metadata.codeEntriesLimited;
    console.info("Independent code preparation", preparation);
  }
  worker.postMessage({ type: "game-status", detail: surface.backend === "software"
    ? "Running guest instructions (development software rendering; unsupported imports stop explicitly)"
    : "Running guest instructions (unsupported imports stop explicitly)" });
  const entryContext = prepared.metadata.dllAttach ? engine.saveCpuContext() : null;
  if (prepared.metadata.dllAttach) {
    engine.enterGuestCallback(prepared.metadata.dllAttach.target,
      prepared.metadata.dllAttach.stackPointer);
  }
  bridge.acknowledgeFast(reflection.generation);
  let generation = reflection.generation;
  let currentThreadId = prepared.metadata.runtime.threadId;
  readStopContext = () => {
    let instructionPointer: number | null = null, eax: number | null = null;
    let stackPointer: number | null = null;
    const stackWords: number[] = [];
    try {
      const context = engine.saveCpuContext();
      instructionPointer = diagnosticWord(context.instructionPointer);
      eax = diagnosticWord(context.eax);
      stackPointer = diagnosticWord(context.esp);
      if (stackPointer !== null) for (let index = 0; index < 4; index++) {
        const address = stackPointer + index * 4;
        if (address > 0xffff_fffc) break;
        const word = diagnosticWord(engine.readGuestU32(address));
        if (word === null) break;
        stackWords.push(word);
      }
    } catch { /* An uninitialized or failed CPU may have no readable context. */ }
    return { ...readRuntimeStopContext(instructionPointer, eax, currentThreadId), stackPointer, stackWords };
  };
  let currentSnapshotRevision = bridge.storeCpuSnapshot(currentThreadId, 0n, engine.saveCpuContext().snapshot);
  const parkCurrentCpu = () => {
    currentSnapshotRevision = bridge.storeCpuSnapshot(currentThreadId, currentSnapshotRevision,
      engine.saveCpuContext().snapshot);
  };
  const pendingBrowserCompletions: PendingBrowserCompletion[] = [];
  const armBrowserDelay = (delay: PendingBrowserCompletion, milliseconds: number) => {
    delay.cancelTimer?.();
    delay.done = false;
    delay.promise = new Promise<void>(resolve => {
      const timer = setTimeout(() => {
        delay.cancelTimer = null;
        delay.done = true;
        resolve();
      }, milliseconds);
      delay.cancelTimer = () => {
        clearTimeout(timer);
        delay.cancelTimer = null;
        resolve();
      };
    });
  };
  const applyDelta = (output: { control: Uint8Array; payload: Uint8Array }, label: string,
    deferAcknowledgement = false): bigint | null => {
    if (output.control.length > 0) {
      const delta = decodeHostDelta(output.control, output.payload, sharedMemory);
      if (delta.fromGeneration !== generation) throw new Error(`Memory generation mismatch after ${label}`);
      engine.applyMemoryDelta(delta);
      if (!deferAcknowledgement) {
        bridge.acknowledgeFast(delta.generation);
        generation = delta.generation;
      }
      return delta.generation;
    } else if (output.payload.length !== 0) {
      throw new Error(`Unpaired memory payload after ${label}`);
    }
    return null;
  };
  const switchThread = (threadId: number, saveCurrent = true) => {
    if (threadId === currentThreadId) return;
    const next = bridge.loadCpuSnapshot(threadId);
    if (saveCurrent) parkCurrentCpu();
    engine.restoreCpuContext(next);
    currentThreadId = threadId;
    currentSnapshotRevision = next.revision;
    recordRecentEvent("thread-switch", currentThreadId, engine.saveCpuContext().instructionPointer);
  };
  const installCreatedThread = (created: CreatedThread | undefined) => {
    if (created === undefined) return;
    if (created.threadId === currentThreadId ||
        created.returnSentinel !== prepared.metadata.entrySentinel) {
      throw new Error("Created thread has an invalid CPU bootstrap");
    }
    bridge.storeCpuSnapshot(created.threadId, 0n, engine.createCpuContext(created).snapshot);
  };
  type MediaRead = { offset: number; length: number; sent: boolean; resolve: (bytes: Uint8Array) => void; reject: (error: Error) => void };
  type MediaTask = { request: MediaRequest; bytes: Uint8Array; read: MediaRead | null;
    result: { metadata: Record<string, unknown>; bytes?: Uint8Array } | null; promise: Promise<void>;
    cancelled: boolean; cancelDelay: (() => void) | null; consumed: Promise<void>; consume: () => void; fault: Error | null;
    sample: DecodedMediaSample | null };
  type CodecResource = { decoder: MediaDecoder | null; request: MediaRequest; audio: boolean; buffered: DecodedMediaSample | null };
  const mediaTasks = new Map<string, MediaTask>();
  const mediaResources = new Map<string, CodecResource>();
  let mediaClosed = false;
  const wakeMediaWork = () => { audioEventReady?.(); audioEventReady = null; };
  const readMedia = async (resource: CodecResource, offset: number, length: number): Promise<Uint8Array> => {
    if (mediaClosed) throw new Error("Decoder source resource was closed");
    const operation = resource.request;
    if (operation.source?.kind === "guest-reader") {
      const task = mediaTasks.get(operation.requestId);
      if (task === undefined || task.read !== null) {
        const fault = new Error("Decoder read lost its pending operation");
        if (task !== undefined) task.fault = fault;
        throw fault;
      }
      return new Promise((resolve, reject) => {
        task.read = { offset, length, sent: false, resolve, reject };
        wakeMediaWork();
      });
    }
    if (operation.source?.kind === "source-file") {
      const verified = sourceFiles.get(operation.source.objectId);
      if (verified === undefined || String(verified.byteLength) !== operation.source.byteLength) {
        throw new Error("Decoder source lost its verified browser capability");
      }
      try {
        return await readSourceRange(request.root, verified.pathSegments, verified.byteLength,
          verified.lastModified, offset, length, verified.sourceVersion);
      } catch (error) {
        // A missing browser file is a native observation. Broken capabilities,
        // changed immutable source bytes and invalid ranges are boundary faults.
        if (!(error instanceof DOMException)) {
          const task = mediaTasks.get(operation.requestId);
          if (task !== undefined) task.fault = error instanceof Error ? error : new Error(String(error));
        }
        throw error;
      }
    }
    if (operation.source?.kind === "writable-file") {
      const bytes = await writable.read(operation.source.relativePath, offset, length);
      if (bytes === null) throw new Error("Decoder writable resource was removed");
      return bytes;
    }
    throw new Error("Decoder operation has no Rust-selected source");
  };
  const performMedia = async (task: MediaTask): Promise<NonNullable<MediaTask["result"]>> => {
    const native = task.request;
    const id = native.resourceId!;
    if (native.delayMilliseconds > 0) await new Promise<void>(resolve => {
      const timer = setTimeout(() => { task.cancelDelay = null; resolve(); }, native.delayMilliseconds);
      task.cancelDelay = () => { clearTimeout(timer); task.cancelDelay = null; resolve(); };
    });
    if (task.cancelled) return { metadata: { kind: "stopped", resourceId: id } };
    if (native.operation === "open") {
      if (mediaResources.has(id) || mediaResources.size >= 32 || native.source === null) {
        throw new Error("Invalid or excessive codec resource creation");
      }
      const byteLength = native.source.kind === "writable-file"
        ? (await writable.stat(native.source.relativePath)) : null;
      if (native.source.kind === "writable-file" && byteLength?.kind !== "file") {
        throw new DOMException("Decoder writable file does not exist", "NotFoundError");
      }
      const length = native.source.kind === "writable-file"
        ? byteLength?.kind === "file" ? byteLength.byteLength : 0
        : Number(native.source.byteLength);
      const resource: CodecResource = { decoder: null, request: native, audio: false, buffered: null };
      try {
        resource.decoder = await MediaDecoder.open(length, (offset, count) => readMedia(resource, offset, count));
        if (mediaClosed) throw new Error("Decoder resource was closed during creation");
        const format = resource.decoder.format;
        resource.audio = format.audioChannels !== 0;
        if (resource.audio && !await submitMediaAudio(native.requestId, id, "create", native.volume)) {
          throw new Error("Chrome decoded audio creation failed");
        }
        mediaResources.set(id, resource);
        return { metadata: { kind: "opened", resourceId: id, ...format } };
      } catch (error) {
        await resource.decoder?.close();
        throw error;
      }
    }
    const resource = mediaResources.get(id);
    if (resource?.decoder === null || resource === undefined) throw new Error("Codec resource was retired");
    if (native.operation === "next-frame") {
      resource.request = native;
      const sample = resource.buffered ?? await resource.decoder.next();
      resource.buffered = null;
      task.sample = sample;
      if (task.cancelled) {
        resource.buffered = sample;
        return { metadata: { kind: "stopped", resourceId: id } };
      }
      if (sample === null) {
        if (resource.audio && !await submitMediaAudio(native.requestId, id, "drain", native.volume)) {
          throw new Error("Chrome decoded audio drain failed");
        }
        return { metadata: { kind: "ended", resourceId: id } };
      }
      const { bytes, ...metadata } = sample;
      return { metadata: { ...metadata, resourceId: id }, bytes };
    }
    if (native.operation === "audio-push") {
      if (!resource.audio || native.audio?.channels !== 2 || native.audio.samplesPerSecond !== 48000 ||
          native.audio.byteLength !== task.bytes.byteLength || task.bytes.byteLength === 0 ||
          task.bytes.byteLength > 512 * 1024 || task.bytes.byteLength % 8 !== 0) {
        throw new Error("Rust-approved media PCM has an invalid Chrome format");
      }
      if (!await submitMediaAudio(native.requestId, id, "sample", native.volume, task.bytes)) {
        throw new Error("Chrome decoded audio queue rejected its sample");
      }
      return { metadata: { kind: "acknowledged" } };
    }
    if (native.operation === "stop" || native.operation === "release") {
      const held = [...mediaTasks.values()].filter(other => other !== task && other.request.resourceId === id &&
        (other.request.operation === "next-frame" || other.request.operation === "audio-push"));
      for (const other of held) {
        other.cancelled = true;
        if (other.sample !== null) resource.buffered = other.sample;
        if (other.request.operation === "audio-push" && other.request.audio !== undefined) {
          const audio = other.request.audio;
          resource.buffered = { kind: "audio", start100ns: audio.start100ns, end100ns: audio.end100ns,
            channels: audio.channels, samplesPerSecond: audio.samplesPerSecond, bytes: other.bytes };
        }
        if (other.result !== null) other.result = { metadata: { kind: "stopped", resourceId: id } };
        other.cancelDelay?.();
        // Finish a real bounded read even when not yet dispatched. Rejecting
        // FFmpeg's device request would poison a codec that Stop/Run can reuse.
      }
      if (resource.audio && !await submitMediaAudio(native.requestId, id, "stop", native.volume)) {
        throw new Error("Chrome decoded audio stop failed");
      }
      await Promise.all(held.map(other => other.consumed));
      if (native.operation === "release") {
        if (resource.audio && !await submitMediaAudio(native.requestId, id, "release", native.volume)) {
          throw new Error("Chrome decoded audio release failed");
        }
        await resource.decoder.close();
        mediaResources.delete(id);
      }
      return { metadata: { kind: "acknowledged" } };
    }
    if (resource.audio && !await submitMediaAudio(native.requestId, id, native.operation, native.volume)) {
      throw new Error("Chrome decoded audio control failed");
    }
    return { metadata: { kind: "acknowledged" } };
  };
  const startMedia = (native: MediaRequest, bytes: Uint8Array) => {
    if (!/^[1-9][0-9]*$/u.test(native.requestId) || native.resourceId === null ||
        !/^[1-9][0-9]*$/u.test(native.resourceId) || native.threadId !== currentThreadId ||
        !Number.isInteger(native.delayMilliseconds) || native.delayMilliseconds < 0 || native.delayMilliseconds > 0xffff_ffff ||
        !Number.isInteger(native.volume) || native.volume < -10000 || native.volume > 0 ||
        !["open", "next-frame", "audio-push", "run", "pause", "stop", "volume", "release"].includes(native.operation) ||
        mediaTasks.has(native.requestId) || mediaTasks.size >= 32 ||
        (native.operation !== "audio-push" && bytes.length !== 0)) throw new Error("Invalid Rust media operation");
    const resource = mediaResources.get(native.resourceId);
    if (native.operation !== "open" && resource?.decoder == null) throw new Error("Rust selected a retired codec resource");
    if (native.operation === "open" && (native.source === null || mediaResources.has(native.resourceId))) {
      throw new Error("Rust media creation lost its selected source/resource identity");
    }
    if ((native.operation === "open" || native.operation === "next-frame") && native.source !== null) {
      if (native.source.kind !== "writable-file" && (!/^[1-9][0-9]*$/u.test(native.source.byteLength) ||
          !Number.isSafeInteger(Number(native.source.byteLength)))) throw new Error("Encoded source length exceeds the browser boundary");
      if (native.source.kind === "source-file") {
        const verified = sourceFiles.get(native.source.objectId);
        if (verified === undefined || String(verified.byteLength) !== native.source.byteLength) {
          throw new Error("Rust-selected decoder source lost its browser capability");
        }
      }
    }
    if (native.operation === "audio-push" && (!resource!.audio || native.audio?.channels !== 2 ||
        native.audio.samplesPerSecond !== 48000 || native.audio.byteLength !== bytes.length ||
        bytes.length === 0 || bytes.length > 512 * 1024 || bytes.length % 8 !== 0)) {
      throw new Error("Rust-approved media PCM has an invalid Chrome format");
    }
    let consume!: () => void;
    const consumed = new Promise<void>(resolve => { consume = resolve; });
    const task: MediaTask = { request: native, bytes, read: null, result: null, promise: Promise.resolve(),
      cancelled: false, cancelDelay: null, consumed, consume, fault: null, sample: null };
    mediaTasks.set(native.requestId, task);
    task.promise = performMedia(task).then(result => { task.result = result; }, error => {
      let message = (error instanceof Error ? error.message : String(error)).slice(0, 512) || "Chrome media failed";
      while (new TextEncoder().encode(message).length > 512) message = message.slice(0, -1);
      task.result = { metadata: { kind: error instanceof DOMException && error.name === "NotFoundError" ? "file-not-found" :
        error instanceof UnsupportedMediaFormat ? "unsupported" : "backend-failed",
        message } };
    }).then(() => {
      if (task.cancelled) task.result = { metadata: { kind: "stopped", resourceId: native.resourceId } };
      wakeMediaWork();
    });
    parkCurrentCpu();
  };
  // A native operation may suspend while FFmpeg requests encoded bytes. Only
  // Rust can turn that demand into an original guest IAsyncReader callback.
  const handleMediaOutput = (output: BridgeOutput<GameDispatch>, initialDispatch = false): boolean => {
    const native = output.metadata;
    recordRecentEvent("completion", "threadId" in native ? native.threadId ?? currentThreadId : currentThreadId,
      null, native.kind,
      native.kind === "com-complete" ? native.completion.eax : null);
    if (!((native.kind === "com-complete" || native.kind === "guest-callback") ||
        native.kind === "media-request" || native.kind === "media-data")) {
      throw new Error("Media returned an unsupported guest continuation");
    }
    // Initial COM presentation may borrow Runtime output. Parking/restoring a
    // different CPU now calls Runtime, ending that view's lifetime before Present.
    const presentation = initialDispatch && native.kind === "com-complete" &&
      native.presentation !== undefined && native.createdThread === undefined &&
      native.threadId !== undefined && native.threadId !== currentThreadId
      ? Uint8Array.from(output.presentation) : output.presentation;
    if (native.threadId !== undefined) switchThread(native.threadId, !cpuRetired);
    const deferAcknowledgement = initialDispatch && native.kind === "com-complete" &&
      native.presentation !== undefined && native.createdThread === undefined;
    const deltaGeneration = applyDelta(output, "media continuation", deferAcknowledgement);
    if (native.kind === "guest-callback") {
      if (native.returnSentinel !== prepared.metadata.callbackSentinel) throw new Error("Media callback used an unknown sentinel");
      engine.enterGuestCallback(native.target, native.stackPointer);
      cpuRetired = false;
      return true;
    }
    if (native.kind === "com-complete") {
      installCreatedThread(native.createdThread);
      if (native.presentation !== undefined) surface.present(native.presentation, presentation);
      if (deferAcknowledgement && deltaGeneration !== null) {
        bridge.acknowledgeFast(deltaGeneration);
        generation = deltaGeneration;
      }
      engine.applyCpuResume(native.completion);
      cpuRetired = false;
      return true;
    }
    if (native.kind === "media-request") startMedia(native, output.mediaBytes);
    else {
      const task = mediaTasks.get(native.requestId);
      const read = task?.read;
      if (read == null || !read.sent || task!.request.threadId !== native.threadId ||
          native.offset !== String(read.offset) || native.byteLength !== read.length || output.mediaBytes.length !== read.length) {
        throw new Error("Encoded media completion disagrees with its pending range");
      }
      task!.read = null;
      read.resolve(output.mediaBytes);
      parkCurrentCpu();
    }
    return false;
  };
  const pumpMediaWork = (): boolean | null => {
    const task = [...mediaTasks.values()].find(task => task.fault !== null || task.result !== null || (task.read !== null && !task.read.sent));
    if (task === undefined) return null;
    if (task.fault !== null) throw task.fault;
    const changed = engine.collectChangedPageViews();
    if (changed.length > 0) {
      generation = bridge.reconcileWritebackPages(generation, changed);
      engine.acknowledgeWriteback?.(generation);
    }
    let output: BridgeOutput<GameDispatch>;
    if (task.result !== null) {
      mediaTasks.delete(task.request.requestId);
      output = bridge.completeComMedia<GameDispatch>(BigInt(task.request.requestId), task.result.metadata, task.result.bytes);
      failedMediaAudio.delete(task.request.requestId);
      task.consume();
    } else {
      const read = task.read!;
      read.sent = true;
      output = bridge.readComMedia<GameDispatch>(BigInt(task.request.requestId), BigInt(read.offset), read.length);
    }
    return handleMediaOutput(output);
  };
  let cpuRetired = false;
  const observeAudioEnds = () => {
    if (audioEnds.length === 0) return false;
    const changed = engine.collectChangedPageViews();
    if (changed.length > 0) {
      generation = bridge.reconcileWritebackPages(generation, changed);
      engine.acknowledgeWriteback?.(generation);
    }
    bridge.synchronizeForwardClock(instructions, performance.now() - startedAt);
    let woke = false;
    while (audioEnds.length > 0) {
      const resourceId = audioEnds.shift()!;
      if (bridge.observeAudioEnd(BigInt(resourceId))) {
        worker.postMessage({ type: "audio-retire", resourceId });
        woke = true;
      }
    }
    return woke;
  };
  const completeReadyBrowserWork = () => {
    if (pendingBrowserCompletions.some(delay => delay.service === "browser.object-wait")) {
      const requestId = bridge.readyObjectWaitRequestId();
      if (requestId !== 0n) {
        const readyObject = pendingBrowserCompletions.find(delay =>
          delay.service === "browser.object-wait" && delay.requestId === requestId);
        if (readyObject === undefined) throw new Error("Ready object wait has no pending browser request");
        readyObject.acknowledgement = 0;
        readyObject.done = true;
        readyObject.cancelTimer?.();
      }
    }
    const ready = pendingBrowserCompletions.findIndex(delay => delay.done);
    if (ready === -1) return false;
    const delay = pendingBrowserCompletions[ready]!;
    const changed = engine.collectChangedPageViews();
    if (changed.length > 0) {
      generation = bridge.reconcileWritebackPages(generation, changed);
      engine.acknowledgeWriteback?.(generation);
    }
    recordRecentEvent("host-completion", delay.threadId, null, delay.service);
    if (delay.service === "browser.audio-delay" || delay.service === "browser.object-wait" || delay.service === "browser.message-wait") {
      bridge.synchronizeForwardClock(instructions, performance.now() - startedAt);
    }
    const completed = bridge.completeHost<BrowserHostCompletion>(delay.requestId, delay.acknowledgement);
    if (completed.metadata.threadId !== delay.threadId) {
      throw new Error("Browser completion disagrees with its guest thread");
    }
    if (completed.metadata.kind === "host-retry") {
      const retry = completed.metadata;
      if ((delay.service !== "browser.timer-delay" && delay.service !== "browser.audio-delay" &&
          delay.service !== "browser.object-wait" && delay.service !== "browser.message-wait") ||
          retry.service !== delay.service ||
          retry.requestId !== delay.requestId.toString() || !Array.isArray(retry.arguments) ||
          "completion" in retry ||
          completed.control.length !== 0 || completed.payload.length !== 0 ||
          completed.presentation.length !== 0 || completed.audio.length !== 0 ||
          completed.mediaBytes.length !== 0) {
        throw new Error("Browser timer retry disagrees with its pending request");
      }
      const milliseconds = browserDelayMilliseconds(retry.service, retry.arguments);
      if (delay.service === "browser.object-wait") delay.acknowledgement = 1;
      armBrowserDelay(delay, milliseconds);
      return true;
    }
    if (completed.metadata.kind !== "host-complete" ||
        completed.metadata.completion.executionBoundary !== undefined) {
      throw new Error("Browser completion disagrees with its guest thread");
    }
    pendingBrowserCompletions.splice(ready, 1);
    applyDelta(completed, "browser completion");
    if (delay.threadId === currentThreadId) {
      engine.applyCpuResume(completed.metadata.completion);
    } else {
      const parked = bridge.loadCpuSnapshot(delay.threadId);
      if (engine.resumeParkedCpuSnapshot) {
        bridge.storeCpuSnapshot(
          delay.threadId,
          parked.revision,
          engine.resumeParkedCpuSnapshot(parked.snapshot, completed.metadata.completion),
        );
      } else {
        const active = engine.saveCpuContext();
        try {
          engine.restoreCpuContext(parked);
          engine.applyCpuResume(completed.metadata.completion);
          bridge.storeCpuSnapshot(delay.threadId, parked.revision, engine.saveCpuContext().snapshot);
        } finally {
          engine.restoreCpuContext(active);
        }
      }
    }
    return true;
  };
  const ingestQueuedInput = () => {
    if (inputOverflow) throw new Error("Browser input event queue overflowed");
    while (inputEvents.length > 0) {
      const event = inputEvents.shift()!;
      if (event.type === "keyboard-input") bridge.ingestKeyboardEvent(event.input);
      else bridge.ingestPointerEvent(event.input);
    }
  };
  const schedule = async () => {
    while (true) {
      if (audioBackendError !== null && failedMediaAudio.size === 0) throw new Error(audioBackendError);
      if (messageBoxError !== null) throw new Error(messageBoxError);
      const media = pumpMediaWork();
      if (media === true) return;
      if (media === false) continue;
      if (completeReadyBrowserWork()) continue;
      observeAudioEnds();
      bridge.synchronizeForwardClock(instructions, performance.now() - startedAt);
      ingestQueuedInput();
      const result = bridge.scheduleNextThread<{
        kind: string; threadId?: number; audioEndPending?: boolean;
        messageWaitPending?: boolean; messageWaitDelayMs?: number | null;
      }>();
      if (result.metadata.kind === "scheduler-idle") {
        if (pendingBrowserCompletions.length === 0 && mediaTasks.size === 0 &&
            result.metadata.audioEndPending !== true && result.metadata.messageWaitPending !== true) {
          throw new Error("All guest threads are idle; no browser work can wake them");
        }
        // Rust owns message waits and window-timer deadlines. Chrome only waits
        // for input or that deadline, then lets Rust decide which thread wakes.
        const audioReady = new Promise<void>(resolve => { audioEventReady = resolve; });
        const inputReady = new Promise<void>(resolve => { inputEventReady = resolve; });
        let timer: ReturnType<typeof setTimeout> | undefined;
        const waits = [audioReady, inputReady, ...pendingBrowserCompletions.map(delay => delay.promise),
          ...[...mediaTasks.values()].map(task => task.promise)];
        const delay = result.metadata.messageWaitDelayMs;
        if (delay !== undefined && delay !== null) {
          if (!Number.isSafeInteger(delay) || delay < 0 || delay > 0x7fff_ffff) {
            throw new Error("Rust returned an invalid message wait deadline");
          }
          waits.push(new Promise<void>(resolve => { timer = setTimeout(resolve, delay); }));
        }
        try { await Promise.race(waits); }
        finally {
          if (timer !== undefined) clearTimeout(timer);
          audioEventReady = null;
          inputEventReady = null;
        }
        continue;
      }
      if (result.metadata.kind !== "thread-scheduled" || result.metadata.threadId === undefined) {
        throw new Error("All guest threads are idle; no browser work can wake them");
      }
      switchThread(result.metadata.threadId, !cpuRetired);
      cpuRetired = false;
      return;
    }
  };
  const retireThread = (exitCode: number) => {
    if (currentThreadId === prepared.metadata.runtime.threadId) {
      throw new Error("The initial guest thread cannot retire through a worker-thread exit");
    }
    bridge.synchronizeForwardClock(instructions, performance.now() - startedAt);
    applyDelta(bridge.finishThread(exitCode), "thread exit");
    // Rust retires the parked CPU snapshot with the owning thread.
    cpuRetired = true;
  };
  const browserTasks = new MessageChannel();
  const yieldBrowserTask = () => new Promise<void>(resolve => {
    // The loop awaits each task before posting another, so one callback is live.
    browserTasks.port1.onmessage = () => resolve();
    browserTasks.port2.postMessage(null);
  });
  let instructions = 0;
  const startedAt = performance.now();
  let browserYieldAt = startedAt;
  try {
    while (true) {
      if (audioBackendError !== null && failedMediaAudio.size === 0) throw new Error(audioBackendError);
      if (messageBoxError !== null) throw new Error(messageBoxError);
      // Give Chrome timer, audio and input tasks a turn at an elapsed interval.
      // MessageChannel tasks do not inherit nested timers' minimum delay.
      // Rust still selects the guest thread at each execution boundary.
      if (performance.now() - browserYieldAt >= 4) {
        await yieldBrowserTask();
        browserYieldAt = performance.now();
      }
      const media = pumpMediaWork();
      if (media !== null) {
        if (!media) await schedule();
        continue;
      }
      if (completeReadyBrowserWork()) {
        // A host completion may leave its newly runnable owner selected in Rust
        // while the CPU still holds another thread's context. Reconcile the two
        // before the next API trap; a second waiter can otherwise run under the
        // wrong Rust thread identity.
        await schedule();
        continue;
      }
      if (cpuRetired) {
        await schedule();
        continue;
      }
      if (observeAudioEnds()) {
        await schedule();
        continue;
      }
      if (inputEvents.length > 0 || inputOverflow) {
        bridge.synchronizeForwardClock(instructions, performance.now() - startedAt);
        ingestQueuedInput();
      }
      engine.setRuntimeClock?.(instructions, performance.now() - startedAt);
      const slice = engine.executeBudget(131072);
      generation = engine.memoryGeneration?.() ?? generation;
      instructions += slice.instructions;
      if (slice.stop.kind !== "budget-yield") {
        if (slice.stop.kind === "timestamp-counter") {
          if (!engine.completeTimestampCounter) throw new Error("CPU timestamp completion is unavailable");
          const changed = engine.collectChangedPageViews();
          if (changed.length > 0) {
            generation = bridge.reconcileWritebackPages(generation, changed);
            engine.acknowledgeWriteback?.(generation);
          }
          const counter = bridge.readTimestampCounter(instructions, performance.now() - startedAt);
          engine.completeTimestampCounter(slice.context.instructionPointer, counter.low, counter.high);
          instructions += 1;
          // A loop of timestamp requests must still allow Rust to select
          // another runnable thread even without a whole-budget CPU yield.
          await schedule();
          continue;
        }
        if (slice.stop.kind === "fault") {
          recordRecentEvent("cpu-stop", currentThreadId, slice.context.instructionPointer, slice.stop.fault, slice.context.eax);
        }
        if (slice.stop.kind === "fault" && slice.stop.fault === "page-fault") {
          if (slice.stop.faultAddress === undefined) {
            throw new Error("Guest page fault has no address");
          }
          const changed = engine.collectChangedPageViews();
          if (changed.length > 0) {
            generation = bridge.reconcileWritebackPages(generation, changed);
            engine.acknowledgeWriteback?.(generation);
          }
          const output = bridge.servicePageFault<{ kind: string }>(
            slice.stop.faultAddress, slice.stop.errorCode,
          );
          if (output.metadata.kind !== "page-fault-resolved" || output.control.length === 0) {
            throw new Error(`Guest page fault at 0x${slice.stop.faultAddress.toString(16)} was not resolved`);
          }
          applyDelta(output, "page fault");
          continue;
        }
        if (slice.stop.kind === "trap" && slice.stop.trap === "invalid-opcode" &&
            slice.context.instructionPointer === prepared.metadata.entrySentinel) {
          const changed = engine.collectChangedPageViews();
          if (changed.length > 0) {
            generation = bridge.reconcileWritebackPages(generation, changed);
            engine.acknowledgeWriteback?.(generation);
          }
          if (currentThreadId === prepared.metadata.runtime.threadId) {
            worker.postMessage({ type: "game-exit", exitCode: slice.context.eax >>> 0, detail: `Guest exited with code ${slice.context.eax >>> 0}` });
            return;
          }
          retireThread(slice.context.eax >>> 0);
          await schedule();
          continue;
        }
        if (slice.stop.kind === "trap" && slice.stop.trap === "invalid-opcode" &&
            slice.context.instructionPointer === prepared.metadata.callbackSentinel) {
          recordRecentEvent("callback-return", currentThreadId, slice.context.instructionPointer, null, slice.context.eax);
          const changed = engine.collectChangedPageViews();
          if (changed.length > 0) {
            generation = bridge.reconcileWritebackPages(generation, changed);
            engine.acknowledgeWriteback?.(generation);
          }
          const output = bridge.completeCallback<
            | { kind: "callback-complete"; completion: GameCompletion }
            | { kind: "guest-callback"; target: number; stackPointer: number; returnSentinel: number }
            | { kind: "dll-attach-callback"; target: number; stackPointer: number }
            | { kind: "dll-attach-complete" }
            | GameDispatch
          >(prepared.metadata.callbackSentinel, slice.context.eax >>> 0, slice.context.esp);
          if (output.metadata.kind === "guest-callback" || output.metadata.kind === "com-complete" ||
              output.metadata.kind === "media-request" || output.metadata.kind === "media-data") {
            if (!handleMediaOutput(output as BridgeOutput<GameDispatch>)) await schedule();
          } else if (output.metadata.kind === "callback-complete" &&
                     output.metadata.completion.executionBoundary === "terminate-process") {
            const code = output.metadata.completion.eax;
            if (code === null || !Number.isInteger(code) || code < 0 || code > 0xffff_ffff) throw new Error("Invalid process exit code");
            applyDelta(output, "process exit");
            worker.postMessage({ type: "game-exit", exitCode: code, detail: `Guest exited with code ${code}` });
            return;
          } else if (output.metadata.kind === "callback-complete" &&
                     output.metadata.completion.executionBoundary === undefined) {
            applyDelta(output, "callback return");
            engine.applyCpuResume(output.metadata.completion);
          } else if (output.metadata.kind === "dll-attach-callback") {
            applyDelta(output, "DLL process attach");
            engine.enterGuestCallback(output.metadata.target, output.metadata.stackPointer);
          } else if (output.metadata.kind === "dll-attach-complete" && entryContext) {
            applyDelta(output, "DLL process attach complete");
            engine.restoreCpuContext(entryContext);
          } else {
            throw new Error("Guest callback returned an unsupported continuation");
          }
          continue;
        }
        const dispatchTrap = slice.stop.kind === "runtime-output" || slice.stop.kind === "runtime-dispatch" ||
          (slice.stop.kind === "trap" && slice.stop.trap === "invalid-opcode");
        if (!dispatchTrap) {
          const fault = `Guest stopped at 0x${slice.context.instructionPointer.toString(16)}: ${JSON.stringify(slice.stop)}`;
          try {
            const changed = engine.collectChangedPageViews();
            if (changed.length > 0) {
              generation = bridge.reconcileWritebackPages(generation, changed);
              engine.acknowledgeWriteback?.(generation);
            }
          } catch (error) {
            throw new Error(`${fault}; completed CPU writeback failed: ${error instanceof Error ? error.message : String(error)}`);
          }
          throw new Error(fault);
        }
        const label = `guest trap 0x${slice.context.instructionPointer.toString(16)}`;
        recordRecentEvent("dispatch", currentThreadId, slice.context.instructionPointer);
        // The CPU mirror is not the authority: reconcile guest writes before Rust decodes
        // pointers or arguments from this call frame.
        const changed = engine.collectChangedPageViews();
        if (changed.length > 0) {
          generation = bridge.reconcileWritebackPages(generation, changed);
          engine.acknowledgeWriteback?.(generation);
        }
        const returnAddress = engine.readGuestU32(slice.context.esp);
        const alreadyDispatched = slice.stop.kind === "runtime-output";
        let nativeOutput = alreadyDispatched ? bridge.readPendingDispatch<GameDispatch>() : undefined;
        if (!alreadyDispatched) bridge.synchronizeForwardClock(instructions, performance.now() - startedAt);
        let output;
        for (;;) {
          try {
            output = nativeOutput ?? bridge.dispatch<GameDispatch>(
              slice.context.instructionPointer, slice.context.esp, returnAddress, instructions, slice.context,
            );
            nativeOutput = undefined;
          } catch (error) {
            throw new Error(`${label} stopped: ${error instanceof Error ? error.message : String(error)}`);
          }
          const lookup = output.metadata;
          recordRecentEvent("completion", "threadId" in lookup ? lookup.threadId ?? currentThreadId : currentThreadId,
            slice.context.instructionPointer, lookup.kind,
            "completion" in lookup ? lookup.completion.eax : null);
          if (lookup.kind !== "source-lookup" && lookup.kind !== "source-search" && lookup.kind !== "source-library") break;
          if (lookup.kind === "source-library") {
            if (!Array.isArray(lookup.relativePathA)) throw new Error("Invalid DLL source path");
            const verified = await inspectSourcePath(request.root, lookup.relativePathA);
            if (verified?.kind === "file") recordRecentEvent("completion", currentThreadId,
              slice.context.instructionPointer, `source-library ${verified.pathSegments.join("/")}`);
            let bytes: Uint8Array | null = null;
            if (verified?.kind === "file") {
              if (verified.byteLength > 16 * 1024 * 1024) throw new Error("DLL exceeds the bridge input limit");
              bytes = new Uint8Array(verified.byteLength);
              for (let offset = 0; offset < bytes.length; offset += 2 * 1024 * 1024) {
                const chunk = await readSourceRange(request.root, verified.pathSegments, verified.byteLength,
                  verified.lastModified, offset, Math.min(2 * 1024 * 1024, bytes.length - offset), verified.sourceVersion);
                bytes.set(chunk, offset);
              }
            }
            applyDelta(bridge.completeSourceLibrary(bytes), "DLL mapping");
          } else if (lookup.kind === "source-lookup") {
            if (!Array.isArray(lookup.relativePathA) || !Number.isInteger(lookup.objectId)) {
              throw new Error("Rust returned an invalid source lookup");
            }
            const verified = await inspectSourcePath(request.root, lookup.relativePathA);
            bridge.completeSourceLookup(verified === null ? null : verified.kind === "directory"
              ? { kind: "directory" }
              : { kind: "file", byteLength: verified.byteLength });
            if (verified?.kind === "file") sourceFiles.set(lookup.objectId!, verified);
          } else if (lookup.kind === "source-search") {
            if (!Array.isArray(lookup.relativeDirectoryA)) {
              throw new Error("Rust returned an invalid source search directory");
            }
            const listing = await listSourceDirectory(request.root, lookup.relativeDirectoryA);
            const selected = bridge.sourceSearchNames(listing);
            if (selected !== null) {
              const verifiedFiles = new Map<string, VerifiedSourceFile>();
              const results = [];
              for (const path of selected) {
                if (!Array.isArray(path)) throw new Error("Rust returned an invalid source search path");
                const verified = await inspectSourcePath(request.root, path);
                if (verified?.kind === "file") verifiedFiles.set(path.join(","), verified);
                results.push(verified === null ? null : verified.kind === "directory"
                  ? { kind: "directory" as const }
                  : { kind: "file" as const, byteLength: verified.byteLength });
              }
              for (const entry of bridge.completeSourceSearch(results)) {
                if (!Number.isInteger(entry.objectId) || !Array.isArray(entry.path)) {
                  throw new Error("Rust returned an invalid source search registration");
                }
                const file = verifiedFiles.get(entry.path.join(","));
                if (!file) throw new Error("Source search registered a file without verified metadata");
                sourceFiles.set(entry.objectId, file);
              }
            }
          }
        }
        if (output.metadata.kind === "audio-request") {
          if (output.metadata.threadId !== currentThreadId || output.control.length !== 0 ||
              output.payload.length !== 0 || output.presentation.length !== 0) {
            throw new Error("Audio request disagrees with the current guest call");
          }
          const accepted = await submitAudio(output.metadata.requestId, output.audio);
          bridge.synchronizeForwardClock(instructions, performance.now() - startedAt);
          const completed = bridge.completeComAudio<{ kind: string; completion: CpuResume }>(
            BigInt(output.metadata.requestId), accepted,
          );
          if (completed.metadata.kind !== "com-complete") throw new Error("Audio returned an invalid COM continuation");
          applyDelta(completed, "audio COM completion");
          engine.applyCpuResume(completed.metadata.completion);
          continue;
        }
        if (output.metadata.kind === "com-complete") {
          handleMediaOutput(output, true);
          continue;
        }
        if (output.metadata.kind === "media-request" || output.metadata.kind === "media-data") {
          if (!handleMediaOutput(output)) await schedule();
          continue;
        }
        if (output.metadata.kind === "host-request") {
          const host = output.metadata;
          if (host.threadId !== currentThreadId) {
            throw new Error(`Browser work requested for thread ${host.threadId}, CPU thread ${currentThreadId} by ${label}`);
          }
          if (host.service === "browser.message-box") {
            if (host.messageBox === undefined || host.arguments.length !== 0 ||
                output.control.length !== 0 || output.payload.length !== 0 ||
                output.presentation.length !== 0 || output.audio.length !== 0 ||
                pendingMessageBox !== null || pendingBrowserCompletions.length >= 32 ||
                !/^[1-9][0-9]*$/u.test(host.requestId)) {
              throw new Error("Invalid or excessive message box request");
            }
            const dialog = {
              requestId: BigInt(host.requestId), threadId: currentThreadId,
              service: host.service, acknowledgement: 0, done: false, promise: Promise.resolve(),
            };
            dialog.promise = new Promise<void>(resolve => {
              pendingMessageBox = { requestId: host.requestId, resolve: result => {
                dialog.acknowledgement = result;
                dialog.done = true;
                resolve();
              } };
              worker.postMessage({ type: "message-box", requestId: host.requestId, presentation: host.messageBox });
            });
            pendingBrowserCompletions.push(dialog);
            await schedule();
            continue;
          }
          if (host.service === "browser.opfs-write" || host.service === "browser.opfs-truncate") {
            const truncate = host.service === "browser.opfs-truncate";
            const [low, high, transferred] = host.arguments;
            const offset = BigInt(low!) | (BigInt(high!) << 32n);
            if (typeof host.relativePath !== "string" || host.arguments.length !== 3 ||
                output.control.length !== 0 || !Number.isInteger(low) || !Number.isInteger(high) ||
                !Number.isInteger(transferred) || (truncate ? transferred !== 0 : transferred! < 1) || transferred! > 1024 * 1024 ||
                output.payload.length !== transferred || offset > 0xffff_ffffn) {
              throw new Error("Invalid OPFS write request");
            }
            let observation = 0;
            try {
              if (truncate) await writable.truncateExisting(host.relativePath, Number(offset));
              else await writable.writeExisting(host.relativePath, output.payload, Number(offset));
            } catch (error) {
              if (!(error instanceof DOMException)) throw error;
              switch (error.name) {
                case "NotFoundError": observation = 1; break;
                case "NotAllowedError":
                case "NoModificationAllowedError":
                case "TypeMismatchError": observation = 2; break;
                case "QuotaExceededError": observation = 3; break;
                default: throw error;
              }
            }
            const completed = bridge.completeOpfsWrite<{
              kind: string; threadId: number; completion: GameCompletion;
            }>(BigInt(host.requestId), observation);
            if (completed.metadata.kind !== "host-complete" ||
                completed.metadata.threadId !== currentThreadId ||
                completed.metadata.completion.executionBoundary !== undefined) {
              throw new Error("OPFS write returned an unsupported guest continuation");
            }
            applyDelta(completed, "OPFS file write");
            engine.applyCpuResume(completed.metadata.completion);
            continue;
          }
          if (host.service === "browser.opfs-open") {
            const [disposition, sourceMissing, sourceParents] = host.arguments;
            if (typeof host.relativePath !== "string" || host.arguments.length !== 3 ||
                ![2, 3, 4].includes(disposition!) || ![0, 2, 3].includes(sourceMissing!) ||
                !Number.isInteger(sourceParents) || sourceParents! < 0 || sourceParents! > 128 ||
                sourceParents! > host.relativePath.split("/").length - 1) {
              throw new Error("Invalid OPFS file open request");
            }
            let observation = 0;
            let byteLength = 0;
            try {
              const existing = await writable.stat(host.relativePath);
              if (existing?.kind === "directory") observation = 6;
              else if (disposition === 3) {
                if (existing?.kind === "file") {
                  observation = 1;
                  byteLength = existing.byteLength;
                } else {
                  const parent = host.relativePath.split("/").slice(0, -1).join("/");
                  observation = sourceMissing === 0 ? 7 : parent && (await writable.stat(parent))?.kind !== "directory" ? 3 : 2;
                }
              } else if (disposition === 4 && existing?.kind === "file") {
                observation = 1;
                byteLength = existing.byteLength;
              } else {
                await writable.createEmpty(host.relativePath, sourceParents);
                observation = existing?.kind === "file" ? 1 : 0;
              }
            } catch (error) {
              if (!(error instanceof DOMException)) throw error;
              switch (error.name) {
                case "NotFoundError": observation = 3; break;
                case "NotAllowedError":
                case "NoModificationAllowedError": observation = 4; break;
                case "TypeMismatchError": observation = 6; break;
                case "QuotaExceededError": observation = 5; break;
                default: throw error;
              }
            }
            const completed = bridge.completeOpfsOpen<{
              kind: string; threadId: number; completion: GameCompletion;
            }>(BigInt(host.requestId), observation, byteLength);
            if (completed.metadata.kind !== "host-complete" ||
                completed.metadata.threadId !== currentThreadId ||
                completed.metadata.completion.executionBoundary !== undefined) {
              throw new Error("OPFS file open returned an unsupported guest continuation");
            }
            applyDelta(completed, "OPFS file open");
            engine.applyCpuResume(completed.metadata.completion);
            continue;
          }
          if (host.service === "browser.opfs-stat") {
            if (typeof host.relativePath !== "string" || host.arguments.length !== 0) {
              throw new Error("Invalid OPFS stat request");
            }
            let observation: number;
            try {
              const entry = await writable.stat(host.relativePath);
              if (entry !== null) observation = entry.kind === "file" ? 1 : 2;
              else {
                const parent = host.relativePath.split("/").slice(0, -1).join("/");
                observation = parent && (await writable.stat(parent))?.kind !== "directory" ? 4 : 3;
              }
            } catch (error) {
              if (!(error instanceof DOMException) ||
                  !["NotAllowedError", "NoModificationAllowedError", "TypeMismatchError"].includes(error.name)) throw error;
              observation = 5;
            }
            const completed = bridge.completeHost<{
              kind: string; threadId: number; completion: GameCompletion;
            }>(BigInt(host.requestId), observation);
            if (completed.metadata.kind !== "host-complete" ||
                completed.metadata.threadId !== currentThreadId ||
                completed.metadata.completion.executionBoundary !== undefined) {
              throw new Error("OPFS stat returned an unsupported guest continuation");
            }
            applyDelta(completed, "OPFS file attributes");
            engine.applyCpuResume(completed.metadata.completion);
            continue;
          }
          if (host.service === "browser.opfs-delete-file") {
            if (typeof host.relativePath !== "string" || host.arguments.length !== 0) {
              throw new Error("Invalid OPFS file deletion request");
            }
            let observation = 0;
            try {
              await writable.removeFile(host.relativePath);
            } catch (error) {
              if (!(error instanceof DOMException)) throw error;
              switch (error.name) {
                case "NotFoundError": {
                  const parent = host.relativePath.split("/").slice(0, -1).join("/");
                  observation = parent && (await writable.stat(parent))?.kind !== "directory" ? 2 : 1;
                  break;
                }
                case "NotAllowedError":
                case "NoModificationAllowedError":
                case "TypeMismatchError": observation = 3; break;
                case "QuotaExceededError": observation = 4; break;
                default: throw error;
              }
            }
            const completed = bridge.completeHost<{
              kind: string; threadId: number; completion: GameCompletion;
            }>(BigInt(host.requestId), observation);
            if (completed.metadata.kind !== "host-complete" ||
                completed.metadata.threadId !== currentThreadId ||
                completed.metadata.completion.executionBoundary !== undefined) {
              throw new Error("OPFS deletion returned an unsupported guest continuation");
            }
            applyDelta(completed, "OPFS file deletion");
            engine.applyCpuResume(completed.metadata.completion);
            continue;
          }
          if (host.service === "browser.opfs-create-directory" || host.service === "browser.opfs-ensure-directory") {
            const sourceParents = host.arguments[0];
            if (typeof host.relativePath !== "string" || host.arguments.length !== 1 ||
                typeof sourceParents !== "number" || !Number.isInteger(sourceParents) ||
                sourceParents < 0 || sourceParents > 128 ||
                sourceParents > host.relativePath.split("/").length - 1) {
              throw new Error("Invalid OPFS directory request");
            }
            let observation = 0;
            try {
              if (host.service === "browser.opfs-create-directory" && await writable.stat(host.relativePath) !== null) observation = 1;
              else await writable.createDirectory(host.relativePath, sourceParents);
            } catch (error) {
              if (!(error instanceof DOMException)) throw error;
              switch (error.name) {
                case "NotFoundError": observation = 2; break;
                case "NotAllowedError":
                case "NoModificationAllowedError":
                case "TypeMismatchError": observation = 3; break;
                case "QuotaExceededError": observation = 4; break;
                default: throw error;
              }
            }
            const completed = bridge.completeHost<{
              kind: string; threadId: number; completion: GameCompletion;
            }>(BigInt(host.requestId), observation);
            if (completed.metadata.kind !== "host-complete" ||
                completed.metadata.threadId !== currentThreadId ||
                completed.metadata.completion.executionBoundary !== undefined) {
              throw new Error("OPFS directory returned an unsupported guest continuation");
            }
            applyDelta(completed, "OPFS directory creation");
            engine.applyCpuResume(completed.metadata.completion);
            continue;
          }
          if (host.service === "browser.gdi-text-raster") {
            const pixels = rasterizeGdiText(host.arguments);
            const completed = bridge.completeGdiTextRaster<{
              kind: string; threadId: number; completion: GameCompletion;
            }>(BigInt(host.requestId), pixels);
            if (completed.metadata.kind !== "host-complete" ||
                completed.metadata.threadId !== currentThreadId ||
                completed.metadata.completion.executionBoundary !== undefined) {
              throw new Error("GDI raster returned an unsupported guest continuation");
            }
            applyDelta(completed, "GDI text raster");
            engine.applyCpuResume(completed.metadata.completion);
            continue;
          }
          if (host.service === "browser.timer-delay" || host.service === "browser.sleep" ||
              host.service === "browser.audio-delay" || host.service === "browser.object-wait" || host.service === "browser.message-wait") {
            const wait = host.service === "browser.timer-delay";
            const audio = host.service === "browser.audio-delay";
            const milliseconds = browserDelayMilliseconds(host.service, host.arguments);
            if (pendingBrowserCompletions.length >= 32) {
              throw new Error("Invalid or excessive browser timer request");
            }
            const delay = {
              requestId: BigInt(host.requestId), threadId: currentThreadId,
              service: host.service,
              acknowledgement: wait || audio || host.service === "browser.object-wait" || host.service === "browser.message-wait" ? 1 : 0, done: false,
              promise: Promise.resolve(),
            };
            armBrowserDelay(delay, milliseconds);
            pendingBrowserCompletions.push(delay);
            await schedule();
            continue;
          }
          if ((host.service !== "storage.source-read" && host.service !== "browser.opfs-read") ||
              host.arguments.length !== 5) {
            throw new Error(`Unsupported browser work requested by ${label}`);
          }
          let readHost = host;
          for (;;) {
          const [sourceId, objectId, low, high, requested] = readHost.arguments;
          const offset = BigInt(low!) | (BigInt(high!) << 32n);
          let bytes: Uint8Array;
          let win32Error = 0;
          if (readHost.service === "browser.opfs-read") {
            if (sourceId !== 0 || !Number.isInteger(objectId) || objectId! < 1 ||
                typeof readHost.relativePath !== "string" ||
                offset > 0xffff_ffffn || requested! <= 0 || requested! > 16 * 1024 * 1024 - 28) {
              throw new Error("Invalid Rust OPFS read request");
            }
            try {
              const result = await writable.read(readHost.relativePath!, Number(offset), requested!);
              if (result === null) {
                bytes = new Uint8Array();
                win32Error = 2;
              } else bytes = result;
            } catch (error) {
              if (!(error instanceof DOMException) ||
                  !["NotAllowedError", "NoModificationAllowedError", "TypeMismatchError"].includes(error.name)) throw error;
              bytes = new Uint8Array();
              win32Error = 5;
            }
          } else {
            const source = sourceFiles.get(objectId!);
            if (sourceId !== 1 || source === undefined ||
                offset > 0xffff_ffffn || requested! <= 0 || requested! > 16 * 1024 * 1024 - 28) {
              throw new Error("Invalid Rust source read request");
            }
            bytes = await readSourceRange(request.root, source.pathSegments,
              source.byteLength, source.lastModified, Number(offset), requested!, source.sourceVersion);
          }
          const control = new Uint8Array(28);
          const view = new DataView(control.buffer);
          [sourceId!, objectId!, low!, high!, requested!, bytes.length, win32Error]
            .forEach((word, index) => view.setUint32(index * 4, word, true));
          const completed = bridge.completeSourceRead<{
            kind: string; threadId: number; completion: GameCompletion;
            requestId?: string; service?: string; arguments?: number[];
          }>(BigInt(readHost.requestId), control, bytes);
          if (completed.metadata.kind === "host-retry") {
            if (readHost.service !== "storage.source-read" || completed.metadata.threadId !== currentThreadId ||
                completed.metadata.requestId !== readHost.requestId || completed.metadata.service !== readHost.service ||
                completed.metadata.arguments?.length !== 5 || completed.control.length || completed.payload.length) {
              throw new Error("Invalid MMIO source read continuation");
            }
            readHost = { ...readHost, arguments: completed.metadata.arguments };
            continue;
          }
          if (completed.metadata.kind !== "host-complete" ||
              completed.metadata.threadId !== currentThreadId ||
              completed.metadata.completion.executionBoundary !== undefined) {
            throw new Error("Source read returned an unsupported guest continuation");
          }
          applyDelta(completed, host.service === "browser.opfs-read" ? "OPFS read" : "source read");
          engine.applyCpuResume(completed.metadata.completion);
          break;
          }
          continue;
        }
        if (output.metadata.kind === "guest-callback") {
          handleMediaOutput(output);
          continue;
        }
        if (output.metadata.kind !== "api-complete") throw new Error(`Unexpected API completion for ${label}`);
        const boundary = output.metadata.completion.executionBoundary;
        if (boundary !== undefined && boundary !== "yield-current-thread" &&
            boundary !== "block-current-thread" && boundary !== "terminate-current-thread" && boundary !== "terminate-process") {
          throw new Error(`Imported function ${label} requested an unimplemented scheduler boundary`);
        }
        if (boundary === "block-current-thread" &&
            output.metadata.threadSelection?.threadId === currentThreadId) {
          throw new Error(`Imported function ${label} selected its blocked guest thread`);
        }
        applyDelta(output, label);
        if (boundary === "terminate-process") {
          const code = output.metadata.completion.eax;
          if (code === null || !Number.isInteger(code) || code < 0 || code > 0xffff_ffff || output.metadata.threadSelection !== undefined) throw new Error(`Invalid process exit for ${label}`);
          worker.postMessage({ type: "game-exit", exitCode: code, detail: `Guest exited with code ${code}` });
          return;
        }
        if (boundary === "terminate-current-thread") {
          const eax = output.metadata.completion.eax;
          if (eax === null || !Number.isInteger(eax) || eax < 0 || eax > 0xffff_ffff ||
              output.metadata.threadSelection !== undefined) {
            throw new Error(`Invalid terminating completion for ${label}`);
          }
          retireThread(eax);
          await schedule();
          continue;
        }
        installCreatedThread(output.metadata.createdThread);
        engine.applyStoppedCpuResume(output.metadata.completion);
        if (output.metadata.threadSelection !== undefined) {
          switchThread(output.metadata.threadSelection.threadId);
        } else if (boundary === "block-current-thread") {
          parkCurrentCpu();
          await schedule();
        }
        continue;
      }
      // The CPU runs one logical thread at a time; Rust chooses the next runnable thread.
      await schedule();
    }
  } finally {
    for (const delay of pendingBrowserCompletions) delay.cancelTimer?.();
    mediaClosed = true;
    for (const resolve of pendingMediaAudio.values()) resolve(false);
    pendingMediaAudio.clear();
    failedMediaAudio.clear();
    for (const task of mediaTasks.values()) {
      task.cancelled = true;
      task.cancelDelay?.();
      task.read?.reject(new Error("Chrome media worker was stopped"));
      task.read = null;
      task.consume();
    }
    void Promise.allSettled([...mediaTasks.values()].map(task => task.promise)).then(async () => {
      for (const resource of mediaResources.values()) await resource.decoder?.close();
      mediaResources.clear();
    }).catch(() => { /* The terminal worker cannot resume a guest continuation. */ });
    browserTasks.port1.onmessage = null;
    browserTasks.port1.close();
    browserTasks.port2.close();
  }
}
