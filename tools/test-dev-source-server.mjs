import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { writeFileSync } from "node:fs";
import { mkdtemp, mkdir, open, readFile, realpath, rename, rm, stat, symlink, utimes, writeFile } from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createDevSourceHandler } from "./dev-source-server.mjs";

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const prefix = "/__vine_dev_source/";
const temp = await realpath(await mkdtemp(path.join(os.tmpdir(), "vine-readonly-source-")));
const selected = path.join(temp, "selected");
const outside = path.join(temp, "outside");
await mkdir(selected);
await mkdir(outside);
const original = Buffer.from("original source bytes 日本語", "utf8");
await writeFile(path.join(selected, "game.exe"), original);
await writeFile(path.join(outside, "private.dat"), "outside source");
await mkdir(path.join(selected, "日本語"));
await writeFile(path.join(selected, "日本語", "asset.dat"), "nested source");
await writeFile(path.join(selected, "empty.dat"), "");

function request(port, pathname, { method = "GET", headers = {} } = {}) {
  return new Promise((resolve, reject) => {
    const request = http.request({ hostname: "127.0.0.1", port, path: pathname, method, headers }, (response) => {
      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.once("end", () => {
        const bytes = Buffer.concat(chunks);
        resolve({ status: response.statusCode, headers: response.headers, bytes,
          json: () => JSON.parse(bytes.toString("utf8")) });
      });
      response.once("error", reject);
    });
    request.once("error", reject);
    request.end();
  });
}

const endpoint = (operation, segments, more = {}) => `${prefix}${operation}?${new URLSearchParams({
  path: JSON.stringify(segments), ...more,
})}`;

async function serve(handler) {
  const server = http.createServer(async (request, response) => {
    if (handler && await handler(request, response)) return;
    response.writeHead(404);
    response.end("not found");
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  return { port: server.address().port,
    close: async () => { await new Promise((resolve) => server.close(resolve)); await handler?.close(); } };
}

async function productServer(arguments_, inspect) {
  const child = spawn(process.execPath, ["tools/check-browser-product.mjs", "--serve", "--port", "0", ...arguments_], {
    cwd: repository, stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  let errors = "";
  const exited = new Promise((resolve) => child.once("exit", (code, signal) => resolve({ code, signal })));
  try {
    const port = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error(`Product server did not start: ${errors}`)), 10000);
      child.stderr.on("data", (chunk) => { errors += chunk; });
      child.stdout.on("data", (chunk) => {
        output += chunk;
        const match = /Vine game selector: http:\/\/127\.0\.0\.1:([0-9]+)\//u.exec(output);
        if (match) { clearTimeout(timer); resolve(Number(match[1])); }
      });
      child.once("error", (error) => { clearTimeout(timer); reject(error); });
      child.once("exit", (code) => { clearTimeout(timer); reject(Error(`Product server exited ${code}: ${errors}`)); });
    });
    await inspect(port);
  } finally {
    child.kill("SIGTERM");
    const result = await exited;
    // Windows terminates the owned child without delivering its SIGTERM handler.
    assert(result.code === 0 || (process.platform === "win32" &&
      result.code === null && result.signal === "SIGTERM"), errors || JSON.stringify(result));
  }
}

