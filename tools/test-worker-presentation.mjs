// Exercise production COM presentation ordering and its failure boundary.
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const workerDirectory = resolve('browser/runtime-worker');
const workerSource = resolve(workerDirectory, 'game-worker.ts');
const dependencies = {
  '../storage/source-directory.js': "export const gameStorageFingerprint = async () => 'storage';",
  './execution-engine.js': 'export const instantiateExecutionEngine = async () => globalThis.workerDiagnostics.engine;',
  './gdi-text-raster.js': "export const rasterizeGdiText = () => { throw new Error('Unexpected GDI work'); };",
  './game-surface.js': `import { texturePixels, validatePresentation } from './game-presentation.js';
    export class GameSurface {
      backend = 'webgl2';
      present(frame, payload) {
        validatePresentation(frame, payload, 4096);
        const pixels = frame.draws.map(draw => texturePixels(draw.texture, payload));
        globalThis.workerDiagnostics.present(frame, payload, pixels);
      }
    }`,
  './runtime-bridge.js': 'export const RuntimeBridge = { instantiate: async () => globalThis.workerDiagnostics.bridge };',
  './media-decoder.js': 'export class MediaDecoder {} export class UnsupportedMediaFormat extends Error {}',
  './source-explorer.js': `export const openSourceExecutable = async () => ({
      size: 1, lastModified: 0, arrayBuffer: async () => new Uint8Array([0]).buffer });
    export const browseSourceDirectory = async () => [];
    export const inspectSourcePath = async () => null;
    export const listSourceDirectory = async () => [];
    export const openSourceDll = async () => null;
    export const readSourceRange = async () => new Uint8Array();`,
  './writable-tree.js': 'export const openWritableTree = async () => ({});',
};
const bundle = await build({
  entryPoints: [workerSource], bundle: true, write: false,
  platform: 'node', format: 'esm',
  define: { 'import.meta.url': JSON.stringify(pathToFileURL(workerSource).href) },
  plugins: [{ name: 'worker-presentation-resources', setup(plugin) {
    plugin.onResolve({ filter: /^\./ }, args => Object.hasOwn(dependencies, args.path)
      ? { path: args.path, namespace: 'worker-resource' } : undefined);
    plugin.onLoad({ filter: /.*/, namespace: 'worker-resource' }, args => ({
      contents: dependencies[args.path], loader: 'js', resolveDir: workerDirectory,
    }));
  } }],
});
const workerModule = `data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].contents).toString('base64')}`;

