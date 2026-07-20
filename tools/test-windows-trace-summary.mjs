import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import {
  idirect3d9MethodNames,
  idirect3ddevice9MethodNames,
  summarizeTrace,
} from "./summarize-windows-trace.mjs";

const schema = "vine.win32-trace.v2";
const adapterIdentifierBefore = Array(1100).fill(0xcc);
const adapterIdentifierBytes = new Uint8Array(1100);
const adapterIdentifierView = new DataView(adapterIdentifierBytes.buffer);
const writeIdentifierString = (offset, value) => {
  adapterIdentifierBytes.set(Buffer.from(value, "ascii"), offset);
};
writeIdentifierString(0, "fixture.dll");
writeIdentifierString(512, "Fixture Adapter");
writeIdentifierString(1024, "\\\\.\\DISPLAY1");
adapterIdentifierView.setUint32(1056, 0x0004_0003, true);
adapterIdentifierView.setUint32(1060, 0x0002_0001, true);
adapterIdentifierView.setUint32(1064, 0x10de, true);
adapterIdentifierView.setUint32(1068, 0x1c82, true);
adapterIdentifierView.setUint32(1072, 0x11aa_22bb, true);
adapterIdentifierView.setUint32(1076, 0xa1, true);
adapterIdentifierBytes.set(
  [0x44, 0x33, 0x22, 0x11, 0x66, 0x55, 0x88, 0x77, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00],
  1080,
);
adapterIdentifierView.setUint32(1096, 1, true);
const adapterIdentifierAfter = [...adapterIdentifierBytes];
const deviceCapsBefore = Array(304).fill(0xcc);
const deviceCapsBytes = new Uint8Array(304);
const deviceCapsView = new DataView(deviceCapsBytes.buffer);
for (let index = 0; index < 76; index += 1) {
  deviceCapsView.setUint32(index * 4, 0x1000_0000 + index, true);
}
deviceCapsView.setUint32(0, 1, true);
deviceCapsView.setUint32(4, 0, true);
deviceCapsView.setFloat32(112, 1.0, true);
deviceCapsView.setFloat32(176, 64.0, true);
deviceCapsView.setInt32(252, -1, true);
const deviceCapsAfter = [...deviceCapsBytes];
const presentParametersBeforeFields = {
  backBufferWidth: 800,
  backBufferHeight: 600,
  backBufferFormat: 22,
  backBufferCount: 1,
  multiSampleType: 0,
  multiSampleQuality: 0,
  swapEffect: 1,
  deviceWindow: 0x55_0000,
  windowed: 1,
  enableAutoDepthStencil: 1,
  autoDepthStencilFormat: 75,
  flags: 0,
  fullScreenRefreshRateInHz: 0,
  presentationInterval: 0x8000_0000,
};
const presentParametersAfterFields = {
  ...presentParametersBeforeFields,
  backBufferCount: 2,
};
const encodePresentParameters = (fields) => {
  const bytes = new Uint8Array(56);
  const view = new DataView(bytes.buffer);
  Object.values(fields).forEach((value, index) =>
    view.setUint32(index * 4, value, true),
  );
  return [...bytes];
};
const presentParametersBefore = encodePresentParameters(
  presentParametersBeforeFields,
);
const presentParametersAfter = encodePresentParameters(
  presentParametersAfterFields,
);
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
    arguments: {
      seconds: 1.25,
      killAfter: true,
      vtables: ["idirect3d9"],
      observations: ["d3d9-create-device"],
    },
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
    selectedObservations: ["d3d9-create-device"],
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
    slots: idirect3d9MethodNames.map((name, slot) => ({
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
    api: "CheckDeviceType",
    interface: "IDirect3D9",
    vtableSlot: 9,
    object: "0x330000",
    arguments: ["0x0", "0x1", "0x16", "0x16", "0x1"],
  },
  {
    schema,
    sequence: 21,
    threadId: 10,
    kind: "api-leave",
    module: "d3d9.dll",
    api: "CheckDeviceType",
    interface: "IDirect3D9",
    vtableSlot: 9,
    object: "0x330000",
    result: "0x0",
  },
  {
    schema,
    sequence: 22,
    threadId: 10,
    kind: "api-enter",
    module: "d3d9.dll",
    api: "GetAdapterDisplayMode",
    interface: "IDirect3D9",
    vtableSlot: 8,
    object: "0x330000",
    arguments: ["0x0", "0x360000"],
  },
  {
    schema,
    sequence: 23,
    threadId: 10,
    kind: "api-leave",
    module: "d3d9.dll",
    api: "GetAdapterDisplayMode",
    interface: "IDirect3D9",
    vtableSlot: 8,
    object: "0x330000",
    result: "0x0",
    d3d9DisplayMode: {
      adapter: 0,
      before: {
        pointer: "0x360000",
        size: 16,
        bytes: Array(16).fill(0xcc),
        fields: {
          width: 0xcccccccc,
          height: 0xcccccccc,
          refreshRate: 0xcccccccc,
          format: 0xcccccccc,
        },
      },
      after: {
        pointer: "0x360000",
        size: 16,
        bytes: [128, 7, 0, 0, 56, 4, 0, 0, 60, 0, 0, 0, 22, 0, 0, 0],
        fields: { width: 1920, height: 1080, refreshRate: 60, format: 22 },
      },
    },
  },
  {
    schema,
    sequence: 24,
    threadId: 10,
    kind: "api-enter",
    module: "d3d9.dll",
    api: "GetAdapterIdentifier",
    interface: "IDirect3D9",
    vtableSlot: 5,
    object: "0x330000",
    arguments: ["0x0", "0x0", "0x370000"],
  },
  {
    schema,
    sequence: 25,
    threadId: 10,
    kind: "api-leave",
    module: "d3d9.dll",
    api: "GetAdapterIdentifier",
    interface: "IDirect3D9",
    vtableSlot: 5,
    object: "0x330000",
    result: "0x0",
    d3d9AdapterIdentifier: {
      adapter: 0,
      flags: 0,
      before: {
        pointer: "0x370000",
        size: 1100,
        bytes: adapterIdentifierBefore,
      },
      after: {
        pointer: "0x370000",
        size: 1100,
        bytes: adapterIdentifierAfter,
      },
    },
  },
  {
    schema,
    sequence: 26,
    threadId: 10,
    kind: "api-enter",
    module: "d3d9.dll",
    api: "GetDeviceCaps",
    interface: "IDirect3D9",
    vtableSlot: 14,
    object: "0x330000",
    arguments: ["0x0", "0x1", "0x380000"],
  },
  {
    schema,
    sequence: 27,
    threadId: 10,
    kind: "api-leave",
    module: "d3d9.dll",
    api: "GetDeviceCaps",
    interface: "IDirect3D9",
    vtableSlot: 14,
    object: "0x330000",
    result: "0x0",
    d3d9DeviceCaps: {
      adapter: 0,
      deviceType: 1,
      callerStackPointerAfterReturn: "0x380000",
      before: {
        pointer: "0x380000",
        size: 304,
        bytes: deviceCapsBefore,
      },
      after: {
        pointer: "0x380000",
        size: 304,
        bytes: deviceCapsAfter,
      },
    },
  },
  {
    schema,
    sequence: 28,
    threadId: 10,
    kind: "api-enter",
    module: "d3d9.dll",
    api: "CheckDeviceFormat",
    interface: "IDirect3D9",
    vtableSlot: 10,
    object: "0x330000",
    arguments: ["0x0", "0x1", "0x16", "0x200", "0x3", "0x16"],
  },
  {
    schema,
    sequence: 29,
    threadId: 10,
    kind: "api-leave",
    module: "d3d9.dll",
    api: "CheckDeviceFormat",
    interface: "IDirect3D9",
    vtableSlot: 10,
    object: "0x330000",
    result: "0x0",
  },
  {
    schema,
    sequence: 30,
    threadId: 10,
    kind: "api-enter",
    module: "d3d9.dll",
    api: "CreateDevice",
    interface: "IDirect3D9",
    vtableSlot: 16,
    object: "0x330000",
    arguments: ["0x0", "0x1", "0x550000", "0x44", "0x381000", "0x382000"],
  },
  {
    schema,
    sequence: 31,
    threadId: 10,
    kind: "api-leave",
    module: "d3d9.dll",
    api: "CreateDevice",
    interface: "IDirect3D9",
    vtableSlot: 16,
    object: "0x330000",
    result: "0x0",
    d3d9CreateDevice: {
      adapter: 0,
      deviceType: 1,
      focusWindow: "0x550000",
      behaviorFlags: 0x44,
      presentationParameters: {
        before: {
          pointer: "0x381000",
          size: 56,
          bytes: presentParametersBefore,
          fields: presentParametersBeforeFields,
        },
        after: {
          pointer: "0x381000",
          size: 56,
          bytes: presentParametersAfter,
          fields: presentParametersAfterFields,
        },
      },
      deviceOutput: {
        before: {
          pointer: "0x382000",
          size: 4,
          bytes: [0xcc, 0xcc, 0xcc, 0xcc],
          value: "0xcccccccc",
        },
        after: {
          pointer: "0x382000",
          size: 4,
          bytes: [0, 0, 0x39, 0],
          value: "0x390000",
        },
      },
      immediateConsumer: null,
    },
  },
  {
    schema,
    sequence: 32,
    threadId: 10,
    kind: "vtable-observed",
    interface: "IDirect3DDevice9",
    object: "0x390000",
    vtable: "0x76020000",
    slots: idirect3ddevice9MethodNames.map((name, slot) => ({
      slot,
      name,
      address: `0x7602${slot.toString(16).padStart(4, "0")}`,
    })),
  },
  {
    schema,
    sequence: 33,
    threadId: 10,
    kind: "api-enter",
    module: "d3d9.dll",
    api: "SetRenderState",
    interface: "IDirect3DDevice9",
    vtableSlot: 57,
    object: "0x390000",
    arguments: ["0x7", "0x1"],
  },
  {
    schema,
    sequence: 34,
    threadId: 10,
    kind: "api-leave",
    module: "d3d9.dll",
    api: "SetRenderState",
    interface: "IDirect3DDevice9",
    vtableSlot: 57,
    object: "0x390000",
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
    caller: {
      address: "0x0045b9d6",
      module: "abi-smoke.exe",
      rva: 0x5b9d6,
    },
    durationMs: 12,
    lastError: 0,
  },
  {
    callId: 10,
    parentCallId: null,
    enterSequence: 22,
    leaveSequence: 23,
    caller: {
      address: "0x00459da5",
      module: "abi-smoke.exe",
      rva: 0x59da5,
    },
    durationMs: 0,
    lastError: 0,
  },
  {
    callId: 11,
    parentCallId: null,
    enterSequence: 24,
    leaveSequence: 25,
    caller: {
      address: "0x0045b91c",
      module: "abi-smoke.exe",
      rva: 0x5b91c,
    },
    durationMs: 0,
    lastError: 0,
  },
  {
    callId: 12,
    parentCallId: null,
    enterSequence: 26,
    leaveSequence: 27,
    caller: {
      address: "0x0045b949",
      module: "abi-smoke.exe",
      rva: 0x5b949,
    },
    durationMs: 0,
    lastError: 0,
  },
  {
    callId: 13,
    parentCallId: null,
    enterSequence: 28,
    leaveSequence: 29,
    caller: {
      address: "0x0045ba69",
      module: "abi-smoke.exe",
      rva: 0x5ba69,
    },
    durationMs: 0,
    lastError: 0,
  },
  {
    callId: 14,
    parentCallId: null,
    enterSequence: 30,
    leaveSequence: 31,
    caller: {
      address: "0x0045aca5",
      module: "abi-smoke.exe",
      rva: 0x5aca5,
    },
    durationMs: 48,
    lastError: 0,
  },
  {
    callId: 15,
    parentCallId: null,
    enterSequence: 33,
    leaveSequence: 34,
    caller: {
      address: "0x0045ad00",
      module: "abi-smoke.exe",
      rva: 0x5ad00,
    },
    durationMs: 1,
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
  observations: ["d3d9-create-device"],
});
assert.deepEqual(summary.trace.selectedVtables, ["idirect3d9"]);
assert.deepEqual(summary.trace.sequence, {
  events: 35,
  first: 0,
  last: 34,
  contiguous: true,
});
assert.deepEqual(summary.trace.apiPairing, {
  contract: "call-id-v1",
  enters: 15,
  leaves: 15,
  pairs: 15,
  maxDepth: 2,
  balanced: true,
});
assert.equal(summary.modules.length, 2);
assert.equal(summary.apis.length, 14);
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
    object: "0x330000",
    vtable: "0x76010000",
    slots: idirect3d9MethodNames.map((name, slot) => ({ slot, name })),
  },
  {
    sequence: 32,
    interface: "IDirect3DDevice9",
    object: "0x390000",
    vtable: "0x76020000",
    slots: idirect3ddevice9MethodNames.map((name, slot) => ({ slot, name })),
  },
]);
assert.deepEqual(summary.d3d9.adapterDisplayModes, [
  {
    sequence: 22,
    callId: 10,
    caller: { module: "abi-smoke.exe", rva: 0x59da5 },
    adapter: 0,
    result: "0x0",
    lastError: 0,
    before: {
      pointer: "0x360000",
      size: 16,
      bytes: Array(16).fill(0xcc),
      fields: {
        width: 0xcccccccc,
        height: 0xcccccccc,
        refreshRate: 0xcccccccc,
        format: 0xcccccccc,
      },
    },
    after: {
      pointer: "0x360000",
      size: 16,
      bytes: [128, 7, 0, 0, 56, 4, 0, 0, 60, 0, 0, 0, 22, 0, 0, 0],
      fields: { width: 1920, height: 1080, refreshRate: 60, format: 22 },
    },
  },
]);
assert.deepEqual(summary.d3d9.adapterIdentifiers, [
  {
    sequence: 24,
    callId: 11,
    caller: { module: "abi-smoke.exe", rva: 0x5b91c },
    adapter: 0,
    flags: 0,
    result: "0x0",
    lastError: 0,
    changedBytes: adapterIdentifierBefore.reduce(
      (count, byte, index) =>
        count + (byte === adapterIdentifierAfter[index] ? 0 : 1),
      0,
    ),
    before: {
      pointer: "0x370000",
      size: 1100,
      bytesSha256: crypto
        .createHash("sha256")
        .update(Uint8Array.from(adapterIdentifierBefore))
        .digest("hex"),
      zeroByteCount: 0,
      driver: {
        text: null,
        byteLength: 512,
        nulTerminated: false,
        zeroPadded: false,
      },
      description: {
        text: null,
        byteLength: 512,
        nulTerminated: false,
        zeroPadded: false,
      },
      deviceName: {
        text: null,
        byteLength: 32,
        nulTerminated: false,
        zeroPadded: false,
      },
      driverVersion: {
        lowPart: 0xcccccccc,
        highPart: 0xcccccccc,
        hex: "0xcccccccccccccccc",
      },
      vendorId: 0xcccccccc,
      deviceId: 0xcccccccc,
      subSysId: 0xcccccccc,
      revision: 0xcccccccc,
      deviceIdentifier: "cccccccc-cccc-cccc-cccc-cccccccccccc",
      whqlLevel: 0xcccccccc,
    },
    after: {
      pointer: "0x370000",
      size: 1100,
      bytesSha256: crypto
        .createHash("sha256")
        .update(adapterIdentifierBytes)
        .digest("hex"),
      zeroByteCount: adapterIdentifierAfter.filter((byte) => byte === 0).length,
      driver: {
        text: "fixture.dll",
        byteLength: 11,
        nulTerminated: true,
        zeroPadded: true,
      },
      description: {
        text: "Fixture Adapter",
        byteLength: 15,
        nulTerminated: true,
        zeroPadded: true,
      },
      deviceName: {
        text: "\\\\.\\DISPLAY1",
        byteLength: 12,
        nulTerminated: true,
        zeroPadded: true,
      },
      driverVersion: {
        lowPart: 0x0004_0003,
        highPart: 0x0002_0001,
        hex: "0x0002000100040003",
      },
      vendorId: 0x10de,
      deviceId: 0x1c82,
      subSysId: 0x11aa_22bb,
      revision: 0xa1,
      deviceIdentifier: "11223344-5566-7788-99aa-bbccddeeff00",
      whqlLevel: 1,
    },
    immediateConsumer: null,
  },
]);
assert.deepEqual(summary.d3d9.deviceTypeChecks, [
  {
    sequence: 20,
    callId: 9,
    parentCallId: null,
    caller: { module: "abi-smoke.exe", rva: 0x5b9d6 },
    adapter: 0,
    deviceType: 1,
    adapterFormat: 22,
    backBufferFormat: 22,
    windowedRaw: 1,
    windowed: true,
    result: "0x0",
    lastError: 0,
  },
]);
assert.deepEqual(summary.d3d9.deviceFormatChecks, [
  {
    sequence: 28,
    callId: 13,
    parentCallId: null,
    caller: { module: "abi-smoke.exe", rva: 0x5ba69 },
    adapter: 0,
    deviceType: 1,
    adapterFormat: 22,
    usage: 0x200,
    resourceType: 3,
    checkFormat: 22,
    result: "0x0",
    lastError: 0,
  },
]);
assert.equal(summary.d3d9.deviceCaps.length, 1);
const [deviceCaps] = summary.d3d9.deviceCaps;
assert.equal(deviceCaps.sequence, 26);
assert.equal(deviceCaps.callId, 12);
assert.deepEqual(deviceCaps.caller, {
  module: "abi-smoke.exe",
  rva: 0x5b949,
});
assert.equal(deviceCaps.adapter, 0);
assert.equal(deviceCaps.deviceType, 1);
assert.equal(deviceCaps.callerStackPointerAfterReturn, "0x380000");
assert.equal(deviceCaps.outputOffsetAfterReturn, 0);
assert.equal(deviceCaps.result, "0x0");
assert.equal(deviceCaps.lastError, 0);
assert.equal(
  deviceCaps.changedBytes,
  deviceCapsBefore.reduce(
    (count, byte, index) =>
      count + (byte === deviceCapsAfter[index] ? 0 : 1),
    0,
  ),
);
assert.equal(deviceCaps.before.pointer, "0x380000");
assert.equal(deviceCaps.before.size, 304);
assert.equal(deviceCaps.before.zeroByteCount, 0);
assert.equal(deviceCaps.after.pointer, "0x380000");
assert.equal(deviceCaps.after.size, 304);
assert.equal(Object.keys(deviceCaps.after.fields).length, 76);
assert.deepEqual(deviceCaps.after.fields.deviceType, {
  offset: 0,
  type: "u32",
  raw: 1,
  value: 1,
});
assert.deepEqual(deviceCaps.after.fields.maxVertexW, {
  offset: 112,
  type: "f32",
  raw: 0x3f80_0000,
  value: 1,
});
assert.deepEqual(deviceCaps.after.fields.maxPointSize, {
  offset: 176,
  type: "f32",
  raw: 0x4280_0000,
  value: 64,
});
assert.deepEqual(deviceCaps.after.fields.vs20DynamicFlowControlDepth, {
  offset: 252,
  type: "i32",
  raw: 0xffff_ffff,
  value: -1,
});
assert.equal(deviceCaps.immediateConsumer, null);
assert.equal(summary.d3d9.createDevices.length, 1);
const [createDevice] = summary.d3d9.createDevices;
assert.equal(createDevice.sequence, 30);
assert.equal(createDevice.callId, 14);
assert.equal(createDevice.parentCallId, null);
assert.deepEqual(createDevice.caller, {
  module: "abi-smoke.exe",
  rva: 0x5aca5,
});
assert.equal(createDevice.adapter, 0);
assert.equal(createDevice.deviceType, 1);
assert.equal(createDevice.focusWindow, "0x550000");
assert.equal(createDevice.behaviorFlags, 0x44);
assert.equal(createDevice.result, "0x0");
assert.equal(createDevice.lastError, 0);
assert.equal(createDevice.presentationChangedBytes, 1);
assert.deepEqual(
  createDevice.presentationParameters.before.fields,
  presentParametersBeforeFields,
);
assert.deepEqual(
  createDevice.presentationParameters.after.fields,
  presentParametersAfterFields,
);
assert.equal(
  createDevice.presentationParameters.before.bytesSha256,
  crypto
    .createHash("sha256")
    .update(Uint8Array.from(presentParametersBefore))
    .digest("hex"),
);
assert.equal(createDevice.deviceOutputChangedBytes, 4);
assert.deepEqual(createDevice.deviceOutput.before, {
  pointer: "0x382000",
  size: 4,
  bytes: [0xcc, 0xcc, 0xcc, 0xcc],
  value: "0xcccccccc",
});
assert.deepEqual(createDevice.deviceOutput.after, {
  pointer: "0x382000",
  size: 4,
  bytes: [0, 0, 0x39, 0],
  value: "0x390000",
});
assert.equal(createDevice.returnedDevice, "0x390000");
assert.equal(createDevice.immediateConsumer, null);
assert.deepEqual(summary.d3d9.deviceCalls, [
  {
    sequence: 33,
    callId: 15,
    parentCallId: null,
    caller: { module: "abi-smoke.exe", rva: 0x5ad00 },
    slot: 57,
    method: "SetRenderState",
    object: "0x390000",
    arguments: ["0x7", "0x1"],
    result: "0x0",
    lastError: 0,
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
  { schema, sequence: 26, kind: "module-load", module: "late.dll", path: "late.dll" },
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

const inconsistentDisplayMode = structuredClone(events);
inconsistentDisplayMode.find(
  (event) => event.sequence === 23,
).d3d9DisplayMode.after.fields.width = 1919;
assert.throws(
  () => summarizeTrace(encode(inconsistentDisplayMode)),
  /inconsistent D3DDISPLAYMODE after bytes/,
);

const changedDisplayModePointer = structuredClone(events);
changedDisplayModePointer.find(
  (event) => event.sequence === 23,
).d3d9DisplayMode.after.pointer = "0x360010";
assert.throws(
  () => summarizeTrace(encode(changedDisplayModePointer)),
  /changed its D3DDISPLAYMODE output pointer/,
);

const truncatedAdapterIdentifier = structuredClone(events);
truncatedAdapterIdentifier
  .find((event) => event.sequence === 25)
  .d3d9AdapterIdentifier.after.bytes.pop();
assert.throws(
  () => summarizeTrace(encode(truncatedAdapterIdentifier)),
  /invalid D3DADAPTER_IDENTIFIER9 after snapshot/,
);

const changedAdapterIdentifierPointer = structuredClone(events);
changedAdapterIdentifierPointer.find(
  (event) => event.sequence === 25,
).d3d9AdapterIdentifier.after.pointer = "0x370010";
assert.throws(
  () => summarizeTrace(encode(changedAdapterIdentifierPointer)),
  /changed its D3DADAPTER_IDENTIFIER9 output pointer/,
);

const truncatedDeviceCaps = structuredClone(events);
truncatedDeviceCaps
  .find((event) => event.sequence === 27)
  .d3d9DeviceCaps.after.bytes.pop();
assert.throws(
  () => summarizeTrace(encode(truncatedDeviceCaps)),
  /invalid D3DCAPS9 after snapshot/,
);

const changedDeviceCapsPointer = structuredClone(events);
changedDeviceCapsPointer.find(
  (event) => event.sequence === 27,
).d3d9DeviceCaps.after.pointer = "0x380010";
assert.throws(
  () => summarizeTrace(encode(changedDeviceCapsPointer)),
  /changed its D3DCAPS9 output pointer/,
);

const truncatedPresentParameters = structuredClone(events);
truncatedPresentParameters
  .find((event) => event.sequence === 31)
  .d3d9CreateDevice.presentationParameters.after.bytes.pop();
assert.throws(
  () => summarizeTrace(encode(truncatedPresentParameters)),
  /invalid D3DPRESENT_PARAMETERS after snapshot/,
);

const inconsistentPresentParameters = structuredClone(events);
inconsistentPresentParameters.find(
  (event) => event.sequence === 31,
).d3d9CreateDevice.presentationParameters.after.fields.backBufferCount = 3;
assert.throws(
  () => summarizeTrace(encode(inconsistentPresentParameters)),
  /inconsistent D3DPRESENT_PARAMETERS after bytes/,
);

const changedPresentParametersPointer = structuredClone(events);
changedPresentParametersPointer.find(
  (event) => event.sequence === 31,
).d3d9CreateDevice.presentationParameters.after.pointer = "0x381010";
assert.throws(
  () => summarizeTrace(encode(changedPresentParametersPointer)),
  /changed its D3DPRESENT_PARAMETERS pointer/,
);

const inconsistentDeviceOutput = structuredClone(events);
inconsistentDeviceOutput.find(
  (event) => event.sequence === 31,
).d3d9CreateDevice.deviceOutput.after.value = "0x390004";
assert.throws(
  () => summarizeTrace(encode(inconsistentDeviceOutput)),
  /inconsistent pointer-slot after bytes/,
);

const changedDeviceOutputPointer = structuredClone(events);
changedDeviceOutputPointer.find(
  (event) => event.sequence === 31,
).d3d9CreateDevice.deviceOutput.after.pointer = "0x382004";
assert.throws(
  () => summarizeTrace(encode(changedDeviceOutputPointer)),
  /changed its CreateDevice output pointer/,
);

const truncatedDeviceVtable = structuredClone(events);
truncatedDeviceVtable.find((event) => event.sequence === 32).slots.pop();
assert.throws(
  () => summarizeTrace(encode(truncatedDeviceVtable)),
  /IDirect3DDevice9 vtable has invalid slot metadata/,
);

const truncatedDeviceMethodArguments = structuredClone(events);
truncatedDeviceMethodArguments.find((event) => event.sequence === 33).arguments.pop();
assert.throws(
  () => summarizeTrace(encode(truncatedDeviceMethodArguments)),
  /IDirect3DDevice9 api pair at sequence 33 is invalid/,
);

const truncatedDeviceTypeArguments = structuredClone(events);
truncatedDeviceTypeArguments
  .find((event) => event.sequence === 20)
  .arguments.pop();
assert.throws(
  () => summarizeTrace(encode(truncatedDeviceTypeArguments)),
  /invalid CheckDeviceType arguments/,
);

const overflowingDeviceTypeArgument = structuredClone(events);
overflowingDeviceTypeArgument.find((event) => event.sequence === 20).arguments[0] =
  "0x100000000";
assert.throws(
  () => summarizeTrace(encode(overflowingDeviceTypeArgument)),
  /invalid CheckDeviceType argument 0/,
);

const truncatedDeviceFormatArguments = structuredClone(events);
truncatedDeviceFormatArguments
  .find((event) => event.sequence === 28)
  .arguments.pop();
assert.throws(
  () => summarizeTrace(encode(truncatedDeviceFormatArguments)),
  /invalid CheckDeviceFormat arguments/,
);

const overflowingDeviceFormatArgument = structuredClone(events);
overflowingDeviceFormatArgument.find((event) => event.sequence === 28).arguments[3] =
  "0x100000000";
assert.throws(
  () => summarizeTrace(encode(overflowingDeviceFormatArgument)),
  /invalid CheckDeviceFormat argument 3/,
);

const wrongObservationSelection = structuredClone(events);
wrongObservationSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["startup-info"];
assert.throws(
  () => summarizeTrace(encode(wrongObservationSelection)),
  /trace-start and agent-ready selected observations differ/,
);

const heapLifecycleSelection = structuredClone(events);
heapLifecycleSelection[0].arguments.observations = ["heap-lifecycle"];
heapLifecycleSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["heap-lifecycle"];
assert.doesNotThrow(() => summarizeTrace(encode(heapLifecycleSelection)));

const heapAllocationSelection = structuredClone(events);
heapAllocationSelection[0].arguments.observations = ["heap-allocation"];
heapAllocationSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["heap-allocation"];
assert.doesNotThrow(() => summarizeTrace(encode(heapAllocationSelection)));

const moduleHandleSelection = structuredClone(events);
moduleHandleSelection[0].arguments.observations = ["module-handle"];
moduleHandleSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["module-handle"];
assert.doesNotThrow(() => summarizeTrace(encode(moduleHandleSelection)));

const dynamicExportSelection = structuredClone(events);
dynamicExportSelection[0].arguments.observations = ["dynamic-export"];
dynamicExportSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["dynamic-export"];
assert.doesNotThrow(() => summarizeTrace(encode(dynamicExportSelection)));

const flsLifecycleSelection = structuredClone(events);
flsLifecycleSelection[0].arguments.observations = ["fls-lifecycle"];
flsLifecycleSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["fls-lifecycle"];
assert.doesNotThrow(() => summarizeTrace(encode(flsLifecycleSelection)));

const criticalSectionSelection = structuredClone(events);
criticalSectionSelection[0].arguments.observations = ["critical-section"];
criticalSectionSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["critical-section"];
assert.doesNotThrow(() => summarizeTrace(encode(criticalSectionSelection)));

const interlockedIncrementSelection = structuredClone(events);
interlockedIncrementSelection[0].arguments.observations = ["interlocked-increment"];
interlockedIncrementSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["interlocked-increment"];
assert.doesNotThrow(() => summarizeTrace(encode(interlockedIncrementSelection)));

const interlockedDecrementSelection = structuredClone(events);
interlockedDecrementSelection[0].arguments.observations = ["interlocked-decrement"];
interlockedDecrementSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["interlocked-decrement"];
assert.doesNotThrow(() => summarizeTrace(encode(interlockedDecrementSelection)));

const getModuleFileNameASelection = structuredClone(events);
getModuleFileNameASelection[0].arguments.observations = ["get-module-file-name-a"];
getModuleFileNameASelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["get-module-file-name-a"];
assert.doesNotThrow(() => summarizeTrace(encode(getModuleFileNameASelection)));

const standardHandleSelection = structuredClone(events);
standardHandleSelection[0].arguments.observations = ["standard-handle"];
standardHandleSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["standard-handle"];
assert.doesNotThrow(() => summarizeTrace(encode(standardHandleSelection)));

const fileTypeSelection = structuredClone(events);
fileTypeSelection[0].arguments.observations = ["file-type"];
fileTypeSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["file-type"];
assert.doesNotThrow(() => summarizeTrace(encode(fileTypeSelection)));

const setHandleCountSelection = structuredClone(events);
setHandleCountSelection[0].arguments.observations = ["set-handle-count"];
setHandleCountSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["set-handle-count"];
assert.doesNotThrow(() => summarizeTrace(encode(setHandleCountSelection)));

const commandLineSelection = structuredClone(events);
commandLineSelection[0].arguments.observations = ["command-line-a"];
commandLineSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["command-line-a"];
assert.doesNotThrow(() => summarizeTrace(encode(commandLineSelection)));

const environmentStringsSelection = structuredClone(events);
environmentStringsSelection[0].arguments.observations = ["environment-strings-w"];
environmentStringsSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["environment-strings-w"];
assert.doesNotThrow(() => summarizeTrace(encode(environmentStringsSelection)));

const wideCharSelection = structuredClone(events);
wideCharSelection[0].arguments.observations = ["wide-char-to-multi-byte"];
wideCharSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["wide-char-to-multi-byte"];
assert.doesNotThrow(() => summarizeTrace(encode(wideCharSelection)));

const multiByteSelection = structuredClone(events);
multiByteSelection[0].arguments.observations = ["multi-byte-to-wide-char"];
multiByteSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["multi-byte-to-wide-char"];
assert.doesNotThrow(() => summarizeTrace(encode(multiByteSelection)));

const setCurrentDirectorySelection = structuredClone(events);
setCurrentDirectorySelection[0].arguments.observations = [
  "set-current-directory-a",
];
setCurrentDirectorySelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["set-current-directory-a"];
assert.doesNotThrow(() => summarizeTrace(encode(setCurrentDirectorySelection)));

const createMutexSelection = structuredClone(events);
createMutexSelection[0].arguments.observations = ["create-mutex-a"];
createMutexSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["create-mutex-a"];
assert.doesNotThrow(() => summarizeTrace(encode(createMutexSelection)));

const createWaitableTimerSelection = structuredClone(events);
createWaitableTimerSelection[0].arguments.observations = [
  "create-waitable-timer-a",
];
createWaitableTimerSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["create-waitable-timer-a"];
assert.doesNotThrow(() => summarizeTrace(encode(createWaitableTimerSelection)));

const loadIconSelection = structuredClone(events);
loadIconSelection[0].arguments.observations = ["load-icon-a"];
loadIconSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["load-icon-a"];
assert.doesNotThrow(() => summarizeTrace(encode(loadIconSelection)));

const loadImageSelection = structuredClone(events);
loadImageSelection[0].arguments.observations = ["load-image-a"];
loadImageSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["load-image-a"];
assert.doesNotThrow(() => summarizeTrace(encode(loadImageSelection)));

const sendMessageSelection = structuredClone(events);
sendMessageSelection[0].arguments.observations = ["send-message-a"];
sendMessageSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["send-message-a"];
assert.doesNotThrow(() => summarizeTrace(encode(sendMessageSelection)));

const setWindowPosSelection = structuredClone(events);
setWindowPosSelection[0].arguments.observations = ["set-window-pos"];
setWindowPosSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["set-window-pos"];
assert.doesNotThrow(() => summarizeTrace(encode(setWindowPosSelection)));

const loadCursorSelection = structuredClone(events);
loadCursorSelection[0].arguments.observations = ["load-cursor-a"];
loadCursorSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["load-cursor-a"];
assert.doesNotThrow(() => summarizeTrace(encode(loadCursorSelection)));

const registerClassSelection = structuredClone(events);
registerClassSelection[0].arguments.observations = ["register-class-ex-a"];
registerClassSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["register-class-ex-a"];
assert.doesNotThrow(() => summarizeTrace(encode(registerClassSelection)));

const getSystemMetricsSelection = structuredClone(events);
getSystemMetricsSelection[0].arguments.observations = ["get-system-metrics"];
getSystemMetricsSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["get-system-metrics"];
assert.doesNotThrow(() => summarizeTrace(encode(getSystemMetricsSelection)));

const getClientRectSelection = structuredClone(events);
getClientRectSelection[0].arguments.observations = ["get-client-rect"];
getClientRectSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["get-client-rect"];
assert.doesNotThrow(() => summarizeTrace(encode(getClientRectSelection)));

const windowDcSelection = structuredClone(events);
windowDcSelection[0].arguments.observations = ["window-dc"];
windowDcSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["window-dc"];
assert.doesNotThrow(() => summarizeTrace(encode(windowDcSelection)));

const setWindowLongASelection = structuredClone(events);
setWindowLongASelection[0].arguments.observations = ["set-window-long-a"];
setWindowLongASelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["set-window-long-a"];
assert.doesNotThrow(() => summarizeTrace(encode(setWindowLongASelection)));

const setWindowPosSizeSelection = structuredClone(events);
setWindowPosSizeSelection[0].arguments.observations = ["set-window-pos-size"];
setWindowPosSizeSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["set-window-pos-size"];
assert.doesNotThrow(() => summarizeTrace(encode(setWindowPosSizeSelection)));

const getStringTypeSelection = structuredClone(events);
getStringTypeSelection[0].arguments.observations = ["get-string-type-w"];
getStringTypeSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["get-string-type-w"];
assert.doesNotThrow(() => summarizeTrace(encode(getStringTypeSelection)));

const lcMapStringSelection = structuredClone(events);
lcMapStringSelection[0].arguments.observations = ["lc-map-string-w"];
lcMapStringSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["lc-map-string-w"];
assert.doesNotThrow(() => summarizeTrace(encode(lcMapStringSelection)));

const getLastErrorSelection = structuredClone(events);
getLastErrorSelection[0].arguments.observations = ["get-last-error"];
getLastErrorSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["get-last-error"];
assert.doesNotThrow(() => summarizeTrace(encode(getLastErrorSelection)));

const setLastErrorSelection = structuredClone(events);
setLastErrorSelection[0].arguments.observations = ["set-last-error"];
setLastErrorSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["set-last-error"];
assert.doesNotThrow(() => summarizeTrace(encode(setLastErrorSelection)));

const getAcpSelection = structuredClone(events);
getAcpSelection[0].arguments.observations = ["get-acp"];
getAcpSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["get-acp"];
assert.doesNotThrow(() => summarizeTrace(encode(getAcpSelection)));

const systemDefaultLangIdSelection = structuredClone(events);
systemDefaultLangIdSelection[0].arguments.observations = [
  "system-default-lang-id",
];
systemDefaultLangIdSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["system-default-lang-id"];
assert.doesNotThrow(() => summarizeTrace(encode(systemDefaultLangIdSelection)));

const driveTypeSelection = structuredClone(events);
driveTypeSelection[0].arguments.observations = ["drive-type"];
driveTypeSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["drive-type"];
assert.doesNotThrow(() => summarizeTrace(encode(driveTypeSelection)));

const swapMouseButtonSelection = structuredClone(events);
swapMouseButtonSelection[0].arguments.observations = ["swap-mouse-button"];
swapMouseButtonSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["swap-mouse-button"];
assert.doesNotThrow(() => summarizeTrace(encode(swapMouseButtonSelection)));

const timeGetDevCapsSelection = structuredClone(events);
timeGetDevCapsSelection[0].arguments.observations = ["time-get-dev-caps"];
timeGetDevCapsSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["time-get-dev-caps"];
assert.doesNotThrow(() => summarizeTrace(encode(timeGetDevCapsSelection)));

const getVersionExASelection = structuredClone(events);
getVersionExASelection[0].arguments.observations = ["get-version-ex-a"];
getVersionExASelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["get-version-ex-a"];
assert.doesNotThrow(() => summarizeTrace(encode(getVersionExASelection)));

const getCpInfoSelection = structuredClone(events);
getCpInfoSelection[0].arguments.observations = ["get-cp-info"];
getCpInfoSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["get-cp-info"];
assert.doesNotThrow(() => summarizeTrace(encode(getCpInfoSelection)));

const processorFeatureSelection = structuredClone(events);
processorFeatureSelection[0].arguments.observations = ["processor-feature"];
processorFeatureSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["processor-feature"];
assert.doesNotThrow(() => summarizeTrace(encode(processorFeatureSelection)));

const unhandledExceptionFilterSelection = structuredClone(events);
unhandledExceptionFilterSelection[0].arguments.observations = [
  "unhandled-exception-filter",
];
unhandledExceptionFilterSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["unhandled-exception-filter"];
assert.doesNotThrow(() => summarizeTrace(encode(unhandledExceptionFilterSelection)));

const comInitializationSelection = structuredClone(events);
comInitializationSelection[0].arguments.observations = ["com-initialization"];
comInitializationSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["com-initialization"];
assert.doesNotThrow(() => summarizeTrace(encode(comInitializationSelection)));

const heapSizeSelection = structuredClone(events);
heapSizeSelection[0].arguments.observations = ["heap-size"];
heapSizeSelection.find(
  (event) => event.kind === "agent-ready",
).selectedObservations = ["heap-size"];
assert.doesNotThrow(() => summarizeTrace(encode(heapSizeSelection)));

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
