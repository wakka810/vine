import fs from "node:fs";

export const README_STATUS_START = "<!-- BEGIN GENERATED CURRENT STATUS -->";
export const README_STATUS_END = "<!-- END GENERATED CURRENT STATUS -->";

const SHA256 = /^[0-9a-f]{64}$/u;
const VALID_STATUS = new Set(["complete", "partial", "gateway", "blocked"]);

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function nonEmptyString(value) {
  return typeof value === "string" && value.length > 0;
}

export function validateCurrentCheckpoint(manifest) {
  const errors = [];
  if (!isRecord(manifest)) {
    return ["current state must be an object"];
  }
  if (manifest.schema !== "vine.current-state") {
    errors.push("schema must be vine.current-state");
  }
  if (manifest.schemaVersion !== 6) {
    errors.push("schemaVersion must be 6");
  }
  if (!/^\d{4}-\d{2}-\d{2}$/u.test(manifest.updatedAt ?? "")) {
    errors.push("updatedAt must be YYYY-MM-DD");
  }

  for (const [label, value] of [
    ["activeTarget", manifest.activeTarget],
    ["activeMilestone", manifest.activeMilestone],
    ["lastCompletedBoundary", manifest.lastCompletedBoundary],
    ["currentForwardStop", manifest.currentForwardStop],
    ["commands", manifest.commands],
    ["development", manifest.development],
    ["historicalAcceptance", manifest.historicalAcceptance],
  ]) {
    if (!isRecord(value)) {
      errors.push(`${label} must be an object`);
    }
  }

  const target = isRecord(manifest.activeTarget) ? manifest.activeTarget : {};
  for (const key of ["title", "executable", "architecture", "browser"]) {
    if (!nonEmptyString(target[key])) {
      errors.push(`activeTarget.${key} must be a non-empty string`);
    }
  }
  if (!SHA256.test(target.sourceSha256 ?? "")) {
    errors.push("activeTarget.sourceSha256 must be a lowercase SHA-256");
  }

  for (const [label, value, keys] of [
    ["activeMilestone", manifest.activeMilestone, ["id", "exitCondition"]],
    ["lastCompletedBoundary", manifest.lastCompletedBoundary, ["id", "summary"]],
    ["currentForwardStop", manifest.currentForwardStop, ["kind", "classification", "endpoint", "reason"]],
  ]) {
    const record = isRecord(value) ? value : {};
    for (const key of keys) {
      if (!nonEmptyString(record[key])) {
        errors.push(`${label}.${key} must be a non-empty string`);
      }
    }
  }
  if (!nonEmptyString(manifest.nextDevelopmentGoal)) {
    errors.push("nextDevelopmentGoal must be a non-empty string");
  }

  if (!Array.isArray(manifest.supportedSubsystems) || manifest.supportedSubsystems.length === 0) {
    errors.push("supportedSubsystems must be a non-empty array");
  } else {
    const ids = new Set();
    for (const [index, subsystem] of manifest.supportedSubsystems.entries()) {
      if (!isRecord(subsystem)) {
        errors.push(`supportedSubsystems[${index}] must be an object`);
        continue;
      }
      if (!nonEmptyString(subsystem.id) || ids.has(subsystem.id)) {
        errors.push(`supportedSubsystems[${index}].id must be unique and non-empty`);
      }
      ids.add(subsystem.id);
      if (!VALID_STATUS.has(subsystem.status)) {
        errors.push(`supportedSubsystems[${index}].status is invalid`);
      }
      if (!nonEmptyString(subsystem.summary)) {
        errors.push(`supportedSubsystems[${index}].summary must be non-empty`);
      }
    }
  }

  if (!Array.isArray(manifest.importantNonClaims) || manifest.importantNonClaims.some((value) => !nonEmptyString(value))) {
    errors.push("importantNonClaims must be an array of non-empty strings");
  }
  if (!Array.isArray(manifest.artifactReferences)) {
    errors.push("artifactReferences must be an array");
  }

  const commands = isRecord(manifest.commands) ? manifest.commands : {};
  for (const key of ["forward", "milestoneAcceptance", "fastCheck"]) {
    if (!nonEmptyString(commands[key])) {
      errors.push(`commands.${key} must be a non-empty string`);
    }
  }
  const development = isRecord(manifest.development) ? manifest.development : {};
  if (development.mode !== "forward") {
    errors.push("development.mode must be forward");
  }
  if (development.runner !== "generic-forward") {
    errors.push("development.runner must be generic-forward");
  }
  for (const key of ["compatibilityPolicy", "runtimeProtocol", "reportPath"]) {
    if (!nonEmptyString(development[key])) {
      errors.push(`development.${key} must be a non-empty string`);
    }
  }
  const historicalAcceptance = isRecord(manifest.historicalAcceptance)
    ? manifest.historicalAcceptance
    : {};
  for (const key of ["supportProfile", "runtimeProfile"]) {
    if (!nonEmptyString(historicalAcceptance[key])) {
      errors.push(`historicalAcceptance.${key} must be a non-empty string`);
    }
  }
  if (!Number.isSafeInteger(development.maximumRunSeconds) || development.maximumRunSeconds < 1 || development.maximumRunSeconds > 600) {
    errors.push("development.maximumRunSeconds must be an integer from 1 to 600");
  }
  if (!Number.isSafeInteger(development.semanticTailEntries) || development.semanticTailEntries < 256 || development.semanticTailEntries > 512) {
    errors.push("development.semanticTailEntries must be an integer from 256 to 512");
  }
  return errors;
}

