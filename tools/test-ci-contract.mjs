import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { parse } from "yaml";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const loadWorkflow = (name) =>
  parse(fs.readFileSync(path.join(root, ".github", "workflows", name), "utf8"));
const ci = loadWorkflow("ci.yml");
const scheduled = loadWorkflow("fuzz.yml");
const benchmarkRunner = fs.readFileSync(
  path.join(root, "tools", "run-v86-browser-benchmark.mjs"),
  "utf8",
);
const benchmarkWorker = fs.readFileSync(
  path.join(root, "browser", "v86-benchmark", "benchmark-worker.ts"),
  "utf8",
);
const expectedJobs = [
  "browser-acceptance",
  "chrome-stable-observation",
  "fixture-build",
  "fixture-core-e2e",
  "fuzz-smoke",
  "performance-observation",
  "rust-tests",
  "static-semantic",
  "windows-native-oracle",
];

assert.deepEqual(Object.keys(ci.jobs).sort(), expectedJobs);
assert.equal(ci.env.VINE_ACCEPTANCE_CHROME_VERSION, "151.0.7922.34");

const runText = (job) =>
  job.steps
    .map((step) => step.run ?? "")
    .filter(Boolean)
    .join("\n");
const chromeVersion = (job) =>
  job.steps.find((step) => step.uses === "browser-actions/setup-chrome@v2")?.with?.[
    "chrome-version"
  ];
const downloadedArtifact = (job) =>
  job.steps.find((step) => step.uses === "actions/download-artifact@v8")?.with
    ?.name;

assert.match(runText(ci.jobs["static-semantic"]), /npm run check:static/u);
assert.match(runText(ci.jobs["rust-tests"]), /cargo test --workspace --locked/u);
assert.equal(ci.jobs["rust-tests"].needs, "fixture-build");
assert.equal(downloadedArtifact(ci.jobs["rust-tests"]), "public-fixture-inputs");
assert.match(runText(ci.jobs["fixture-build"]), /npm run prepare:engine/u);
assert.equal(ci.jobs["fixture-core-e2e"].needs, "fixture-build");
assert.equal(
  downloadedArtifact(ci.jobs["fixture-core-e2e"]),
  "public-fixture-inputs",
);
assert.equal(ci.jobs["browser-acceptance"].needs, "fixture-build");
assert.equal(
  downloadedArtifact(ci.jobs["browser-acceptance"]),
  "public-fixture-inputs",
);
assert.equal(
  chromeVersion(ci.jobs["browser-acceptance"]),
  "${{ env.VINE_ACCEPTANCE_CHROME_VERSION }}",
);
assert.equal(ci.jobs["performance-observation"].needs, "browser-acceptance");
assert.equal(
  chromeVersion(ci.jobs["performance-observation"]),
  "${{ env.VINE_ACCEPTANCE_CHROME_VERSION }}",
);
assert.equal(ci.jobs["chrome-stable-observation"].needs, "browser-acceptance");
assert.equal(ci.jobs["chrome-stable-observation"]["continue-on-error"], true);
assert.equal(chromeVersion(ci.jobs["chrome-stable-observation"]), "stable");
assert.equal(ci.jobs["windows-native-oracle"]["runs-on"], "windows-2022");
assert.match(runText(ci.jobs["windows-native-oracle"]), /npm run check:toolchain/u);
assert.match(runText(ci.jobs["windows-native-oracle"]), /VINE_WASM_CLANG/u);
assert.match(runText(ci.jobs["windows-native-oracle"]), /--print-targets/u);
assert.match(
  benchmarkRunner,
  /\["tools\/run-browser-runtime-check\.mjs", "--serve", "--port", "0"\]/u,
);
assert.match(benchmarkWorker, /const productionApiSetupBudget = 16;/u);
assert.match(benchmarkWorker, /mirror\.execute\(productionApiSetupBudget\)/u);

const fuzzSmoke = runText(ci.jobs["fuzz-smoke"]);
for (const target of [
  "pe32_load_plan",
  "cpu_writeback_decode",
  "source_read_response_decode",
]) {
  assert.match(fuzzSmoke, new RegExp(`\\b${target}\\b`, "u"));
  assert(scheduled.jobs["long-fuzz"].strategy.matrix.target.includes(target));
}
assert.match(fuzzSmoke, /-max_total_time=15/u);
assert.equal(scheduled.on.schedule.length, 1);
assert.match(runText(scheduled.jobs["long-fuzz"]), /-max_total_time=1800/u);

for (const workflow of [ci, scheduled]) {
  for (const job of Object.values(workflow.jobs)) {
    for (const match of runText(job).matchAll(/npm run ([\w:-]+)/gu)) {
      assert(
        Object.hasOwn(packageJson.scripts, match[1]),
        `workflow references unknown npm script ${match[1]}`,
      );
    }
  }
}

console.log("CI contract passed: split gates, pinned acceptance Chrome, Windows oracle, fuzz tiers");
