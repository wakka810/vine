import assert from "node:assert/strict";
import { build } from "esbuild";

const contexts = [], nodes = [];
globalThis.AudioContext = class {
  state = "suspended";
  destination = {};
  audioWorklet = { addModule: async url => { assert.match(url.pathname, /pcm-worklet\.js$/u); } };
  constructor() { contexts.push(this); }
  async resume() { this.state = "running"; }
  async close() { this.state = "closed"; this.onstatechange?.(); }
};
globalThis.AudioWorkletNode = class {
  messages = [];
  port = { postMessage: message => this.messages.push(message), close: () => { this.closed = true; } };
  constructor(context, name, options) {
    assert.equal(name, "vine-pcm"); assert.deepEqual(options.outputChannelCount, [2]);
    this.context = context; nodes.push(this);
  }
  connect(destination) { assert.equal(destination, this.context.destination); this.connected = true; }
  disconnect() { this.connected = false; }
};
const { outputFiles } = await build({ entryPoints: ["browser/runtime-worker/browser-audio.ts"],
  bundle: true, write: false, platform: "node", format: "esm" });
// Keep the real module URL resolution while evaluating the TypeScript bundle.
const source = new TextDecoder().decode(outputFiles[0].contents).replaceAll("import.meta.url",
  JSON.stringify(new URL("../browser/runtime-worker/browser-audio.ts", import.meta.url).href));
const { BrowserAudio } = await import(`data:text/javascript;base64,${Buffer.from(source).toString("base64")}`);
const failures = [], ends = [];
let audio = await BrowserAudio.create(id => ends.push(id), error => failures.push(error.message));
const first = nodes.at(-1);
assert.equal(first.connected, true);
const mediaPending = audio.submitMedia("10", "9007199254740993", "create", 0);
first.port.onmessage({data:{type:"media-ack", requestId:"11", accepted:true}});
assert.throws(() => audio.submitMedia("10", "9007199254740993", "run", 0), /pending/);
first.port.onmessage({data:{type:"media-ack", requestId:"10", accepted:true}});
assert.equal(await mediaPending, true);
assert.throws(() => audio.submitMedia("12", "1", "sample", 0, new ArrayBuffer(4)), /bounded/);
let pending = audio.submit("1", new ArrayBuffer(48));
first.port.onmessage({ data: { type: "ack", requestId: "2", accepted: true } });
assert.throws(() => audio.submit("3", new ArrayBuffer(48)), /pending/);
first.port.onmessage({ data: { type: "ack", requestId: "1", accepted: true } });
assert.equal(await pending, true);
first.port.onmessage({ data: { type: "ended", resourceId: "1" } });
assert.deepEqual(ends, ["1"]);
audio.retire("1");
assert.deepEqual(first.messages.at(-1), { type: "retire", resourceId: "1" });
audio.close();
assert.equal(first.connected, false);
assert.equal(first.closed, true);
assert.deepEqual(failures, [], "ordinary teardown is not a backend fault");

audio = await BrowserAudio.create(() => {}, error => failures.push(error.message));
nodes.at(-1).onprocessorerror();
assert.equal(failures.length, 1, "failure without a pending call is reported");
assert.equal(await audio.submit("4", new ArrayBuffer(48)), false);
audio.close();
assert.equal(failures.length, 1);

const order = [];
audio = await BrowserAudio.create(() => {}, () => order.push("fatal"));
pending = audio.submit("5", new ArrayBuffer(48)).then(accepted => { order.push("ack"); return accepted; });
const mediaFailed = audio.submitMedia("13", "1", "drain", 0);
contexts.at(-1).state = "suspended";
contexts.at(-1).onstatechange();
assert.equal(await pending, false);
assert.equal(await mediaFailed, false, "backend teardown rejects its pending media drain");
await Promise.resolve();
assert.deepEqual(order, ["fatal", "ack"], "the worker learns the fatal failure before a false ACK can resume guest instructions");
assert.equal(nodes.at(-1).connected, false);
console.log("Browser audio: connected resource, ACK identity, ordinary teardown and actual backend failures passed");