let server;
try {
  // The normal server does not instantiate a filesystem capability on any platform.
  const normal = await serve(null);
  try { assert.equal((await request(normal.port, `${prefix}config`)).status, 404); }
  finally { await normal.close(); }
  const noServe = spawnSync(process.execPath, ["tools/check-browser-product.mjs", "--dev-game-root", selected], {
    cwd: repository, encoding: "utf8",
  });
  assert.notEqual(noServe.status, 0);
  assert.match(noServe.stderr, /requires --serve/u);
  const productIndex = path.join(repository, ".work", "browser-runtime", "index.html");
  let builtIndex;
  try { builtIndex = await readFile(productIndex, "utf8"); } catch (error) { if (error.code !== "ENOENT") throw error; }
  await productServer([], async (port) => {
    assert.equal((await request(port, `${prefix}config`)).status, 404);
    if (builtIndex !== undefined) assert.equal((await request(port, "/")).bytes.toString("utf8"), builtIndex);
  });
  if (process.platform !== "linux") {
    await assert.rejects(createDevSourceHandler(selected), /requires Linux/u);
    console.log("Development source is disabled by default and fails closed outside Linux");
  } else {
    await assert.rejects(createDevSourceHandler("relative"), /absolute/u);
    await assert.rejects(createDevSourceHandler(path.join(selected, "game.exe")), /directory/u);
    await symlink(selected, path.join(temp, "root-link"));
    await assert.rejects(createDevSourceHandler(path.join(temp, "root-link")), /without symlinks/u);
    await symlink(path.join(outside, "private.dat"), path.join(selected, "file-link"));
    await symlink(outside, path.join(selected, "directory-link"));
    await symlink(path.join(selected, "game.exe"), path.join(selected, "internal-link"));
    assert.equal(spawnSync("mkfifo", [path.join(selected, "fifo")]).status, 0);

    const handler = await createDevSourceHandler(selected);
    server = await serve(handler);
    const configResponse = await request(server.port, `${prefix}config`);
    assert.equal(configResponse.status, 200);
    const config = configResponse.json();
    assert.deepEqual(Object.keys(config).sort(), ["id", "kind", "name", "token", "transport"]);
    assert.equal(config.kind, "directory");
    assert.equal(config.transport, "dev-server");
    assert.equal(config.name, "selected");
    assert.match(config.id, /^[0-9a-f]{64}$/u);
    assert.match(config.token, /^[0-9a-f]{64}$/u);
    assert.equal(configResponse.headers["cache-control"], "no-store");
    assert.equal(configResponse.headers["cross-origin-resource-policy"], "same-origin");
    assert.equal(configResponse.headers["cross-origin-embedder-policy"], "require-corp");
    assert.equal(configResponse.headers["cross-origin-opener-policy"], "same-origin");
    assert.equal(configResponse.headers["x-content-type-options"], "nosniff");
    assert.equal(configResponse.headers["access-control-allow-origin"], undefined);
    const headers = { "x-vine-source-token": config.token, "sec-fetch-site": "same-origin" };
    const call = (operation, segments, more, options = {}) => request(server.port,
      endpoint(operation, segments, more), { ...options, headers: { ...headers, ...options.headers } });

    const listing = await call("list", []);
    assert.equal(listing.status, 200);
    assert.deepEqual(listing.json().entries.map((entry) => entry.name).sort(), ["empty.dat", "game.exe", "日本語"]);
    assert.deepEqual((await call("list", ["日本語"])).json(), { entries: [{ name: "asset.dat", kind: "file" }] });
    assert.equal((await call("file", ["日本語", "asset.dat"])).json().sha256, sha256("nested source"));
    assert.equal((await call("file", ["empty.dat"])).json().sha256, sha256(""));

    // Spy only on the test process's file-handle stream method to prove cache hits
    // avoid reading the source again; the production API has no debug endpoint.
    const probe = await open(path.join(selected, "game.exe"), "r");
    const prototype = Object.getPrototypeOf(probe);
    const createReadStream = prototype.createReadStream;
    await probe.close();
    let hashed = 0;
    prototype.createReadStream = function (...arguments_) { hashed += 1; return createReadStream.apply(this, arguments_); };
    let metadata;
    try {
      metadata = (await call("file", ["game.exe"])).json();
      assert.equal(metadata.size, original.length);
      assert.equal(metadata.sha256, sha256(original));
      assert(Number.isSafeInteger(metadata.lastModified));
      assert.match(metadata.version, /^[0-9a-f]{64}$/u);
      assert.deepEqual((await call("file", ["game.exe"])).json(), metadata);
      assert.equal(hashed, 1, "unchanged file metadata must be cached");
    } finally { prototype.createReadStream = createReadStream; }
    const range = (offset, length, version = metadata.version) => call("range", ["game.exe"], { offset, length, version });
    assert.deepEqual((await range(2, 8)).bytes, original.subarray(2, 10));
    assert.deepEqual((await range(original.length - 2, 20)).bytes, original.subarray(-2));
    assert.equal((await range(original.length, 10)).bytes.length, 0);
    assert.equal((await range(0, 0)).bytes.length, 0);
    for (const [offset, length] of [[-1, 1], [0, 2 * 1024 * 1024 + 1], ["1e2", 1],
      ["9007199254740992", 1], ["00", 1], ["0.5", 1]]) assert.equal((await range(offset, length)).status, 400);
    assert.equal((await range(0, 1, "bad")).status, 400);
    assert.equal((await range(0, 1, "0".repeat(64))).status, 409);

    const beforeWrite = await stat(path.join(selected, "game.exe"));
    const replacement = Buffer.alloc(original.length, 65);
    await writeFile(path.join(selected, "game.exe"), replacement);
    await utimes(path.join(selected, "game.exe"), beforeWrite.atime, beforeWrite.mtime);
    assert.equal((await range(0, 1)).status, 409, "restoring mtime must not hide source writes");
    const updated = (await call("file", ["game.exe"])).json();
    assert.equal(updated.sha256, sha256(replacement));
    assert.notEqual(updated.version, metadata.version);
    await writeFile(path.join(selected, "replacement.exe"), replacement);
    await utimes(path.join(selected, "replacement.exe"), beforeWrite.atime, beforeWrite.mtime);
    await rename(path.join(selected, "replacement.exe"), path.join(selected, "game.exe"));
    assert.equal((await range(0, 1, updated.version)).status, 409, "inode replacement must invalidate the snapshot");

    const big = await open(path.join(selected, "large.dat"), "w");
    const block = Buffer.alloc(1024 * 1024, 90);
    const expectedBig = createHash("sha256");
    for (let index = 0; index < 12; index += 1) { await big.write(block); expectedBig.update(block); }
    await big.close();
    const large = (await call("file", ["large.dat"])).json();
    assert.equal(large.size, 12 * 1024 * 1024);
    assert.equal(large.sha256, expectedBig.digest("hex"));
    const largeRange = await call("range", ["large.dat"], { offset: 7 * 1024 * 1024, length: 2 * 1024 * 1024, version: large.version });
    assert.equal(largeRange.status, 200);
    assert.equal(largeRange.bytes.length, 2 * 1024 * 1024);
    assert(largeRange.bytes.every((value) => value === 90));

    const changingPath = path.join(selected, "changing.dat");
    await writeFile(changingPath, block);
    prototype.createReadStream = function (...arguments_) {
      const stream = createReadStream.apply(this, arguments_);
      stream.once("data", () => writeFileSync(changingPath, Buffer.alloc(block.length, 91)));
      return stream;
    };
    try {
      assert.equal((await call("file", ["changing.dat"])).status, 409, "writes during hashing must reject the result");
    } finally { prototype.createReadStream = createReadStream; }
    const changing = (await call("file", ["changing.dat"])).json();
    assert.equal(changing.sha256, sha256(Buffer.alloc(block.length, 91)));
    const read = prototype.read;
    prototype.read = async function (...arguments_) {
      const result = await read.apply(this, arguments_);
      await writeFile(changingPath, Buffer.alloc(block.length, 92));
      return result;
    };
    try {
      assert.equal((await call("range", ["changing.dat"], {
        offset: 0, length: 16, version: changing.version,
      })).status, 409, "writes during ranged reads must reject all returned bytes");
    } finally { prototype.read = read; }

    for (const invalid of [null, "game.exe", ["..", "outside", "private.dat"], ["."], ["/etc"], ["a/b"],
      ["a\\b"], ["\0"], ["\n"], [""], ["a".repeat(256)], ["日".repeat(100)], Array(257).fill("a"), [5]]) {
      assert.equal((await call("file", invalid)).status, 400, JSON.stringify(invalid));
    }
    assert.equal((await call("file", [])).status, 400);
    assert.equal((await request(server.port, `${prefix}file?path=not-json`, { headers })).status, 400);
    assert.equal((await request(server.port, `${endpoint("file", ["game.exe"])}&path=[]`, { headers })).status, 400);
    assert.equal((await request(server.port, `${prefix}config?unexpected=1`)).status, 400);
    assert.equal((await call("file", ["missing"])).status, 404);
    for (const name of ["file-link", "internal-link", "fifo", "日本語"]) {
      assert.equal((await call("file", [name])).status, 403, name);
    }
    assert.equal((await call("list", ["directory-link"])).status, 403);
    assert.equal((await call("file", ["directory-link", "private.dat"])).status, 403);
    assert.equal((await call("list", ["game.exe"])).status, 403);
    assert.equal((await request(server.port, `${prefix}other`, { headers })).status, 404);

    for (const method of ["POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"]) {
      const response = await request(server.port, `${prefix}config`, { method });
      assert.equal(response.status, 405, method);
      assert.equal(response.headers.allow, "GET");
    }
    assert.equal((await request(server.port, endpoint("list", []))).status, 403);
    for (const token of ["wrong", "0".repeat(64), "é".repeat(64)]) {
      assert.equal((await call("list", [], {}, { headers: { "x-vine-source-token": token } })).status, 403);
    }
    for (const host of [`untrusted.example:${server.port}`, `127.0.0.1.evil:${server.port}`, "127.0.0.1", "127.0.0.1:1"]) {
      assert.equal((await request(server.port, `${prefix}config`, { headers: { host } })).status, 403);
    }
    for (const site of ["cross-site", "same-site"]) {
      assert.equal((await request(server.port, `${prefix}config`, { headers: { "sec-fetch-site": site } })).status, 403);
    }
    for (const origin of ["http://untrusted.example", "null", "http://127.0.0.1:1"]) {
      assert.equal((await request(server.port, `${prefix}config`, { headers: { origin } })).status, 403);
    }
    for (const host of [`127.0.0.1:${server.port}`, `127.0.0.2:${server.port}`, `localhost:${server.port}`]) {
      assert.equal((await request(server.port, `${prefix}config`, { headers: { host, origin: `http://${host}`, "sec-fetch-site": "same-origin" } })).status, 200);
    }
    assert.equal((await request(server.port, `${prefix}config`, { headers: {
      host: `127.0.0.2:${server.port}`, origin: `https://127.0.0.2:${server.port}`, "sec-fetch-site": "same-origin",
    } })).status, 200);
    assert.equal((await request(server.port, `${prefix}config`, { headers: {
      host: `127.0.0.1:${server.port}`, origin: `https://127.0.0.1:${server.port}`, "sec-fetch-site": "same-origin",
    } })).status, 403);

    const bounded = path.join(selected, "bounded");
    await mkdir(bounded);
    for (let start = 0; start < 4097; start += 128) {
      await Promise.all(Array.from({ length: Math.min(128, 4097 - start) }, (_, index) => mkdir(path.join(bounded, String(start + index)))));
    }
    assert.equal((await call("list", ["bounded"])).status, 413);
    await rename(selected, path.join(temp, "moved-source"));
    try {
      assert.equal((await call("list", [])).status, 409, "moving the selected root invalidates its descriptor path");
    } finally { await rename(path.join(temp, "moved-source"), selected); }
    const second = await serve(await createDevSourceHandler(selected));
    try {
      const secondConfig = (await request(second.port, `${prefix}config`)).json();
      assert.equal(secondConfig.id, config.id);
      assert.notEqual(secondConfig.token, config.token);
    } finally { await second.close(); }

    await productServer(["--dev-game-root", selected], async (port) => {
      const response = await request(port, `${prefix}config`);
      assert.equal(response.status, 200);
      assert.equal(response.json().id, config.id);
      if (builtIndex !== undefined) {
        const html = (await request(port, "/")).bytes.toString("utf8");
        assert.match(html, /<meta name="vine-dev-source" content="enabled">/u);
        assert.equal(await readFile(productIndex, "utf8"), builtIndex, "serving a dev marker must not edit browser assets");
      }
    });
    assert.equal((await readFile(path.join(outside, "private.dat"))).toString(), "outside source");
    assert.equal((await readFile(path.join(selected, "日本語", "asset.dat"))).toString(), "nested source");
    console.log("Read-only development source HTTP, bounded streaming/cache/ranges, snapshot changes, containment, origin/host/token and opt-in checks passed");
  }
} finally {
  await server?.close();
  await rm(temp, { recursive: true, force: true });
}
