/// <reference lib="webworker" />

import {
  decodeReflection,
  encodeCpuWriteback,
  type MemoryReflection,
  type ReflectedPage,
  memoryPageSize,
} from "../runtime-worker/memory-transport.js";
import {
  type ApiCompletion,
  type PreparedMetadata,
  RuntimeBridge,
} from "../runtime-worker/runtime-bridge.js";
import { V86Mirror, v86StopReason } from "../runtime-worker/v86-mirror.js";

interface BenchmarkConfiguration {
  workloadDurationMilliseconds: number;
}

type BenchmarkRequest =
  | { type: "cold-start" }
  | { type: "run-suite"; configuration: BenchmarkConfiguration }
  | { type: "integer-probe"; durationMilliseconds: number };

interface ProductionSession {
  bridge: RuntimeBridge;
  mirror: V86Mirror;
  generation: bigint;
}

interface WritebackReconciliationMetadata {
  kind: string;
  receipt: {
    generation: number;
    touchedPages: number;
  };
}

interface HostRequestMetadata {
  kind: string;
  requestId: number;
  service: string;
  arguments: number[];
}

interface CompletionMetadata {
  kind: string;
  completion: ApiCompletion;
}

const worker = self as DedicatedWorkerGlobalScope;
const codeBase = 0x1000_0000;
const stackBase = 0x006f_f000;
const stackPointer = 0x006f_ff00;
const controlBase = 0x7f00_0000;
const callbackTarget = codeBase + 0x100;
const callbackStack = stackBase + 0xe00;
const dataBase = 0x2000_0000;
const adapterUrl = new URL("./v86-adapter.wasm", import.meta.url);
const runtimeCoreUrl = new URL("./runtime-core.wasm", import.meta.url);
const fixtureUrl = new URL("./browser-vertical.exe", import.meta.url);
let steadyProductionSession: ProductionSession | null = null;

const integerCode = Uint8Array.from([
  0xb9, 0xff, 0xff, 0xff, 0x7f,
  0xb8, 0xdf, 0x9b, 0x57, 0x13,
  0xbb, 0xe0, 0xac, 0x68, 0x24,
  0x01, 0xd8,
  0x31, 0xc3,
  0xc1, 0xc0, 0x05,
  0x6b, 0xd8, 0x03,
  0x49,
  0x75, 0xf3,
]);

const x87Code = Uint8Array.from([
  0xb9, 0xff, 0xff, 0xff, 0x7f,
  0xd9, 0xe8,
  0xd9, 0xeb,
  0xd8, 0xc1,
  0xd8, 0xc9,
  0xd8, 0xe1,
  0xd8, 0xf1,
  0x49,
  0x75, 0xf5,
]);

const sseCode = Uint8Array.from([
  0xb9, 0xff, 0xff, 0xff, 0x7f,
  0x66, 0x0f, 0x76, 0xc0,
  0x66, 0x0f, 0xef, 0xc9,
  0x66, 0x0f, 0xfe, 0xc8,
  0x66, 0x0f, 0xfa, 0xc8,
  0x66, 0x0f, 0xef, 0xc8,
  0x66, 0x0f, 0xef, 0xc8,
  0x49,
  0x75, 0xed,
]);

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function page(
  address: number,
  owner: number,
  protection: { read: boolean; write: boolean; execute: boolean },
  content: Uint8Array = new Uint8Array(memoryPageSize),
): ReflectedPage {
  const bytes = new Uint8Array(memoryPageSize);
  bytes.set(content);
  return { address, owner, protection, ownerSealed: true, dirty: true, bytes };
}

function reflection(pages: ReflectedPage[]): MemoryReflection {
  return {
    generation: 1n,
    cacheEffects: {
      flushTlb: true,
      invalidateInstructionFetch: true,
      invalidateGeneratedCode: true,
    },
    pages,
  };
}

function prepared(entryPoint: number, pageCount: number): PreparedMetadata {
  return {
    kind: "prepared",
    schemaVersion: 1,
    entryPoint,
    generation: 1,
    stackTop: stackBase + memoryPageSize,
    initialStackPointer: stackPointer,
    callbackSentinel: controlBase + 0x10,
    entrySentinel: controlBase + 0x20,
    pageCount,
    endpoints: [],
  };
}

