import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { summarizeTrace } from "./summarize-windows-trace.mjs";

const schema = "vine.win32-trace.v2";
const events = [
  {
    schema,
    kind: "trace-start",
    timestamp: "2026-07-10T00:00:00.000Z",
    executable: "C:\\fixture\\abi-smoke.exe",
    executableBytes: 2560,
    executableSha256: "a".repeat(64),
    python: "3.14.6",
    frida: "17.15.4",
    platform: "Windows-11-test",
    arguments: { seconds: 1.25, killAfter: true, vtables: ["idirect3d9"] },
  },
  {
    schema,
    sequence: 0,
    kind: "module-load",
    module: "abi-smoke.exe",
    path: "C:\\fixture\\abi-smoke.exe",
  },
  {
    schema,
    sequence: 1,
    kind: "module-load",
    module: "KERNEL32.DLL",
    path: "C:\\Windows\\System32\\KERNEL32.DLL",
  },
  {
    schema,
    sequence: 2,
    kind: "agent-ready",
    architecture: "ia32",
    pointerSize: 4,
    processId: 123,
    selectedVtables: ["idirect3d9"],
  },
  {
    schema,
    sequence: 3,
    threadId: 10,
    kind: "api-enter",
    module: "KERNEL32.DLL",
    api: "CreateFileA",
    arguments: ["C:\\fixture\\data.arc"],
  },
  {
    schema,
    sequence: 4,
    threadId: 10,
    kind: "api-enter",
    module: "KERNEL32.DLL",
    api: "CreateFileW",
    arguments: ["C:\\fixture\\data.arc"],
  },
  {
    schema,
    sequence: 5,
    threadId: 10,
    kind: "api-leave",
    module: "KERNEL32.DLL",
    api: "CreateFileW",
    result: "0x44",
  },
  {
    schema,
    sequence: 6,
    threadId: 10,
    kind: "api-leave",
    module: "KERNEL32.DLL",
    api: "CreateFileA",
    result: "0x44",
  },
  {
    schema,
    sequence: 7,
    threadId: 10,
    kind: "api-enter",
    module: "KERNEL32.DLL",
    api: "CreateFileW",
    arguments: ["C:\\fixture\\Graphic\\missing.bin"],
  },
  {
    schema,
    sequence: 8,
    threadId: 10,
    kind: "api-leave",
    module: "KERNEL32.DLL",
    api: "CreateFileW",
    result: "0xffffffff",
  },
  {
    schema,
    sequence: 9,
    threadId: 10,
    kind: "api-enter",
    module: "USER32.dll",
    api: "CreateWindowExA",
    arguments: ["0x0", "FixtureWindow", "Fixture"],
  },
  {
    schema,
    sequence: 10,
    threadId: 10,
    kind: "api-leave",
    module: "USER32.dll",
    api: "CreateWindowExA",
    result: "0x55",
  },
  {
    schema,
    sequence: 11,
    threadId: 10,
    kind: "api-enter",
    module: "d3d9.dll",
    api: "Direct3DCreate9",
    arguments: ["0x20"],
  },
  {
    schema,
    sequence: 12,
    threadId: 10,
    kind: "api-leave",
    module: "d3d9.dll",
    api: "Direct3DCreate9",
    result: "0x66",
  },
  {
    schema,
    sequence: 13,
    threadId: 10,
    kind: "api-enter",
    module: "DINPUT8.dll",
    api: "DirectInput8Create",
    arguments: [],
  },
  {
    schema,
    sequence: 14,
    threadId: 10,
    kind: "api-leave",
    module: "DINPUT8.dll",
    api: "DirectInput8Create",
    result: "0x0",
  },
  {
    schema,
    sequence: 15,
    threadId: 10,
    kind: "api-enter",
    module: "DSOUND.dll",
    api: "DirectSoundCreate8",
    arguments: [],
  },
  {
    schema,
    sequence: 16,
    threadId: 10,
    kind: "api-leave",
    module: "DSOUND.dll",
    api: "DirectSoundCreate8",
    result: "0x0",
  },
  {
    schema,
    sequence: 17,
    threadId: 10,
    kind: "api-enter",
    module: "ole32.dll",
    api: "CoInitializeEx",
    arguments: [],
  },
  {
    schema,
    sequence: 18,
    threadId: 10,
    kind: "api-leave",
    module: "ole32.dll",
    api: "CoInitializeEx",
    result: "0x0",
  },
  {
    schema,
    sequence: 19,
    threadId: 10,
    kind: "vtable-observed",
    interface: "IDirect3D9",
    object: "0x330000",
    vtable: "0x76010000",
    slots: [
      "QueryInterface",
      "AddRef",
      "Release",
      "RegisterSoftwareDevice",
      "GetAdapterCount",
      "GetAdapterIdentifier",
      "GetAdapterModeCount",
      "EnumAdapterModes",
      "GetAdapterDisplayMode",
      "CheckDeviceType",
      "CheckDeviceFormat",
      "CheckDeviceMultiSampleType",
      "CheckDepthStencilMatch",
      "CheckDeviceFormatConversion",
      "GetDeviceCaps",
      "GetAdapterMonitor",
      "CreateDevice",
    ].map((name, slot) => ({
      slot,
      name,
      address: `0x7601${slot.toString(16).padStart(4, "0")}`,
    })),
  },
  {
    schema,
    sequence: 20,
    threadId: 10,
    kind: "api-enter",
    module: "d3d9.dll",
    api: "CreateDevice",
    interface: "IDirect3D9",
    vtableSlot: 16,
    object: "0x330000",
    arguments: ["0x0", "0x1", "0x44", "0x44", "0x340000", "0x350000"],
  },
  {
    schema,
    sequence: 21,
    threadId: 10,
    kind: "api-leave",
    module: "d3d9.dll",
    api: "CreateDevice",
    interface: "IDirect3D9",
    vtableSlot: 16,
    object: "0x330000",
    result: "0x0",
  },
  {
    schema,
    kind: "trace-stop",
    timestamp: "2026-07-10T00:00:01.250Z",
    reason: "timeout-kill",
    detachedReason: null,
    crash: null,
  },
];

