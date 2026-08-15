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

const wrapped = new Float32Array(3);
assert.equal(processor.process([], [[wrapped]]), true);
assert.deepEqual([...wrapped], [6, 7, 0]);

delete globalThis.registerProcessor;
delete globalThis.AudioWorkletProcessor;
delete globalThis.sampleRate;

console.log("DirectSound worklet incremental ring-buffer contract passed");
