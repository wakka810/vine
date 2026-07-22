import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";

const finite = (value) => typeof value === "number" && Number.isFinite(value);

function invariant(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function validatePositive(value, label) {
  invariant(finite(value) && value > 0, `${label} must be positive`);
}

export function validateBrowserPerformanceEnvironmentPolicy(policy) {
  invariant(policy !== null && typeof policy === "object", "environment policy is missing");
  invariant(
    policy.schema === "vine.browser-performance-environment",
    "environment policy schema is invalid",
  );
  invariant(policy.schemaVersion === 1, "environment policy schema version is unsupported");
  invariant(typeof policy.id === "string" && policy.id.length > 0, "environment policy ID is invalid");
  validatePositive(policy.sampleIntervalMilliseconds, "sample interval");
  invariant(
    finite(policy.minimumSampleCoverage) &&
      policy.minimumSampleCoverage > 0 &&
      policy.minimumSampleCoverage <= 1,
    "minimum sample coverage must be in (0, 1]",
  );
  validatePositive(policy.maximumSampleGapMilliseconds, "maximum sample gap");
  invariant(
    Number.isSafeInteger(policy.warmupSamples) && policy.warmupSamples >= 0,
    "warmup sample count is invalid",
  );
  for (const [sectionName, valueNames] of [
    ["cpu", ["maximumTotalPercent", "maximumExternalPercent"]],
    ["disk", ["maximumPhysicalBytesPerSecond", "maximumBusyPercent"]],
  ]) {
    const section = policy[sectionName];
    invariant(section !== null && typeof section === "object", `${sectionName} policy is missing`);
    for (const valueName of valueNames) {
      validatePositive(section[valueName], `${sectionName}.${valueName}`);
    }
    invariant(
      Number.isSafeInteger(section.consecutiveSamples) && section.consecutiveSamples > 0,
      `${sectionName}.consecutiveSamples is invalid`,
    );
  }
  invariant(policy.updater !== null && typeof policy.updater === "object", "updater policy is missing");
  for (const [name, values] of Object.entries({
    executableNames: policy.updater.executableNames,
    pathFragments: policy.updater.pathFragments,
  })) {
    invariant(Array.isArray(values) && values.length > 0, `updater.${name} must be non-empty`);
    invariant(
      values.every((value) => typeof value === "string" && value === value.toLowerCase()),
      `updater.${name} must contain lowercase strings`,
    );
  }
  invariant(policy.qos !== null && typeof policy.qos === "object", "QoS policy is missing");
  for (const name of [
    "invalidateOnPriorityClassChange",
    "invalidateOnPriorityBoostChange",
    "invalidateOnPowerThrottlingChange",
  ]) {
    invariant(typeof policy.qos[name] === "boolean", `qos.${name} must be boolean`);
  }
  return policy;
}

export async function loadBrowserPerformanceEnvironmentPolicy(policyPath) {
  const bytes = await readFile(policyPath);
  return {
    bytes,
    value: validateBrowserPerformanceEnvironmentPolicy(JSON.parse(bytes)),
  };
}

function maximum(values) {
  return values.length === 0 ? null : Math.max(...values);
}

function mean(values) {
  return values.length === 0 ? null : values.reduce((sum, value) => sum + value, 0) / values.length;
}

function longestExcursion(samples, selector, threshold, startIndex) {
  let current = [];
  let longest = [];
  for (const sample of samples.slice(startIndex)) {
    const value = selector(sample);
    if (finite(value) && value > threshold) {
      current.push({ sequence: sample.sequence, elapsedMilliseconds: sample.elapsedMilliseconds, value });
      if (current.length > longest.length) {
        longest = [...current];
      }
    } else {
      current = [];
    }
  }
  return longest;
}

function updaterMatches(candidate, updaterPolicy) {
  const name = String(candidate.name ?? "").toLowerCase();
  const executablePath = String(candidate.executablePath ?? "").toLowerCase();
  return (
    updaterPolicy.executableNames.includes(name) ||
    updaterPolicy.pathFragments.some((fragment) => executablePath.includes(fragment))
  );
}

function qosChanges(samples, policy) {
  const previous = new Map();
  const changes = [];
  for (const sample of samples) {
    for (const process of sample.browserProcesses ?? []) {
      if (typeof process.identity !== "string") {
        continue;
      }
      const before = previous.get(process.identity);
      if (before !== undefined) {
        const fields = [];
        if (
          policy.invalidateOnPriorityClassChange &&
          process.priorityClass !== before.priorityClass
        ) {
          fields.push("priority-class");
        }
        if (
          policy.invalidateOnPriorityBoostChange &&
          process.priorityBoostDisabled !== before.priorityBoostDisabled
        ) {
          fields.push("priority-boost");
        }
        if (
          policy.invalidateOnPowerThrottlingChange &&
          (process.powerThrottlingControlMask !== before.powerThrottlingControlMask ||
            process.powerThrottlingStateMask !== before.powerThrottlingStateMask)
        ) {
          fields.push("power-throttling");
        }
        if (fields.length > 0) {
          changes.push({
            identity: process.identity,
            processId: process.processId,
            name: process.name,
            sequence: sample.sequence,
            elapsedMilliseconds: sample.elapsedMilliseconds,
            fields,
            before,
            after: {
              priorityClass: process.priorityClass,
              priorityBoostDisabled: process.priorityBoostDisabled,
              powerThrottlingControlMask: process.powerThrottlingControlMask,
              powerThrottlingStateMask: process.powerThrottlingStateMask,
            },
          });
        }
      }
      previous.set(process.identity, {
        priorityClass: process.priorityClass,
        priorityBoostDisabled: process.priorityBoostDisabled,
        powerThrottlingControlMask: process.powerThrottlingControlMask,
        powerThrottlingStateMask: process.powerThrottlingStateMask,
      });
    }
  }
  return changes;
}

export function evaluateBrowserPerformanceEnvironment(policyValue, observation, durationMilliseconds) {
  const policy = validateBrowserPerformanceEnvironmentPolicy(policyValue);
  validatePositive(durationMilliseconds, "benchmark duration");
  invariant(observation !== null && typeof observation === "object", "environment observation is missing");
  const samples = Array.isArray(observation.samples) ? observation.samples : [];
  const reasons = [];
  const expectedSamples = Math.floor(durationMilliseconds / policy.sampleIntervalMilliseconds);
  const minimumSamples = Math.floor(expectedSamples * policy.minimumSampleCoverage);
  if (samples.length < minimumSamples) {
    reasons.push({
      id: "insufficient-monitor-samples",
      observed: samples.length,
      required: minimumSamples,
    });
  }
  const gaps = [];
  for (let index = 1; index < samples.length; index += 1) {
    const gap = samples[index].elapsedMilliseconds - samples[index - 1].elapsedMilliseconds;
    if (!finite(gap) || gap > policy.maximumSampleGapMilliseconds) {
      gaps.push({
        fromSequence: samples[index - 1].sequence,
        toSequence: samples[index].sequence,
        milliseconds: gap,
      });
    }
  }
  if (gaps.length > 0) {
    reasons.push({ id: "monitor-sample-gap", occurrences: gaps });
  }
  const monitorErrors = Array.isArray(observation.errors) ? observation.errors : [];
  if (monitorErrors.length > 0) {
    reasons.push({ id: "environment-monitor-error", occurrences: monitorErrors });
  }
  const counterMissing = samples.filter(
    (sample) =>
      !finite(sample.totalCpuPercent) ||
      !finite(sample.physicalDiskBytesPerSecond) ||
      !finite(sample.physicalDiskBusyPercent) ||
      sample.rootProcessPresent !== true,
  );
  if (counterMissing.length > Math.max(1, Math.floor(samples.length * 0.05))) {
    reasons.push({
      id: "required-environment-counter-missing",
      occurrences: counterMissing.map((sample) => sample.sequence),
    });
  }
  const updaterEvents = [];
  for (const sample of samples) {
    for (const candidate of sample.updateCandidates ?? []) {
      if (updaterMatches(candidate, policy.updater)) {
        updaterEvents.push({
          sequence: sample.sequence,
          elapsedMilliseconds: sample.elapsedMilliseconds,
          ...candidate,
        });
      }
    }
  }
  if (updaterEvents.length > 0) {
    reasons.push({ id: "updater-process-observed", occurrences: updaterEvents });
  }
  const excursionDefinitions = [
    {
      id: "sustained-total-cpu",
      selector: (sample) => sample.totalCpuPercent,
      threshold: policy.cpu.maximumTotalPercent,
      required: policy.cpu.consecutiveSamples,
    },
    {
      id: "sustained-external-cpu",
      selector: (sample) => sample.externalCpuPercent,
      threshold: policy.cpu.maximumExternalPercent,
      required: policy.cpu.consecutiveSamples,
    },
    {
      id: "sustained-physical-disk-throughput",
      selector: (sample) => sample.physicalDiskBytesPerSecond,
      threshold: policy.disk.maximumPhysicalBytesPerSecond,
      required: policy.disk.consecutiveSamples,
    },
    {
      id: "sustained-physical-disk-busy",
      selector: (sample) => sample.physicalDiskBusyPercent,
      threshold: policy.disk.maximumBusyPercent,
      required: policy.disk.consecutiveSamples,
    },
  ];
  const excursions = [];
  for (const definition of excursionDefinitions) {
    const occurrence = longestExcursion(
      samples,
      definition.selector,
      definition.threshold,
      policy.warmupSamples,
    );
    excursions.push({
      id: definition.id,
      threshold: definition.threshold,
      requiredConsecutiveSamples: definition.required,
      longestConsecutiveSamples: occurrence.length,
      occurrence,
    });
    if (occurrence.length >= definition.required) {
      reasons.push({
        id: definition.id,
        threshold: definition.threshold,
        requiredConsecutiveSamples: definition.required,
        occurrence,
      });
    }
  }
  const observedQosChanges = qosChanges(samples, policy.qos);
  if (observedQosChanges.length > 0) {
    reasons.push({ id: "chrome-qos-changed", occurrences: observedQosChanges });
  }
  const values = (name) => samples.map((sample) => sample[name]).filter(finite);
  return {
    schema: "vine.browser-performance-environment-decision",
    schemaVersion: 1,
    status: reasons.length === 0 ? "valid" : "invalid-environment",
    policyId: policy.id,
    reasonIds: reasons.map((reason) => reason.id),
    reasons,
    sampleCoverage: {
      expected: expectedSamples,
      minimum: minimumSamples,
      observed: samples.length,
    },
    metrics: {
      totalCpuPercent: {
        mean: mean(values("totalCpuPercent")),
        maximum: maximum(values("totalCpuPercent")),
      },
      browserCpuPercent: {
        mean: mean(values("browserCpuPercent")),
        maximum: maximum(values("browserCpuPercent")),
      },
      externalCpuPercent: {
        mean: mean(values("externalCpuPercent")),
        maximum: maximum(values("externalCpuPercent")),
      },
      physicalDiskBytesPerSecond: {
        mean: mean(values("physicalDiskBytesPerSecond")),
        maximum: maximum(values("physicalDiskBytesPerSecond")),
      },
      physicalDiskBusyPercent: {
        mean: mean(values("physicalDiskBusyPercent")),
        maximum: maximum(values("physicalDiskBusyPercent")),
      },
      browserIoBytesPerSecond: {
        mean: mean(values("browserIoBytesPerSecond")),
        maximum: maximum(values("browserIoBytesPerSecond")),
      },
    },
    excursions,
    updaterEvents,
    qosChanges: observedQosChanges,
  };
}

export function qualifyBenchmarkDecision(workloadDecision, environmentDecision) {
  invariant(workloadDecision !== null && typeof workloadDecision === "object", "workload decision is missing");
  invariant(
    environmentDecision?.status === "valid" ||
      environmentDecision?.status === "invalid-environment",
    "environment decision status is invalid",
  );
  if (environmentDecision.status === "valid") {
    return {
      ...workloadDecision,
      environmentStatus: "valid",
      invalidEnvironmentReasonIds: [],
    };
  }
  return {
    ...workloadDecision,
    status: "invalid-environment",
    reason: `${environmentDecision.reasonIds.length}-environment-qualification-reasons`,
    environmentStatus: "invalid-environment",
    invalidEnvironmentReasonIds: environmentDecision.reasonIds,
  };
}

export async function startBrowserPerformanceEnvironmentMonitor({
  root,
  rootProcessId,
  policy,
  durationMilliseconds,
}) {
  validateBrowserPerformanceEnvironmentPolicy(policy);
  assert.equal(process.platform, "win32", "browser performance environment monitor is Windows-only");
  assert(Number.isSafeInteger(rootProcessId) && rootProcessId > 0, "Chrome root process ID is invalid");
  const scriptPath = path.join(root, "tools", "windows", "monitor-browser-performance-environment.ps1");
  const child = spawn(
    "powershell.exe",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      scriptPath,
      "-RootProcessId",
      String(rootProcessId),
      "-SampleIntervalMilliseconds",
      String(policy.sampleIntervalMilliseconds),
    ],
    { cwd: root, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] },
  );
  const samples = [];
  const errors = [];
  let ready = null;
  let stdoutBuffer = "";
  let stderr = "";
  let exited = false;
  let exitDetails = null;
  const exit = new Promise((resolve) => {
    child.once("close", (code, signal) => {
      exited = true;
      exitDetails = { code, signal };
      resolve(exitDetails);
    });
  });
  let settleReady;
  let rejectReady;
  let readySettled = false;
  const readyPromise = new Promise((resolve, reject) => {
    settleReady = resolve;
    rejectReady = reject;
  });
  const readyTimeout = setTimeout(() => {
    rejectReady(new Error(`environment monitor did not become ready\n${stderr}`.trim()));
  }, 10_000);
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  child.stdout.on("data", (chunk) => {
    stdoutBuffer += chunk;
    const lines = stdoutBuffer.split(/\r?\n/u);
    stdoutBuffer = lines.pop() ?? "";
    for (const line of lines) {
      if (line.trim().length === 0) {
        continue;
      }
      try {
        const item = JSON.parse(line);
        if (item.kind === "ready" && ready === null) {
          ready = item;
        } else if (item.kind === "sample") {
          samples.push(item);
        } else if (item.kind === "error") {
          errors.push(item);
        } else {
          errors.push({ kind: "unexpected-monitor-output", value: item });
        }
      } catch (error) {
        errors.push({ kind: "monitor-json-error", line, message: error.message });
      }
    }
    if (!readySettled && ready !== null && samples.length > 0) {
      readySettled = true;
      clearTimeout(readyTimeout);
      settleReady(ready);
    }
  });
  child.once("error", (error) => {
    clearTimeout(readyTimeout);
    rejectReady(error);
  });
  exit.then(({ code, signal }) => {
    if (ready === null) {
      clearTimeout(readyTimeout);
      rejectReady(new Error(`environment monitor exited before ready (${signal ?? code})\n${stderr}`));
    }
  });
  await readyPromise;

  return {
    ready,
    async stop() {
      if (!exited) {
        child.kill("SIGKILL");
        await Promise.race([
          exit,
          new Promise((resolve) => setTimeout(resolve, 2_000)),
        ]);
      }
      if (!exited) {
        errors.push({ kind: "monitor-stop-timeout" });
        child.kill("SIGKILL");
        await exit;
      }
      const observation = {
        schema: "vine.browser-performance-environment-observation",
        schemaVersion: 1,
        ready,
        monitorExit: exitDetails,
        stderr: stderr.trim(),
        errors,
        samples,
      };
      return {
        observation,
        decision: evaluateBrowserPerformanceEnvironment(
          policy,
          observation,
          durationMilliseconds,
        ),
      };
    },
  };
}