const mainCaller = {
  address: "0x00401100",
  module: "abi-smoke.exe",
  rva: 0x1100,
};
const wrapperCaller = {
  address: "0x76001234",
  module: "KERNEL32.DLL",
  rva: 0x1234,
};
for (const call of [
  {
    callId: 1,
    parentCallId: null,
    enterSequence: 3,
    leaveSequence: 6,
    caller: mainCaller,
    durationMs: 5,
    lastError: 0,
  },
  {
    callId: 2,
    parentCallId: 1,
    enterSequence: 4,
    leaveSequence: 5,
    caller: wrapperCaller,
    durationMs: 2,
    lastError: 0,
  },
  {
    callId: 3,
    parentCallId: null,
    enterSequence: 7,
    leaveSequence: 8,
    caller: mainCaller,
    durationMs: 3,
    lastError: 2,
  },
  {
    callId: 4,
    parentCallId: null,
    enterSequence: 9,
    leaveSequence: 10,
    caller: mainCaller,
    durationMs: 4,
    lastError: 0,
  },
  {
    callId: 5,
    parentCallId: null,
    enterSequence: 11,
    leaveSequence: 12,
    caller: mainCaller,
    durationMs: 6,
    lastError: 0,
  },
  {
    callId: 6,
    parentCallId: null,
    enterSequence: 13,
    leaveSequence: 14,
    caller: mainCaller,
    durationMs: 7,
    lastError: 0,
  },
  {
    callId: 7,
    parentCallId: null,
    enterSequence: 15,
    leaveSequence: 16,
    caller: mainCaller,
    durationMs: 8,
    lastError: 0,
  },
  {
    callId: 8,
    parentCallId: null,
    enterSequence: 17,
    leaveSequence: 18,
    caller: mainCaller,
    durationMs: 9,
    lastError: 0,
  },
  {
    callId: 9,
    parentCallId: null,
    enterSequence: 20,
    leaveSequence: 21,
    caller: mainCaller,
    durationMs: 12,
    lastError: 0,
  },
]) {
  const enter = events.find((event) => event.sequence === call.enterSequence);
  const leave = events.find((event) => event.sequence === call.leaveSequence);
  Object.assign(enter, {
    callId: call.callId,
    parentCallId: call.parentCallId,
    caller: { ...call.caller },
  });
  Object.assign(leave, {
    callId: call.callId,
    parentCallId: call.parentCallId,
    caller: { ...call.caller },
    durationMs: call.durationMs,
    lastError: call.lastError,
  });
}

