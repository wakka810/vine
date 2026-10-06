import assert from "node:assert/strict";
import { readFile, mkdir } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

// Exercise the actual worker error boundary without a browser or a guest fixture.
// Dependencies supply only the resources needed to reach that boundary.
const workerDependencies = {
  "../storage/source-directory.js": "export const gameStorageFingerprint = async () => 'storage';",
  "./execution-engine.js": `export const instantiateExecutionEngine = async () => {
    if (globalThis.workerDiagnostics.engineLoadFailure) throw new Error('Original CPU load failure');
    return globalThis.workerDiagnostics.engine;
  };`,
  "./gdi-text-raster.js": "export const rasterizeGdiText = () => { throw new Error('Unexpected raster request'); };",
  "./game-surface.js": "export class GameSurface {}",
  "./memory-transport.js": "export const decodeReflection = () => ({ generation: 1n }); export const decodeHostDelta = () => { throw new Error('Unexpected delta'); };",
  "./runtime-bridge.js": "export const RuntimeBridge = { instantiate: async () => globalThis.workerDiagnostics.bridge };",
  "./media-decoder.js": "export class MediaDecoder {} export class UnsupportedMediaFormat extends Error {}",
  "./source-explorer.js": `export const openSourceExecutable = async () => ({ size: 1, lastModified: 0, arrayBuffer: async () => new Uint8Array([0]).buffer });
    export const browseSourceDirectory = async () => []; export const inspectSourcePath = async () => null;
    export const listSourceDirectory = async () => []; export const openSourceDll = async () => null;
    export const readSourceRange = async () => new Uint8Array();`,
  "./writable-tree.js": "export const openWritableTree = async () => ({});",
};
const workerBundle = await build({
  entryPoints: ["browser/runtime-worker/game-worker.ts"], bundle: true, write: false,
  platform: "node", format: "esm",
  define: { "import.meta.url": JSON.stringify(pathToFileURL(`${process.cwd()}/browser/runtime-worker/game-worker.ts`).href) },
  plugins: [{ name: "worker-error-resources", setup(plugin) {
    plugin.onResolve({ filter: /^\./ }, args => args.path in workerDependencies
      ? { path: args.path, namespace: "worker-error-resource" } : undefined);
    plugin.onLoad({ filter: /.*/, namespace: "worker-error-resource" }, args => ({
      contents: workerDependencies[args.path], loader: "js",
    }));
  } }],
});
const workerModule = `data:text/javascript;base64,${Buffer.from(workerBundle.outputFiles[0].contents).toString("base64")}`;
const previousGlobals = Object.fromEntries(["self", "OffscreenCanvas", "workerDiagnostics"].map(key =>
  [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
try {
  for (const failure of ["dispatch", "diagnostic", "cpu-context", "engine-load", "engine-initialize", "prelaunch"]) {
    let listener, dispatches = 0, diagnosticReads = 0;
    const cpuContext = { instructionPointer: 0x70000123, eax: 0x80004005, esp: 0x1000 };
    const sha256 = "a".repeat(64);
    let resolveError;
    const errorMessage = new Promise(resolve => { resolveError = resolve; });
    globalThis.self = {
      addEventListener(type, callback) { assert.equal(type, "message"); listener = callback; },
      postMessage(message) { if (message.type === "game-error") resolveError(message); },
    };
    globalThis.OffscreenCanvas = class OffscreenCanvas {};
    globalThis.workerDiagnostics = {
      engineLoadFailure: failure === "engine-load",
      bridge: {
        inspectExecutable: () => ({ sha256, guestImports: [] }),
        prepareExecutable: () => ({ metadata: { maximumPhysicalPages: 1, runtime: { threadId: 8192 } },
          control: new Uint8Array(), payload: new Uint8Array() }),
        acknowledgeFast() {}, synchronizeForwardClock() {},
        dispatch() {
          if (++dispatches === 22) throw new Error("Original unsupported operation");
          return { metadata: { kind: "api-complete", completion: { eax: 0 } },
            control: new Uint8Array(), payload: new Uint8Array() };
        },
        stopContext(instructionPointer) {
          diagnosticReads++;
          assert.equal(instructionPointer, ["cpu-context", "engine-load", "engine-initialize"].includes(failure)
            ? 0 : cpuContext.instructionPointer);
          if (failure === "diagnostic") throw new Error("Secondary diagnostic failure");
          return { kind: "stop-context", threadId: 9000, instructionPointer, lastError: 87,
            module: "kernel32.dll", symbol: "Example" + "x".repeat(200) };
        },
      },
      engine: {
        initialize() {
          if (failure === "engine-initialize") throw new Error("Original CPU initialize failure");
        }, saveCpuContext() {
          if (failure === "cpu-context" && dispatches === 22) throw new Error("Secondary CPU context failure");
          return cpuContext;
        },
        executeBudget: () => ({ stop: { kind: "runtime-dispatch" }, context: cpuContext, instructions: 1 }),
        collectChangedPageViews: () => [], readGuestU32: () => 0x401000, applyCpuResume() {},
      },
    };
    await import(`${workerModule}#${failure}`);
    listener({ data: { type: "launch-executable", canvas: failure === "prelaunch" ? {} : new OffscreenCanvas(),
      root: { kind: "directory" }, segments: [], name: "fixture.exe", sha256, storageFingerprint: "storage" } });
    let timeout;
    const message = await Promise.race([errorMessage,
      new Promise((_, reject) => { timeout = setTimeout(() => reject(new Error("Worker error was not reported")), 2000); }),
    ]).finally(() => clearTimeout(timeout));
    const originalFault = failure === "prelaunch" ? "No game surface was transferred" :
      failure === "engine-load" ? "Original CPU load failure" :
      failure === "engine-initialize" ? "Original CPU initialize failure" :
      "guest trap 0x70000123 stopped: Original unsupported operation";
    assert(message.detail.startsWith(originalFault), message.detail);
    assert(!message.detail.includes("Secondary"), "diagnostics must preserve the original fault");
    const [faultAndContext, recent] = message.detail.split("\nRecent events:\n");
    const context = faultAndContext.split("\n").at(-1);
    const recentLines = recent.split("\n");
    assert(recentLines.length <= 16, "the UI receives only a bounded recent history");
    assert(recentLines.every(line => line.length <= 200), "each displayed event is short");
    assert(context.length <= 512, "the displayed context has bounded text fields");
    assert(recentLines.at(-1).includes("worker-error"));
    if (failure === "prelaunch") {
      assert.equal(diagnosticReads, 0);
      assert(context.includes("thread=unknown (unknown)") && context.includes("EIP=unknown"));
      continue;
    }
    if (["cpu-context", "engine-load", "engine-initialize"].includes(failure)) {
      assert.equal(diagnosticReads, 1);
      assert(context.includes("thread=9000 (runtime)"), "prepared Runtime thread survives a missing CPU context");
      assert(context.includes("LastError=87"));
      assert(context.includes("EIP=unknown") && context.includes("EAX=unknown"));
      assert(context.includes("module=unknown") && context.includes("symbol=unknown"),
        "a thread-only lookup must not manufacture an EIP or module");
      if (failure !== "cpu-context") continue;
    }
    assert.equal(recentLines.length, 16);
    assert(recentLines.some(line => line.includes("dispatch: tid=8192, eip=0x70000123")));
    assert(recentLines.some(line => line.includes("completion: tid=8192, eip=0x70000123, api-complete, eax=0x00000000")));
    if (failure === "dispatch") {
      assert.equal(diagnosticReads, 1, "the Runtime snapshot is read only at the stop boundary");
      assert(context.includes("thread=9000 (runtime)"), "Rust's current thread takes precedence over the execution adapter");
      assert(context.includes("LastError=87") && context.includes("module=kernel32.dll"));
      assert(context.match(/symbol=(.*), LastError=/u)[1].length <= 96, "displayed diagnostic text is bounded");
      assert(context.includes("EIP=0x70000123") && context.includes("EAX=0x80004005"));
    } else if (failure === "diagnostic") {
      assert(context.includes("thread=8192 (execution-adapter)"));
      assert(context.includes("LastError=unknown"));
      assert(context.includes("module=unknown") && context.includes("symbol=unknown"));
      assert.equal(diagnosticReads, 1);
    }
  }
} finally {
  for (const [key, descriptor] of Object.entries(previousGlobals)) {
    if (descriptor === undefined) delete globalThis[key];
    else Object.defineProperty(globalThis, key, descriptor);
  }
}
console.log("worker stops preserve the original fault with bounded diagnostics and safe unknown fields");
if (process.argv.includes("--worker-diagnostics")) process.exit(0);

await mkdir(".work/generic-bootstrap", { recursive: true });
const outfile = ".work/generic-bootstrap/driver.mjs";
await build({
  stdin: {
    contents: `export { RuntimeBridge } from "./browser/runtime-worker/runtime-bridge.ts";
      export { decodeReflection } from "./browser/runtime-worker/memory-transport.ts";
      export { instantiateExecutionEngineBytes } from "./browser/runtime-worker/execution-engine.ts";`,
    resolveDir: process.cwd(),
    loader: "ts",
  },
  bundle: true,
  format: "esm",
  platform: "node",
  target: "node24",
  outfile,
});
const { RuntimeBridge, decodeReflection, instantiateExecutionEngineBytes } = await import(
  `${pathToFileURL(`${process.cwd()}/${outfile}`).href}?v=${Date.now()}`
);
const wasm = await WebAssembly.instantiate(await readFile(
  "target/wasm32-unknown-unknown/release/vine_runtime_wasm.wasm",
));
const bridge = RuntimeBridge.fromExports(wasm.instance.exports);
const prepared = bridge.prepareExecutable(
  await readFile(".work/toolchain-spike/bootstrap-fs.exe"), "folder/日本語.exe",
);
const cpuBytes = await readFile(".work/vine-v86/build/v86-fallback.wasm");
await assert.rejects(instantiateExecutionEngineBytes(cpuBytes.subarray(0, 16),
  { maximumPhysicalPages: prepared.metadata.maximumPhysicalPages }), WebAssembly.CompileError);
await assert.rejects(instantiateExecutionEngineBytes(cpuBytes),
  /v86 mirror requires the runtime physical page limit/u);
await assert.rejects(instantiateExecutionEngineBytes(cpuBytes, { maximumPhysicalPages: 0 }),
  /maximum physical page count must be a positive integer/u);
const engine = await instantiateExecutionEngineBytes(cpuBytes,
  { maximumPhysicalPages: prepared.metadata.maximumPhysicalPages });
engine.initialize(decodeReflection(prepared.control, prepared.payload), prepared.metadata);
const initialContext = engine.saveCpuContext();
for (const invalid of [
  { ...initialContext, byteLength: initialContext.byteLength - 4 },
  { ...initialContext, opaqueWords: new Uint32Array(initialContext.opaqueWords.length - 1) },
]) {
  assert.throws(() => engine.restoreCpuContext(invalid), /CPU context ABI mismatch/u);
  assert.deepEqual(engine.saveCpuContext(), initialContext, "rejected context must preserve CPU state");
}
for (const index of [0, 1]) {
  const opaqueWords = Uint32Array.from(initialContext.opaqueWords);
  opaqueWords[index] = 0;
  assert.throws(() => engine.restoreCpuContext({ ...initialContext, opaqueWords }),
    /v86 rejected a resumed CPU state/u);
  assert.deepEqual(engine.saveCpuContext(), initialContext, "invalid header must preserve CPU state");
}
let stopped = false;
for (let sliceIndex = 0; sliceIndex < 100; sliceIndex += 1) {
  const slice = engine.executeBudget(4096);
  if (slice.stop.kind === "budget-yield") continue;
  assert.equal(slice.stop.kind, "trap", JSON.stringify(slice.stop));
  assert.equal(slice.stop.trap, "invalid-opcode");
  assert.equal(slice.context.instructionPointer, prepared.metadata.entrySentinel);
  assert.equal(slice.context.eax, 0, "guest verified FS/TEB/PEB and returned success");
  stopped = true;
  break;
}
assert(stopped, "guest did not return within 409600 instructions");
console.log("selected PE32 executed through the pinned v86 CPU and returned 0");
