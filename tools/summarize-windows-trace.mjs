import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const legacyTraceSchema = "vine.win32-trace.v1";
const traceSchema = "vine.win32-trace.v2";
const supportedTraceSchemas = new Set([legacyTraceSchema, traceSchema]);
const summarySchema = "vine.win32-trace-summary.v2";
const supportedVtables = new Set(["idirect3d9"]);
const supportedObservations = new Set([
  "startup-info",
  "heap-set-information",
  "heap-lifecycle",
  "heap-allocation",
  "heap-size",
  "module-handle",
  "dynamic-export",
  "tls-lifecycle",
  "fls-lifecycle",
  "pointer-encoding",
  "critical-section",
  "interlocked-increment",
  "interlocked-decrement",
  "get-module-file-name-a",
  "standard-handle",
  "file-type",
  "set-handle-count",
  "command-line-a",
  "environment-strings-w",
  "wide-char-to-multi-byte",
  "multi-byte-to-wide-char",
  "set-current-directory-a",
  "create-mutex-a",
  "create-waitable-timer-a",
  "load-icon-a",
  "load-image-a",
  "send-message-a",
  "set-window-pos",
  "set-window-pos-size",
  "load-cursor-a",
  "register-class-ex-a",
  "get-system-metrics",
  "get-client-rect",
  "window-dc",
  "set-window-long-a",
  "get-string-type-w",
  "lc-map-string-w",
  "get-last-error",
  "set-last-error",
  "get-acp",
  "system-default-lang-id",
  "drive-type",
  "swap-mouse-button",
  "time-get-dev-caps",
  "time-get-time",
  "get-version-ex-a",
  "get-cp-info",
  "processor-feature",
  "unhandled-exception-filter",
  "com-initialization",
  "d3d9-create-device",
  "d3d9-get-surface-level",
  "d3d9-surface-get-dc",
  "d3d9-initial-textures",
  "d3d9-initial-vertex-buffer",
]);
export const idirect3d9MethodNames = [
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
];
const idirect3ddevice9Methods = [
  ["QueryInterface", 2],
  ["AddRef", 0],
  ["Release", 0],
  ["TestCooperativeLevel", 0],
  ["GetAvailableTextureMem", 0],
  ["EvictManagedResources", 0],
  ["GetDirect3D", 1],
  ["GetDeviceCaps", 1],
  ["GetDisplayMode", 2],
  ["GetCreationParameters", 1],
  ["SetCursorProperties", 3],
  ["SetCursorPosition", 3],
  ["ShowCursor", 1],
  ["CreateAdditionalSwapChain", 2],
  ["GetSwapChain", 2],
  ["GetNumberOfSwapChains", 0],
  ["Reset", 1],
  ["Present", 4],
  ["GetBackBuffer", 4],
  ["GetRasterStatus", 2],
  ["SetDialogBoxMode", 1],
  ["SetGammaRamp", 3],
  ["GetGammaRamp", 2],
  ["CreateTexture", 8],
  ["CreateVolumeTexture", 9],
  ["CreateCubeTexture", 7],
  ["CreateVertexBuffer", 6],
  ["CreateIndexBuffer", 6],
  ["CreateRenderTarget", 8],
  ["CreateDepthStencilSurface", 8],
  ["UpdateSurface", 4],
  ["UpdateTexture", 2],
  ["GetRenderTargetData", 2],
  ["GetFrontBufferData", 2],
  ["StretchRect", 5],
  ["ColorFill", 3],
  ["CreateOffscreenPlainSurface", 6],
  ["SetRenderTarget", 2],
  ["GetRenderTarget", 2],
  ["SetDepthStencilSurface", 1],
  ["GetDepthStencilSurface", 1],
  ["BeginScene", 0],
  ["EndScene", 0],
  ["Clear", 6],
  ["SetTransform", 2],
  ["GetTransform", 2],
  ["MultiplyTransform", 2],
  ["SetViewport", 1],
  ["GetViewport", 1],
  ["SetMaterial", 1],
  ["GetMaterial", 1],
  ["SetLight", 2],
  ["GetLight", 2],
  ["LightEnable", 2],
  ["GetLightEnable", 2],
  ["SetClipPlane", 2],
  ["GetClipPlane", 2],
  ["SetRenderState", 2],
  ["GetRenderState", 2],
  ["CreateStateBlock", 2],
  ["BeginStateBlock", 0],
  ["EndStateBlock", 1],
  ["SetClipStatus", 1],
  ["GetClipStatus", 1],
  ["GetTexture", 2],
  ["SetTexture", 2],
  ["GetTextureStageState", 3],
  ["SetTextureStageState", 3],
  ["GetSamplerState", 3],
  ["SetSamplerState", 3],
  ["ValidateDevice", 1],
  ["SetPaletteEntries", 2],
  ["GetPaletteEntries", 2],
  ["SetCurrentTexturePalette", 1],
  ["GetCurrentTexturePalette", 1],
  ["SetScissorRect", 1],
  ["GetScissorRect", 1],
  ["SetSoftwareVertexProcessing", 1],
  ["GetSoftwareVertexProcessing", 0],
  ["SetNPatchMode", 1],
  ["GetNPatchMode", 0],
  ["DrawPrimitive", 3],
  ["DrawIndexedPrimitive", 6],
  ["DrawPrimitiveUP", 4],
  ["DrawIndexedPrimitiveUP", 8],
  ["ProcessVertices", 6],
  ["CreateVertexDeclaration", 2],
  ["SetVertexDeclaration", 1],
  ["GetVertexDeclaration", 1],
  ["SetFVF", 1],
  ["GetFVF", 1],
  ["CreateVertexShader", 2],
  ["SetVertexShader", 1],
  ["GetVertexShader", 1],
  ["SetVertexShaderConstantF", 3],
  ["GetVertexShaderConstantF", 3],
  ["SetVertexShaderConstantI", 3],
  ["GetVertexShaderConstantI", 3],
  ["SetVertexShaderConstantB", 3],
  ["GetVertexShaderConstantB", 3],
  ["SetStreamSource", 4],
  ["GetStreamSource", 4],
  ["SetStreamSourceFreq", 2],
  ["GetStreamSourceFreq", 2],
  ["SetIndices", 1],
  ["GetIndices", 1],
  ["CreatePixelShader", 2],
  ["SetPixelShader", 1],
  ["GetPixelShader", 1],
  ["SetPixelShaderConstantF", 3],
  ["GetPixelShaderConstantF", 3],
  ["SetPixelShaderConstantI", 3],
  ["GetPixelShaderConstantI", 3],
  ["SetPixelShaderConstantB", 3],
  ["GetPixelShaderConstantB", 3],
  ["DrawRectPatch", 3],
  ["DrawTriPatch", 3],
  ["DeletePatch", 1],
  ["CreateQuery", 2],
];
export const idirect3ddevice9MethodNames = idirect3ddevice9Methods.map(
  ([name]) => name,
);
const idirect3dtexture9Methods = [
  ["QueryInterface", 2],
  ["AddRef", 0],
  ["Release", 0],
  ["GetDevice", 1],
  ["SetPrivateData", 4],
  ["GetPrivateData", 3],
  ["FreePrivateData", 1],
  ["SetPriority", 1],
  ["GetPriority", 0],
  ["PreLoad", 0],
  ["GetType", 0],
  ["SetLOD", 1],
  ["GetLOD", 0],
  ["GetLevelCount", 0],
  ["SetAutoGenFilterType", 1],
  ["GetAutoGenFilterType", 0],
  ["GenerateMipSubLevels", 0],
  ["GetLevelDesc", 2],
  ["GetSurfaceLevel", 2],
  ["LockRect", 4],
  ["UnlockRect", 1],
  ["AddDirtyRect", 1],
];
export const idirect3dtexture9MethodNames = idirect3dtexture9Methods.map(
  ([name]) => name,
);
const idirect3dvertexbuffer9Methods = [
  ["QueryInterface", 2],
  ["AddRef", 0],
  ["Release", 0],
  ["GetDevice", 1],
  ["SetPrivateData", 4],
  ["GetPrivateData", 3],
  ["FreePrivateData", 1],
  ["SetPriority", 1],
  ["GetPriority", 0],
  ["PreLoad", 0],
  ["GetType", 0],
  ["Lock", 4],
  ["Unlock", 0],
  ["GetDesc", 1],
];
export const idirect3dvertexbuffer9MethodNames =
  idirect3dvertexbuffer9Methods.map(([name]) => name);
const idirect3dsurface9Methods = [
  ["QueryInterface", 2],
  ["AddRef", 0],
  ["Release", 0],
  ["GetDevice", 1],
  ["SetPrivateData", 4],
  ["GetPrivateData", 3],
  ["FreePrivateData", 1],
  ["SetPriority", 1],
  ["GetPriority", 0],
  ["PreLoad", 0],
  ["GetType", 0],
  ["GetContainer", 2],
  ["GetDesc", 1],
  ["LockRect", 3],
  ["UnlockRect", 0],
  ["GetDC", 1],
  ["ReleaseDC", 1],
];
export const idirect3dsurface9MethodNames = idirect3dsurface9Methods.map(
  ([name]) => name,
);
const d3d9PresentParameterFields = [
  "backBufferWidth",
  "backBufferHeight",
  "backBufferFormat",
  "backBufferCount",
  "multiSampleType",
  "multiSampleQuality",
  "swapEffect",
  "deviceWindow",
  "windowed",
  "enableAutoDepthStencil",
  "autoDepthStencilFormat",
  "flags",
  "fullScreenRefreshRateInHz",
  "presentationInterval",
];
const d3d9DeviceCapsFieldLayout = [
  ["deviceType", "u32"],
  ["adapterOrdinal", "u32"],
  ["caps", "u32"],
  ["caps2", "u32"],
  ["caps3", "u32"],
  ["presentationIntervals", "u32"],
  ["cursorCaps", "u32"],
  ["devCaps", "u32"],
  ["primitiveMiscCaps", "u32"],
  ["rasterCaps", "u32"],
  ["zCmpCaps", "u32"],
  ["srcBlendCaps", "u32"],
  ["destBlendCaps", "u32"],
  ["alphaCmpCaps", "u32"],
  ["shadeCaps", "u32"],
  ["textureCaps", "u32"],
  ["textureFilterCaps", "u32"],
  ["cubeTextureFilterCaps", "u32"],
  ["volumeTextureFilterCaps", "u32"],
  ["textureAddressCaps", "u32"],
  ["volumeTextureAddressCaps", "u32"],
  ["lineCaps", "u32"],
  ["maxTextureWidth", "u32"],
  ["maxTextureHeight", "u32"],
  ["maxVolumeExtent", "u32"],
  ["maxTextureRepeat", "u32"],
  ["maxTextureAspectRatio", "u32"],
  ["maxAnisotropy", "u32"],
  ["maxVertexW", "f32"],
  ["guardBandLeft", "f32"],
  ["guardBandTop", "f32"],
  ["guardBandRight", "f32"],
  ["guardBandBottom", "f32"],
  ["extentsAdjust", "f32"],
  ["stencilCaps", "u32"],
  ["fvfCaps", "u32"],
  ["textureOpCaps", "u32"],
  ["maxTextureBlendStages", "u32"],
  ["maxSimultaneousTextures", "u32"],
  ["vertexProcessingCaps", "u32"],
  ["maxActiveLights", "u32"],
  ["maxUserClipPlanes", "u32"],
  ["maxVertexBlendMatrices", "u32"],
  ["maxVertexBlendMatrixIndex", "u32"],
  ["maxPointSize", "f32"],
  ["maxPrimitiveCount", "u32"],
  ["maxVertexIndex", "u32"],
  ["maxStreams", "u32"],
  ["maxStreamStride", "u32"],
  ["vertexShaderVersion", "u32"],
  ["maxVertexShaderConst", "u32"],
  ["pixelShaderVersion", "u32"],
  ["pixelShader1xMaxValue", "f32"],
  ["devCaps2", "u32"],
  ["maxNpatchTessellationLevel", "f32"],
  ["reserved5", "u32"],
  ["masterAdapterOrdinal", "u32"],
  ["adapterOrdinalInGroup", "u32"],
  ["numberOfAdaptersInGroup", "u32"],
  ["declTypes", "u32"],
  ["numSimultaneousRts", "u32"],
  ["stretchRectFilterCaps", "u32"],
  ["vs20Caps", "u32"],
  ["vs20DynamicFlowControlDepth", "i32"],
  ["vs20NumTemps", "i32"],
  ["vs20StaticFlowControlDepth", "i32"],
  ["ps20Caps", "u32"],
  ["ps20DynamicFlowControlDepth", "i32"],
  ["ps20NumTemps", "i32"],
  ["ps20StaticFlowControlDepth", "i32"],
  ["ps20NumInstructionSlots", "i32"],
  ["vertexTextureFilterCaps", "u32"],
  ["maxVShaderInstructionsExecuted", "u32"],
  ["maxPShaderInstructionsExecuted", "u32"],
  ["maxVertexShader30InstructionSlots", "u32"],
  ["maxPixelShader30InstructionSlots", "u32"],
];