const encode = (items) =>
  Buffer.from(`${items.map((event) => JSON.stringify(event)).join("\n")}\n`);
const buffer = encode(events);
const summary = summarizeTrace(buffer, "fixture.jsonl");

assert.equal(summary.schema, "vine.win32-trace-summary.v2");
assert.equal(summary.source.lines, events.length);
assert.equal(summary.trace.inputSchema, schema);
assert.equal(summary.trace.durationMs, 1250);
assert.equal(summary.trace.architecture, "ia32");
assert.equal(summary.trace.pointerSize, 4);
assert.equal(summary.trace.executableBytes, 2560);
assert.equal(summary.trace.executableSha256, "a".repeat(64));
assert.equal(summary.trace.stopReason, "timeout-kill");
assert.deepEqual(summary.trace.tools, {
  python: "3.14.6",
  frida: "17.15.4",
  platform: "Windows-11-test",
});
assert.deepEqual(summary.trace.captureArguments, {
  seconds: 1.25,
  killAfter: true,
  vtables: ["idirect3d9"],
});
assert.deepEqual(summary.trace.selectedVtables, ["idirect3d9"]);
assert.deepEqual(summary.trace.sequence, {
  events: 22,
  first: 0,
  last: 21,
  contiguous: true,
});
assert.deepEqual(summary.trace.apiPairing, {
  contract: "call-id-v1",
  enters: 9,
  leaves: 9,
  pairs: 9,
  maxDepth: 2,
  balanced: true,
});
assert.equal(summary.modules.length, 2);
assert.equal(summary.apis.length, 8);
assert.deepEqual(summary.apis[0], {
  module: "KERNEL32.DLL",
  api: "CreateFileA",
  interface: null,
  vtableSlot: null,
  calls: 1,
  firstSequence: 3,
  lastSequence: 3,
  callers: [{ module: "abi-smoke.exe", rva: 0x1100, calls: 1 }],
  durationMs: { total: 5, max: 5 },
  lastErrors: [{ value: 0, calls: 1 }],
});
assert.deepEqual(summary.vtables, [
  {
    sequence: 19,
    interface: "IDirect3D9",
    slots: [
      "QueryInterface",
      "AddRef",
      "Release",
      "RegisterSoftwareDevice",
      "GetAdapterCount",
      "GetAdapterIdentifier",
      "GetAdapterModeCount",
      "EnumAdapterModes",
      "GetAdapterDisplayMode",
      "CheckDeviceType",
      "CheckDeviceFormat",
      "CheckDeviceMultiSampleType",
      "CheckDepthStencilMatch",
      "CheckDeviceFormatConversion",
      "GetDeviceCaps",
      "GetAdapterMonitor",
      "CreateDevice",
    ].map((name, slot) => ({ slot, name })),
  },
]);
assert.equal(summary.categories.file.calls, 3);
assert.equal(summary.categories.window.first.api, "CreateWindowExA");
assert.equal(summary.categories.graphics.first.api, "Direct3DCreate9");
assert.equal(summary.categories.input.calls, 1);
assert.equal(summary.categories.audio.calls, 1);
assert.equal(summary.categories.com.calls, 1);
assert.deepEqual(summary.files.topPaths, [
  { path: "C:\\fixture\\Graphic\\missing.bin", calls: 1 },
  { path: "C:\\fixture\\data.arc", calls: 1 },
]);
assert.equal(summary.files.logicalCalls, 2);
assert.deepEqual(summary.files.titleRelative.successfulPaths, ["data.arc"]);
assert.equal(summary.files.titleRelative.failedCalls, 1);
assert.deepEqual(summary.files.titleRelative.failedSearchRoots, [
  { root: "Graphic", calls: 1, uniquePaths: 1 },
]);
assert.deepEqual(summary.diagnostics, {
  "hook-error": [],
  "vtable-error": [],
  "call-stack-error": [],
  "agent-message": [],
  "cleanup-error": [],
});

const duplicateSequence = events.map((event) => ({ ...event }));
duplicateSequence[5].sequence = 3;
assert.throws(
  () => summarizeTrace(encode(duplicateSequence)),
  /trace sequence 3 follows 3/,
);

