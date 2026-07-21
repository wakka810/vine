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
  idirect3dsurface9MethodNames,
  idirect3dtexture9MethodNames,
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
assert.deepEqual(summary.d3d9.createTextures, []);
assert.deepEqual(summary.d3d9.getSurfaceLevels, []);
assert.deepEqual(summary.d3d9.textureCalls, []);
assert.deepEqual(summary.d3d9.surfaceCalls, []);
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

const pointerSlot = (pointer, value, bytes) => ({
  pointer,
  size: 4,
  bytes,
  value,
});
const vtableSlots = (names, base) =>
  names.map((name, slot) => ({
    slot,
    name,
    address: `0x${(base + slot * 0x10).toString(16)}`,
  }));
const d3d9ResourceEvents = [
  {
    schema,
    kind: "trace-start",
    timestamp: "2026-07-20T00:00:00.000Z",
    executable: "C:\\fixture\\BGI.exe",
    executableBytes: 1_193_472,
    executableSha256: "b".repeat(64),
    python: "3.14.6",
    frida: "17.15.4",
    platform: "Windows-11-test",
    arguments: {
      seconds: 10,
      killAfter: true,
      stopOnEvent: "d3d9-get-surface-level-complete",
      vtables: ["idirect3d9"],
      observations: ["d3d9-get-surface-level"],
    },
  },
  {
    schema,
    sequence: 0,
    kind: "module-load",
    module: "BGI.exe",
    path: "C:\\fixture\\BGI.exe",
  },
  {
    schema,
    sequence: 1,
    kind: "module-load",
    module: "d3d9.dll",
    path: "C:\\Windows\\System32\\d3d9.dll",
  },
  {
    schema,
    sequence: 2,
    kind: "agent-ready",
    architecture: "ia32",
    pointerSize: 4,
    processId: 456,
    selectedVtables: ["idirect3d9"],
    selectedObservations: ["d3d9-get-surface-level"],
  },
  {
    schema,
    sequence: 3,
    kind: "vtable-observed",
    interface: "IDirect3DDevice9",
    object: "0x490000",
    vtable: "0x76020000",
    slots: vtableSlots(idirect3ddevice9MethodNames, 0x7602_0000),
  },
  {
    schema,
    sequence: 4,
    threadId: 20,
    kind: "api-enter",
    callId: 1,
    parentCallId: null,
    caller: {
      address: "0x0045a715",
      module: "BGI.exe",
      rva: 0x5a715,
    },
    module: "d3d9.dll",
    api: "CreateTexture",
    interface: "IDirect3DDevice9",
    vtableSlot: 23,
    object: "0x490000",
    arguments: [
      "0x400",
      "0x400",
      "0x1",
      "0x0",
      "0x16",
      "0x2",
      "0x6ffd40",
      "0x0",
    ],
  },
  {
    schema,
    sequence: 5,
    threadId: 20,
    kind: "api-leave",
    callId: 1,
    parentCallId: null,
    caller: {
      address: "0x0045a715",
      module: "BGI.exe",
      rva: 0x5a715,
    },
    durationMs: 1,
    lastError: 0,
    module: "d3d9.dll",
    api: "CreateTexture",
    interface: "IDirect3DDevice9",
    vtableSlot: 23,
    object: "0x490000",
    result: "0x0",
    d3d9CreateTexture: {
      textureOutput: {
        before: pointerSlot("0x6ffd40", "0xcccccccc", [0xcc, 0xcc, 0xcc, 0xcc]),
        after: pointerSlot("0x6ffd40", "0x500000", [0, 0, 0x50, 0]),
      },
      immediateConsumer: null,
    },
  },
  {
    schema,
    sequence: 6,
    kind: "vtable-observed",
    interface: "IDirect3DTexture9",
    object: "0x500000",
    vtable: "0x76030000",
    slots: vtableSlots(idirect3dtexture9MethodNames, 0x7603_0000),
  },
  {
    schema,
    sequence: 7,
    threadId: 20,
    kind: "api-enter",
    callId: 2,
    parentCallId: null,
    caller: {
      address: "0x0045a731",
      module: "BGI.exe",
      rva: 0x5a731,
    },
    module: "d3d9.dll",
    api: "GetSurfaceLevel",
    interface: "IDirect3DTexture9",
    vtableSlot: 18,
    object: "0x500000",
    arguments: ["0x0", "0x6ffd38"],
  },
  {
    schema,
    sequence: 8,
    threadId: 20,
    kind: "api-leave",
    callId: 2,
    parentCallId: null,
    caller: {
      address: "0x0045a731",
      module: "BGI.exe",
      rva: 0x5a731,
    },
    durationMs: 1,
    lastError: 0,
    module: "d3d9.dll",
    api: "GetSurfaceLevel",
    interface: "IDirect3DTexture9",
    vtableSlot: 18,
    object: "0x500000",
    result: "0x0",
    d3d9GetSurfaceLevel: {
      level: 0,
      surfaceOutput: {
        before: pointerSlot("0x6ffd38", "0xcccccccc", [0xcc, 0xcc, 0xcc, 0xcc]),
        after: pointerSlot("0x6ffd38", "0x510000", [0, 0, 0x51, 0]),
      },
      immediateConsumer: null,
    },
  },
  {
    schema,
    sequence: 9,
    kind: "vtable-observed",
    interface: "IDirect3DSurface9",
    object: "0x510000",
    vtable: "0x76040000",
    slots: vtableSlots(idirect3dsurface9MethodNames, 0x7604_0000),
  },
  {
    schema,
    sequence: 10,
    threadId: 20,
    kind: "api-enter",
    callId: 3,
    parentCallId: null,
    caller: {
      address: "0x0045a746",
      module: "BGI.exe",
      rva: 0x5a746,
    },
    module: "d3d9.dll",
    api: "GetDC",
    interface: "IDirect3DSurface9",
    vtableSlot: 15,
    object: "0x510000",
    arguments: ["0x6ffd3c"],
  },
  {
    schema,
    sequence: 11,
    threadId: 20,
    kind: "api-leave",
    callId: 3,
    parentCallId: null,
    caller: {
      address: "0x0045a746",
      module: "BGI.exe",
      rva: 0x5a746,
    },
    durationMs: 1,
    lastError: 0,
    module: "d3d9.dll",
    api: "GetDC",
    interface: "IDirect3DSurface9",
    vtableSlot: 15,
    object: "0x510000",
    result: "0x0",
  },
  {
    schema,
    sequence: 12,
    kind: "d3d9-get-surface-level-complete",
    interface: "IDirect3DSurface9",
    vtableSlot: 15,
    method: "GetDC",
    result: "0x0",
  },
  {
    schema,
    kind: "trace-stop",
    timestamp: "2026-07-20T00:00:01.000Z",
    reason: "event-d3d9-get-surface-level-complete-kill",
    detachedReason: "process-terminated",
    crash: null,
  },
];
const d3d9ResourceSummary = summarizeTrace(
  encode(d3d9ResourceEvents),
  "d3d9-resource-fixture.jsonl",
);
assert.deepEqual(
  d3d9ResourceSummary.vtables.map((vtable) => vtable.interface),
  ["IDirect3DDevice9", "IDirect3DTexture9", "IDirect3DSurface9"],
);
assert.equal(d3d9ResourceSummary.d3d9.createTextures.length, 1);
assert.equal(d3d9ResourceSummary.d3d9.createTextures[0].returnedTexture, "0x500000");
assert.equal(d3d9ResourceSummary.d3d9.getSurfaceLevels.length, 1);
assert.deepEqual(d3d9ResourceSummary.d3d9.getSurfaceLevels[0], {
  sequence: 7,
  callId: 2,
  parentCallId: null,
  caller: { module: "BGI.exe", rva: 0x5a731 },
  object: "0x500000",
  level: 0,
  result: "0x0",
  lastError: 0,
  outputChangedBytes: 4,
  surfaceOutput: {
    before: pointerSlot("0x6ffd38", "0xcccccccc", [0xcc, 0xcc, 0xcc, 0xcc]),
    after: pointerSlot("0x6ffd38", "0x510000", [0, 0, 0x51, 0]),
  },
  returnedSurface: "0x510000",
  immediateConsumer: null,
});
assert.equal(d3d9ResourceSummary.d3d9.textureCalls[0].method, "GetSurfaceLevel");
assert.equal(d3d9ResourceSummary.d3d9.surfaceCalls[0].method, "GetDC");

