import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import vm from "node:vm";

let Processor;
const messages = [];
const context = vm.createContext({ ArrayBuffer, DataView, Float32Array, Map, Math,
  sampleRate: 44100,
  AudioWorkletProcessor: class { port = { postMessage: message => messages.push(structuredClone(message)) }; },
  registerProcessor(name, type) { assert.equal(name, "vine-pcm"); Processor = type; },
});
vm.runInContext(await readFile("browser/runtime-worker/pcm-worklet.js", "utf8"), context);
const processor = new Processor();

function command(operation, id, pcm = new Uint8Array(0), options = {}) {
  const bytes = new ArrayBuffer(48 + pcm.length), view = new DataView(bytes);
  const words = [operation, Number(id), 0, options.channels ?? 1, 44100, options.bits ?? 16,
    options.volume ?? 0, options.pan ?? 0, options.frequency ?? 44100,
    options.position ?? 0, options.looping ?? 0, pcm.length];
  words.forEach((word, index) => view.setUint32(index * 4, word, true));
  new Uint8Array(bytes, 48).set(pcm);
  return bytes;
}
function render(frames = 4, target = processor) {
  const left = new Float32Array(frames), right = new Float32Array(frames);
  target.process([], [[left, right]]);
  return [left, right];
}
function closeEnough(actual, expected) {
  assert.equal(actual.length, expected.length);
  actual.forEach((value, index) => assert.ok(Math.abs(value - expected[index]) < 1e-5));
}
const pcm16 = new Uint8Array(8);
const samples = new DataView(pcm16.buffer);
[-32768, 0, 16384, 32767].forEach((sample, index) => samples.setInt16(index * 2, sample, true));
processor.port.onmessage({ data: { type: "command", requestId: "1", bytes: command(1, 1, pcm16) } });
assert.deepEqual(messages.pop(), { type: "ack", requestId: "1", accepted: true });
const [left, right] = render();
closeEnough(left, [-1, 0, 0.5, 32767 / 32768]);
closeEnough(right, Array.from(left));
render();
assert.deepEqual(messages.pop(), { type: "ended", resourceId: "1" });
render();
assert.equal(messages.length, 0, "one-shot end is delivered once");

// Validation and conversion precede publication, so a rejected command keeps
// the exact live resource and cursor, including a pending one-shot end.
const previous = processor.voices.get("1");
processor.port.onmessage({ data: { type: "command", requestId: "2", bytes: command(2, 1, pcm16, { channels: 2 }) } });
assert.deepEqual(messages.pop(), { type: "ack", requestId: "2", accepted: false });
assert.equal(processor.voices.get("1"), previous);
assert.throws(() => processor.command(new ArrayBuffer(48 + 16 * 1024 * 1024 + 1)));
assert.equal(processor.voices.get("1"), previous);

// A seek changes the real resource token. An older end observation cannot
// retire or stop the newly positioned resource.
const oldId = new Uint8Array(8); new DataView(oldId.buffer).setBigUint64(0, 1n, true);
processor.command(command(4, 3, oldId, { position: 4 }));
assert.equal(processor.voices.has("1"), false);
assert.equal(processor.voices.has("3"), true);
processor.port.onmessage({ data: { type: "retire", resourceId: "1" } });
closeEnough(render(1)[0], [0.5]);
processor.command(command(5, 3));
assert.equal(processor.voices.size, 0);
const media = (requestId, operation, bytes, volume = 0) => {
  processor.port.onmessage({data:{type:"media-command", requestId, resourceId:"1", operation, volume, bytes}});
};
media("100", "create");
assert.deepEqual(messages.pop(), {type:"media-ack", requestId:"100", accepted:true});
const decoded = new Float32Array(96);
for (let i=0;i<48;i++) { decoded[i*2]=0.25; decoded[i*2+1]=-0.5; }
media("101", "sample", decoded.buffer);
assert.deepEqual(messages.pop(), {type:"media-ack", requestId:"101", accepted:true});
closeEnough(render()[0], [0,0,0,0]);
const voice = processor.media.get("1");
assert.equal(voice.cursor, 0, "paused media does not consume decoded samples");
media("102", "drain");
assert.equal(messages.length, 0, "queue acceptance is not an actual drain");
media("103", "sample", new Float32Array([NaN, 1]).buffer);
assert.deepEqual(messages.pop(), {type:"media-ack", requestId:"103", accepted:false});
assert.equal(voice.bytes, 384, "failed PCM admission preserves the queue");
media("104", "run");
assert.deepEqual(messages.pop(), {type:"media-ack", requestId:"104", accepted:true});
const played = render(64);
assert.equal(played[0][0], 0.25); assert.equal(played[1][0], -0.5);
assert.equal(played[0].at(-1), 0);
assert.deepEqual(messages.pop(), {type:"media-ack", requestId:"102", accepted:true});
assert.equal(voice.bytes, 0);
media("105", "sample", decoded.buffer);
media("106", "pause");
messages.length=0;
closeEnough(render()[0], [0,0,0,0]);
assert.equal(voice.bytes, 384);
media("107", "drain");
media("108", "stop");
assert.deepEqual(messages.splice(0), [
  {type:"media-ack", requestId:"107", accepted:false}, {type:"media-ack", requestId:"108", accepted:true},
]);
assert.equal(voice.bytes, 0);
media("109", "volume", undefined, -2000);
media("110", "sample", decoded.buffer);
media("111", "run");
messages.length=0;
closeEnough(render(1)[0], [0.025]);
media("112", "release");
assert.equal(processor.media.size, 0);
messages.length=0;
closeEnough(render()[0], [0, 0, 0, 0]);

