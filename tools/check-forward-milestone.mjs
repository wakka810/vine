import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { loadCurrentCheckpoint } from "./current-checkpoint.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const checkpoint = loadCurrentCheckpoint(
  path.join(root, "docs", "current-checkpoint.json"),
);
const reportPath = path.resolve(root, checkpoint.development.reportPath);
const reportBytes = fs.readFileSync(reportPath);
assert(
  reportBytes.byteLength <= 2 * 1024 * 1024,
  `forward report exceeds 2 MiB (${reportBytes.byteLength} bytes)`,
);
const report = JSON.parse(reportBytes.toString("utf8"));

assert.equal(report.schema, "vine.target-forward-report");
assert.equal(report.schemaVersion, 1);
assert.equal(report.mode, "release-chrome-forward");
assert.equal(report.source.sha256, checkpoint.activeTarget.sourceSha256);
assert.equal(report.source.immutableAfterRun, true);
assert(
  Number.isFinite(report.elapsedMilliseconds) &&
    report.elapsedMilliseconds > 0 &&
    report.elapsedMilliseconds <= checkpoint.development.maximumRunSeconds * 1000,
  "forward run exceeded its configured development bound",
);
assert(Array.isArray(report.semanticTail), "forward semantic tail is missing");
assert(
  report.semanticTail.length <= checkpoint.development.semanticTailEntries,
  "forward semantic tail exceeds its configured bound",
);
assert.equal(report.pending.hostCalls, 0);
assert.equal(report.pending.callbacks, 0);
assert.equal(report.pending.hostRequestRecords, 0);
assert.equal(report.pending.timerPacing, 0);

switch (checkpoint.lastCompletedBoundary.id) {
  case "primary-audible-output": {
    assert(report.audio.playbackRequests > 0, "no browser audio request was observed");
    assert(report.audio.playbacksStarted > 0, "no browser playback started");
    const playback = report.audio.latestPlayback;
    assert(playback !== null, "latest browser playback diagnostic is missing");
    assert.equal(playback.contextState, "running");
    assert(playback.frameCount > 0, "browser playback has no PCM frames");
    assert(playback.durationSeconds > 0, "browser playback duration is empty");
    assert(
      playback.peakAbsoluteSample > 0,
      "browser playback PCM is silent",
    );
    break;
  }
  default:
    throw new Error(
      `no bounded acceptance is registered for ${checkpoint.lastCompletedBoundary.id}`,
    );
}

console.log(
  `forward milestone accepted: ${checkpoint.lastCompletedBoundary.id}; ` +
    `${report.audio.playbacksStarted} playback(s), ${reportBytes.byteLength} bytes`,
);