const mismatchedSurfaceOutput = structuredClone(d3d9ResourceEvents);
mismatchedSurfaceOutput.find(
  (event) => event.kind === "api-enter" && event.api === "GetSurfaceLevel",
).arguments[1] = "0x6ffd34";
assert.throws(
  () => summarizeTrace(encode(mismatchedSurfaceOutput)),
  /changed its GetSurfaceLevel output pointer/,
);

const d3d9InitialTextureEvents = [
  {
    schema,
    kind: "trace-start",
    timestamp: "2026-07-22T00:00:00.000Z",
    executable: "C:\\fixture\\BGI.exe",
    executableBytes: 1_193_472,
    executableSha256: "c".repeat(64),
    python: "3.14.6",
    frida: "17.15.4",
    platform: "Windows-11-test",
    arguments: {
      seconds: 10,
      killAfter: true,
      stopOnEvent: "d3d9-initial-textures-complete",
      vtables: ["idirect3d9"],
      observations: ["d3d9-initial-textures"],
    },
  },
  {
    schema,
    sequence: 0,
    kind: "module-load",
    module: "BGI.exe",
    path: "C:\\fixture\\BGI.exe",
  },
  {
    schema,
    sequence: 1,
    kind: "module-load",
    module: "d3d9.dll",
    path: "C:\\Windows\\System32\\d3d9.dll",
  },
  {
    schema,
    sequence: 2,
    kind: "agent-ready",
    architecture: "ia32",
    pointerSize: 4,
    processId: 456,
    selectedVtables: ["idirect3d9"],
    selectedObservations: ["d3d9-initial-textures"],
  },
  {
    schema,
    sequence: 3,
    kind: "vtable-observed",
    interface: "IDirect3DDevice9",
    object: "0x490000",
    vtable: "0x76020000",
    slots: vtableSlots(idirect3ddevice9MethodNames, 0x7602_0000),
  },
];
const initialTextureSnapshots = [];
for (const [index, fixture] of [
  {
    rva: 0x5a715,
    usage: "0x0",
    pool: "0x2",
    output: "0x6ffd40",
    returned: "0x500000",
    returnedBytes: [0, 0, 0x50, 0],
  },
  {
    rva: 0x5a7b7,
    usage: "0x0",
    pool: "0x0",
    output: "0x6ffd44",
    returned: "0x510000",
    returnedBytes: [0, 0, 0x51, 0],
  },
  {
    rva: 0x5a7dc,
    usage: "0x200",
    pool: "0x0",
    output: "0x6ffd48",
    returned: "0x520000",
    returnedBytes: [0, 0, 0x52, 0],
  },
].entries()) {
  const callId = index + 1;
  const caller = {
    address: `0x${(0x0040_0000 + fixture.rva).toString(16)}`,
    module: "BGI.exe",
    rva: fixture.rva,
  };
  const arguments_ = [
    "0x400",
    "0x400",
    "0x1",
    fixture.usage,
    "0x16",
    fixture.pool,
    fixture.output,
    "0x0",
  ];
  const observation = {
    textureOutput: {
      before: pointerSlot(fixture.output, "0xcccccccc", [0xcc, 0xcc, 0xcc, 0xcc]),
      after: pointerSlot(fixture.output, fixture.returned, fixture.returnedBytes),
    },
    immediateConsumer: null,
  };
  d3d9InitialTextureEvents.push(
    {
      schema,
      sequence: 4 + index * 2,
      threadId: 20,
      kind: "api-enter",
      callId,
      parentCallId: null,
      caller,
      module: "d3d9.dll",
      api: "CreateTexture",
      interface: "IDirect3DDevice9",
      vtableSlot: 23,
      object: "0x490000",
      arguments: arguments_,
    },
    {
      schema,
      sequence: 5 + index * 2,
      threadId: 20,
      kind: "api-leave",
      callId,
      parentCallId: null,
      caller,
      durationMs: 1,
      lastError: 0,
      module: "d3d9.dll",
      api: "CreateTexture",
      interface: "IDirect3DDevice9",
      vtableSlot: 23,
      object: "0x490000",
      result: "0x0",
      d3d9CreateTexture: observation,
    },
  );
  initialTextureSnapshots.push({
    callId,
    api: "CreateTexture",
    interface: "IDirect3DDevice9",
    vtableSlot: 23,
    object: "0x490000",
    arguments: arguments_,
    result: "0x0",
    lastError: 0,
    d3d9CreateTexture: observation,
    caller,
  });
}
d3d9InitialTextureEvents.push(
  {
    schema,
    sequence: 10,
    kind: "d3d9-initial-textures-complete",
    profile: "x86-bgi-initial-textures-v1",
    device: "0x490000",
    calls: initialTextureSnapshots,
  },
  {
    schema,
    kind: "trace-stop",
    timestamp: "2026-07-22T00:00:01.000Z",
    reason: "event-d3d9-initial-textures-complete-kill",
    detachedReason: "process-terminated",
    crash: null,
  },
);
const d3d9InitialTextureSummary = summarizeTrace(
  encode(d3d9InitialTextureEvents),
  "d3d9-initial-textures-fixture.jsonl",
);
assert.equal(d3d9InitialTextureSummary.d3d9.createTextures.length, 3);
assert.equal(d3d9InitialTextureSummary.d3d9.initialTextureSequences.length, 1);
assert.deepEqual(
  d3d9InitialTextureSummary.d3d9.initialTextureSequences[0].calls.map((call) => ({
    caller: call.caller,
    arguments: call.arguments.slice(0, 6),
    returnedTexture: call.returnedTexture,
  })),
  [
    {
      caller: { module: "BGI.exe", rva: 0x5a715 },
      arguments: ["0x400", "0x400", "0x1", "0x0", "0x16", "0x2"],
      returnedTexture: "0x500000",
    },
    {
      caller: { module: "BGI.exe", rva: 0x5a7b7 },
      arguments: ["0x400", "0x400", "0x1", "0x0", "0x16", "0x0"],
      returnedTexture: "0x510000",
    },
    {
      caller: { module: "BGI.exe", rva: 0x5a7dc },
      arguments: ["0x400", "0x400", "0x1", "0x200", "0x16", "0x0"],
      returnedTexture: "0x520000",
    },
  ],
);
const missingInitialTextureCompletion = d3d9InitialTextureEvents.filter(
  (event) => event.kind !== "d3d9-initial-textures-complete",
);
assert.throws(
  () => summarizeTrace(encode(missingInitialTextureCompletion)),
  /requires exactly one structured completion/,
);
const malformedInitialTextureTuple = structuredClone(d3d9InitialTextureEvents);
malformedInitialTextureTuple.find(
  (event) => event.kind === "api-enter" && event.callId === 2,
).arguments[5] = "0x2";
malformedInitialTextureTuple.find(
  (event) => event.kind === "d3d9-initial-textures-complete",
).calls[1].arguments[5] = "0x2";
assert.throws(
  () => summarizeTrace(encode(malformedInitialTextureTuple)),
  /violates its caller, tuple, or output contract/,
);