async function createMirror(
  code: Uint8Array,
  additionalPages: ReflectedPage[] = [],
): Promise<V86Mirror> {
  const mirror = await V86Mirror.instantiate(adapterUrl);
  const pages = [
    page(codeBase, 1, { read: true, write: false, execute: true }, code),
    page(stackBase, 2, { read: true, write: true, execute: false }),
    ...additionalPages,
  ];
  mirror.install(reflection(pages), prepared(codeBase, pages.length));
  return mirror;
}

function median(values: number[]): number {
  invariant(values.length > 0, "median requires samples");
  const ordered = [...values].sort((left, right) => left - right);
  const middle = Math.floor(ordered.length / 2);
  return ordered.length % 2 === 0
    ? ((ordered[middle - 1] ?? 0) + (ordered[middle] ?? 0)) / 2
    : (ordered[middle] ?? 0);
}

function percentile(values: number[], percentile_: number): number {
  invariant(values.length > 0, "percentile requires samples");
  const ordered = [...values].sort((left, right) => left - right);
  const index = Math.min(ordered.length - 1, Math.ceil(percentile_ * ordered.length) - 1);
  return ordered[index] ?? 0;
}

async function instructionThroughput(
  id: string,
  code: Uint8Array,
  durationMilliseconds: number,
): Promise<Record<string, number | string>> {
  const mirror = await createMirror(code);
  for (let index = 0; index < 3; index += 1) {
    const warm = mirror.execute(100_000);
    invariant(warm.reason === v86StopReason.budget, `${id} warmup did not yield by budget`);
  }
  let instructions = 0;
  let slices = 0;
  const startedAt = performance.now();
  while (performance.now() - startedAt < durationMilliseconds) {
    const result = mirror.execute(250_000);
    invariant(result.reason === v86StopReason.budget, `${id} workload stopped unexpectedly`);
    instructions += result.instructions;
    slices += 1;
  }
  const elapsedMilliseconds = performance.now() - startedAt;
  return {
    id,
    instructions,
    slices,
    elapsedMilliseconds,
    instructionsPerSecond: (instructions * 1000) / elapsedMilliseconds,
    wasmExportCallsPerSlice: 8,
    wasmExportCalls: slices * 8,
  };
}

async function sliceMatrix(integerInstructionsPerSecond: number): Promise<Record<string, unknown>[]> {
  const targets = [1, 5, 10, 16];
  const results: Record<string, unknown>[] = [];
  for (const targetMilliseconds of targets) {
    const mirror = await createMirror(integerCode);
    const budget = Math.max(
      1,
      Math.round((integerInstructionsPerSecond * targetMilliseconds) / 1000),
    );
    const slices = Math.max(24, Math.ceil(1500 / targetMilliseconds));
    for (let index = 0; index < 3; index += 1) {
      mirror.execute(budget);
    }
    const startedAt = performance.now();
    let instructions = 0;
    for (let index = 0; index < slices; index += 1) {
      const result = mirror.execute(budget);
      invariant(result.reason === v86StopReason.budget, "slice workload stopped unexpectedly");
      instructions += result.instructions;
    }
    const elapsedMilliseconds = performance.now() - startedAt;
    const effectiveInstructionsPerSecond = (instructions * 1000) / elapsedMilliseconds;
    results.push({
      targetMilliseconds,
      budget,
      slices,
      elapsedMilliseconds,
      meanSliceMilliseconds: elapsedMilliseconds / slices,
      effectiveInstructionsPerSecond,
      relativeThroughput: effectiveInstructionsPerSecond / integerInstructionsPerSecond,
      wasmExportCallsPerSlice: 8,
    });
  }
  return results;
}

function writeRelative32(bytes: Uint8Array, offset: number, target: number, sourceAfter: number): void {
  new DataView(bytes.buffer).setInt32(offset, target - sourceAfter, true);
}

