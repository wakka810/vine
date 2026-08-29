import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { loadCurrentCheckpoint } from "./current-checkpoint.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const checkpoint = loadCurrentCheckpoint(
  path.join(root, "docs", "current-checkpoint.json"),
);
const reportReference =
  checkpoint.lastCompletedBoundary.id === "primary-first-route-ending"
    ? checkpoint.artifactReferences.find((reference) =>
        reference.endsWith("first-route-ending-report.json"),
      )
    : checkpoint.development.reportPath;
assert(reportReference, "accepted milestone report reference is missing");
const reportPath = path.resolve(root, reportReference);
function loadBoundedReport(file) {
  const bytes = fs.readFileSync(file);
  assert(
    bytes.byteLength <= 2 * 1024 * 1024,
    `forward report exceeds 2 MiB (${bytes.byteLength} bytes)`,
  );
  const value = JSON.parse(bytes.toString("utf8"));
  assert.equal(value.schema, "vine.target-forward-report");
  assert.equal(value.schemaVersion, 1);
  assert.equal(value.mode, "release-chrome-forward");
  assert.equal(value.source.sha256, checkpoint.activeTarget.sourceSha256);
  assert.equal(value.source.immutableAfterRun, true);
  assert(
    Number.isFinite(value.elapsedMilliseconds) &&
      value.elapsedMilliseconds > 0 &&
      value.elapsedMilliseconds <= checkpoint.development.maximumRunSeconds * 1000,
    "forward run exceeded its configured development bound",
  );
  assert(Array.isArray(value.semanticTail), "forward semantic tail is missing");
  assert(
    value.semanticTail.length <= checkpoint.development.semanticTailEntries,
    "forward semantic tail exceeds its configured bound",
  );
  assert.equal(value.pending.hostCalls, 0);
  assert.equal(value.pending.callbacks, 0);
  assert.equal(value.pending.hostRequestRecords, 0);
  assert.equal(value.pending.timerPacing, 0);
  return { bytes, value };
}

const { bytes: reportBytes, value: report } = loadBoundedReport(reportPath);

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

function assertVisiblePrimaryFrame() {
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
  return frameBytes.byteLength;
}