const d3d9SurfaceDcEvents = structuredClone(d3d9ResourceEvents);
d3d9SurfaceDcEvents[0].arguments.stopOnEvent = "d3d9-surface-get-dc-complete";
d3d9SurfaceDcEvents[0].arguments.observations = ["d3d9-surface-get-dc"];
d3d9SurfaceDcEvents.find((event) => event.kind === "agent-ready").selectedObservations = [
  "d3d9-surface-get-dc",
];
const getDcLeave = d3d9SurfaceDcEvents.find(
  (event) => event.kind === "api-leave" && event.api === "GetDC",
);
const getDcObservation = {
  output: {
    before: pointerSlot("0x6ffd3c", "0x400", [0, 4, 0, 0]),
    after: pointerSlot("0x6ffd3c", "0x52010a1e", [0x1e, 0x0a, 0x01, 0x52]),
  },
  immediateConsumer: null,
};
getDcLeave.d3d9SurfaceGetDc = getDcObservation;
const fillRectSnapshot = {
  profile: "x86-rect-v1",
  raw: {
    pointer: "0x6ffd50",
    requestedBytes: 16,
    maximumBytes: 64,
    bytes: [0, 0, 0, 0, 0, 0, 0, 0, 0x20, 3, 0, 0, 0x58, 2, 0, 0],
  },
  fields: { left: 0, top: 0, right: 800, bottom: 600 },
};
const surfaceDcContinuation = [
  {
    schema,
    sequence: 12,
    threadId: 20,
    kind: "api-enter",
    callId: 4,
    parentCallId: null,
    caller: { address: "0x0045a76c", module: "BGI.exe", rva: 0x5a76c },
    module: "GDI32.dll",
    api: "GetStockObject",
    arguments: [4],
    d3d9SurfaceDcGdi: { rect: null, immediateConsumer: null },
  },
  {
    schema,
    sequence: 13,
    threadId: 20,
    kind: "api-leave",
    callId: 4,
    parentCallId: null,
    caller: { address: "0x0045a76c", module: "BGI.exe", rva: 0x5a76c },
    durationMs: 0,
    lastError: 0,
    module: "GDI32.dll",
    api: "GetStockObject",
    result: "0x900011",
    d3d9SurfaceDcGdi: { rect: null },
  },
  {
    schema,
    sequence: 14,
    threadId: 20,
    kind: "api-enter",
    callId: 5,
    parentCallId: null,
    caller: { address: "0x0045a77d", module: "BGI.exe", rva: 0x5a77d },
    module: "USER32.dll",
    api: "FillRect",
    arguments: ["0x52010a1e", "0x6ffd50", "0x900011"],
    d3d9SurfaceDcGdi: { rect: fillRectSnapshot, immediateConsumer: null },
  },
  {
    schema,
    sequence: 15,
    threadId: 20,
    kind: "api-leave",
    callId: 5,
    parentCallId: null,
    caller: { address: "0x0045a77d", module: "BGI.exe", rva: 0x5a77d },
    durationMs: 1,
    lastError: 0,
    module: "USER32.dll",
    api: "FillRect",
    result: "0x1",
    d3d9SurfaceDcGdi: { rect: fillRectSnapshot },
  },
  {
    schema,
    sequence: 16,
    threadId: 20,
    kind: "api-enter",
    callId: 6,
    parentCallId: null,
    caller: { address: "0x0045a78e", module: "BGI.exe", rva: 0x5a78e },
    module: "d3d9.dll",
    api: "ReleaseDC",
    interface: "IDirect3DSurface9",
    vtableSlot: 16,
    object: "0x510000",
    arguments: ["0x52010a1e"],
  },
  {
    schema,
    sequence: 17,
    threadId: 20,
    kind: "api-leave",
    callId: 6,
    parentCallId: null,
    caller: { address: "0x0045a78e", module: "BGI.exe", rva: 0x5a78e },
    durationMs: 0,
    lastError: 0,
    module: "d3d9.dll",
    api: "ReleaseDC",
    interface: "IDirect3DSurface9",
    vtableSlot: 16,
    object: "0x510000",
    result: "0x0",
  },
  {
    schema,
    sequence: 18,
    threadId: 20,
    kind: "api-enter",
    callId: 7,
    parentCallId: null,
    caller: { address: "0x0045a79a", module: "BGI.exe", rva: 0x5a79a },
    module: "d3d9.dll",
    api: "Release",
    interface: "IDirect3DSurface9",
    vtableSlot: 2,
    object: "0x510000",
    arguments: [],
  },
  {
    schema,
    sequence: 19,
    threadId: 20,
    kind: "api-leave",
    callId: 7,
    parentCallId: null,
    caller: { address: "0x0045a79a", module: "BGI.exe", rva: 0x5a79a },
    durationMs: 0,
    lastError: 0,
    module: "d3d9.dll",
    api: "Release",
    interface: "IDirect3DSurface9",
    vtableSlot: 2,
    object: "0x510000",
    result: "0x1",
  },
];
const surfaceDcCompletion = {
  schema,
  sequence: 20,
  kind: "d3d9-surface-get-dc-complete",
  profile: "x86-bgi-system-memory-surface-dc-fill-v1",
  surface: "0x510000",
  calls: [
    {
      api: "GetDC",
      interface: "IDirect3DSurface9",
      vtableSlot: 15,
      object: "0x510000",
      arguments: ["0x6ffd3c"],
      result: "0x0",
      lastError: 0,
      d3d9SurfaceGetDc: getDcObservation,
      caller: { address: "0x0045a746", module: "BGI.exe", rva: 0x5a746 },
    },
    {
      api: "GetStockObject",
      module: "GDI32.dll",
      arguments: [4],
      result: "0x900011",
      lastError: 0,
      rect: null,
      caller: { address: "0x0045a76c", module: "BGI.exe", rva: 0x5a76c },
    },
    {
      api: "FillRect",
      module: "USER32.dll",
      arguments: ["0x52010a1e", "0x6ffd50", "0x900011"],
      result: "0x1",
      lastError: 0,
      rect: fillRectSnapshot,
      caller: { address: "0x0045a77d", module: "BGI.exe", rva: 0x5a77d },
    },
    {
      api: "ReleaseDC",
      interface: "IDirect3DSurface9",
      vtableSlot: 16,
      object: "0x510000",
      arguments: ["0x52010a1e"],
      result: "0x0",
      lastError: 0,
      caller: { address: "0x0045a78e", module: "BGI.exe", rva: 0x5a78e },
    },
    {
      api: "Release",
      interface: "IDirect3DSurface9",
      vtableSlot: 2,
      object: "0x510000",
      arguments: [],
      result: "0x1",
      lastError: 0,
      caller: { address: "0x0045a79a", module: "BGI.exe", rva: 0x5a79a },
    },
  ],
};
const oldResourceCompletionIndex = d3d9SurfaceDcEvents.findIndex(
  (event) => event.kind === "d3d9-get-surface-level-complete",
);
d3d9SurfaceDcEvents.splice(
  oldResourceCompletionIndex,
  1,
  ...surfaceDcContinuation,
  surfaceDcCompletion,
);
d3d9SurfaceDcEvents.at(-1).reason = "event-d3d9-surface-get-dc-complete-kill";
const d3d9SurfaceDcSummary = summarizeTrace(
  encode(d3d9SurfaceDcEvents),
  "d3d9-surface-get-dc-fixture.jsonl",
);
assert.equal(d3d9SurfaceDcSummary.d3d9.surfaceDcFlows.length, 1);
const surfaceDcFlow = d3d9SurfaceDcSummary.d3d9.surfaceDcFlows[0];
assert.equal(surfaceDcFlow.sequence, 20);
assert.equal(surfaceDcFlow.surface, "0x510000");
assert.equal(surfaceDcFlow.hdc, "0x52010a1e");
assert.equal(surfaceDcFlow.stockBrush, "0x900011");
assert.deepEqual(surfaceDcFlow.rect, { left: 0, top: 0, right: 800, bottom: 600 });
assert.deepEqual(
  surfaceDcFlow.calls.map((call) => [call.method, call.caller.rva, call.result]),
  [
    ["GetDC", 0x5a746, "0x0"],
    ["GetStockObject", 0x5a76c, "0x900011"],
    ["FillRect", 0x5a77d, "0x1"],
    ["ReleaseDC", 0x5a78e, "0x0"],
    ["Release", 0x5a79a, "0x1"],
  ],
);
assert.deepEqual(surfaceDcFlow.getDcOutput.after, getDcObservation.output.after);