async function trapRoundTrips(): Promise<Record<string, number>> {
  const code = new Uint8Array(memoryPageSize);
  code[0] = 0xe8;
  writeRelative32(code, 1, controlBase, codeBase + 5);
  code[5] = 0xe9;
  writeRelative32(code, 6, codeBase, codeBase + 10);
  const control = new Uint8Array(memoryPageSize);
  control.set([0x0f, 0x0b], 0);
  const mirror = await createMirror(code, [
    page(controlBase, 3, { read: true, write: false, execute: true }, control),
  ]);
  const rounds = 5_000;
  const runRound = () => {
    const trap = mirror.execute(32);
    invariant(trap.reason === v86StopReason.invalidOpcode, "API trap did not stop on UD2");
    invariant(trap.state.instructionPointer === controlBase, "API trap address mismatch");
    const returnAddress = mirror.readU32(trap.state.esp);
    mirror.applyCompletion({
      implementationId: "benchmark-trap",
      instructionPointer: returnAddress,
      stackPointer: trap.state.esp + 4,
      eax: { action: "set", value: 1 },
      edx: { action: "preserve" },
      eflags: { action: "preserve" },
      lastError: 0,
    });
  };
  for (let index = 0; index < 100; index += 1) {
    runRound();
  }
  const startedAt = performance.now();
  for (let index = 0; index < rounds; index += 1) {
    runRound();
  }
  const elapsedMilliseconds = performance.now() - startedAt;
  return {
    rounds,
    elapsedMilliseconds,
    roundTripsPerSecond: (rounds * 1000) / elapsedMilliseconds,
    microsecondsPerRoundTrip: (elapsedMilliseconds * 1000) / rounds,
    wasmExportCallsPerRoundTrip: 14,
    wasmExportCalls: rounds * 14,
  };
}

async function callbackRoundTrips(): Promise<Record<string, number>> {
  const code = new Uint8Array(memoryPageSize);
  code[0] = 0xe8;
  writeRelative32(code, 1, controlBase, codeBase + 5);
  code[5] = 0xe9;
  writeRelative32(code, 6, codeBase, codeBase + 10);
  code.set([0x8b, 0x44, 0x24, 0x04, 0x40, 0xc3], callbackTarget - codeBase);
  const control = new Uint8Array(memoryPageSize);
  control.set([0x0f, 0x0b], 0);
  control.set([0x0f, 0x0b], 0x10);
  const stack = new Uint8Array(memoryPageSize);
  const stackView = new DataView(stack.buffer);
  stackView.setUint32(callbackStack - stackBase, controlBase + 0x10, true);
  stackView.setUint32(callbackStack - stackBase + 4, 42, true);
  const mirror = await V86Mirror.instantiate(adapterUrl);
  const pages = [
    page(codeBase, 1, { read: true, write: false, execute: true }, code),
    page(stackBase, 2, { read: true, write: true, execute: false }, stack),
    page(controlBase, 3, { read: true, write: false, execute: true }, control),
  ];
  mirror.install(reflection(pages), prepared(codeBase, pages.length));
  const rounds = 2_000;
  const runRound = () => {
    const apiTrap = mirror.execute(32);
    invariant(apiTrap.state.instructionPointer === controlBase, "callback API trap mismatch");
    const returnAddress = mirror.readU32(apiTrap.state.esp);
    mirror.enterCallback(callbackTarget, callbackStack);
    const callbackReturn = mirror.execute(32);
    invariant(
      callbackReturn.state.instructionPointer === controlBase + 0x10,
      "callback sentinel mismatch",
    );
    invariant(callbackReturn.state.eax === 43, "callback result mismatch");
    mirror.applyCompletion({
      implementationId: "benchmark-callback",
      instructionPointer: returnAddress,
      stackPointer: apiTrap.state.esp + 4,
      eax: { action: "set", value: callbackReturn.state.eax },
      edx: { action: "preserve" },
      eflags: { action: "preserve" },
      lastError: 0,
    });
  };
  for (let index = 0; index < 50; index += 1) {
    runRound();
  }
  const startedAt = performance.now();
  for (let index = 0; index < rounds; index += 1) {
    runRound();
  }
  const elapsedMilliseconds = performance.now() - startedAt;
  return {
    rounds,
    elapsedMilliseconds,
    roundTripsPerSecond: (rounds * 1000) / elapsedMilliseconds,
    microsecondsPerRoundTrip: (elapsedMilliseconds * 1000) / rounds,
    wasmExportCallsPerRoundTrip: 28,
    wasmExportCalls: rounds * 28,
  };
}

async function contextCost(): Promise<Record<string, number>> {
  const mirror = await createMirror(integerCode);
  const state = mirror.saveState();
  const samples = 5_000;
  let startedAt = performance.now();
  for (let index = 0; index < samples; index += 1) {
    mirror.saveState();
  }
  const saveMilliseconds = performance.now() - startedAt;
  startedAt = performance.now();
  for (let index = 0; index < samples; index += 1) {
    mirror.enterCallback(state.instructionPointer, state.esp);
  }
  const saveRestoreMilliseconds = performance.now() - startedAt;
  return {
    samples,
    saveMicroseconds: (saveMilliseconds * 1000) / samples,
    saveRestoreMicroseconds: (saveRestoreMilliseconds * 1000) / samples,
    estimatedRestoreMicroseconds:
      ((saveRestoreMilliseconds - saveMilliseconds) * 1000) / samples,
    wasmExportCallsPerSave: 3,
    wasmExportCallsPerSaveRestore: 6,
  };
}

