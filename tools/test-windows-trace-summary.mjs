import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { summarizeTrace } from "./summarize-windows-trace.mjs";

const schema = "vine.win32-trace.v1";
const events = [
  {
    schema,
    kind: "trace-start",
    timestamp: "2026-07-10T00:00:00.000Z",
    executable: "C:\\fixture\\abi-smoke.exe",
    executableBytes: 2560,
    executableSha256: "a".repeat(64),
    python: "3.14.6",
    frida: "17.15.4",
    platform: "Windows-11-test",
    arguments: { seconds: 1.25, killAfter: true },
  },
  {
    schema,
    sequence: 0,
    kind: "module-load",
    module: "abi-smoke.exe",
    path: "C:\\fixture\\abi-smoke.exe",
  },
  {
    schema,
    sequence: 1,
    kind: "module-load",
    module: "KERNEL32.DLL",
    path: "C:\\Windows\\System32\\KERNEL32.DLL",
  },
  {
    schema,
    sequence: 2,
    kind: "agent-ready",
    architecture: "ia32",
    pointerSize: 4,
    processId: 123,
  },
  {
    schema,
    sequence: 3,
    threadId: 10,
    kind: "api-enter",
    module: "KERNEL32.DLL",
    api: "CreateFileA",
    arguments: ["C:\\fixture\\data.arc"],
  },
  {
    schema,
    sequence: 4,
    threadId: 10,
    kind: "api-enter",
    module: "KERNEL32.DLL",
    api: "CreateFileW",
    arguments: ["C:\\fixture\\data.arc"],
  },
  {
    schema,
    sequence: 5,
    threadId: 10,
    kind: "api-leave",
    module: "KERNEL32.DLL",
    api: "CreateFileW",
    result: "0x44",
  },
  {
    schema,
    sequence: 6,
    threadId: 10,
    kind: "api-leave",
    module: "KERNEL32.DLL",
    api: "CreateFileA",
    result: "0x44",
  },
  {
    schema,
    sequence: 7,
    threadId: 10,
    kind: "api-enter",
    module: "KERNEL32.DLL",
    api: "CreateFileW",
    arguments: ["C:\\fixture\\Graphic\\missing.bin"],
  },
  {
    schema,
    sequence: 8,
    threadId: 10,
    kind: "api-leave",
    module: "KERNEL32.DLL",
    api: "CreateFileW",
    result: "0xffffffff",
  },
  {
    schema,
    sequence: 9,
    threadId: 10,
    kind: "api-enter",
    module: "USER32.dll",
    api: "CreateWindowExA",
    arguments: ["0x0", "FixtureWindow", "Fixture"],
  },
  {
    schema,
    sequence: 10,
    threadId: 10,
    kind: "api-leave",
    module: "USER32.dll",
    api: "CreateWindowExA",
    result: "0x55",
  },
  {
    schema,
    sequence: 11,
    threadId: 10,
    kind: "api-enter",
    module: "d3d9.dll",
    api: "Direct3DCreate9",
    arguments: ["0x20"],
  },
  {
    schema,
    sequence: 12,
    threadId: 10,
    kind: "api-leave",
    module: "d3d9.dll",
    api: "Direct3DCreate9",
    result: "0x66",
  },
  {
    schema,
    sequence: 13,
    threadId: 10,
    kind: "api-enter",
    module: "DINPUT8.dll",
    api: "DirectInput8Create",
    arguments: [],
  },
  {
    schema,
    sequence: 14,
    threadId: 10,
    kind: "api-leave",
    module: "DINPUT8.dll",
    api: "DirectInput8Create",
    result: "0x0",
  },
  {
    schema,
    sequence: 15,
    threadId: 10,
    kind: "api-enter",
    module: "DSOUND.dll",
    api: "DirectSoundCreate8",
    arguments: [],
  },
  {
    schema,
    sequence: 16,
    threadId: 10,
    kind: "api-leave",
    module: "DSOUND.dll",
    api: "DirectSoundCreate8",
    result: "0x0",
  },
  {
    schema,
    sequence: 17,
    threadId: 10,
    kind: "api-enter",
    module: "ole32.dll",
    api: "CoInitializeEx",
    arguments: [],
  },
  {
    schema,
    sequence: 18,
    threadId: 10,
    kind: "api-leave",
    module: "ole32.dll",
    api: "CoInitializeEx",
    result: "0x0",
  },
  {
    schema,
    kind: "trace-stop",
    timestamp: "2026-07-10T00:00:01.250Z",
    reason: "timeout-kill",
    detachedReason: null,
    crash: null,
  },
];

const encode = (items) =>
  Buffer.from(`${items.map((event) => JSON.stringify(event)).join("\n")}\n`);
