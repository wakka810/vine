import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  bgiProfileOutputDirectory,
  planBgiCheckpointCommand,
} from "./bgi-checkpoint-command.mjs";
import { loadCurrentCheckpoint } from "./current-checkpoint.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifest = loadCurrentCheckpoint(
  path.join(root, "docs", "current-checkpoint.json"),
);
const packageJson = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
assert.deepEqual(
  Object.keys(packageJson.scripts)
    .filter((name) => name.startsWith("check:runtime:bgi"))
    .sort(),
  [
    "check:runtime:bgi",
    "check:runtime:bgi-browser",
    "check:runtime:bgi-browser-benchmark",
    "check:runtime:bgi-browser-benchmark:prepared",
    "check:runtime:bgi-browser:prepared",
  ],
);
assert.equal(packageJson.scripts["check:runtime:bgi"], "node tools/run-bgi-checkpoint.mjs");

const markdownFiles = [path.join(root, "README.md")];
const collectMarkdown = (directory) => {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      collectMarkdown(absolute);
    } else if (entry.isFile() && entry.name.endsWith(".md")) {
      markdownFiles.push(absolute);
    }
  }
};
collectMarkdown(path.join(root, "docs"));
for (const file of markdownFiles) {
  const source = fs.readFileSync(file, "utf8");
  for (const match of source.matchAll(/npm run check:runtime:(bgi-[a-z0-9-]+)/gu)) {
    assert(
      new Set(["bgi-browser", "bgi-browser-benchmark"]).has(match[1]),
      `${path.relative(root, file)} references removed BGI alias ${match[1]}`,
    );
  }
}
assert(
  !fs
    .readFileSync(path.join(root, "tools", "run-browser-runtime-check.mjs"), "utf8")
    .includes('"v86-bgi-'),
  "browser BGI oracle directory must be derived from the active profile",
);

const active = planBgiCheckpointCommand([], manifest, root);
assert.deepEqual(active.selection, {
  kind: "active-profile",
  profile: manifest.activeRuntimeCheckpoint.profile,
});
assert.equal(
  active.outputDirectory,
  bgiProfileOutputDirectory(manifest.activeRuntimeCheckpoint.profile),
);
assert.deepEqual(
  active.runnerArguments.slice(0, 8),
  [
    "--checkpoint",
    manifest.historicalBaseline.checkpoint,
    "--generic-runtime-reentry",
    "--evidence-level",
    "summary",
    "--runtime-capability",
    manifest.activeRuntimeCheckpoint.profile,
    "--runtime-policy",
  ],
);
assert.equal(
  active.runnerArguments[8],
  path.join(root, manifest.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source),
);
assert.deepEqual(planBgiCheckpointCommand(["--profile", "active"], manifest, root), active);

const fullEvidence = planBgiCheckpointCommand(
  ["--profile", "active", "--evidence-level", "full"],
  manifest,
  root,
);
assert.equal(fullEvidence.runnerArguments[4], "full");

const explicitActive = planBgiCheckpointCommand(
  ["--profile", manifest.activeRuntimeCheckpoint.profile],
  manifest,
  root,
);
assert.deepEqual(explicitActive, active);

const baseline = planBgiCheckpointCommand(["--profile", "baseline"], manifest, root);
assert.equal(baseline.outputDirectory, "v86-bgi-generic-runtime-reentry");
assert.deepEqual(baseline.runnerArguments, [
  "--checkpoint",
  manifest.historicalBaseline.checkpoint,
  "--generic-runtime-reentry",
  "--evidence-level",
  "summary",
]);

const historicalProfile = planBgiCheckpointCommand(
  ["--profile", "kernel32-standard-error-file-type"],
  manifest,
  root,
);
assert.equal(historicalProfile.outputDirectory, "v86-bgi-standard-error-file-type");
assert(!historicalProfile.runnerArguments.includes("--runtime-policy"));

const historicalCheckpoint = planBgiCheckpointCommand(
  ["--checkpoint", "bgi-first-win32-call"],
  manifest,
  root,
);
assert.deepEqual(historicalCheckpoint, {
  selection: {
    kind: "historical-checkpoint",
    checkpoint: "bgi-first-win32-call",
  },
  outputDirectory: "v86-bgi-first-win32-call",
  runnerArguments: ["--checkpoint", "bgi-first-win32-call"],
});

for (const arguments_ of [
  ["--profile", "baseline", "--checkpoint", "bgi-first-trap"],
  ["--profile", "active", "--profile", "active"],
  ["--checkpoint", "../outside"],
  ["--evidence-level", "diagnostic"],
  ["--evidence-level", "full", "--evidence-level", "summary"],
  ["--unknown", "value"],
]) {
  assert.throws(() => planBgiCheckpointCommand(arguments_, manifest, root));
}
assert.equal(bgiProfileOutputDirectory("user32-message"), "v86-bgi-message");
assert.equal(bgiProfileOutputDirectory("ole32-com"), "v86-bgi-com");
assert.equal(bgiProfileOutputDirectory("winmm-audio"), "v86-bgi-audio");
assert.equal(bgiProfileOutputDirectory("gdi-window-dc-fill"), "v86-bgi-window-dc-fill");
assert.equal(
  bgiProfileOutputDirectory("d3d9-root-factory-and-identity"),
  "v86-bgi-d3d9-root-factory-and-identity",
);
assert.throws(() => bgiProfileOutputDirectory("dsound-audio"));

console.log("generic BGI checkpoint command contract passed");