function reflection() {
  const control = new Uint8Array(36), words = new DataView(control.buffer);
  control.set(new TextEncoder().encode('VINEREFL'));
  words.setUint32(8, 4096, true);
  words.setBigUint64(12, 1n, true);
  words.setUint32(24, 16, true);
  return control;
}
function delta() {
  const control = new Uint8Array(68), words = new DataView(control.buffer);
  control.set(new TextEncoder().encode('VINEDELT'));
  words.setUint32(8, 4096, true);
  words.setBigUint64(12, 1n, true);
  words.setBigUint64(20, 2n, true);
  words.setUint32(32, 40, true);
  words.setUint32(36, 1, true);
  words.setUint32(40, 16, true);
  words.setUint32(52, 0x2000, true);
  words.setUint32(56, 1, true);
  words.setUint32(60, 3, true);
  return { control, payload: new Uint8Array(4096).fill(0x5a) };
}
function quadVertices() {
  const bytes = new Uint8Array(112), words = new DataView(bytes.buffer);
  [[0, 0, 0, 0], [2, 0, 1, 0], [0, 1, 0, 1], [2, 1, 1, 1]].forEach(([x, y, u, v], vertex) => {
    const base = vertex * 28;
    for (const [offset, value] of [[0, x], [4, y], [8, 0], [12, 1], [20, u], [24, v]]) {
      words.setFloat32(base + offset, value, true);
    }
    words.setUint32(base + 16, 0xffffffff, true);
  });
  return [...bytes];
}
const frame = { width: 2, height: 1, colorArgb: null, draws: [{
  vertices: quadVertices(), texture: { width: 2, height: 1, fills: [],
    pixels: { offset: 0, length: 8, pitch: 8 } },
}] };
const bgrx = [1, 2, 3, 0, 4, 5, 6, 99];
const rgba = [3, 2, 1, 255, 6, 5, 4, 255];
const entrySentinel = 0x70000001, callbackSentinel = 0x70000002;
const initialContext = { instructionPointer: 0x70000123, eax: 0, esp: 0x1000 };
const completion = { instructionPointer: entrySentinel, stackPointer: 0x1000, eax: 0 };
const scenarios = [
  { name: 'valid-delta', events: ['delta', 'present', 'ack', 'resume'] },
  { name: 'empty-delta', events: ['present', 'resume'] },
  { name: 'truncated-delta', events: [], error: 'host delta control header is truncated' },
  { name: 'from-generation-mismatch', events: [], error: 'Memory generation mismatch after ' },
  { name: 'unpaired-payload', events: [], error: 'Unpaired memory payload after ' },
  { name: 'malformed-later-draw', events: ['delta'], error: 'Invalid presentation pixel range' },
  { name: 'render-failure', events: ['delta', 'present'], error: 'Original render failure' },
  { name: 'ack-failure', events: ['delta', 'present', 'ack'], error: 'Original ACK failure' },
  { name: 'created-thread-owned', events: ['delta', 'ack', 'created', 'present', 'resume'] },
];
const savedGlobals = Object.fromEntries(['self', 'OffscreenCanvas', 'workerDiagnostics'].map(key =>
  [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
let passed = 0;
try {
  for (const scenario of scenarios) {
    const events = [], runtimeMemory = new WebAssembly.Memory({ initial: 1 });
    const cpuMemory = new WebAssembly.Memory({ initial: 1 });
    const runtimeBufferBefore = runtimeMemory.buffer, cpuBufferBefore = cpuMemory.buffer;
    const presentationOffset = 4096;
    new Uint8Array(runtimeMemory.buffer, presentationOffset, bgrx.length).set(bgrx);
    const empty = new Uint8Array(), buffers = delta();
    let control = buffers.control, payload = buffers.payload;
    if (scenario.name === 'empty-delta') control = payload = empty;
    if (scenario.name === 'truncated-delta') control = control.subarray(0, 51);
    if (scenario.name === 'from-generation-mismatch') new DataView(control.buffer).setBigUint64(12, 99n, true);
    if (scenario.name === 'unpaired-payload') { control = empty; payload = Uint8Array.of(1); }
    const owned = scenario.name === 'created-thread-owned';
    const rawPresentation = new Uint8Array(runtimeMemory.buffer, presentationOffset, bgrx.length);
    const presentationFrame = scenario.name === 'malformed-later-draw' ? {
      ...frame, draws: [frame.draws[0], { ...frame.draws[0], texture: {
        ...frame.draws[0].texture, pixels: { ...frame.draws[0].texture.pixels, offset: bgrx.length },
      } }],
    } : frame;
    const output = {
      metadata: { kind: 'com-complete', threadId: 8192, completion, presentation: presentationFrame,
        ...(owned ? { createdThread: { threadId: 8193, entryPoint: 0x401000,
          initialStackPointer: 0x8000, fsBase: 0x9000, returnSentinel: entrySentinel } } : {}) },
      control, payload, presentation: owned ? Uint8Array.from(rawPresentation) : rawPresentation,
      audio: empty, mediaBytes: empty,
    };
    let listener, resolveTerminal, startupAcks = 0, dispatches = 0, executions = 0;
    let resumes = 0, presentations = 0, diagnosticReads = 0, decodedPixels = null;
    let currentContext = { ...initialContext };
    const terminal = new Promise(resolveTerminalValue => { resolveTerminal = resolveTerminalValue; });
    const sha256 = 'a'.repeat(64);
    globalThis.self = {
      addEventListener(type, callback) { assert.equal(type, 'message'); listener = callback; },
      postMessage(message) {
        if (message.type === 'game-exit' || message.type === 'game-error') resolveTerminal(message);
      },
    };
    globalThis.OffscreenCanvas = class OffscreenCanvas {};
    globalThis.workerDiagnostics = {
      present(actualFrame, bytes, decoded) {
        events.push('present'); presentations++;
        assert.deepEqual(actualFrame, frame);
        assert.deepEqual([...bytes], bgrx, 'presentation consumes bytes before any Runtime mutation');
        assert.equal(bytes.buffer === runtimeMemory.buffer, !owned, 'only the selected initial receipt borrows');
        assert.equal(decoded.length, 1);
        assert.deepEqual([...decoded[0]], rgba);
        assert.notEqual(decoded[0].buffer, bytes.buffer, 'decoded renderer pixels own their buffer');
        decodedPixels = decoded[0];
        if (scenario.name === 'render-failure') throw new Error('Original render failure');
      },
      bridge: {
        inspectExecutable: () => ({ sha256, guestImports: [] }),
        prepareExecutable: () => ({ metadata: { maximumPhysicalPages: 1, runtime: { threadId: 8192 },
          entrySentinel, callbackSentinel }, control: reflection(), payload: empty }),
        synchronizeForwardClock() {},
        acknowledgeFast(generation) {
          if (generation === 1n) { startupAcks++; return; }
          assert.equal(generation, 2n);
          events.push('ack');
          new Uint8Array(runtimeMemory.buffer, presentationOffset, bgrx.length).fill(0);
          runtimeMemory.grow(1);
          if (scenario.name === 'ack-failure') throw new Error('Original ACK failure');
        },
        dispatch() { assert.equal(++dispatches, 1); return output; },
        stopContext(instructionPointer) {
          diagnosticReads++;
          assert.equal(instructionPointer, currentContext.instructionPointer);
          // The terminal diagnostic may grow Runtime memory too; no presentation
          // may consume a failed receipt again after this call.
          runtimeMemory.grow(1);
          return { kind: 'stop-context', threadId: 8192, instructionPointer,
            lastError: 87, module: null, symbol: null };
        },
      },
      engine: {
        initialize(reflectionValue) {
          assert.equal(reflectionValue.generation, 1n);
          assert.deepEqual(reflectionValue.pages, []);
        },
        saveCpuContext: () => currentContext,
        executeBudget() {
          assert(++executions <= 2, 'the fixture must stop after its one COM receipt');
          return { stop: executions === 1 ? { kind: 'runtime-dispatch' }
            : { kind: 'trap', trap: 'invalid-opcode' },
          context: currentContext, instructions: 1 };
        },
        collectChangedPageViews: () => [],
        readGuestU32: () => 0x401000,
        applyMemoryDelta(value) {
          events.push('delta');
          assert.equal(value.fromGeneration, 1n); assert.equal(value.generation, 2n);
          assert.deepEqual(value.mappingOperations, []);
          assert.equal(value.pages.length, 1); assert.equal(value.pages[0].address, 0x2000);
          assert.equal(value.pages[0].owner, 1);
          assert(value.pages[0].bytes.every(byte => byte === 0x5a));
          cpuMemory.grow(1);
          new Uint8Array(cpuMemory.buffer, 0x2000, 4096).set(value.pages[0].bytes);
          assert.notEqual(cpuMemory.buffer, runtimeMemory.buffer);
          assert.equal(runtimeMemory.buffer, runtimeBufferBefore, 'CPU growth leaves the Runtime borrow live');
          assert.deepEqual([...rawPresentation], bgrx);
        },
        createCpuContext(created) {
          events.push('created'); assert.equal(created.threadId, 8193);
          assert.equal(created.returnSentinel, entrySentinel);
          return { ...initialContext, instructionPointer: created.entryPoint,
            esp: created.initialStackPointer };
        },
        applyCpuResume(actualCompletion) {
          events.push('resume'); resumes++;
          assert.deepEqual(actualCompletion, completion);
          currentContext = { ...currentContext, instructionPointer: actualCompletion.instructionPointer,
            esp: actualCompletion.stackPointer, eax: actualCompletion.eax };
        },
        applyStoppedCpuResume() { throw new Error('Unexpected imported API completion'); },
      },
    };
    await import(`${workerModule}#${scenario.name}`);
    listener({ data: { type: 'launch-executable', canvas: new OffscreenCanvas(),
      root: { kind: 'directory' }, segments: [], name: 'fixture.exe', sha256, storageFingerprint: 'storage' } });
    let timeout;
    const message = await Promise.race([terminal, new Promise((_, reject) => {
      timeout = setTimeout(() => reject(new Error(`Worker did not stop: ${scenario.name}`)), 2000);
    })]).finally(() => clearTimeout(timeout));
    assert.deepEqual(events, scenario.events, scenario.name);
    assert.equal(startupAcks, 1); assert.equal(dispatches, 1);
    assert.equal(presentations, scenario.events.includes('present') ? 1 : 0);
    assert.equal(resumes, scenario.error ? 0 : 1);
    if (scenario.error) {
      assert.equal(message.type, 'game-error');
      assert(message.detail.startsWith(scenario.error), message.detail);
      assert.equal(diagnosticReads, 1);
    } else {
      assert.equal(message.type, 'game-exit'); assert.equal(message.exitCode, 0);
      assert.equal(diagnosticReads, 0); assert.equal(executions, 2);
    }
    if (scenario.events.includes('delta')) {
      assert.notEqual(cpuMemory.buffer, cpuBufferBefore);
      assert(new Uint8Array(cpuMemory.buffer, 0x2000, 4096).every(byte => byte === 0x5a));
    }
    if (scenario.events.includes('ack')) {
      assert.notEqual(runtimeMemory.buffer, runtimeBufferBefore);
      assert.equal(rawPresentation.byteLength, 0, 'ACK expires the borrowed Runtime view');
    }
    if (decodedPixels !== null) assert.deepEqual([...decodedPixels], rgba, 'later Runtime growth cannot change owned pixels');
    if (owned) assert.deepEqual([...output.presentation], bgrx, 'created-thread fallback remains owned after ACK');
    passed++;
  }
} finally {
  for (const [key, descriptor] of Object.entries(savedGlobals)) {
    if (descriptor === undefined) delete globalThis[key];
    else Object.defineProperty(globalThis, key, descriptor);
  }
}
console.log(JSON.stringify({ passed: true, scenarios: passed,
  workerSource, productionExportsAdded: false }));