const buffer = encode(events);
const summary = summarizeTrace(buffer, "fixture.jsonl");

assert.equal(summary.schema, "vine.win32-trace-summary.v1");
assert.equal(summary.source.lines, events.length);
assert.equal(summary.trace.durationMs, 1250);
assert.equal(summary.trace.architecture, "ia32");
assert.equal(summary.trace.pointerSize, 4);
assert.equal(summary.trace.executableBytes, 2560);
assert.equal(summary.trace.executableSha256, "a".repeat(64));
assert.equal(summary.trace.stopReason, "timeout-kill");
assert.deepEqual(summary.trace.tools, {
  python: "3.14.6",
  frida: "17.15.4",
  platform: "Windows-11-test",
});
assert.deepEqual(summary.trace.captureArguments, {
  seconds: 1.25,
  killAfter: true,
});
assert.deepEqual(summary.trace.sequence, {
  events: 19,
  first: 0,
  last: 18,
  contiguous: true,
});
assert.deepEqual(summary.trace.apiPairing, {
  enters: 8,
  leaves: 8,
  pairs: 8,
  maxDepth: 2,
  balanced: true,
});
assert.equal(summary.modules.length, 2);
assert.equal(summary.apis.length, 7);
assert.equal(summary.categories.file.calls, 3);
assert.equal(summary.categories.window.first.api, "CreateWindowExA");
assert.equal(summary.categories.graphics.first.api, "Direct3DCreate9");
assert.equal(summary.categories.input.calls, 1);
assert.equal(summary.categories.audio.calls, 1);
assert.equal(summary.categories.com.calls, 1);
assert.deepEqual(summary.files.topPaths, [
  { path: "C:\\fixture\\Graphic\\missing.bin", calls: 1 },
  { path: "C:\\fixture\\data.arc", calls: 1 },
]);
assert.equal(summary.files.logicalCalls, 2);
assert.deepEqual(summary.files.titleRelative.successfulPaths, ["data.arc"]);
assert.equal(summary.files.titleRelative.failedCalls, 1);
assert.deepEqual(summary.files.titleRelative.failedSearchRoots, [
  { root: "Graphic", calls: 1, uniquePaths: 1 },
]);
assert.deepEqual(summary.diagnostics, {
  "hook-error": [],
  "agent-message": [],
  "cleanup-error": [],
});

const duplicateSequence = events.map((event) => ({ ...event }));
duplicateSequence[5].sequence = 3;
assert.throws(
  () => summarizeTrace(encode(duplicateSequence)),
  /trace sequence 3 follows 3/,
);

const missingSequence = events.filter((event) => event.sequence !== 1);
assert.throws(
  () => summarizeTrace(encode(missingSequence)),
  /trace sequence 2 is not contiguous after 0/,
);

const stopBeforeTail = [
  ...events.slice(0, -1),
  events.at(-1),
  { schema, sequence: 19, kind: "module-load", module: "late.dll", path: "late.dll" },
];
assert.throws(
  () => summarizeTrace(encode(stopBeforeTail)),
  /trace-stop must be the last record/,
);

const unsequencedReady = events.map((event) => ({ ...event }));
delete unsequencedReady.find((event) => event.kind === "agent-ready").sequence;
assert.throws(
  () => summarizeTrace(encode(unsequencedReady)),
  /agent-ready must have an integer sequence/,
);

const negativeDuration = events.map((event) => ({ ...event }));
negativeDuration.at(-1).timestamp = "2026-07-09T23:59:59.000Z";
assert.throws(
  () => summarizeTrace(encode(negativeDuration)),
  /trace stop timestamp precedes trace start/,
);

const temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "vine-trace-"));
try {
  const input = path.join(temporaryDirectory, "trace.jsonl");
  const output = path.join(temporaryDirectory, "summary.json");
  const script = fileURLToPath(
    new URL("./summarize-windows-trace.mjs", import.meta.url),
  );
  fs.writeFileSync(input, buffer);
  const first = spawnSync(process.execPath, [script, "--input", input, "--output", output]);
  assert.equal(first.status, 0, first.stderr.toString());
  const overwrite = spawnSync(process.execPath, [
    script,
    "--input",
    input,
    "--output",
    output,
  ]);
  assert.equal(overwrite.status, 1);
  assert.match(overwrite.stderr.toString(), /summary output already exists/);
  const forced = spawnSync(process.execPath, [
    script,
    "--input",
    input,
    "--output",
    output,
    "--force",
  ]);
  assert.equal(forced.status, 0, forced.stderr.toString());
} finally {
  fs.rmSync(temporaryDirectory, { recursive: true, force: true });
}

console.log("Windows trace summary contract passed");