const categoryApis = {
  file: new Set(["CreateFileA", "CreateFileW"]),
  window: new Set(["CreateWindowExA", "CreateWindowExW"]),
  graphics: new Set([
    "BitBlt",
    "StretchDIBits",
    "GdiplusStartup",
    "Direct3DCreate9",
  ]),
  input: new Set(["DirectInput8Create"]),
  audio: new Set([
    "DirectSoundCreate",
    "DirectSoundCreate8",
    "mciSendCommandA",
    "mciSendCommandW",
    "timeGetDevCaps",
    "waveOutOpen",
  ]),
  com: new Set(["CoInitialize", "CoInitializeEx", "CoCreateInstance"]),
};

const compareOrdinal = (left, right) => (left < right ? -1 : left > right ? 1 : 0);

const exactlyOne = (events, kind) => {
  const matches = events.filter((event) => event.kind === kind);
  if (matches.length !== 1) {
    throw new Error(`expected one ${kind} event, found ${matches.length}`);
  }
  return matches[0];
};

const sortedCounts = (values) =>
  Object.fromEntries(
    [...values.entries()].sort(([left], [right]) => compareOrdinal(left, right)),
  );

const firstSequence = (event) =>
  Number.isInteger(event.sequence) ? event.sequence : null;

const apiEvent = (event) => ({
  sequence: firstSequence(event),
  callId: event.callId ?? null,
  parentCallId: event.parentCallId ?? null,
  caller:
    event.caller === undefined
      ? null
      : { module: event.caller.module, rva: event.caller.rva },
  module: event.module,
  api: event.api,
  arguments: Array.isArray(event.arguments) ? event.arguments : [],
});

const validateCaller = (event) => {
  const caller = event.caller;
  if (caller === null || typeof caller !== "object") {
    throw new Error(`${event.kind} sequence ${event.sequence} has no caller`);
  }
  if (
    typeof caller.address !== "string" ||
    !/^0x[0-9a-f]+$/iu.test(caller.address)
  ) {
    throw new Error(`${event.kind} sequence ${event.sequence} has invalid caller address`);
  }
  if (caller.module === null) {
    if (caller.rva !== null) {
      throw new Error(`${event.kind} sequence ${event.sequence} has caller RVA without module`);
    }
    return;
  }
  if (typeof caller.module !== "string" || caller.module.length === 0) {
    throw new Error(`${event.kind} sequence ${event.sequence} has invalid caller module`);
  }
  if (
    !Number.isInteger(caller.rva) ||
    caller.rva < 0 ||
    caller.rva > 0xffffffff
  ) {
    throw new Error(`${event.kind} sequence ${event.sequence} has invalid caller RVA`);
  }
};

const sameCaller = (left, right) =>
  left.address.toLowerCase() === right.address.toLowerCase() &&
  (left.module === null
    ? right.module === null
    : typeof right.module === "string" &&
      left.module.toLowerCase() === right.module.toLowerCase()) &&
  left.rva === right.rva;

const validateCallTarget = (event, selectedVtables) => {
  if (event.interface === undefined) {
    if (event.vtableSlot !== undefined || event.object !== undefined) {
      throw new Error(`${event.kind} sequence ${event.sequence} has partial vtable target`);
    }
    return;
  }
  if (!selectedVtables.includes("idirect3d9")) {
    throw new Error(`${event.kind} sequence ${event.sequence} has unselected interface`);
  }
  const methodNames =
    event.interface === "IDirect3D9"
      ? idirect3d9MethodNames
      : event.interface === "IDirect3DDevice9"
        ? idirect3ddevice9MethodNames
        : event.interface === "IDirect3DTexture9"
          ? idirect3dtexture9MethodNames
          : event.interface === "IDirect3DVertexBuffer9"
            ? idirect3dvertexbuffer9MethodNames
          : event.interface === "IDirect3DSurface9"
            ? idirect3dsurface9MethodNames
            : null;
  if (methodNames === null) {
    throw new Error(`${event.kind} sequence ${event.sequence} has unselected interface`);
  }
  if (
    !Number.isInteger(event.vtableSlot) ||
    methodNames[event.vtableSlot] !== event.api
  ) {
    throw new Error(`${event.kind} sequence ${event.sequence} has invalid vtable slot`);
  }
  if (typeof event.object !== "string" || !/^0x[0-9a-f]+$/iu.test(event.object)) {
    throw new Error(`${event.kind} sequence ${event.sequence} has invalid COM object`);
  }
};

const pairApiCalls = (events, requireCallMetadata) => {
  const stacks = new Map();
  const pairs = [];
  let enters = 0;
  let leaves = 0;
  let maxDepth = 0;
  let expectedCallId = 1;

  for (const event of events) {
    if (event.kind !== "api-enter" && event.kind !== "api-leave") {
      continue;
    }
    if (!Number.isInteger(event.threadId)) {
      throw new Error(`${event.kind} sequence ${event.sequence} has no threadId`);
    }

    const stack = stacks.get(event.threadId) ?? [];
    stacks.set(event.threadId, stack);
    if (event.kind === "api-enter") {
      if (requireCallMetadata) {
        if (event.callId !== expectedCallId) {
          throw new Error(
            `api-enter sequence ${event.sequence} has callId ${event.callId}, ` +
              `expected ${expectedCallId}`,
          );
        }
        const expectedParentCallId = stack.at(-1)?.enter.callId ?? null;
        if (event.parentCallId !== expectedParentCallId) {
          throw new Error(
            `api-enter call ${event.callId} has parent ${event.parentCallId}, ` +
              `expected ${expectedParentCallId}`,
          );
        }
        validateCaller(event);
        expectedCallId += 1;
      }
      enters += 1;
      stack.push({ enter: event, parent: stack.at(-1)?.enter ?? null });
      maxDepth = Math.max(maxDepth, stack.length);
      continue;
    }

    leaves += 1;
    const frame = stack.pop();
    if (frame === undefined) {
      throw new Error(`api-leave sequence ${event.sequence} has no matching enter`);
    }
    if (requireCallMetadata) {
      if (event.callId !== frame.enter.callId) {
        throw new Error(
          `api-leave sequence ${event.sequence} has callId ${event.callId}, ` +
            `expected ${frame.enter.callId}`,
        );
      }
      if (event.parentCallId !== frame.enter.parentCallId) {
        throw new Error(
          `api-leave call ${event.callId} has parent ${event.parentCallId}, ` +
            `expected ${frame.enter.parentCallId}`,
        );
      }
      validateCaller(event);
      if (!sameCaller(frame.enter.caller, event.caller)) {
        throw new Error(`api-leave call ${event.callId} changed caller attribution`);
      }
      if (
        !Number.isInteger(event.lastError) ||
        event.lastError < 0 ||
        event.lastError > 0xffffffff
      ) {
        throw new Error(`api-leave call ${event.callId} has invalid lastError`);
      }
      if (!Number.isInteger(event.durationMs) || event.durationMs < 0) {
        throw new Error(`api-leave call ${event.callId} has invalid durationMs`);
      }
    }
    if (
      frame.enter.module.toLowerCase() !== event.module.toLowerCase() ||
      frame.enter.api !== event.api
    ) {
      throw new Error(
        `api-leave sequence ${event.sequence} closes ${event.module}!${event.api}, ` +
          `expected ${frame.enter.module}!${frame.enter.api}`,
      );
    }
    if (
      (frame.enter.interface ?? null) !== (event.interface ?? null) ||
      (frame.enter.vtableSlot ?? null) !== (event.vtableSlot ?? null) ||
      (frame.enter.object ?? null) !== (event.object ?? null)
    ) {
      throw new Error(`api-leave call ${event.callId ?? "<legacy>"} changed call target`);
    }
    pairs.push({ enter: frame.enter, leave: event, parent: frame.parent });
  }

  for (const [threadId, stack] of stacks) {
    if (stack.length !== 0) {
      const frame = stack.at(-1);
      throw new Error(
        `thread ${threadId} has unclosed ${frame.enter.module}!${frame.enter.api}`,
      );
    }
  }

  pairs.sort((left, right) => left.enter.sequence - right.enter.sequence);
  return { pairs, enters, leaves, maxDepth, callMetadata: requireCallMetadata };
};

const normalizedWindowsPath = (value) => value.replaceAll("/", "\\");
const windowsPathKey = (value) => normalizedWindowsPath(value).toLowerCase();
const isSuccessfulFileResult = (value) => {
  const result = String(value).toLowerCase();
  return result !== "0xffffffff" && result !== "0xffffffffffffffff";
};

const normalizeD3d9DisplayModeSnapshot = (snapshot, sequence, phase) => {
  if (
    snapshot === null ||
    typeof snapshot !== "object" ||
    typeof snapshot.pointer !== "string" ||
    !/^0x[0-9a-f]+$/iu.test(snapshot.pointer) ||
    snapshot.size !== 16 ||
    !Array.isArray(snapshot.bytes) ||
    snapshot.bytes.length !== 16 ||
    !snapshot.bytes.every(
      (byte) => Number.isInteger(byte) && byte >= 0 && byte <= 0xff,
    ) ||
    snapshot.fields === null ||
    typeof snapshot.fields !== "object" ||
    !["width", "height", "refreshRate", "format"].every(
      (field) =>
        Number.isInteger(snapshot.fields[field]) &&
        snapshot.fields[field] >= 0 &&
        snapshot.fields[field] <= 0xffffffff,
    )
  ) {
    throw new Error(
      `api-leave sequence ${sequence} has invalid D3DDISPLAYMODE ${phase} snapshot`,
    );
  }
  const view = new DataView(Uint8Array.from(snapshot.bytes).buffer);
  const decoded = {
    width: view.getUint32(0, true),
    height: view.getUint32(4, true),
    refreshRate: view.getUint32(8, true),
    format: view.getUint32(12, true),
  };
  if (JSON.stringify(decoded) !== JSON.stringify(snapshot.fields)) {
    throw new Error(
      `api-leave sequence ${sequence} has inconsistent D3DDISPLAYMODE ${phase} bytes`,
    );
  }
  return {
    pointer: snapshot.pointer,
    size: snapshot.size,
    bytes: [...snapshot.bytes],
    fields: decoded,
  };
};

const decodeD3d9IdentifierString = (bytes, offset, size) => {
  const field = bytes.slice(offset, offset + size);
  const terminator = field.indexOf(0);
  const content = terminator < 0 ? field : field.slice(0, terminator);
  return {
    text: terminator < 0 ? null : Buffer.from(content).toString("latin1"),
    byteLength: content.length,
    nulTerminated: terminator >= 0,
    zeroPadded:
      terminator >= 0 && field.slice(terminator).every((byte) => byte === 0),
  };
};

const formatD3d9IdentifierGuid = (view, bytes) => {
  const data1 = view.getUint32(1080, true).toString(16).padStart(8, "0");
  const data2 = view.getUint16(1084, true).toString(16).padStart(4, "0");
  const data3 = view.getUint16(1086, true).toString(16).padStart(4, "0");
  const data4 = Array.from(bytes.slice(1088, 1096), (byte) =>
    byte.toString(16).padStart(2, "0"),
  );
  return `${data1}-${data2}-${data3}-${data4.slice(0, 2).join("")}-${data4
    .slice(2)
    .join("")}`;
};

