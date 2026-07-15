import assert from "node:assert/strict";

import { build } from "esbuild";

const bundle = await build({
  entryPoints: ["browser/runtime-worker/browser-resource-table.ts"],
  bundle: true,
  format: "esm",
  platform: "node",
  target: "es2023",
  write: false,
});
assert.equal(bundle.outputFiles.length, 1);
const source = bundle.outputFiles[0].text;
const module = await import(`data:text/javascript;base64,${Buffer.from(source).toString("base64")}`);
const { BrowserResourceTable } = module;

const table = new BrowserResourceTable();
let rustOwnedReferenceCount = 2;
const adapter = { kind: "gpu-adapter", closed: false };
const surface = { kind: "canvas-surface", closed: false };
const unrelated = { kind: "audio-node", closed: false };
const adapterRecord = table.register(
  "graphics-adapter",
  { objectId: "41", interfaceId: "72", interfaceName: "ISyntheticGraphics" },
  adapter,
);
const surfaceRecord = table.register(
  "graphics-surface",
  { objectId: "41", interfaceId: "73", interfaceName: "ISyntheticSurface" },
  surface,
);
const unrelatedRecord = table.register(
  "audio-node",
  { objectId: "42", interfaceId: "74", interfaceName: "ISyntheticAudio" },
  unrelated,
);

assert.equal(adapterRecord.id, "browser-resource:1");
assert.equal(surfaceRecord.id, "browser-resource:2");
assert.notEqual(adapterRecord.id, adapterRecord.binding.objectId);
assert.notEqual(adapterRecord.id, adapterRecord.binding.interfaceId);
assert.equal(table.get(adapterRecord.id).resource, adapter);
assert.equal(table.size, 3);
assert.deepEqual(table.snapshot(), [
  {
    id: "browser-resource:1",
    kind: "graphics-adapter",
    binding: { objectId: "41", interfaceId: "72", interfaceName: "ISyntheticGraphics" },
  },
  {
    id: "browser-resource:2",
    kind: "graphics-surface",
    binding: { objectId: "41", interfaceId: "73", interfaceName: "ISyntheticSurface" },
  },
  {
    id: "browser-resource:3",
    kind: "audio-node",
    binding: { objectId: "42", interfaceId: "74", interfaceName: "ISyntheticAudio" },
  },
]);
assert.equal("resource" in table.snapshot()[0], false);
assert.throws(() => {
  adapterRecord.binding.objectId = "99";
}, TypeError);

const locallyReleased = table.release(adapterRecord.id);
locallyReleased.resource.closed = true;
assert.equal(rustOwnedReferenceCount, 2, "browser release must not mutate Rust COM refcount");
assert.equal(table.size, 2);
assert.throws(() => table.get(adapterRecord.id), /unknown/u);
const replacementRecord = table.register(
  "graphics-adapter",
  { objectId: "41", interfaceId: "72", interfaceName: "ISyntheticGraphics" },
  { kind: "replacement-adapter", closed: false },
);
assert.equal(replacementRecord.id, "browser-resource:4", "released browser IDs must not be reused");
assert.equal(rustOwnedReferenceCount, 2);

rustOwnedReferenceCount = 0;
const retired = table.retireLogicalComObject("41");
assert.deepEqual(retired.map((record) => record.id), [surfaceRecord.id, replacementRecord.id]);
assert.equal(table.size, 1);
assert.equal(table.get(unrelatedRecord.id).resource, unrelated);
assert.deepEqual(table.retireLogicalComObject("999"), []);
assert.throws(
  () => table.register("bad", { objectId: "0", interfaceId: "1", interfaceName: "IBad" }, {}),
  /positive decimal identity/u,
);
assert.throws(
  () =>
    table.register(
      "bad",
      {
        objectId: "18446744073709551616",
        interfaceId: "1",
        interfaceName: "IBad",
      },
      {},
    ),
  /exceeds Rust u64 identity space/u,
);

console.log("browser resource / logical COM identity separation passed");
