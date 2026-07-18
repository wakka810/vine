import { spawn, spawnSync } from "node:child_process";
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
      const error = new Error(
        `${command} ${args.join(" ")} timed out after ${options.timeoutMs} ms`,
      );
      error.code = "ETIMEDOUT";
      throw error;
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

const runNativeFixtureOnce = (executable, { timeoutMs, windowsHide }) =>
  new Promise((resolve, reject) => {
    const child = spawn(executable, [], {
      cwd: root,
      stdio: "ignore",
      windowsHide,
    });
    let settled = false;
    const finish = (callback, value) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      callback(value);
    };
    const timer = setTimeout(() => {
      const error = new Error(`${executable} timed out after ${timeoutMs} ms`);
      error.code = "ETIMEDOUT";
      finish(reject, error);
      child.kill();
    }, timeoutMs);
    child.once("error", (error) => {
      finish(reject, new Error(`${executable} could not start: ${error.message}`));
    });
    child.once("exit", (code, signal) => {
      if (code === 0) {
        finish(resolve);
        return;
      }
      finish(
        reject,
        new Error(
          signal === null
            ? `${executable} failed with exit code ${code}`
            : `${executable} terminated by signal ${signal}`,
        ),
      );
    });
  });

const runNativeFixture = async (
  executable,
  { timeoutMs = 30_000, timeoutRetries = 0, windowsHide = true } = {},
) => {
  for (let attempt = 1; attempt <= timeoutRetries + 1; attempt += 1) {
    try {
      await runNativeFixtureOnce(executable, { timeoutMs, windowsHide });
      return attempt;
    } catch (error) {
      if (error?.code !== "ETIMEDOUT" || attempt > timeoutRetries) {
        throw error;
      }
    }
  }
  throw new Error("native fixture retry loop exhausted unexpectedly");
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

const buildDibIcon = (dimension, [red, green, blue]) => {
  const xorBytes = dimension * dimension * 4;
  const maskStride = Math.ceil(dimension / 32) * 4;
  const maskBytes = maskStride * dimension;
  const header = Buffer.alloc(40);
  header.writeUInt32LE(40, 0);
  header.writeInt32LE(dimension, 4);
  header.writeInt32LE(dimension * 2, 8);
  header.writeUInt16LE(1, 12);
  header.writeUInt16LE(32, 14);
  header.writeUInt32LE(xorBytes, 20);
  const pixels = Buffer.alloc(xorBytes);
  for (let offset = 0; offset < pixels.length; offset += 4) {
    pixels[offset] = blue;
    pixels[offset + 1] = green;
    pixels[offset + 2] = red;
    pixels[offset + 3] = 0xff;
  }
  return Buffer.concat([header, pixels, Buffer.alloc(maskBytes)]);
};

const buildIconFile = () => {
  const images = [
    { dimension: 48, bytes: buildDibIcon(48, [0x44, 0x88, 0xcc]) },
    { dimension: 32, bytes: buildDibIcon(32, [0x22, 0x66, 0xaa]) },
    { dimension: 16, bytes: buildDibIcon(16, [0x11, 0x55, 0x99]) },
  ];
  const directoryBytes = 6 + images.length * 16;
  const directory = Buffer.alloc(directoryBytes);
  directory.writeUInt16LE(1, 2);
  directory.writeUInt16LE(images.length, 4);
  let imageOffset = directoryBytes;
  images.forEach((image, index) => {
    const offset = 6 + index * 16;
    directory[offset] = image.dimension;
    directory[offset + 1] = image.dimension;
    directory.writeUInt16LE(1, offset + 4);
    directory.writeUInt16LE(32, offset + 6);
    directory.writeUInt32LE(image.bytes.length, offset + 8);
    directory.writeUInt32LE(imageOffset, offset + 12);
    imageOffset += image.bytes.length;
  });
  return Buffer.concat([directory, ...images.map(({ bytes }) => bytes)]);
};

const buildCursorFile = () => {
  const image = buildDibIcon(32, [0x33, 0x77, 0xbb]);
  const directory = Buffer.alloc(22);
  directory.writeUInt16LE(2, 2);
  directory.writeUInt16LE(1, 4);
  directory[6] = 32;
  directory[7] = 32;
  directory.writeUInt16LE(3, 10);
  directory.writeUInt16LE(4, 12);
  directory.writeUInt32LE(image.length, 14);
  directory.writeUInt32LE(directory.length, 18);
  return Buffer.concat([directory, image]);
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
const sourceReadFixtureDirectory = path.join(root, "fixtures", "pe32", "source-read");
const sourceReadObject = path.join(outputDirectory, "fixture-source-read.obj");
const sourceReadExe = path.join(outputDirectory, "source-read.exe");
const sourceReadKernel32Library = path.join(outputDirectory, "source-read-kernel32.lib");
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
const iconFixtureDirectory = path.join(
  root,
  "fixtures",
  "pe32",
  "icon-resource",
);
const iconObject = path.join(outputDirectory, "fixture-icon.obj");
const iconFile = path.join(outputDirectory, "icon-resource.ico");
const cursorFile = path.join(outputDirectory, "cursor-resource.cur");
const iconResourceScript = path.join(outputDirectory, "icon-resource.rc");
const iconResource = path.join(outputDirectory, "icon-resource.res");
const iconExe = path.join(outputDirectory, "icon-resource.exe");
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
  path.join(sourceReadFixtureDirectory, "fixture-source-read.c"),
  sourceReadObject,
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
  path.join(iconFixtureDirectory, "fixture-icon.c"),
  iconObject,
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

run("llvm-dlltool", [
  "-m",
  "i386",
  "-k",
  "-d",
  path.join(sourceReadFixtureDirectory, "kernel32.def"),
  "-l",
  sourceReadKernel32Library,
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
  `/out:${sourceReadExe}`,
  sourceReadObject,
  sourceReadKernel32Library,
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

fs.writeFileSync(iconFile, buildIconFile());
fs.writeFileSync(cursorFile, buildCursorFile());
const iconFileForResourceScript = path
  .relative(root, iconFile)
  .replaceAll("\\", "/");
const cursorFileForResourceScript = path
  .relative(root, cursorFile)
  .replaceAll("\\", "/");
fs.writeFileSync(
  iconResourceScript,
  `LANGUAGE 0x11, 0x1\n101 ICON "${iconFileForResourceScript}"\n106 CURSOR "${cursorFileForResourceScript}"\n`,
);
run("llvm-rc", ["/nologo", `/fo${iconResource}`, iconResourceScript]);
run("lld-link", [
  "/nodefaultlib",
  "/machine:x86",
  "/subsystem:windows,6.0",
  "/entry:fixture_entry",
  "/dynamicbase",
  "/nxcompat",
  "/timestamp:0",
  `/out:${iconExe}`,
  iconObject,
  iconResource,
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
let sourceReadNativeExitCode = null;
let dllLifecycleNativeExitCode = null;
let dllLifecycleNativeAttempts = null;
let windowPixelNativeExitCode = null;
let bootstrapFsNativeExitCode = null;
let bootstrapParamsNativeExitCode = null;
let tlsGetValueNativeExitCode = null;
let flsGetValueNativeExitCode = null;
if (process.platform === "win32") {
  await runNativeFixture(exeFile);
  nativeAbiExitCode = 0;
  await runNativeFixture(browserExeFile);
  browserVerticalNativeExitCode = 0;
  await runNativeFixture(sourceReadExe);
  sourceReadNativeExitCode = 0;
  // A freshly linked multi-DLL image can transiently remain in a zero-CPU host
  // launch wait while Windows inspects it. This gate checks loader semantics,
  // not launch latency, so retry one bounded timeout while still rejecting two
  // consecutive hangs.
  dllLifecycleNativeAttempts = await runNativeFixture(lifecycleExe, {
    timeoutRetries: 1,
  });
  dllLifecycleNativeExitCode = 0;
  await runNativeFixture(windowExe, { windowsHide: false });
  windowPixelNativeExitCode = 0;
  await runNativeFixture(bootstrapExe);
  bootstrapFsNativeExitCode = 0;
  await runNativeFixture(bootstrapParamsExe);
  bootstrapParamsNativeExitCode = 0;
  await runNativeFixture(tlsGetValueExe);
  tlsGetValueNativeExitCode = 0;
  await runNativeFixture(flsGetValueExe);
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
  "--coff-exports",
  browserExeFile,
]);
const browserDllInspection = run("llvm-readobj", [
  "--coff-exports",
  browserDllFile,
]);
const sourceReadInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  sourceReadExe,
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
const iconInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  "--coff-resources",
  iconExe,
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
  "Name: fixture_thread_entry",
  "Name: fixture_thread_marker",
]) {
  if (!browserExeInspection.includes(expected)) {
    throw new Error(`browser vertical PE32 inspection is missing: ${expected}`);
  }
}

for (const expected of [
  "Format: COFF-i386",
  "Machine: IMAGE_FILE_MACHINE_I386",
  "Name: kernel32.dll",
  "Symbol: CloseHandle",
  "Symbol: CreateFileA",
  "Symbol: GetFileSize",
  "Symbol: ReadFile",
  "Symbol: SetFilePointer",
]) {
  if (!sourceReadInspection.includes(expected)) {
    throw new Error(`source read PE32 inspection is missing: ${expected}`);
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
  "Total Number of Resources: 6",
  "Type: CURSOR (ID 1)",
  "Type: ICON (ID 3)",
  "Type: GROUP_CURSOR (ID 12)",
  "Type: GROUP_ICON (ID 14)",
  "Name: (ID 101)",
  "Language: (ID 1041)",
  "DataSize: 48",
]) {
  if (!iconInspection.includes(expected)) {
    throw new Error(`icon resource inspection is missing: ${expected}`);
  }
}
if (parseCoffImports(iconInspection).length !== 0) {
  throw new Error("icon resource fixture must remain import-free");
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
    sourceReadNativeExitCode,
    dllLifecycleFormat: "PE32/i386",
    dllLifecycleAttachOrder: ["lifecycle_codec.dll", "lifecycle_engine.dll"],
    dllLifecycleDetachOrder: ["lifecycle_engine.dll", "lifecycle_codec.dll"],
    dllLifecycleRuntimeImports: ["kernel32.dll!GetCurrentProcessId"],
    dllLifecycleNativeExitCode,
    dllLifecycleNativeAttempts,
    windowPixelFormat: "PE32/i386",
    windowPixelImports,
    windowPixelExpectedEvents: ["WM_CREATE", "WM_PAINT", "WM_DESTROY"],
    windowPixelNativeExitCode,
    iconResourceFormat: "PE32/i386",
    iconResourceGroupId: 101,
    iconResourceLanguageId: 1041,
    iconResourceDimensions: [48, 32, 16],
    cursorResourceGroupId: 106,
    cursorResourceLanguageId: 1041,
    cursorResourceDimensions: [32, 32],
    cursorResourceHotspot: [3, 4],
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
    sourceReadExe,
    sourceReadKernel32Library,
    lifecycleExe,
    lifecycleEngineDll,
    lifecycleEngineLibrary,
    lifecycleCodecDll,
    lifecycleCodecLibrary,
    lifecycleKernel32Library,
    windowExe,
    iconExe,
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
