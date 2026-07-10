"use strict";

const schema = "vine.win32-trace.v1";
let sequence = 0;
const attached = new Set();

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

function findExport(module, name) {
  if (typeof module.findExportByName === "function") {
    return module.findExportByName(name);
  }
  if (typeof Module.findExportByName === "function") {
    return Module.findExportByName(module.name, name);
  }
  return null;
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
      this.traceArguments = decoders.map((decoder, index) =>
        decoder === null ? args[index].toString() : decodeArgument(args[index], decoder),
      );
      emit("api-enter", {
        module: module.name,
        api: name,
        arguments: this.traceArguments,
      });
    },
    onLeave(result) {
      emit("api-leave", {
        module: module.name,
        api: name,
        result: result.toString(),
      });
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
});
