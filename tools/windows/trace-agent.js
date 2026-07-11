"use strict";

const schema = "vine.win32-trace.v2";
const agentOptions = globalThis.__vineTraceOptions ?? {
  vtables: [],
  observations: [],
};
const selectedVtables = new Set(agentOptions.vtables ?? []);
const selectedObservations = new Set(agentOptions.observations ?? []);
let sequence = 0;
let nextCallId = 1;
const attached = new Set();
const attachedVtableMethods = new Set();
const observedVtables = new Set();
const activeCallsByThread = new Map();

function emit(kind, fields = {}) {
  send({
    schema,
    sequence: sequence++,
    timestampMs: Date.now(),
    threadId: Process.getCurrentThreadId(),
    kind,
    ...fields,
  });
}

function readString(pointer, encoding) {
  if (pointer.isNull()) {
    return null;
  }
  try {
    return encoding === "utf16"
      ? pointer.readUtf16String()
      : pointer.readAnsiString();
  } catch (error) {
    return `<unreadable:${error.message}>`;
  }
}

function decodeArgument(argument, decoder) {
  if (decoder === "ansi") {
    return readString(argument, "ansi");
  }
  if (decoder === "utf16") {
    return readString(argument, "utf16");
  }
  if (decoder === "proc") {
    const value = argument.toUInt32();
    return value <= 0xffff ? `#${value}` : readString(argument, "ansi");
  }
  return argument.toString();
}

function readOptionalUtf16Address(address) {
  if (address === 0) {
    return null;
  }
  try {
    return ptr(address).readUtf16String();
  } catch (error) {
    return `<unreadable:${error.message}>`;
  }
}

function snapshotStartupInfoW(pointer, decodeStrings = false) {
  const size = 68;
  if (Process.pointerSize !== 4) {
    return {
      pointer: pointer.toString(),
      size,
      error: `x86 STARTUPINFOW observation requires 4-byte pointers, got ${Process.pointerSize}`,
    };
  }
  if (pointer.isNull()) {
    return { pointer: pointer.toString(), size, error: "null pointer" };
  }
  try {
    const buffer = pointer.readByteArray(size);
    const bytes = [...new Uint8Array(buffer)];
    const view = new DataView(buffer);
    const fields = {
      cb: view.getUint32(0, true),
      lpReserved: view.getUint32(4, true),
      lpDesktop: view.getUint32(8, true),
      lpTitle: view.getUint32(12, true),
      dwX: view.getUint32(16, true),
      dwY: view.getUint32(20, true),
      dwXSize: view.getUint32(24, true),
      dwYSize: view.getUint32(28, true),
      dwXCountChars: view.getUint32(32, true),
      dwYCountChars: view.getUint32(36, true),
      dwFillAttribute: view.getUint32(40, true),
      dwFlags: view.getUint32(44, true),
      wShowWindow: view.getUint16(48, true),
      cbReserved2: view.getUint16(50, true),
      lpReserved2: view.getUint32(52, true),
      hStdInput: view.getUint32(56, true),
      hStdOutput: view.getUint32(60, true),
      hStdError: view.getUint32(64, true),
    };
    return {
      pointer: pointer.toString(),
      size,
      bytes,
      fields,
      ...(decodeStrings
        ? {
            strings: {
              reserved: readOptionalUtf16Address(fields.lpReserved),
              desktop: readOptionalUtf16Address(fields.lpDesktop),
              title: readOptionalUtf16Address(fields.lpTitle),
            },
          }
        : {}),
    };
  } catch (error) {
    return { pointer: pointer.toString(), size, error: error.message };
  }
}

function describeCaller(address) {
  const module = Process.findModuleByAddress(address);
  return {
    address: address.toString(),
    module: module === null ? null : module.name,
    rva: module === null ? null : address.sub(module.base).toUInt32(),
  };
}

function beginCall(fields, returnAddress) {
  const threadId = Process.getCurrentThreadId();
  const stack = activeCallsByThread.get(threadId) ?? [];
  activeCallsByThread.set(threadId, stack);
  const frame = {
    callId: nextCallId++,
    parentCallId: stack.at(-1)?.callId ?? null,
    caller: describeCaller(returnAddress),
    startedAtMs: Date.now(),
    threadId,
  };
  stack.push(frame);
  emit("api-enter", {
    threadId,
    callId: frame.callId,
    parentCallId: frame.parentCallId,
    caller: frame.caller,
    ...fields,
  });
  return frame;
}

