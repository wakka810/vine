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
