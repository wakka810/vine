import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = path.join(root, ".work", "toolchain-spike");
const wasmClang = process.env.VINE_WASM_CLANG || "clang";

const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    timeout: options.timeoutMs,
    windowsHide: options.windowsHide ?? false,
  });

  if (result.error) {
    if (result.error.code === "ETIMEDOUT") {
      throw new Error(
        `${command} ${args.join(" ")} timed out after ${options.timeoutMs} ms`,
      );
    }
    throw new Error(`${command} could not start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      [
        `${command} ${args.join(" ")} failed with exit code ${result.status}`,
        result.stdout.trim(),
        result.stderr.trim(),
      ]
        .filter(Boolean)
        .join("\n"),
    );
  }

  return `${result.stdout}${result.stderr}`.trim();
};

const versionLine = (value) =>
  value.split(/\r?\n/).find((line) => /version/i.test(line))?.trim() ??
  value.split(/\r?\n/, 1)[0];
const sha256 = (file) =>
  crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
const artifact = (file) => ({
  path: path.relative(root, file),
  bytes: fs.statSync(file).size,
  sha256: sha256(file),
});
const parseCoffImports = (inspection) => {
  const imports = [];
  let current = null;
  for (const rawLine of inspection.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line === "Import {") {
      if (current !== null) {
        throw new Error("nested COFF import block");
      }
      current = { module: null, symbols: [] };
    } else if (current !== null && line.startsWith("Name: ")) {
      current.module = line.slice("Name: ".length);
    } else if (current !== null && line.startsWith("Symbol: ")) {
      current.symbols.push(
        line.slice("Symbol: ".length).replace(/ \(\d+\)$/, ""),
      );
    } else if (current !== null && line === "}") {
      if (current.module === null) {
        throw new Error("COFF import block has no module name");
      }
      imports.push(current);
      current = null;
    }
  }
  if (current !== null) {
    throw new Error("unterminated COFF import block");
  }
  return imports;
};

fs.rmSync(outputDirectory, { recursive: true, force: true });
fs.mkdirSync(outputDirectory, { recursive: true });

const tools = {
  clang: versionLine(run("clang", ["--version"])),
  wasmClang: versionLine(run(wasmClang, ["--version"])),
  lldLink: versionLine(run("lld-link", ["--version"])),
  llvmReadobj: versionLine(run("llvm-readobj", ["--version"])),
  node: process.version,
};

const wasmSource = path.join(root, "spikes", "toolchain", "wasm-smoke.c");
const wasmModule = path.join(outputDirectory, "wasm-smoke.wasm");
run(wasmClang, [
  "--target=wasm32-unknown-unknown",
  "-O2",
  "-Wall",
  "-Wextra",
  "-Werror",
  "-nostdlib",
  "-Wl,--no-entry",
  "-Wl,--export=vine_add",
  "-Wl,--export-memory",
  wasmSource,
  "-o",
  wasmModule,
]);

const wasmResult = await WebAssembly.instantiate(fs.readFileSync(wasmModule));
const wasmValue = wasmResult.instance.exports.vine_add(19, 23);
if (wasmValue !== 42) {
  throw new Error(`Wasm smoke returned ${wasmValue}, expected 42`);
}

const fixtureDirectory = path.join(root, "fixtures", "pe32", "abi-smoke");
const dllObject = path.join(outputDirectory, "fixture-dll.obj");
const exeObject = path.join(outputDirectory, "fixture-exe.obj");
const dllFile = path.join(outputDirectory, "vine_fixture.dll");
const importLibrary = path.join(outputDirectory, "vine_fixture.lib");
const exeFile = path.join(outputDirectory, "abi-smoke.exe");
const browserFixtureDirectory = path.join(
  root,
  "fixtures",
  "pe32",
  "browser-vertical",
);
const browserDllObject = path.join(outputDirectory, "browser-fixture-dll.obj");
const browserExeObject = path.join(outputDirectory, "browser-fixture-exe.obj");
const browserDllFile = path.join(outputDirectory, "vine_browser_fixture.dll");
const browserImportLibrary = path.join(outputDirectory, "vine_browser_fixture.lib");
const browserExeFile = path.join(outputDirectory, "browser-vertical.exe");
const lifecycleFixtureDirectory = path.join(
  root,
  "fixtures",
  "pe32",
  "dll-lifecycle",
);
const lifecycleCodecObject = path.join(outputDirectory, "lifecycle-codec.obj");
const lifecycleCodecDll = path.join(outputDirectory, "lifecycle_codec.dll");
const lifecycleCodecLibrary = path.join(outputDirectory, "lifecycle_codec.lib");
const lifecycleEngineObject = path.join(outputDirectory, "lifecycle-engine.obj");
const lifecycleEngineDll = path.join(outputDirectory, "lifecycle_engine.dll");
const lifecycleEngineLibrary = path.join(outputDirectory, "lifecycle_engine.lib");
const lifecycleMainObject = path.join(outputDirectory, "lifecycle-main.obj");
const lifecycleExe = path.join(outputDirectory, "dll-lifecycle.exe");
const lifecycleKernel32Definition = path.join(
  lifecycleFixtureDirectory,
  "kernel32.def",
);
const lifecycleKernel32Library = path.join(
  outputDirectory,
  "lifecycle-kernel32.lib",
);
const windowFixtureDirectory = path.join(
  root,
  "fixtures",
  "pe32",
  "window-pixel",
);
const windowObject = path.join(outputDirectory, "fixture-window.obj");
const windowExe = path.join(outputDirectory, "window-pixel.exe");
const memoryFixtureDirectory = path.join(
  root,
  "fixtures",
  "pe32",
  "high-memory",
);
const memoryObject = path.join(outputDirectory, "fixture-memory.obj");
const memoryExe = path.join(outputDirectory, "high-memory.exe");
const bootstrapFixtureDirectory = path.join(
  root,
  "fixtures",
  "pe32",
  "bootstrap-fs",
);
const bootstrapObject = path.join(outputDirectory, "fixture-bootstrap.obj");
const bootstrapExe = path.join(outputDirectory, "bootstrap-fs.exe");
const bootstrapParamsFixtureDirectory = path.join(
  root,
  "fixtures",
  "pe32",
  "bootstrap-params",
);
const bootstrapParamsObject = path.join(
  outputDirectory,
  "fixture-bootstrap-params.obj",
);
const bootstrapParamsExe = path.join(outputDirectory, "bootstrap-params.exe");
const tlsGetValueFixtureDirectory = path.join(
  root,
  "fixtures",
  "pe32",
  "tls-get-value",
);
const tlsGetValueObject = path.join(outputDirectory, "fixture-tls-get-value.obj");
const tlsGetValueExe = path.join(outputDirectory, "tls-get-value.exe");
const tlsGetValueKernel32Library = path.join(
  outputDirectory,
  "tls-get-value-kernel32.lib",
);
const flsGetValueFixtureDirectory = path.join(
  root,
  "fixtures",
  "pe32",
  "fls-get-value",
);
const flsGetValueObject = path.join(outputDirectory, "fixture-fls-get-value.obj");
const flsGetValueExe = path.join(outputDirectory, "fls-get-value.exe");
const flsGetValueKernel32Library = path.join(
  outputDirectory,
  "fls-get-value-kernel32.lib",
);
const windowImportLibraries = ["kernel32", "user32", "gdi32"].map(
  (name) => ({
    definition: path.join(windowFixtureDirectory, `${name}.def`),
    library: path.join(outputDirectory, `window-${name}.lib`),
  }),
);

const compilePeObject = (source, output) => {
  run("clang", [
    "--target=i686-pc-windows-msvc",
    "-O1",
    "-Wall",
    "-Wextra",
    "-Werror",
    "-ffreestanding",
    "-fno-builtin",
    "-fno-stack-protector",
    "-c",
    source,
    "-o",
    output,
  ]);
};

compilePeObject(path.join(fixtureDirectory, "fixture-dll.c"), dllObject);
compilePeObject(path.join(fixtureDirectory, "fixture-exe.c"), exeObject);
compilePeObject(
  path.join(browserFixtureDirectory, "fixture-dll.c"),
  browserDllObject,
);
compilePeObject(
  path.join(browserFixtureDirectory, "fixture-exe.c"),
  browserExeObject,
);
compilePeObject(
  path.join(lifecycleFixtureDirectory, "codec.c"),
  lifecycleCodecObject,
);
compilePeObject(
  path.join(lifecycleFixtureDirectory, "engine.c"),
  lifecycleEngineObject,
);
compilePeObject(
  path.join(lifecycleFixtureDirectory, "main.c"),
  lifecycleMainObject,
);
compilePeObject(
  path.join(windowFixtureDirectory, "fixture-window.c"),
  windowObject,
);
compilePeObject(
  path.join(memoryFixtureDirectory, "fixture-memory.c"),
  memoryObject,
);
compilePeObject(
  path.join(bootstrapFixtureDirectory, "fixture-bootstrap.c"),
  bootstrapObject,
);
compilePeObject(
  path.join(bootstrapParamsFixtureDirectory, "fixture-bootstrap-params.c"),
  bootstrapParamsObject,
);
compilePeObject(
  path.join(tlsGetValueFixtureDirectory, "fixture-tls.c"),
  tlsGetValueObject,
);
compilePeObject(
  path.join(flsGetValueFixtureDirectory, "fixture-fls.c"),
  flsGetValueObject,
);

for (const { definition, library } of windowImportLibraries) {
  run("llvm-dlltool", [
    "-m",
    "i386",
    "-k",
    "-d",
    definition,
    "-l",
    library,
  ]);
}

run("llvm-dlltool", [
  "-m",
  "i386",
  "-k",
  "-d",
  lifecycleKernel32Definition,
  "-l",
  lifecycleKernel32Library,
]);

run("llvm-dlltool", [
  "-m",
  "i386",
  "-k",
  "-d",
  path.join(tlsGetValueFixtureDirectory, "kernel32.def"),
  "-l",
  tlsGetValueKernel32Library,
]);

run("llvm-dlltool", [
  "-m",
  "i386",
  "-k",
  "-d",
  path.join(flsGetValueFixtureDirectory, "kernel32.def"),
  "-l",
  flsGetValueKernel32Library,
]);

run("lld-link", [
  "/dll",
  "/noentry",
  "/nodefaultlib",
  "/machine:x86",
  "/timestamp:0",
  `/out:${dllFile}`,
  `/implib:${importLibrary}`,
  dllObject,
]);

run("lld-link", [
  "/nodefaultlib",
  "/machine:x86",
  "/subsystem:windows,6.0",
  "/entry:fixture_entry",
  "/dynamicbase",
  "/nxcompat",
  "/timestamp:0",
  `/out:${exeFile}`,
  exeObject,
  importLibrary,
]);

run("lld-link", [
  "/dll",
  "/nodefaultlib",
  "/machine:x86",
  "/entry:fixture_dll_entry@12",
  "/base:0x30000000",
  "/dynamicbase",
  "/nxcompat",
  "/timestamp:0",
  `/out:${lifecycleCodecDll}`,
  `/implib:${lifecycleCodecLibrary}`,
  lifecycleCodecObject,
]);

run("lld-link", [
  "/dll",
  "/nodefaultlib",
  "/machine:x86",
  "/entry:fixture_dll_entry@12",
  "/base:0x31000000",
  "/dynamicbase",
  "/nxcompat",
  "/timestamp:0",
  `/out:${lifecycleEngineDll}`,
  `/implib:${lifecycleEngineLibrary}`,
  lifecycleEngineObject,
  lifecycleCodecLibrary,
]);

run("lld-link", [
  "/nodefaultlib",
  "/machine:x86",
  "/subsystem:windows,6.0",
  "/entry:fixture_entry",
  "/base:0x32000000",
  "/dynamicbase",
  "/nxcompat",
  "/timestamp:0",
  `/out:${lifecycleExe}`,
  lifecycleMainObject,
  lifecycleEngineLibrary,
  lifecycleKernel32Library,
]);

run("lld-link", [
  "/dll",
  "/noentry",
  "/nodefaultlib",
  "/machine:x86",
  "/timestamp:0",
  `/out:${browserDllFile}`,
  `/implib:${browserImportLibrary}`,
  browserDllObject,
]);

run("lld-link", [
  "/nodefaultlib",
  "/machine:x86",
  "/subsystem:windows,6.0",
  "/entry:fixture_entry",
  "/dynamicbase",
  "/nxcompat",
  "/timestamp:0",
  `/out:${browserExeFile}`,
  browserExeObject,
  browserImportLibrary,
]);

run("lld-link", [
  "/nodefaultlib",
  "/machine:x86",
  "/subsystem:windows,6.0",
  "/entry:fixture_entry",
  "/dynamicbase",
  "/nxcompat",
  "/timestamp:0",
  `/out:${windowExe}`,
  windowObject,
  ...windowImportLibraries.map(({ library }) => library),
]);

run("lld-link", [
  "/nodefaultlib",
  "/machine:x86",
  "/subsystem:windows,6.0",
  "/entry:fixture_entry",
  "/base:0xf1000000",
  "/fixed",
  "/largeaddressaware",
  "/nxcompat",
  "/timestamp:0",
  `/out:${memoryExe}`,
  memoryObject,
]);

run("lld-link", [
  "/nodefaultlib",
  "/machine:x86",
  "/subsystem:windows,6.0",
  "/entry:fixture_entry",
  "/dynamicbase",
  "/nxcompat",
  "/timestamp:0",
  `/out:${bootstrapExe}`,
  bootstrapObject,
]);

run("lld-link", [
  "/nodefaultlib",
  "/machine:x86",
  "/subsystem:windows,6.0",
  "/entry:fixture_entry",
  "/dynamicbase",
  "/nxcompat",
  "/timestamp:0",
  `/out:${bootstrapParamsExe}`,
  bootstrapParamsObject,
]);

run("lld-link", [
  "/nodefaultlib",
  "/machine:x86",
  "/subsystem:windows,6.0",
  "/entry:fixture_entry",
  "/dynamicbase",
  "/nxcompat",
  "/timestamp:0",
  `/out:${tlsGetValueExe}`,
  tlsGetValueObject,
  tlsGetValueKernel32Library,
]);

run("lld-link", [
  "/nodefaultlib",
  "/machine:x86",
  "/subsystem:windows,6.0",
  "/entry:fixture_entry",
  "/dynamicbase",
  "/nxcompat",
  "/timestamp:0",
  `/out:${flsGetValueExe}`,
  flsGetValueObject,
  flsGetValueKernel32Library,
]);

let nativeAbiExitCode = null;
let browserVerticalNativeExitCode = null;
let dllLifecycleNativeExitCode = null;
let windowPixelNativeExitCode = null;
let bootstrapFsNativeExitCode = null;
let bootstrapParamsNativeExitCode = null;
let tlsGetValueNativeExitCode = null;
let flsGetValueNativeExitCode = null;
if (process.platform === "win32") {
  run(exeFile, [], { timeoutMs: 10_000 });
  nativeAbiExitCode = 0;
  run(browserExeFile, [], { timeoutMs: 10_000 });
  browserVerticalNativeExitCode = 0;
  run(lifecycleExe, [], { timeoutMs: 10_000 });
  dllLifecycleNativeExitCode = 0;
  run(windowExe, [], { timeoutMs: 10_000, windowsHide: false });
  windowPixelNativeExitCode = 0;
  run(bootstrapExe, [], { timeoutMs: 10_000 });
  bootstrapFsNativeExitCode = 0;
  run(bootstrapParamsExe, [], { timeoutMs: 10_000 });
  bootstrapParamsNativeExitCode = 0;
  run(tlsGetValueExe, [], { timeoutMs: 10_000 });
  tlsGetValueNativeExitCode = 0;
  run(flsGetValueExe, [], { timeoutMs: 10_000 });
  flsGetValueNativeExitCode = 0;
}

const exeInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  exeFile,
]);
const dllInspection = run("llvm-readobj", ["--coff-exports", dllFile]);
const browserExeInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  browserExeFile,
]);
const browserDllInspection = run("llvm-readobj", [
  "--coff-exports",
  browserDllFile,
]);
const lifecycleExeInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  lifecycleExe,
]);
const lifecycleEngineInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  "--coff-exports",
  lifecycleEngineDll,
]);
const lifecycleCodecInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-exports",
  lifecycleCodecDll,
]);
const windowInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  windowExe,
]);
const memoryInspection = run("llvm-readobj", [
  "--file-headers",
  "--sections",
  "--coff-exports",
  memoryExe,
]);
const bootstrapInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  "--coff-exports",
  bootstrapExe,
]);
const bootstrapParamsInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  "--coff-exports",
  bootstrapParamsExe,
]);
const tlsGetValueInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  tlsGetValueExe,
]);
const flsGetValueInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  flsGetValueExe,
]);

const requiredExeEvidence = [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
  "Name: vine_fixture.dll",
  "Symbol: fixture_add",
  "Symbol: fixture_call_callback",
];
for (const expected of requiredExeEvidence) {
  if (!exeInspection.includes(expected)) {
    throw new Error(`PE32 inspection is missing: ${expected}`);
  }
}

for (const expected of ["Name: fixture_add", "Name: fixture_call_callback"]) {
  if (!dllInspection.includes(expected)) {
    throw new Error(`DLL export inspection is missing: ${expected}`);
  }
}

for (const expected of [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
  "Name: vine_browser_fixture.dll",
  "Symbol: fixture_call_callback",
  "Symbol: fixture_commit_outputs",
  "Symbol: fixture_deferred_add",
]) {
  if (!browserExeInspection.includes(expected)) {
    throw new Error(`browser vertical PE32 inspection is missing: ${expected}`);
  }
}

for (const expected of [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
  "Name: kernel32.dll",
  "Symbol: FlsAlloc",
  "Symbol: FlsSetValue",
  "Symbol: FlsGetValue",
  "Symbol: FlsFree",
  "Symbol: CreateThread",
  "Symbol: WaitForSingleObject",
  "Symbol: GetExitCodeThread",
  "Symbol: SetLastError",
  "Symbol: GetLastError",
]) {
  if (!flsGetValueInspection.includes(expected)) {
    throw new Error(`FLS GetValue fixture inspection is missing: ${expected}`);
  }
}
for (const expected of [
  "Name: fixture_call_callback",
  "Name: fixture_commit_outputs",
  "Name: fixture_deferred_add",
]) {
  if (!browserDllInspection.includes(expected)) {
    throw new Error(`browser vertical DLL inspection is missing: ${expected}`);
  }
}

for (const expected of [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
  "ImageBase: 0x32000000",
  "Name: lifecycle_engine.dll",
  "Symbol: engine_verify_attached",
  "Name: kernel32.dll",
  "Symbol: GetCurrentProcessId",
]) {
  if (!lifecycleExeInspection.includes(expected)) {
    throw new Error(`DLL lifecycle EXE inspection is missing: ${expected}`);
  }
}
for (const expected of [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
  "AddressOfEntryPoint: 0x1020",
  "ImageBase: 0x31000000",
  "Name: lifecycle_codec.dll",
  "Symbol: codec_get_state",
  "Symbol: codec_note_engine_detached",
  "Name: engine_verify_attached",
]) {
  if (!lifecycleEngineInspection.includes(expected)) {
    throw new Error(`DLL lifecycle engine inspection is missing: ${expected}`);
  }
}
for (const expected of [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
  "AddressOfEntryPoint: 0x1030",
  "ImageBase: 0x30000000",
  "Name: codec_get_state",
  "Name: codec_note_engine_detached",
]) {
  if (!lifecycleCodecInspection.includes(expected)) {
    throw new Error(`DLL lifecycle codec inspection is missing: ${expected}`);
  }
}

for (const expected of [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
]) {
  if (!windowInspection.includes(expected)) {
    throw new Error(`window pixel inspection is missing: ${expected}`);
  }
}

for (const expected of [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
  "Name: fixture_entry",
  "Name: fixture_peb",
  "Name: fixture_teb",
  "Name: fixture_teb_peb",
  "Name: fixture_teb_self",
]) {
  if (!bootstrapInspection.includes(expected)) {
    throw new Error(`bootstrap FS inspection is missing: ${expected}`);
  }
}
if (parseCoffImports(bootstrapInspection).length !== 0) {
  throw new Error("bootstrap FS fixture must remain import-free");
}

for (const expected of [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
  "Name: fixture_command_line_buffer",
  "Name: fixture_command_line_length",
  "Name: fixture_command_line_match_index",
  "Name: fixture_command_line_maximum_length",
  "Name: fixture_entry",
  "Name: fixture_peb",
  "Name: fixture_process_parameters",
]) {
  if (!bootstrapParamsInspection.includes(expected)) {
    throw new Error(`bootstrap params inspection is missing: ${expected}`);
  }
}
if (parseCoffImports(bootstrapParamsInspection).length !== 0) {
  throw new Error("bootstrap params fixture must remain import-free");
}

for (const expected of [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
  "Name: kernel32.dll",
  "Symbol: TlsAlloc",
  "Symbol: TlsSetValue",
  "Symbol: TlsGetValue",
  "Symbol: TlsFree",
  "Symbol: CreateThread",
  "Symbol: WaitForSingleObject",
  "Symbol: GetExitCodeThread",
  "Symbol: SetLastError",
  "Symbol: GetLastError",
]) {
  if (!tlsGetValueInspection.includes(expected)) {
    throw new Error(`TLS GetValue fixture inspection is missing: ${expected}`);
  }
}

for (const expected of [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
  "ImageBase: 0xF1000000",
  "IMAGE_FILE_LARGE_ADDRESS_AWARE",
  "IMAGE_DLL_CHARACTERISTICS_NX_COMPAT",
  "Name: .rprobe",
  "Name: .nxprobe",
  "Name: fixture_entry",
  "Name: fixture_nonexec_code",
  "Name: fixture_readonly",
  "Name: fixture_writable",
]) {
  if (!memoryInspection.includes(expected)) {
    throw new Error(`high-memory inspection is missing: ${expected}`);
  }
}

const windowPixelImports = [
  { module: "kernel32.dll", symbols: ["GetModuleHandleA"] },
  {
    module: "user32.dll",
    symbols: [
      "BeginPaint",
      "CreateWindowExA",
      "DefWindowProcA",
      "DestroyWindow",
      "DispatchMessageA",
      "EndPaint",
      "GetMessageA",
      "InvalidateRect",
      "PostQuitMessage",
      "RegisterClassExA",
      "ShowWindow",
    ],
  },
  { module: "gdi32.dll", symbols: ["SetPixel"] },
];
const inspectedWindowImports = parseCoffImports(windowInspection);
if (JSON.stringify(inspectedWindowImports) !== JSON.stringify(windowPixelImports)) {
  throw new Error(
    `unexpected window pixel imports: ${JSON.stringify(inspectedWindowImports)}`,
  );
}

const report = {
  schemaVersion: 1,
  tools,
  checks: {
    wasmAddResult: wasmValue,
    peFormat: "PE32/i386",
    peImports: ["fixture_add", "fixture_call_callback"],
    dllExports: ["fixture_add", "fixture_call_callback"],
    nativeAbiExitCode,
    browserVerticalFormat: "PE32/i386",
    browserVerticalImports: [
      "fixture_call_callback",
      "fixture_commit_outputs",
      "fixture_deferred_add",
    ],
    browserVerticalNativeExitCode,
    dllLifecycleFormat: "PE32/i386",
    dllLifecycleAttachOrder: ["lifecycle_codec.dll", "lifecycle_engine.dll"],
    dllLifecycleDetachOrder: ["lifecycle_engine.dll", "lifecycle_codec.dll"],
    dllLifecycleRuntimeImports: ["kernel32.dll!GetCurrentProcessId"],
    dllLifecycleNativeExitCode,
    windowPixelFormat: "PE32/i386",
    windowPixelImports,
    windowPixelExpectedEvents: ["WM_CREATE", "WM_PAINT", "WM_DESTROY"],
    windowPixelNativeExitCode,
    highMemoryFormat: "PE32/i386",
    highMemoryImageBase: "0xf1000000",
    highMemoryNativeExecution: "not-run",
    bootstrapFsFormat: "PE32/i386",
    bootstrapFsOffsets: { tebSelf: "fs:[0x18]", peb: "fs:[0x30]" },
    bootstrapFsNativeExitCode,
    bootstrapParamsFormat: "PE32/i386",
    bootstrapParamsOffsets: {
      pebProcessParameters: "peb+0x10",
      commandLine: "process-parameters+0x40",
    },
    bootstrapParamsNativeExitCode,
    tlsGetValueFormat: "PE32/i386",
    tlsGetValueNativeExitCode,
    tlsGetValueOracle: {
      mainValue: "0x12345678",
      workerInitialValue: "0x00000000",
      workerValue: "0x23456789",
      successLastError: 0,
      invalidIndexLastError: 87,
    },
    flsGetValueFormat: "PE32/i386",
    flsGetValueNativeExitCode,
    flsGetValueOracle: {
      mainValue: "0x12345678",
      workerInitialValue: "0x00000000",
      workerInitialLastError: 87,
      workerValue: "0x23456789",
      successLastError: 0,
      invalidIndexLastError: 87,
    },
  },
  artifacts: [
    wasmModule,
    exeFile,
    dllFile,
    importLibrary,
    browserExeFile,
    browserDllFile,
    browserImportLibrary,
    lifecycleExe,
    lifecycleEngineDll,
    lifecycleEngineLibrary,
    lifecycleCodecDll,
    lifecycleCodecLibrary,
    lifecycleKernel32Library,
    windowExe,
    memoryExe,
    bootstrapExe,
    bootstrapParamsExe,
    tlsGetValueExe,
    tlsGetValueKernel32Library,
    flsGetValueExe,
    flsGetValueKernel32Library,
  ].map(artifact),
};

const reportFile = path.join(outputDirectory, "report.json");
fs.writeFileSync(reportFile, `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify(report, null, 2));
