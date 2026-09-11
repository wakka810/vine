import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
let Processor = null;

class FakePort {
  onmessage = null;
  messages = [];

  postMessage(message) {
    this.messages.push(message);
  }
}

globalThis.sampleRate = 48_000;
globalThis.AudioWorkletProcessor = class {
  constructor() {
    this.port = new FakePort();
  }
};
globalThis.registerProcessor = (name, implementation) => {
  assert.equal(name, "vine-direct-sound-stream");
  Processor = implementation;
};

await import(
  `${pathToFileURL(path.join(root, "browser", "runtime-worker", "direct-sound-worklet.js")).href}?v=${Date.now()}`
);
assert.notEqual(Processor, null, "DirectSound worklet did not register");

const processor = new Processor();
const initial = Float32Array.from([0, 1, 2, 3, 4, 5, 6, 7]);
processor.receive({
  type: "start",
  channels: [initial.buffer],
  frameCount: initial.length,
  regionOffsetFrames: 0,
  frequencyHz: 48_000,
  playback: "playing-looping",
  startFrame: 0,
});

const first = new Float32Array(3);
assert.equal(processor.process([], [[first]]), true);
assert.deepEqual([...first], [0, 1, 2]);

const replacement = Float32Array.from([40, 50]);
processor.receive({
  type: "update",
  channels: [replacement.buffer],
  frameCount: initial.length,
  regionOffsetFrames: 4,
  frequencyHz: 48_000,
  playback: "playing-looping",
  startFrame: 0,
});

const afterUpdate = new Float32Array(3);
assert.equal(processor.process([], [[afterUpdate]]), true);
assert.deepEqual(
  [...afterUpdate],
  [3, 40, 50],
  "incremental ring-buffer update rewound playback or replaced untouched frames",
);

const endOfInitialLap = new Float32Array(2);
assert.equal(processor.process([], [[endOfInitialLap]]), true);
assert.deepEqual([...endOfInitialLap], [6, 7]);

const staleReplay = new Float32Array(3);
assert.equal(processor.process([], [[staleReplay]]), true);
assert.deepEqual(
  [...staleReplay],
  [0, 0, 0],
  "streaming buffer replayed stale frames after the producer fell behind",
);
assert.deepEqual(processor.port.messages.at(-1), {
  type: "stream-underrun",
  staleReplayPreventedFrames: 3,
});

const resumed = Float32Array.from([80, 90]);
processor.receive({
  type: "update",
  channels: [resumed.buffer],
  frameCount: initial.length,
  regionOffsetFrames: 0,
  frequencyHz: 48_000,
  playback: "playing-looping",
  startFrame: 0,
});
const afterResume = new Float32Array(3);
assert.equal(processor.process([], [[afterResume]]), true);
assert.deepEqual(
  [...afterResume],
  [80, 90, 0],
  "streaming buffer did not resume at the newly supplied region",
);

const staticProcessor = new Processor();
const staticLoop = Float32Array.from([10, 20, 30]);
staticProcessor.receive({
  type: "start",
  channels: [staticLoop.buffer],
  frameCount: staticLoop.length,
  regionOffsetFrames: 0,
  frequencyHz: 48_000,
  playback: "playing-looping",
  startFrame: 0,
});
const staticOutput = new Float32Array(5);
assert.equal(staticProcessor.process([], [[staticOutput]]), true);
assert.deepEqual(
  [...staticOutput],
  [10, 20, 30, 10, 20],
  "static looping buffer stopped repeating without incremental updates",
);

delete globalThis.registerProcessor;
delete globalThis.AudioWorkletProcessor;
delete globalThis.sampleRate;

console.log("DirectSound worklet streaming underrun contract passed");

// Session end must retire active nodes and plays still awaiting worklet readiness.
const { build } = await import("esbuild");
const rendererModule = path.join(root, ".work", "direct-sound-test", "audio-renderer.mjs");
await build({entryPoints: [path.join(root, "browser", "runtime-worker", "audio-renderer.ts")],
  bundle: true, platform: "node", format: "esm", outfile: rendererModule});
const { BrowserPcmRenderer } = await import(pathToFileURL(rendererModule));
let releaseModule;
const moduleReady = new Promise(resolve => { releaseModule = resolve; });
globalThis.AudioContext = class {
  state = "running";
  currentTime = 0;
  destination = {};
  audioWorklet = { addModule: () => moduleReady };
};
const audioNodes = [];
class FakeAudioNode {
  disconnected = false;
  constructor() { audioNodes.push(this); }
  connect(other) { return other; }
  disconnect() { this.disconnected = true; }
}
globalThis.GainNode = class extends FakeAudioNode { gain = { setValueAtTime() {} }; };
globalThis.StereoPannerNode = class extends FakeAudioNode { pan = { setValueAtTime() {} }; };
globalThis.AudioWorkletNode = class extends FakeAudioNode {
  port = { messages: [], closed: false, addEventListener() {}, start() {},
    postMessage(message) { this.messages.push(message); }, close() { this.closed = true; } };
};
const pcm = new BrowserPcmRenderer();
const message = {bufferId: 1, bufferGeneration: 1, contentGeneration: 1,
  format: {channels: 1, samplesPerSecond: 48000, bitsPerSample: 16, blockAlign: 2, averageBytesPerSecond: 96000},
  bufferByteLength: 8, playback: "playing-looping",
  controls: {volumeHundredthsDb: 0, panHundredthsDb: 0, currentPositionBytes: 0},
  sampleUpdate: {kind: "snapshot", samples: new Int16Array([1, 2, 3, 4]).buffer}};
const pendingPlay = pcm.play(message);
await Promise.resolve();
pcm.stopAll();
releaseModule();
assert.equal((await pendingPlay).staleMessageIgnored, true);
assert.equal(audioNodes.length, 0, "queued play resurrected after session end");
assert.equal((await pcm.play(message)).staleMessageIgnored, false, "new session did not restart");
assert.equal(audioNodes.length, 3);
const activeWorklet = audioNodes.find(node => "port" in node);
pcm.stopAll();
assert(audioNodes.every(node => node.disconnected));
assert(activeWorklet.port.closed);
assert.equal(activeWorklet.port.messages.at(-1).type, "stop");
pcm.stopAll();
for (const name of ["AudioContext", "GainNode", "StereoPannerNode", "AudioWorkletNode"]) delete globalThis[name];
console.log("Browser PCM session cleanup and queued-play race passed");
