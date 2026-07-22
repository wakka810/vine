import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import {
  evaluateBrowserPerformanceEnvironment,
  loadBrowserPerformanceEnvironmentPolicy,
  qualifyBenchmarkDecision,
  startBrowserPerformanceEnvironmentMonitor,
  validateBrowserPerformanceEnvironmentPolicy,
} from "./browser-performance-environment.mjs";
import {
  CANONICAL_CHROME_FOR_TESTING_MANIFEST,
  validateChromeForTestingManifest,
} from "./chrome-for-testing.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const policyPath = path.join(
  root,
  "fixtures",
  "benchmarks",
  "browser-performance-environment-windows-v1.json",
);
const { value: policy } = await loadBrowserPerformanceEnvironmentPolicy(policyPath);
const chromeManifest = validateChromeForTestingManifest(
  JSON.parse(await readFile(path.join(root, CANONICAL_CHROME_FOR_TESTING_MANIFEST))),
);
assert.equal(chromeManifest.version, "151.0.7922.34");
assert.equal(
  chromeManifest.source.archive.sha256,
  "045621e45a9dd27002c7fc1d8e10fe9f5f71f4cadbf44ec6f397f56f0179725c",
);
assert.equal(
  chromeManifest.executable.sha256,
  "409805a16d6416087e6b2f778df1cf8f7bbb267d6b99f6b5bb0a618eace234f2",
);
const packageJson = JSON.parse(await readFile(path.join(root, "package.json")));
assert.match(
  packageJson.scripts["check:runtime:bgi-browser-benchmark"],
  /npm run prepare:chrome-for-testing/u,
);
assert.equal(
  packageJson.scripts["prepare:chrome-for-testing"],
  "node tools/prepare-chrome-for-testing.mjs",
);
assert.throws(
  () =>
    validateChromeForTestingManifest({
      ...chromeManifest,
      executable: { ...chromeManifest.executable, relativePath: "../chrome.exe" },
    }),
  /escapes its artifact root/u,
);
assert.throws(
  () => validateBrowserPerformanceEnvironmentPolicy({ ...policy, minimumSampleCoverage: 1.1 }),
  /minimum sample coverage/u,
);

const processState = {
  identity: "100:1",
  processId: 100,
  name: "chrome.exe",
  priorityClass: 32,
  priorityBoostDisabled: false,
  powerThrottlingControlMask: 0,
  powerThrottlingStateMask: 0,
};
const makeSamples = () =>
  Array.from({ length: 8 }, (_, sequence) => ({
    kind: "sample",
    sequence,
    elapsedMilliseconds: (sequence + 1) * 1_000,
    rootProcessPresent: true,
    totalCpuPercent: 10,
    browserCpuPercent: 8,
    externalCpuPercent: 2,
    physicalDiskBytesPerSecond: 1_000,
    physicalDiskBusyPercent: 1,
    browserIoBytesPerSecond: 100,
    browserProcesses: [{ ...processState }],
    updateCandidates: [],
  }));
const syntheticPolicy = structuredClone(policy);
syntheticPolicy.warmupSamples = 0;
syntheticPolicy.minimumSampleCoverage = 0.75;
const evaluate = (samples, errors = []) =>
  evaluateBrowserPerformanceEnvironment(
    syntheticPolicy,
    { samples, errors },
    samples.length * syntheticPolicy.sampleIntervalMilliseconds,
  );

const valid = evaluate(makeSamples());
assert.equal(valid.status, "valid");
assert.deepEqual(valid.reasonIds, []);
assert.equal(valid.sampleCoverage.observed, 8);

const updaterSamples = makeSamples();
updaterSamples[2].updateCandidates.push({
  processId: 900,
  name: "GoogleUpdater.exe",
  executablePath: "C:\\Program Files (x86)\\Google\\GoogleUpdater\\updater.exe",
  identity: "900:1",
});
assert.deepEqual(evaluate(updaterSamples).reasonIds, ["updater-process-observed"]);

const cpuSamples = makeSamples();
for (const index of [2, 3, 4]) {
  cpuSamples[index].externalCpuPercent = 20;
}
assert(evaluate(cpuSamples).reasonIds.includes("sustained-external-cpu"));

const diskSamples = makeSamples();
for (const index of [1, 2, 3]) {
  diskSamples[index].physicalDiskBytesPerSecond = 40 * 1024 * 1024;
}
assert(evaluate(diskSamples).reasonIds.includes("sustained-physical-disk-throughput"));

const qosSamples = makeSamples();
for (let index = 4; index < qosSamples.length; index += 1) {
  qosSamples[index].browserProcesses[0].powerThrottlingStateMask = 1;
}
assert(evaluate(qosSamples).reasonIds.includes("chrome-qos-changed"));

const warmupQosPolicy = structuredClone(syntheticPolicy);
warmupQosPolicy.warmupSamples = 5;
const warmupQos = evaluateBrowserPerformanceEnvironment(
  warmupQosPolicy,
  { samples: qosSamples, errors: [] },
  qosSamples.length * warmupQosPolicy.sampleIntervalMilliseconds,
);
assert.equal(warmupQos.status, "valid");
assert.equal(warmupQos.qosChanges.length, 1);
assert.equal(warmupQos.qosChanges[0].sampleIndex, 4);

const workloadDecision = {
  classification: "repeated-distribution-threshold",
  status: "passed",
  reason: "all-17-versioned-thresholds-passed",
  evaluationCount: 17,
  failureIds: [],
};
assert.equal(qualifyBenchmarkDecision(workloadDecision, valid).status, "passed");
assert.deepEqual(
  qualifyBenchmarkDecision(workloadDecision, evaluate(updaterSamples)).invalidEnvironmentReasonIds,
  ["updater-process-observed"],
);

if (process.platform === "win32") {
  const liveMonitor = await startBrowserPerformanceEnvironmentMonitor({
    root,
    rootProcessId: process.pid,
    policy,
    durationMilliseconds: 2_000,
  });
  await new Promise((resolve) => setTimeout(resolve, 2_200));
  const live = await liveMonitor.stop();
  assert(live.observation.samples.length >= 2, "live monitor did not provide two samples");
  assert(
    live.observation.samples.every(
      (sample) =>
        sample.rootProcessPresent === true &&
        Number.isFinite(sample.totalCpuPercent) &&
        Number.isFinite(sample.physicalDiskBytesPerSecond),
    ),
    "live monitor omitted a required Windows counter",
  );
  assert(
    live.observation.samples.every((sample) =>
      sample.browserProcesses.some(
        (entry) =>
          entry.processId === process.pid &&
          Number.isSafeInteger(entry.priorityClass) &&
          typeof entry.priorityBoostDisabled === "boolean" &&
          Number.isSafeInteger(entry.powerThrottlingControlMask) &&
          Number.isSafeInteger(entry.powerThrottlingStateMask),
      ),
    ),
    "live monitor omitted the target process QoS state",
  );
  assert(
    !live.decision.reasonIds.includes("environment-monitor-error"),
    "live monitor reported an internal error",
  );
}

console.log("browser performance environment contract passed");