async function reflectionBandwidth(): Promise<Record<string, unknown>[]> {
  const results: Record<string, unknown>[] = [];
  for (const pageCount of [8, 64, 256]) {
    const samples: number[] = [];
    for (let sampleIndex = 0; sampleIndex < 5; sampleIndex += 1) {
      const mirror = await V86Mirror.instantiate(adapterUrl);
      const pages = [
        page(codeBase, 1, { read: true, write: false, execute: true }, integerCode),
        page(stackBase, 2, { read: true, write: true, execute: false }),
      ];
      for (let index = 0; index < pageCount - 2; index += 1) {
        pages.push(
          page(dataBase + index * memoryPageSize, 3, {
            read: true,
            write: true,
            execute: false,
          }),
        );
      }
      const startedAt = performance.now();
      mirror.install(reflection(pages), prepared(codeBase, pages.length));
      samples.push(performance.now() - startedAt);
    }
    const medianMilliseconds = median(samples);
    results.push({
      pages: pageCount,
      bytes: pageCount * memoryPageSize,
      samplesMilliseconds: samples,
      medianMilliseconds,
      mebibytesPerSecond:
        ((pageCount * memoryPageSize) / (1024 * 1024)) * (1000 / medianMilliseconds),
    });
  }
  return results;
}

function writebackProgram(pageCount: number): Uint8Array {
  const code = new Uint8Array(memoryPageSize);
  let offset = 0;
  code[offset++] = 0xb8;
  new DataView(code.buffer).setUint32(offset, 0x5a5a_5a5a, true);
  offset += 4;
  code[offset++] = 0xbf;
  new DataView(code.buffer).setUint32(offset, dataBase, true);
  offset += 4;
  code[offset++] = 0xb9;
  new DataView(code.buffer).setUint32(offset, pageCount * (memoryPageSize / 4), true);
  offset += 4;
  code.set([0xf3, 0xab, 0x0f, 0x0b], offset);
  return code;
}

async function mirrorWritebackBandwidth(): Promise<Record<string, unknown>[]> {
  const results: Record<string, unknown>[] = [];
  for (const pageCount of [1, 16, 64, 128]) {
    const samples: number[] = [];
    for (let sampleIndex = 0; sampleIndex < 5; sampleIndex += 1) {
      const pages: ReflectedPage[] = [];
      for (let index = 0; index < pageCount; index += 1) {
        pages.push(
          page(dataBase + index * memoryPageSize, 3, {
            read: true,
            write: true,
            execute: false,
          }),
        );
      }
      const mirror = await createMirror(writebackProgram(pageCount), pages);
      const execution = mirror.execute(pageCount * (memoryPageSize / 4) + 100);
      invariant(execution.reason === v86StopReason.invalidOpcode, "writeback writer did not finish");
      const startedAt = performance.now();
      const dirtyPages = mirror.discoverDirtyPages();
      const buffers = encodeCpuWriteback(1n, dirtyPages);
      const elapsed = performance.now() - startedAt;
      invariant(dirtyPages.length === pageCount, "writeback dirty-page count mismatch");
      invariant(buffers.payload.byteLength === pageCount * memoryPageSize, "writeback size mismatch");
      samples.push(elapsed);
    }
    const medianMilliseconds = median(samples);
    results.push({
      pages: pageCount,
      bytes: pageCount * memoryPageSize,
      samplesMilliseconds: samples,
      medianMilliseconds,
      mebibytesPerSecond:
        ((pageCount * memoryPageSize) / (1024 * 1024)) * (1000 / medianMilliseconds),
    });
  }
  return results;
}