const mismatchedSurfaceDcCompletion = structuredClone(d3d9SurfaceDcEvents);
mismatchedSurfaceDcCompletion.find(
  (event) => event.kind === "d3d9-surface-get-dc-complete",
).calls[3].arguments[0] = "0x52010a1f";
assert.throws(
  () => summarizeTrace(encode(mismatchedSurfaceDcCompletion)),
  /completion call 3 does not match its paired trace/,
);

const mismatchedSurfaceDcIdentity = structuredClone(d3d9SurfaceDcEvents);
mismatchedSurfaceDcIdentity.find(
  (event) => event.kind === "api-enter" && event.api === "ReleaseDC",
).arguments[0] = "0x52010a1f";
mismatchedSurfaceDcIdentity.find(
  (event) => event.kind === "d3d9-surface-get-dc-complete",
).calls[3].arguments[0] = "0x52010a1f";
assert.throws(
  () => summarizeTrace(encode(mismatchedSurfaceDcIdentity)),
  /surface, HDC, brush, or fill identity contract/,
);

const missingSurfaceDcCompletion = d3d9SurfaceDcEvents.filter(
  (event) => event.kind !== "d3d9-surface-get-dc-complete",
);
assert.throws(
  () => summarizeTrace(encode(missingSurfaceDcCompletion)),
  /requires exactly one structured completion/,
);