function finishCall(frame, fields, lastError) {
  const finishedAtMs = Date.now();
  const stack = activeCallsByThread.get(frame.threadId) ?? [];
  if (stack.at(-1) === frame) {
    stack.pop();
  } else {
    emit("call-stack-error", {
      threadId: frame.threadId,
      callId: frame.callId,
      expectedCallId: stack.at(-1)?.callId ?? null,
    });
    const index = stack.lastIndexOf(frame);
    if (index !== -1) {
      stack.splice(index, 1);
    }
  }
  if (stack.length === 0) {
    activeCallsByThread.delete(frame.threadId);
  }
  emit("api-leave", {
    threadId: frame.threadId,
    callId: frame.callId,
    parentCallId: frame.parentCallId,
    caller: frame.caller,
    lastError: Number(lastError) >>> 0,
    durationMs: Math.max(0, finishedAtMs - frame.startedAtMs),
    ...fields,
  });
}

const hooks = {
  "kernel32.dll": [
    ["LoadLibraryA", ["ansi"]],
    ["LoadLibraryW", ["utf16"]],
    ["GetProcAddress", [null, "proc"]],
    ["CreateFileA", ["ansi"]],
    ["CreateFileW", ["utf16"]],
    ["CreateThread", []],
  ],
  "user32.dll": [
    ["CreateWindowExA", [null, "ansi", "ansi"]],
    ["CreateWindowExW", [null, "utf16", "utf16"]],
  ],
  "gdi32.dll": [
    ["BitBlt", []],
    ["StretchDIBits", []],
  ],
  "gdiplus.dll": [["GdiplusStartup", []]],
  "d3d9.dll": [["Direct3DCreate9", [null]]],
  "dinput8.dll": [["DirectInput8Create", []]],
  "dsound.dll": [
    ["DirectSoundCreate", []],
    ["DirectSoundCreate8", []],
  ],
  "winmm.dll": [
    ["mciSendCommandA", [null, null, null, null]],
    ["mciSendCommandW", [null, null, null, null]],
    ["waveOutOpen", []],
  ],
  "ole32.dll": [
    ["CoInitialize", []],
    ["CoInitializeEx", []],
    ["CoCreateInstance", []],
  ],
};

if (selectedObservations.has("startup-info")) {
  hooks["kernel32.dll"].push(["GetStartupInfoW", [null]]);
}
if (selectedObservations.has("heap-set-information")) {
  hooks["kernel32.dll"].push([
    "HeapSetInformation",
    [null, null, null, null],
  ]);
}
if (selectedObservations.has("heap-lifecycle")) {
  hooks["kernel32.dll"].push(
    ["HeapCreate", [null, null, null]],
    ["HeapAlloc", [null, null, null]],
    ["HeapReAlloc", [null, null, null, null]],
    ["HeapFree", [null, null, null]],
    ["HeapSize", [null, null, null]],
    ["HeapDestroy", [null]],
  );
}

const idirect3d9Methods = [
  [0, "QueryInterface", 2],
  [1, "AddRef", 0],
  [2, "Release", 0],
  [3, "RegisterSoftwareDevice", 1],
  [4, "GetAdapterCount", 0],
  [5, "GetAdapterIdentifier", 3],
  [6, "GetAdapterModeCount", 2],
  [7, "EnumAdapterModes", 4],
  [8, "GetAdapterDisplayMode", 2],
  [9, "CheckDeviceType", 5],
  [10, "CheckDeviceFormat", 6],
  [11, "CheckDeviceMultiSampleType", 6],
  [12, "CheckDepthStencilMatch", 5],
  [13, "CheckDeviceFormatConversion", 4],
  [14, "GetDeviceCaps", 3],
  [15, "GetAdapterMonitor", 1],
  [16, "CreateDevice", 6],
];

function findExport(module, name) {
  if (typeof module.findExportByName === "function") {
    return module.findExportByName(name);
  }
  if (typeof Module.findExportByName === "function") {
    return Module.findExportByName(module.name, name);
  }
  return null;
}