async function productionWritebackBandwidth(): Promise<Record<string, unknown>> {
  const fixtureBytes = new Uint8Array(await (await fetch(fixtureUrl)).arrayBuffer());
  const samples: number[] = [];
  for (let sampleIndex = 0; sampleIndex < 9; sampleIndex += 1) {
    const [bridge, mirror] = await Promise.all([
      RuntimeBridge.instantiate(runtimeCoreUrl),
      V86Mirror.instantiate(adapterUrl),
    ]);
    const preparedOutput = bridge.prepare(fixtureBytes);
    const initial = decodeReflection(preparedOutput.control, preparedOutput.payload);
    mirror.install(initial, preparedOutput.metadata);
    bridge.acknowledge(initial.generation);
    const execution = mirror.execute(4);
    invariant(
      execution.reason === v86StopReason.budget ||
        execution.reason === v86StopReason.invalidOpcode,
      "production writeback setup did not reach a bounded stop",
    );
    const dirty = mirror.discoverDirtyPages();
    invariant(dirty.length === 1, "production writeback setup expected one dirty page");
    const buffers = encodeCpuWriteback(initial.generation, dirty);
    const startedAt = performance.now();
    const output = bridge.reconcileWriteback<{
      kind: string;
      receipt: { touchedPages: number };
    }>(buffers.control, buffers.payload);
    samples.push(performance.now() - startedAt);
    invariant(output.metadata.kind === "writeback-reconciled", "production receipt kind mismatch");
    invariant(output.metadata.receipt.touchedPages === 1, "production receipt page mismatch");
  }
  const medianMilliseconds = median(samples);
  return {
    pagesPerBatch: 1,
    bytesPerBatch: memoryPageSize,
    samplesMilliseconds: samples,
    medianMilliseconds,
    mebibytesPerSecond: (memoryPageSize / (1024 * 1024)) * (1000 / medianMilliseconds),
  };
}

async function productionApiRoundTrips(): Promise<Record<string, unknown>> {
  const rounds = 101;
  const samples: number[] = [];
  for (let index = 0; index < rounds; index += 1) {
    const session = await setupProductionSession();
    const execution = session.mirror.execute(8);
    invariant(
      execution.reason === v86StopReason.invalidOpcode && execution.errorCode === -1,
      "production API setup did not reach an owned UD2",
    );
    const dirtyPages = session.mirror.discoverDirtyPages();
    invariant(dirtyPages.length === 1, "production API setup expected one dirty stack page");
    const writeback = encodeCpuWriteback(session.generation, dirtyPages);
    const reconciliation = session.bridge.reconcileWriteback<WritebackReconciliationMetadata>(
      writeback.control,
      writeback.payload,
    ).metadata;
    invariant(reconciliation.kind === "writeback-reconciled", "production API writeback kind mismatch");
    invariant(reconciliation.receipt.touchedPages === 1, "production API writeback page mismatch");
    session.generation = BigInt(reconciliation.receipt.generation);

    const endpoint = execution.state.instructionPointer;
    const returnAddress = session.mirror.readU32(execution.state.esp);
    const startedAt = performance.now();
    const request = session.bridge.dispatch<HostRequestMetadata>(
      endpoint,
      execution.state.esp,
      returnAddress,
    ).metadata;
    invariant(request.kind === "host-request", "production API dispatch was not deferred");
    invariant(request.service === "browser.deferred-add", "production API service mismatch");
    invariant(
      request.arguments.length === 2 &&
        request.arguments[0] === 19 &&
        request.arguments[1] === 23,
      "production API arguments mismatch",
    );
    const completion = session.bridge.completeHost<CompletionMetadata>(
      BigInt(request.requestId),
      42,
    ).metadata;
    invariant(completion.kind === "host-complete", "production API completion kind mismatch");
    session.mirror.applyCompletion(completion.completion);
    samples.push(performance.now() - startedAt);

    const resumed = session.mirror.saveState();
    invariant(resumed.instructionPointer === returnAddress, "production API resume address mismatch");
    invariant(resumed.eax === 42, "production API result mismatch");
    const cancelled = session.bridge.cancel<{ kind: string; pendingHostCalls: number }>().metadata;
    invariant(cancelled.kind === "cancelled", "production API sample cleanup kind mismatch");
    invariant(cancelled.pendingHostCalls === 0, "production API sample retained a host request");
  }
  const elapsedMilliseconds = samples.reduce((total, sample) => total + sample, 0);
  return {
    rounds,
    samplesMilliseconds: samples,
    elapsedMilliseconds,
    roundTripsPerSecond: (rounds * 1000) / elapsedMilliseconds,
    medianMicroseconds: median(samples) * 1000,
    p95Microseconds: percentile(samples, 0.95) * 1000,
    runtimeBridgeExportCallsPerRoundTrip: 14,
    adapterExportCallsPerRoundTrip: 6,
    wasmExportCallsPerRoundTrip: 20,
  };
}