const normalizeD3d9AdapterIdentifierSnapshot = (snapshot, sequence, phase) => {
  if (
    snapshot === null ||
    typeof snapshot !== "object" ||
    typeof snapshot.pointer !== "string" ||
    !/^0x[0-9a-f]+$/iu.test(snapshot.pointer) ||
    snapshot.size !== 1100 ||
    !Array.isArray(snapshot.bytes) ||
    snapshot.bytes.length !== 1100 ||
    !snapshot.bytes.every(
      (byte) => Number.isInteger(byte) && byte >= 0 && byte <= 0xff,
    )
  ) {
    throw new Error(
      `api-leave sequence ${sequence} has invalid D3DADAPTER_IDENTIFIER9 ${phase} snapshot`,
    );
  }
  const bytes = Uint8Array.from(snapshot.bytes);
  const view = new DataView(bytes.buffer);
  const lowPart = view.getUint32(1056, true);
  const highPart = view.getUint32(1060, true);
  return {
    pointer: snapshot.pointer,
    size: snapshot.size,
    bytesSha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    zeroByteCount: bytes.reduce(
      (count, byte) => count + (byte === 0 ? 1 : 0),
      0,
    ),
    driver: decodeD3d9IdentifierString(bytes, 0, 512),
    description: decodeD3d9IdentifierString(bytes, 512, 512),
    deviceName: decodeD3d9IdentifierString(bytes, 1024, 32),
    driverVersion: {
      lowPart,
      highPart,
      hex: `0x${highPart.toString(16).padStart(8, "0")}${lowPart
        .toString(16)
        .padStart(8, "0")}`,
    },
    vendorId: view.getUint32(1064, true),
    deviceId: view.getUint32(1068, true),
    subSysId: view.getUint32(1072, true),
    revision: view.getUint32(1076, true),
    deviceIdentifier: formatD3d9IdentifierGuid(view, bytes),
    whqlLevel: view.getUint32(1096, true),
  };
};

const normalizeD3d9DeviceCapsSnapshot = (snapshot, sequence, phase) => {
  if (
    snapshot === null ||
    typeof snapshot !== "object" ||
    typeof snapshot.pointer !== "string" ||
    !/^0x[0-9a-f]+$/iu.test(snapshot.pointer) ||
    snapshot.size !== 304 ||
    !Array.isArray(snapshot.bytes) ||
    snapshot.bytes.length !== 304 ||
    !snapshot.bytes.every(
      (byte) => Number.isInteger(byte) && byte >= 0 && byte <= 0xff,
    )
  ) {
    throw new Error(
      `api-leave sequence ${sequence} has invalid D3DCAPS9 ${phase} snapshot`,
    );
  }
  const bytes = Uint8Array.from(snapshot.bytes);
  const view = new DataView(bytes.buffer);
  const fields = Object.fromEntries(
    d3d9DeviceCapsFieldLayout.map(([name, type], index) => {
      const offset = index * 4;
      const raw = view.getUint32(offset, true);
      const value =
        type === "i32"
          ? view.getInt32(offset, true)
          : type === "f32"
            ? view.getFloat32(offset, true)
            : raw;
      return [
        name,
        {
          offset,
          type,
          raw,
          value: Number.isFinite(value) ? value : null,
        },
      ];
    }),
  );
  return {
    pointer: snapshot.pointer,
    size: snapshot.size,
    bytesSha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    zeroByteCount: bytes.reduce(
      (count, byte) => count + (byte === 0 ? 1 : 0),
      0,
    ),
    fields,
  };
};

const normalizeD3d9PresentParametersSnapshot = (snapshot, sequence, phase) => {
  if (
    snapshot === null ||
    typeof snapshot !== "object" ||
    typeof snapshot.pointer !== "string" ||
    !/^0x[0-9a-f]+$/iu.test(snapshot.pointer) ||
    snapshot.size !== 56 ||
    !Array.isArray(snapshot.bytes) ||
    snapshot.bytes.length !== 56 ||
    !snapshot.bytes.every(
      (byte) => Number.isInteger(byte) && byte >= 0 && byte <= 0xff,
    ) ||
    snapshot.fields === null ||
    typeof snapshot.fields !== "object" ||
    !d3d9PresentParameterFields.every(
      (field) =>
        Number.isInteger(snapshot.fields[field]) &&
        snapshot.fields[field] >= 0 &&
        snapshot.fields[field] <= 0xffffffff,
    )
  ) {
    throw new Error(
      `api-leave sequence ${sequence} has invalid D3DPRESENT_PARAMETERS ${phase} snapshot`,
    );
  }
  const bytes = Uint8Array.from(snapshot.bytes);
  const view = new DataView(bytes.buffer);
  const fields = Object.fromEntries(
    d3d9PresentParameterFields.map((field, index) => [
      field,
      view.getUint32(index * 4, true),
    ]),
  );
  if (JSON.stringify(fields) !== JSON.stringify(snapshot.fields)) {
    throw new Error(
      `api-leave sequence ${sequence} has inconsistent D3DPRESENT_PARAMETERS ${phase} bytes`,
    );
  }
  return {
    pointer: snapshot.pointer,
    size: snapshot.size,
    bytes: [...snapshot.bytes],
    bytesSha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    fields,
  };
};

const normalizePointerSlotSnapshot = (snapshot, sequence, phase) => {
  if (
    snapshot === null ||
    typeof snapshot !== "object" ||
    typeof snapshot.pointer !== "string" ||
    !/^0x[0-9a-f]+$/iu.test(snapshot.pointer) ||
    snapshot.size !== 4 ||
    !Array.isArray(snapshot.bytes) ||
    snapshot.bytes.length !== 4 ||
    !snapshot.bytes.every(
      (byte) => Number.isInteger(byte) && byte >= 0 && byte <= 0xff,
    ) ||
    typeof snapshot.value !== "string" ||
    !/^0x[0-9a-f]+$/iu.test(snapshot.value)
  ) {
    throw new Error(
      `api-leave sequence ${sequence} has invalid pointer-slot ${phase} snapshot`,
    );
  }
  const bytes = Uint8Array.from(snapshot.bytes);
  const value = new DataView(bytes.buffer).getUint32(0, true);
  if (BigInt(snapshot.value) !== BigInt(value)) {
    throw new Error(
      `api-leave sequence ${sequence} has inconsistent pointer-slot ${phase} bytes`,
    );
  }
  return {
    pointer: snapshot.pointer,
    size: snapshot.size,
    bytes: [...snapshot.bytes],
    value: `0x${value.toString(16)}`,
  };
};

const normalizeBoundedRawBytesSnapshot = (
  snapshot,
  expectedBytes,
  sequence,
  phase,
) => {
  if (
    snapshot === null ||
    typeof snapshot !== "object" ||
    typeof snapshot.pointer !== "string" ||
    !/^0x[0-9a-f]+$/iu.test(snapshot.pointer) ||
    snapshot.requestedBytes !== expectedBytes ||
    snapshot.maximumBytes !== expectedBytes ||
    !Array.isArray(snapshot.bytes) ||
    snapshot.bytes.length !== expectedBytes ||
    !snapshot.bytes.every(
      (byte) => Number.isInteger(byte) && byte >= 0 && byte <= 0xff,
    )
  ) {
    throw new Error(
      `api sequence ${sequence} has invalid bounded raw ${phase} snapshot`,
    );
  }
  const bytes = Uint8Array.from(snapshot.bytes);
  return {
    pointer: snapshot.pointer,
    bytes: [...snapshot.bytes],
    bytesSha256: crypto.createHash("sha256").update(bytes).digest("hex"),
  };
};

const normalizeD3d9U32Argument = (value, sequence, index, method) => {
  if (typeof value !== "string" || !/^0x[0-9a-f]+$/iu.test(value)) {
    throw new Error(
      `api-enter sequence ${sequence} has invalid ${method} argument ${index}`,
    );
  }
  const parsed = BigInt(value);
  if (parsed > 0xffff_ffffn) {
    throw new Error(
      `api-enter sequence ${sequence} has invalid ${method} argument ${index}`,
    );
  }
  return Number(parsed);
};

