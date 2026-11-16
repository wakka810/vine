import assert from "node:assert/strict";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const crate = path.join(root, ".work/shared-memory-authority");
const target = path.resolve(process.env.VINE_SHARED_TEST_TARGET ?? path.join(crate, "target"));
await mkdir(crate, { recursive: true });
await writeFile(path.join(crate, "Cargo.toml"), `[package]
name = "vine-shared-memory-authority-test"
version = "0.0.0"
edition = "2024"
[workspace]
[lib]
path = "lib.rs"
crate-type = ["cdylib"]
[dependencies]
vine-core = { path = ${JSON.stringify(path.join(root, "crates/vine-core").replaceAll("\\", "/"))}, features = ["shared-guest-memory"] }
[profile.release]
opt-level = 3
panic = "abort"
`);
await copyFile(path.join(root, "fixtures/pe32/shared-memory-authority.rs"), path.join(crate, "lib.rs"));
await copyFile(path.join(root, "Cargo.lock"), path.join(crate, "Cargo.lock"));
const result = spawnSync("cargo", ["build", "--release", "--target", "wasm32-unknown-unknown"],
  { cwd: crate, env: { ...process.env, CARGO_TARGET_DIR: target }, encoding: "utf8", windowsHide: true });
assert.equal(result.status, 0, result.stderr || result.error?.message);
const bytes = await readFile(path.join(target, "wasm32-unknown-unknown/release/vine_shared_memory_authority_test.wasm"));
const { instance: { exports: wasm } } = await WebAssembly.instantiate(bytes, { env: {
  test_panic(pointer, length) { throw new Error(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, pointer >>> 0, length))); },
} });
for (let repeat = 0; repeat < 2; repeat++) {
  const pointer = wasm.setup() >>> 0;
  new Uint8Array(wasm.memory.buffer, pointer, 4).set([0x42, 0x19, 0x87, 0x65]);
  assert.equal(wasm.verify(), 0);
  assert.equal(new Uint8Array(wasm.memory.buffer)[pointer], 0x7f);
  const permissions = new Uint8Array(wasm.memory.buffer, wasm.permission_pointer() >>> 0, 1 << 20);
  assert.equal(permissions[0x10], 3);
  assert.equal(permissions[0x11], 0);
  assert.equal(permissions[0xfffff], 0);
  wasm.discard();
}
console.log("Shared Wasm authority: CPU bytes, quota/range rejection, protection, release/reuse, transaction rollback and arena lifetime passed");