async function invalidationCost(): Promise<Record<string, number>> {
  const mirror = await createMirror(integerCode);
  const iterations = 10_000;
  const effects = {
    flushTlb: true,
    invalidateInstructionFetch: true,
    invalidateGeneratedCode: true,
  };
  for (let index = 0; index < 100; index += 1) {
    mirror.applyCacheEffects(effects);
  }
  const startedAt = performance.now();
  for (let index = 0; index < iterations; index += 1) {
    mirror.applyCacheEffects(effects);
  }
  const elapsedMilliseconds = performance.now() - startedAt;
  return {
    iterations,
    elapsedMilliseconds,
    microsecondsPerInvalidation: (elapsedMilliseconds * 1000) / iterations,
    wasmExportCallsPerInvalidation: 2,
  };
}

async function setupProductionSession(): Promise<ProductionSession> {
  const [bridge, mirror, fixtureResponse] = await Promise.all([
    RuntimeBridge.instantiate(runtimeCoreUrl),
    V86Mirror.instantiate(adapterUrl),
    fetch(fixtureUrl),
  ]);
  invariant(fixtureResponse.ok, "cold-start fixture fetch failed");
  const output = bridge.prepare(new Uint8Array(await fixtureResponse.arrayBuffer()));
  const initial = decodeReflection(output.control, output.payload);
  mirror.install(initial, output.metadata);
  bridge.acknowledge(initial.generation);
  return { bridge, mirror, generation: initial.generation };
}

async function runSuite(configuration: BenchmarkConfiguration): Promise<Record<string, unknown>> {
  worker.postMessage({ type: "progress", phase: "cpu" });
  const integer = await instructionThroughput(
    "integer-mix-v1",
    integerCode,
    configuration.workloadDurationMilliseconds,
  );
  const x87 = await instructionThroughput(
    "x87-mix-v1",
    x87Code,
    configuration.workloadDurationMilliseconds,
  );
  const sse = await instructionThroughput(
    "sse2-integer-mix-v1",
    sseCode,
    configuration.workloadDurationMilliseconds,
  );
  worker.postMessage({ type: "progress", phase: "abi" });
  const traps = await trapRoundTrips();
  const productionApi = await productionApiRoundTrips();
  const callbacks = await callbackRoundTrips();
  const context = await contextCost();
  const slices = await sliceMatrix(Number(integer.instructionsPerSecond));
  worker.postMessage({ type: "progress", phase: "memory" });
  const reflection_ = await reflectionBandwidth();
  const mirrorWriteback = await mirrorWritebackBandwidth();
  const productionWriteback = await productionWritebackBandwidth();
  const invalidation = await invalidationCost();
  steadyProductionSession = await setupProductionSession();
  return {
    integer,
    x87,
    sse,
    traps,
    productionApi,
    callbacks,
    context,
    slices,
    reflection: reflection_,
    mirrorWriteback,
    productionWriteback,
    invalidation,
    environment: {
      hardwareConcurrency: navigator.hardwareConcurrency,
      deviceMemoryGiB:
        "deviceMemory" in navigator
          ? (navigator as Navigator & { deviceMemory: number }).deviceMemory
          : null,
      userAgent: navigator.userAgent,
      steadyProductionSessionRetained: steadyProductionSession !== null,
    },
  };
}

worker.addEventListener("message", (event: MessageEvent<BenchmarkRequest>) => {
  void (async () => {
    try {
      switch (event.data.type) {
        case "cold-start": {
          const startedAt = performance.now();
          await setupProductionSession();
          worker.postMessage({
            type: "cold-start-complete",
            internalMilliseconds: performance.now() - startedAt,
          });
          worker.close();
          break;
        }
        case "run-suite":
          worker.postMessage({
            type: "suite-complete",
            result: await runSuite(event.data.configuration),
          });
          break;
        case "integer-probe":
          invariant(steadyProductionSession !== null, "steady production session was not retained");
          worker.postMessage({
            type: "integer-probe-complete",
            result: await instructionThroughput(
              "background-integer-mix-v1",
              integerCode,
              event.data.durationMilliseconds,
            ),
          });
          break;
      }
    } catch (error) {
      worker.postMessage({
        type: "benchmark-error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  })();
});