const missingSequence = events.filter((event) => event.sequence !== 1);
assert.throws(
  () => summarizeTrace(encode(missingSequence)),
  /trace sequence 2 is not contiguous after 0/,
);

const stopBeforeTail = [
  ...events.slice(0, -1),
  events.at(-1),
  { schema, sequence: 22, kind: "module-load", module: "late.dll", path: "late.dll" },
];
assert.throws(
  () => summarizeTrace(encode(stopBeforeTail)),
  /trace-stop must be the last record/,
);

const unsequencedReady = events.map((event) => ({ ...event }));
delete unsequencedReady.find((event) => event.kind === "agent-ready").sequence;
assert.throws(
  () => summarizeTrace(encode(unsequencedReady)),
  /agent-ready must have an integer sequence/,
);

const negativeDuration = events.map((event) => ({ ...event }));
negativeDuration.at(-1).timestamp = "2026-07-09T23:59:59.000Z";
assert.throws(
  () => summarizeTrace(encode(negativeDuration)),
  /trace stop timestamp precedes trace start/,
);

const wrongCallId = structuredClone(events);
wrongCallId.find((event) => event.sequence === 5).callId = 99;
assert.throws(
  () => summarizeTrace(encode(wrongCallId)),
  /api-leave sequence 5 has callId 99, expected 2/,
);

const wrongParent = structuredClone(events);
wrongParent.find((event) => event.sequence === 4).parentCallId = null;
assert.throws(
  () => summarizeTrace(encode(wrongParent)),
  /api-enter call 2 has parent null, expected 1/,
);

const missingCaller = structuredClone(events);
delete missingCaller.find((event) => event.sequence === 3).caller;
assert.throws(
  () => summarizeTrace(encode(missingCaller)),
  /api-enter sequence 3 has no caller/,
);

const invalidLeaveMetadata = structuredClone(events);
invalidLeaveMetadata.find((event) => event.sequence === 8).lastError = -1;
assert.throws(
  () => summarizeTrace(encode(invalidLeaveMetadata)),
  /api-leave call 3 has invalid lastError/,
);
invalidLeaveMetadata.find((event) => event.sequence === 8).lastError = 2;
invalidLeaveMetadata.find((event) => event.sequence === 8).durationMs = -1;
assert.throws(
  () => summarizeTrace(encode(invalidLeaveMetadata)),
  /api-leave call 3 has invalid durationMs/,
);

const legacyEvents = structuredClone(events);
for (const event of legacyEvents) {
  event.schema = "vine.win32-trace.v1";
  if (event.kind === "api-enter" || event.kind === "api-leave") {
    delete event.callId;
    delete event.parentCallId;
    delete event.caller;
    delete event.durationMs;
    delete event.lastError;
  }
}
delete legacyEvents.find((event) => event.kind === "agent-ready").selectedVtables;
delete legacyEvents[0].arguments.vtables;
const legacySummary = summarizeTrace(encode(legacyEvents));
assert.equal(legacySummary.trace.inputSchema, "vine.win32-trace.v1");
assert.equal(legacySummary.trace.apiPairing.contract, "legacy-thread-stack");

const temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "vine-trace-"));
try {
  const input = path.join(temporaryDirectory, "trace.jsonl");
  const output = path.join(temporaryDirectory, "summary.json");
  const script = fileURLToPath(
    new URL("./summarize-windows-trace.mjs", import.meta.url),
  );
  fs.writeFileSync(input, buffer);
  const first = spawnSync(process.execPath, [script, "--input", input, "--output", output]);
  assert.equal(first.status, 0, first.stderr.toString());
  const overwrite = spawnSync(process.execPath, [
    script,
    "--input",
    input,
    "--output",
    output,
  ]);
  assert.equal(overwrite.status, 1);
  assert.match(overwrite.stderr.toString(), /summary output already exists/);
  const forced = spawnSync(process.execPath, [
    script,
    "--input",
    input,
    "--output",
    output,
    "--force",
  ]);
  assert.equal(forced.status, 0, forced.stderr.toString());
} finally {
  fs.rmSync(temporaryDirectory, { recursive: true, force: true });
}

console.log("Windows trace summary contract passed");
