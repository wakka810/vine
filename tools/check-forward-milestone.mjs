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

function assertPrimaryAudio() {
  assert(report.audio.playbackRequests > 0, "no browser audio request was observed");
  assert(report.audio.playbacksStarted > 0, "no browser playback started");
  const playback = report.audio.latestPlayback;
  assert(playback !== null, "latest browser playback diagnostic is missing");
  assert.equal(playback.contextState, "running");
  assert(playback.frameCount > 0, "browser playback has no PCM frames");
  assert(playback.durationSeconds > 0, "browser playback duration is empty");
  assert(playback.peakAbsoluteSample > 0, "browser playback PCM is silent");
}

let acceptanceSummary;
switch (checkpoint.lastCompletedBoundary.id) {
  case "primary-audible-output": {
    assertPrimaryAudio();
    acceptanceSummary = `${report.audio.playbacksStarted} playback(s)`;
    break;
  }
  case "primary-visible-action-response-or-readable-frame": {
    assertPrimaryAudio();
    assert(report.input.pointerMessages > 0, "no primary pointer input reached the guest");
    assert(
      report.input.latestPointerMessage !== null,
      "latest primary pointer diagnostic is missing",
    );
    assert(report.presentation.presentCount > 0, "no primary Present was observed");
    const presentation = report.presentation.latest;
    assert(presentation !== null, "latest browser presentation is missing");
    assert(
      presentation.centerPixelRgba.slice(0, 3).some((component) => component > 0),
      "latest primary frame is still uniformly black at its center sample",
    );
    const frame = report.presentation.finalFrame;
    assert(frame !== null, "final primary frame artifact is missing");
    const framePath = path.resolve(root, frame.file);
    const targetDirectory = path.resolve(root, ".work", "target-forward");
    assert(
      framePath.startsWith(`${targetDirectory}${path.sep}`),
      "final frame artifact leaves the target-forward directory",
    );
    const frameBytes = fs.readFileSync(framePath);
    assert.equal(frameBytes.byteLength, frame.bytes);
    assert(frameBytes.byteLength > 0 && frameBytes.byteLength <= 2 * 1024 * 1024);
    acceptanceSummary = `${report.presentation.presentCount} Present(s), ${frameBytes.byteLength}-byte frame`;
    break;
  }
  default:
    throw new Error(
      `no bounded acceptance is registered for ${checkpoint.lastCompletedBoundary.id}`,
    );
}

console.log(
  `forward milestone accepted: ${checkpoint.lastCompletedBoundary.id}; ` +
    `${acceptanceSummary}, ${reportBytes.byteLength}-byte report`,
);
