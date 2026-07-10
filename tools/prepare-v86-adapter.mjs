import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const executableName = (name) =>
  process.platform === "win32" ? `${name}.exe` : name;
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workDirectory = path.join(root, ".work");
const sourceDirectory = path.join(workDirectory, "v86-adapter-source");
const toolchainDirectory = path.join(workDirectory, "toolchains");
const rustupHome = path.join(toolchainDirectory, "rustup");
const cargoHome = path.join(toolchainDirectory, "cargo");
const cargoBin = path.join(cargoHome, "bin");
const rustup = path.join(cargoBin, executableName("rustup"));
const rustc = path.join(cargoBin, executableName("rustc"));
const boundedPatchPath = path.join(
  root,
  "spikes",
  "x86-engine",
  "patches",
  "v86-bounded-execution.patch",
);
const statePatchPath = path.join(
  root,
  "spikes",
  "x86-engine",
  "patches",
  "v86-state-module.patch",
);
const stateSourcePath = path.join(
  root,
  "spikes",
  "x86-engine",
  "v86",
  "vine.rs",
);
const stateTargetPath = path.join(sourceDirectory, "src", "rust", "cpu", "vine.rs");
const adapterCheck = path.join(root, "tools", "run-v86-adapter-spike.mjs");
const wasmPath = path.join(sourceDirectory, "build", "v86-fallback.wasm");
const repository = "https://github.com/copy/v86.git";
const revision = "2f1346b0e7d88d4cbbbcc05fe15b4e369c3de23f";
const rustVersion = "1.97.0";

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? root,
    env: options.env ?? process.env,
    encoding: options.capture ? "utf8" : undefined,
    stdio: options.capture ? "pipe" : "inherit",
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0 && !options.allowFailure) {
    throw new Error(`${command} exited with status ${result.status}`);
  }
  return result;
}

function capture(command, args, options = {}) {
  const result = run(command, args, { ...options, capture: true });
  return result.stdout.trim();
}

function canApply(patch, args) {
  return (
    run("git", ["-C", sourceDirectory, "apply", ...args, "--check", patch], {
      capture: true,
      allowFailure: true,
    }).status === 0
  );
}

function ensurePatch(patch, label) {
  if (canApply(patch, [])) {
    run("git", ["-C", sourceDirectory, "apply", patch]);
    console.log(`Applied ${label}`);
  } else if (canApply(patch, ["--reverse"])) {
    console.log(`${label} is already applied`);
  } else {
    throw new Error(
      `v86 source has changes that conflict with ${label}; preserve them and inspect manually`,
    );
  }
}

async function installRustup() {
  if (fs.existsSync(rustup)) {
    return;
  }

  console.log(`Installing rustup under ${path.relative(root, toolchainDirectory)}`);
  const response = await fetch("https://sh.rustup.rs");
  assert(response.ok, `rustup installer download failed: ${response.status}`);
  const installerDirectory = path.join(workDirectory, "downloads");
  const installerPath = path.join(installerDirectory, "rustup-init.sh");
  fs.mkdirSync(installerDirectory, { recursive: true });
  fs.writeFileSync(installerPath, await response.text(), { mode: 0o700 });

  run(
    "sh",
    [
      installerPath,
      "-y",
      "--profile",
      "minimal",
      "--default-toolchain",
      rustVersion,
      "--no-modify-path",
    ],
    {
      env: {
        ...process.env,
        RUSTUP_HOME: rustupHome,
        CARGO_HOME: cargoHome,
      },
    },
  );
}

function windowsFallbackBuildArguments(environment) {
  const rustcDetails = capture(rustc, ["-vV"], { env: environment });
  const host = /^host: (.+)$/m.exec(rustcDetails)?.[1];
  assert(host, "rustc did not report a host triple");

  const sysroot = capture(rustc, ["--print", "sysroot"], {
    env: environment,
  });
  const rustLld = path.join(sysroot, "lib", "rustlib", host, "bin", "rust-lld.exe");
  assert(fs.existsSync(rustLld), `rust-lld not found: ${rustLld}`);

  const cargoFlags = [
    "--target wasm32-unknown-unknown",
    "--",
    `-C linker=${rustLld.replaceAll("\\", "/")}`,
    '-C link-args="--import-table --global-base=4096 --no-stack-first"',
    '-C link-args="build/softfloat.o"',
    '-C link-args="build/zstddeclib.o"',
    "--verbose",
  ].join(" ");

  return [`CARGO_FLAGS_SAFE=${cargoFlags}`, "build/v86-fallback.wasm"];
}

function prepareSource() {
  if (!fs.existsSync(sourceDirectory)) {
    fs.mkdirSync(sourceDirectory, { recursive: true });
    run("git", ["init", "--quiet"], { cwd: sourceDirectory });
    run("git", ["remote", "add", "origin", repository], {
      cwd: sourceDirectory,
    });
    run("git", ["fetch", "--depth", "1", "origin", revision], {
      cwd: sourceDirectory,
    });
    run("git", ["checkout", "--detach", "--quiet", "FETCH_HEAD"], {
      cwd: sourceDirectory,
    });
  }

  assert(fs.existsSync(path.join(sourceDirectory, ".git")));
  const head = capture("git", ["-C", sourceDirectory, "rev-parse", "HEAD"]);
  assert.equal(
    head,
    revision,
    `unexpected v86 revision in ${path.relative(root, sourceDirectory)}`,
  );

  ensurePatch(boundedPatchPath, "Vine bounded-execution patch");
  ensurePatch(statePatchPath, "Vine state-module patch");

  const expectedStateSource = fs.readFileSync(stateSourcePath);
  if (!fs.existsSync(stateTargetPath)) {
    fs.copyFileSync(stateSourcePath, stateTargetPath);
    console.log("Installed Vine state ABI source");
  } else {
    assert.deepEqual(
      fs.readFileSync(stateTargetPath),
      expectedStateSource,
      "v86 state ABI source differs from the tracked overlay",
    );
  }
  run("git", ["-C", sourceDirectory, "diff", "--check"]);
}

for (const command of ["git", "make", "clang"]) {
  run(command, ["--version"], { capture: true });
}
fs.mkdirSync(workDirectory, { recursive: true });
prepareSource();
await installRustup();

const rustEnvironment = {
  ...process.env,
  RUSTUP_HOME: rustupHome,
  CARGO_HOME: cargoHome,
  RUSTUP_TOOLCHAIN: rustVersion,
  PATH: `${cargoBin}${path.delimiter}${process.env.PATH ?? ""}`,
};

run(
  rustup,
  ["toolchain", "install", rustVersion, "--profile", "minimal"],
  { env: rustEnvironment },
);
run(
  rustup,
  ["target", "add", "wasm32-unknown-unknown", "--toolchain", rustVersion],
  { env: rustEnvironment },
);
const buildArguments =
  process.platform === "win32"
    ? windowsFallbackBuildArguments(rustEnvironment)
    : ["build/v86-fallback.wasm"];
run("make", buildArguments, {
  cwd: sourceDirectory,
  env: rustEnvironment,
});
run(process.execPath, [adapterCheck, wasmPath]);

const rustcVersion = capture(rustc, ["--version"], {
  env: rustEnvironment,
});
const clangVersion = capture("clang", ["--version"]).split("\n", 1)[0];
console.log(
  `v86 adapter ready: ${path.relative(root, wasmPath)}\n` +
    `${rustcVersion}; ${clangVersion}`,
);
