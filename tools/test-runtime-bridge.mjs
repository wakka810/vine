import assert from "node:assert/strict";
import { build } from "esbuild";

const { outputFiles } = await build({
  entryPoints: ["browser/runtime-worker/runtime-bridge.ts"],
  bundle: true, write: false, platform: "node", format: "esm",
});
const { RuntimeBridge } = await import(
  `data:text/javascript;base64,${Buffer.from(outputFiles[0].contents).toString("base64")}`
);
const transport = await build({
  entryPoints: ["browser/runtime-worker/memory-transport.ts"],
  bundle: true, write: false, platform: "node", format: "esm",
});
const { decodeReflection, decodeHostDelta, encodeCpuWritebackControl } = await import(
  `data:text/javascript;base64,${Buffer.from(transport.outputFiles[0].contents).toString("base64")}`
);

for (const [magic, headerBytes, decode] of [
  ["VINEREFL", 36, decodeReflection],
  ["VINEDELT", 52, decodeHostDelta],
]) {
  const control = new Uint8Array(headerBytes);
  const view = new DataView(control.buffer);
  control.set(new TextEncoder().encode(magic));
  view.setUint32(8, 4096, true);
  if (magic === "VINEREFL") {
    view.setUint32(24, 16, true);
  } else {
    view.setUint32(32, 40, true);
    view.setUint32(40, 16, true);
  }
  assert.deepEqual(decode(control, new Uint8Array(0)).pages, []);
  assert.throws(() => decode(control.subarray(0, headerBytes - 1), new Uint8Array(0)), /truncated/);
  control.set(new TextEncoder().encode(magic === "VINEREFL" ? "VINERF01" : "VINEDL01"));
  assert.throws(() => decode(control, new Uint8Array(0)), /expected VINE/);
}
const emptyWriteback = encodeCpuWritebackControl(7n, []);
assert.equal(emptyWriteback.byteLength, 28);
assert.equal(new DataView(emptyWriteback.buffer).getBigUint64(12, true), 7n);

const memory = new WebAssembly.Memory({ initial: 2 });
let metadata = new TextEncoder().encode('{"kind":"ok"}');
new Uint8Array(memory.buffer, 0, metadata.length).set(metadata);
let metadataPointer = 0;
let controlPointer = 128;
let controlLength = 4;
let inputPointer = 4096;
let resizes = 0;
let grow = false;
let keyboardCalls = 0;
let pointerCalls = 0;
let lastPointerWords;
let pointerIgnored = false;
let presentationPointer = 512;
let presentationLength = 4;
let lastKeyboardWords;
let mediaPointer = 768, mediaLength = 4, mediaReadCalls = 0, mediaCompleteCalls = 0;
let lastMediaRead, lastMediaCompletion;
const exports = {
  memory,
  vine_bridge_input_resize() {
    resizes++;
    if (grow) memory.grow(1);
    return 0;
  },
  vine_bridge_input_ptr: () => inputPointer,
  vine_bridge_metadata_ptr: () => metadataPointer,
  vine_bridge_metadata_len: () => metadata.length,
  vine_bridge_control_ptr: () => controlPointer,
  vine_bridge_control_len: () => controlLength,
  vine_bridge_payload_ptr: () => 256,
  vine_bridge_payload_len: () => 2,
  vine_bridge_presentation_ptr: () => presentationPointer,
  vine_bridge_presentation_len: () => presentationLength,
  vine_bridge_audio_ptr: () => 0,
  vine_bridge_audio_len: () => 0,
  vine_bridge_media_ptr: () => mediaPointer,
  vine_bridge_media_len: () => mediaLength,
  vine_bridge_read_com_media(...words) { mediaReadCalls++; lastMediaRead = words; return 0; },
  vine_bridge_complete_com_media(low, high, metadataLength, payloadLength) {
    mediaCompleteCalls++;
    const input = new Uint8Array(memory.buffer, inputPointer, metadataLength + payloadLength);
    lastMediaCompletion = { low, high, metadata: JSON.parse(new TextDecoder().decode(input.subarray(0, metadataLength))),
      payload: Array.from(input.subarray(metadataLength)) };
    return 0;
  },
  vine_bridge_schedule_next_thread: () => 0,
  vine_bridge_ingest_keyboard_event(low, high, kind, key, repeat) {
    keyboardCalls++;
    lastKeyboardWords = [low, high, kind, key, repeat];
    metadata = new TextEncoder().encode(`{"kind":"keyboard-event-ingested","sequence":${low}}`);
    new Uint8Array(memory.buffer, 0, metadata.length).set(metadata);
    return 0;
  },
  vine_bridge_ingest_pointer_event(low, high, kind, button, x, y) {
    pointerCalls++;
    lastPointerWords = [low, high, kind, button, x, y];
    metadata = new TextEncoder().encode(`{"kind":"${pointerIgnored ? 'pointer-event-ignored' : 'pointer-event-ingested'}","sequence":${low}}`);
    new Uint8Array(memory.buffer, 0, metadata.length).set(metadata);
    return 0;
  },
  vine_bridge_inspect_executable: () => 0,
  vine_bridge_reconcile_writeback: () => 0,
  vine_bridge_writeback_generation_low: () => 9,
  vine_bridge_writeback_generation_high: () => 0,
};
const bridge = RuntimeBridge.fromExports(exports);
new Uint8Array(memory.buffer, 128, 4).set([1, 2, 3, 4]);
new Uint8Array(memory.buffer, 512, 4).set([3, 2, 1, 0]);
new Uint8Array(memory.buffer, 768, 4).set([9, 8, 7, 6]);
const output = bridge.scheduleNextThread();
assert.deepEqual(output.metadata, { kind: "ok" });
assert.deepEqual([...output.control], [1, 2, 3, 4]);
new Uint8Array(memory.buffer, 128, 4).fill(0);
assert.deepEqual([...output.control], [1, 2, 3, 4], "outputs are independent of later Wasm writes");

