import assert from "node:assert/strict";
import { build } from "esbuild";

const { outputFiles } = await build({
  entryPoints: ["browser/runtime-worker/writable-tree.ts"],
  bundle: true,
  write: false,
  platform: "node",
  format: "esm",
});
const { openWritableTree } = await import(
  `data:text/javascript;base64,${Buffer.from(outputFiles[0].contents).toString("base64")}`
);

const missing = () => new DOMException("Missing", "NotFoundError");
let failNextWrite = false;
let failNextDelete = false;
function directory() {
  const children = new Map();
  return {
    children,
    async *keys() { yield* children.keys(); },
    async getDirectoryHandle(name, { create }) {
      let child = children.get(name);
      if (child === undefined && create) children.set(name, child = directory());
      if (child?.children === undefined) throw missing();
      return child;
    },
    async getFileHandle(name, { create }) {
      let child = children.get(name);
      if (child === undefined && create) children.set(name, child = file());
      if (child?.createWritable === undefined) throw missing();
      return child;
    },
    async removeEntry(name) {
      if (failNextDelete) { failNextDelete = false; throw new DOMException("Denied", "NoModificationAllowedError"); }
      if (!children.delete(name)) throw missing();
    },
  };
}
function file() {
  let stored = new Uint8Array();
  return {
    async getFile() { return new Blob([stored]); },
    async createWritable({ keepExistingData }) {
      let pending = keepExistingData ? new Uint8Array(stored) : new Uint8Array();
      let position = 0;
      let aborted = false;
      return {
        async seek(offset) { position = offset; },
        async write(bytes) {
          if (failNextWrite) { failNextWrite = false; throw new DOMException("Full", "QuotaExceededError"); }
          const next = new Uint8Array(Math.max(pending.length, position + bytes.length));
          next.set(pending);
          next.set(bytes, position);
          pending = next;
        },
        async close() { if (aborted) throw new Error("aborted"); stored = pending; },
        async abort() { aborted = true; },
      };
    },
  };
}
const root = directory();
Object.defineProperty(globalThis, "navigator", {
  value: { storage: { async getDirectory() { return root; } } },
  configurable: true,
});
const id = "a".repeat(64);
const read = (tree, path) => tree.read(path, 0, 2);
await assert.rejects(openWritableTree("../other"));
const tree = await openWritableTree(id);
assert.equal(await read(tree, "save\\slot.dat"), null);
assert.equal(await tree.stat("save/slot.dat"), null);
assert.equal(await tree.stat("missing/slot.dat"), null);
await assert.rejects(tree.createEmpty("save/slot.dat"), { name: "NotFoundError" });
await tree.createDirectory("save");
assert.deepEqual(await tree.stat("save"), { kind: "directory" });
await assert.rejects(tree.createDirectory("source-only/nested/slot"), { name: "NotFoundError" });
await assert.rejects(tree.createDirectory("source-only/nested/slot", 3), /Invalid source-verified parent count/);
await tree.createDirectory("source-only/nested/slot", 2);
assert.deepEqual(await tree.stat("source-only/nested/slot"), { kind: "directory" });
await tree.createEmpty("source-file-only/child/empty.dat", 2);
assert.deepEqual(await tree.stat("source-file-only/child/empty.dat"), { kind: "file", byteLength: 0 });
await assert.rejects(tree.createDirectory("unrelated/nested/slot", 1), { name: "NotFoundError" });
assert.equal(await tree.stat("unrelated/nested"), null, "unverified nested parent was not created");
await assert.rejects(tree.writeExisting("save/absent.dat", Uint8Array.of(9), 0), { name: "NotFoundError" });
assert.equal(await tree.stat("save/absent.dat"), null, "WriteFile cannot create a missing open file");
await tree.createEmpty("save/empty.dat");
assert.deepEqual(await tree.stat("save/empty.dat"), { kind: "file", byteLength: 0 });
await tree.writeExisting("save/empty.dat", Uint8Array.of(6), 0);
assert.deepEqual(await tree.read("save/empty.dat", 0, 2), Uint8Array.of(6));
await tree.createEmpty("save\\slot.dat");
await tree.writeExisting("save\\slot.dat", Uint8Array.of(1, 2), 0);
assert.deepEqual(await tree.stat("save/slot.dat"), { kind: "file", byteLength: 2 });
assert.deepEqual(await read(tree, "save/slot.dat"), Uint8Array.of(1, 2));
assert.deepEqual(await tree.read("save/slot.dat", 1, 1), Uint8Array.of(2));
await tree.writeExisting("save/slot.dat", Uint8Array.of(3), 1);
assert.deepEqual(await read(tree, "save/slot.dat"), Uint8Array.of(1, 3));
await tree.writeExisting("save/slot.dat", Uint8Array.of(4), 2);
assert.deepEqual(await tree.read("save/slot.dat", 0, 4), Uint8Array.of(1, 3, 4));
await tree.writeExisting("save/slot.dat", Uint8Array.of(7), 0);
assert.deepEqual(await tree.read("save/slot.dat", 0, 4), Uint8Array.of(7, 3, 4));
await tree.createEmpty("save/empty.dat");
assert.deepEqual(await tree.read("save/empty.dat", 0, 2), new Uint8Array(), "CREATE_ALWAYS truncates");
await assert.rejects(tree.removeFile("save"));
assert.deepEqual(await tree.stat("save"), { kind: "directory" }, "DeleteFile must preserve directories");
await assert.rejects(tree.removeFile("save/missing.dat"), { name: "NotFoundError" });
await tree.createEmpty("save/to-delete.dat");
await tree.writeExisting("save/to-delete.dat", Uint8Array.of(77), 0);
failNextDelete = true;
await assert.rejects(tree.removeFile("save/to-delete.dat"), { name: "NoModificationAllowedError" });
assert.deepEqual(await tree.read("save/to-delete.dat", 0, 1), Uint8Array.of(77),
  "failed DeleteFile must preserve the original save");