function attachIdirect3d9(object) {
  if (object.isNull()) {
    return;
  }
  try {
    const vtable = object.readPointer();
    const vtableKey = vtable.toString();
    if (observedVtables.has(vtableKey)) {
      return;
    }
    observedVtables.add(vtableKey);
    const slots = [];
    for (const [slot, name, argumentCount] of idirect3d9Methods) {
      const address = vtable.add(slot * Process.pointerSize).readPointer();
      slots.push({ slot, name, address: address.toString() });
      const key = `IDirect3D9:${slot}:${address}`;
      if (attachedVtableMethods.has(key)) {
        continue;
      }
      attachedVtableMethods.add(key);
      const implementationModule = Process.findModuleByAddress(address);
      Interceptor.attach(address, {
        onEnter(args) {
          const objectAddress = args[0].toString();
          const arguments_ = Array.from({ length: argumentCount }, (_, index) =>
            args[index + 1].toString(),
          );
          this.traceObject = objectAddress;
          this.traceFrame = beginCall(
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3D9",
              vtableSlot: slot,
              object: objectAddress,
              arguments: arguments_,
            },
            this.returnAddress,
          );
        },
        onLeave(result) {
          const lastError = this.lastError;
          finishCall(
            this.traceFrame,
            {
              module: implementationModule?.name ?? "<unknown>",
              api: name,
              interface: "IDirect3D9",
              vtableSlot: slot,
              object: this.traceObject,
              result: result.toString(),
            },
            lastError,
          );
        },
      });
    }
    emit("vtable-observed", {
      interface: "IDirect3D9",
      object: object.toString(),
      vtable: vtable.toString(),
      slots,
    });
  } catch (error) {
    emit("vtable-error", {
      interface: "IDirect3D9",
      object: object.toString(),
      error: error.message,
    });
  }
}

function attachHook(module, name, decoders) {
  const address = findExport(module, name);
  if (address === null) {
    return;
  }
  const key = address.toString();
  if (attached.has(key)) {
    return;
  }
  attached.add(key);

  Interceptor.attach(address, {
    onEnter(args) {
      const arguments_ = decoders.map((decoder, index) =>
        decoder === null ? args[index].toString() : decodeArgument(args[index], decoder),
      );
      const observation =
        name === "GetStartupInfoW"
          ? { startupInfoBefore: snapshotStartupInfoW(args[0]) }
          : name.startsWith("Heap")
            ? { lastErrorBefore: Number(this.lastError) >>> 0 }
            : {};
      this.traceStartupInfoPointer =
        name === "GetStartupInfoW" ? args[0] : null;
      this.traceFrame = beginCall(
        {
          module: module.name,
          api: name,
          arguments: arguments_,
          ...observation,
        },
        this.returnAddress,
      );
    },
    onLeave(result) {
      const lastError = this.lastError;
      const observation =
        this.traceStartupInfoPointer === null
          ? {}
          : {
              startupInfoAfter: snapshotStartupInfoW(
                this.traceStartupInfoPointer,
                true,
              ),
            };
      finishCall(
        this.traceFrame,
        {
          module: module.name,
          api: name,
          result: result.toString(),
          ...observation,
        },
        lastError,
      );
      if (name === "Direct3DCreate9" && selectedVtables.has("idirect3d9")) {
        attachIdirect3d9(result);
      }
    },
  });
}

Process.attachModuleObserver({
  onAdded(module) {
    emit("module-load", {
      module: module.name,
      base: module.base.toString(),
      size: module.size,
      path: module.path,
    });

    const moduleHooks = hooks[module.name.toLowerCase()];
    if (moduleHooks === undefined) {
      return;
    }
    for (const [name, decoders] of moduleHooks) {
      try {
        attachHook(module, name, decoders);
      } catch (error) {
        emit("hook-error", { module: module.name, api: name, error: error.message });
      }
    }
  },
});

emit("agent-ready", {
  architecture: Process.arch,
  pointerSize: Process.pointerSize,
  processId: Process.id,
  selectedVtables: [...selectedVtables],
  selectedObservations: [...selectedObservations],
});