export function loadCurrentCheckpoint(file) {
  const manifest = JSON.parse(fs.readFileSync(file, "utf8"));
  const errors = validateCurrentCheckpoint(manifest);
  if (errors.length > 0) {
    throw new Error(errors.join("\n"));
  }
  return manifest;
}

export function renderReadmeStatus(manifest) {
  const rows = manifest.supportedSubsystems
    .map(({ id, status, summary }) => `| \`${id}\` | \`${status}\` | ${summary} |`)
    .join("\n");
  const nonClaims = manifest.importantNonClaims
    .map((claim) => `- ${claim}`)
    .join("\n");
  return `${README_STATUS_START}
## Current Status

This section is generated from [\`docs/current-checkpoint.json\`](docs/current-checkpoint.json). Edit the compact state and run \`npm run docs:generate\`.

| Area | Current state |
|---|---|
| Active target | ${manifest.activeTarget.title} \`${manifest.activeTarget.executable}\` (${manifest.activeTarget.architecture}) in ${manifest.activeTarget.browser} |
| Active milestone | \`${manifest.activeMilestone.id}\`: ${manifest.activeMilestone.exitCondition} |
| Last completed boundary | \`${manifest.lastCompletedBoundary.id}\`: ${manifest.lastCompletedBoundary.summary} |
| Current forward stop | \`${manifest.currentForwardStop.classification}\` / \`${manifest.currentForwardStop.endpoint}\`: ${manifest.currentForwardStop.reason} |
| Next development goal | ${manifest.nextDevelopmentGoal} |

### Subsystems

| Subsystem | Status | Boundary |
|---|---|---|
${rows}

### Important Non-Claims

${nonClaims}

### Commands

\`\`\`sh
${manifest.commands.forward}
${manifest.commands.fastCheck}
${manifest.commands.milestoneAcceptance}
\`\`\`
${README_STATUS_END}`;
}

export function replaceReadmeStatus(readme, generated) {
  const start = readme.indexOf(README_STATUS_START);
  const end = readme.indexOf(README_STATUS_END);
  if (start < 0 || end < start) {
    throw new Error("README generated status markers are missing or out of order");
  }
  if (readme.indexOf(README_STATUS_START, start + 1) >= 0 || readme.indexOf(README_STATUS_END, end + 1) >= 0) {
    throw new Error("README contains duplicate generated status markers");
  }
  return `${readme.slice(0, start)}${generated}${readme.slice(end + README_STATUS_END.length)}`;
}