assert.deepEqual([...output.presentation], [3, 2, 1, 0]);
assert.deepEqual([...output.mediaBytes], [9, 8, 7, 6]);
new Uint8Array(memory.buffer, 768, 4).fill(0);
assert.deepEqual([...output.mediaBytes], [9, 8, 7, 6], "encoded media is independent of subsequent Wasm output");
mediaLength = 2 * 1024 * 1024 + 1;
assert.throws(() => bridge.scheduleNextThread(), /bounded buffer/);
mediaLength = 4;
mediaPointer = memory.buffer.byteLength - 2;
assert.throws(() => bridge.scheduleNextThread(), /outside Wasm memory/);
mediaPointer = 768;
new Uint8Array(memory.buffer, 512, 4).fill(0);
assert.deepEqual([...output.presentation], [3, 2, 1, 0], 'presentation pixels are independent of later Wasm writes');
presentationPointer = memory.buffer.byteLength - 2;
assert.throws(() => bridge.scheduleNextThread(), /outside Wasm memory/);
presentationPointer = 512;
presentationLength = 16 * 1024 * 1024 + 1;
assert.throws(() => bridge.scheduleNextThread(), /bounded buffer/);
presentationLength = 0;
metadataPointer = memory.buffer.byteLength;
assert.throws(() => bridge.scheduleNextThread(), /outside Wasm memory/);
metadataPointer = 0;
controlPointer = memory.buffer.byteLength - 2;
assert.throws(() => bridge.scheduleNextThread(), /outside Wasm memory/);
controlPointer = 128;
controlLength = -1;
assert.throws(() => bridge.scheduleNextThread(), /outside Wasm memory/);
controlLength = 4;
assert.throws(() => bridge.inspectExecutable(new Uint8Array(16 * 1024 * 1024), "x"), /bounded buffer/);
assert.equal(resizes, 0, "oversized input is rejected before entering Wasm");
inputPointer = memory.buffer.byteLength;
assert.throws(() => bridge.inspectExecutable(Uint8Array.of(1), "x"), /outside Wasm memory/);
inputPointer = 4096;

for (const bad of [
  {sequence:'0', kind:'key-down', virtualKey:65, repeat:false},
  {sequence:'9007199254740992', kind:'key-down', virtualKey:65, repeat:false},
  {sequence:'1', kind:'__proto__', virtualKey:65, repeat:false},
  {sequence:'1', kind:'key-down', virtualKey:256, repeat:false},
  {sequence:'1', kind:'key-down', virtualKey:65, repeat:1},
]) assert.throws(() => bridge.ingestKeyboardEvent(bad));
assert.equal(keyboardCalls, 0, 'malformed input must not reach the Wasm bridge');
bridge.ingestKeyboardEvent({sequence:'1', kind:'focus-gained', virtualKey:null, repeat:false});
assert.deepEqual(lastKeyboardWords, [1, 0, 1, 0xffffffff, 0]);
bridge.ingestKeyboardEvent({sequence:'2', kind:'key-down', virtualKey:65, repeat:true});
assert.deepEqual(lastKeyboardWords, [2, 0, 3, 65, 1]);
assert.equal(keyboardCalls, 2);

for (const length of [0, -1, 1.5, 2 * 1024 * 1024 + 1]) {
  assert.throws(() => bridge.readComMedia(1n, 0n, length), /bounded range/);
}
assert.equal(mediaReadCalls, 0, "invalid encoded ranges never enter Rust");
bridge.readComMedia(0x100000002n, 0x300000004n, 32);
assert.deepEqual(lastMediaRead, [2, 1, 4, 3, 32]);
bridge.completeComMedia(0x100000005n, {kind:"frame", resourceId:"9007199254740993", start100ns:0, end100ns:400000}, Uint8Array.of(7,6,5,4));
assert.deepEqual(lastMediaCompletion, {low:5, high:1,
  metadata:{kind:"frame", resourceId:"9007199254740993", start100ns:0, end100ns:400000}, payload:[7,6,5,4]});
