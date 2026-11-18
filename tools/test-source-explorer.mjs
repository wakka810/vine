import assert from "node:assert/strict";
import { build } from "esbuild";

const { outputFiles } = await build({
  entryPoints: ["browser/runtime-worker/source-explorer.ts"],
  bundle: true,
  write: false,
  platform: "node",
  format: "esm",
});
const { browseSourceDirectory, inspectSourcePath, listSourceDirectory, openSourceExecutable, readSourceRange } = await import(
  `data:text/javascript;base64,${Buffer.from(outputFiles[0].contents).toString("base64")}`
);

const calls = [];
const nested = {
  name: "assets",
  kind: "directory",
  async *entries() {
    yield ["game.EXE", { kind: "file" }];
    yield ["save.dat", { kind: "file" }];
  },
  async getFileHandle(name, options) {
    calls.push(["file", name, options]);
    assert.equal(name, "game.EXE");
    return { getFile: async () => ({ name, size: 42 }) };
  },
};
const root = {
  name: "selected",
  async *entries() {
    yield ["other.txt", { kind: "file" }];
    yield ["assets", { kind: "directory" }];
    yield ["start.exe", { kind: "file" }];
    yield ["..", { kind: "directory" }];
  },
  async getDirectoryHandle(name, options) {
    calls.push(["directory", name, options]);
    assert.equal(name, "assets");
    return nested;
  },
};
assert.deepEqual(await browseSourceDirectory(root, []), [
  { name: "assets", kind: "directory" },
  { name: "other.txt", kind: "file" },
  { name: "start.exe", kind: "executable" },
]);
assert.deepEqual(await browseSourceDirectory(root, ["assets"]), [
  { name: "game.EXE", kind: "executable" },
  { name: "save.dat", kind: "file" },
]);
assert.deepEqual(await openSourceExecutable(root, ["assets"], "game.EXE"), {
  name: "game.EXE", size: 42,
});
for (const path of ["..", "../elsewhere", "a/b", "a\\b", "\0"]) {
  await assert.rejects(browseSourceDirectory(root, [path]));
  await assert.rejects(openSourceExecutable(root, [], path));
}
assert.deepEqual(calls, [
  ["directory", "assets", { create: false }],
  ["directory", "assets", { create: false }],
  ["file", "game.EXE", { create: false }],
]);
const sourceCalls = [];
let sourceBytes = Uint8Array.of(10, 20, 30, 40);
let sourceModified = 1234;
let changeDuringRead = false;
const sourceFile = {
  async getFile() {
    const bytes = sourceBytes;
    const lastModified = sourceModified;
    return {
      size: bytes.byteLength,
      lastModified,
      slice(start, end) {
        if (changeDuringRead) { sourceModified += 1; changeDuringRead = false; }
        return new Blob([bytes.slice(start, end)]);
      },
    };
  },
};
const sourceRoot = {
  async getDirectoryHandle(name, options) {
    sourceCalls.push(["directory", name, options]);
    assert.equal(name, "日本語");
    return {
      async getFileHandle(filename, fileOptions) {
        sourceCalls.push(["file", filename, fileOptions]);
        assert.equal(filename, "data.dat");
        return sourceFile;
      },
    };
  },
};
const sourcePath = ["日本語", "data.dat"];
assert.deepEqual(await readSourceRange(sourceRoot, sourcePath, 4, 1234, 1, 2), Uint8Array.of(20, 30));
assert.deepEqual(await readSourceRange(sourceRoot, sourcePath, 4, 1234, 3, 2), Uint8Array.of(40));
for (const offset of [4, 5, 0xffff_ffff]) {
  assert.deepEqual(await readSourceRange(sourceRoot, sourcePath, 4, 1234, offset, 2), new Uint8Array());
}
for (const path of [[], ["../other", "data.dat"], ["日本語", ".."], ["日本語", "a/b"]]) {
  await assert.rejects(readSourceRange(sourceRoot, path, 4, 1234, 0, 2));
}
for (const [offset, length] of [[-1, 1], [0x1_0000_0000, 1], [0, 16 * 1024 * 1024 - 27]]) {
  await assert.rejects(readSourceRange(sourceRoot, sourcePath, 4, 1234, offset, length));
}
assert.equal(sourceCalls.every((call) => call[2].create === false), true);
changeDuringRead = true;
await assert.rejects(readSourceRange(sourceRoot, sourcePath, 4, 1234, 0, 2), /changed during/);
await assert.rejects(readSourceRange(sourceRoot, sourcePath, 4, 1234, 0, 2), /changed since/);
sourceBytes = Uint8Array.of(1, 2, 3);
await assert.rejects(readSourceRange(sourceRoot, sourcePath, 4, 1235, 0, 2), /changed since/);
const leaf = {
  kind: "directory",
  async *entries() { yield ["Data.DAT", { kind: "file" }]; },
  async getFileHandle(name, options) {
    assert.equal(name, "Data.DAT"); assert.equal(options.create, false);
    return sourceFile;
  },
};
const selected = {
  async *entries() { yield ["日本語", leaf]; },
  async getDirectoryHandle(name, options) {
    assert.equal(name, "日本語"); assert.equal(options.create, false);
    return leaf;
  },
};
await assert.rejects(inspectSourcePath(selected, [0x81]), /encoded|decode|invalid/iu);
await assert.rejects(inspectSourcePath(selected, Array.from("..\\Data.DAT", (c) => c.charCodeAt(0))));
const verified = await inspectSourcePath(selected, [0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea,
  0x5c, ...Array.from("data.dat", (c) => c.charCodeAt(0))]);
