import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = path.join(root, ".work", "vine-v86");
const repository = "https://github.com/wakka810/vine-v86.git";
const revision = "12a9ba0a6f92485ef8dd3f115c71667d5d46d2c8";

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? root,
    env: options.env ?? process.env,
    encoding: "utf8",
    stdio: options.capture ? "pipe" : "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw Error(`${command} exited with status ${result.status}`);
  return result.stdout?.trim();
}

function git(...args) {
  return run("git", ["-C", source, ...args], { capture: true });
}

for (const command of ["git", "make", "clang", "cargo"]) {
  run(command, ["--version"], { capture: true });
}
if (!fs.existsSync(source)) {
  fs.mkdirSync(source, { recursive: true });
  run("git", ["init", "--quiet", source]);
  git("remote", "add", "origin", repository);
  git("fetch", "--depth", "1", "origin", revision);
  git("checkout", "--detach", "--quiet", revision);
}
assert.equal(git("remote", "get-url", "origin"), repository, "unexpected CPU repository");
assert.equal(git("status", "--porcelain"), "", "CPU source has local changes; preserve them before preparing");
let head = git("rev-parse", "--verify", "HEAD");
if (head !== revision) {
  git("fetch", "--depth", "1", "origin", revision);
  git("checkout", "--detach", "--quiet", revision);
  head = git("rev-parse", "HEAD");
}
assert.equal(head, revision, "CPU checkout differs from the pinned revision");

const details = run("rustc", ["-vV"], { capture: true });
const host = /^host: (.+)$/mu.exec(details)?.[1];
assert(host, "rustc did not report its host triple");
const sysroot = run("rustc", ["--print", "sysroot"], { capture: true });
const linker = path.join(sysroot, "lib", "rustlib", host, "bin",
  process.platform === "win32" ? "rust-lld.exe" : "rust-lld");
assert(fs.existsSync(linker), `rust-lld not found: ${linker}`);
const flags = [
  "--target wasm32-unknown-unknown", "--",
  `-C linker="${linker.replaceAll("\\", "/")}"`,
  '-C link-args="--import-table --global-base=4096 --no-stack-first"',
  '-C link-args="build/softfloat.o"', '-C link-args="build/zstddeclib.o"',
].join(" ");
run("make", ["--always-make", `CARGO_FLAGS_SAFE=${flags}`, "build/v86-fallback.wasm"], {
  cwd: source,
  env: { ...process.env, CARGO_TARGET_DIR: path.join(source, "build") },
});
assert(fs.existsSync(path.join(source, "build", "v86-fallback.wasm")), "CPU build produced no Wasm");
assert.equal(git("status", "--porcelain"), "", "CPU build changed source files");
console.log(`Vine-v86 ready at ${revision}: .work/vine-v86/build/v86-fallback.wasm`);
