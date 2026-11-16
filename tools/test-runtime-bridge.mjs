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
{
  const counterMemory = new WebAssembly.Memory({ initial: 1 });
  let value = { kind: "timestamp-counter", low: 0xffff_ffff, high: 42 }, encoded, calls = [];
  const counter = RuntimeBridge.fromExports({
    memory: counterMemory,
    vine_bridge_read_timestamp_counter(...words) {
      calls.push(words);
      encoded = new TextEncoder().encode(JSON.stringify(value));
      new Uint8Array(counterMemory.buffer, 0, encoded.length).set(encoded);
      return 0;
    },
    vine_bridge_metadata_ptr: () => 0, vine_bridge_metadata_len: () => encoded.length,
    ...Object.fromEntries(["control","payload","presentation","audio","media"]
      .flatMap(name => [[`vine_bridge_${name}_ptr`, () => 0], [`vine_bridge_${name}_len`, () => 0]])),
  });
  assert.deepEqual(counter.readTimestampCounter(0x1_0000_0001, 429496.7297), { low: 0xffff_ffff, high: 42 });
  assert.deepEqual(calls, [[1, 1, 1, 1]], "clock and retirement words cross independently");
  for (const time of [-1, NaN, Infinity]) assert.throws(() => counter.readTimestampCounter(0, time));
  for (const retired of [-1, 0.5, Number.MAX_SAFE_INTEGER + 1])
    assert.throws(() => counter.readTimestampCounter(retired, 0));
  assert.equal(calls.length, 1, "invalid observations must not reach Rust");
  for (const invalid of [{ kind: "other", low: 0, high: 0 },
    { kind: "timestamp-counter", low: -1, high: 0 },
    { kind: "timestamp-counter", low: 0, high: 0x1_0000_0000 }]) {
    value = invalid;
    assert.throws(() => counter.readTimestampCounter(0, 0), /invalid timestamp counter/);
  }
}
// Wait observations remain Rust-owned scalars and do not consume/replace
// borrowed output buffers. Reject malformed observations at the adapter edge.
{
  let request = 0n;
  const waits = RuntimeBridge.fromExports({ vine_bridge_ready_object_wait_request_id: () => request });
  for (request of [0n, 1n, 1n << 40n, -1n])
    assert.equal(waits.readyObjectWaitRequestId(), BigInt.asUintN(64, request));
  for (request of [1, null, 1n << 63n, -(1n << 63n) - 1n])
    assert.throws(() => waits.readyObjectWaitRequestId(), /invalid object wait request/);
}
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
{
  const shared = new Uint8Array(memory.buffer, 65536, 4096).fill(0x61);
  const external = new Uint8Array(4096).fill(0x72);
  const batches = [
    [
      { address: 0x3000, owner: 2, bytes: shared },
      { address: 0x4000, owner: 3, bytes: external },
      { address: 0x5000, owner: 4, bytes: new Uint8Array(4096).fill(0x83) },
      { address: 0x6000, owner: 5, bytes: new Uint8Array(4096).fill(0x94) },
    ],
    [{ address: 0x7000, owner: 6, bytes: new Uint8Array(4096).fill(0xa5) }],
    [],
    () => [
      { address: 0x8000, owner: 7, bytes: new Uint8Array(4096).fill(0xb6) },
      { address: 0x9000, owner: 8, bytes: new Uint8Array(memory.buffer, 69632, 4096).fill(0xc7) },
      { address: 0xa000, owner: 9, bytes: new Uint8Array(4096).fill(0xd8) },
    ],
  ];
  for (const next of batches) {
    const batch = typeof next === "function" ? next() : next;
    const expected = batch.map(p => ({ address: p.address, owner: p.owner, bytes: Uint8Array.from(p.bytes),
      external: p.bytes.buffer !== memory.buffer ? p.bytes : null }));
    assert.equal(bridge.reconcileWritebackPages(8n, batch), 9n);
    const length = 28 + batch.length * 8;
    const packet = new Uint8Array(memory.buffer, inputPointer, length + batch.length * 4096);
    const control = new DataView(packet.buffer, packet.byteOffset, length);
    assert.equal(control.getUint32(20, true), batch.length, "writeback count does not retain a stale tail");
    for (let index = 0; index < batch.length; index++) {
      assert.equal(control.getUint32(28 + index * 8, true), expected[index].address);
      assert.equal(control.getUint32(32 + index * 8, true), expected[index].owner);
      assert.deepEqual(packet.subarray(length + index * 4096, length + (index + 1) * 4096), expected[index].bytes);
      if (expected[index].external) assert.equal(batch[index].bytes, expected[index].external,
        "an old shared offset must never replace a new external page");
      else assert.equal(batch[index].bytes.buffer, memory.buffer);
    }
  }
}
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
{
  const core = new WebAssembly.Memory({ initial: 1 });
  let completionPointer = 1024, status = 0, growOnDispatch = false;
  let jsonReads = 0, completionReads = 0, payloadPointer = 2048;
  const writeCompletion = (eax, hasEax) => {
    new Uint32Array(core.buffer, 1024, 4).set([0x401234, 0x801000, eax, hasEax]);
    new Uint8Array(core.buffer, 2048, 2).set([42, 43]);
  };
  writeCompletion(0xffffffff, 1);
  const json = { kind: "api-complete", completion: {
    instructionPointer: 0x401234, stackPointer: 0x801000, eax: 7,
    executionBoundary: "yield-current-thread",
  }, createdThread: { threadId: 2 }, threadSelection: { threadId: 3 } };
  let fallback = json;
  const encoded = () => {
    const bytes = new TextEncoder().encode(JSON.stringify(fallback));
    new Uint8Array(core.buffer, 128, bytes.length).set(bytes);
    return bytes.length;
  };
  const fast = RuntimeBridge.fromExports({
    memory: core,
    vine_bridge_dispatch() { if (growOnDispatch) core.grow(1); return status; },
    vine_bridge_api_completion_ptr() { completionReads++; return completionPointer; },
    vine_bridge_metadata_ptr: () => 128,
    vine_bridge_metadata_len() { jsonReads++; return encoded(); },
    vine_bridge_control_ptr: () => 2048, vine_bridge_control_len: () => 2,
    vine_bridge_payload_ptr: () => payloadPointer, vine_bridge_payload_len: () => 2,
    vine_bridge_presentation_ptr: () => 2048, vine_bridge_presentation_len: () => 2,
    vine_bridge_audio_ptr: () => 2048, vine_bridge_audio_len: () => 2,
    vine_bridge_media_ptr: () => 2048, vine_bridge_media_len: () => 2,
  });
  const registers = Object.fromEntries(["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi",
    "instructionPointer", "eflags", "es", "cs", "ss", "ds", "fs", "gs"].map(name => [name, 0]));
  const dispatch = () => fast.dispatch(0x70000000, 0x801000, 0x401234, 0, registers);
  const borrowed = dispatch();
  assert.deepEqual(borrowed.metadata, { kind: "api-complete", completion: {
    instructionPointer: 0x401234, stackPointer: 0x801000, eax: 0xffffffff,
  } });
  assert.equal(jsonReads, 0, "plain completion skips JSON decoding");
  assert.equal(borrowed.control.buffer, core.buffer);
  assert.equal(borrowed.payload.buffer, core.buffer);
  for (const channel of ["presentation", "audio", "mediaBytes"]) {
    assert.notEqual(borrowed[channel].buffer, core.buffer, "async resource bytes remain owned");
  }
  new Uint8Array(core.buffer, 2048, 2).fill(0);
  assert.deepEqual([...borrowed.control], [0, 0], "plain delta lifetime ends at the next core mutation");
  assert.deepEqual([...borrowed.presentation], [42, 43]);
  assert.equal(borrowed.metadata.completion.eax, 0xffffffff, "resume values survive delta invalidation");
  writeCompletion(0xdeadbeef, 0);
  growOnDispatch = true;
  const grown = dispatch();
  growOnDispatch = false;
  assert.equal(grown.payload.buffer, core.buffer, "views use the buffer after core growth during dispatch");
  assert.equal(grown.metadata.completion.eax, null, "absent EAX preserves the CPU register");
  const beforePayload = [...grown.payload];
  for (const pointer of [-1, 1.5, NaN, core.buffer.byteLength - 8]) {
    completionPointer = pointer;
    assert.throws(dispatch, /outside Wasm memory/);
    assert.deepEqual([...grown.payload], beforePayload);
  }
  completionPointer = 1024;
  writeCompletion(0, 2);
  assert.throws(dispatch, /register flag/);
  writeCompletion(0, 1);
  payloadPointer = core.buffer.byteLength - 1;
  assert.throws(dispatch, /outside Wasm memory/);
  payloadPointer = 2048;
  completionPointer = 0;
  const complex = dispatch();
  assert.deepEqual(complex.metadata, json, "thread creation and selection retain the complete JSON receipt");
  new Uint8Array(core.buffer, 2048, 2).fill(99);
  assert.deepEqual([...complex.payload], [42, 43], "complex dispatch deltas remain owned");
  fallback = { kind: "error", message: "reached failure" };
  status = 1;
  const beforeFailure = completionReads;
  assert.throws(dispatch, /reached failure/);
  assert.equal(completionReads, beforeFailure, "failed dispatch reads its error before a stale fast header");
}
console.log("runtime bridge: bounded input/output, owned async receipts, synchronous borrowed completion, mixed-page memory growth, and stop diagnostics passed");