assert.deepEqual(verified, {
  kind: "file", pathSegments: ["日本語", "Data.DAT"], byteLength: 3, lastModified: sourceModified,
});
const largeAsset = {
  size: 3 * 1024 * 1024 * 1024, lastModified: 42,
  slice() { throw new Error("Asset inspection must not read bytes"); },
  arrayBuffer() { throw new Error("Asset inspection must not read bytes"); },
};
let inspectedFiles = [largeAsset, largeAsset];
const largeRoot = {
  async *entries() { yield ["archive.dat", { kind: "file" }]; },
  async getFileHandle(name, options) {
    assert.equal(name, "archive.dat"); assert.equal(options.create, false);
    return { getFile: async () => inspectedFiles.shift() };
  },
};
const largePath = Array.from("archive.dat", character => character.charCodeAt(0));
assert.deepEqual(await inspectSourcePath(largeRoot, largePath), {
  kind: "file", pathSegments: ["archive.dat"], byteLength: largeAsset.size, lastModified: 42,
});
for (const changed of [
  { ...largeAsset, size: largeAsset.size - 1 },
  { ...largeAsset, lastModified: 43 },
  { ...largeAsset, sourceVersion: "new-snapshot" },
]) {
  inspectedFiles = [largeAsset, changed];
  await assert.rejects(inspectSourcePath(largeRoot, largePath), /changed during inspection/);
}
for (const invalid of [
  { ...largeAsset, size: -1 },
  { ...largeAsset, size: 0x1_0000_0000 },
  { ...largeAsset, lastModified: NaN },
]) {
  inspectedFiles = [invalid];
  await assert.rejects(inspectSourcePath(largeRoot, largePath), /Invalid source file metadata/);
}
assert.deepEqual(await inspectSourcePath(selected, [0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea]), {kind: "directory"});
assert.deepEqual(await listSourceDirectory(selected, [0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea]),
  {pathSegments: ["日本語"], entries: [{name: "Data.DAT", kind: "file"}]});
assert.equal(await listSourceDirectory(selected, Array.from("missing", (c) => c.charCodeAt(0))), null);
await assert.rejects(listSourceDirectory(selected, [0x81]), /encoded|decode|invalid/iu);
await assert.rejects(listSourceDirectory(selected, Array.from("..", c => c.charCodeAt(0))),
  /escapes/u);
const collisionRoot = {
  async *entries() {
    yield ["Data", { kind: "directory" }];
    yield ["data", { kind: "directory" }];
  },
};
await assert.rejects(listSourceDirectory(collisionRoot, Array.from("data", c => c.charCodeAt(0))),
  /Ambiguous/u);
const crowdedRoot = {
  async *entries() {
    for (let index = 0; index <= 4096; index += 1) yield [`file-${index}.dat`, { kind: "file" }];
  },
};
await assert.rejects(listSourceDirectory(crowdedRoot, []), /bounded enumeration/u);
assert.equal(await inspectSourcePath(selected, Array.from("missing", (c) => c.charCodeAt(0))), null);
console.log("source explorer: read-only traversal, bounded verified reads, and executable selection passed");

const chunkLimit = 2 * 1024 * 1024;
const multiChunkSize = 7 * chunkLimit + 17;
let chunkFailure = false;
let chunkVersion = 1;
let changeLastChunk = false;
const chunks = [];
const chunkRoot = {
  async getFileHandle(name, options) {
    assert.equal(options.create, false);
    return { async getFile() {
      return { size: multiChunkSize, lastModified: chunkVersion, slice(start, end) {
        assert(end - start <= chunkLimit);
        chunks.push([start, end]);
        return { async arrayBuffer() {
          if (chunkFailure && start >= chunkLimit) throw new Error("read failed");
          if (changeLastChunk && end === multiChunkSize) chunkVersion++;
          return new Uint8Array(end - start).fill(Math.floor(start / chunkLimit) + 1).buffer;
        }};
      }};
    }};
  },
};
const whole = await readSourceRange(chunkRoot, ["archive.dat"], multiChunkSize, 1, 0, multiChunkSize);
assert.equal(whole.length, multiChunkSize);
for (let index = 0; index < 8; index++) assert.equal(whole[index * chunkLimit], index + 1);
assert.equal(chunks.length, 8);
const readLimit = 16 * 1024 * 1024 - 28;
const tail = await readSourceRange(chunkRoot, ["archive.dat"], multiChunkSize, 1, 7 * chunkLimit, readLimit);
assert.deepEqual(tail, new Uint8Array(17).fill(8));
await assert.rejects(readSourceRange(chunkRoot, ["archive.dat"], multiChunkSize, 1, 0, readLimit + 1), /Invalid source read range/);
chunkFailure = true;
await assert.rejects(readSourceRange(chunkRoot, ["archive.dat"], multiChunkSize, 1, 0, multiChunkSize), /read failed/);
chunkFailure = false;
changeLastChunk = true;
await assert.rejects(readSourceRange(chunkRoot, ["archive.dat"], multiChunkSize, 1, 0, multiChunkSize), /changed during/);
console.log("source explorer: multi-chunk reads, short EOF and final-chunk failure checks passed");
