import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

// Exercise the same Rust/Wasm PE parser used by the browser selection worker.
const wasm = await readFile("target/wasm32-unknown-unknown/release/vine_runtime_wasm.wasm");
const { instance } = await WebAssembly.instantiate(wasm);
const exports = instance.exports;

function inspect(bytes, name = "game.exe") {
  const nameBytes = new TextEncoder().encode(name);
  const input = new Uint8Array(bytes.length + nameBytes.length);
  input.set(bytes);
  input.set(nameBytes, bytes.length);
  assert.equal(exports.vine_bridge_input_resize(input.length), 0);
  new Uint8Array(exports.memory.buffer, exports.vine_bridge_input_ptr(), input.length).set(input);
  const status = exports.vine_bridge_inspect_executable(bytes.length, nameBytes.length);
  const metadata = JSON.parse(new TextDecoder().decode(new Uint8Array(
    exports.memory.buffer,
    exports.vine_bridge_metadata_ptr(),
    exports.vine_bridge_metadata_len(),
  )));
  return { status, metadata };
}

function prepare(bytes, name = "game.exe") {
  const nameBytes = new TextEncoder().encode(name);
  const input = new Uint8Array(bytes.length + nameBytes.length);
  input.set(bytes);
  input.set(nameBytes, bytes.length);
  assert.equal(exports.vine_bridge_input_resize(input.length), 0);
  new Uint8Array(exports.memory.buffer, exports.vine_bridge_input_ptr(), input.length).set(input);
  const status = exports.vine_bridge_prepare_executable(bytes.length, nameBytes.length, 0xec019d3b);
  const metadata = JSON.parse(new TextDecoder().decode(new Uint8Array(
    exports.memory.buffer, exports.vine_bridge_metadata_ptr(), exports.vine_bridge_metadata_len(),
  )));
  return { status, metadata };
}

const valid = inspect(await readFile(".work/toolchain-spike/browser-vertical.exe"));
assert.equal(valid.status, 0);
assert.match(valid.metadata.sha256, /^[0-9a-f]{64}$/u);
assert(valid.metadata.entryPoint > 0);
assert(valid.metadata.imageBase > 0);
const japanese = inspect(await readFile(".work/toolchain-spike/browser-vertical.exe"), "data/日本語.exe");
assert.equal(japanese.status, 0);
for (const name of ["../escape.exe", "data/../escape.exe", "/absolute.exe", "🦀.exe", "a:stream.exe"]) {
  assert.notEqual(inspect(await readFile(".work/toolchain-spike/browser-vertical.exe"), name).status, 0);
}
const invalid = inspect(new TextEncoder().encode("not a PE32 executable"));
assert.notEqual(invalid.status, 0);
assert.equal(typeof invalid.metadata.message, "string");
// A forged length must not read beyond the JS-provided Wasm input.
assert.notEqual(exports.vine_bridge_inspect_executable(256, 0), 0);
assert.notEqual(exports.vine_bridge_inspect_executable(0xffff_ffff, 2), 0);
assert.notEqual(prepare(new TextEncoder().encode("not PE32")).status, 0);
const dll = prepare(await readFile(".work/toolchain-spike/vine_fixture.dll"));
assert.notEqual(dll.status, 0);
assert.match(dll.metadata.message, /DLL/u);
const invalidPath = prepare(await readFile(".work/toolchain-spike/bootstrap-fs.exe"), "../game.exe");
assert.notEqual(invalidPath.status, 0);
assert.match(invalidPath.metadata.message, /relative path/u);
const guestDll = prepare(await readFile(".work/toolchain-spike/browser-vertical.exe"));
assert.notEqual(guestDll.status, 0);
assert.match(guestDll.metadata.message, /vine_browser_fixture\.dll/u);
const withImports = prepare(await readFile(".work/toolchain-spike/fls-get-value.exe"));
assert.equal(withImports.status, 0, withImports.metadata.message);
assert(withImports.metadata.entryPoint > 0);
const standalone = prepare(await readFile(".work/toolchain-spike/bootstrap-fs.exe"), "data/日本語.exe");
assert.equal(standalone.status, 0, standalone.metadata.message);
assert.equal(standalone.metadata.entryPoint, 0x401000);
assert(standalone.metadata.fsBase > 0);
assert(standalone.metadata.initialStackPointer > 0);
assert(standalone.metadata.regions.length > 0);
assert(exports.vine_bridge_control_len() > 0);
assert(exports.vine_bridge_payload_len() > 0);
console.log("Rust/Wasm PE32 inspection, import mapping, and executable preparation passed");