const timeGetTimeConsumerInstructions = [
  ["0x121ec7", "mov", "ecx, dword ptr [0x1e2358]"],
  ["0x121ecd", "cmp", "eax, ecx"],
  ["0x121ecf", "jb", "0x121edd"],
  ["0x121ed1", "mov", "edx, dword ptr [0x188dd8]"],
  ["0x121ed7", "add", "edx, ecx"],
  ["0x121ed9", "cmp", "edx, eax"],
  ["0x121edb", "jae", "0x121ee7"],
  ["0x121edd", "mov", "edx, eax"],
  ["0x121edf", "sub", "edx, ecx"],
  ["0x121ee1", "add", "dword ptr [0x1e2360], edx"],
  ["0x121ee7", "mov", "ecx, dword ptr [0x1e235c]"],
  ["0x121eed", "mov", "dword ptr [0x1e2358], eax"],
  ["0x121ef2", "test", "ecx, ecx"],
  ["0x121ef4", "je", "0x121ef9"],
  ["0x121ef6", "mov", "eax, ecx"],
  ["0x121ef8", "ret", ""],
  ["0x121ef9", "sub", "eax, dword ptr [0x1e2360]"],
  ["0x121eff", "ret", ""],
].map(([address, mnemonic, opStr]) => ({ address, mnemonic, opStr, size: 1 }));
const timeGetTimeConsumer = {
  apiContinuation: {
    entry: {
      address: "0x121ebf",
      maximumInstructions: 96,
      instructions: [
        { address: "0x121ebf", mnemonic: "jmp", opStr: "0x121ec7", size: 2 },
      ],
    },
    directJumpTarget: {
      address: "0x121ec7",
      maximumInstructions: 96,
      instructions: timeGetTimeConsumerInstructions,
    },
  },
  frameCallerContinuations: {
    maximumFrames: 6,
    maximumInstructions: 20,
    frames: [
      {
        depth: 0,
        framePointer: "0x6ff000",
        caller: { address: "0x113144", module: "BGI.exe", rva: 0x83144 },
        address: "0x113144",
        maximumInstructions: 20,
        instructions: [],
      },
    ],
  },
};
const timeGetTimeOracleCalls = [0, 87, 0x1234_5678].map((sentinel) => ({
  sentinel,
  tickCountBefore: 1000,
  systemResult: 1000,
  tickCountAfter: 1000,
  tickCountSpan: 0,
  resultOffset: 0,
  hookResult: 1000,
  hookLastErrorAfter: sentinel,
  systemLastError: sentinel,
  resultWithinTickCountBracket: true,
  hookResultMatches: true,
  hookPreservesLastError: true,
  systemPreservesLastError: true,
}));
const timeGetTimeEvents = [
  {
    schema,
    kind: "trace-start",
    timestamp: "2026-07-20T00:00:00.000Z",
    executable: "C:\\fixture\\BGI.exe",
    executableBytes: 1193472,
    executableSha256: "b".repeat(64),
    arguments: {
      seconds: 30,
      killAfter: true,
      stopOnEvent: "time-get-time-complete",
      vtables: [],
      observations: ["time-get-time"],
    },
  },
  {
    schema,
    sequence: 0,
    kind: "module-load",
    module: "BGI.exe",
    path: "C:\\fixture\\BGI.exe",
  },
  {
    schema,
    sequence: 1,
    kind: "module-load",
    module: "WINMM.dll",
    path: "C:\\Windows\\System32\\WINMM.dll",
  },
  {
    schema,
    sequence: 2,
    kind: "agent-ready",
    architecture: "ia32",
    pointerSize: 4,
    processId: 456,
    selectedVtables: [],
    selectedObservations: ["time-get-time"],
  },
];
for (const [index, sentinel] of [0, 87, 0x1234_5678].entries()) {
  const sequence = 3 + index * 2;
  const caller = { address: "0x70001000", module: "frida-agent.dll", rva: 0x1000 };
  timeGetTimeEvents.push(
    {
      schema,
      sequence,
      threadId: 30,
      kind: "api-enter",
      callId: index + 1,
      parentCallId: null,
      caller,
      module: "WINMM.dll",
      api: "timeGetTime",
      arguments: [],
      lastErrorBefore: sentinel,
      timeGetTime: { immediateConsumer: null },
    },
    {
      schema,
      sequence: sequence + 1,
      threadId: 30,
      kind: "api-leave",
      callId: index + 1,
      parentCallId: null,
      caller,
      durationMs: 0,
      lastError: sentinel,
      module: "WINMM.dll",
      api: "timeGetTime",
      result: "0x3e8",
      timeGetTime: { result: 1000 },
    },
  );
}
timeGetTimeEvents.push(
  {
    schema,
    sequence: 9,
    threadId: 30,
    kind: "time-get-time-oracle",
    profile: "x86-time-get-time-shared-tick-epoch-v1",
    calls: timeGetTimeOracleCalls,
    allResultsMatchHooks: true,
    allPreserveLastError: true,
    allResultsWithinTickCountBrackets: true,
  },
  {
    schema,
    sequence: 10,
    threadId: 31,
    kind: "api-enter",
    callId: 4,
    parentCallId: null,
    caller: { address: "0x121ebf", module: "BGI.exe", rva: 0x91ebf },
    module: "WINMM.dll",
    api: "timeGetTime",
    arguments: [],
    lastErrorBefore: 0,
    timeGetTime: { immediateConsumer: timeGetTimeConsumer },
  },
  {
    schema,
    sequence: 11,
    threadId: 31,
    kind: "api-leave",
    callId: 4,
    parentCallId: null,
    caller: { address: "0x121ebf", module: "BGI.exe", rva: 0x91ebf },
    durationMs: 0,
    lastError: 0,
    module: "WINMM.dll",
    api: "timeGetTime",
    result: "0x7d0",
    timeGetTime: { result: 2000 },
  },
  {
    schema,
    sequence: 12,
    threadId: 31,
    kind: "time-get-time-complete",
    profile: "x86-bgi-time-get-time-v1",
    oracleProfile: "x86-time-get-time-shared-tick-epoch-v1",
    mainModuleCalls: [
      {
        callId: 4,
        returnAddress: "0x121ebf",
        result: 2000,
        lastErrorBefore: 0,
        lastError: 0,
        immediateConsumer: timeGetTimeConsumer,
      },
    ],
  },
  {
    schema,
    kind: "trace-stop",
    timestamp: "2026-07-20T00:00:01.000Z",
    reason: "event-time-get-time-complete-kill",
    detachedReason: "process-terminated",
    crash: null,
  },
);
const timeGetTimeSummary = summarizeTrace(
  encode(timeGetTimeEvents),
  "time-get-time-fixture.jsonl",
);
assert.equal(timeGetTimeSummary.winmm.timeGetTime.length, 1);
assert.equal(
  timeGetTimeSummary.winmm.timeGetTime[0].oracle.profile,
  "x86-time-get-time-shared-tick-epoch-v1",
);
assert.deepEqual(
  timeGetTimeSummary.winmm.timeGetTime[0].oracle.calls.map((call) => call.sentinel),
  [0, 87, 0x1234_5678],
);
assert.deepEqual(timeGetTimeSummary.winmm.timeGetTime[0].mainModuleCalls[0], {
  sequence: 10,
  callId: 4,
  threadId: 31,
  caller: { module: "BGI.exe", rva: 0x91ebf },
  returnAddress: "0x121ebf",
  result: 2000,
  lastErrorBefore: 0,
  lastError: 0,
  consumer: {
    profile: "wrap-aware-eax-delta-v1",
    directJumpTarget: "0x121ec7",
    frameCallers: [{ depth: 0, module: "BGI.exe", rva: 0x83144 }],
  },
});