// Unsigned PCM8, looping, frequency, pan and volume drive actual output samples.
processor.command(command(1, 4, new Uint8Array([0, 128, 255, 128]), {
  bits: 8, frequency: 22050, volume: -2000, pan: 10000, looping: 1,
}));
const [quietLeft, loudRight] = render();
closeEnough(loudRight, [-0.1, -0.05, 0, 0.1 * 127 / 256]);
closeEnough(quietLeft, Array.from(loudRight, value => value * 1e-5));
const retired = new Uint8Array(8); new DataView(retired.buffer).setBigUint64(0, 4n, true);
processor.command(command(6, 0, retired));
assert.equal(processor.voices.size, 0);

// A streaming replacement changes samples at the live playback position. The
// Rust notification position in the packet is not a request to seek backwards.
const streaming = new Processor();
function signedPcm(values) {
  const bytes = new Uint8Array(values.length * 2), view = new DataView(bytes.buffer);
  values.forEach((value, index) => view.setInt16(index * 2, value, true));
  return bytes;
}
const ramp = signedPcm([4096, 8192, 12288, 16384, 20480, 24576, 28672, -28672]);
const updatedRamp = signedPcm([-4096, -8192, -12288, -16384, -20480, -24576, -28672, 28672]);
streaming.command(command(1, 20, ramp, { looping: 1 }));
closeEnough(render(2, streaming)[0], [0.125, 0.25]);
streaming.command(command(2, 20, updatedRamp, { position: 0, looping: 1 }));
closeEnough(render(2, streaming)[0], [-0.375, -0.5]);
streaming.command(command(3, 20, undefined, {
  frequency: 22050, volume: -2000, position: 0, looping: 1,
}));
closeEnough(render(3, streaming)[0], [-0.0625, -0.06875, -0.075]);
const beforeBadEncoding = streaming.voices.get("20");
assert.throws(() => streaming.command(command(2, 20, new Uint8Array(8), { bits: 8, looping: 1 })), /layout/);
assert.throws(() => streaming.command(command(3, 20, undefined, { bits: 8, looping: 1 })), /layout/);
assert.equal(streaming.voices.get("20"), beforeBadEncoding);
// Rejection retains the fractional playback position and the existing controls.
closeEnough(render(1, streaming)[0], [-0.08125]);

const previousStreamingId = new Uint8Array(8);
new DataView(previousStreamingId.buffer).setBigUint64(0, 20n, true);
streaming.command(command(4, 21, previousStreamingId, {
  position: 14, frequency: 22050, looping: 1,
}));
streaming.port.onmessage({ data: { type: "retire", resourceId: "20" } });
assert.equal(streaming.voices.has("20"), false);
closeEnough(render(3, streaming)[0], [0.875, 0.375, -0.125]);
streaming.command(command(5, 21));

// Unsigned PCM follows the same update and interpolation rules without a
// whole-buffer conversion during command admission.
streaming.command(command(1, 22, new Uint8Array([0, 64, 128, 192]), { bits: 8, looping: 1 }));
closeEnough(render(1, streaming)[0], [-1]);
streaming.command(command(2, 22, new Uint8Array([128, 192, 0, 64]), { bits: 8, looping: 1 }));
closeEnough(render(3, streaming)[0], [0.5, -1, -0.5]);
streaming.command(command(3, 22, undefined, { bits: 8, frequency: 22050, looping: 1 }));
closeEnough(render(3, streaming)[0], [0, 0.25, 0.5]);
streaming.command(command(5, 22));

const stereo = signedPcm([4096, -8192, 12288, -16384, 20480, -24576]);
streaming.command(command(1, 23, stereo, { channels: 2 }));
const stereoOutput = render(3, streaming);
closeEnough(stereoOutput[0], [0.125, 0.375, 0.625]);
closeEnough(stereoOutput[1], [-0.25, -0.5, -0.75]);
console.log("PCM worklet: real samples, streaming updates, controls, seek identity, one-shot end and atomic rejection passed");