{
  const core = new WebAssembly.Memory({ initial: 1 });
  const frame = { width: 1, height: 1, colorArgb: null, draws: [] };
  let json = { kind: 'com-complete', threadId: 8192,
    completion: { instructionPointer: 0x401000, stackPointer: 0x801000, eax: 0 }, presentation: frame };
  let pointer = 2048, length = 8, growNext = false;
  const bytes = [1, 2, 3, 4, 5, 6, 7, 8];
  const refresh = () => new Uint8Array(core.buffer, 2048, 8).set(bytes);
  refresh();
  const encoded = () => {
    const bytes = new TextEncoder().encode(JSON.stringify(json));
    new Uint8Array(core.buffer, 128, bytes.length).set(bytes);
    return bytes.length;
  };
  const exports = {
    memory: core,
    vine_bridge_dispatch() { if (growNext) { core.grow(1); refresh(); } return 0; },
    vine_bridge_api_completion_ptr: () => 0,
    vine_bridge_metadata_ptr: () => 128, vine_bridge_metadata_len: encoded,
    vine_bridge_control_ptr: () => 2048, vine_bridge_control_len: () => 2,
    vine_bridge_payload_ptr: () => 2048, vine_bridge_payload_len: () => 2,
    vine_bridge_presentation_ptr: () => pointer, vine_bridge_presentation_len: () => length,
    vine_bridge_audio_ptr: () => 2048, vine_bridge_audio_len: () => 2,
    vine_bridge_media_ptr: () => 2048, vine_bridge_media_len: () => 2,
    vine_bridge_input_resize: () => 0, vine_bridge_input_ptr: () => 4096,
    vine_bridge_complete_callback: () => 0,
    vine_bridge_complete_com_audio: () => 0,
    vine_bridge_complete_com_media: () => 0,
    vine_bridge_schedule_next_thread: () => 0,
  };
  const bridge = RuntimeBridge.fromExports(exports);
  const registers = Object.fromEntries(['eax', 'ecx', 'edx', 'ebx', 'esp', 'ebp', 'esi', 'edi',
    'instructionPointer', 'eflags', 'es', 'cs', 'ss', 'ds', 'fs', 'gs'].map(name => [name, 0]));
  const dispatch = () => bridge.dispatch(0x70000000, 0x801000, 0x401000, 0, registers);
  const borrowed = dispatch();
  assert.equal(borrowed.presentation.buffer, core.buffer, 'only initial synchronous COM presentation is borrowed');
  assert.deepEqual([...borrowed.presentation], bytes);
  for (const channel of ['control', 'payload', 'audio', 'mediaBytes'])
    assert.notEqual(borrowed[channel].buffer, core.buffer, channel + ' retains its existing ownership');
  new Uint8Array(core.buffer, 2048, 8).fill(99);
  assert.deepEqual([...borrowed.presentation], Array(8).fill(99), 'view lifetime ends at the next Runtime mutation');
  assert.deepEqual([...borrowed.control], [1, 2]);
  assert.equal(borrowed.metadata.completion.instructionPointer, 0x401000, 'parsed metadata remains owned');
  refresh();
  growNext = true;
  const grown = dispatch();
  growNext = false;
  assert.equal(grown.presentation.buffer, core.buffer, 'borrow uses the buffer after dispatch-time growth');
  assert.deepEqual([...grown.presentation], bytes);
  for (const operation of [
    () => bridge.completeCallback(0x70000001, 0),
    () => bridge.completeComAudio(1n, true),
    () => bridge.completeComMedia(1n, { kind: 'acknowledged' }),
    () => bridge.scheduleNextThread(),
  ]) {
    refresh(); const output = operation();
    assert.notEqual(output.presentation.buffer, core.buffer, 'same COM metadata in deferred operation remains owned');
    new Uint8Array(core.buffer, 2048, 8).fill(77);
    assert.deepEqual([...output.presentation], bytes);
  }
  refresh();
  json.createdThread = { threadId: 8193 };
  const created = dispatch();
  assert.notEqual(created.presentation.buffer, core.buffer, 'created-thread COM retains owned output');
  new Uint8Array(core.buffer, 2048, 8).fill(66);
  assert.deepEqual([...created.presentation], bytes);
  delete json.createdThread;
  delete json.presentation;
  refresh();
  const plain = dispatch();
  assert.notEqual(plain.presentation.buffer, core.buffer, 'COM without a presentation retains owned resource output');
  json.presentation = frame;
  for (const invalid of [-1, 1.5, NaN, core.buffer.byteLength - 7]) {
    pointer = invalid;
    assert.throws(dispatch, /outside Wasm memory/);
  }
  pointer = 2048;
  for (const invalid of [-1, 1.5, NaN, 16 * 1024 * 1024 + 1]) {
    length = invalid;
    assert.throws(dispatch, /bounded buffer/);
  }
  length = 0; pointer = core.buffer.byteLength + 1;
  const emptyBorrow = dispatch(), emptyOwned = bridge.scheduleNextThread();
  assert.equal(emptyBorrow.presentation.byteLength, 0);
  assert.equal(emptyBorrow.presentation, emptyOwned.presentation, 'zero-length path preserves the shared empty object');
  console.log('presentation bridge: initial provenance, bounds/growth, owned deferred/created fallbacks passed');
}