export const summarizeTrace = (buffer, sourcePath = null) => {
  const text = buffer.toString("utf8");
  const lines = text.split(/\r?\n/);
  if (lines.at(-1) === "") {
    lines.pop();
  }
  if (lines.length === 0) {
    throw new Error("trace is empty");
  }

  const events = lines.map((line, index) => {
    if (line.trim() === "") {
      throw new Error(`trace line ${index + 1} is empty`);
    }
    let event;
    try {
      event = JSON.parse(line);
    } catch (error) {
      throw new Error(`trace line ${index + 1} is not valid JSON: ${error.message}`);
    }
    if (!supportedTraceSchemas.has(event.schema)) {
      throw new Error(
        `trace line ${index + 1} has schema ${event.schema ?? "<missing>"}`,
      );
    }
    return event;
  });

  const traceStart = exactlyOne(events, "trace-start");
  const traceStop = exactlyOne(events, "trace-stop");
  const agentReady = exactlyOne(events, "agent-ready");
  if (events[0] !== traceStart) {
    throw new Error("trace-start must be the first record");
  }
  if (events.at(-1) !== traceStop) {
    throw new Error("trace-stop must be the last record");
  }
  if (events.some((event) => event.schema !== traceStart.schema)) {
    throw new Error("trace mixes schema versions");
  }
  if (!Number.isInteger(agentReady.sequence)) {
    throw new Error("agent-ready must have an integer sequence");
  }
  const requireCallMetadata = traceStart.schema === traceSchema;
  let selectedVtables = [];
  let selectedObservations = [];
  if (requireCallMetadata) {
    const requestedVtables = traceStart.arguments?.vtables;
    selectedVtables = agentReady.selectedVtables;
    if (
      !Array.isArray(requestedVtables) ||
      !requestedVtables.every((item) => typeof item === "string") ||
      !Array.isArray(selectedVtables) ||
      !selectedVtables.every((item) => typeof item === "string")
    ) {
      throw new Error("trace v2 has invalid selected vtable metadata");
    }
    if (
      new Set(selectedVtables).size !== selectedVtables.length ||
      selectedVtables.some((item) => !supportedVtables.has(item))
    ) {
      throw new Error("trace v2 has unsupported or duplicate selected vtable");
    }
    if (JSON.stringify(requestedVtables) !== JSON.stringify(selectedVtables)) {
      throw new Error("trace-start and agent-ready selected vtables differ");
    }
    const requestedObservations = traceStart.arguments?.observations ?? [];
    selectedObservations = agentReady.selectedObservations ?? [];
    if (
      !Array.isArray(requestedObservations) ||
      !requestedObservations.every((item) => typeof item === "string") ||
      !Array.isArray(selectedObservations) ||
      !selectedObservations.every((item) => typeof item === "string") ||
      new Set(selectedObservations).size !== selectedObservations.length ||
      selectedObservations.some((item) => !supportedObservations.has(item))
    ) {
      throw new Error("trace v2 has unsupported or duplicate selected observation");
    }
    if (JSON.stringify(requestedObservations) !== JSON.stringify(selectedObservations)) {
      throw new Error("trace-start and agent-ready selected observations differ");
    }
    for (const event of events) {
      if (event.kind === "api-enter" || event.kind === "api-leave") {
        validateCallTarget(event, selectedVtables);
      }
    }
  }
  const sequenced = events.filter((event) => event.sequence !== undefined);
  let previousSequence = -1;
  for (const event of sequenced) {
    if (!Number.isInteger(event.sequence) || event.sequence < 0) {
      throw new Error(`invalid trace sequence: ${event.sequence}`);
    }
    if (event.sequence <= previousSequence) {
      throw new Error(
        `trace sequence ${event.sequence} follows ${previousSequence}`,
      );
    }
    if (event.sequence !== previousSequence + 1) {
      throw new Error(
        `trace sequence ${event.sequence} is not contiguous after ${previousSequence}`,
      );
    }
    previousSequence = event.sequence;
  }

  const eventCounts = new Map();
  for (const event of events) {
    eventCounts.set(event.kind, (eventCounts.get(event.kind) ?? 0) + 1);
  }
  const apiPairing = pairApiCalls(events, requireCallMetadata);

  const modulesByName = new Map();
  for (const event of events.filter((item) => item.kind === "module-load")) {
    const key = event.module.toLowerCase();
    const existing = modulesByName.get(key);
    if (existing === undefined) {
      modulesByName.set(key, {
        name: event.module,
        firstSequence: event.sequence,
        loadCount: 1,
        paths: new Set([event.path]),
      });
    } else {
      existing.loadCount += 1;
      existing.paths.add(event.path);
    }
  }
  const modules = [...modulesByName.values()]
    .map((module) => ({ ...module, paths: [...module.paths].sort() }))
    .sort((left, right) => left.firstSequence - right.firstSequence);

  const apiEntries = events.filter((event) => event.kind === "api-enter");
  const apisByName = new Map();
  for (const pair of apiPairing.pairs) {
    const event = pair.enter;
    const key =
      `${event.module.toLowerCase()}\0${event.interface ?? ""}\0` +
      `${event.vtableSlot ?? ""}\0${event.api}`;
    const existing = apisByName.get(key);
    const api =
      existing ??
      {
        module: event.module,
        api: event.api,
        interface: event.interface ?? null,
        vtableSlot: event.vtableSlot ?? null,
        calls: 0,
        firstSequence: event.sequence,
        lastSequence: event.sequence,
        callers: new Map(),
        durationTotalMs: 0,
        durationMaxMs: 0,
        lastErrors: new Map(),
      };
    api.calls += 1;
    api.lastSequence = event.sequence;
    if (requireCallMetadata) {
      const caller = event.caller;
      const callerKey = `${caller.module ?? "<unknown>"}\0${caller.rva ?? "<unknown>"}`;
      const callerStats = api.callers.get(callerKey) ?? {
        module: caller.module,
        rva: caller.rva,
        calls: 0,
      };
      callerStats.calls += 1;
      api.callers.set(callerKey, callerStats);
      api.durationTotalMs += pair.leave.durationMs;
      api.durationMaxMs = Math.max(api.durationMaxMs, pair.leave.durationMs);
      api.lastErrors.set(
        pair.leave.lastError,
        (api.lastErrors.get(pair.leave.lastError) ?? 0) + 1,
      );
    }
    apisByName.set(key, api);
  }
  const apis = [...apisByName.values()]
    .sort((left, right) => left.firstSequence - right.firstSequence)
    .map((api) => ({
      module: api.module,
      api: api.api,
      interface: api.interface,
      vtableSlot: api.vtableSlot,
      calls: api.calls,
      firstSequence: api.firstSequence,
      lastSequence: api.lastSequence,
      callers: [...api.callers.values()].sort(
        (left, right) =>
          right.calls - left.calls ||
          compareOrdinal(left.module ?? "", right.module ?? "") ||
          (left.rva ?? -1) - (right.rva ?? -1),
      ),
      durationMs: requireCallMetadata
        ? { total: api.durationTotalMs, max: api.durationMaxMs }
        : null,
      lastErrors: [...api.lastErrors.entries()]
        .sort(([left], [right]) => left - right)
        .map(([value, calls]) => ({ value, calls })),
    }));

  const categories = {};
  for (const [category, names] of Object.entries(categoryApis)) {
    const matchingEntries = apiEntries.filter((event) => names.has(event.api));
    const matchingApis = apis.filter((api) => names.has(api.api));
    categories[category] = {
      calls: matchingEntries.length,
      first: matchingEntries.length === 0 ? null : apiEvent(matchingEntries[0]),
      apis: matchingApis,
    };
  }

  const logicalFileCalls = apiPairing.pairs.filter(
    (pair) =>
      categoryApis.file.has(pair.enter.api) &&
      (pair.parent === null || !categoryApis.file.has(pair.parent.api)),
  );
  const filePaths = new Map();
  const titleRoot = normalizedWindowsPath(path.win32.dirname(traceStart.executable));
  const titleRootKey = windowsPathKey(titleRoot);
  const titlePaths = new Map();
  for (const pair of logicalFileCalls) {
    const filePath = pair.enter.arguments?.[0];
    if (typeof filePath === "string") {
      const normalizedPath = normalizedWindowsPath(filePath);
      const key = windowsPathKey(normalizedPath);
      const file = filePaths.get(key) ?? { path: normalizedPath, calls: 0 };
      file.calls += 1;
      filePaths.set(key, file);

      const titlePrefix = `${titleRootKey}\\`;
      if (key.startsWith(titlePrefix)) {
        const relativePath = normalizedPath.slice(titleRoot.length + 1);
        const relativeKey = windowsPathKey(relativePath);
        const titleFile = titlePaths.get(relativeKey) ?? {
          path: relativePath,
          calls: 0,
          successfulCalls: 0,
          failedCalls: 0,
        };
        titleFile.calls += 1;
        if (isSuccessfulFileResult(pair.leave.result)) {
          titleFile.successfulCalls += 1;
        } else {
          titleFile.failedCalls += 1;
        }
        titlePaths.set(relativeKey, titleFile);
      }
    }
  }
  const topFilePaths = [...filePaths.values()]
    .sort(
      (left, right) =>
        right.calls - left.calls || compareOrdinal(left.path, right.path),
    )
    .slice(0, 20);
  const titleFileList = [...titlePaths.values()].sort((left, right) =>
    compareOrdinal(left.path, right.path),
  );
  const failedSearchRoots = new Map();
  for (const file of titleFileList.filter((item) => item.failedCalls > 0)) {
    const separator = file.path.indexOf("\\");
    const root = separator === -1 ? "." : file.path.slice(0, separator);
    const key = root.toLowerCase();
    const searchRoot = failedSearchRoots.get(key) ?? {
      root,
      calls: 0,
      paths: new Set(),
    };
    searchRoot.calls += file.failedCalls;
    searchRoot.paths.add(windowsPathKey(file.path));
    failedSearchRoots.set(key, searchRoot);
  }

  const startedAtMs = Date.parse(traceStart.timestamp);
  const stoppedAtMs = Date.parse(traceStop.timestamp);
  if (!Number.isFinite(startedAtMs) || !Number.isFinite(stoppedAtMs)) {
    throw new Error("trace start or stop timestamp is invalid");
  }
  if (stoppedAtMs < startedAtMs) {
    throw new Error("trace stop timestamp precedes trace start");
  }

  const diagnostics = {};
  for (const kind of [
    "hook-error",
    "vtable-error",
    "call-stack-error",
    "agent-message",
    "cleanup-error",
  ]) {
    diagnostics[kind] = events.filter((event) => event.kind === kind);
  }

  const vtables = events
    .filter((event) => event.kind === "vtable-observed")
    .map((event) => {
      if (requireCallMetadata) {
        const methodNames =
          event.interface === "IDirect3D9"
            ? idirect3d9MethodNames
            : event.interface === "IDirect3DDevice9"
              ? idirect3ddevice9MethodNames
              : event.interface === "IDirect3DTexture9"
                ? idirect3dtexture9MethodNames
                : event.interface === "IDirect3DVertexBuffer9"
                  ? idirect3dvertexbuffer9MethodNames
                : event.interface === "IDirect3DSurface9"
                  ? idirect3dsurface9MethodNames
                  : null;
        if (methodNames === null) {
          throw new Error(`unsupported observed vtable: ${event.interface}`);
        }
        if (!selectedVtables.includes("idirect3d9")) {
          throw new Error(`${event.interface} vtable was observed without selection`);
        }
        if (
          typeof event.object !== "string" ||
          !/^0x[0-9a-f]+$/iu.test(event.object) ||
          typeof event.vtable !== "string" ||
          !/^0x[0-9a-f]+$/iu.test(event.vtable) ||
          !Array.isArray(event.slots) ||
          event.slots.length !== methodNames.length ||
          !event.slots.every(
            (slot, index) =>
              slot?.slot === index &&
              slot.name === methodNames[index] &&
              typeof slot.address === "string" &&
              /^0x[0-9a-f]+$/iu.test(slot.address),
          )
        ) {
          throw new Error(`${event.interface} vtable has invalid slot metadata`);
        }
      }
      return {
        sequence: event.sequence,
        interface: event.interface,
        object: event.object ?? null,
        vtable: event.vtable ?? null,
        slots: Array.isArray(event.slots)
          ? event.slots.map(({ slot, name }) => ({ slot, name }))
          : [],
      };
    });

  const adapterDisplayModes = apiPairing.pairs
    .filter(
      (pair) =>
        pair.enter.interface === "IDirect3D9" &&
        pair.enter.vtableSlot === 8 &&
        pair.enter.api === "GetAdapterDisplayMode" &&
        pair.leave.d3d9DisplayMode !== undefined,
    )
    .map((pair) => {
      const observation = pair.leave.d3d9DisplayMode;
      if (
        observation === null ||
        typeof observation !== "object" ||
        !Number.isInteger(observation.adapter) ||
        observation.adapter < 0 ||
        observation.adapter > 0xffffffff
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} has invalid D3DDISPLAYMODE observation`,
        );
      }
      const before = normalizeD3d9DisplayModeSnapshot(
        observation.before,
        pair.leave.sequence,
        "before",
      );
      const after = normalizeD3d9DisplayModeSnapshot(
        observation.after,
        pair.leave.sequence,
        "after",
      );
      if (
        before.pointer.toLowerCase() !== after.pointer.toLowerCase() ||
        after.pointer.toLowerCase() !==
          String(pair.enter.arguments?.[1] ?? "").toLowerCase()
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} changed its D3DDISPLAYMODE output pointer`,
        );
      }
      return {
        sequence: pair.enter.sequence,
        callId: pair.enter.callId ?? null,
        caller:
          pair.enter.caller === undefined
            ? null
            : { module: pair.enter.caller.module, rva: pair.enter.caller.rva },
        adapter: observation.adapter,
        result: pair.leave.result,
        lastError: pair.leave.lastError ?? null,
        before,
        after,
      };
    });

  const adapterIdentifiers = apiPairing.pairs
    .filter(
      (pair) =>
        pair.enter.interface === "IDirect3D9" &&
        pair.enter.vtableSlot === 5 &&
        pair.enter.api === "GetAdapterIdentifier" &&
        pair.leave.d3d9AdapterIdentifier !== undefined,
    )
    .map((pair) => {
      const observation = pair.leave.d3d9AdapterIdentifier;
      if (
        observation === null ||
        typeof observation !== "object" ||
        !Number.isInteger(observation.adapter) ||
        observation.adapter < 0 ||
        observation.adapter > 0xffffffff ||
        !Number.isInteger(observation.flags) ||
        observation.flags < 0 ||
        observation.flags > 0xffffffff
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} has invalid D3DADAPTER_IDENTIFIER9 observation`,
        );
      }
      const before = normalizeD3d9AdapterIdentifierSnapshot(
        observation.before,
        pair.leave.sequence,
        "before",
      );
      const after = normalizeD3d9AdapterIdentifierSnapshot(
        observation.after,
        pair.leave.sequence,
        "after",
      );
      if (
        before.pointer.toLowerCase() !== after.pointer.toLowerCase() ||
        after.pointer.toLowerCase() !==
          String(pair.enter.arguments?.[2] ?? "").toLowerCase()
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} changed its D3DADAPTER_IDENTIFIER9 output pointer`,
        );
      }
      const changedBytes = observation.before.bytes.reduce(
        (count, byte, index) =>
          count + (byte === observation.after.bytes[index] ? 0 : 1),
        0,
      );
      return {
        sequence: pair.enter.sequence,
        callId: pair.enter.callId ?? null,
        caller:
          pair.enter.caller === undefined
            ? null
            : { module: pair.enter.caller.module, rva: pair.enter.caller.rva },
        adapter: observation.adapter,
        flags: observation.flags,
        result: pair.leave.result,
        lastError: pair.leave.lastError ?? null,
        changedBytes,
        before,
        after,
        immediateConsumer: observation.immediateConsumer ?? null,
      };
    });

  const deviceTypeChecks = apiPairing.pairs
    .filter(
      (pair) =>
        pair.enter.interface === "IDirect3D9" &&
        pair.enter.vtableSlot === 9 &&
        pair.enter.api === "CheckDeviceType",
    )
    .map((pair) => {
      if (!Array.isArray(pair.enter.arguments) || pair.enter.arguments.length !== 5) {
        throw new Error(
          `api-enter sequence ${pair.enter.sequence} has invalid CheckDeviceType arguments`,
        );
      }
      if (
        typeof pair.leave.result !== "string" ||
        !/^0x[0-9a-f]+$/iu.test(pair.leave.result)
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} has invalid CheckDeviceType result`,
        );
      }
      const [adapter, deviceType, adapterFormat, backBufferFormat, windowedRaw] =
        pair.enter.arguments.map((argument, index) =>
          normalizeD3d9U32Argument(
            argument,
            pair.enter.sequence,
            index,
            "CheckDeviceType",
          ),
        );
      return {
        sequence: pair.enter.sequence,
        callId: pair.enter.callId ?? null,
        parentCallId: pair.enter.parentCallId ?? null,
        caller:
          pair.enter.caller === undefined
            ? null
            : { module: pair.enter.caller.module, rva: pair.enter.caller.rva },
        adapter,
        deviceType,
        adapterFormat,
        backBufferFormat,
        windowedRaw,
        windowed: windowedRaw !== 0,
        result: pair.leave.result,
        lastError: pair.leave.lastError ?? null,
      };
    });

  const deviceFormatChecks = apiPairing.pairs
    .filter(
      (pair) =>
        pair.enter.interface === "IDirect3D9" &&
        pair.enter.vtableSlot === 10 &&
        pair.enter.api === "CheckDeviceFormat",
    )
    .map((pair) => {
      if (!Array.isArray(pair.enter.arguments) || pair.enter.arguments.length !== 6) {
        throw new Error(
          `api-enter sequence ${pair.enter.sequence} has invalid CheckDeviceFormat arguments`,
        );
      }
      if (
        typeof pair.leave.result !== "string" ||
        !/^0x[0-9a-f]+$/iu.test(pair.leave.result)
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} has invalid CheckDeviceFormat result`,
        );
      }
      const [adapter, deviceType, adapterFormat, usage, resourceType, checkFormat] =
        pair.enter.arguments.map((argument, index) =>
          normalizeD3d9U32Argument(
            argument,
            pair.enter.sequence,
            index,
            "CheckDeviceFormat",
          ),
        );
      return {
        sequence: pair.enter.sequence,
        callId: pair.enter.callId ?? null,
        parentCallId: pair.enter.parentCallId ?? null,
        caller:
          pair.enter.caller === undefined
            ? null
            : { module: pair.enter.caller.module, rva: pair.enter.caller.rva },
        adapter,
        deviceType,
        adapterFormat,
        usage,
        resourceType,
        checkFormat,
        result: pair.leave.result,
        lastError: pair.leave.lastError ?? null,
      };
    });

  const deviceCaps = apiPairing.pairs
    .filter(
      (pair) =>
        pair.enter.interface === "IDirect3D9" &&
        pair.enter.vtableSlot === 14 &&
        pair.enter.api === "GetDeviceCaps" &&
        pair.leave.d3d9DeviceCaps !== undefined,
    )
    .map((pair) => {
      const observation = pair.leave.d3d9DeviceCaps;
      if (
        observation === null ||
        typeof observation !== "object" ||
        !Number.isInteger(observation.adapter) ||
        observation.adapter < 0 ||
        observation.adapter > 0xffffffff ||
        !Number.isInteger(observation.deviceType) ||
        observation.deviceType < 0 ||
        observation.deviceType > 0xffffffff ||
        typeof observation.callerStackPointerAfterReturn !== "string" ||
        !/^0x[0-9a-f]+$/iu.test(observation.callerStackPointerAfterReturn)
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} has invalid D3DCAPS9 observation`,
        );
      }
      const before = normalizeD3d9DeviceCapsSnapshot(
        observation.before,
        pair.leave.sequence,
        "before",
      );
      const after = normalizeD3d9DeviceCapsSnapshot(
        observation.after,
        pair.leave.sequence,
        "after",
      );
      if (
        before.pointer.toLowerCase() !== after.pointer.toLowerCase() ||
        after.pointer.toLowerCase() !==
          String(pair.enter.arguments?.[2] ?? "").toLowerCase()
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} changed its D3DCAPS9 output pointer`,
        );
      }
      const changedBytes = observation.before.bytes.reduce(
        (count, byte, index) =>
          count + (byte === observation.after.bytes[index] ? 0 : 1),
        0,
      );
      const outputOffsetAfterReturn = Number(
        BigInt(after.pointer) - BigInt(observation.callerStackPointerAfterReturn),
      );
      if (!Number.isSafeInteger(outputOffsetAfterReturn)) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} has unsafe D3DCAPS9 stack relation`,
        );
      }
      return {
        sequence: pair.enter.sequence,
        callId: pair.enter.callId ?? null,
        caller:
          pair.enter.caller === undefined
            ? null
            : { module: pair.enter.caller.module, rva: pair.enter.caller.rva },
        adapter: observation.adapter,
        deviceType: observation.deviceType,
        callerStackPointerAfterReturn:
          observation.callerStackPointerAfterReturn,
        outputOffsetAfterReturn,
        result: pair.leave.result,
        lastError: pair.leave.lastError ?? null,
        changedBytes,
        before,
        after,
        immediateConsumer: observation.immediateConsumer ?? null,
      };
    });

  const createDevices = apiPairing.pairs
    .filter(
      (pair) =>
        pair.enter.interface === "IDirect3D9" &&
        pair.enter.vtableSlot === 16 &&
        pair.enter.api === "CreateDevice" &&
        pair.leave.d3d9CreateDevice !== undefined,
    )
    .map((pair) => {
      if (!Array.isArray(pair.enter.arguments) || pair.enter.arguments.length !== 6) {
        throw new Error(
          `api-enter sequence ${pair.enter.sequence} has invalid CreateDevice arguments`,
        );
      }
      const [adapter, deviceType, focusWindow, behaviorFlags] = pair.enter.arguments
        .slice(0, 4)
        .map((argument, index) =>
          normalizeD3d9U32Argument(
            argument,
            pair.enter.sequence,
            index,
            "CreateDevice",
          ),
        );
      normalizeD3d9U32Argument(
        pair.enter.arguments[4],
        pair.enter.sequence,
        4,
        "CreateDevice",
      );
      normalizeD3d9U32Argument(
        pair.enter.arguments[5],
        pair.enter.sequence,
        5,
        "CreateDevice",
      );
      const observation = pair.leave.d3d9CreateDevice;
      if (
        observation === null ||
        typeof observation !== "object" ||
        observation.adapter !== adapter ||
        observation.deviceType !== deviceType ||
        typeof observation.focusWindow !== "string" ||
        !/^0x[0-9a-f]+$/iu.test(observation.focusWindow) ||
        BigInt(observation.focusWindow) !== BigInt(focusWindow) ||
        observation.behaviorFlags !== behaviorFlags ||
        observation.presentationParameters === null ||
        typeof observation.presentationParameters !== "object" ||
        observation.deviceOutput === null ||
        typeof observation.deviceOutput !== "object"
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} has invalid CreateDevice observation`,
        );
      }
      const presentationBefore = normalizeD3d9PresentParametersSnapshot(
        observation.presentationParameters.before,
        pair.leave.sequence,
        "before",
      );
      const presentationAfter = normalizeD3d9PresentParametersSnapshot(
        observation.presentationParameters.after,
        pair.leave.sequence,
        "after",
      );
      const deviceOutputBefore = normalizePointerSlotSnapshot(
        observation.deviceOutput.before,
        pair.leave.sequence,
        "before",
      );
      const deviceOutputAfter = normalizePointerSlotSnapshot(
        observation.deviceOutput.after,
        pair.leave.sequence,
        "after",
      );
      if (
        presentationBefore.pointer.toLowerCase() !==
          presentationAfter.pointer.toLowerCase() ||
        presentationAfter.pointer.toLowerCase() !==
          pair.enter.arguments[4].toLowerCase()
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} changed its D3DPRESENT_PARAMETERS pointer`,
        );
      }
      if (
        deviceOutputBefore.pointer.toLowerCase() !==
          deviceOutputAfter.pointer.toLowerCase() ||
        deviceOutputAfter.pointer.toLowerCase() !==
          pair.enter.arguments[5].toLowerCase()
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} changed its CreateDevice output pointer`,
        );
      }
      const presentationChangedBytes = observation.presentationParameters.before.bytes.reduce(
        (count, byte, index) =>
          count +
          (byte === observation.presentationParameters.after.bytes[index] ? 0 : 1),
        0,
      );
      const deviceOutputChangedBytes = observation.deviceOutput.before.bytes.reduce(
        (count, byte, index) =>
          count + (byte === observation.deviceOutput.after.bytes[index] ? 0 : 1),
        0,
      );
      return {
        sequence: pair.enter.sequence,
        callId: pair.enter.callId ?? null,
        parentCallId: pair.enter.parentCallId ?? null,
        caller:
          pair.enter.caller === undefined
            ? null
            : { module: pair.enter.caller.module, rva: pair.enter.caller.rva },
        adapter,
        deviceType,
        focusWindow: `0x${focusWindow.toString(16)}`,
        behaviorFlags,
        result: pair.leave.result,
        lastError: pair.leave.lastError ?? null,
        presentationChangedBytes,
        presentationParameters: {
          before: presentationBefore,
          after: presentationAfter,
        },
        deviceOutputChangedBytes,
        deviceOutput: {
          before: deviceOutputBefore,
          after: deviceOutputAfter,
        },
        returnedDevice: deviceOutputAfter.value,
        immediateConsumer: observation.immediateConsumer ?? null,
      };
    });

  const createTextures = apiPairing.pairs
    .filter(
      (pair) =>
        pair.enter.interface === "IDirect3DDevice9" &&
        pair.enter.vtableSlot === 23 &&
        pair.enter.api === "CreateTexture" &&
        pair.leave.d3d9CreateTexture !== undefined,
    )
    .map((pair) => {
      if (!Array.isArray(pair.enter.arguments) || pair.enter.arguments.length !== 8) {
        throw new Error(
          `api-enter sequence ${pair.enter.sequence} has invalid CreateTexture arguments`,
        );
      }
      const observation = pair.leave.d3d9CreateTexture;
      if (observation === null || typeof observation !== "object") {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} has invalid CreateTexture observation`,
        );
      }
      const textureOutputBefore = normalizePointerSlotSnapshot(
        observation.textureOutput?.before,
        pair.leave.sequence,
        "before",
      );
      const textureOutputAfter = normalizePointerSlotSnapshot(
        observation.textureOutput?.after,
        pair.leave.sequence,
        "after",
      );
      if (
        textureOutputBefore.pointer.toLowerCase() !==
          textureOutputAfter.pointer.toLowerCase() ||
        textureOutputAfter.pointer.toLowerCase() !==
          pair.enter.arguments[6].toLowerCase()
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} changed its CreateTexture output pointer`,
        );
      }
      const outputChangedBytes = textureOutputBefore.bytes.reduce(
        (count, byte, index) =>
          count + (byte === textureOutputAfter.bytes[index] ? 0 : 1),
        0,
      );
      return {
        sequence: pair.enter.sequence,
        callId: pair.enter.callId ?? null,
        parentCallId: pair.enter.parentCallId ?? null,
        caller:
          pair.enter.caller === undefined
            ? null
            : { module: pair.enter.caller.module, rva: pair.enter.caller.rva },
        object: pair.enter.object,
        arguments: [...pair.enter.arguments],
        result: pair.leave.result,
        lastError: pair.leave.lastError ?? null,
        outputChangedBytes,
        textureOutput: {
          before: textureOutputBefore,
          after: textureOutputAfter,
        },
        returnedTexture: textureOutputAfter.value,
        immediateConsumer: observation.immediateConsumer ?? null,
      };
    });

  const createVertexBuffers = apiPairing.pairs
    .filter(
      (pair) =>
        pair.enter.interface === "IDirect3DDevice9" &&
        pair.enter.vtableSlot === 26 &&
        pair.enter.api === "CreateVertexBuffer" &&
        pair.leave.d3d9CreateVertexBuffer !== undefined,
    )
    .map((pair) => {
      if (!Array.isArray(pair.enter.arguments) || pair.enter.arguments.length !== 6) {
        throw new Error(
          `api-enter sequence ${pair.enter.sequence} has invalid CreateVertexBuffer arguments`,
        );
      }
      const observation = pair.leave.d3d9CreateVertexBuffer;
      if (observation === null || typeof observation !== "object") {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} has invalid CreateVertexBuffer observation`,
        );
      }
      const outputBefore = normalizePointerSlotSnapshot(
        observation.vertexBufferOutput?.before,
        pair.leave.sequence,
        "before",
      );
      const outputAfter = normalizePointerSlotSnapshot(
        observation.vertexBufferOutput?.after,
        pair.leave.sequence,
        "after",
      );
      if (
        outputBefore.pointer.toLowerCase() !== outputAfter.pointer.toLowerCase() ||
        outputAfter.pointer.toLowerCase() !== pair.enter.arguments[4].toLowerCase()
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} changed its CreateVertexBuffer output pointer`,
        );
      }
      return {
        sequence: pair.enter.sequence,
        callId: pair.enter.callId ?? null,
        parentCallId: pair.enter.parentCallId ?? null,
        caller:
          pair.enter.caller === undefined
            ? null
            : { module: pair.enter.caller.module, rva: pair.enter.caller.rva },
        object: pair.enter.object,
        arguments: [...pair.enter.arguments],
        result: pair.leave.result,
        lastError: pair.leave.lastError ?? null,
        vertexBufferOutput: { before: outputBefore, after: outputAfter },
        returnedVertexBuffer: outputAfter.value,
        immediateConsumer: observation.immediateConsumer ?? null,
      };
    });

  const initialTextureCompletionEvents = events.filter(
    (event) => event.kind === "d3d9-initial-textures-complete",
  );
  const requiresInitialTextureCompletion =
    selectedObservations.length === 1 &&
    selectedObservations[0] === "d3d9-initial-textures" &&
    traceStart.arguments?.stopOnEvent === "d3d9-initial-textures-complete";
  if (
    initialTextureCompletionEvents.length > 1 ||
    (requiresInitialTextureCompletion && initialTextureCompletionEvents.length !== 1)
  ) {
    throw new Error(
      "d3d9-initial-textures observation requires exactly one structured completion",
    );
  }
  const pairsByCallId = new Map(
    apiPairing.pairs.map((pair) => [pair.enter.callId, pair]),
  );
  const createTexturesBySequence = new Map(
    createTextures.map((texture) => [texture.sequence, texture]),
  );
  const createVertexBuffersBySequence = new Map(
    createVertexBuffers.map((buffer) => [buffer.sequence, buffer]),
  );
  const initialTextureSequences = initialTextureCompletionEvents.map((completion) => {
    const expectedArguments = [
      [0x400n, 0x400n, 1n, 0n, 0x16n, 2n],
      [0x400n, 0x400n, 1n, 0n, 0x16n, 0n],
      [0x400n, 0x400n, 1n, 0x200n, 0x16n, 0n],
    ];
    const expectedCallers = [0x5a715, 0x5a7b7, 0x5a7dc];
    if (
      completion.profile !== "x86-bgi-initial-textures-v1" ||
      typeof completion.device !== "string" ||
      !/^0x[0-9a-f]+$/iu.test(completion.device) ||
      BigInt(completion.device) === 0n ||
      !Array.isArray(completion.calls) ||
      completion.calls.length !== expectedArguments.length ||
      !Number.isInteger(completion.sequence)
    ) {
      throw new Error("d3d9-initial-textures completion has invalid structure");
    }
    const executableModule = path.win32.basename(traceStart.executable).toLowerCase();
    const returnedTextures = new Set();
    let threadId = null;
    const calls = completion.calls.map((snapshot, index) => {
      const pair = pairsByCallId.get(snapshot?.callId);
      const createTexture =
        pair === undefined ? undefined : createTexturesBySequence.get(pair.enter.sequence);
      if (
        pair === undefined ||
        createTexture === undefined ||
        snapshot.api !== "CreateTexture" ||
        snapshot.interface !== "IDirect3DDevice9" ||
        snapshot.vtableSlot !== 23 ||
        snapshot.object?.toLowerCase() !== completion.device.toLowerCase() ||
        JSON.stringify(snapshot.arguments) !== JSON.stringify(pair.enter.arguments) ||
        snapshot.result !== pair.leave.result ||
        snapshot.lastError !== pair.leave.lastError ||
        JSON.stringify(snapshot.d3d9CreateTexture) !==
          JSON.stringify(pair.leave.d3d9CreateTexture) ||
        snapshot.caller?.module !== pair.enter.caller?.module ||
        snapshot.caller?.rva !== pair.enter.caller?.rva ||
        pair.enter.parentCallId !== null ||
        pair.enter.caller?.module?.toLowerCase() !== executableModule ||
        pair.enter.caller?.rva !== expectedCallers[index] ||
        pair.enter.object?.toLowerCase() !== completion.device.toLowerCase() ||
        pair.enter.arguments.length !== 8 ||
        expectedArguments[index].some(
          (expected, argumentIndex) =>
            BigInt(pair.enter.arguments[argumentIndex]) !== expected,
        ) ||
        BigInt(pair.enter.arguments[7]) !== 0n ||
        BigInt(pair.leave.result) !== 0n ||
        pair.leave.lastError !== 0 ||
        BigInt(createTexture.returnedTexture) === 0n ||
        createTexture.textureOutput.after.pointer.toLowerCase() !==
          pair.enter.arguments[6].toLowerCase()
      ) {
        throw new Error(
          `d3d9-initial-textures completion call ${index} violates its caller, tuple, or output contract`,
        );
      }
      threadId ??= pair.enter.threadId;
      if (
        pair.enter.threadId !== threadId ||
        returnedTextures.has(createTexture.returnedTexture.toLowerCase())
      ) {
        throw new Error(
          "d3d9-initial-textures completion does not describe one thread with distinct texture identities",
        );
      }
      returnedTextures.add(createTexture.returnedTexture.toLowerCase());
      return createTexture;
    });
    return {
      sequence: completion.sequence,
      profile: completion.profile,
      threadId,
      device: completion.device,
      calls,
    };
  });

  const initialVertexBufferCompletionEvents = events.filter(
    (event) => event.kind === "d3d9-initial-vertex-buffer-complete",
  );
  const requiresInitialVertexBufferCompletion =
    selectedObservations.length === 1 &&
    selectedObservations[0] === "d3d9-initial-vertex-buffer" &&
    traceStart.arguments?.stopOnEvent ===
      "d3d9-initial-vertex-buffer-complete";
  if (
    initialVertexBufferCompletionEvents.length > 1 ||
    (requiresInitialVertexBufferCompletion &&
      initialVertexBufferCompletionEvents.length !== 1)
  ) {
    throw new Error(
      "d3d9-initial-vertex-buffer observation requires exactly one structured completion",
    );
  }
  const initialVertexBufferSequences = initialVertexBufferCompletionEvents.map(
    (completion) => {
      if (
        completion.profile !== "x86-bgi-initial-vertex-buffer-v1" ||
        typeof completion.device !== "string" ||
        !/^0x[0-9a-f]+$/iu.test(completion.device) ||
        BigInt(completion.device) === 0n ||
        typeof completion.vertexBuffer !== "string" ||
        !/^0x[0-9a-f]+$/iu.test(completion.vertexBuffer) ||
        BigInt(completion.vertexBuffer) === 0n ||
        !Array.isArray(completion.calls) ||
        completion.calls.length !== 3 ||
        !Number.isInteger(completion.sequence)
      ) {
        throw new Error(
          "d3d9-initial-vertex-buffer completion has invalid structure",
        );
      }
      const executableModule = path.win32
        .basename(traceStart.executable)
        .toLowerCase();
      const pairs = completion.calls.map((snapshot, index) => {
        const pair = pairsByCallId.get(snapshot?.callId);
        if (
          pair === undefined ||
          snapshot.api !== pair.enter.api ||
          snapshot.interface !== pair.enter.interface ||
          snapshot.vtableSlot !== pair.enter.vtableSlot ||
          snapshot.object?.toLowerCase() !== pair.enter.object?.toLowerCase() ||
          JSON.stringify(snapshot.arguments) !==
            JSON.stringify(pair.enter.arguments) ||
          snapshot.result !== pair.leave.result ||
          snapshot.lastError !== pair.leave.lastError ||
          snapshot.caller?.module !== pair.enter.caller?.module ||
          snapshot.caller?.rva !== pair.enter.caller?.rva ||
          pair.enter.parentCallId !== null ||
          pair.enter.caller?.module?.toLowerCase() !== executableModule ||
          pair.enter.caller?.rva !== [0x5a890, 0x5aa96, 0x5aab2][index] ||
          pair.enter.api !== ["CreateVertexBuffer", "Lock", "Unlock"][index] ||
          pair.enter.vtableSlot !== [26, 11, 12][index] ||
          BigInt(pair.leave.result) !== 0n ||
          pair.leave.lastError !== 0
        ) {
          throw new Error(
            `d3d9-initial-vertex-buffer completion call ${index} violates its caller, method, or result contract`,
          );
        }
        return pair;
      });
      if (
        !pairs.every((pair) => pair.enter.threadId === pairs[0].enter.threadId) ||
        pairs[0].enter.object.toLowerCase() !== completion.device.toLowerCase() ||
        pairs[1].enter.object.toLowerCase() !==
          completion.vertexBuffer.toLowerCase() ||
        pairs[2].enter.object.toLowerCase() !==
          completion.vertexBuffer.toLowerCase() ||
        pairs[0].enter.arguments.length !== 6 ||
        pairs[0].enter.arguments
          .slice(0, 4)
          .some(
            (argument, index) =>
              BigInt(argument) !== [0x70n, 0x8n, 0x144n, 0x1n][index],
          ) ||
        BigInt(pairs[0].enter.arguments[5]) !== 0n ||
        pairs[1].enter.arguments.length !== 4 ||
        [0, 1, 3].some(
          (index) => BigInt(pairs[1].enter.arguments[index]) !== 0n,
        ) ||
        pairs[2].enter.arguments.length !== 0
      ) {
        throw new Error(
          "d3d9-initial-vertex-buffer completion violates its identity or argument contract",
        );
      }

      const create = createVertexBuffersBySequence.get(
        pairs[0].enter.sequence,
      );
      if (
        create === undefined ||
        create.returnedVertexBuffer.toLowerCase() !==
          completion.vertexBuffer.toLowerCase() ||
        create.vertexBufferOutput.after.pointer.toLowerCase() !==
          pairs[0].enter.arguments[4].toLowerCase() ||
        JSON.stringify(completion.calls[0].d3d9CreateVertexBuffer) !==
          JSON.stringify(pairs[0].leave.d3d9CreateVertexBuffer)
      ) {
        throw new Error(
          "d3d9-initial-vertex-buffer completion violates its creation output contract",
        );
      }

      const lockObservation = pairs[1].leave.d3d9VertexBufferLock;
      if (
        lockObservation === null ||
        typeof lockObservation !== "object" ||
        JSON.stringify(completion.calls[1].d3d9VertexBufferLock) !==
          JSON.stringify(lockObservation)
      ) {
        throw new Error(
          "d3d9-initial-vertex-buffer completion has invalid Lock observation",
        );
      }
      const lockOutputBefore = normalizePointerSlotSnapshot(
        lockObservation.dataOutput?.before,
        pairs[1].leave.sequence,
        "Lock before",
      );
      const lockOutputAfter = normalizePointerSlotSnapshot(
        lockObservation.dataOutput?.after,
        pairs[1].leave.sequence,
        "Lock after",
      );
      const dataBefore = normalizeBoundedRawBytesSnapshot(
        lockObservation.dataBefore,
        112,
        pairs[1].leave.sequence,
        "Lock data-before",
      );
      if (
        lockOutputBefore.pointer.toLowerCase() !==
          lockOutputAfter.pointer.toLowerCase() ||
        lockOutputAfter.pointer.toLowerCase() !==
          pairs[1].enter.arguments[2].toLowerCase() ||
        BigInt(lockOutputAfter.value) === 0n ||
        dataBefore.pointer.toLowerCase() !== lockOutputAfter.value.toLowerCase()
      ) {
        throw new Error(
          "d3d9-initial-vertex-buffer completion violates its Lock pointer contract",
        );
      }

      const unlockObservation = pairs[2].leave.d3d9VertexBufferUnlock;
      if (
        unlockObservation === null ||
        typeof unlockObservation !== "object" ||
        JSON.stringify(completion.calls[2].d3d9VertexBufferUnlock) !==
          JSON.stringify(unlockObservation)
      ) {
        throw new Error(
          "d3d9-initial-vertex-buffer completion has invalid Unlock observation",
        );
      }
      const dataAtUnlock = normalizeBoundedRawBytesSnapshot(
        unlockObservation.dataAtUnlock,
        112,
        pairs[2].leave.sequence,
        "Unlock data",
      );
      if (
        dataAtUnlock.pointer.toLowerCase() !== lockOutputAfter.value.toLowerCase()
      ) {
        throw new Error(
          "d3d9-initial-vertex-buffer completion changed its Lock data pointer",
        );
      }
      return {
        sequence: completion.sequence,
        profile: completion.profile,
        threadId: pairs[0].enter.threadId,
        device: completion.device,
        vertexBuffer: completion.vertexBuffer,
        create,
        lock: {
          sequence: pairs[1].enter.sequence,
          caller: {
            module: pairs[1].enter.caller.module,
            rva: pairs[1].enter.caller.rva,
          },
          arguments: [...pairs[1].enter.arguments],
          result: pairs[1].leave.result,
          lastError: pairs[1].leave.lastError,
          dataOutput: { before: lockOutputBefore, after: lockOutputAfter },
          dataBefore,
        },
        unlock: {
          sequence: pairs[2].enter.sequence,
          caller: {
            module: pairs[2].enter.caller.module,
            rva: pairs[2].enter.caller.rva,
          },
          result: pairs[2].leave.result,
          lastError: pairs[2].leave.lastError,
          dataAtUnlock,
        },
      };
    },
  );

  const getSurfaceLevels = apiPairing.pairs
    .filter(
      (pair) =>
        pair.enter.interface === "IDirect3DTexture9" &&
        pair.enter.vtableSlot === 18 &&
        pair.enter.api === "GetSurfaceLevel" &&
        pair.leave.d3d9GetSurfaceLevel !== undefined,
    )
    .map((pair) => {
      if (!Array.isArray(pair.enter.arguments) || pair.enter.arguments.length !== 2) {
        throw new Error(
          `api-enter sequence ${pair.enter.sequence} has invalid GetSurfaceLevel arguments`,
        );
      }
      const level = normalizeD3d9U32Argument(
        pair.enter.arguments[0],
        pair.enter.sequence,
        0,
        "GetSurfaceLevel",
      );
      normalizeD3d9U32Argument(
        pair.enter.arguments[1],
        pair.enter.sequence,
        1,
        "GetSurfaceLevel",
      );
      const observation = pair.leave.d3d9GetSurfaceLevel;
      if (
        observation === null ||
        typeof observation !== "object" ||
        observation.level !== level
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} has invalid GetSurfaceLevel observation`,
        );
      }
      const surfaceOutputBefore = normalizePointerSlotSnapshot(
        observation.surfaceOutput?.before,
        pair.leave.sequence,
        "before",
      );
      const surfaceOutputAfter = normalizePointerSlotSnapshot(
        observation.surfaceOutput?.after,
        pair.leave.sequence,
        "after",
      );
      if (
        surfaceOutputBefore.pointer.toLowerCase() !==
          surfaceOutputAfter.pointer.toLowerCase() ||
        surfaceOutputAfter.pointer.toLowerCase() !==
          pair.enter.arguments[1].toLowerCase()
      ) {
        throw new Error(
          `api-leave sequence ${pair.leave.sequence} changed its GetSurfaceLevel output pointer`,
        );
      }
      const outputChangedBytes = surfaceOutputBefore.bytes.reduce(
        (count, byte, index) =>
          count + (byte === surfaceOutputAfter.bytes[index] ? 0 : 1),
        0,
      );
      return {
        sequence: pair.enter.sequence,
        callId: pair.enter.callId ?? null,
        parentCallId: pair.enter.parentCallId ?? null,
        caller:
          pair.enter.caller === undefined
            ? null
            : { module: pair.enter.caller.module, rva: pair.enter.caller.rva },
        object: pair.enter.object,
        level,
        result: pair.leave.result,
        lastError: pair.leave.lastError ?? null,
        outputChangedBytes,
        surfaceOutput: {
          before: surfaceOutputBefore,
          after: surfaceOutputAfter,
        },
        returnedSurface: surfaceOutputAfter.value,
        immediateConsumer: observation.immediateConsumer ?? null,
      };
    });

  const summarizeD3d9InterfaceCalls = (interfaceName, methods) =>
    apiPairing.pairs
      .filter((pair) => pair.enter.interface === interfaceName)
      .map((pair) => {
        const method = methods[pair.enter.vtableSlot];
        if (
          method === undefined ||
          method[0] !== pair.enter.api ||
          !Array.isArray(pair.enter.arguments) ||
          pair.enter.arguments.length !== method[1] ||
          !pair.enter.arguments.every(
            (argument) =>
              typeof argument === "string" && /^0x[0-9a-f]+$/iu.test(argument),
          ) ||
          typeof pair.leave.result !== "string" ||
          !/^0x[0-9a-f]+$/iu.test(pair.leave.result)
        ) {
          throw new Error(
            `${interfaceName} api pair at sequence ${pair.enter.sequence} is invalid`,
          );
        }
        return {
          sequence: pair.enter.sequence,
          callId: pair.enter.callId ?? null,
          parentCallId: pair.enter.parentCallId ?? null,
          caller:
            pair.enter.caller === undefined
              ? null
              : { module: pair.enter.caller.module, rva: pair.enter.caller.rva },
          slot: pair.enter.vtableSlot,
          method: pair.enter.api,
          object: pair.enter.object,
          arguments: [...pair.enter.arguments],
          result: pair.leave.result,
          lastError: pair.leave.lastError ?? null,
        };
      });
  const deviceCalls = summarizeD3d9InterfaceCalls(
    "IDirect3DDevice9",
    idirect3ddevice9Methods,
  );
  const textureCalls = summarizeD3d9InterfaceCalls(
    "IDirect3DTexture9",
    idirect3dtexture9Methods,
  );
  const vertexBufferCalls = summarizeD3d9InterfaceCalls(
    "IDirect3DVertexBuffer9",
    idirect3dvertexbuffer9Methods,
  );
  const surfaceCalls = summarizeD3d9InterfaceCalls(
    "IDirect3DSurface9",
    idirect3dsurface9Methods,
  );

  const timeGetTimeOracleEvents = events.filter(
    (event) => event.kind === "time-get-time-oracle",
  );
  const timeGetTimeCompletionEvents = events.filter(
    (event) => event.kind === "time-get-time-complete",
  );
  const requiresTimeGetTimeCompletion =
    selectedObservations.length === 1 &&
    selectedObservations[0] === "time-get-time" &&
    traceStart.arguments?.stopOnEvent === "time-get-time-complete";
  if (
    timeGetTimeOracleEvents.length > 1 ||
    timeGetTimeCompletionEvents.length > 1 ||
    (requiresTimeGetTimeCompletion &&
      (timeGetTimeOracleEvents.length !== 1 ||
        timeGetTimeCompletionEvents.length !== 1))
  ) {
    throw new Error(
      "time-get-time observation requires exactly one oracle and structured completion",
    );
  }

  const isU32 = (value) =>
    Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff;
  const timeGetTime = timeGetTimeCompletionEvents.map((completion) => {
    const oracle = timeGetTimeOracleEvents[0];
    const sentinels = [0, 87, 0x1234_5678];
    if (
      oracle === undefined ||
      oracle.profile !== "x86-time-get-time-shared-tick-epoch-v1" ||
      !Array.isArray(oracle.calls) ||
      oracle.calls.length !== sentinels.length ||
      oracle.allResultsMatchHooks !== true ||
      oracle.allPreserveLastError !== true ||
      oracle.allResultsWithinTickCountBrackets !== true ||
      completion.profile !== "x86-bgi-time-get-time-v1" ||
      completion.oracleProfile !== oracle.profile ||
      !Array.isArray(completion.mainModuleCalls) ||
      completion.mainModuleCalls.length !== 1 ||
      completion.sequence <= oracle.sequence
    ) {
      throw new Error("time-get-time completion or oracle has invalid structure");
    }

    const oraclePairs = apiPairing.pairs.filter(
      (pair) =>
        pair.enter.api === "timeGetTime" &&
        pair.enter.caller?.module?.toLowerCase() === "frida-agent.dll",
    );
    if (oraclePairs.length !== sentinels.length) {
      throw new Error("time-get-time oracle calls do not match their paired traces");
    }
    const oracleCalls = oracle.calls.map((call, index) => {
      const pair = oraclePairs[index];
      const sentinel = sentinels[index];
      if (
        !isU32(call.sentinel) ||
        call.sentinel !== sentinel ||
        ![
          call.tickCountBefore,
          call.systemResult,
          call.tickCountAfter,
          call.tickCountSpan,
          call.resultOffset,
          call.hookResult,
          call.hookLastErrorAfter,
          call.systemLastError,
        ].every(isU32) ||
        call.tickCountSpan !==
          ((call.tickCountAfter - call.tickCountBefore) >>> 0) ||
        call.resultOffset !== ((call.systemResult - call.tickCountBefore) >>> 0) ||
        call.resultOffset > call.tickCountSpan ||
        call.hookResult !== call.systemResult ||
        call.hookLastErrorAfter !== sentinel ||
        call.systemLastError !== sentinel ||
        call.resultWithinTickCountBracket !== true ||
        call.hookResultMatches !== true ||
        call.hookPreservesLastError !== true ||
        call.systemPreservesLastError !== true ||
        pair.enter.module.toLowerCase() !== "winmm.dll" ||
        pair.enter.parentCallId !== null ||
        !Array.isArray(pair.enter.arguments) ||
        pair.enter.arguments.length !== 0 ||
        pair.enter.lastErrorBefore !== sentinel ||
        pair.leave.lastError !== sentinel ||
        BigInt(pair.leave.result) !== BigInt(call.systemResult) ||
        pair.leave.timeGetTime?.result !== call.systemResult
      ) {
        throw new Error(
          `time-get-time oracle call ${index} violates its tick or LastError contract`,
        );
      }
      return { ...call, callId: pair.enter.callId, sequence: pair.enter.sequence };
    });

    const snapshot = completion.mainModuleCalls[0];
    const pair = apiPairing.pairs.find(
      (candidate) => candidate.enter.callId === snapshot.callId,
    );
    const executableModule = path.win32.basename(traceStart.executable).toLowerCase();
    if (
      pair === undefined ||
      pair.enter.api !== "timeGetTime" ||
      pair.enter.module.toLowerCase() !== "winmm.dll" ||
      pair.enter.caller?.module?.toLowerCase() !== executableModule ||
      pair.enter.caller?.rva !== 0x91ebf ||
      pair.enter.parentCallId !== null ||
      !Array.isArray(pair.enter.arguments) ||
      pair.enter.arguments.length !== 0 ||
      pair.leave.sequence + 1 !== completion.sequence ||
      pair.leave.timeGetTime?.result !== snapshot.result ||
      BigInt(pair.leave.result) !== BigInt(snapshot.result) ||
      snapshot.returnAddress.toLowerCase() !==
        pair.enter.caller.address.toLowerCase() ||
      snapshot.result !== pair.leave.timeGetTime.result ||
      snapshot.lastErrorBefore !== pair.enter.lastErrorBefore ||
      snapshot.lastError !== pair.leave.lastError ||
      snapshot.lastErrorBefore !== snapshot.lastError ||
      JSON.stringify(snapshot.immediateConsumer) !==
        JSON.stringify(pair.enter.timeGetTime?.immediateConsumer)
    ) {
      throw new Error(
        "time-get-time completion does not match its primary paired trace",
      );
    }

    const consumer = snapshot.immediateConsumer;
    const entry = consumer?.apiContinuation?.entry;
    const target = consumer?.apiContinuation?.directJumpTarget;
    const frames = consumer?.frameCallerContinuations?.frames;
    const expectedConsumer = [
      ["mov", "ecx, dword ptr [0x1e2358]"],
      ["cmp", "eax, ecx"],
      ["jb", "0x121edd"],
      ["mov", "edx, dword ptr [0x188dd8]"],
      ["add", "edx, ecx"],
      ["cmp", "edx, eax"],
      ["jae", "0x121ee7"],
      ["mov", "edx, eax"],
      ["sub", "edx, ecx"],
      ["add", "dword ptr [0x1e2360], edx"],
      ["mov", "ecx, dword ptr [0x1e235c]"],
      ["mov", "dword ptr [0x1e2358], eax"],
      ["test", "ecx, ecx"],
      ["je", "0x121ef9"],
      ["mov", "eax, ecx"],
      ["ret", ""],
      ["sub", "eax, dword ptr [0x1e2360]"],
      ["ret", ""],
    ];
    if (
      consumer === null ||
      typeof consumer !== "object" ||
      entry?.address?.toLowerCase() !== snapshot.returnAddress.toLowerCase() ||
      entry?.instructions?.[0]?.mnemonic !== "jmp" ||
      entry.instructions[0].opStr?.toLowerCase() !== target?.address?.toLowerCase() ||
      target?.address?.toLowerCase() !== "0x121ec7" ||
      !Array.isArray(target.instructions) ||
      target.instructions.length < expectedConsumer.length ||
      expectedConsumer.some(
        ([mnemonic, opStr], index) =>
          target.instructions[index]?.mnemonic !== mnemonic ||
          target.instructions[index]?.opStr?.toLowerCase() !== opStr,
      ) ||
      !Array.isArray(frames) ||
      frames.length === 0 ||
      frames.some(
        (frame, index) =>
          frame.depth !== index ||
          frame.caller?.module?.toLowerCase() !== executableModule,
      )
    ) {
      throw new Error(
        "time-get-time primary consumer is not the observed wrap-aware EAX delta flow",
      );
    }

    return {
      sequence: completion.sequence,
      profile: completion.profile,
      oracle: {
        sequence: oracle.sequence,
        profile: oracle.profile,
        calls: oracleCalls,
      },
      mainModuleCalls: [
        {
          sequence: pair.enter.sequence,
          callId: pair.enter.callId,
          threadId: pair.enter.threadId,
          caller: {
            module: pair.enter.caller.module,
            rva: pair.enter.caller.rva,
          },
          returnAddress: snapshot.returnAddress,
          result: snapshot.result,
          lastErrorBefore: snapshot.lastErrorBefore,
          lastError: snapshot.lastError,
          consumer: {
            profile: "wrap-aware-eax-delta-v1",
            directJumpTarget: target.address,
            frameCallers: frames.map((frame) => ({
              depth: frame.depth,
              module: frame.caller.module,
              rva: frame.caller.rva,
            })),
          },
        },
      ],
    };
  });

  const surfaceDcCompletionEvents = events.filter(
    (event) => event.kind === "d3d9-surface-get-dc-complete",
  );
  const requiresSurfaceDcCompletion =
    selectedObservations.length === 1 &&
    selectedObservations[0] === "d3d9-surface-get-dc" &&
    traceStart.arguments?.stopOnEvent === "d3d9-surface-get-dc-complete";
  if (
    surfaceDcCompletionEvents.length > 1 ||
    (requiresSurfaceDcCompletion && surfaceDcCompletionEvents.length !== 1)
  ) {
    throw new Error(
      "d3d9-surface-get-dc observation requires exactly one structured completion",
    );
  }
  const pairsByEnterSequence = new Map(
    apiPairing.pairs.map((pair) => [pair.enter.sequence, pair]),
  );
  const surfaceDcFlows = surfaceDcCompletionEvents.map((completion) => {
    const expectedCalls = [
      { api: "GetDC", module: "d3d9.dll", interface: "IDirect3DSurface9", slot: 15 },
      { api: "GetStockObject", module: "gdi32.dll", interface: null, slot: null },
      { api: "FillRect", module: "user32.dll", interface: null, slot: null },
      { api: "ReleaseDC", module: "d3d9.dll", interface: "IDirect3DSurface9", slot: 16 },
      { api: "Release", module: "d3d9.dll", interface: "IDirect3DSurface9", slot: 2 },
    ];
    if (
      completion.profile !== "x86-bgi-system-memory-surface-dc-fill-v1" ||
      typeof completion.surface !== "string" ||
      !/^0x[0-9a-f]+$/iu.test(completion.surface) ||
      BigInt(completion.surface) === 0n ||
      !Array.isArray(completion.calls) ||
      completion.calls.length !== expectedCalls.length ||
      !Number.isInteger(completion.sequence) ||
      completion.sequence < expectedCalls.length * 2
    ) {
      throw new Error("d3d9-surface-get-dc completion has invalid structure");
    }

    const firstEnterSequence = completion.sequence - expectedCalls.length * 2;
    const pairs = expectedCalls.map((expected, index) => {
      const enterSequence = firstEnterSequence + index * 2;
      const pair = pairsByEnterSequence.get(enterSequence);
      const snapshot = completion.calls[index];
      if (
        pair === undefined ||
        pair.leave.sequence !== enterSequence + 1 ||
        pair.enter.api !== expected.api ||
        pair.enter.module.toLowerCase() !== expected.module ||
        (pair.enter.interface ?? null) !== expected.interface ||
        (pair.enter.vtableSlot ?? null) !== expected.slot ||
        snapshot === null ||
        typeof snapshot !== "object" ||
        snapshot.api !== pair.enter.api ||
        JSON.stringify(snapshot.arguments) !== JSON.stringify(pair.enter.arguments) ||
        snapshot.result !== pair.leave.result ||
        snapshot.lastError !== pair.leave.lastError ||
        snapshot.caller?.module !== pair.enter.caller?.module ||
        snapshot.caller?.rva !== pair.enter.caller?.rva ||
        (expected.interface !== null &&
          (snapshot.interface !== expected.interface ||
            snapshot.vtableSlot !== expected.slot ||
            snapshot.object !== pair.enter.object)) ||
        (expected.interface === null &&
          snapshot.module?.toLowerCase() !== expected.module)
      ) {
        throw new Error(
          `d3d9-surface-get-dc completion call ${index} does not match its paired trace`,
        );
      }
      return pair;
    });
    if (pairs.at(-1).leave.sequence + 1 !== completion.sequence) {
      throw new Error("d3d9-surface-get-dc completion is not adjacent to its call chain");
    }
    const threadId = pairs[0].enter.threadId;
    const executableModule = path.win32.basename(traceStart.executable).toLowerCase();
    if (
      pairs.some(
        (pair) =>
          pair.enter.threadId !== threadId ||
          pair.enter.parentCallId !== null ||
          pair.enter.caller?.module?.toLowerCase() !== executableModule,
      )
    ) {
      throw new Error(
        "d3d9-surface-get-dc completion does not describe one primary-thread caller chain",
      );
    }

    const [getDc, getStockObject, fillRect, releaseDc, release] = pairs;
    const getDcObservation = getDc.leave.d3d9SurfaceGetDc;
    const outputBefore = normalizePointerSlotSnapshot(
      getDcObservation?.output?.before,
      getDc.leave.sequence,
      "before",
    );
    const outputAfter = normalizePointerSlotSnapshot(
      getDcObservation?.output?.after,
      getDc.leave.sequence,
      "after",
    );
    const rect = fillRect.leave.d3d9SurfaceDcGdi?.rect;
    const expectedRect = { left: 0, top: 0, right: 800, bottom: 600 };
    if (
      JSON.stringify(completion.calls[0].d3d9SurfaceGetDc) !==
        JSON.stringify(getDcObservation) ||
      outputBefore.pointer.toLowerCase() !== outputAfter.pointer.toLowerCase() ||
      outputAfter.pointer.toLowerCase() !== getDc.enter.arguments[0].toLowerCase() ||
      BigInt(outputAfter.value) === 0n ||
      getDc.enter.object.toLowerCase() !== completion.surface.toLowerCase() ||
      releaseDc.enter.object.toLowerCase() !== completion.surface.toLowerCase() ||
      release.enter.object.toLowerCase() !== completion.surface.toLowerCase() ||
      BigInt(getDc.leave.result) !== 0n ||
      getStockObject.enter.arguments.length !== 1 ||
      getStockObject.enter.arguments[0] !== 4 ||
      BigInt(getStockObject.leave.result) === 0n ||
      fillRect.enter.arguments.length !== 3 ||
      BigInt(fillRect.enter.arguments[0]) !== BigInt(outputAfter.value) ||
      BigInt(fillRect.enter.arguments[2]) !== BigInt(getStockObject.leave.result) ||
      BigInt(fillRect.leave.result) !== 1n ||
      rect?.profile !== "x86-rect-v1" ||
      rect.raw?.pointer?.toLowerCase() !== fillRect.enter.arguments[1].toLowerCase() ||
      JSON.stringify(rect.fields) !== JSON.stringify(expectedRect) ||
      JSON.stringify(completion.calls[1].rect) !==
        JSON.stringify(getStockObject.leave.d3d9SurfaceDcGdi?.rect ?? null) ||
      JSON.stringify(completion.calls[2].rect) !== JSON.stringify(rect) ||
      releaseDc.enter.arguments.length !== 1 ||
      BigInt(releaseDc.enter.arguments[0]) !== BigInt(outputAfter.value) ||
      BigInt(releaseDc.leave.result) !== 0n ||
      release.enter.arguments.length !== 0 ||
      BigInt(release.leave.result) !== 1n
    ) {
      throw new Error(
        "d3d9-surface-get-dc completion violates its surface, HDC, brush, or fill identity contract",
      );
    }

    return {
      sequence: completion.sequence,
      profile: completion.profile,
      threadId,
      surface: completion.surface,
      hdc: outputAfter.value,
      stockBrush: getStockObject.leave.result,
      rect: expectedRect,
      calls: pairs.map((pair) => ({
        sequence: pair.enter.sequence,
        callId: pair.enter.callId,
        caller: {
          module: pair.enter.caller.module,
          rva: pair.enter.caller.rva,
        },
        module: pair.enter.module,
        interface: pair.enter.interface ?? null,
        slot: pair.enter.vtableSlot ?? null,
        method: pair.enter.api,
        object: pair.enter.object ?? null,
        arguments: [...pair.enter.arguments],
        result: pair.leave.result,
        lastError: pair.leave.lastError,
      })),
      getDcOutput: { before: outputBefore, after: outputAfter },
    };
  });

  return {
    schema: summarySchema,
    source: {
      path: sourcePath,
      bytes: buffer.length,
      lines: lines.length,
      sha256: crypto.createHash("sha256").update(buffer).digest("hex"),
    },
    trace: {
      inputSchema: traceStart.schema,
      executable: traceStart.executable,
      executableBytes: traceStart.executableBytes ?? null,
      executableSha256: traceStart.executableSha256 ?? null,
      startedAt: traceStart.timestamp,
      stoppedAt: traceStop.timestamp,
      durationMs: stoppedAtMs - startedAtMs,
      stopReason: traceStop.reason ?? null,
      detachedReason: traceStop.detachedReason ?? null,
      crash: traceStop.crash ?? null,
      tools: {
        python: traceStart.python ?? null,
        frida: traceStart.frida ?? null,
        platform: traceStart.platform ?? null,
      },
      captureArguments: traceStart.arguments ?? null,
      architecture: agentReady.architecture,
      pointerSize: agentReady.pointerSize,
      processId: agentReady.processId,
      selectedVtables,
      sequence: {
        events: sequenced.length,
        first: sequenced.length === 0 ? null : sequenced[0].sequence,
        last: sequenced.length === 0 ? null : sequenced.at(-1).sequence,
        contiguous: true,
      },
      apiPairing: {
        contract: apiPairing.callMetadata ? "call-id-v1" : "legacy-thread-stack",
        enters: apiPairing.enters,
        leaves: apiPairing.leaves,
        pairs: apiPairing.pairs.length,
        maxDepth: apiPairing.maxDepth,
        balanced: apiPairing.enters === apiPairing.leaves,
      },
    },
    eventCounts: sortedCounts(eventCounts),
    diagnostics,
    modules,
    apis,
    vtables,
    winmm: {
      timeGetTime,
    },
    d3d9: {
      adapterIdentifiers,
      adapterDisplayModes,
      deviceTypeChecks,
      deviceFormatChecks,
      deviceCaps,
      createDevices,
      createTextures,
      createVertexBuffers,
      initialTextureSequences,
      initialVertexBufferSequences,
      getSurfaceLevels,
      surfaceDcFlows,
      deviceCalls,
      textureCalls,
      vertexBufferCalls,
      surfaceCalls,
    },
    categories,
    files: {
      hookCalls: categories.file.calls,
      logicalCalls: logicalFileCalls.length,
      uniquePaths: filePaths.size,
      topPaths: topFilePaths,
      titleRelative: {
        calls: titleFileList.reduce((total, file) => total + file.calls, 0),
        uniquePaths: titleFileList.length,
        successfulCalls: titleFileList.reduce(
          (total, file) => total + file.successfulCalls,
          0,
        ),
        failedCalls: titleFileList.reduce(
          (total, file) => total + file.failedCalls,
          0,
        ),
        successfulPaths: titleFileList
          .filter((file) => file.successfulCalls > 0)
          .map((file) => file.path),
        failedSearchRoots: [...failedSearchRoots.values()]
          .map((searchRoot) => ({
            root: searchRoot.root,
            calls: searchRoot.calls,
            uniquePaths: searchRoot.paths.size,
          }))
          .sort(
            (left, right) =>
              right.calls - left.calls || compareOrdinal(left.root, right.root),
          ),
        topFailedPaths: titleFileList
          .filter((file) => file.failedCalls > 0)
          .sort(
            (left, right) =>
              right.failedCalls - left.failedCalls ||
              compareOrdinal(left.path, right.path),
          )
          .slice(0, 20)
          .map((file) => ({ path: file.path, calls: file.failedCalls })),
      },
    },
  };
};

const parseArguments = (args) => {
  const options = {};
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--force") {
      options.force = true;
      continue;
    }
    if (argument !== "--input" && argument !== "--output") {
      throw new Error(`unknown argument: ${argument}`);
    }
    const value = args[index + 1];
    if (value === undefined) {
      throw new Error(`${argument} requires a value`);
    }
    options[argument.slice(2)] = value;
    index += 1;
  }
  if (options.input === undefined) {
    throw new Error("--input is required");
  }
  return options;
};

const main = () => {
  const options = parseArguments(process.argv.slice(2));
  const input = path.resolve(options.input);
  const buffer = fs.readFileSync(input);
  const summary = summarizeTrace(buffer, options.input);
  const json = `${JSON.stringify(summary, null, 2)}\n`;
  if (options.output === undefined) {
    process.stdout.write(json);
    return;
  }

  const output = path.resolve(options.output);
  if (input === output) {
    throw new Error("trace input and summary output must differ");
  }
  fs.mkdirSync(path.dirname(output), { recursive: true });
  try {
    fs.writeFileSync(output, json, { flag: options.force ? "w" : "wx" });
  } catch (error) {
    if (error.code === "EEXIST") {
      throw new Error(`summary output already exists (use --force): ${output}`);
    }
    throw error;
  }
  console.log(`trace summary written: ${output}`);
};

const isMain =
  process.argv[1] !== undefined &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  try {
    main();
  } catch (error) {
    console.error(`trace summary failed: ${error.message}`);
    process.exitCode = 1;
  }
}