function assertUnchangedHydratedSaveSlot() {
  const restartProfile = report.storage.profile;
  const hydration = report.storage.latestWritableHydration;
  assert(restartProfile !== null, "restart storage profile is missing");
  assert(hydration !== null, "restart save hydration is missing");
  const hydratedBefore = restartProfile.before.files.find(
    (file) => file.objectId === hydration.objectId,
  );
  const hydratedAfter = restartProfile.after.files.find(
    (file) => file.objectId === hydration.objectId,
  );
  assert(hydratedBefore !== undefined && hydratedAfter !== undefined);
  const relativePath = Buffer.from(hydratedBefore.relativePathA).toString("latin1");
  assert(
    /^UserData\\[^\\]+\.sud$/i.test(relativePath),
    "latest restart hydration is not a title-local save slot",
  );
  assert(hydratedBefore.exists, "hydrated save slot is not marked present");
  assert(hydratedBefore.generation > 0, "hydrated save slot has no generation");
  assert(hydratedBefore.byteLength > 0, "hydrated save slot is empty");
  assert.equal(hydratedBefore.generation, hydration.generation);
  assert.equal(hydratedBefore.byteLength, hydration.byteLength);
  assert.equal(hydratedBefore.sha256, hydration.sha256);
  assert.deepEqual(hydratedAfter, hydratedBefore);
  assert(
    report.storage.writableHydratedObjects >= 2,
    "restart did not hydrate configuration and save objects",
  );
  assert(
    report.storage.writableHydratedBytes >= hydration.byteLength,
    "restart hydration byte total excludes the save slot",
  );
  return { hydration, relativePath };
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
    const frameByteLength = assertVisiblePrimaryFrame();
    acceptanceSummary = `${report.presentation.presentCount} Present(s), ${frameByteLength}-byte frame`;
    break;
  }
  case "primary-save-browser-restart-load": {
    const commitReportPath = path.resolve(
      root,
      ".work",
      "target-forward",
      "persistence-commit-report.json",
    );
    const { value: commitReport } = loadBoundedReport(commitReportPath);
    assert(
      Date.parse(commitReport.measuredAt) < Date.parse(report.measuredAt),
      "persistence commit must precede the restart run",
    );
    const committedProfile = commitReport.storage.profile;
    const restartProfile = report.storage.profile;
    assert(committedProfile !== null && restartProfile !== null);
    assert.equal(committedProfile.database, restartProfile.database);
    assert(
      committedProfile.database.endsWith("/forward.sqlite"),
      "forward persistence did not use the durable development profile",
    );
    const committedBefore = committedProfile.before.files[0];
    const committedAfter = committedProfile.after.files[0];
    const commit = commitReport.storageLatest.writableCommit;
    assert(commit !== null && commit.succeeded && commit.win32Error === 0);
    assert.equal(commit.request.objectId, committedAfter.objectId);
    assert.equal(commit.request.baseGeneration, committedBefore.generation);
    assert.equal(commit.committedGeneration, committedAfter.generation);
    assert.equal(commit.request.byteLength, committedAfter.byteLength);
    assert.equal(commit.request.contentSha256, committedAfter.sha256);
    assert(committedAfter.generation > committedBefore.generation);

    const restartBefore = restartProfile.before.files[0];
    const restartAfter = restartProfile.after.files[0];
    const hydration = report.storage.latestWritableHydration;
    assert.equal(restartBefore.generation, committedAfter.generation);
    assert.equal(restartBefore.byteLength, committedAfter.byteLength);
    assert.equal(restartBefore.sha256, committedAfter.sha256);
    assert.equal(report.storage.writableHydratedObjects, 1);
    assert.equal(report.storage.writableHydratedBytes, committedAfter.byteLength);
    assert.deepEqual(hydration, {
      objectId: committedAfter.objectId,
      generation: committedAfter.generation,
      byteLength: committedAfter.byteLength,
      sha256: committedAfter.sha256,
    });
    assert.deepEqual(restartAfter, restartBefore);
    assert(report.presentation.presentCount > 0, "restart did not resume guest presentation");
    acceptanceSummary =
      `generation ${committedBefore.generation}->${committedAfter.generation}, ` +
      `${committedAfter.byteLength}-byte restart hydration`;
    break;
  }
  case "primary-save-slot-restart-catalog": {
    assertPrimaryAudio();
    const frameByteLength = assertVisiblePrimaryFrame();
    assert(report.input.pointerMessages > 0, "no catalog pointer input reached the guest");
    assert(report.input.completedActions > 0, "the pointer campaign completed no actions");
    const { hydration, relativePath } = assertUnchangedHydratedSaveSlot();
    acceptanceSummary =
      `${relativePath}, generation ${hydration.generation}, ` +
      `${hydration.byteLength}-byte hydration, ${report.presentation.presentCount} Present(s), ` +
      `${frameByteLength}-byte frame`;
    break;
  }
  case "primary-save-slot-restart-load": {
    assertPrimaryAudio();
    const frameByteLength = assertVisiblePrimaryFrame();
    assert(report.input.pointerMessages > 0, "no Load pointer input reached the guest");
    assert(report.input.completedActions > 0, "the Load campaign completed no actions");
    assert(report.input.latestPointerMessage !== null, "latest Load pointer message is missing");
    assert.equal(report.outcome, "cancelled");
    assert.equal(report.stop.endpoint, null);
    assert.equal(report.stop.fault.code, "forward-run-cancelled");
    const { hydration, relativePath } = assertUnchangedHydratedSaveSlot();
    acceptanceSummary =
      `${relativePath}, generation ${hydration.generation}, ` +
      `${hydration.byteLength}-byte unchanged save, ${report.presentation.presentCount} Present(s), ` +
      `${report.audio.playbacksStarted} playback(s), ${frameByteLength}-byte restored frame`;
    break;
  }
  case "primary-representative-flow": {
    assertPrimaryAudio();
    const frameByteLength = assertVisiblePrimaryFrame();
    assert.equal(report.outcome, "watchdog");
    assert.equal(report.stop.endpoint, null);
    assert.equal(report.stop.fault.code, "forward-run-budget-exhausted");
    assert(
      report.input.completedActions >= 100,
      "the scenario pointer campaign did not sustain representative input",
    );
    assert(
      report.presentation.presentCount >= report.input.completedActions,
      "scenario input did not produce a sustained presentation sequence",
    );
    const commit = report.storageLatest.writableCommit;
    assert(commit !== null && commit.succeeded && commit.win32Error === 0);
    assert(report.storage.writableCommits > 0);
    acceptanceSummary =
      `${report.input.completedActions} completed action(s), ` +
      `${report.presentation.presentCount} Present(s), ` +
      `${report.audio.playbacksStarted} playback(s), ${frameByteLength}-byte later frame`;
    break;
  }
  case "primary-first-route-ending": {
    assertPrimaryAudio();
    assert.equal(report.outcome, "checkpoint-complete");
    assert.equal(report.stop, null);
    assert(report.input.completedActions > 0, "the ending run completed no input actions");
    assert(report.presentation.presentCount > 0, "the ending run produced no Present");
    const frame = report.presentation.finalFrame;
    assert(frame !== null, "the ending frame artifact is missing");
    const frameReference = checkpoint.artifactReferences.find((reference) =>
      reference.endsWith("first-route-ending-final-frame.png"),
    );
    assert(frameReference, "accepted ending frame reference is missing");
    const framePath = path.resolve(root, frameReference);
    const targetDirectory = path.resolve(root, ".work", "target-forward");
    assert(framePath.startsWith(`${targetDirectory}${path.sep}`));
    const frameBytes = fs.readFileSync(framePath);
    assert.equal(frameBytes.byteLength, frame.bytes);
    const profile = report.storage.profile;
    const hydration = report.storage.latestWritableHydration;
    assert(profile !== null && hydration !== null);
    const hydratedBefore = profile.before.files.find(
      (file) => file.objectId === hydration.objectId,
    );
    const hydratedAfter = profile.after.files.find(
      (file) => file.objectId === hydration.objectId,
    );
    assert(hydratedBefore !== undefined && hydratedAfter !== undefined);
    assert(hydratedBefore.exists && hydratedBefore.byteLength > 0);
    assert.equal(hydratedBefore.generation, hydration.generation);
    assert.equal(hydratedBefore.byteLength, hydration.byteLength);
    assert.equal(hydratedBefore.sha256, hydration.sha256);
    assert.deepEqual(hydratedAfter, hydratedBefore);
    acceptanceSummary =
      `generation ${hydration.generation}, ${hydration.byteLength}-byte unchanged save, ` +
      `${report.input.completedActions} action(s), ` +
      `${report.presentation.presentCount} Present(s), ` +
      `${report.audio.playbacksStarted} playback(s), ${frameBytes.byteLength}-byte title frame`;
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
