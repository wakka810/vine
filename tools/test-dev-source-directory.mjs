import assert from "node:assert/strict";
import { build } from "esbuild";

const { outputFiles } = await build({
  entryPoints: ["browser/storage/dev-source-directory.ts"], bundle: true, write: false,
  platform: "node", format: "esm",
});
const { discoverDevSource, listDevSource, openDevSourceFile } = await import(
  `data:text/javascript;base64,${Buffer.from(outputFiles[0].contents).toString("base64")}`,
);
const source = { kind: "directory", transport: "dev-server", name: "selected", id: "a".repeat(64), token: "b".repeat(64) };
const calls = [];
let size = 5 * 1024 * 1024;
let version = "snapshot-1";
let corruptRange = false;
let disabled = false;
let missing = false;
const originalFetch = globalThis.fetch;
globalThis.fetch = async (input, options) => {
  const url = new URL(input, "http://localhost");
  calls.push({ url, options });
  assert.equal(options.cache, "no-store");
  if (url.pathname.endsWith("/config")) return disabled ? new Response(null, { status: 404 }) : Response.json(source);
  assert.equal(options.headers["x-vine-source-token"], source.token);
  if (missing) return new Response(null, { status: 404 });
  assert.deepEqual(JSON.parse(url.searchParams.get("path")), ["game.exe"]);
  if (url.pathname.endsWith("/list")) return Response.json({entries: [{ name: "game.exe", kind: "file" }]});
  if (url.pathname.endsWith("/file")) return Response.json({ size, lastModified: 100, sha256: "c".repeat(64), version });
  assert.equal(url.pathname, "/__vine_dev_source/range");
  if (url.searchParams.get("version") !== version) return new Response(null, {status:409});
  const offset = Number(url.searchParams.get("offset"));
  const length = Number(url.searchParams.get("length"));
  assert(length <= 2 * 1024 * 1024);
  assert(offset + length <= size);
  return new Response(new Uint8Array(corruptRange ? length + 1 : length).fill(offset / 1024 / 1024));
};
try {
  assert.deepEqual(await discoverDevSource(), source);
  disabled = true;
  assert.equal(await discoverDevSource(), null);
  assert.deepEqual(await listDevSource(source, ["game.exe"]), [{name:"game.exe", kind:"file"}]);
  const file = await openDevSourceFile(source, ["game.exe"]);
  assert.equal(calls.filter(({url})=>url.pathname.endsWith("/range")).length, 0, "metadata never downloads archive bytes");
  assert.equal(file.sourceVersion, version);
  const bytes = new Uint8Array(await file.arrayBuffer());
  assert.equal(bytes.length, size);
  assert.equal(bytes[0], 0); assert.equal(bytes[2 * 1024 * 1024], 2); assert.equal(bytes[4 * 1024 * 1024], 4);
  assert.equal((await file.slice(size + 50, size + 100).arrayBuffer()).byteLength, 0);
  await assert.rejects(file.slice(0, 2 * 1024 * 1024 + 1).arrayBuffer(), /range/);
  corruptRange = true;
  await assert.rejects(file.slice(0, 8).arrayBuffer(), /Incomplete/);
  corruptRange = false;
  version = "snapshot-2";
  await assert.rejects(file.slice(0, 8).arrayBuffer(), /409/);
  size = 3 * 1024 * 1024 * 1024;
  const archive = await openDevSourceFile(source, ["game.exe"]);
  assert.equal(archive.size, size);
  await assert.rejects(archive.arrayBuffer(), /limit/);
  assert.equal((await archive.slice(size - 16, size).arrayBuffer()).byteLength, 16);
  missing = true;
  await assert.rejects(archive.slice(0, 8).arrayBuffer(), error => error instanceof DOMException && error.name === "NotFoundError");
  missing = false;
  for (const path of [[".."], ["a/b"], ["a\\b"], ["\0"], ["x".repeat(256)]]) {
    await assert.rejects(openDevSourceFile(source, path), /path/);
  }
  await assert.rejects(openDevSourceFile({...source, token:"bad"}, ["game.exe"]), /capability/);
} finally { globalThis.fetch = originalFetch; }
console.log("development source client: cloneable capability, bounded ranges, immutable versions, large metadata-only archives passed");

// The same Windows-name resolver and source-version checks serve both transports.
const explorerBuild = await build({ entryPoints: ["browser/runtime-worker/source-explorer.ts"],
  bundle: true, write: false, platform: "node", format: "esm" });
const { inspectSourcePath, listSourceDirectory, readSourceRange } = await import(
  `data:text/javascript;base64,${Buffer.from(explorerBuild.outputFiles[0].contents).toString("base64")}`,
);
let currentVersion = "d".repeat(64);
let changeAfterRead = false;
let rangeCalls = 0;
globalThis.fetch = async (input) => {
  const url = new URL(input, "http://localhost");
  const parts = JSON.parse(url.searchParams.get("path"));
  if (url.pathname.endsWith("/list")) return Response.json({ entries: parts.length === 0
    ? [{name: "日本語", kind: "directory"}]
    : [{name:"Data.DAT", kind:"file"}] });
  assert.deepEqual(parts, ["日本語", "Data.DAT"]);
  if (url.pathname.endsWith("/file")) return Response.json({ size: 3 * 1024 * 1024 * 1024,
    lastModified: 123, sha256: "f".repeat(64), version: currentVersion });
  rangeCalls += 1;
  if (changeAfterRead) currentVersion = "e".repeat(64);
  return new Response(new Uint8Array(Number(url.searchParams.get("length"))).fill(7));
};
try {
  const japanese = [0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea];
  const verified = await inspectSourcePath(source, [...japanese, 0x5c, ...Buffer.from("data.dat")]);
  assert.equal(verified.byteLength, 3 * 1024 * 1024 * 1024);
  assert.equal(verified.sha256, "f".repeat(64));
  assert.equal(rangeCalls, 0, "large archive inspection must not transfer or locally hash bytes");
  assert.deepEqual(await listSourceDirectory(source, japanese), {
    pathSegments: ["日本語"], entries: [{ name:"Data.DAT", kind:"file" }],
  });
  const read = () => readSourceRange(source, verified.pathSegments, verified.byteLength,
    verified.lastModified, 99, 10, verified.sourceVersion);
  assert.deepEqual(await read(), new Uint8Array(10).fill(7));
  changeAfterRead = true;
  await assert.rejects(read(), /changed during/);
  await assert.rejects(read(), /changed since/);
} finally { globalThis.fetch = originalFetch; }
const storageBuild = await build({ entryPoints: ["browser/storage/source-directory.ts"],
  bundle: true, write: false, platform: "node", format: "esm" });
const { gameStorageFingerprint } = await import(
  `data:text/javascript;base64,${Buffer.from(storageBuild.outputFiles[0].contents).toString("base64")}`,
);
const fingerprint = await gameStorageFingerprint(source, [], "game.exe", "f".repeat(64));
assert.equal(await gameStorageFingerprint({...source, token: "e".repeat(64)}, [], "game.exe", "f".repeat(64)), fingerprint);
assert.notEqual(await gameStorageFingerprint({...source, id: "e".repeat(64)}, [], "game.exe", "f".repeat(64)), fingerprint);
assert.notEqual(await gameStorageFingerprint(source, ["copy"], "game.exe", "f".repeat(64)), fingerprint);
console.log("development source integration: CP932 case lookup, metadata-only 3 GiB archive, version changes and stable isolated OPFS identity passed");
