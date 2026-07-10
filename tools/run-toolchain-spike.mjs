import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = path.join(root, ".work", "toolchain-spike");

const run = (command, args) => {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });

  if (result.error) {
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

fs.rmSync(outputDirectory, { recursive: true, force: true });
fs.mkdirSync(outputDirectory, { recursive: true });

const tools = {
  clang: versionLine(run("clang", ["--version"])),
  lldLink: versionLine(run("lld-link", ["--version"])),
  llvmReadobj: versionLine(run("llvm-readobj", ["--version"])),
  node: process.version,
};

const wasmSource = path.join(root, "spikes", "toolchain", "wasm-smoke.c");
const wasmModule = path.join(outputDirectory, "wasm-smoke.wasm");
run("clang", [
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

const exeInspection = run("llvm-readobj", [
  "--file-headers",
  "--coff-imports",
  exeFile,
]);
const dllInspection = run("llvm-readobj", ["--coff-exports", dllFile]);

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

const report = {
  schemaVersion: 1,
  tools,
  checks: {
    wasmAddResult: wasmValue,
    peFormat: "PE32/i386",
    peImports: ["fixture_add", "fixture_call_callback"],
    dllExports: ["fixture_add", "fixture_call_callback"],
  },
  artifacts: [wasmModule, exeFile, dllFile, importLibrary].map(artifact),
};

const reportFile = path.join(outputDirectory, "report.json");
fs.writeFileSync(reportFile, `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify(report, null, 2));