assert.throws(() => bridge.completeComMedia(1n, {kind:"backend-failed", message:"x".repeat(4096)}), /metadata exceeds/);
assert.throws(() => bridge.completeComMedia(1n, {kind:"frame"}, new Uint8Array(16*1024*1024)), /bounded buffer/);
assert.equal(mediaCompleteCalls, 1, "invalid media completions do not consume native tokens");
for (const bad of [
  {sequence:'0', kind:'move', button:'none', x:0, y:0},
  {sequence:'9007199254740992', kind:'down', button:'primary', x:0, y:0},
  {sequence:'3', kind:'move', button:'primary', x:1, y:2},
  {sequence:'3', kind:'up', button:'none', x:1, y:2},
  {sequence:'3', kind:'down', button:'middle', x:1, y:2},
  {sequence:'3', kind:'down', button:'secondary', x:65536, y:2},
  {sequence:'3', kind:'down', button:'secondary', x:0, y:-1},
  {sequence:'3', kind:'down', button:'secondary', x:NaN, y:1},
]) assert.throws(() => bridge.ingestPointerEvent(bad));
assert.equal(pointerCalls, 0, 'malformed pointer input must not reach Wasm');
bridge.ingestPointerEvent({sequence:'3', kind:'move', button:'none', x:32768, y:65535});
assert.deepEqual(lastPointerWords, [3, 0, 1, 0, 32768, 65535]);
bridge.ingestPointerEvent({sequence:'4', kind:'down', button:'secondary', x:100, y:200});
assert.deepEqual(lastPointerWords, [4, 0, 2, 2, 100, 200]);
pointerIgnored = true;
bridge.ingestPointerEvent({sequence:'5', kind:'move', button:'none', x:0, y:0});
assert.equal(pointerCalls, 3, 'a pointer event with no guest foreground may be ignored explicitly');
assert.equal(keyboardCalls, 2);

const sharedPage = new Uint8Array(memory.buffer, 65536, 4096);
sharedPage.fill(0x5a);
const externalPage = new Uint8Array(4096).fill(0xa5);
const pages = [
  { address: 0x1000, owner: 1, bytes: sharedPage },
  { address: 0x2000, owner: 1, bytes: externalPage },
];
grow = true;
assert.equal(bridge.reconcileWritebackPages(8n, pages), 9n);
assert.equal(sharedPage.byteLength, 0, "memory.grow detached the old shared page");
assert.equal(pages[0].bytes.buffer, memory.buffer);
const writebackControlBytes = 28 + pages.length * 8;
const input = new Uint8Array(memory.buffer, inputPointer, writebackControlBytes + 8192);
assert(input.subarray(writebackControlBytes, writebackControlBytes + 4096).every(byte => byte === 0x5a));
assert(input.subarray(writebackControlBytes + 4096).every(byte => byte === 0xa5));
let stopCalls = 0, stopStatus = 0, stopLength = null, stopPointer = 16384;
let stopFields = { threadId: 3, lastError: 5, module: "fixture.dll", symbol: "FailingApi" };
let stopBytes;
const stopBridge = RuntimeBridge.fromExports({
  ...exports,
  vine_bridge_stop_context(instructionPointer) {
    stopCalls++;
    stopBytes = new TextEncoder().encode(JSON.stringify({
      kind: "stop-context", instructionPointer, ...stopFields,
    }));
    new Uint8Array(memory.buffer, 16384, stopBytes.length).set(stopBytes);
    return stopStatus;
  },
  vine_bridge_stop_context_ptr: () => stopPointer,
  vine_bridge_stop_context_len: () => stopLength ?? stopBytes.length,
});
const beforeStop = new Uint8Array(memory.buffer, 0, metadata.length).slice();
const resizesBeforeStop = resizes;
assert.deepEqual(stopBridge.stopContext(0x401000), {
  kind: "stop-context", instructionPointer: 0x401000, ...stopFields,
});
assert.deepEqual(new Uint8Array(memory.buffer, 0, metadata.length), beforeStop,
  "stop diagnostics preserve the original bridge error/output buffer");
assert.equal(resizes, resizesBeforeStop, "stop diagnostics do not resize guest input");
stopFields = { threadId: null, lastError: null, module: null, symbol: null };
assert.equal(stopBridge.stopContext(0).threadId, null, "unknown runtime context stays unknown");
const callsBeforeInvalidStop = stopCalls;
for (const eip of [-1, 1.5, 0x1_0000_0000, NaN]) {
  assert.throws(() => stopBridge.stopContext(eip), /outside PE32/);
}
assert.equal(stopCalls, callsBeforeInvalidStop, "invalid stop addresses never enter Wasm");
stopFields.lastError = -1;
assert.throws(() => stopBridge.stopContext(0), /diagnostics are invalid/);
stopFields.lastError = null;
stopLength = 65537;
assert.throws(() => stopBridge.stopContext(0), /bounded buffer/);
stopLength = null;
stopPointer = memory.buffer.byteLength - 1;
assert.throws(() => stopBridge.stopContext(0), /outside Wasm memory/);
stopPointer = 16384;
stopStatus = 1;
assert.throws(() => stopBridge.stopContext(0), /diagnostics are unavailable/);
console.log("runtime bridge: bounded input/output, independent receipts, mixed-page memory growth, and stop diagnostics passed");