// CPU snapshots are opaque owned transfers. Reject a malformed receipt before
// reading its payload, and never expose the Runtime's reusable output buffer.
{
  const core = new WebAssembly.Memory({ initial: 1 });
  let receipt, payloadLength = 0, extraControl = 0, storedBytes, putCalls = 0, getCalls = 0;
  const encode = () => {
    const bytes = new TextEncoder().encode(JSON.stringify(receipt));
    new Uint8Array(core.buffer, 128, bytes.length).set(bytes); return bytes.length;
  };
  const api = {
    memory: core,
    vine_bridge_input_resize: () => { core.grow(1); return 0; },
    vine_bridge_input_ptr: () => 4096,
    vine_bridge_metadata_ptr: () => 128, vine_bridge_metadata_len: encode,
    vine_bridge_control_len: () => extraControl,
    vine_bridge_payload_ptr: () => 8192, vine_bridge_payload_len: () => payloadLength,
    vine_bridge_presentation_len: () => 0, vine_bridge_audio_len: () => 0, vine_bridge_media_len: () => 0,
    vine_bridge_cpu_snapshot_put(threadId, low, high, backendTag, abiVersion, byteLength) {
      putCalls++;
      storedBytes = Uint8Array.from(new Uint8Array(core.buffer, 4096, byteLength));
      const next = (BigInt(low) | BigInt(high) << 32n) + 1n;
      receipt = { kind: 'cpu-snapshot-stored', threadId, revisionLow: Number(next & 0xffff_ffffn),
        revisionHigh: Number(next >> 32n), backendTag, abiVersion, byteLength };
      payloadLength = 0; return 0;
    },
    vine_bridge_cpu_snapshot_get() { getCalls++; return 0; },
  };
  const bridge = RuntimeBridge.fromExports(api);
  for (const [backend, abiVersion, length] of [['v86', 1, 612], ['recompiler', 2, 512]]) {
    const words = Uint32Array.from({ length: length / 4 }, (_, i) => (i * 0x10204081) >>> 0);
    const original = Uint32Array.from(words);
    assert.equal(bridge.storeCpuSnapshot(8192, 0xffff_ffffn, { backend, abiVersion, words }), 0x1_0000_0000n);
    assert.deepEqual(storedBytes, new Uint8Array(original.buffer));
    words.fill(0);
    receipt.kind = 'cpu-snapshot'; payloadLength = length;
    new Uint8Array(core.buffer, 8192, length).set(storedBytes);
    const loaded = bridge.loadCpuSnapshot(8192);
    assert.equal(loaded.revision, 0x1_0000_0000n);
    assert.equal(loaded.snapshot.backend, backend); assert.equal(loaded.snapshot.abiVersion, abiVersion);
    assert.deepEqual(loaded.snapshot.words, original);
    new Uint8Array(core.buffer, 8192, length).fill(0);
    assert.deepEqual(loaded.snapshot.words, original, 'parked bytes must survive the next Runtime call');
    const valid = { ...receipt };
    for (const patch of [{ threadId: 8193 }, { revisionLow: -1 }, { revisionLow: 0, revisionHigh: 0 },
      { backendTag: 9 }, { abiVersion: 99 }, { byteLength: 0x7fff_ffff }, { byteLength: length - 4 }]) {
      receipt = { ...valid, ...patch };
      assert.throws(() => bridge.loadCpuSnapshot(8192), /snapshot receipt/);
    }
    receipt = valid; payloadLength = length - 4;
    assert.throws(() => bridge.loadCpuSnapshot(8192), /payload length/);
    payloadLength = length; extraControl = 1;
    assert.throws(() => bridge.loadCpuSnapshot(8192), /snapshot receipt/);
    extraControl = 0;
  }
  const calls = putCalls;
  for (const snapshot of [
    { backend: 'recompiler', abiVersion: 1, words: new Uint32Array(105) },
    { backend: 'recompiler', abiVersion: 2, words: new Uint32Array(129) },
    { backend: 'v86', abiVersion: 1, words: new Uint32Array(152) },
  ]) assert.throws(() => bridge.storeCpuSnapshot(8192, 0n, snapshot), /ABI mismatch/);
  assert.equal(putCalls, calls, 'invalid input cannot mutate Runtime storage');
  assert(getCalls > 0);
  console.log('CPU snapshot bridge: opaque ownership, growth, revisions and malformed receipt bounds passed');
}