const invalidTimeGetTimeOracle = structuredClone(timeGetTimeEvents);
invalidTimeGetTimeOracle.find(
  (event) => event.kind === "time-get-time-oracle",
).calls[1].resultOffset = 1;
assert.throws(
  () => summarizeTrace(encode(invalidTimeGetTimeOracle)),
  /oracle call 1 violates its tick or LastError contract/,
);

const invalidTimeGetTimeConsumer = structuredClone(timeGetTimeEvents);
invalidTimeGetTimeConsumer.find(
  (event) => event.kind === "api-enter" && event.caller.module === "BGI.exe",
).timeGetTime.immediateConsumer.apiContinuation.directJumpTarget.instructions[8].opStr =
  "edx, eax";
invalidTimeGetTimeConsumer.find(
  (event) => event.kind === "time-get-time-complete",
).mainModuleCalls[0].immediateConsumer.apiContinuation.directJumpTarget.instructions[8].opStr =
  "edx, eax";
assert.throws(
  () => summarizeTrace(encode(invalidTimeGetTimeConsumer)),
  /wrap-aware EAX delta flow/,
);

const missingTimeGetTimeCompletion = timeGetTimeEvents.filter(
  (event) => event.kind !== "time-get-time-complete",
);
assert.throws(
  () => summarizeTrace(encode(missingTimeGetTimeCompletion)),
  /requires exactly one oracle and structured completion/,
);

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