await tree.removeFile("SAVE/TO-DELETE.DAT");
assert.equal(await tree.stat("save/to-delete.dat"), null, "DeleteFile must remove the original case-insensitive file");
await tree.createEmpty("save/sparse.dat");
await tree.writeExisting("save/sparse.dat", Uint8Array.of(5), 2);
assert.deepEqual(await tree.read("save/sparse.dat", 0, 4), Uint8Array.of(0, 0, 5));
for (const offset of [-1, 0.5, Number.MAX_SAFE_INTEGER]) {
  await assert.rejects(tree.writeExisting("save/slot.dat", Uint8Array.of(9), offset));
}
await assert.rejects(tree.writeExisting("save/slot.dat", new Uint8Array(2 ** 21 + 1), 0));
assert.deepEqual(await tree.read("save/slot.dat", 0, 4), Uint8Array.of(7, 3, 4));
for (const [offset, length] of [[-1, 1], [0, 2 ** 21 + 1], [Number.MAX_SAFE_INTEGER, 1]]) {
  await assert.rejects(tree.read("save/slot.dat", offset, length));
}
const restarted = await openWritableTree(id);
assert.deepEqual(await restarted.read("save/slot.dat", 0, 4), Uint8Array.of(7, 3, 4));
assert.deepEqual(await restarted.stat("save/slot.dat"), { kind: "file", byteLength: 3 });
assert.deepEqual(await restarted.stat("SAVE/SLOT.DAT"), { kind: "file", byteLength: 3 });
assert.deepEqual(await restarted.read("SAVE/SLOT.DAT", 0, 3), Uint8Array.of(7, 3, 4));
await restarted.writeExisting("SAVE/SLOT.DAT", Uint8Array.of(8), 2);
assert.deepEqual(await restarted.read("save/slot.dat", 0, 3), Uint8Array.of(7, 3, 8));
await restarted.writeExisting("save/slot.dat", Uint8Array.of(4), 2);
assert.equal(root.children.get(id).children.get("default").children.get("save").children.size, 3,
  "case variant of an existing save must not create another file");
await restarted.createDirectory("SAVE");
assert.equal(root.children.get(id).children.get("default").children.size, 4);
const saveChildren = root.children.get(id).children.get("default").children.get("save").children;
saveChildren.set("SLOT.DAT", file());
await assert.rejects(restarted.stat("save/slot.dat"), /Ambiguous case-insensitive OPFS path/);
await assert.rejects(restarted.read("save/slot.dat", 0, 3), /Ambiguous case-insensitive OPFS path/);
await assert.rejects(restarted.writeExisting("save/slot.dat", Uint8Array.of(9), 0), /Ambiguous case-insensitive OPFS path/);
await assert.rejects(restarted.remove("save/slot.dat"), /Ambiguous case-insensitive OPFS path/);
await assert.rejects(restarted.removeFile("save/slot.dat"), /Ambiguous case-insensitive OPFS path/);
saveChildren.delete("SLOT.DAT");
const gameChildren = root.children.get(id).children.get("default").children;
gameChildren.set("SAVE", directory());
await assert.rejects(restarted.stat("save/slot.dat"), /Ambiguous case-insensitive OPFS path/);
await assert.rejects(restarted.createEmpty("save/new.dat"), /Ambiguous case-insensitive OPFS path/);
gameChildren.delete("SAVE");
failNextWrite = true;
await assert.rejects(restarted.writeExisting("save/slot.dat", Uint8Array.of(9), 1), { name: "QuotaExceededError" });
assert.deepEqual(await restarted.read("save/slot.dat", 0, 4), Uint8Array.of(7, 3, 4));
failNextWrite = true;
await assert.rejects(restarted.createEmpty("SAVE/SLOT.DAT"), { name: "QuotaExceededError" });
assert.deepEqual(await restarted.read("save/slot.dat", 0, 3), Uint8Array.of(7, 3, 4),
  "failed CREATE_ALWAYS must retain the previous save");
failNextWrite = true;
await assert.rejects(restarted.createEmpty("save/new.dat"), { name: "QuotaExceededError" });
assert.equal(await read(restarted, "save/new.dat"), null, "failed create must not leave an empty save file");
assert.equal(root.children.get(id).children.get("default").children.get("save").children.has("new.dat"), false);
for (const path of ["", "/abs", "C:\\abs", "../other", "save/../other", "save//file", "save/.", "save/evil:stream", "save/evil\0file"]) {
  for (const operation of [
    () => tree.stat(path),
    () => tree.read(path, 0, 2),
    () => tree.writeExisting(path, Uint8Array.of(9), 0),
    () => tree.createEmpty(path),
    () => tree.createDirectory(path),
    () => tree.remove(path),
    () => tree.removeFile(path),
  ]) await assert.rejects(operation(), undefined, `accepted ${JSON.stringify(path)}`);
}
assert.equal(root.children.size, 1, "invalid paths must not create entries outside the game root");
await restarted.remove("SAVE/SLOT.DAT");
assert.equal(await read(tree, "save/slot.dat"), null);
console.log("writable tree: sandbox, reopen, failure preservation, and deletion passed");
