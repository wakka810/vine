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
const browserRunner = fs.readFileSync(
  path.join(root, "tools", "run-browser-runtime-check.mjs"),
  "utf8",
);
const genericRunner = fs.readFileSync(
  path.join(root, "tools", "run-runtime-v86-bgi-first-trap.mjs"),
  "utf8",
);
assert(
  browserRunner.includes(
    `const runtimeProtocol = "${manifest.browserCheckpoint.browserRuntimeProtocol}";`,
  ),
  "browser BGI runner protocol must match the current checkpoint",
);
assert.match(
  genericRunner,
  /acquireWritableProfileLease\(writableProfile, crypto\.randomUUID\(\)\)/u,
);
assert.match(
  genericRunner,
  /requestKey: `\$\{writableProfileLease\.sessionKey\}:\$\{metadata\.requestId\}`/u,
);
assert.match(genericRunner, /sessionEpoch: writableProfileLease\.epoch/u);
for (const flag of [
  "--disable-background-timer-throttling",
  "--disable-backgrounding-occluded-windows",
  "--disable-renderer-backgrounding",
]) {
  assert(browserRunner.includes(`"${flag}"`), `BGI performance runner omitted ${flag}`);
}

const activeProfile =
  manifest.activeRuntimeCheckpoint.capabilityProfile ??
  manifest.activeRuntimeCheckpoint.profile;
const active = planBgiCheckpointCommand([], manifest, root);
assert.deepEqual(active.selection, {
  kind: "active-profile",
  profile: activeProfile,
});
assert.equal(
  active.outputDirectory,
  bgiProfileOutputDirectory(activeProfile),
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
    activeProfile,
    "--runtime-policy",
  ],
);
assert.equal(
  active.runnerArguments[8],
  path.join(root, manifest.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source),
);
assert.deepEqual(active.runnerArguments.slice(9), [
  "--source-manifest",
  path.join(root, manifest.activeRuntimeCheckpoint.sourceSnapshot.source),
]);
assert.equal(
  manifest.activeRuntimeCheckpoint.sourceSnapshot.snapshotId,
  "primary-game-archives-v2",
);
assert.equal(manifest.activeRuntimeCheckpoint.writableProfile.profileId, "default");
assert.deepEqual(planBgiCheckpointCommand(["--profile", "active"], manifest, root), active);

const fullEvidence = planBgiCheckpointCommand(
  ["--profile", "active", "--evidence-level", "full"],
  manifest,
  root,
);
assert.equal(fullEvidence.runnerArguments[4], "full");

const explicitActive = planBgiCheckpointCommand(
  ["--profile", activeProfile],
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
assert.equal(
  bgiProfileOutputDirectory("kernel32-find-first-file-a"),
  "v86-bgi-find-first-file-a",
);
assert.equal(
  bgiProfileOutputDirectory("kernel32-delete-file-a"),
  "v86-bgi-delete-file-a",
);
assert.equal(
  bgiProfileOutputDirectory("shell32-drag-accept-files"),
  "v86-bgi-drag-accept-files",
);
assert.equal(bgiProfileOutputDirectory("ole32-com"), "v86-bgi-com");
assert.equal(bgiProfileOutputDirectory("winmm-audio"), "v86-bgi-audio");
assert.equal(bgiProfileOutputDirectory("gdi-window-dc-fill"), "v86-bgi-window-dc-fill");
assert.equal(
  bgiProfileOutputDirectory("d3d9-root-factory-and-identity"),
  "v86-bgi-d3d9-root-factory-and-identity",
);
assert.equal(
  bgiProfileOutputDirectory("dinput8-api-contract"),
  "v86-bgi-dinput8-api-contract",
);
assert.equal(
  bgiProfileOutputDirectory("dinput8-enum-devices"),
  "v86-bgi-dinput8-enum-devices",
);
assert.equal(
  bgiProfileOutputDirectory("dsound-root-and-cooperative-level"),
  "v86-bgi-dsound-root-and-cooperative-level",
);
assert.equal(
  bgiProfileOutputDirectory("dsound-initial-buffers"),
  "v86-bgi-dsound-initial-buffers",
);
assert.equal(
  bgiProfileOutputDirectory("dsound-buffer-lock-unlock-play"),
  "v86-bgi-dsound-buffer-lock-unlock-play",
);
assert.equal(
  bgiProfileOutputDirectory("user32-set-timer"),
  "v86-bgi-set-timer",
);
assert.equal(
  bgiProfileOutputDirectory("kernel32-create-thread"),
  "v86-bgi-create-thread",
);
assert.equal(
  bgiProfileOutputDirectory("kernel32-set-thread-priority"),
  "v86-bgi-set-thread-priority",
);
assert.equal(bgiProfileOutputDirectory("gdi-screen-dc"), "v86-bgi-screen-dc");
assert.equal(
  bgiProfileOutputDirectory("gdi-font-enumeration"),
  "v86-bgi-font-enumeration",
);
assert.equal(
  bgiProfileOutputDirectory("gdi-font-creation"),
  "v86-bgi-font-creation",
);
assert.equal(
  bgiProfileOutputDirectory("gdi-compatible-dc"),
  "v86-bgi-compatible-dc",
);
assert.equal(
  bgiProfileOutputDirectory("gdi-dib-lifecycle"),
  "v86-bgi-dib-lifecycle",
);
assert.equal(
  bgiProfileOutputDirectory("gdi-outline-text-metrics"),
  "v86-bgi-outline-text-metrics",
);
assert.equal(
  bgiProfileOutputDirectory("gdiplus-startup"),
  "v86-bgi-startup",
);
assert.equal(
  bgiProfileOutputDirectory("kernel32-virtual-alloc"),
  "v86-bgi-virtual-alloc",
);
assert.equal(
  bgiProfileOutputDirectory("imm32-default-ime-context"),
  "v86-bgi-default-ime-context",
);
assert.equal(
  bgiProfileOutputDirectory("kernel32-set-waitable-timer"),
  "v86-bgi-set-waitable-timer",
);
assert.equal(
  bgiProfileOutputDirectory("kernel32-wait-for-single-object-ex"),
  "v86-bgi-wait-for-single-object-ex",
);
assert.equal(
  bgiProfileOutputDirectory("kernel32-enter-critical-section"),
  "v86-bgi-enter-critical-section",
);
assert.equal(
  bgiProfileOutputDirectory("primary-main-loop-progression"),
  "v86-bgi-primary-main-loop-progression",
);
assert.equal(
  bgiProfileOutputDirectory("kernel32-leave-critical-section"),
  "v86-bgi-leave-critical-section",
);
assert.equal(
  bgiProfileOutputDirectory("kernel32-enter-critical-section-continuation"),
  "v86-bgi-enter-critical-section-continuation",
);
assert.equal(
  bgiProfileOutputDirectory("kernel32-leave-critical-section-continuation"),
  "v86-bgi-leave-critical-section-continuation",
);
assert.throws(() => bgiProfileOutputDirectory("xaudio2-audio"));

console.log("generic BGI checkpoint command contract passed");
