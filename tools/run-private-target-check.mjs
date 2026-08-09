import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { loadCurrentCheckpoint } from "./current-checkpoint.mjs";
import {
  BROWSER_BGI_REPORT_SCHEMA,
  BROWSER_BGI_REPORT_SCHEMA_VERSION,
} from "./browser-bgi-report-contract.mjs";
import {
  assertValidPrivateTargetEvidence,
  finalizePrivateTargetEvidence,
  PRIVATE_TARGET_EVIDENCE_SCHEMA,
  PRIVATE_TARGET_EVIDENCE_VERSION,
  projectPrivateTargetExpectedBrowserEvidence,
} from "./private-target-evidence.mjs";
import {
  projectCapabilityCheckpointCompletion,
  projectWaitableTimerPacingRequests,
} from "./private-target-report-shape.mjs";
import {
  formatManifestRuntimeEndpointIdentity,
  formatRuntimeEndpointSymbol,
} from "./runtime-endpoint-symbol.mjs";
import { normalizeSemanticCall } from "../browser/runtime-worker/semantic-call.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const defaultOutput = path.join(root, "docs", "private-target-evidence.json");
const maximumSemanticReportBytes = 4 * 1024 * 1024;
const defaultBgi =
  process.platform === "win32" ? "V:\\softs\\\u30b5\u30af\u30e9\u30ce\u8a69\\BGI.exe" : null;
const protectedInputs = [
  "browser/",
  "crates/",
  "fixtures/",
  "tools/",
  "Cargo.lock",
  "Cargo.toml",
  "package-lock.json",
  "package.json",
  "rust-toolchain.toml",
  "docs/current-checkpoint.json",
];

function fail(message) {
  throw new Error(`private target gate failed: ${message}`);
}

function normalizeRepositoryPath(value) {
  const relative = path.relative(root, path.resolve(root, value)).replaceAll("\\", "/");
  if (relative === "" || relative === ".." || relative.startsWith("../")) {
    fail(`path is outside the repository: ${value}`);
  }
  return relative;
}

function parseArguments(arguments_) {
  let output = defaultOutput;
  const allowedDirtyPaths = [];
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--output" && index + 1 < arguments_.length) {
      output = path.resolve(root, arguments_[index + 1]);
      index += 1;
    } else if (argument === "--allow-dirty" && index + 1 < arguments_.length) {
      allowedDirtyPaths.push(normalizeRepositoryPath(arguments_[index + 1]));
      index += 1;
    } else {
      fail(`unsupported argument: ${argument}`);
    }
  }
  return {
    output,
    outputRelative: normalizeRepositoryPath(output),
    allowedDirtyPaths: [...new Set(allowedDirtyPaths)].sort(),
  };
}

function capture(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    cwd: root,
    encoding: options.encoding ?? "utf8",
    windowsHide: true,
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    fail(
      `${command} ${arguments_.join(" ")} exited ${result.status}\n${result.stdout ?? ""}${result.stderr ?? ""}`.trim(),
    );
  }
  return result.stdout;
}

function runInherited(command, arguments_) {
  const result = spawnSync(command, arguments_, {
    cwd: root,
    stdio: "inherit",
    windowsHide: true,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    fail(`${command} ${arguments_.join(" ")} exited ${result.status}`);
  }
}

function git(arguments_, options = {}) {
  return capture("git", arguments_, options);
}

function splitNullTerminated(value) {
  return value
    .split("\0")
    .filter((entry) => entry.length > 0)
    .map((entry) => entry.replaceAll("\\", "/"));
}

function dirtyPaths() {
  const tracked = splitNullTerminated(git(["diff", "--name-only", "-z", "HEAD", "--"]));
  const untracked = splitNullTerminated(
    git(["ls-files", "--others", "--exclude-standard", "-z"]),
  );
  return [...new Set([...tracked, ...untracked])].sort();
}

function isProtectedInput(relative) {
  return protectedInputs.some((entry) =>
    entry.endsWith("/") ? relative.startsWith(entry) : relative === entry,
  );
}

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function artifact(file) {
  const bytes = fs.readFileSync(file);
  return {
    path: path.relative(root, file).replaceAll("\\", "/"),
    bytes: bytes.byteLength,
    sha256: sha256(bytes),
  };
}

function executionEngineIdentity(profile, actualArtifact) {
  assert(Array.isArray(profile.artifacts), "execution engine profile has no artifact allowlist");
  const matched = profile.artifacts.find(
    ({ bytes, sha256: digest }) =>
      bytes === actualArtifact.bytes && digest === actualArtifact.sha256,
  );
  assert(matched !== undefined, "execution engine artifact is not in the pinned profile");
  return {
    schema: profile.schema,
    schemaVersion: profile.schemaVersion,
    engine: profile.engine,
    sourceRevision: profile.sourceRevision,
    cpuContextAbi: structuredClone(profile.cpuContextAbi),
    artifact: {
      bytes: matched.bytes,
      sha256: matched.sha256,
    },
  };
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function firstLine(command, arguments_ = []) {
  return capture(command, arguments_).trim().split(/\r?\n/u)[0];
}

function hex32(value) {
  assert(Number.isSafeInteger(value) && value >= 0 && value <= 0xffff_ffff);
  return `0x${value.toString(16).padStart(8, "0")}`;
}

function optionalHex32(value) {
  return value === undefined ? undefined : hex32(value);
}

function assertCapabilityCheckpoint(
  actual,
  expected,
  { requireWriteReceipt = false } = {},
) {
  assert(actual !== null, "capability checkpoint metadata is missing");
  assert.equal(actual.capability, expected.completion.capability);
  assert.equal(actual.implementationId, expected.completion.implementationId);
  if (expected.completion.requiredEntries !== undefined) {
    assert.equal(actual.requiredEntries, expected.completion.requiredEntries);
    assert.equal(actual.entryCount, expected.completion.entryCount);
    assert.equal(
      actual.lastEntrySequence,
      expected.completion.lastEntrySequence,
    );
    assert.equal(
      actual.lastEntryInstructionCount,
      expected.completion.lastEntryInstructionCount,
    );
    assert.equal(actual.dispatchMessageRelease.pendingCallback, true);
    assert.equal(
      actual.dispatchMessageRelease.sequence,
      actual.lastEntrySequence,
    );
    assert.equal(
      actual.dispatchMessageRelease.trapInstructionCount,
      actual.lastEntryInstructionCount,
    );
    assert.equal(actual.releaseCallbackEntry.sequence, actual.lastEntrySequence);
    assert.equal(
      actual.releaseCallbackEntry.instructionCount,
      actual.lastEntryInstructionCount,
    );
    assert.equal(actual.releaseCallbackEntry.depth, 1);
    assert.equal(actual.queueDepthAtDispatchEntry, 0);
    assert.equal(actual.dispatchMessageReleaseBytesPreserved, true);
    if (expected.completion.requiredCompletions !== undefined) {
      const postWindowTimerCallbackPeekContinuation =
        expected.completion.capability ===
        "user32-peek-message-a-post-window-timer-callback-continuation";
      const postWaitableTimerWindowCallbackCycle =
        postWindowTimerCallbackPeekContinuation ||
        expected.completion.capability ===
        "user32-window-timer-callback-post-waitable-timer-cycle";
      const postInputScanWaitableTimerCycle =
        postWaitableTimerWindowCallbackCycle ||
        expected.completion.capability ===
          "d3d9-post-input-scan-waitable-timer-cycle";
      const postInputScanTestCooperativeLevel =
        postInputScanWaitableTimerCycle ||
        expected.completion.capability ===
        "d3d9-test-cooperative-level-post-input-scan-continuation";
      const postIsIconicGetAsyncKeyStateScan =
        postInputScanTestCooperativeLevel ||
        expected.completion.capability ===
          "user32-get-async-key-state-post-is-iconic-clock-continuation";
      const postIsIconicTimeGetTime =
        postIsIconicGetAsyncKeyStateScan ||
        expected.completion.capability ===
        "winmm-time-get-time-post-is-iconic-continuation";
      const postReleaseIsIconic =
        postIsIconicTimeGetTime ||
        expected.completion.capability ===
        "user32-is-iconic-post-pointer-release-continuation";
      const postReleaseGetSystemMetricsFixedFrameHeight =
        postReleaseIsIconic ||
        expected.completion.capability ===
        "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation";
      const postReleaseGetSystemMetricsCaption =
        postReleaseGetSystemMetricsFixedFrameHeight ||
        expected.completion.capability ===
        "user32-get-system-metrics-caption-post-pointer-release-continuation";
      const postReleaseGetSystemMetrics =
        postReleaseGetSystemMetricsCaption ||
        expected.completion.capability ===
          "user32-get-system-metrics-post-pointer-release-continuation";
      const postReleaseGetCursorPos =
        postReleaseGetSystemMetrics ||
        expected.completion.capability ===
        "user32-get-cursor-pos-post-pointer-release-continuation";
      const postReleaseGetWindowRect =
        postReleaseGetCursorPos ||
        expected.completion.capability ===
        "user32-get-window-rect-post-pointer-release-continuation";
      const postReleaseTimeGetTimeRepeatBatch =
        postReleaseGetWindowRect ||
        expected.completion.capability ===
          "winmm-time-get-time-post-pointer-release-repeat-batch";
      const postReleaseTimeGetTime =
        postReleaseTimeGetTimeRepeatBatch ||
        expected.completion.capability ===
          "winmm-time-get-time-post-pointer-release-continuation";
      const postReleasePeek =
        postReleaseTimeGetTime ||
        expected.completion.capability ===
          "user32-peek-message-a-post-pointer-release-continuation";
      assert.equal(
        actual.requiredCompletions,
        expected.completion.requiredCompletions,
      );
      assert.equal(actual.completionCount, expected.completion.completionCount);
      assert.equal(
        actual.lastCompletionSequence,
        expected.completion.lastCompletionSequence,
      );
      assert.equal(
        actual.lastCompletionInstructionCount,
        expected.completion.lastCompletionInstructionCount,
      );
      if (postIsIconicGetAsyncKeyStateScan) {
        assert.equal(
          actual.firstCompletionSequence,
          postWindowTimerCallbackPeekContinuation
            ? 162_087
            : postWaitableTimerWindowCallbackCycle
            ? 162_083
            : postInputScanWaitableTimerCycle
            ? 162_081
            : postInputScanTestCooperativeLevel
              ? 162_080
              : 161_965,
        );
        assert.equal(
          actual.firstCompletionInstructionCount,
          postWindowTimerCallbackPeekContinuation
            ? 321_469_064
            : postWaitableTimerWindowCallbackCycle
            ? 321_468_397
            : postInputScanWaitableTimerCycle
            ? 321_468_343
            : postInputScanTestCooperativeLevel
              ? 321_468_292
              : 321_463_448,
        );
        assert.deepEqual(actual.scanVirtualKeys, [
          14, 15, 13, 16, 27, 38, 33, 40, 34, 37, 39, 49, 97, 50, 98,
          51, 99, 52, 100, 53, 101, 54, 102, 55, 103, 56, 104, 57, 105,
          48, 96, 9, 17, 1, 2, 4, 5, 6,
        ]);
        assert.deepEqual(actual.result, { action: "set", value: 0 });
        assert.equal(actual.lastError, 0);
        assert.equal(actual.guestWriteBytes, 0);
        assert.equal(
          actual.getAsyncKeyStatePostIsIconicClockActiveCallbackDepthAtCompletion,
          0,
        );
        assert.equal(actual.setFocusActiveCallbackDepthAtStop, 1);
        const first = actual.getAsyncKeyStatePostIsIconicClockFirst;
        const last = actual.getAsyncKeyStatePostIsIconicClockLast;
        assert(first !== undefined);
        assert(last !== undefined);
        for (const [sample, expectedSample] of [
          [
            first,
            {
              sequence: 161_965,
              occurrence: 386,
              trapInstructionCount: 321_463_448,
              virtualKey: 14,
              stackPointerBefore: 7_337_996,
              stackPointerAfter: 7_338_004,
            },
          ],
          [
            last,
            {
              sequence: 162_079,
              occurrence: 423,
              trapInstructionCount: 321_468_229,
              virtualKey: 6,
              stackPointerBefore: 7_338_184,
              stackPointerAfter: 7_338_192,
            },
          ],
        ]) {
          assert.equal(sample.sequence, expectedSample.sequence);
          assert.equal(sample.occurrence, expectedSample.occurrence);
          assert.equal(
            sample.trapInstructionCount,
            expectedSample.trapInstructionCount,
          );
          assert.equal(sample.runtimeThreadId, 8_192);
          assert.equal(sample.module.toLowerCase(), "user32.dll");
          assert.equal(sample.symbol, "GetAsyncKeyState");
          assert.equal(
            sample.implementationId,
            "user32.get-async-key-state.v1",
          );
          assert.deepEqual(sample.arguments, [expectedSample.virtualKey]);
          assert.deepEqual(sample.result, { action: "set", value: 0 });
          assert.equal(sample.returnAddress, 268_862_469);
          assert.equal(
            sample.stackPointerBefore,
            expectedSample.stackPointerBefore,
          );
          assert.equal(sample.stackPointerAfter, expectedSample.stackPointerAfter);
          assert.equal(sample.lastError, 0);
          assert.equal(sample.guestWrites.length, 0);
        }
      }
      if (postInputScanTestCooperativeLevel) {
        assert.equal(actual.prerequisiteScanCompletionSequence, 162_079);
        assert.equal(
          actual.prerequisiteScanCompletionInstructionCount,
          321_468_229,
        );
        assert.equal(
          actual.d3d9TestCooperativeLevelPostInputScanActiveCallbackDepthAtCompletion,
          0,
        );
        const call = actual.d3d9TestCooperativeLevelPostInputScan;
        assert(call !== undefined);
        assert.equal(call.sequence, 162_080);
        assert.equal(call.trapInstructionCount, 321_468_292);
        assert.equal(call.runtimeThreadId, 8_192);
        assert.equal(call.module.toLowerCase(), "<com>");
        assert.equal(call.symbol, "IDirect3DDevice9.TestCooperativeLevel");
        assert.equal(call.occurrence, 11);
        assert.equal(call.interface, "IDirect3DDevice9");
        assert.equal(call.vtableSlot, 3);
        assert.deepEqual(call.arguments, [0x0101_0004]);
        assert.deepEqual(call.result, { kind: "h-result", value: 0 });
        assert.equal(call.returnAddress, 268_807_629);
        assert.equal(call.stackPointerBefore, 7_338_228);
        assert.equal(call.stackPointerAfter, 7_338_236);
        assert.equal(call.referenceCount, 5);
        assert.equal(call.objectRetired, false);
        assert.deepEqual(call.guestWrites, []);
        assert.equal(actual.cumulativeCompletions, 9);
      }
      if (postInputScanWaitableTimerCycle) {
        assert.equal(actual.prerequisiteTestCooperativeLevelSequence, 162_080);
        assert.equal(
          actual.prerequisiteTestCooperativeLevelInstructionCount,
          321_468_292,
        );
        assert.equal(actual.timerArmActiveCallbackDepthAtCompletion, 0);
        assert.equal(actual.timerWaitActiveCallbackDepthAtCompletion, 0);
        const arm = actual.timerArm;
        const wait = actual.timerWait;
        assert(arm !== undefined);
        assert(wait !== undefined);
        assert.equal(arm.sequence, 162_081);
        assert.equal(arm.occurrence, 11_476);
        assert.equal(arm.trapInstructionCount, 321_468_343);
        assert.equal(arm.runtimeThreadId, 8_192);
        assert.equal(arm.module.toLowerCase(), "kernel32.dll");
        assert.equal(arm.symbol, "SetWaitableTimer");
        assert.equal(
          arm.implementationId,
          "kernel32.set-waitable-timer.relative-one-shot.v1",
        );
        assert.deepEqual(arm.arguments, [1_459_683_329, 7_338_224, 0, 0, 0, 0]);
        assert.deepEqual(arm.result, { action: "set", value: 1 });
        assert.equal(arm.returnAddress, 268_764_679);
        assert.equal(arm.stackPointerBefore, 7_338_196);
        assert.equal(arm.stackPointerAfter, 7_338_224);
        assert.equal(arm.lastError, 0);
        assert.deepEqual(arm.guestWrites, []);
        assert.equal(arm.waitableTimer.handle, 1_459_683_329);
        assert.equal(arm.waitableTimer.timer.id, 1);
        assert.equal(arm.waitableTimer.timer.manualReset, true);
        assert.deepEqual(arm.waitableTimer.timer.arm, {
          relativeDueTime100ns: 20_000,
          armedAtTime100ns: 57_470_000,
          deadlineTime100ns: 57_490_000,
          periodMilliseconds: 0,
          generation: 9,
        });
        assert.equal(arm.waitableTimer.objectRecord.id, 20);
        assert.deepEqual(arm.waitableTimer.objectRecord.waitState, {
          state: "nonsignaled",
        });
        assert.equal(wait.sequence, 162_082);
        assert.equal(wait.occurrence, 11_475);
        assert.equal(wait.trapInstructionCount, 321_468_349);
        assert.equal(wait.runtimeThreadId, 8_192);
        assert.equal(wait.module.toLowerCase(), "kernel32.dll");
        assert.equal(wait.symbol, "WaitForSingleObjectEx");
        assert.equal(
          wait.implementationId,
          "kernel32.wait-for-single-object-ex.waitable-timer.v1",
        );
        assert.deepEqual(wait.arguments, [1_459_683_329, 10, 1]);
        assert.deepEqual(wait.result, { action: "set", value: 0 });
        assert.equal(wait.returnAddress, 268_764_693);
        assert.equal(wait.stackPointerBefore, 7_338_208);
        assert.equal(wait.stackPointerAfter, 7_338_224);
        assert.equal(wait.lastError, 0);
        assert.deepEqual(wait.guestWrites, []);
        assert.equal(wait.waitableTimer.handle, 1_459_683_329);
        assert.equal(wait.waitableTimer.timer.id, 1);
        assert.equal(wait.waitableTimer.timer.manualReset, true);
        assert.deepEqual(wait.waitableTimer.timer.lastExpiration, {
          armGeneration: 9,
          deadlineTime100ns: 57_490_000,
        });
        assert.equal(wait.waitableTimer.objectRecord.id, 20);
        assert.deepEqual(wait.waitableTimer.objectRecord.waitState, {
          state: "signaled",
          exit_code: 0,
        });
        assert.deepEqual(wait.waitableTimer.timerClock, {
          currentTime100ns: 57_490_000,
          generation: 11_474,
        });
        assert.equal(wait.waitableTimer.nextWaitRegistrationId, 11_475);
        assert.equal(actual.cumulativeTimerArmCompletions, 11_476);
        assert.equal(actual.cumulativeTimerWaitCompletions, 11_475);
      }
      if (postWaitableTimerWindowCallbackCycle) {
        assert.equal(actual.requiredDeliveries, 1);
        assert.equal(actual.deliveryCount, 3);
        assert.equal(actual.prerequisiteTimerWaitSequence, 162_082);
        assert.equal(
          actual.prerequisiteTimerWaitInstructionCount,
          321_468_349,
        );
        const peek = actual.windowTimerPeek;
        assert(peek !== undefined);
        assert.equal(peek.sequence, 162_083);
        assert.equal(peek.occurrence, 13);
        assert.equal(peek.trapInstructionCount, 321_468_397);
        assert.equal(peek.runtimeThreadId, 8_192);
        assert.equal(peek.module.toLowerCase(), "user32.dll");
        assert.equal(peek.symbol, "PeekMessageA");
        assert.equal(
          peek.implementationId,
          "user32.peek-message-a.current-thread.v1",
        );
        assert.deepEqual(peek.arguments, [7_338_216, 0, 0, 0, 1]);
        assert.deepEqual(peek.result, { action: "set", value: 1 });
        assert.equal(peek.returnAddress, 269_039_499);
        assert.equal(peek.stackPointerBefore, 7_338_184);
        assert.equal(peek.stackPointerAfter, 7_338_208);
        assert.equal(peek.lastError, 0);
        assert.equal(peek.guestWrites.length, 1);
        assert.equal(peek.guestWrites[0].address, 7_338_216);
        assert.deepEqual(peek.guestWrites[0].bytes, [
          1, 0, 1, 72, 19, 1, 0, 0, 1, 0, 0, 0, 0, 150, 9, 16, 77,
          243, 54, 0, 192, 3, 0, 0, 28, 2, 0, 0,
        ]);
        assert.equal(peek.windowTimer.deliveryCount, 3);
        assert.equal(peek.windowTimer.callbackAddress, 269_063_680);
        assert.equal(peek.windowTimer.armedAtTime100ns, 57_490_000);
        assert.equal(peek.windowTimer.nextDeadlineTime100ns, 57_690_000);
        assert.deepEqual(peek.windowTimer.lastDelivery, {
          registrationGeneration: 1,
          deliveryCount: 3,
          scheduledDeadlineTime100ns: 57_435_000,
          deliveredAtTime100ns: 57_490_000,
        });
        const translate = actual.windowTimerTranslate;
        assert(translate !== undefined);
        assert.equal(translate.sequence, 162_084);
        assert.equal(translate.occurrence, 5);
        assert.equal(translate.trapInstructionCount, 321_468_407);
        assert.deepEqual(translate.result, { action: "set", value: 0 });
        assert.equal(translate.stackPointerBefore, 7_338_196);
        assert.equal(translate.stackPointerAfter, 7_338_204);
        const dispatch = actual.windowTimerDispatch;
        assert(dispatch !== undefined);
        assert.equal(dispatch.sequence, 162_085);
        assert.equal(dispatch.occurrence, 5);
        assert.equal(dispatch.trapInstructionCount, 321_468_411);
        assert.equal(dispatch.runtimeThreadId, 8_192);
        assert.equal(dispatch.module.toLowerCase(), "user32.dll");
        assert.equal(dispatch.symbol, "DispatchMessageA");
        assert.equal(
          dispatch.implementationId,
          "user32.dispatch-message-a.window-timer-pointer-callback.v2",
        );
        assert.deepEqual(dispatch.result, { action: "set", value: 0 });
        assert.equal(dispatch.returnAddress, 269_039_532);
        assert.equal(dispatch.stackPointerBefore, 7_338_196);
        assert.equal(dispatch.stackPointerAfter, 7_338_204);
        assert.deepEqual(dispatch.callbackArguments, [
          1_208_025_089, 275, 1, 3_601_229,
        ]);
        assert.deepEqual(dispatch.callback, {
          frameId: 35,
          target: 269_063_680,
          allocationStart: 7_338_176,
          stackPointer: 7_338_176,
          scratchAddress: 0,
          scratchBytes: 0,
          frameBytes: 20,
          returnSentinel: 2_130_771_968,
          returnInstructionCount: 321_469_055,
          result: 64,
        });
        assert.equal(actual.windowTimerCallbackNestedCompletionCount, 1);
        assert.equal(actual.windowTimerCallbackActiveDepthAtCompletion, 1);
        assert.equal(actual.windowTimerCallbackNestedCalls.length, 1);
        const nested = actual.windowTimerCallbackNestedCalls[0];
        assert.equal(nested.sequence, 162_086);
        assert.equal(nested.occurrence, 1_007);
        assert.equal(nested.trapInstructionCount, 321_468_432);
        assert.equal(nested.runtimeThreadId, 8_192);
        assert.equal(nested.module.toLowerCase(), "kernel32.dll");
        assert.equal(nested.symbol, "GetTickCount");
        assert.equal(nested.implementationId, "kernel32.get-tick-count.v1");
        assert.deepEqual(nested.arguments, []);
        assert.deepEqual(nested.result, { action: "set", value: 3_601_229 });
        assert.equal(nested.returnAddress, 269_063_479);
        assert.equal(nested.stackPointerBefore, 7_338_128);
        assert.equal(nested.stackPointerAfter, 7_338_132);
        assert.equal(nested.lastError, 0);
        assert.deepEqual(nested.guestWrites, []);
        assert.deepEqual(nested.tickCount, {
          returnedTickCount: 3_601_229,
          nextTickCount: 3_601_230,
          incrementPerRead: 1,
        });
      }
      if (postWindowTimerCallbackPeekContinuation) {
        assert.equal(actual.prerequisiteWindowTimerDispatchSequence, 162_085);
        assert.equal(
          actual.prerequisiteWindowTimerDispatchInstructionCount,
          321_469_055,
        );
        assert.equal(
          actual.peekMessagePostWindowTimerCallbackActiveCallbackDepthAtCompletion,
          0,
        );
        assert.equal(actual.postWindowTimerCallbackQueueDepthBefore, 0);
        assert.equal(actual.postWindowTimerCallbackQueueDepthAfter, 0);
        assert.equal(actual.postWindowTimerCallbackMessageBytesPreserved, true);
        const peek = actual.peekMessagePostWindowTimerCallback;
        assert.equal(peek.sequence, 162_087);
        assert.equal(peek.occurrence, 14);
        assert.equal(peek.trapInstructionCount, 321_469_064);
        assert.equal(peek.runtimeThreadId, 8_192);
        assert.equal(peek.module.toLowerCase(), "user32.dll");
        assert.equal(peek.symbol, "PeekMessageA");
        assert.equal(
          peek.implementationId,
          "user32.peek-message-a.current-thread.v1",
        );
        assert.deepEqual(peek.arguments, [7_338_216, 0, 0, 0, 1]);
        assert.deepEqual(peek.result, { action: "set", value: 0 });
        assert.equal(peek.returnAddress, 269_039_547);
        assert.equal(peek.stackPointerBefore, 7_338_180);
        assert.equal(peek.stackPointerAfter, 7_338_204);
        assert.equal(peek.lastError, 0);
        assert.deepEqual(peek.guestWrites, []);
      }
      if (postReleasePeek) {
        assert.equal(
          actual.peekMessagePostRelease.sequence,
          161_953,
        );
        assert.equal(
          actual.peekMessagePostRelease.trapInstructionCount,
          321_462_521,
        );
        assert.equal(
          actual.peekMessagePostRelease.implementationId,
          "user32.peek-message-a.current-thread.v1",
        );
        assert.equal(actual.peekMessagePostRelease.result.value, 0);
        assert.equal(actual.postReleaseQueueDepthBefore, 0);
        assert.equal(actual.postReleaseQueueDepthAfter, 0);
        assert.equal(actual.postReleaseMessageBytesPreserved, true);
      }
      if (postReleaseTimeGetTime) {
        const finalTimeGetTime = postReleaseTimeGetTimeRepeatBatch
          ? actual.timeGetTimePostReleaseCompletions.at(-1)
          : actual.timeGetTimePostRelease;
        assert(finalTimeGetTime !== undefined);
        assert.equal(
          finalTimeGetTime.sequence,
          postReleaseGetWindowRect ? 161_957 : actual.lastCompletionSequence,
        );
        assert.equal(
          finalTimeGetTime.trapInstructionCount,
          postReleaseGetWindowRect
            ? 321_463_010
            : actual.lastCompletionInstructionCount,
        );
        assert.equal(
          finalTimeGetTime.implementationId,
          "winmm.time-get-time.v1",
        );
        assert.equal(
          finalTimeGetTime.result.value,
          postReleaseTimeGetTimeRepeatBatch ? 3_601_227 : 3_601_224,
        );
        assert.equal(
          finalTimeGetTime.tickCount.returnedTickCount,
          postReleaseTimeGetTimeRepeatBatch ? 3_601_227 : 3_601_224,
        );
        assert.equal(
          finalTimeGetTime.tickCount.nextTickCount,
          postReleaseTimeGetTimeRepeatBatch ? 3_601_228 : 3_601_225,
        );
        assert.equal(
          finalTimeGetTime.tickCount.incrementPerRead,
          1,
        );
        if (postReleaseTimeGetTimeRepeatBatch) {
          assert.equal(actual.maximumRepeatCompletions, 16);
          assert.equal(actual.timeGetTimePostReleaseCompletions.length, 4);
          assert.deepEqual(
            actual.timeGetTimePostReleaseCompletions.map((sample) => ({
              sequence: sample.sequence,
              occurrence: sample.occurrence,
              instructionCount: sample.trapInstructionCount,
              stackPointerBefore: sample.stackPointerBefore,
              returnedTickCount: sample.tickCount.returnedTickCount,
              nextTickCount: sample.tickCount.nextTickCount,
            })),
            [
              { sequence: 161_954, occurrence: 219, instructionCount: 321_462_555, stackPointerBefore: 7_338_244, returnedTickCount: 3_601_224, nextTickCount: 3_601_225 },
              { sequence: 161_955, occurrence: 220, instructionCount: 321_462_721, stackPointerBefore: 7_337_876, returnedTickCount: 3_601_225, nextTickCount: 3_601_226 },
              { sequence: 161_956, occurrence: 221, instructionCount: 321_462_906, stackPointerBefore: 7_338_216, returnedTickCount: 3_601_226, nextTickCount: 3_601_227 },
              { sequence: 161_957, occurrence: 222, instructionCount: 321_463_010, stackPointerBefore: 7_338_236, returnedTickCount: 3_601_227, nextTickCount: 3_601_228 },
            ],
          );
          assert(
            actual.timeGetTimePostReleaseCompletions.every(
              (sample) =>
                sample.runtimeThreadId === 8_192 &&
                sample.module.toLowerCase() === "winmm.dll" &&
                sample.symbol === "timeGetTime" &&
                sample.implementationId === "winmm.time-get-time.v1" &&
                sample.arguments.length === 0 &&
                sample.result.action === "set" &&
                sample.returnAddress === 269_033_151 &&
                sample.stackPointerAfter === sample.stackPointerBefore + 4 &&
                sample.lastError === 0 &&
                sample.guestWrites.length === 0 &&
                sample.tickCount.incrementPerRead === 1,
            ),
          );
          assert.equal(actual.timeGetTimePostRelease.sequence, 161_954);
          assert.equal(
            actual.timeGetTimePostRelease.tickCount.returnedTickCount,
            3_601_224,
          );
        }
      }
      if (postReleaseGetWindowRect) {
        const getWindowRect = actual.getWindowRectPostRelease;
        assert(getWindowRect !== undefined);
        assert.equal(getWindowRect.sequence, 161_958);
        assert.equal(
          getWindowRect.trapInstructionCount,
          321_463_115,
        );
        assert.equal(getWindowRect.runtimeThreadId, 8_192);
        assert.equal(getWindowRect.module.toLowerCase(), "user32.dll");
        assert.equal(getWindowRect.symbol, "GetWindowRect");
        assert.equal(getWindowRect.occurrence, 17);
        assert.equal(
          getWindowRect.implementationId,
          "user32.get-window-rect.v1",
        );
        assert.deepEqual(getWindowRect.arguments, [1_208_025_089, 7_338_172]);
        assert.deepEqual(getWindowRect.result, { action: "set", value: 1 });
        assert.equal(getWindowRect.returnAddress, 268_993_880);
        assert.equal(getWindowRect.stackPointerBefore, 7_338_140);
        assert.equal(getWindowRect.stackPointerAfter, 7_338_152);
        assert.equal(getWindowRect.lastError, 0);
        assert.equal(getWindowRect.guestWrites.length, 1);
        assert.equal(getWindowRect.guestWrites[0].address, 7_338_172);
        assert.deepEqual(getWindowRect.guestWrites[0].bytes, [
          61, 1, 0, 0, 154, 0, 0, 0, 67, 6, 0, 0, 135, 3, 0, 0,
        ]);
        if (requireWriteReceipt) {
          assert.equal(getWindowRect.guestWrites[0].result.bytesWritten, 16);
          assert.equal(getWindowRect.guestWrites[0].result.dirtyPages, 1);
          assert.equal(
            getWindowRect.guestWrites[0].result.invalidation.dataTlb,
            false,
          );
          assert.equal(
            getWindowRect.guestWrites[0].result.invalidation.instructionFetch,
            false,
          );
          assert.equal(
            getWindowRect.guestWrites[0].result.invalidation.generatedCode,
            false,
          );
        } else {
          assert.equal(getWindowRect.guestWrites[0].result, undefined);
        }
      }
      if (postReleaseGetCursorPos) {
        const getCursorPos = actual.getCursorPosPostRelease;
        assert(getCursorPos !== undefined);
        assert.equal(getCursorPos.sequence, 161_959);
        assert.equal(
          getCursorPos.trapInstructionCount,
          321_463_119,
        );
        assert.equal(getCursorPos.runtimeThreadId, 8_192);
        assert.equal(getCursorPos.module.toLowerCase(), "user32.dll");
        assert.equal(getCursorPos.symbol, "GetCursorPos");
        assert.equal(getCursorPos.occurrence, 17);
        assert.equal(
          getCursorPos.implementationId,
          "user32.get-cursor-pos.v1",
        );
        assert.deepEqual(getCursorPos.arguments, [7_338_164]);
        assert.deepEqual(getCursorPos.result, { action: "set", value: 1 });
        assert.equal(getCursorPos.returnAddress, 268_993_891);
        assert.equal(getCursorPos.stackPointerBefore, 7_338_144);
        assert.equal(getCursorPos.stackPointerAfter, 7_338_152);
        assert.equal(getCursorPos.lastError, 0);
        assert.equal(
          actual.getCursorPosPostReleaseActiveCallbackDepthAtCompletion,
          0,
        );
        assert.equal(getCursorPos.guestWrites.length, 1);
        assert.equal(getCursorPos.guestWrites[0].address, 7_338_164);
        assert.deepEqual(getCursorPos.guestWrites[0].bytes, [
          192, 3, 0, 0, 28, 2, 0, 0,
        ]);
        if (requireWriteReceipt) {
          assert.equal(getCursorPos.guestWrites[0].result.bytesWritten, 8);
          assert.equal(getCursorPos.guestWrites[0].result.dirtyPages, 1);
          assert.equal(
            getCursorPos.guestWrites[0].result.invalidation.dataTlb,
            false,
          );
          assert.equal(
            getCursorPos.guestWrites[0].result.invalidation.instructionFetch,
            false,
          );
          assert.equal(
            getCursorPos.guestWrites[0].result.invalidation.generatedCode,
            false,
          );
        } else {
          assert.equal(getCursorPos.guestWrites[0].result, undefined);
        }
        if (postReleaseGetSystemMetrics) {
          const getSystemMetrics = actual.getSystemMetricsPostRelease;
          assert(getSystemMetrics !== undefined);
          assert.equal(getSystemMetrics.sequence, 161_960);
          assert.equal(
            getSystemMetrics.trapInstructionCount,
            321_463_128,
          );
          assert.equal(getSystemMetrics.runtimeThreadId, 8_192);
          assert.equal(getSystemMetrics.module.toLowerCase(), "user32.dll");
          assert.equal(getSystemMetrics.symbol, "GetSystemMetrics");
          assert.equal(getSystemMetrics.occurrence, 66);
          assert.equal(
            getSystemMetrics.implementationId,
            "user32.get-system-metrics.v1",
          );
          assert.deepEqual(getSystemMetrics.arguments, [7]);
          assert.deepEqual(getSystemMetrics.result, {
            action: "set",
            value: 3,
          });
          assert.equal(getSystemMetrics.returnAddress, 268_993_910);
          assert.equal(getSystemMetrics.stackPointerBefore, 7_338_144);
          assert.equal(getSystemMetrics.stackPointerAfter, 7_338_152);
          assert.equal(getSystemMetrics.lastError, 0);
          assert.equal(getSystemMetrics.guestWrites.length, 0);
          assert.equal(
            actual.getSystemMetricsPostReleaseActiveCallbackDepthAtCompletion,
            0,
          );
          if (postReleaseGetSystemMetricsCaption) {
            const getSystemMetricsCaption =
              actual.getSystemMetricsCaptionPostRelease;
            assert(getSystemMetricsCaption !== undefined);
            assert.equal(getSystemMetricsCaption.sequence, 161_961);
            assert.equal(
              getSystemMetricsCaption.trapInstructionCount,
              321_463_133,
            );
            assert.equal(getSystemMetricsCaption.runtimeThreadId, 8_192);
            assert.equal(
              getSystemMetricsCaption.module.toLowerCase(),
              "user32.dll",
            );
            assert.equal(getSystemMetricsCaption.symbol, "GetSystemMetrics");
            assert.equal(getSystemMetricsCaption.occurrence, 67);
            assert.equal(
              getSystemMetricsCaption.implementationId,
              "user32.get-system-metrics.v1",
            );
            assert.deepEqual(getSystemMetricsCaption.arguments, [4]);
            assert.deepEqual(getSystemMetricsCaption.result, {
              action: "set",
              value: 23,
            });
            assert.equal(getSystemMetricsCaption.returnAddress, 268_993_922);
            assert.equal(getSystemMetricsCaption.stackPointerBefore, 7_338_144);
            assert.equal(getSystemMetricsCaption.stackPointerAfter, 7_338_152);
            assert.equal(getSystemMetricsCaption.lastError, 0);
            assert.equal(getSystemMetricsCaption.guestWrites.length, 0);
            assert.equal(
              actual
                .getSystemMetricsCaptionPostReleaseActiveCallbackDepthAtCompletion,
              0,
            );
            if (postReleaseGetSystemMetricsFixedFrameHeight) {
              const getSystemMetricsFixedFrameHeight =
                actual.getSystemMetricsFixedFrameHeightPostRelease;
              assert(getSystemMetricsFixedFrameHeight !== undefined);
              assert.equal(
                getSystemMetricsFixedFrameHeight.sequence,
                161_962,
              );
              assert.equal(
                getSystemMetricsFixedFrameHeight.trapInstructionCount,
                321_463_139,
              );
              assert.equal(
                getSystemMetricsFixedFrameHeight.runtimeThreadId,
                8_192,
              );
              assert.equal(
                getSystemMetricsFixedFrameHeight.module.toLowerCase(),
                "user32.dll",
              );
              assert.equal(
                getSystemMetricsFixedFrameHeight.symbol,
                "GetSystemMetrics",
              );
              assert.equal(getSystemMetricsFixedFrameHeight.occurrence, 68);
              assert.equal(
                getSystemMetricsFixedFrameHeight.implementationId,
                "user32.get-system-metrics.v1",
              );
              assert.deepEqual(getSystemMetricsFixedFrameHeight.arguments, [8]);
              assert.deepEqual(getSystemMetricsFixedFrameHeight.result, {
                action: "set",
                value: 3,
              });
              assert.equal(
                getSystemMetricsFixedFrameHeight.returnAddress,
                268_993_936,
              );
              assert.equal(
                getSystemMetricsFixedFrameHeight.stackPointerBefore,
                7_338_144,
              );
              assert.equal(
                getSystemMetricsFixedFrameHeight.stackPointerAfter,
                7_338_152,
              );
              assert.equal(getSystemMetricsFixedFrameHeight.lastError, 0);
              assert.equal(getSystemMetricsFixedFrameHeight.guestWrites.length, 0);
              assert.equal(
                actual
                  .getSystemMetricsFixedFrameHeightPostReleaseActiveCallbackDepthAtCompletion,
                0,
              );
              if (postReleaseIsIconic) {
                const isIconic = actual.isIconicPostRelease;
                assert(isIconic !== undefined);
                assert.equal(
                  isIconic.sequence,
                  postIsIconicTimeGetTime
                    ? 161_963
                    : actual.lastCompletionSequence,
                );
                assert.equal(
                  isIconic.trapInstructionCount,
                  postIsIconicTimeGetTime
                    ? 321_463_282
                    : actual.lastCompletionInstructionCount,
                );
                assert.equal(isIconic.runtimeThreadId, 8_192);
                assert.equal(isIconic.module.toLowerCase(), "user32.dll");
                assert.equal(isIconic.symbol, "IsIconic");
                assert.equal(isIconic.occurrence, 8);
                assert.equal(
                  isIconic.implementationId,
                  "user32.is-iconic.logical-window-style.v1",
                );
                assert.deepEqual(isIconic.arguments, [0x4801_0001]);
                assert.deepEqual(isIconic.result, {
                  action: "set",
                  value: 0,
                });
                assert.equal(isIconic.returnAddress, 268_995_012);
                assert.equal(isIconic.stackPointerBefore, 7_338_200);
                assert.equal(isIconic.stackPointerAfter, 7_338_208);
                assert.equal(isIconic.lastError, 0);
                assert.equal(isIconic.guestWrites.length, 0);
                assert.equal(
                  actual.isIconicPostReleaseActiveCallbackDepthAtCompletion,
                  0,
                );
              }
              if (postIsIconicTimeGetTime) {
                const timeGetTime = actual.timeGetTimePostIsIconic;
                assert(timeGetTime !== undefined);
                assert.equal(
                  timeGetTime.sequence,
                  postIsIconicGetAsyncKeyStateScan
                    ? 161_964
                    : actual.lastCompletionSequence,
                );
                assert.equal(
                  timeGetTime.trapInstructionCount,
                  postIsIconicGetAsyncKeyStateScan
                    ? 321_463_291
                    : actual.lastCompletionInstructionCount,
                );
                assert.equal(timeGetTime.runtimeThreadId, 8_192);
                assert.equal(timeGetTime.module.toLowerCase(), "winmm.dll");
                assert.equal(timeGetTime.symbol, "timeGetTime");
                assert.equal(timeGetTime.occurrence, 223);
                assert.equal(
                  timeGetTime.implementationId,
                  "winmm.time-get-time.v1",
                );
                assert.deepEqual(timeGetTime.arguments, []);
                assert.deepEqual(timeGetTime.result, {
                  action: "set",
                  value: 3_601_228,
                });
                assert.equal(timeGetTime.returnAddress, 269_033_151);
                assert.equal(timeGetTime.stackPointerBefore, 7_338_200);
                assert.equal(timeGetTime.stackPointerAfter, 7_338_204);
                assert.equal(timeGetTime.lastError, 0);
                assert.equal(timeGetTime.guestWrites.length, 0);
                assert.deepEqual(timeGetTime.tickCount, {
                  returnedTickCount: 3_601_228,
                  nextTickCount: 3_601_229,
                  incrementPerRead: 1,
                });
                assert.equal(
                  actual.timeGetTimePostIsIconicActiveCallbackDepthAtCompletion,
                  0,
                );
              }
            }
          }
        }
      } else if (!postReleasePeek) {
        assert.equal(
          actual.defWindowProcARelease.sequence,
          actual.lastCompletionSequence,
        );
        assert.equal(
          actual.defWindowProcARelease.trapInstructionCount,
          actual.lastCompletionInstructionCount,
        );
      }
      assert.equal(
        actual.defWindowProcARelease.implementationId,
        "user32.def-window-proc-a.v1",
      );
      assert.equal(actual.defWindowProcARelease.result.value, 0);
      assert.equal(actual.releaseCallbackReturn.action, "return");
      assert.equal(
        actual.releaseCallbackReturn.frameId,
        actual.releaseCallbackEntry.frameId,
      );
      assert.equal(actual.releaseCallbackReturn.result, 0);
      assert.equal(actual.dispatchMessageReleaseResult, 0);
      assert.equal(actual.activeCallbackDepthAtStop, 0);
    } else {
      assert.equal(actual.activeCallbackDepthAtStop, 1);
    }
  } else if (expected.completion.requiredPointerMessages !== undefined) {
    assert.equal(
      actual.requiredPointerMessages,
      expected.completion.requiredPointerMessages,
    );
    assert.equal(
      actual.pointerMessageDeliveryCount,
      expected.completion.pointerMessageDeliveryCount,
    );
    assert.equal(actual.dispatchEntry.sequence, expected.completion.dispatchSequence);
    assert.equal(
      actual.dispatchEntry.trapInstructionCount,
      expected.completion.dispatchInstructionCount,
    );
    assert.equal(
      actual.releaseMessageQueued,
      expected.completion.releaseMessageQueued,
    );
    assert.equal(
      actual.activeCallbackDepthAtStop,
      expected.completion.activeCallbackDepthAtStop,
    );
  } else {
    const projected = projectCapabilityCheckpointCompletion(actual);
    assert.equal(projected.requiredCompletions, expected.completion.requiredCompletions);
    assert.equal(projected.completionCount, expected.completion.completionCount);
    assert.equal(
      projected.lastCompletionSequence,
      expected.completion.lastCompletionSequence,
    );
    assert.equal(
      projected.lastCompletionInstructionCount,
      expected.completion.lastCompletionInstructionCount,
    );
  }
  assert.equal(actual.instructionCount, expected.instructionCount);
  assert.equal(actual.nextEndpoint.id, expected.nextEndpoint.endpointId);
  assert.equal(hex32(actual.nextEndpoint.address), expected.nextEndpoint.endpointAddress);
  assert.equal(
    `${actual.nextEndpoint.module}!${formatRuntimeEndpointSymbol(
      actual.nextEndpoint,
    )}`.toLowerCase(),
    formatManifestRuntimeEndpointIdentity(expected.nextEndpoint).toLowerCase(),
  );
  if (expected.completion.roundtrips !== undefined) {
    assert.deepEqual(actual.roundtrips, expected.completion.roundtrips);
  }
  if (expected.completion.threadSelections !== undefined) {
    assert.deepEqual(actual.threadSelections, expected.completion.threadSelections);
  }
  for (const field of [
    "criticalSectionAddress",
    "releaseThreadId",
    "acquisitionThreadId",
    "lastError",
  ]) {
    if (expected.completion[field] !== undefined) {
      assert.equal(actual[field], expected.completion[field]);
    }
  }
}

function executionStateEvidence(report) {
  const context = report.watchdogContext;
  return {
    finalState: report.executionState,
    transitionCount: report.executionTransitionCount,
    lastTransitionTo: report.lastExecutionTransition?.to ?? null,
    pendingHostRequestRecords: report.pendingHostRequestRecords,
    watchdogContext:
      context === null
        ? null
        : {
            threadId: context.threadId,
            requestId: context.requestId,
            lastTransition:
              context.lastTransition === null
                ? null
                : {
                    from: context.lastTransition.from,
                    to: context.lastTransition.to,
                    cause: context.lastTransition.cause,
                  },
          },
  };
}

function assertExecutionStateReport(report, scenario) {
  assert.equal(report.executionState, "stopped");
  assert(report.executionTransitionCount > 0, "execution state machine did not transition");
  assert.equal(report.lastExecutionTransition?.to, "stopped");
  assert.equal(report.pendingHostRequestRecords, 0);
  if (scenario === "watchdog") {
    assert(report.watchdogContext !== null, "watchdog context is missing");
    assert(Number.isSafeInteger(report.watchdogContext.threadId));
    assert(report.watchdogContext.threadId > 0);
    assert(report.watchdogContext.lastTransition !== null, "watchdog transition is missing");
  } else {
    assert.equal(report.watchdogContext, null, "non-watchdog report retained watchdog context");
  }
}

function isScheduledSharedClockNodeReport(report) {
  return (
    report?.schema === "vine.v86-bgi-scheduled-shared-clock-primary" ||
    report?.schema === "vine.v86-bgi-find-file-a-primary" ||
    report?.schema === "vine.v86-bgi-delete-file-a-primary" ||
    report?.schema ===
      "vine.v86-bgi-private-heap-ordinary-segment-growth-primary" ||
    report?.schema === "vine.v86-bgi-create-font-a-224x112-primary" ||
    report?.schema === "vine.v86-bgi-create-dib-section-448x336-primary" ||
    report?.schema === "vine.v86-bgi-outline-text-metrics-224x112-primary" ||
    report?.schema === "vine.v86-bgi-get-cursor-info-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-update-texture-partial-dirty-region-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-begin-scene-post-partial-dirty-region-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-set-sampler-state-post-begin-scene-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-set-texture-stage-state-post-sampler-state-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-set-texture-post-texture-stage-state-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-set-fvf-post-texture-binding-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-set-stream-source-post-fvf-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-draw-primitive-post-stream-source-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-end-scene-post-draw-primitive-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-get-raster-status-post-end-scene-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-present-post-raster-status-primary" ||
    report?.schema ===
      "vine.v86-bgi-winmm-time-get-time-post-present-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-test-cooperative-level-post-time-get-time-primary" ||
    report?.schema ===
      "vine.v86-bgi-kernel32-set-waitable-timer-post-test-cooperative-level-primary" ||
    report?.schema ===
      "vine.v86-bgi-kernel32-wait-for-single-object-ex-post-test-cooperative-level-timer-arm-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-peek-message-a-post-test-cooperative-level-timer-wait-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-translate-message-post-test-cooperative-level-timer-peek-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-dispatch-message-a-post-test-cooperative-level-timer-translate-primary" ||
    report?.schema ===
      "vine.v86-bgi-kernel32-get-tick-count-post-test-cooperative-level-timer-dispatch-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-peek-message-a-post-test-cooperative-level-timer-callback-primary" ||
    report?.schema ===
      "vine.v86-bgi-winmm-time-get-time-post-test-cooperative-level-timer-callback-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-get-cursor-pos-post-test-cooperative-level-timer-callback-time-get-time-window-rect-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-get-window-rect-post-test-cooperative-level-timer-callback-time-get-time-primary"
  );
}

function isGetCursorInfoOrLaterNodeReport(report) {
  return (
    report?.schema === "vine.v86-bgi-get-cursor-info-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-update-texture-partial-dirty-region-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-begin-scene-post-partial-dirty-region-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-set-sampler-state-post-begin-scene-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-set-texture-stage-state-post-sampler-state-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-set-texture-post-texture-stage-state-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-set-fvf-post-texture-binding-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-set-stream-source-post-fvf-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-draw-primitive-post-stream-source-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-end-scene-post-draw-primitive-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-get-raster-status-post-end-scene-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-present-post-raster-status-primary" ||
    report?.schema ===
      "vine.v86-bgi-winmm-time-get-time-post-present-primary" ||
    report?.schema ===
      "vine.v86-bgi-d3d9-test-cooperative-level-post-time-get-time-primary" ||
    report?.schema ===
      "vine.v86-bgi-kernel32-set-waitable-timer-post-test-cooperative-level-primary" ||
    report?.schema ===
      "vine.v86-bgi-kernel32-wait-for-single-object-ex-post-test-cooperative-level-timer-arm-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-peek-message-a-post-test-cooperative-level-timer-wait-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-translate-message-post-test-cooperative-level-timer-peek-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-dispatch-message-a-post-test-cooperative-level-timer-translate-primary" ||
    report?.schema ===
      "vine.v86-bgi-kernel32-get-tick-count-post-test-cooperative-level-timer-dispatch-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-peek-message-a-post-test-cooperative-level-timer-callback-primary" ||
    report?.schema ===
      "vine.v86-bgi-winmm-time-get-time-post-test-cooperative-level-timer-callback-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-get-cursor-pos-post-test-cooperative-level-timer-callback-time-get-time-window-rect-primary" ||
    report?.schema ===
      "vine.v86-bgi-user32-get-window-rect-post-test-cooperative-level-timer-callback-time-get-time-primary"
  );
}

function projectScheduledSharedClockCheckpointCompletion(
  nodeReport,
  expectedNext,
) {
  const execution = nodeReport.execution;
  if (
    nodeReport.schema ===
    "vine.v86-bgi-user32-get-cursor-pos-post-test-cooperative-level-timer-callback-time-get-time-window-rect-primary"
  ) {
    return structuredClone(
      execution
        .user32GetCursorPosPostTestCooperativeLevelTimerCallbackTimeGetTimeWindowRect
        .checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-user32-get-window-rect-post-test-cooperative-level-timer-callback-time-get-time-primary"
  ) {
    return structuredClone(
      execution
        .user32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTime
        .checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-winmm-time-get-time-post-test-cooperative-level-timer-callback-primary"
  ) {
    return structuredClone(
      execution.winmmTimeGetTimePostTestCooperativeLevelTimerCallback
        .checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-user32-peek-message-a-post-test-cooperative-level-timer-callback-primary"
  ) {
    return structuredClone(
      execution.user32PeekMessageAPostTestCooperativeLevelTimerCallback
        .checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-kernel32-get-tick-count-post-test-cooperative-level-timer-dispatch-primary"
  ) {
    return structuredClone(
      execution.kernel32GetTickCountPostTestCooperativeLevelTimerDispatch
        .checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-user32-dispatch-message-a-post-test-cooperative-level-timer-translate-primary"
  ) {
    return structuredClone(
      execution.user32DispatchMessageAPostTestCooperativeLevelTimerTranslate
        .checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-user32-translate-message-post-test-cooperative-level-timer-peek-primary"
  ) {
    return structuredClone(
      execution.user32TranslateMessagePostTestCooperativeLevelTimerPeek
        .checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-user32-peek-message-a-post-test-cooperative-level-timer-wait-primary"
  ) {
    return structuredClone(
      execution.user32PeekMessageAPostTestCooperativeLevelTimerWait
        .checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-kernel32-wait-for-single-object-ex-post-test-cooperative-level-timer-arm-primary"
  ) {
    return structuredClone(
      execution.kernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArm
        .checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-kernel32-set-waitable-timer-post-test-cooperative-level-primary"
  ) {
    return structuredClone(
      execution.kernel32SetWaitableTimerPostTestCooperativeLevel
        .checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-test-cooperative-level-post-time-get-time-primary"
  ) {
    return structuredClone(
      execution.d3d9TestCooperativeLevelPostTimeGetTime.checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-winmm-time-get-time-post-present-primary"
  ) {
    return structuredClone(
      execution.winmmTimeGetTimePostPresent.checkpointCompletion,
    );
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-present-post-raster-status-primary"
  ) {
    const { call, ...completion } =
      execution.d3d9PresentPostRasterStatus.checkpointCompletion;
    const {
      occurrence: _occurrence,
      trapInstructionCount: _trapInstructionCount,
      ...projectedCall
    } = call;
    return {
      ...structuredClone(completion),
      call: structuredClone(projectedCall),
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-get-raster-status-post-end-scene-primary"
  ) {
    const { call, ...completion } =
      execution.d3d9GetRasterStatusPostEndScene.checkpointCompletion;
    const {
      occurrence: _occurrence,
      trapInstructionCount: _trapInstructionCount,
      ...projectedCall
    } = call;
    const guestWrites = projectedCall.guestWrites.map(
      ({ result: _writeReceipt, ...write }) => structuredClone(write),
    );
    return {
      ...structuredClone(completion),
      call: {
        ...structuredClone(projectedCall),
        guestWrites,
      },
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-end-scene-post-draw-primitive-primary"
  ) {
    const { call, ...completion } =
      execution.d3d9EndScenePostDrawPrimitive.checkpointCompletion;
    const {
      occurrence: _occurrence,
      trapInstructionCount: _trapInstructionCount,
      ...projectedCall
    } = call;
    return {
      ...structuredClone(completion),
      call: structuredClone(projectedCall),
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-draw-primitive-post-stream-source-primary"
  ) {
    const { call, ...completion } =
      execution.d3d9DrawPrimitivePostStreamSource.checkpointCompletion;
    const {
      occurrence: _occurrence,
      trapInstructionCount: _trapInstructionCount,
      ...projectedCall
    } = call;
    return {
      ...structuredClone(completion),
      call: structuredClone(projectedCall),
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-stream-source-post-fvf-primary"
  ) {
    const { call, ...completion } =
      execution.d3d9SetStreamSourcePostFvf.checkpointCompletion;
    const {
      occurrence: _occurrence,
      trapInstructionCount: _trapInstructionCount,
      ...projectedCall
    } = call;
    return {
      ...structuredClone(completion),
      call: structuredClone(projectedCall),
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-fvf-post-texture-binding-primary"
  ) {
    const { call, ...completion } =
      execution.d3d9SetFvfPostTextureBinding.checkpointCompletion;
    const {
      occurrence: _occurrence,
      trapInstructionCount: _trapInstructionCount,
      ...projectedCall
    } = call;
    return {
      ...structuredClone(completion),
      call: structuredClone(projectedCall),
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-texture-post-texture-stage-state-primary"
  ) {
    const { call, ...completion } =
      execution.d3d9SetTexturePostTextureStageState.checkpointCompletion;
    const {
      occurrence: _occurrence,
      trapInstructionCount: _trapInstructionCount,
      ...projectedCall
    } = call;
    return {
      ...structuredClone(completion),
      call: structuredClone(projectedCall),
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-texture-stage-state-post-sampler-state-primary"
  ) {
    const { calls, ...completion } =
      execution.d3d9SetTextureStageStatePostSamplerState.checkpointCompletion;
    const projectCall = ({
      occurrence: _occurrence,
      trapInstructionCount: _trapInstructionCount,
      ...call
    }) => structuredClone(call);
    return {
      ...structuredClone(completion),
      calls: calls.map(projectCall),
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-sampler-state-post-begin-scene-primary"
  ) {
    const {
      calls,
      postSamplerRenderStateRejections,
      ...completion
    } = execution.d3d9SetSamplerStatePostBeginScene.checkpointCompletion;
    const projectCall = ({
      occurrence: _occurrence,
      trapInstructionCount: _trapInstructionCount,
      ...call
    }) => structuredClone(call);
    return {
      ...structuredClone(completion),
      calls: calls.map(projectCall),
      postSamplerRenderStateRejections:
        postSamplerRenderStateRejections.map(projectCall),
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-begin-scene-post-partial-dirty-region-primary"
  ) {
    const {
      call,
      nextReturnAddress: _nextReturnAddress,
      nextStackPointer: _nextStackPointer,
      ...completion
    } = execution.d3d9BeginScenePostPartialDirtyRegion.checkpointCompletion;
    return {
      ...structuredClone(completion),
      presentedFrameSequence: call.graphicsDevice.presentedFrame.sequence,
      clockProfileId: nodeReport.contract.clockProfile.id,
      clockSampleCount: execution.clockSamples.count,
      windowTimerCallbackCount: execution.windowTimerCallbacks.count,
      presentCount: execution.presentations.count,
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-update-texture-partial-dirty-region-primary"
  ) {
    const { call: _call, ...completion } =
      execution.d3d9UpdateTexturePartialDirtyRegion.checkpointCompletion;
    return {
      ...structuredClone(completion),
      clockProfileId: nodeReport.contract.clockProfile.id,
      clockSampleCount: execution.clockSamples.count,
      windowTimerCallbackCount: execution.windowTimerCallbacks.count,
      presentCount: execution.presentations.count,
    };
  }
  if (nodeReport.schema === "vine.v86-bgi-get-cursor-info-primary") {
    const cursorInfo = execution.getCursorInfo;
    const calls = cursorInfo.calls.map((call) => ({
      sequence: call.sequence,
      module: call.module.toLowerCase(),
      symbol: call.symbol,
      arguments: call.arguments,
      result: call.result,
      implementationId: call.implementationId,
      returnAddress: call.returnAddress,
      stackPointerBefore: call.stackPointerBefore,
      stackPointerAfter: call.stackPointerAfter,
      lastError: call.lastError,
      guestWrites: call.guestWrites.map(({ address, bytes }) => ({
        address,
        bytes,
      })),
    }));
    const firstBytes = Buffer.from(calls[0].guestWrites[0].bytes);
    const target = nodeReport.stop.fault.endpoint.target;
    return {
      capability: "user32-get-cursor-info-post-outline-text-metrics-224x112",
      implementationId:
        "runtime.user32-get-cursor-info-post-outline-text-metrics-224x112.v1",
      requiredCompletions: 2,
      completionCount: cursorInfo.completionCount,
      prerequisiteOutlineTextMetricsSequence:
        execution.outlineTextMetrics224x112.calls.at(-1).sequence,
      prerequisiteOutlineTextMetricsInstructionCount:
        execution.outlineTextMetrics224x112.calls.at(-1).trapInstructionCount,
      firstCompletionSequence: calls[0].sequence,
      firstCompletionInstructionCount:
        cursorInfo.calls[0].trapInstructionCount,
      lastCompletionSequence: calls.at(-1).sequence,
      lastCompletionInstructionCount:
        cursorInfo.calls.at(-1).trapInstructionCount,
      calls,
      cursorInfoByteLength: firstBytes.byteLength,
      cursorSource: "system-arrow",
      cursorFlags: firstBytes.readUInt32LE(4),
      logicalCursorHandle: firstBytes.readUInt32LE(8),
      cursorPosition: {
        x: firstBytes.readInt32LE(12),
        y: firstBytes.readInt32LE(16),
      },
      cursorIdentityAuthority: "rust-runtime-user32",
      cursorPositionAuthority: "rust-runtime-input",
      browserOwnsCursorInfoState: false,
      lastErrorPolicy: "preserve",
      mutation: "guest-memory-preflight-atomic-write",
      clockProfileId: nodeReport.contract.clockProfile.id,
      clockSampleCount: execution.clockSamples.count,
      windowTimerCallbackCount: execution.windowTimerCallbacks.count,
      presentCount: execution.presentations.count,
      pointerInputEventCount: execution.pointerInput.count,
      inputTimerCycleCount:
        expectedNext.completion.inputTimerCycleCount,
      inputQuerySequence: execution.pointerInput.last.committedAfterSequence,
      thirdPresentReached: execution.presentations.thirdPresentReached,
      clockFreezeWasProximateCause: false,
      archiveEnumerationWasProximateCause: false,
      missingFileDeleteWasProximateCause: false,
      ordinaryHeapCapacityWasProximateCause: false,
      fontCreationWasProximateCause: false,
      dibSectionWasProximateCause: false,
      outlineTextMetricsWasProximateCause: false,
      cursorInfoWasProximateCause: false,
      instructionCount: nodeReport.stop.terminal.instructionCount,
      nextEndpoint: {
        endpointAddress: target.method.endpoint,
        kind: "com-method",
        target: {
          endpointAddress: target.method.endpoint,
          iid: target.interface.iid,
          interface: target.interface.name,
          interfaceId: target.interface.id,
          method: target.method.name,
          objectId: target.object.id,
          vtableSlot: target.method.slot,
        },
      },
      memoryContentSha256: nodeReport.stop.fault.memoryContentSha256,
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-outline-text-metrics-224x112-primary"
  ) {
    const outline = execution.outlineTextMetrics224x112;
    const [sizeQuery, fullCall] = outline.calls;
    const nextEndpoint = nodeReport.stop.fault.endpoint.endpoint;
    const reachedNextEndpointAfterOutlineMetrics =
      execution.clockSamples.count > 0 &&
      execution.windowTimerCallbacks.count > 0 &&
      execution.presentations.count === 3 &&
      execution.presentations.thirdPresentReached === true &&
      outline.completionCount === 2 &&
      sizeQuery.result.value === 269 &&
      fullCall.result.value === 269 &&
      nodeReport.stop.fault.fault.code === "endpoint-contract-missing";
    const deviceContext = fullCall.gdi.deviceContext;
    const selectedFont = fullCall.gdi.selectedFont;
    return {
      capability:
        "gdi32-get-outline-text-metrics-a-224x112-post-create-dib-section-448x336",
      implementationId:
        "runtime.gdi32-get-outline-text-metrics-a-224x112-post-create-dib-section-448x336.v1",
      requiredCompletions: 2,
      completionCount: outline.completionCount,
      prerequisiteDibSectionSequence:
        execution.createDibSection448x336.call.sequence,
      prerequisiteDibSectionInstructionCount:
        execution.createDibSection448x336.call.trapInstructionCount,
      firstCompletionSequence: sizeQuery.sequence,
      firstCompletionInstructionCount: sizeQuery.trapInstructionCount,
      lastCompletionSequence: fullCall.sequence,
      lastCompletionInstructionCount: fullCall.trapInstructionCount,
      sizeQueryCall: {
        sequence: sizeQuery.sequence,
        module: sizeQuery.module.toLowerCase(),
        symbol: sizeQuery.symbol,
        arguments: sizeQuery.arguments,
        result: sizeQuery.result,
        implementationId: sizeQuery.implementationId,
        returnAddress: sizeQuery.returnAddress,
        stackPointerBefore: sizeQuery.stackPointerBefore,
        stackPointerAfter: sizeQuery.stackPointerAfter,
        lastError: sizeQuery.lastError,
        guestWrites: sizeQuery.guestWrites,
        gdi: sizeQuery.gdi,
      },
      fullCall: {
        sequence: fullCall.sequence,
        module: fullCall.module.toLowerCase(),
        symbol: fullCall.symbol,
        arguments: fullCall.arguments,
        result: fullCall.result,
        implementationId: fullCall.implementationId,
        returnAddress: fullCall.returnAddress,
        stackPointerBefore: fullCall.stackPointerBefore,
        stackPointerAfter: fullCall.stackPointerAfter,
        lastError: fullCall.lastError,
        guestWrites: fullCall.guestWrites.map(({ address, bytes }) => ({
          address,
          bytes,
        })),
        gdi: fullCall.gdi,
      },
      outlineTextMetricsProfileId:
        nodeReport.contract.outlineTextMetricsProfileId,
      metricsByteLength: 269,
      logicalDeviceContextId: deviceContext.id,
      logicalDeviceContextHandle: deviceContext.handle,
      logicalFontObjectId: selectedFont.id,
      logicalFontHandle: selectedFont.handle,
      logicalBitmapObjectId: deviceContext.selectedBitmapId,
      logicalBitmapHandle: execution.createDibSection448x336.call.result.value,
      selectionGeneration: deviceContext.selectionGeneration,
      outputAddress: fullCall.gdi.outputAddress,
      writtenByteLength: fullCall.gdi.writtenByteLength,
      hostFontImported: false,
      browserFontImported: false,
      clockProfileId: nodeReport.contract.clockProfile.id,
      clockSampleCount: execution.clockSamples.count,
      windowTimerCallbackCount: execution.windowTimerCallbacks.count,
      presentCount: execution.presentations.count,
      pointerInputEventCount: execution.pointerInput.count,
      inputTimerCycleCount: expectedNext.completion.inputTimerCycleCount,
      inputQuerySequence: execution.pointerInput.last.committedAfterSequence,
      thirdPresentReached: execution.presentations.thirdPresentReached,
      clockFreezeWasProximateCause:
        !reachedNextEndpointAfterOutlineMetrics,
      archiveEnumerationWasProximateCause:
        !reachedNextEndpointAfterOutlineMetrics,
      missingFileDeleteWasProximateCause:
        !reachedNextEndpointAfterOutlineMetrics,
      ordinaryHeapCapacityWasProximateCause:
        !reachedNextEndpointAfterOutlineMetrics,
      fontCreationWasProximateCause:
        !reachedNextEndpointAfterOutlineMetrics,
      dibSectionWasProximateCause:
        !reachedNextEndpointAfterOutlineMetrics,
      outlineTextMetricsWasProximateCause:
        !reachedNextEndpointAfterOutlineMetrics,
      instructionCount: nodeReport.stop.terminal.instructionCount,
      nextEndpoint,
      memoryContentSha256: nodeReport.stop.fault.memoryContentSha256,
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-create-dib-section-448x336-primary"
  ) {
    const dib = execution.createDibSection448x336;
    const call = dib.call;
    const nextEndpoint = nodeReport.stop.fault.endpoint.endpoint;
    const reachedNextEndpointAfterDibCreation =
      execution.clockSamples.count > 0 &&
      execution.windowTimerCallbacks.count > 0 &&
      execution.presentations.count === 2 &&
      dib.completionCount === 1 &&
      call.result.value === 0x4702_0007 &&
      nodeReport.stop.fault.fault.code ===
        "outline-text-metrics-font-variant-unsupported";
    const backing = call.gdi.object.object.backing;
    return {
      capability:
        "gdi32-create-dib-section-448x336-post-create-font-a-224x112",
      implementationId:
        "runtime.gdi32-create-dib-section-448x336-post-create-font-a-224x112.v1",
      requiredCompletions: 1,
      completionCount: dib.completionCount,
      prerequisiteFontCreationSequence: execution.createFontA224x112.call.sequence,
      prerequisiteFontCreationInstructionCount:
        execution.createFontA224x112.call.trapInstructionCount,
      completionSequence: call.sequence,
      completionInstructionCount: call.trapInstructionCount,
      lastCompletionSequence: call.sequence,
      lastCompletionInstructionCount: call.trapInstructionCount,
      call: {
        sequence: call.sequence,
        module: call.module.toLowerCase(),
        symbol: call.symbol,
        arguments: call.arguments,
        result: call.result,
        implementationId: call.implementationId,
        returnAddress: call.returnAddress,
        stackPointerBefore: call.stackPointerBefore,
        stackPointerAfter: call.stackPointerAfter,
        lastError: call.lastError,
        guestWrites: call.guestWrites,
        gdi: call.gdi,
      },
      dibSectionProfileId:
        nodeReport.contract.dibSectionProfileId ??
        "primary-448x-336-8bpp-reverse-grayscale-v1",
      logicalBitmapObjectId: call.gdi.object.id,
      logicalBitmapHandle: call.gdi.object.handle,
      liveGdiObjects: call.gdi.liveObjects,
      pixelByteLength: call.gdi.object.object.descriptor.pixelByteLength,
      mappedByteLength: backing.range.byteLength,
      bitsAddress: backing.range.start,
      addressAllocationId: backing.addressAllocationId,
      memoryOwner: backing.memoryOwner,
      hostBitmapImported: false,
      browserBitmapImported: false,
      clockProfileId: nodeReport.contract.clockProfile.id,
      clockSampleCount: execution.clockSamples.count,
      windowTimerCallbackCount: execution.windowTimerCallbacks.count,
      presentCount: execution.presentations.count,
      pointerInputEventCount: execution.pointerInput.count,
      inputTimerCycleCount: expectedNext.completion.inputTimerCycleCount,
      inputQuerySequence: execution.pointerInput.last.committedAfterSequence,
      thirdPresentReached: execution.presentations.thirdPresentReached,
      clockFreezeWasProximateCause: !reachedNextEndpointAfterDibCreation,
      archiveEnumerationWasProximateCause:
        !reachedNextEndpointAfterDibCreation,
      missingFileDeleteWasProximateCause:
        !reachedNextEndpointAfterDibCreation,
      ordinaryHeapCapacityWasProximateCause:
        !reachedNextEndpointAfterDibCreation,
      fontCreationWasProximateCause: !reachedNextEndpointAfterDibCreation,
      dibSectionWasProximateCause: !reachedNextEndpointAfterDibCreation,
      instructionCount: nodeReport.stop.terminal.instructionCount,
      nextEndpoint,
      memoryContentSha256: nodeReport.stop.fault.memoryContentSha256,
    };
  }
  if (nodeReport.schema === "vine.v86-bgi-create-font-a-224x112-primary") {
    const font = execution.createFontA224x112;
    const call = font.call;
    const nextEndpoint = nodeReport.stop.fault.endpoint.endpoint;
    const reachedNextEndpointAfterFontCreation =
      execution.clockSamples.count > 0 &&
      execution.windowTimerCallbacks.count > 0 &&
      execution.presentations.count === 2 &&
      font.completionCount === 1 &&
      call.result.value === 0x4702_0006 &&
      nodeReport.stop.fault.fault.code ===
        "create-dib-section-variant-unsupported";
    return {
      capability:
        "gdi32-create-font-a-224x112-post-private-heap-segment-growth",
      implementationId:
        "runtime.gdi32-create-font-a-224x112-post-private-heap-segment-growth.v1",
      requiredCompletions: 1,
      completionCount: font.completionCount,
      prerequisiteHeapGrowthSequence:
        execution.ordinaryHeapSegmentGrowth.call.sequence,
      prerequisiteHeapGrowthInstructionCount:
        execution.ordinaryHeapSegmentGrowth.call.trapInstructionCount,
      completionSequence: call.sequence,
      completionInstructionCount: call.trapInstructionCount,
      lastCompletionSequence: call.sequence,
      lastCompletionInstructionCount: call.trapInstructionCount,
      call: {
        sequence: call.sequence,
        module: call.module.toLowerCase(),
        symbol: call.symbol,
        arguments: call.arguments,
        result: call.result,
        implementationId: call.implementationId,
        returnAddress: call.returnAddress,
        stackPointerBefore: call.stackPointerBefore,
        stackPointerAfter: call.stackPointerAfter,
        lastError: call.lastError,
        guestWrites: call.guestWrites,
        gdi: call.gdi,
      },
      creationProfileId: font.creationProfileId ??
        "primary-cp932-ms-gothic-224x112-thin-v1",
      logicalFontObjectId: call.gdi.object.id,
      logicalFontHandle: call.gdi.object.handle,
      liveGdiObjects: call.gdi.liveObjects,
      hostFontImported: false,
      browserFontImported: false,
      clockProfileId: nodeReport.contract.clockProfile.id,
      clockSampleCount: execution.clockSamples.count,
      windowTimerCallbackCount: execution.windowTimerCallbacks.count,
      presentCount: execution.presentations.count,
      pointerInputEventCount: execution.pointerInput.count,
      inputTimerCycleCount: expectedNext.completion.inputTimerCycleCount,
      inputQuerySequence: execution.pointerInput.last.committedAfterSequence,
      thirdPresentReached: execution.presentations.thirdPresentReached,
      clockFreezeWasProximateCause: !reachedNextEndpointAfterFontCreation,
      archiveEnumerationWasProximateCause:
        !reachedNextEndpointAfterFontCreation,
      missingFileDeleteWasProximateCause:
        !reachedNextEndpointAfterFontCreation,
      ordinaryHeapCapacityWasProximateCause:
        !reachedNextEndpointAfterFontCreation,
      fontCreationWasProximateCause:
        !reachedNextEndpointAfterFontCreation,
      instructionCount: nodeReport.stop.terminal.instructionCount,
      nextEndpoint,
      memoryContentSha256: nodeReport.stop.fault.memoryContentSha256,
    };
  }
  if (
    nodeReport.schema ===
    "vine.v86-bgi-private-heap-ordinary-segment-growth-primary"
  ) {
    const growth = execution.ordinaryHeapSegmentGrowth;
    const call = growth.call;
    const nextEndpoint = nodeReport.stop.fault.endpoint.endpoint;
    const reachedNextEndpointAfterGrowth =
      execution.clockSamples.count > 0 &&
      execution.windowTimerCallbacks.count > 0 &&
      execution.presentations.count === 2 &&
      growth.completionCount === 1 &&
      call.result.value === 0x3e00_0000 &&
      nodeReport.stop.fault.fault.code ===
        "create-font-a-variant-unsupported";
    return {
      capability:
        "kernel32-private-heap-ordinary-segment-growth-post-delete-file-a",
      implementationId:
        "runtime.kernel32-private-heap-ordinary-segment-growth-post-delete-file-a.v1",
      requiredCompletions: 1,
      completionCount: growth.completionCount,
      prerequisiteDeleteFileASequence: execution.deleteFileA.last.sequence,
      prerequisiteDeleteFileAInstructionCount:
        execution.deleteFileA.last.trapInstructionCount,
      completionSequence: call.sequence,
      completionInstructionCount: call.trapInstructionCount,
      lastCompletionSequence: call.sequence,
      lastCompletionInstructionCount: call.trapInstructionCount,
      call: {
        sequence: call.sequence,
        module: call.module.toLowerCase(),
        symbol: call.symbol,
        arguments: call.arguments,
        result: call.result,
        implementationId: call.implementationId,
        returnAddress: call.returnAddress,
        stackPointerBefore: call.stackPointerBefore,
        stackPointerAfter: call.stackPointerAfter,
        lastError: call.lastError,
        guestWrites: call.guestWrites,
        heapAllocation: call.heapAllocation,
      },
      ordinarySegmentSlot: growth.ordinarySegmentSlot,
      ordinarySegmentCapacityBytes: growth.ordinarySegmentCapacityBytes,
      ordinarySegmentCountAfter: growth.ordinarySegmentCountAfter,
      segmentSelectionPolicy: growth.segmentSelectionPolicy,
      clockProfileId: nodeReport.contract.clockProfile.id,
      clockSampleCount: execution.clockSamples.count,
      windowTimerCallbackCount: execution.windowTimerCallbacks.count,
      presentCount: execution.presentations.count,
      pointerInputEventCount: execution.pointerInput.count,
      inputTimerCycleCount: expectedNext.completion.inputTimerCycleCount,
      inputQuerySequence: execution.pointerInput.last.committedAfterSequence,
      thirdPresentReached: execution.presentations.thirdPresentReached,
      clockFreezeWasProximateCause: !reachedNextEndpointAfterGrowth,
      archiveEnumerationWasProximateCause: !reachedNextEndpointAfterGrowth,
      missingFileDeleteWasProximateCause: !reachedNextEndpointAfterGrowth,
      ordinaryHeapCapacityWasProximateCause:
        !reachedNextEndpointAfterGrowth,
      instructionCount: nodeReport.stop.terminal.instructionCount,
      nextEndpoint,
      memoryContentSha256: nodeReport.stop.fault.memoryContentSha256,
    };
  }
  if (nodeReport.schema === "vine.v86-bgi-delete-file-a-primary") {
    const deletion = execution.deleteFileA;
    const nextEndpoint = nodeReport.stop.fault.endpoint.endpoint;
    const reachedNextEndpointAfterMissingDeletes =
      execution.clockSamples.count > 0 &&
      execution.windowTimerCallbacks.count > 0 &&
      deletion.completionCount === 2 &&
      deletion.first.result.value === 0 &&
      deletion.last.result.value === 0 &&
      deletion.win32Error === 2 &&
      deletion.existingObjectMutation === false &&
      nodeReport.stop.fault.fault.code === "heap-backing-capacity-exceeded";
    return {
      capability: "kernel32-delete-file-a-post-find-file-a",
      implementationId:
        "runtime.kernel32-delete-file-a-post-find-file-a.v1",
      requiredCompletions: 2,
      completionCount: deletion.completionCount,
      prerequisiteFindCloseSequence: execution.findFileA.close.sequence,
      prerequisiteFindCloseInstructionCount:
        execution.findFileA.close.trapInstructionCount,
      firstCompletionSequence: deletion.first.sequence,
      firstCompletionInstructionCount: deletion.first.trapInstructionCount,
      lastCompletionSequence: deletion.last.sequence,
      lastCompletionInstructionCount: deletion.last.trapInstructionCount,
      win32Error: deletion.win32Error,
      missingFileOnly: true,
      existingObjectMutation: deletion.existingObjectMutation,
      clockProfileId: nodeReport.contract.clockProfile.id,
      clockSampleCount: execution.clockSamples.count,
      windowTimerCallbackCount: execution.windowTimerCallbacks.count,
      presentCount: execution.presentations.count,
      pointerInputEventCount: execution.pointerInput.count,
      inputTimerCycleCount: expectedNext.completion.inputTimerCycleCount,
      inputQuerySequence: execution.pointerInput.last.committedAfterSequence,
      thirdPresentReached: execution.presentations.thirdPresentReached,
      clockFreezeWasProximateCause: !reachedNextEndpointAfterMissingDeletes,
      archiveEnumerationWasProximateCause:
        !reachedNextEndpointAfterMissingDeletes,
      missingFileDeleteWasProximateCause:
        !reachedNextEndpointAfterMissingDeletes,
      instructionCount: nodeReport.stop.terminal.instructionCount,
      nextEndpoint,
      memoryContentSha256: nodeReport.stop.fault.memoryContentSha256,
    };
  }
  if (nodeReport.schema === "vine.v86-bgi-find-file-a-primary") {
    const find = execution.findFileA;
    const nextEndpoint = nodeReport.stop.fault.endpoint.endpoint;
    const reachedNextEndpointAfterEnumeration =
      execution.clockSamples.count > 0 &&
      execution.windowTimerCallbacks.count > 0 &&
      find.entryCount === 45 &&
      find.terminalNext.lastError === 18 &&
      find.close.result.value === 1 &&
      nodeReport.stop.fault.fault.code === "endpoint-contract-missing";
    return {
      capability: "kernel32-find-file-a-post-set-window-text",
      implementationId:
        "runtime.kernel32-find-file-a-post-set-window-text.v1",
      requiredCompletions: 1,
      completionCount: 1,
      lastCompletionSequence: find.close.sequence,
      lastCompletionInstructionCount: find.close.trapInstructionCount,
      firstCompletionSequence: find.first.sequence,
      firstCompletionInstructionCount: find.first.trapInstructionCount,
      entryCount: find.entryCount,
      lastSuccessfulNextSequence: find.lastSuccessfulNext.sequence,
      terminalNextSequence: find.terminalNext.sequence,
      terminalWin32Error: find.terminalWin32Error,
      searchHandle: `0x${find.first.result.value.toString(16).padStart(8, "0")}`,
      clockProfileId: nodeReport.contract.clockProfile.id,
      clockSampleCount: execution.clockSamples.count,
      windowTimerCallbackCount: execution.windowTimerCallbacks.count,
      presentCount: execution.presentations.count,
      pointerInputEventCount: execution.pointerInput.count,
      inputTimerCycleCount: expectedNext.completion.inputTimerCycleCount,
      inputQuerySequence: execution.pointerInput.last.committedAfterSequence,
      thirdPresentReached: execution.presentations.thirdPresentReached,
      clockFreezeWasProximateCause: !reachedNextEndpointAfterEnumeration,
      archiveEnumerationWasProximateCause:
        !reachedNextEndpointAfterEnumeration,
      instructionCount: nodeReport.stop.terminal.instructionCount,
      nextEndpoint,
      memoryContentSha256: nodeReport.stop.fault.memoryContentSha256,
    };
  }
  const setWindowText = execution.setWindowTextA;
  const call = setWindowText.call;
  const nextEndpoint = nodeReport.stop.fault.endpoint.endpoint;
  const reachedNextEndpointWithClockProgress =
    execution.clockSamples.count > 0 &&
    execution.windowTimerCallbacks.count > 0 &&
    setWindowText.count > 0 &&
    nodeReport.stop.fault.fault.code === "endpoint-contract-missing";
  return {
    capability: "user32-set-window-text-a-post-scheduled-shared-clock",
    implementationId:
      "runtime.user32-set-window-text-a-post-scheduled-shared-clock.v1",
    requiredCompletions: 1,
    completionCount: setWindowText.count,
    lastCompletionSequence: call.sequence,
    lastCompletionInstructionCount: call.callback.returnInstructionCount,
    defaultProcessingSequence: setWindowText.defaultProcessing.sequence,
    callbackEventCount: setWindowText.callbackEvents.length,
    clockProfileId: nodeReport.contract.clockProfile.id,
    clockSampleCount: execution.clockSamples.count,
    windowTimerCallbackCount: execution.windowTimerCallbacks.count,
    presentCount: execution.presentations.count,
    pointerInputEventCount: execution.pointerInput.count,
    // The cycle is a static scenario scheduling boundary. The compact Node
    // report records both injected receipts and their exact call boundaries.
    inputTimerCycleCount: expectedNext.completion.inputTimerCycleCount,
    inputQuerySequence: execution.pointerInput.last.committedAfterSequence,
    thirdPresentReached: execution.presentations.thirdPresentReached,
    clockFreezeWasProximateCause: !reachedNextEndpointWithClockProgress,
    instructionCount: nodeReport.stop.terminal.instructionCount,
    nextEndpoint,
    memoryContentSha256: nodeReport.stop.fault.memoryContentSha256,
  };
}

function assertCompletedCapabilityBeforeStructuredStop(
  actual,
  expectedCompletion,
  expectedNext,
) {
  assert(actual !== null, "completed capability metadata is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteOutlineTextMetricsSequence",
    "prerequisiteOutlineTextMetricsInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "cursorInfoByteLength",
    "cursorSource",
    "cursorFlags",
    "logicalCursorHandle",
    "cursorIdentityAuthority",
    "cursorPositionAuthority",
    "browserOwnsCursorInfoState",
    "lastErrorPolicy",
    "mutation",
    "clockProfileId",
    "clockSampleCount",
    "windowTimerCallbackCount",
    "presentCount",
    "pointerInputEventCount",
    "inputTimerCycleCount",
    "inputQuerySequence",
    "thirdPresentReached",
    "clockFreezeWasProximateCause",
    "archiveEnumerationWasProximateCause",
    "missingFileDeleteWasProximateCause",
    "ordinaryHeapCapacityWasProximateCause",
    "fontCreationWasProximateCause",
    "dibSectionWasProximateCause",
    "outlineTextMetricsWasProximateCause",
    "cursorInfoWasProximateCause",
    "instructionCount",
    "memoryContentSha256",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.cursorPosition, expectedCompletion.cursorPosition);
  assert.equal(actual.calls.length, expectedCompletion.completionCount);
  for (const call of actual.calls) {
    assert.equal(call.module.toLowerCase(), "user32.dll");
    assert.equal(call.symbol, "GetCursorInfo");
    assert.equal(
      call.implementationId,
      "user32.get-cursor-info.system-arrow.v1",
    );
    assert.deepEqual(call.result, { action: "set", value: 1 });
    assert.equal(call.lastError, 0);
    assert.equal(call.guestWrites.length, 1);
    assert.equal(call.guestWrites[0].bytes.length, 20);
    assert.equal(
      sha256(Buffer.from(call.guestWrites[0].bytes)),
      "83409a8eb120ec1c8528928c2ad5ec6935dc2a2dbca6bfbc4be4efe11b761780",
    );
  }
  assert.equal(actual.nextEndpoint.kind, "com-method");
  assert.equal(
    hex32(actual.nextEndpoint.endpointAddress),
    expectedNext.endpointAddress,
  );
  assert.equal(actual.nextEndpoint.target.interface, expectedNext.interface);
  assert.equal(actual.nextEndpoint.target.interfaceId, expectedNext.interfaceId);
  assert.equal(actual.nextEndpoint.target.objectId, expectedNext.objectId);
  assert.equal(actual.nextEndpoint.target.vtableSlot, expectedNext.vtableSlot);
  assert.equal(actual.nextEndpoint.target.method, expectedNext.method);
}

function assertD3d9UpdateTexturePartialDirtyRegionCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "partial UpdateTexture checkpoint metadata is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteGetCursorInfoSequence",
    "prerequisiteGetCursorInfoInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "updateTextureProfileId",
    "byteAuthority",
    "transferPublication",
    "partialTransferPrecondition",
    "sourceContentGeneration",
    "destinationContentGeneration",
    "sourceDirtyRegionAfter",
    "mutation",
    "failureAtomicity",
    "admittedAdditionalPartialDirtyRegions",
    "arbitraryPartialHistoryImplemented",
    "dynamicDestinationImplemented",
    "activeCallbackDepthAtCompletion",
    "clockProfileId",
    "clockSampleCount",
    "windowTimerCallbackCount",
    "presentCount",
    "instructionCount",
    "memoryContentSha256",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.dirtyRegion, expectedCompletion.dirtyRegion);
  for (const field of [
    "id",
    "address",
    "module",
    "disposition",
    "interfaceIndex",
    "vtableSlot",
    "interface",
    "method",
  ]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertD3d9SetTexturePostTextureStageStateCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-texture-stage SetTexture checkpoint is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteTextureStageStateSequence",
    "prerequisiteTextureStageStateInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "textureBindingProfileId",
    "stateAuthority",
    "bindingOwnership",
    "mutation",
    "sameValueGenerationChange",
    "failureAtomicity",
    "logicalGraphicsDeviceId",
    "logicalGraphicsComObjectId",
    "activeSceneSequence",
    "logicalGraphicsDeviceGeneration",
    "textureBindingStageZero",
    "backBufferContentGeneration",
    "backBufferClearSequence",
    "presentedFrameSequence",
    "browserRendererResourceCreated",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  for (const field of [
    "id",
    "address",
    "module",
    "disposition",
    "interfaceIndex",
    "vtableSlot",
    "interface",
    "method",
  ]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertD3d9SetFvfPostTextureBindingCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-texture-binding SetFVF checkpoint is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteTextureBindingSequence",
    "prerequisiteTextureBindingInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "flexibleVertexFormatProfileId",
    "stateAuthority",
    "mutation",
    "sameValueGenerationChange",
    "unsupportedVariantResult",
    "failureAtomicity",
    "logicalGraphicsDeviceId",
    "logicalGraphicsComObjectId",
    "activeSceneSequence",
    "logicalGraphicsDeviceGeneration",
    "flexibleVertexFormat",
    "textureBindingStageZero",
    "backBufferContentGeneration",
    "backBufferClearSequence",
    "presentedFrameSequence",
    "browserRendererResourceCreated",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  for (const field of [
    "id",
    "address",
    "module",
    "disposition",
    "interfaceIndex",
    "vtableSlot",
    "interface",
    "method",
  ]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertD3d9SetStreamSourcePostFvfCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-FVF SetStreamSource checkpoint is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteFvfSequence",
    "prerequisiteFvfInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "streamSourceProfileId",
    "stateAuthority",
    "bindingOwnership",
    "mutation",
    "sameValueGenerationChange",
    "supportedStream",
    "supportedOffsetInBytes",
    "supportedStride",
    "nullUnbindImplemented",
    "unsupportedVariantResult",
    "failureAtomicity",
    "logicalGraphicsDeviceId",
    "logicalGraphicsComObjectId",
    "activeSceneSequence",
    "logicalGraphicsDeviceGeneration",
    "vertexBufferId",
    "offsetInBytes",
    "stride",
    "flexibleVertexFormat",
    "textureBindingStageZero",
    "backBufferContentGeneration",
    "backBufferClearSequence",
    "presentedFrameSequence",
    "browserRendererResourceCreated",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  for (const field of [
    "id",
    "address",
    "module",
    "disposition",
    "interfaceIndex",
    "vtableSlot",
    "interface",
    "method",
  ]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertD3d9DrawPrimitivePostStreamSourceCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-stream-source DrawPrimitive checkpoint is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteStreamSourceSequence",
    "prerequisiteStreamSourceInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "drawPrimitiveProfileId",
    "stateAuthority",
    "vertexByteAuthority",
    "mutation",
    "supportedPrimitiveTopology",
    "supportedStartVertex",
    "supportedPrimitiveCount",
    "capturedVertexBytes",
    "vertexDataSha256",
    "pendingSubmissionLimit",
    "pendingVertexBytesLimit",
    "clearDiscardsPendingSubmissions",
    "failureAtomicity",
    "logicalGraphicsDeviceId",
    "logicalGraphicsComObjectId",
    "activeSceneSequence",
    "logicalGraphicsDeviceGeneration",
    "backBufferContentGeneration",
    "backBufferClearSequence",
    "drawSubmissionCount",
    "drawSubmissionSequence",
    "drawSubmissionSceneSequence",
    "vertexBufferId",
    "vertexBufferContentGeneration",
    "textureId",
    "textureContentGeneration",
    "presentedFrameSequence",
    "browserRendererResourceCreated",
    "visiblePixelsProduced",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  for (const field of [
    "id",
    "address",
    "module",
    "disposition",
    "interfaceIndex",
    "vtableSlot",
    "interface",
    "method",
  ]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertD3d9EndScenePostDrawPrimitiveCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-draw EndScene checkpoint is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteDrawPrimitiveSequence",
    "prerequisiteDrawPrimitiveInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "endSceneProfileId",
    "sceneAuthority",
    "lifecycle",
    "mutation",
    "preservesDrawSubmissions",
    "repeatedEndReturnsInvalidCall",
    "failureAtomicity",
    "logicalGraphicsDeviceId",
    "logicalGraphicsComObjectId",
    "logicalGraphicsDeviceGeneration",
    "activeSceneCleared",
    "backBufferContentGeneration",
    "backBufferClearSequence",
    "drawSubmissionCount",
    "drawSubmissionSequence",
    "drawSubmissionSceneSequence",
    "textureId",
    "textureContentGeneration",
    "presentedFrameSequence",
    "browserRendererResourceCreated",
    "visiblePixelsProduced",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  for (const field of [
    "id",
    "address",
    "module",
    "disposition",
    "interfaceIndex",
    "vtableSlot",
    "interface",
    "method",
  ]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertD3d9GetRasterStatusPostEndSceneCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-EndScene GetRasterStatus checkpoint is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteEndSceneSequence",
    "prerequisiteEndSceneInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "rasterStatusProfileId",
    "displayProfileId",
    "stateAuthority",
    "outputAddress",
    "outputByteLength",
    "outputHex",
    "inVBlank",
    "scanLine",
    "runtimeStateMutation",
    "referenceCountMutation",
    "lastErrorPolicy",
    "guestWriteAtomic",
    "failureAtomicity",
    "hostScanoutImported",
    "browserScanoutImported",
    "browserRendererResourceCreated",
    "visiblePixelsProduced",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  for (const field of [
    "id",
    "address",
    "module",
    "disposition",
    "interfaceIndex",
    "vtableSlot",
    "interface",
    "method",
  ]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertD3d9PresentPostRasterStatusCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-raster-status Present checkpoint is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteRasterStatusSequence",
    "prerequisiteRasterStatusInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "presentProfileId",
    "frameAuthority",
    "browserResourceAuthority",
    "mutation",
    "presentMode",
    "sourceRectImplemented",
    "destinationRectImplemented",
    "destinationWindowOverrideImplemented",
    "dirtyRegionImplemented",
    "failureAtomicity",
    "logicalGraphicsDeviceId",
    "logicalGraphicsComObjectId",
    "logicalGraphicsDeviceGeneration",
    "presentedFrameSequence",
    "presentedContentGeneration",
    "discardedBackBufferContentGeneration",
    "backBufferClearSequence",
    "drawSubmissionCount",
    "drawSubmissionSequence",
    "drawSubmissionSceneSequence",
    "textureId",
    "textureContentGeneration",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  for (const field of ["id", "address", "module", "disposition"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    optionalHex32(actual.nextEndpoint.iatAddress),
    expectedEndpoint.iatAddress,
    "iatAddress",
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);

  const browserPresentation = actual.call?.browserPresentation;
  if (browserPresentation === undefined) {
    assert.equal(actual.browserRendererResourceMaterialized, false);
    assert.equal(actual.renderedBeforeGuestResume, false);
    return;
  }
  for (const field of [
    "browserRendererResourceMaterialized",
    "browserRendererResourceId",
    "browserRendererResourceCount",
    "browserRendererResourceReused",
    "browserCapacityPreflightBeforeGuestExecution",
    "browserBindingPublishedAfterSnapshotValidation",
    "uploadedTextureBytes",
    "uploadedVertexBytes",
    "webGlError",
    "renderedBeforeGuestResume",
    "visiblePixelsProduced",
    "asynchronousDeviceLossRecoveryImplemented",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.centerPixelRgba, expectedCompletion.centerPixelRgba);
  assert.equal(browserPresentation.backend, "webgl2-offscreen-canvas");
  assert.equal(browserPresentation.resource.id, expectedCompletion.browserRendererResourceId);
  assert.equal(browserPresentation.resource.kind, "d3d9-webgl2-device");
  assert.deepEqual(browserPresentation.resource.binding, {
    objectId: "21",
    interfaceId: "42",
    interfaceName: "IDirect3DDevice9",
  });
  assert.equal(browserPresentation.browserResourceCount, 1);
  assert.equal(browserPresentation.deviceId, 3);
  assert.equal(browserPresentation.deviceGeneration, 74);
  assert.equal(browserPresentation.frameSequence, 4);
  assert.equal(browserPresentation.presentedContentGeneration, 11);
  assert.equal(browserPresentation.discardedBackBufferContentGeneration, 12);
  assert.equal(browserPresentation.width, 1_280);
  assert.equal(browserPresentation.height, 720);
  assert.equal(browserPresentation.drawSubmissionCount, 1);
  assert.equal(browserPresentation.uploadedTextureBytes, 8_388_608);
  assert.equal(browserPresentation.uploadedVertexBytes, 112);
  assert.deepEqual(browserPresentation.centerPixelRgba, [0, 0, 0, 255]);
  assert.equal(browserPresentation.webGlError, 0);
  assert.equal(browserPresentation.capacityPreflightBeforeGuestExecution, true);
  assert.equal(browserPresentation.bindingPublishedAfterSnapshotValidation, true);
  assert.equal(browserPresentation.renderedBeforeGuestResume, true);
}

function assertWinmmTimeGetTimePostPresentCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-Present timeGetTime checkpoint is missing");
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisitePresentSequence",
    "prerequisitePresentInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "clockProfileId",
    "clockAuthority",
    "clockRelation",
    "returnedTickCount",
    "nextTickCount",
    "incrementPerRead",
    "apiReadsAdvanceClock",
    "timerResolutionAffectsSampledClock",
    "lastErrorPolicy",
    "runtimeStateMutation",
    "failureAtomicity",
    "hostClockImported",
    "browserClockImported",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.timerClock, expectedCompletion.timerClock);
  assert.deepEqual(
    actual.scheduledExecutionClock,
    expectedCompletion.scheduledExecutionClock,
  );
  assert.deepEqual(actual.nextArguments, [0x0101_0004]);
  for (const field of [
    "id",
    "address",
    "module",
    "disposition",
    "interfaceIndex",
    "vtableSlot",
    "interface",
    "method",
  ]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertD3d9TestCooperativeLevelPostTimeGetTimeCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(
    actual !== null,
    "post-timeGetTime TestCooperativeLevel checkpoint is missing",
  );
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteTimeGetTimeSequence",
    "prerequisiteTimeGetTimeInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "testCooperativeLevelProfileId",
    "stateAuthority",
    "mutation",
    "lastErrorPolicy",
    "guestWriteBytes",
    "referenceCountMutation",
    "failureAtomicity",
    "deviceLostStateImplemented",
    "deviceNotResetStateImplemented",
    "resetImplemented",
    "browserContextLossImported",
    "logicalGraphicsDeviceId",
    "logicalGraphicsComObjectId",
    "logicalGraphicsDeviceGeneration",
    "deviceReferenceCount",
    "deviceObjectRetired",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.nextArguments, [
    0x5701_0001,
    0x006f_f8f0,
    0,
    0,
    0,
    0,
  ]);
  for (const field of ["id", "address", "disposition", "iatAddress"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    actual.nextEndpoint.module.toLowerCase(),
    expectedEndpoint.module.toLowerCase(),
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertKernel32SetWaitableTimerPostTestCooperativeLevelCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(
    actual !== null,
    "post-TestCooperativeLevel SetWaitableTimer checkpoint is missing",
  );
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteTestCooperativeLevelSequence",
    "prerequisiteTestCooperativeLevelInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "stateAuthority",
    "mutation",
    "failureAtomicity",
    "logicalTimerHandle",
    "logicalTimerId",
    "logicalObjectId",
    "manualReset",
    "relativeDueTime100ns",
    "armedAtTime100ns",
    "deadlineTime100ns",
    "periodMilliseconds",
    "armGeneration",
    "objectWaitState",
    "openHandles",
    "lastErrorPolicy",
    "guestWriteBytes",
    "hostTimerImported",
    "browserTimerCreated",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.nextArguments, [0x5701_0001, 10, 1]);
  for (const field of ["id", "address", "disposition", "iatAddress"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    actual.nextEndpoint.module.toLowerCase(),
    expectedEndpoint.module.toLowerCase(),
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertKernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArmCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(
    actual !== null,
    "post-TestCooperativeLevel timer-arm WaitForSingleObjectEx checkpoint is missing",
  );
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteSetWaitableTimerSequence",
    "prerequisiteSetWaitableTimerInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "stateAuthority",
    "mutation",
    "failureAtomicity",
    "logicalTimerHandle",
    "logicalTimerId",
    "logicalObjectId",
    "manualReset",
    "deadlineTime100ns",
    "armGeneration",
    "timerClockTime100ns",
    "timerClockGeneration",
    "nextWaitRegistrationId",
    "objectWaitState",
    "openHandles",
    "timeoutMilliseconds",
    "alertable",
    "waitResult",
    "lastErrorPolicy",
    "guestWriteBytes",
    "hostClockImported",
    "browserOwnsWaitState",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.nextArguments, [0x006f_f8e8]);
  for (const field of ["id", "address", "disposition", "iatAddress"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    actual.nextEndpoint.module.toLowerCase(),
    expectedEndpoint.module.toLowerCase(),
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertUser32PeekMessageAPostTestCooperativeLevelTimerWaitCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(
    actual !== null,
    "post-TestCooperativeLevel timer-wait PeekMessageA checkpoint is missing",
  );
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteWaitForSingleObjectExSequence",
    "prerequisiteWaitForSingleObjectExInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "retrievalProfileId",
    "stateAuthority",
    "mutation",
    "failureAtomicity",
    "currentThreadId",
    "messageOutputAddress",
    "messageOutputByteLength",
    "removeMessage",
    "queueDepthBefore",
    "queueDepthAfter",
    "messageSource",
    "timerMaterialization",
    "missedPeriodPolicy",
    "peekResult",
    "lastErrorPolicy",
    "guestWriteBytes",
    "hostMessageImported",
    "browserOwnsMessageQueueState",
    "browserTimerCreated",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.message, {
    hwnd: expectedCompletion.messageHwnd,
    message: expectedCompletion.messageId,
    wParam: expectedCompletion.messageWParam,
    lParam: expectedCompletion.messageLParam,
    time: expectedCompletion.messageTime,
    pointX: expectedCompletion.messagePointX,
    pointY: expectedCompletion.messagePointY,
  });
  assert.equal(actual.windowTimer.id, expectedCompletion.windowTimerId);
  assert.equal(
    actual.windowTimer.windowId,
    expectedCompletion.windowTimerWindowId,
  );
  assert.equal(
    actual.windowTimer.windowHandle,
    expectedCompletion.windowTimerWindowHandle,
  );
  assert.equal(
    actual.windowTimer.ownerThreadId,
    expectedCompletion.windowTimerOwnerThreadId,
  );
  assert.equal(
    actual.windowTimer.callbackAddress,
    expectedCompletion.windowTimerCallbackAddress,
  );
  assert.equal(
    actual.windowTimer.registrationGeneration,
    expectedCompletion.windowTimerRegistrationGeneration,
  );
  assert.equal(
    actual.windowTimer.deliveryCount,
    expectedCompletion.windowTimerDeliveryCount,
  );
  assert.equal(
    actual.windowTimer.lastDelivery.scheduledDeadlineTime100ns,
    expectedCompletion.windowTimerScheduledDeadlineTime100ns,
  );
  assert.equal(
    actual.windowTimer.lastDelivery.deliveredAtTime100ns,
    expectedCompletion.windowTimerDeliveredAtTime100ns,
  );
  assert.equal(
    actual.windowTimer.nextDeadlineTime100ns,
    expectedCompletion.windowTimerNextDeadlineTime100ns,
  );
  assert.deepEqual(actual.nextArguments, [0x006f_f8e8]);
  for (const field of ["id", "address", "disposition", "iatAddress"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    actual.nextEndpoint.module.toLowerCase(),
    expectedEndpoint.module.toLowerCase(),
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertUser32TranslateMessagePostTestCooperativeLevelTimerPeekCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(
    actual !== null,
    "post-TestCooperativeLevel timer-peek TranslateMessage checkpoint is missing",
  );
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisitePeekMessageASequence",
    "prerequisitePeekMessageAInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "translationProfileId",
    "stateAuthority",
    "mutation",
    "failureAtomicity",
    "currentThreadId",
    "messageAddress",
    "messageByteLength",
    "messageContentSha256",
    "translateResult",
    "lastErrorPolicy",
    "guestWriteBytes",
    "queueDepthAtCompletion",
    "messageBytesPreserved",
    "hostMessageImported",
    "browserOwnsMessageQueueState",
    "browserTimerCreated",
    "activeCallbackDepthAtCompletion",
    "generalKeyboardTranslationImplemented",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.message, {
    hwnd: expectedCompletion.messageHwnd,
    message: expectedCompletion.messageId,
    wParam: expectedCompletion.messageWParam,
    lParam: expectedCompletion.messageLParam,
    time: expectedCompletion.messageTime,
    pointX: expectedCompletion.messagePointX,
    pointY: expectedCompletion.messagePointY,
  });
  assert.equal(actual.windowTimer.id, expectedCompletion.windowTimerId);
  assert.equal(
    actual.windowTimer.windowId,
    expectedCompletion.windowTimerWindowId,
  );
  assert.equal(
    actual.windowTimer.windowHandle,
    expectedCompletion.windowTimerWindowHandle,
  );
  assert.equal(
    actual.windowTimer.ownerThreadId,
    expectedCompletion.windowTimerOwnerThreadId,
  );
  assert.equal(
    actual.windowTimer.callbackAddress,
    expectedCompletion.windowTimerCallbackAddress,
  );
  assert.equal(
    actual.windowTimer.registrationGeneration,
    expectedCompletion.windowTimerRegistrationGeneration,
  );
  assert.equal(
    actual.windowTimer.deliveryCount,
    expectedCompletion.windowTimerDeliveryCount,
  );
  assert.equal(
    actual.windowTimer.lastDelivery.scheduledDeadlineTime100ns,
    expectedCompletion.windowTimerScheduledDeadlineTime100ns,
  );
  assert.equal(
    actual.windowTimer.lastDelivery.deliveredAtTime100ns,
    expectedCompletion.windowTimerDeliveredAtTime100ns,
  );
  assert.equal(
    actual.windowTimer.nextDeadlineTime100ns,
    expectedCompletion.windowTimerNextDeadlineTime100ns,
  );
  assert.deepEqual(actual.nextArguments, [0x006f_f8e8]);
  for (const field of ["id", "address", "disposition", "iatAddress"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    actual.nextEndpoint.module.toLowerCase(),
    expectedEndpoint.module.toLowerCase(),
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertUser32PeekMessageAPostTestCooperativeLevelTimerCallbackCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(
    actual !== null,
    "post-callback empty PeekMessageA checkpoint is missing",
  );
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteGetTickCountSequence",
    "prerequisiteGetTickCountInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "retrievalProfileId",
    "stateAuthority",
    "mutation",
    "failureAtomicity",
    "currentThreadId",
    "callbackFrameId",
    "callbackCompletedBeforeCompletion",
    "outerDispatchCompletedBeforeCompletion",
    "activeCallbackDepthAtCompletion",
    "queueDepthBefore",
    "queueDepthAfter",
    "peekResult",
    "lastErrorPolicy",
    "guestWriteBytes",
    "hostMessageImported",
    "browserOwnsMessageQueueState",
    "messageBytesPreserved",
    "messageContentSha256",
    "activeCallbackDepthAtStop",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.equal(
    actual.call.module.toLowerCase(),
    expectedCompletion.call.module.toLowerCase(),
    "call.module",
  );
  assert.deepEqual(
    { ...actual.call, module: undefined },
    { ...expectedCompletion.call, module: undefined },
    "call",
  );
  assert.deepEqual(actual.nextArguments, expectedCompletion.nextArguments);
  for (const field of ["id", "address", "disposition", "iatAddress"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    actual.nextEndpoint.module.toLowerCase(),
    expectedEndpoint.module.toLowerCase(),
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertPrimaryMainLoopPostCallbackWindowInputScanBatchCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-callback window/input scan batch is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteGetCursorPosSequence",
    "prerequisiteGetCursorPosInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "stateAuthority",
    "mutation",
    "failureAtomicity",
    "browserOwnsInputState",
    "browserOwnsWindowState",
    "browserOwnsMessageQueueState",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.deepEqual(actual[field], expectedCompletion[field], field);
  }
  const actualCursor = actual.prerequisiteGetCursorPos;
  const expectedCursor = expectedCompletion.prerequisiteGetCursorPos;
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteGetWindowRectSequence",
    "prerequisiteGetWindowRectInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "cursorPositionAuthority",
    "cursorPosition",
    "cursorPositionGeneration",
    "outputType",
    "lastErrorPolicy",
    "mutation",
    "runtimeStateMutation",
    "failureAtomicity",
    "hostPointerImported",
    "browserPointerImported",
    "currentThreadId",
    "prerequisiteCallbackFrameId",
    "prerequisiteCallbackCompleted",
    "prerequisiteOuterDispatchCompleted",
    "activeCallbackDepthAtCompletion",
  ]) {
    assert.deepEqual(actualCursor[field], expectedCursor[field], `cursor.${field}`);
  }
  assert.deepEqual(
    normalizeSemanticCall(actualCursor.call),
    normalizeSemanticCall(expectedCursor.call),
  );
  for (const field of ["occurrence", "trapInstructionCount", "runtimeThreadId"]) {
    assert.equal(actualCursor.call[field], expectedCursor.call[field], `cursor.call.${field}`);
  }
  assert.equal(actual.metricCalls.length, expectedCompletion.metricCalls.length);
  assert.deepEqual(
    actual.metricCalls.map(normalizeSemanticCall),
    expectedCompletion.metricCalls.map(normalizeSemanticCall),
  );
  assert.deepEqual(
    actual.metricCalls.map(({ occurrence, trapInstructionCount }) => ({
      occurrence,
      trapInstructionCount,
    })),
    expectedCompletion.metricCalls.map(({ occurrence, trapInstructionCount }) => ({
      occurrence,
      trapInstructionCount,
    })),
  );
  assert.deepEqual(
    normalizeSemanticCall(actual.isIconicCall),
    normalizeSemanticCall(expectedCompletion.isIconicCall),
  );
  for (const field of ["occurrence", "trapInstructionCount", "runtimeThreadId"]) {
    assert.equal(
      actual.isIconicCall[field],
      expectedCompletion.isIconicCall[field],
      `isIconic.${field}`,
    );
  }
  assert.deepEqual(actual.actionInputEvents, expectedCompletion.actionInputEvents);
  assert.deepEqual(actual.pendingActionMessage, expectedCompletion.pendingActionMessage);
  assert.deepEqual(actual.nextArguments, expectedCompletion.nextArguments);
  for (const field of ["id", "address", "disposition", "iatAddress"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    actual.nextEndpoint.module.toLowerCase(),
    expectedEndpoint.module.toLowerCase(),
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertUser32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTimeCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
  expectedPrerequisiteTimeGetTime,
) {
  assert(actual !== null, "post-callback GetWindowRect checkpoint is missing");
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteTimeGetTimeSequence",
    "prerequisiteTimeGetTimeInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "windowAuthority",
    "geometrySource",
    "lastErrorPolicy",
    "runtimeStateMutation",
    "failureAtomicity",
    "hostGeometryImported",
    "browserGeometryImported",
    "currentThreadId",
    "prerequisiteCallbackFrameId",
    "prerequisiteCallbackCompleted",
    "prerequisiteOuterDispatchCompleted",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "returnedTickCount",
    "prerequisiteCallbackFrameId",
    "activeCallbackDepthAtCompletion",
  ]) {
    assert.equal(
      actual.prerequisiteTimeGetTime[field],
      expectedPrerequisiteTimeGetTime[field],
      `prerequisiteTimeGetTime.${field}`,
    );
  }
  const normalizeCall = (call) => ({
    ...call,
    module: call.module.toLowerCase(),
    guestWrites: call.guestWrites.map(({ address, bytes }) => ({
      address,
      bytes,
    })),
  });
  assert.deepEqual(
    normalizeCall(actual.call),
    normalizeCall(expectedCompletion.call),
  );
  assert.deepEqual(actual.nextArguments, expectedCompletion.nextArguments);
  for (const field of ["id", "address", "disposition", "iatAddress"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    actual.nextEndpoint.module.toLowerCase(),
    expectedEndpoint.module.toLowerCase(),
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertWinmmTimeGetTimePostTestCooperativeLevelTimerCallbackCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-callback timeGetTime checkpoint is missing");
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisitePeekMessageASequence",
    "prerequisitePeekMessageAInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "clockProfileId",
    "clockAuthority",
    "clockRelation",
    "returnedTickCount",
    "nextTickCount",
    "incrementPerRead",
    "apiReadsAdvanceClock",
    "timerResolutionAffectsSampledClock",
    "lastErrorPolicy",
    "runtimeStateMutation",
    "failureAtomicity",
    "hostClockImported",
    "browserClockImported",
    "currentThreadId",
    "prerequisiteCallbackFrameId",
    "prerequisiteCallbackCompleted",
    "prerequisiteOuterDispatchCompleted",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.timerClock, expectedCompletion.timerClock);
  assert.deepEqual(
    actual.scheduledExecutionClock,
    expectedCompletion.scheduledExecutionClock,
  );
  assert.equal(
    actual.call.module.toLowerCase(),
    expectedCompletion.call.module.toLowerCase(),
    "call.module",
  );
  assert.deepEqual(
    { ...actual.call, module: undefined },
    { ...expectedCompletion.call, module: undefined },
    "call",
  );
  assert.deepEqual(actual.nextArguments, expectedCompletion.nextArguments);
  for (const field of ["id", "address", "disposition", "iatAddress"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    actual.nextEndpoint.module.toLowerCase(),
    expectedEndpoint.module.toLowerCase(),
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertKernel32GetTickCountPostTestCooperativeLevelTimerDispatchCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(
    actual !== null,
    "post-DispatchMessageA timer GetTickCount checkpoint is missing",
  );
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteDispatchMessageASequence",
    "prerequisiteDispatchMessageAInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "clockProfileId",
    "stateAuthority",
    "mutation",
    "failureAtomicity",
    "currentThreadId",
    "callbackFrameId",
    "activeCallbackDepthAtCompletion",
    "callbackCompletedAtCompletion",
    "outerDispatchPendingAtCompletion",
    "lastErrorPolicy",
    "guestWriteBytes",
    "hostClockImported",
    "browserClockImported",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
    "callbackResultIgnored",
    "outerDispatchResult",
    "callbackCompletedBeforeNextEndpoint",
    "outerDispatchCompletedBeforeNextEndpoint",
    "activeCallbackDepthAtStop",
    "messageBytesPreserved",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  for (const field of ["call", "dispatchCompletion"]) {
    assert.equal(
      actual[field].module.toLowerCase(),
      expectedCompletion[field].module.toLowerCase(),
      `${field}.module`,
    );
    assert.deepEqual(
      { ...actual[field], module: undefined },
      { ...expectedCompletion[field], module: undefined },
      field,
    );
  }
  assert.deepEqual(actual.callbackReturn, expectedCompletion.callbackReturn);
  assert.deepEqual(actual.nextArguments, expectedCompletion.nextArguments);
  for (const field of ["id", "address", "disposition", "iatAddress"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    actual.nextEndpoint.module.toLowerCase(),
    expectedEndpoint.module.toLowerCase(),
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertUser32DispatchMessageAPostTestCooperativeLevelTimerTranslateCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(
    actual !== null,
    "post-TranslateMessage timer DispatchMessageA checkpoint is missing",
  );
  for (const field of [
    "capability",
    "implementationId",
    "semanticImplementationId",
    "requiredEntries",
    "entryCount",
    "prerequisiteTranslateMessageSequence",
    "prerequisiteTranslateMessageInstructionCount",
    "firstEntrySequence",
    "firstEntryInstructionCount",
    "lastEntrySequence",
    "lastEntryInstructionCount",
    "compatibilityPolicyId",
    "dispatchProfileId",
    "stateAuthority",
    "mutation",
    "failureAtomicity",
    "currentThreadId",
    "messageAddress",
    "messageByteLength",
    "messageContentSha256",
    "callbackResultPolicy",
    "outerDispatchPending",
    "lastErrorPolicy",
    "guestWriteBytes",
    "queueDepthAtEntry",
    "messageBytesPreserved",
    "hostMessageImported",
    "browserOwnsMessageQueueState",
    "browserOwnsCallbackState",
    "browserTimerCreated",
    "activeCallbackDepthAtEntry",
    "callbackCompleted",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.message, expectedCompletion.message);
  assert.deepEqual(actual.windowTimer, expectedCompletion.windowTimer);
  assert.deepEqual(
    { ...actual.dispatchEntry, module: actual.dispatchEntry.module.toLowerCase() },
    {
      ...expectedCompletion.dispatchEntry,
      module: expectedCompletion.dispatchEntry.module.toLowerCase(),
    },
  );
  assert.deepEqual(actual.callbackEntry, expectedCompletion.callbackEntry);
  assert.deepEqual(actual.nextArguments, expectedCompletion.nextArguments);
  for (const field of ["id", "address", "disposition", "iatAddress"]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.equal(
    actual.nextEndpoint.module.toLowerCase(),
    expectedEndpoint.module.toLowerCase(),
  );
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertD3d9SetTextureStageStatePostSamplerStateCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(
    actual !== null,
    "post-sampler texture-stage-state checkpoint metadata is missing",
  );
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteSamplerStateSequence",
    "prerequisiteSamplerStateInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "textureStageStateProfileId",
    "stateAuthority",
    "mutation",
    "sameValueGenerationChange",
    "failureAtomicity",
    "logicalGraphicsDeviceId",
    "logicalGraphicsComObjectId",
    "activeSceneSequence",
    "logicalGraphicsDeviceGeneration",
    "backBufferContentGeneration",
    "backBufferClearSequence",
    "presentedFrameSequence",
    "browserRendererResourceCreated",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.deepEqual(actual.textureStageState, expectedCompletion.textureStageState);
  assert.equal(actual.calls.length, 6);
  for (const field of [
    "id",
    "address",
    "module",
    "disposition",
    "interfaceIndex",
    "vtableSlot",
    "interface",
    "method",
  ]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertD3d9SetSamplerStatePostBeginSceneCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-BeginScene sampler checkpoint metadata is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteBeginSceneSequence",
    "prerequisiteBeginSceneInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "postSamplerRenderStateRejectionCount",
    "compatibilityPolicyId",
    "samplerStateProfileId",
    "stateAuthority",
    "mutation",
    "sameValueGenerationChange",
    "failureAtomicity",
    "logicalGraphicsDeviceId",
    "logicalGraphicsComObjectId",
    "activeSceneSequence",
    "logicalGraphicsDeviceGeneration",
    "samplerZeroMinFilter",
    "samplerZeroMagFilter",
    "backBufferContentGeneration",
    "backBufferClearSequence",
    "presentedFrameSequence",
    "browserRendererResourceCreated",
    "activeCallbackDepthAtCompletion",
    "instructionCount",
    "memoryContentSha256",
    "nextReturnAddress",
    "nextStackPointer",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  assert.equal(actual.calls.length, 2);
  assert.equal(actual.postSamplerRenderStateRejections.length, 3);
  for (const field of [
    "id",
    "address",
    "module",
    "disposition",
    "interfaceIndex",
    "vtableSlot",
    "interface",
    "method",
  ]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertD3d9BeginScenePostPartialDirtyRegionCheckpoint(
  actual,
  expectedCompletion,
  expectedEndpoint,
) {
  assert(actual !== null, "post-partial BeginScene checkpoint metadata is missing");
  for (const field of [
    "capability",
    "implementationId",
    "requiredCompletions",
    "completionCount",
    "prerequisiteUpdateTextureSequence",
    "prerequisiteUpdateTextureInstructionCount",
    "firstCompletionSequence",
    "firstCompletionInstructionCount",
    "lastCompletionSequence",
    "lastCompletionInstructionCount",
    "compatibilityPolicyId",
    "beginSceneProfileId",
    "authority",
    "lifecycle",
    "mutation",
    "failureAtomicity",
    "logicalGraphicsDeviceId",
    "logicalGraphicsComObjectId",
    "activeSceneSequence",
    "logicalGraphicsDeviceGeneration",
    "backBufferContentGeneration",
    "backBufferClearSequence",
    "browserRendererResourceCreated",
    "activeCallbackDepthAtCompletion",
    "clockProfileId",
    "clockSampleCount",
    "windowTimerCallbackCount",
    "presentCount",
    "instructionCount",
    "memoryContentSha256",
  ]) {
    assert.equal(actual[field], expectedCompletion[field], field);
  }
  const presentedFrameSequence =
    actual.presentedFrameSequence ??
    actual.call?.graphicsDevice?.presentedFrame?.sequence;
  assert.equal(
    presentedFrameSequence,
    expectedCompletion.presentedFrameSequence,
    "presentedFrameSequence",
  );
  for (const field of [
    "id",
    "address",
    "module",
    "disposition",
    "interfaceIndex",
    "vtableSlot",
    "interface",
    "method",
  ]) {
    assert.equal(actual.nextEndpoint[field], expectedEndpoint[field], field);
  }
  assert.deepEqual(actual.nextEndpoint.symbol, expectedEndpoint.symbol);
}

function assertScheduledSharedClockReportSemantics({
  checkpoint,
  nodeReport,
  browserReport,
  policy,
  executionEngine,
  source,
}) {
  const active = checkpoint.activeRuntimeCheckpoint;
  const scheduled = active.scheduledSharedClock;
  const expected = projectPrivateTargetExpectedBrowserEvidence(checkpoint);
  const expectedNext = checkpoint.nextStructuredStop;
  const nodeExecution = nodeReport.execution;
  const nodeStop = nodeReport.stop;
  const browser = browserReport.success.report;
  const sourceIdentity = { bytes: source.bytes, sha256: source.sha256 };
  const d3d9UpdateTexturePartialDirtyRegionReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-update-texture-partial-dirty-region-primary";
  const d3d9BeginScenePostPartialDirtyRegionReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-begin-scene-post-partial-dirty-region-primary";
  const d3d9SetSamplerStatePostBeginSceneReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-sampler-state-post-begin-scene-primary";
  const d3d9SetTextureStageStatePostSamplerStateReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-texture-stage-state-post-sampler-state-primary";
  const d3d9SetTexturePostTextureStageStateReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-texture-post-texture-stage-state-primary";
  const d3d9SetFvfPostTextureBindingReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-fvf-post-texture-binding-primary";
  const d3d9SetStreamSourcePostFvfReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-stream-source-post-fvf-primary";
  const d3d9DrawPrimitivePostStreamSourceReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-draw-primitive-post-stream-source-primary";
  const d3d9EndScenePostDrawPrimitiveReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-end-scene-post-draw-primitive-primary";
  const d3d9GetRasterStatusPostEndSceneReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-get-raster-status-post-end-scene-primary";
  const d3d9PresentPostRasterStatusReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-present-post-raster-status-primary";
  const winmmTimeGetTimePostPresentReport =
    nodeReport.schema ===
    "vine.v86-bgi-winmm-time-get-time-post-present-primary";
  const d3d9TestCooperativeLevelPostTimeGetTimeReport =
    nodeReport.schema ===
    "vine.v86-bgi-d3d9-test-cooperative-level-post-time-get-time-primary";
  const kernel32SetWaitableTimerPostTestCooperativeLevelReport =
    nodeReport.schema ===
    "vine.v86-bgi-kernel32-set-waitable-timer-post-test-cooperative-level-primary";
  const kernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArmReport =
    nodeReport.schema ===
    "vine.v86-bgi-kernel32-wait-for-single-object-ex-post-test-cooperative-level-timer-arm-primary";
  const user32PeekMessageAPostTestCooperativeLevelTimerWaitReport =
    nodeReport.schema ===
    "vine.v86-bgi-user32-peek-message-a-post-test-cooperative-level-timer-wait-primary";
  const user32TranslateMessagePostTestCooperativeLevelTimerPeekReport =
    nodeReport.schema ===
    "vine.v86-bgi-user32-translate-message-post-test-cooperative-level-timer-peek-primary";
  const user32DispatchMessageAPostTestCooperativeLevelTimerTranslateReport =
    nodeReport.schema ===
    "vine.v86-bgi-user32-dispatch-message-a-post-test-cooperative-level-timer-translate-primary";
  const kernel32GetTickCountPostTestCooperativeLevelTimerDispatchReport =
    nodeReport.schema ===
    "vine.v86-bgi-kernel32-get-tick-count-post-test-cooperative-level-timer-dispatch-primary";
  const user32PeekMessageAPostTestCooperativeLevelTimerCallbackReport =
    nodeReport.schema ===
    "vine.v86-bgi-user32-peek-message-a-post-test-cooperative-level-timer-callback-primary";
  const winmmTimeGetTimePostTestCooperativeLevelTimerCallbackReport =
    nodeReport.schema ===
    "vine.v86-bgi-winmm-time-get-time-post-test-cooperative-level-timer-callback-primary";
  const user32GetCursorPosPostTestCooperativeLevelTimerCallbackTimeGetTimeWindowRectReport =
    nodeReport.schema ===
    "vine.v86-bgi-user32-get-cursor-pos-post-test-cooperative-level-timer-callback-time-get-time-window-rect-primary";
  const user32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTimeReport =
    nodeReport.schema ===
    "vine.v86-bgi-user32-get-window-rect-post-test-cooperative-level-timer-callback-time-get-time-primary";

  assert(
    expectedNext.kind === "capability-checkpoint" ||
      (nodeReport.schema === "vine.v86-bgi-get-cursor-info-primary" &&
        expectedNext.kind === "com-method"),
    "scheduled checkpoint must end at its completed capability or the following owned COM stop",
  );
  assert.equal(scheduled.status, "passed");
  assert.deepEqual(nodeReport.source, sourceIdentity, "Node source fingerprint mismatch");
  assert.deepEqual(browserReport.source, sourceIdentity, "browser source fingerprint mismatch");
  assert.deepEqual(nodeReport.sourceSnapshot, {
    immutableAcrossRun: true,
    before: sourceIdentity,
    after: sourceIdentity,
  });
  assert.deepEqual(nodeReport.contract.clockProfile, scheduled.profile);
  assert.deepEqual(policy.clockProfile, scheduled.profile);
  assert.equal(nodeReport.contract.scheduleSource, scheduled.scheduleSource);
  assert.equal(
    active.kernel32CompatibilityPolicy.clockScheduleSource,
    scheduled.scheduleSource,
  );
  assert.equal(
    nodeReport.contract.retiredInstructionsPerSecond,
    scheduled.retiredInstructionsPerSecond,
  );
  assert.equal(
    active.kernel32CompatibilityPolicy.clockScheduledRetiredInstructionsPerSecond,
    scheduled.retiredInstructionsPerSecond,
  );
  assert.equal(nodeReport.contract.resolution100ns, scheduled.resolution100ns);
  assert.equal(
    active.kernel32CompatibilityPolicy.clockResolution100ns,
    scheduled.resolution100ns,
  );
  assert.equal(nodeReport.contract.apiReadsAdvanceClock, false);
  assert.equal(active.kernel32CompatibilityPolicy.clockApiReadsAdvance, false);
  assert.equal(nodeReport.contract.waitDeadlinesShareClock, true);
  assert.equal(
    active.kernel32CompatibilityPolicy.clockWaitDeadlinesShareAuthority,
    true,
  );
  assert.equal(nodeReport.contract.hostTimeImported, false);
  assert.equal(active.kernel32CompatibilityPolicy.clockHostTimeImported, false);
  assert.equal(active.kernel32CompatibilityPolicy.clockBrowserTimeImported, false);
  assert.equal(
    nodeReport.contract.semanticCallDigestAlgorithm,
    "sha256-canonical-json-v1",
  );
  assert.equal(
    nodeReport.process.implementedApiContracts,
    active.genericDispatcherContractCount,
  );
  assert.equal(
    nodeExecution.completedSupportedCalls,
    active.genericDispatcherCallOccurrenceCount,
  );
  assert.equal(
    nodeExecution.completedSupportedCalls,
    scheduled.nodePrimaryEvidence.completedSupportedCalls,
  );
  assert.equal(
    nodeExecution.supportedCallSemanticsSha256,
    scheduled.nodePrimaryEvidence.supportedCallSemanticsSha256,
  );
  assert.equal(nodeExecution.supportedCallSemanticsSha256, expected.callSemanticsSha256);
  assert.equal(Object.hasOwn(nodeExecution, "supportedCalls"), false);
  assert.deepEqual(
    nodeExecution.threadExecution.registeredThreadIds,
    browser.registeredThreadIds,
  );
  assert.deepEqual(
    nodeExecution.threadExecution.executedThreadIds,
    browser.executedThreadIds,
  );
  assert.equal(
    nodeExecution.threadExecution.contextSwitches,
    browser.contextSwitches,
  );

  const nodePacing = projectWaitableTimerPacingRequests(
    nodeExecution.timerPacing,
  );
  assert.equal(
    nodePacing.totalCount,
    scheduled.nodePrimaryEvidence.timerPacingCompletionCount,
  );
  assert.equal(
    nodePacing.totalCount,
    scheduled.browserBoundaryEvidence.privatePrimaryTimerPacingCompletionCount,
  );
  for (const request of nodePacing.sampledRequests) {
    assert.equal(request.outcome, "object-signaled");
    assert.equal(request.pacing, "deterministic-node-ack");
  }
  assert.deepEqual(browser.timerPacing, {
    requests: nodePacing.totalCount,
    completions: nodePacing.totalCount,
    cancellations: 0,
    freezes: 0,
    resumes: 0,
    staleCallbacks: 0,
    boundedCatchUps: 0,
    pending: 0,
  });

  assert.equal(
    nodeExecution.clockSamples.count,
    scheduled.nodePrimaryEvidence.clockSampleCount,
  );
  assert.equal(
    nodeExecution.windowTimerCallbacks.count,
    scheduled.nodePrimaryEvidence.windowTimerCallbackCount,
  );
  assert.equal(
    nodeExecution.presentations.count,
    scheduled.nodePrimaryEvidence.presentCount,
  );
  assert.equal(
    nodeExecution.presentations.thirdPresentReached,
    scheduled.nodePrimaryEvidence.thirdPresentReached,
  );
  assert.equal(nodeExecution.keyboardInput.count, 0);
  assert.equal(nodeExecution.pointerInput.count, 2);
  assert.equal(browser.keyboardInput.receivedMessages, 0);
  assert.equal(browser.keyboardInput.receipts.length, 0);
  assert.equal(browser.pointerInput.receivedMessages, 2);
  assert.equal(browser.pointerInput.queuedBeforePrepare, 2);
  assert.deepEqual(browser.pointerInput.receipts, [
    nodeExecution.pointerInput.first.receipt,
    nodeExecution.pointerInput.last.receipt,
  ]);
  assert.equal(
    nodeExecution.pointerInput.first.committedAfterSequence,
    scheduled.nodePrimaryEvidence.pointerInputDownCommittedAfterSequence,
  );
  assert.equal(
    nodeExecution.pointerInput.last.committedAfterSequence,
    scheduled.nodePrimaryEvidence.pointerInputUpCommittedAfterSequence,
  );

  const setWindowText = nodeExecution.setWindowTextA;
  assert.equal(setWindowText.count, 1);
  assert.equal(setWindowText.call.symbol, "SetWindowTextA");
  assert.equal(setWindowText.call.result.value, 1);
  assert.equal(setWindowText.call.window.name.value, "サクラノ詩");
  assert.equal(setWindowText.call.window.name.encoding, "windows-932-v1");
  assert.equal(setWindowText.call.callback.result, 1);
  assert.equal(setWindowText.defaultProcessing.symbol, "DefWindowProcA");
  assert.deepEqual(
    setWindowText.callbackEvents.map(({ action }) => action),
    ["enter", "return"],
  );
  if (
    nodeReport.schema === "vine.v86-bgi-find-file-a-primary" ||
    nodeReport.schema === "vine.v86-bgi-delete-file-a-primary" ||
    nodeReport.schema ===
      "vine.v86-bgi-private-heap-ordinary-segment-growth-primary" ||
    nodeReport.schema === "vine.v86-bgi-create-font-a-224x112-primary" ||
    nodeReport.schema === "vine.v86-bgi-create-dib-section-448x336-primary" ||
    nodeReport.schema === "vine.v86-bgi-outline-text-metrics-224x112-primary" ||
    isGetCursorInfoOrLaterNodeReport(nodeReport)
  ) {
    const find = nodeExecution.findFileA;
    assert.equal(find.pattern, "data0????.arc");
    assert.equal(find.entryCount, 45);
    assert.equal(find.entries[0].fileName, "data01099.arc");
    assert.equal(find.entries.at(-1).fileName, "data05010.arc");
    assert.equal(find.first.sequence, 251_694);
    assert.equal(find.lastSuccessfulNext.sequence, 251_738);
    assert.equal(find.terminalNext.sequence, 251_739);
    assert.equal(find.terminalNext.lastError, 18);
    assert.deepEqual(find.terminalNext.guestWrites, []);
    assert.equal(find.close.sequence, 251_740);
    assert.equal(find.close.result.value, 1);
    assert.equal(find.close.lastError, 18);
    assert.equal(find.searchHandleNamespace, "vine.runtime-file-search.v1");
    assert.equal(nodeReport.contract.hostDirectoryEnumerationImported, false);
  }
  if (
    nodeReport.schema === "vine.v86-bgi-delete-file-a-primary" ||
    nodeReport.schema ===
      "vine.v86-bgi-private-heap-ordinary-segment-growth-primary" ||
    nodeReport.schema === "vine.v86-bgi-create-font-a-224x112-primary" ||
    nodeReport.schema === "vine.v86-bgi-create-dib-section-448x336-primary" ||
    nodeReport.schema === "vine.v86-bgi-outline-text-metrics-224x112-primary" ||
    isGetCursorInfoOrLaterNodeReport(nodeReport)
  ) {
    const deletion = nodeExecution.deleteFileA;
    assert.equal(deletion.completionCount, 2);
    assert.equal(deletion.first.sequence, 255_411);
    assert.equal(deletion.first.trapInstructionCount, 328_088_002);
    assert.deepEqual(deletion.first.arguments, [0x006f_f388]);
    assert.equal(deletion.first.result.value, 0);
    assert.equal(deletion.first.lastError, 2);
    assert.equal(deletion.last.sequence, 255_432);
    assert.equal(deletion.last.trapInstructionCount, 328_097_369);
    assert.deepEqual(deletion.last.arguments, [0x006f_f388]);
    assert.equal(deletion.last.result.value, 0);
    assert.equal(deletion.last.lastError, 2);
    assert.equal(deletion.existingObjectMutation, false);
    assert.equal(nodeReport.contract.fileDeletionAuthority, "rust-runtime-vfs");
    assert.equal(nodeReport.contract.hostFilesystemMutationImported, false);
  }
  if (
    nodeReport.schema ===
      "vine.v86-bgi-private-heap-ordinary-segment-growth-primary" ||
    nodeReport.schema === "vine.v86-bgi-create-font-a-224x112-primary" ||
    nodeReport.schema === "vine.v86-bgi-create-dib-section-448x336-primary" ||
    nodeReport.schema === "vine.v86-bgi-outline-text-metrics-224x112-primary" ||
    isGetCursorInfoOrLaterNodeReport(nodeReport)
  ) {
    const growth = nodeExecution.ordinaryHeapSegmentGrowth;
    assert.equal(growth.completionCount, 1);
    assert.equal(growth.call.sequence, 260_631);
    assert.equal(growth.call.trapInstructionCount, 341_618_572);
    assert.deepEqual(growth.call.arguments, [0x6801_0001, 0, 0x0040_6000]);
    assert.equal(growth.call.result.value, 0x3e00_0000);
    assert.equal(growth.call.heapAllocation.allocation.id, 1_847);
    assert.equal(growth.call.heapAllocation.liveAllocations, 1_144);
    assert.equal(growth.ordinarySegmentSlot, 31);
    assert.equal(growth.ordinarySegmentCountAfter, 3);
    assert.equal(nodeReport.contract.ordinaryHeapProcessSlotLimit, 32);
  }
  if (
    nodeReport.schema === "vine.v86-bgi-create-font-a-224x112-primary" ||
    nodeReport.schema === "vine.v86-bgi-create-dib-section-448x336-primary" ||
    nodeReport.schema === "vine.v86-bgi-outline-text-metrics-224x112-primary" ||
    isGetCursorInfoOrLaterNodeReport(nodeReport)
  ) {
    const font = nodeExecution.createFontA224x112;
    assert.equal(font.completionCount, 1);
    assert.equal(font.call.sequence, 261_028);
    assert.equal(font.call.trapInstructionCount, 342_873_024);
    assert.deepEqual(font.call.arguments.slice(0, 13), [
      224,
      112,
      0,
      0,
      100,
      0,
      0,
      0,
      128,
      4,
      0,
      2,
      0,
    ]);
    assert.equal(font.call.arguments.length, 14);
    assert(Number.isSafeInteger(font.call.arguments[13]));
    assert(font.call.arguments[13] > 0);
    assert.equal(font.call.result.value, 0x4702_0006);
    assert.equal(
      font.call.implementationId,
      "gdi32.create-font-a.cp932-logical-object.v1",
    );
    assert.equal(font.call.gdi.action, "font-created");
    assert.equal(font.call.gdi.object.id, 6);
    assert.equal(font.call.gdi.object.handle, 0x4702_0006);
    assert.equal(font.call.gdi.object.object.kind, "font-a");
    assert.equal(font.call.gdi.object.object.logFont.height, 224);
    assert.equal(font.call.gdi.object.object.logFont.width, 112);
    assert.deepEqual(
      font.call.gdi.object.object.logFont.faceNameWindows932,
      [
        0x82, 0x6c, 0x82, 0x72, 0x20, 0x83, 0x53, 0x83, 0x56, 0x83, 0x62,
        0x83, 0x4e,
      ],
    );
    assert.equal(font.call.gdi.liveObjects, 4);
  }
  if (
    nodeReport.schema === "vine.v86-bgi-create-dib-section-448x336-primary" ||
    nodeReport.schema === "vine.v86-bgi-outline-text-metrics-224x112-primary" ||
    isGetCursorInfoOrLaterNodeReport(nodeReport)
  ) {
    const dib = nodeExecution.createDibSection448x336;
    assert.equal(dib.completionCount, 1);
    assert.equal(dib.call.sequence, 261_031);
    assert.equal(dib.call.trapInstructionCount, 342_875_151);
    assert.deepEqual(dib.call.arguments, [
      0x4701_000b,
      0x3e40_e3d0,
      0,
      0x20ff_d9e8,
      0,
      0,
    ]);
    assert.equal(dib.call.result.value, 0x4702_0007);
    assert.equal(
      dib.call.implementationId,
      "gdi32.create-dib-section.primary-8bpp.v1",
    );
    assert.equal(dib.call.gdi.action, "dib-section-created");
    assert.equal(dib.call.gdi.object.id, 7);
    assert.equal(dib.call.gdi.object.handle, 0x4702_0007);
    assert.equal(dib.call.gdi.object.object.kind, "dib-section");
    const descriptor = dib.call.gdi.object.object.descriptor;
    assert.equal(descriptor.width, 448);
    assert.equal(descriptor.height, -336);
    assert.equal(descriptor.strideBytes, 448);
    assert.equal(descriptor.pixelByteLength, 150_528);
    assert.equal(descriptor.palette.length, 256);
    assert.equal(dib.call.gdi.object.object.backing.range.byteLength, 151_552);
    assert.equal(dib.call.gdi.liveObjects, 5);
  }
  if (
    nodeReport.schema ===
      "vine.v86-bgi-outline-text-metrics-224x112-primary" ||
    isGetCursorInfoOrLaterNodeReport(nodeReport)
  ) {
    const outline = nodeExecution.outlineTextMetrics224x112;
    assert.equal(outline.completionCount, 2);
    assert.equal(outline.calls.length, 2);
    const [sizeQuery, fullCall] = outline.calls;
    assert.equal(sizeQuery.sequence, 261_036);
    assert.equal(sizeQuery.trapInstructionCount, 342_875_244);
    assert.deepEqual(sizeQuery.arguments, [0x4701_000b, 0, 0]);
    assert.equal(sizeQuery.result.value, 269);
    assert.equal(sizeQuery.guestWrites.length, 0);
    assert.equal(fullCall.sequence, 261_038);
    assert.equal(fullCall.trapInstructionCount, 342_875_297);
    assert.deepEqual(fullCall.arguments, [
      0x4701_000b,
      269,
      0x20ff_d9f8,
    ]);
    assert.equal(fullCall.result.value, 269);
    assert.equal(
      fullCall.implementationId,
      "gdi32.get-outline-text-metrics-a.cp932-selected-font.v1",
    );
    assert.equal(fullCall.guestWrites.length, 1);
    assert.equal(fullCall.guestWrites[0].address, 0x20ff_d9f8);
    assert.equal(fullCall.guestWrites[0].bytes.length, 269);
    assert.equal(
      sha256(Buffer.from(fullCall.guestWrites[0].bytes)),
      "61c4984d45246eb696425105af0908561546b01acf02a4242fe2c9f6cf884adc",
    );
    assert.equal(fullCall.gdi.action, "outline-text-metrics-read");
    assert.equal(fullCall.gdi.deviceContext.id, 11);
    assert.equal(fullCall.gdi.deviceContext.handle, 0x4701_000b);
    assert.equal(fullCall.gdi.deviceContext.selectedFontId, 6);
    assert.equal(fullCall.gdi.deviceContext.selectedBitmapId, 7);
    assert.equal(fullCall.gdi.deviceContext.selectionGeneration, 2);
    assert.equal(fullCall.gdi.selectedFont.id, 6);
    assert.equal(fullCall.gdi.selectedFont.handle, 0x4702_0006);
    assert.equal(
      fullCall.gdi.profileId,
      "primary-cp932-ms-gothic-224x112-outline-metrics-v1",
    );
    assert.equal(fullCall.gdi.requestedByteLength, 269);
    assert.equal(fullCall.gdi.returnedByteLength, 269);
    assert.equal(fullCall.gdi.writtenByteLength, 269);
  }

  const getCursorInfoReport = isGetCursorInfoOrLaterNodeReport(nodeReport);
  if (getCursorInfoReport) {
    const cursorInfo = nodeExecution.getCursorInfo;
    assert.equal(cursorInfo.completionCount, 2);
    assert.equal(cursorInfo.calls.length, 2);
    assert.deepEqual(
      cursorInfo.calls.map((call) => call.sequence),
      [282_389, 282_396],
    );
    assert.deepEqual(
      cursorInfo.calls.map((call) => call.trapInstructionCount),
      [375_285_853, 375_288_207],
    );
    assert.deepEqual(
      cursorInfo.calls.map((call) => call.arguments),
      [[0x006f_f530], [0x006f_f550]],
    );
    for (const call of cursorInfo.calls) {
      assert.equal(call.module.toLowerCase(), "user32.dll");
      assert.equal(call.symbol, "GetCursorInfo");
      assert.equal(
        call.implementationId,
        "user32.get-cursor-info.system-arrow.v1",
      );
      assert.deepEqual(call.result, { action: "set", value: 1 });
      assert.equal(call.lastError, 0);
      assert.equal(call.guestWrites.length, 1);
      assert.equal(
        sha256(Buffer.from(call.guestWrites[0].bytes)),
        "83409a8eb120ec1c8528928c2ad5ec6935dc2a2dbca6bfbc4be4efe11b761780",
      );
    }
    assert.equal(
      nodeReport.contract.cursorInfoAuthority,
      "runtime/user32.rs::RuntimeUser32+runtime/input.rs::RuntimeInput",
    );
    assert.equal(nodeReport.contract.cursorInfoCursorProjection, "loaded-system-arrow");
    assert.equal(nodeReport.contract.cursorInfoFlags, "cursor-showing-not-suppressed");
    assert.equal(nodeReport.contract.hostCursorStateImported, false);
    assert.equal(nodeReport.contract.browserCursorStateImported, false);
  }

  const nodeCheckpoint = projectScheduledSharedClockCheckpointCompletion(
    nodeReport,
    expectedNext,
  );
  let expectedFaultCode = null;
  if (
    user32GetCursorPosPostTestCooperativeLevelTimerCallbackTimeGetTimeWindowRectReport
  ) {
    const completion =
      nodeExecution
        .user32GetCursorPosPostTestCooperativeLevelTimerCallbackTimeGetTimeWindowRect
        .checkpointCompletion;
    const getCursorPos = completion.prerequisiteGetCursorPos;
    const cursorWrite = getCursorPos.call.guestWrites[0];
    assert.equal(nodeStop.fault, null);
    assert.equal(nodeStop.terminal.instructionCount, expectedNext.instructionCount);
    assert.equal(getCursorPos.call.sequence, 282_880);
    assert.equal(getCursorPos.call.occurrence, 145);
    assert.equal(getCursorPos.call.trapInstructionCount, 392_355_424);
    assert.equal(getCursorPos.call.module.toLowerCase(), "user32.dll");
    assert.equal(getCursorPos.call.symbol, "GetCursorPos");
    assert.deepEqual(getCursorPos.call.arguments, [0x006f_f88c]);
    assert.deepEqual(getCursorPos.call.result, { action: "set", value: 1 });
    assert.equal(getCursorPos.call.implementationId, "user32.get-cursor-pos.v1");
    assert.equal(cursorWrite.address, 0x006f_f88c);
    assert.deepEqual(cursorWrite.bytes, [0xc0, 0x03, 0, 0, 0x1c, 0x02, 0, 0]);
    assert.equal(cursorWrite.result.invalidation.generation, 333_520);
    assert.equal(completion.metricCalls.length, 18);
    assert.deepEqual(
      completion.metricCalls.map((call) => call.arguments[0]),
      Array.from({ length: 6 }, () => [7, 4, 8]).flat(),
    );
    assert.deepEqual(
      completion.metricCalls.map((call) => call.result.value),
      Array.from({ length: 6 }, () => [3, 23, 3]).flat(),
    );
    assert.equal(completion.isIconicCall.sequence, 283_412);
    assert.equal(completion.isIconicCall.occurrence, 68);
    assert.deepEqual(completion.isIconicCall.arguments, [0x4801_0001]);
    assert.deepEqual(completion.isIconicCall.result, { action: "set", value: 0 });
    assert.equal(completion.actionInputEvents.length, 1);
    const action = completion.actionInputEvents[0];
    assert.equal(action.event.sequence, 3);
    assert.equal(action.event.kind, "button-down");
    assert.equal(action.event.button, "primary");
    assert.equal(action.receipt.message.message, 0x0201);
    assert.equal(action.receipt.message.wParam, 1);
    assert.equal(action.receipt.message.lParam, 0x0168_0280);
    assert.deepEqual(completion.pendingActionMessage, action.receipt.message);
    assertPrimaryMainLoopPostCallbackWindowInputScanBatchCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (
    user32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTimeReport
  ) {
    const getWindowRect =
      nodeExecution
        .user32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTime;
    const completion = getWindowRect.checkpointCompletion;
    const call = completion.call;
    const write = call.guestWrites[0];
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.equal(completion.prerequisiteTimeGetTimeSequence, 282_878);
    assert.equal(
      completion.prerequisiteTimeGetTimeInstructionCount,
      392_355_124,
    );
    assert.equal(call.sequence, 282_879);
    assert.equal(call.occurrence, 145);
    assert.equal(call.trapInstructionCount, 392_355_420);
    assert.equal(call.module.toLowerCase(), "user32.dll");
    assert.equal(call.symbol, "GetWindowRect");
    assert.deepEqual(call.arguments, [0x4801_0001, 0x006f_f894]);
    assert.deepEqual(call.result, { action: "set", value: 1 });
    assert.equal(call.implementationId, "user32.get-window-rect.v1");
    assert.equal(call.returnAddress, 0x1008_8558);
    assert.equal(call.stackPointerBefore, 0x006f_f874);
    assert.equal(call.stackPointerAfter, 0x006f_f880);
    assert.equal(call.lastError, 0);
    assert.equal(call.guestWrites.length, 1);
    assert.equal(write.address, 0x006f_f894);
    assert.deepEqual(write.bytes, [
      61, 1, 0, 0, 154, 0, 0, 0, 67, 6, 0, 0, 135, 3, 0, 0,
    ]);
    assert.deepEqual(write.result, {
      bytesWritten: 16,
      dirtyPages: 1,
      invalidation: {
        generation: 333_518,
        dataTlb: false,
        instructionFetch: false,
        generatedCode: false,
      },
    });
    assert.equal(completion.prerequisiteCallbackFrameId, 75);
    assert.equal(completion.prerequisiteCallbackCompleted, true);
    assert.equal(completion.prerequisiteOuterDispatchCompleted, true);
    assert.equal(completion.activeCallbackDepthAtCompletion, 0);
    assertUser32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTimeCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
      expectedNext.prerequisiteTimeGetTimeCheckpoint,
    );
  } else if (winmmTimeGetTimePostTestCooperativeLevelTimerCallbackReport) {
    const timeGetTime =
      nodeExecution.winmmTimeGetTimePostTestCooperativeLevelTimerCallback;
    const completion = timeGetTime.checkpointCompletion;
    const call = completion.call;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.equal(completion.prerequisitePeekMessageASequence, 282_877);
    assert.equal(
      completion.prerequisitePeekMessageAInstructionCount,
      392_355_090,
    );
    assert.equal(call.sequence, 282_878);
    assert.equal(call.occurrence, 480);
    assert.equal(call.trapInstructionCount, 392_355_124);
    assert.equal(call.module.toLowerCase(), "winmm.dll");
    assert.equal(call.symbol, "timeGetTime");
    assert.deepEqual(call.arguments, []);
    assert.deepEqual(call.result, { action: "set", value: 3_800_773 });
    assert.equal(call.implementationId, "winmm.time-get-time.v1");
    assert.equal(call.returnAddress, 0x1009_1ebf);
    assert.equal(call.stackPointerBefore, 0x006f_f904);
    assert.equal(call.stackPointerAfter, 0x006f_f908);
    assert.equal(call.lastError, 0);
    assert.deepEqual(call.guestWrites, []);
    assert.equal(call.tickCount.returnedTickCount, 3_800_773);
    assert.equal(call.tickCount.nextTickCount, 3_800_773);
    assert.equal(call.tickCount.incrementPerRead, 0);
    assert.equal(call.tickCount.timerClock.currentTime100ns, 2_007_735_000);
    assert.equal(call.tickCount.timerClock.generation, 16_309);
    assert.equal(completion.prerequisiteCallbackFrameId, 75);
    assert.equal(completion.prerequisiteCallbackCompleted, true);
    assert.equal(completion.prerequisiteOuterDispatchCompleted, true);
    assert.equal(completion.activeCallbackDepthAtCompletion, 0);
    assertWinmmTimeGetTimePostTestCooperativeLevelTimerCallbackCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (user32PeekMessageAPostTestCooperativeLevelTimerCallbackReport) {
    const peekMessageA =
      nodeExecution.user32PeekMessageAPostTestCooperativeLevelTimerCallback;
    const completion = peekMessageA.checkpointCompletion;
    const call = completion.call;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.equal(completion.prerequisiteGetTickCountSequence, 282_876);
    assert.equal(
      completion.prerequisiteGetTickCountInstructionCount,
      392_354_458,
    );
    assert.equal(call.sequence, 282_877);
    assert.equal(call.occurrence, 111);
    assert.equal(call.trapInstructionCount, 392_355_090);
    assert.equal(call.module.toLowerCase(), "user32.dll");
    assert.equal(call.symbol, "PeekMessageA");
    assert.deepEqual(call.arguments, [0x006f_f8e8, 0, 0, 0, 1]);
    assert.deepEqual(call.result, { action: "set", value: 0 });
    assert.equal(call.implementationId, "user32.peek-message-a.current-thread.v1");
    assert.equal(call.returnAddress, 0x1009_37bb);
    assert.equal(call.stackPointerBefore, 0x006f_f8c4);
    assert.equal(call.stackPointerAfter, 0x006f_f8dc);
    assert.equal(call.lastError, 0);
    assert.deepEqual(call.guestWrites, []);
    assert.equal(completion.callbackFrameId, 75);
    assert.equal(completion.callbackCompletedBeforeCompletion, true);
    assert.equal(completion.outerDispatchCompletedBeforeCompletion, true);
    assert.equal(completion.queueDepthBefore, 0);
    assert.equal(completion.queueDepthAfter, 0);
    assert.equal(completion.messageBytesPreserved, true);
    assertUser32PeekMessageAPostTestCooperativeLevelTimerCallbackCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (kernel32GetTickCountPostTestCooperativeLevelTimerDispatchReport) {
    const getTickCount =
      nodeExecution.kernel32GetTickCountPostTestCooperativeLevelTimerDispatch;
    const completion = getTickCount.checkpointCompletion;
    const call = completion.call;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.equal(completion.prerequisiteDispatchMessageASequence, 282_875);
    assert.equal(
      completion.prerequisiteDispatchMessageAInstructionCount,
      392_354_437,
    );
    assert.equal(call.sequence, 282_876);
    assert.equal(call.occurrence, 100_079);
    assert.equal(call.trapInstructionCount, 392_354_458);
    assert.equal(call.module.toLowerCase(), "kernel32.dll");
    assert.equal(call.symbol, "GetTickCount");
    assert.deepEqual(call.arguments, []);
    assert.deepEqual(call.result, { action: "set", value: 3_800_773 });
    assert.equal(call.implementationId, "kernel32.get-tick-count.v1");
    assert.equal(call.returnAddress, 0x1009_9537);
    assert.equal(call.stackPointerBefore, 0x006f_f890);
    assert.equal(call.stackPointerAfter, 0x006f_f894);
    assert.equal(call.lastError, 0);
    assert.deepEqual(call.guestWrites, []);
    assert.equal(call.tickCount.returnedTickCount, 3_800_773);
    assert.equal(call.tickCount.nextTickCount, 3_800_773);
    assert.equal(call.tickCount.incrementPerRead, 0);
    assert.equal(call.tickCount.timerClock.currentTime100ns, 2_007_735_000);
    assert.equal(call.tickCount.timerClock.generation, 16_309);
    assert.equal(completion.dispatchCompletion.sequence, 282_875);
    assert.equal(completion.dispatchCompletion.callback.frameId, 75);
    assert.deepEqual(completion.callbackReturn, {
      action: "return",
      sequence: 282_875,
      frameId: 75,
      instructionCount: 392_355_081,
      result: 64,
    });
    assertKernel32GetTickCountPostTestCooperativeLevelTimerDispatchCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (user32DispatchMessageAPostTestCooperativeLevelTimerTranslateReport) {
    const dispatchMessageA =
      nodeExecution.user32DispatchMessageAPostTestCooperativeLevelTimerTranslate;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.equal(
      dispatchMessageA.checkpointCompletion.prerequisiteTranslateMessageSequence,
      282_874,
    );
    assert.equal(
      dispatchMessageA.checkpointCompletion
        .prerequisiteTranslateMessageInstructionCount,
      392_354_433,
    );
    assert.equal(dispatchMessageA.checkpointCompletion.dispatchEntry.sequence, 282_875);
    assert.equal(
      dispatchMessageA.checkpointCompletion.dispatchEntry.pendingCallback,
      true,
    );
    assertUser32DispatchMessageAPostTestCooperativeLevelTimerTranslateCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (user32TranslateMessagePostTestCooperativeLevelTimerPeekReport) {
    const translateMessage =
      nodeExecution.user32TranslateMessagePostTestCooperativeLevelTimerPeek;
    const translateCall = translateMessage.checkpointCompletion.call;
    const peekMessageA =
      nodeExecution.user32PeekMessageAPostTestCooperativeLevelTimerWait;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.equal(translateCall.sequence, 282_874);
    assert.equal(translateCall.occurrence, 43);
    assert.equal(translateCall.trapInstructionCount, 392_354_433);
    assert.equal(translateCall.module.toLowerCase(), "user32.dll");
    assert.equal(translateCall.symbol, "TranslateMessage");
    assert.deepEqual(translateCall.arguments, [0x006f_f8e8]);
    assert.deepEqual(translateCall.result, { action: "set", value: 0 });
    assert.equal(
      translateCall.implementationId,
      "user32.translate-message.window-timer-pointer-noop.v2",
    );
    assert.equal(translateCall.returnAddress, 0x1009_37a2);
    assert.equal(translateCall.stackPointerBefore, 0x006f_f8d4);
    assert.equal(translateCall.stackPointerAfter, 0x006f_f8dc);
    assert.equal(translateCall.lastError, 0);
    assert.deepEqual(translateCall.guestWrites, []);
    assert.equal(translateCall.windowTimer, undefined);
    assert.equal(peekMessageA.call.sequence, 282_873);
    assert.equal(
      Buffer.from(peekMessageA.call.guestWrites[0].bytes).toString("hex"),
      "01000148130100000100000000960910c5fe3900c00300001c020000",
    );
    assertUser32TranslateMessagePostTestCooperativeLevelTimerPeekCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (user32PeekMessageAPostTestCooperativeLevelTimerWaitReport) {
    const peekMessageA =
      nodeExecution.user32PeekMessageAPostTestCooperativeLevelTimerWait;
    const peekCall = peekMessageA.checkpointCompletion.call;
    const waitForSingleObjectEx =
      nodeExecution.kernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArm;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.equal(peekCall.sequence, 282_873);
    assert.equal(peekCall.occurrence, 110);
    assert.equal(peekCall.trapInstructionCount, 392_354_423);
    assert.equal(peekCall.module.toLowerCase(), "user32.dll");
    assert.equal(peekCall.symbol, "PeekMessageA");
    assert.deepEqual(peekCall.arguments, [0x006f_f8e8, 0, 0, 0, 1]);
    assert.deepEqual(peekCall.result, { action: "set", value: 1 });
    assert.equal(
      peekCall.implementationId,
      "user32.peek-message-a.current-thread.v1",
    );
    assert.equal(peekCall.returnAddress, 0x1009_378b);
    assert.equal(peekCall.stackPointerBefore, 0x006f_f8c8);
    assert.equal(peekCall.stackPointerAfter, 0x006f_f8e0);
    assert.equal(peekCall.lastError, 0);
    assert.equal(peekCall.guestWrites.length, 1);
    assert.equal(peekCall.guestWrites[0].address, 0x006f_f8e8);
    assert.equal(
      Buffer.from(peekCall.guestWrites[0].bytes).toString("hex"),
      "01000148130100000100000000960910c5fe3900c00300001c020000",
    );
    assert.equal(peekCall.guestWrites[0].result.bytesWritten, 28);
    assert.equal(peekCall.guestWrites[0].result.dirtyPages, 1);
    assert.equal(
      waitForSingleObjectEx.call.sequence,
      expectedNext.prerequisiteWaitForSingleObjectExCheckpoint
        .lastCompletionSequence,
    );
    assert.equal(peekCall.windowTimer.deliveryCount, 41);
    assert.equal(
      peekCall.windowTimer.lastDelivery.scheduledDeadlineTime100ns,
      1_915_270_000,
    );
    assert.equal(
      peekCall.windowTimer.lastDelivery.deliveredAtTime100ns,
      2_007_735_000,
    );
    assert.equal(peekCall.windowTimer.nextDeadlineTime100ns, 2_007_935_000);
    assertUser32PeekMessageAPostTestCooperativeLevelTimerWaitCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (kernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArmReport) {
    const waitForSingleObjectEx =
      nodeExecution.kernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArm;
    const waitCall = waitForSingleObjectEx.checkpointCompletion.call;
    const setWaitableTimer =
      nodeExecution.kernel32SetWaitableTimerPostTestCooperativeLevel;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.equal(waitCall.sequence, 282_872);
    assert.equal(waitCall.occurrence, 9_920);
    assert.equal(waitCall.trapInstructionCount, 392_354_375);
    assert.equal(waitCall.module.toLowerCase(), "kernel32.dll");
    assert.equal(waitCall.symbol, "WaitForSingleObjectEx");
    assert.deepEqual(waitCall.arguments, [0x5701_0001, 10, 1]);
    assert.deepEqual(waitCall.result, { action: "set", value: 0 });
    assert.equal(
      waitCall.implementationId,
      "kernel32.wait-for-single-object-ex.waitable-timer.v1",
    );
    assert.equal(waitCall.returnAddress, 0x1005_0615);
    assert.equal(waitCall.stackPointerBefore, 0x006f_f8e0);
    assert.equal(waitCall.stackPointerAfter, 0x006f_f8f0);
    assert.equal(waitCall.lastError, 0);
    assert.deepEqual(waitCall.guestWrites, []);
    assert.deepEqual(waitCall.waitableTimer, {
      handle: 0x5701_0001,
      timer: {
        id: 1,
        manualReset: true,
        lastExpiration: {
          armGeneration: 68,
          deadlineTime100ns: 2_007_735_000,
        },
      },
      objectRecord: {
        id: 20,
        key: { kind: "waitable-timer", timer_id: 1 },
        kind: "waitable-timer",
        namespace: "vine.runtime-waitable-timer-object.v1",
        closePolicy: "close-handle",
        waitState: { state: "signaled", exit_code: 0 },
        openHandles: 1,
      },
      timerClock: {
        currentTime100ns: 2_007_735_000,
        generation: 16_309,
      },
      nextWaitRegistrationId: 9_004,
    });
    assert.deepEqual(waitForSingleObjectEx.checkpointCompletion.pacing, {
      sequence: 282_872,
      requestId: expectedNext.completion.pacingRequestId,
      delayMilliseconds: expectedNext.completion.pacingDelayMilliseconds,
      registrationId: expectedNext.completion.pacingRegistrationId,
      timerId: "1",
      targetTime100ns: expectedNext.completion.pacingTargetTime100ns,
      timerClockGeneration:
        expectedNext.completion.pacingTimerClockGeneration,
      armGeneration: expectedNext.completion.pacingArmGeneration,
      outcome: expectedNext.completion.pacingOutcome,
      pacing: expectedNext.completion.nodePacingRole,
    });
    assert.equal(setWaitableTimer.call.sequence, 282_871);
    assert.equal(setWaitableTimer.call.result.value, 1);
    assertKernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArmCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (kernel32SetWaitableTimerPostTestCooperativeLevelReport) {
    const setWaitableTimer =
      nodeExecution.kernel32SetWaitableTimerPostTestCooperativeLevel;
    const setWaitableTimerCall =
      setWaitableTimer.checkpointCompletion.call;
    const testCooperativeLevel =
      nodeExecution.d3d9TestCooperativeLevelPostTimeGetTime;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.equal(setWaitableTimerCall.sequence, 282_871);
    assert.equal(setWaitableTimerCall.occurrence, 9_921);
    assert.equal(setWaitableTimerCall.trapInstructionCount, 392_354_369);
    assert.equal(setWaitableTimerCall.module.toLowerCase(), "kernel32.dll");
    assert.equal(setWaitableTimerCall.symbol, "SetWaitableTimer");
    assert.deepEqual(setWaitableTimerCall.arguments, [
      0x5701_0001,
      0x006f_f8f0,
      0,
      0,
      0,
      0,
    ]);
    assert.deepEqual(setWaitableTimerCall.result, {
      action: "set",
      value: 1,
    });
    assert.equal(
      setWaitableTimerCall.implementationId,
      "kernel32.set-waitable-timer.relative-one-shot.v1",
    );
    assert.equal(setWaitableTimerCall.returnAddress, 0x1005_0607);
    assert.equal(setWaitableTimerCall.stackPointerBefore, 0x006f_f8d4);
    assert.equal(setWaitableTimerCall.stackPointerAfter, 0x006f_f8f0);
    assert.equal(setWaitableTimerCall.lastError, 0);
    assert.deepEqual(setWaitableTimerCall.guestWrites, []);
    assert.deepEqual(setWaitableTimerCall.waitableTimer, {
      handle: 0x5701_0001,
      timer: {
        id: 1,
        manualReset: true,
        arm: {
          relativeDueTime100ns: 5_000,
          armedAtTime100ns: 2_007_730_000,
          deadlineTime100ns: 2_007_735_000,
          periodMilliseconds: 0,
          generation: 68,
        },
      },
      objectRecord: {
        id: 20,
        key: { kind: "waitable-timer", timer_id: 1 },
        kind: "waitable-timer",
        namespace: "vine.runtime-waitable-timer-object.v1",
        closePolicy: "close-handle",
        waitState: { state: "nonsignaled" },
        openHandles: 1,
      },
    });
    assert.equal(testCooperativeLevel.call.sequence, 282_870);
    assert.equal(testCooperativeLevel.call.result.value, 0);
    assertKernel32SetWaitableTimerPostTestCooperativeLevelCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9TestCooperativeLevelPostTimeGetTimeReport) {
    const testCooperativeLevel =
      nodeExecution.d3d9TestCooperativeLevelPostTimeGetTime;
    const timeGetTime = nodeExecution.winmmTimeGetTimePostPresent;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.deepEqual(
      testCooperativeLevel.call,
      testCooperativeLevel.checkpointCompletion.call,
    );
    assert.equal(testCooperativeLevel.call.sequence, 282_870);
    assert.equal(testCooperativeLevel.call.occurrence, 70);
    assert.equal(
      testCooperativeLevel.call.trapInstructionCount,
      392_354_323,
    );
    assert.equal(testCooperativeLevel.call.module, "<com>");
    assert.equal(
      testCooperativeLevel.call.symbol,
      "IDirect3DDevice9.TestCooperativeLevel",
    );
    assert.equal(testCooperativeLevel.call.interface, "IDirect3DDevice9");
    assert.equal(testCooperativeLevel.call.vtableSlot, 3);
    assert.deepEqual(testCooperativeLevel.call.arguments, [0x0101_0004]);
    assert.deepEqual(testCooperativeLevel.call.result, {
      kind: "h-result",
      value: 0,
    });
    assert.equal(testCooperativeLevel.call.returnAddress, 0x1005_adcd);
    assert.equal(testCooperativeLevel.call.stackPointerBefore, 0x006f_f8f4);
    assert.equal(testCooperativeLevel.call.stackPointerAfter, 0x006f_f8fc);
    assert.equal(testCooperativeLevel.call.referenceCount, 5);
    assert.equal(testCooperativeLevel.call.objectRetired, false);
    assert.deepEqual(testCooperativeLevel.call.guestWrites, []);
    assert.equal(timeGetTime.call.sequence, 282_869);
    assert.equal(timeGetTime.call.result.value, 3_800_773);
    assertD3d9TestCooperativeLevelPostTimeGetTimeCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (winmmTimeGetTimePostPresentReport) {
    const timeGetTime = nodeExecution.winmmTimeGetTimePostPresent;
    const present = nodeExecution.d3d9PresentPostRasterStatus;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.deepEqual(timeGetTime.call, timeGetTime.checkpointCompletion.call);
    assert.equal(timeGetTime.call.sequence, 282_869);
    assert.equal(timeGetTime.call.occurrence, 479);
    assert.equal(timeGetTime.call.trapInstructionCount, 392_354_190);
    assert.equal(timeGetTime.call.module.toLowerCase(), "winmm.dll");
    assert.equal(timeGetTime.call.symbol, "timeGetTime");
    assert.deepEqual(timeGetTime.call.arguments, []);
    assert.deepEqual(timeGetTime.call.result, {
      action: "set",
      value: 3_800_773,
    });
    assert.equal(timeGetTime.call.implementationId, "winmm.time-get-time.v1");
    assert.equal(timeGetTime.call.returnAddress, 0x1009_1ebf);
    assert.equal(timeGetTime.call.stackPointerBefore, 0x006f_b8a0);
    assert.equal(timeGetTime.call.stackPointerAfter, 0x006f_b8a4);
    assert.equal(timeGetTime.call.lastError, 0);
    assert.deepEqual(timeGetTime.call.guestWrites, []);
    assert.deepEqual(timeGetTime.call.tickCount, {
      returnedTickCount: 3_800_773,
      nextTickCount: 3_800_773,
      incrementPerRead: 0,
      clockProfileId: "scheduled-shared-logical-deadline-v1",
      timerClock: {
        currentTime100ns: 2_007_730_000,
        generation: 16_308,
      },
      scheduledExecutionClock: {
        retiredInstructions: 392_354_190,
        pendingRetiredInstructions: 190,
        retiredInstructionsPerSecond: 2_000_000,
      },
    });
    assert.equal(present.call.sequence, 282_868);
    assert.equal(present.call.graphicsDevice.presentedFrame.sequence, 4);
    assertWinmmTimeGetTimePostPresentCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9PresentPostRasterStatusReport) {
    const present = nodeExecution.d3d9PresentPostRasterStatus;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.deepEqual(present.call, present.checkpointCompletion.call);
    assert.equal(present.call.sequence, 282_868);
    assert.equal(present.call.occurrence, 4);
    assert.equal(present.call.symbol, "IDirect3DDevice9.Present");
    assert.equal(present.call.vtableSlot, 17);
    assert.deepEqual(present.call.arguments, [0x0101_0004, 0, 0, 0, 0]);
    assert.deepEqual(present.call.result, { kind: "h-result", value: 0 });
    assert.equal(present.call.referenceCount, 5);
    assert.equal(present.call.objectRetired, false);
    assert.deepEqual(present.call.guestWrites, []);
    assert.equal(present.call.graphicsDevice.id, 3);
    assert.equal(present.call.graphicsDevice.comObjectId, "21");
    assert.equal(present.call.graphicsDevice.generation, 74);
    assert.equal(present.call.graphicsDevice.presentedFrame.sequence, 4);
    assert.equal(
      present.call.graphicsDevice.presentedFrame.backBuffer.contentGeneration,
      11,
    );
    assert.equal(present.call.graphicsDevice.backBuffer.contentGeneration, 12);
    assert.equal(
      present.call.graphicsDevice.presentedFrame.backBuffer.drawSubmissions[0]
        .texture.content.pixelSource.range.byteLength,
      8_388_608,
    );
    assertD3d9PresentPostRasterStatusCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9GetRasterStatusPostEndSceneReport) {
    const raster = nodeExecution.d3d9GetRasterStatusPostEndScene;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.deepEqual(raster.call, raster.checkpointCompletion.call);
    assert.equal(raster.call.sequence, 282_867);
    assert.equal(raster.call.occurrence, 9_080);
    assert.equal(raster.call.symbol, "IDirect3DDevice9.GetRasterStatus");
    assert.equal(raster.call.vtableSlot, 19);
    assert.deepEqual(raster.call.arguments, [0x0101_0004, 0, 0x006f_b890]);
    assert.deepEqual(raster.call.result, { kind: "h-result", value: 0 });
    assert.equal(raster.call.referenceCount, 5);
    assert.equal(raster.call.objectRetired, false);
    assert.equal(raster.call.guestWrites.length, 1);
    assert.deepEqual(
      raster.call.guestWrites[0].bytes,
      [0, 0, 0, 0, 0x37, 0x04, 0, 0],
    );
    assertD3d9GetRasterStatusPostEndSceneCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9EndScenePostDrawPrimitiveReport) {
    const endScene = nodeExecution.d3d9EndScenePostDrawPrimitive;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.deepEqual(endScene.call, endScene.checkpointCompletion.call);
    assert.equal(endScene.call.sequence, 282_866);
    assert.equal(endScene.call.symbol, "IDirect3DDevice9.EndScene");
    assert.equal(endScene.call.vtableSlot, 42);
    assert.deepEqual(endScene.call.arguments, [0x0101_0004]);
    assert.deepEqual(endScene.call.result, { kind: "h-result", value: 0 });
    assert.deepEqual(endScene.call.guestWrites, []);
    assert.equal(endScene.call.graphicsDevice.id, 3);
    assert.equal(endScene.call.graphicsDevice.generation, 73);
    assert.equal(endScene.call.graphicsDevice.activeScene, undefined);
    assert.equal(endScene.call.graphicsDevice.backBuffer.contentGeneration, 11);
    assert.equal(
      endScene.call.graphicsDevice.backBuffer.drawSubmissions.length,
      1,
    );
    const submission =
      endScene.call.graphicsDevice.backBuffer.drawSubmissions[0];
    assert.equal(submission.sequence, 4);
    assert.equal(submission.sceneSequence, 4);
    assert.equal(submission.vertexData.length, 112);
    assert.equal(submission.texture.textureId, 8);
    assert.equal(submission.texture.content.contentGeneration, 4);
    assertD3d9EndScenePostDrawPrimitiveCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9DrawPrimitivePostStreamSourceReport) {
    const draw = nodeExecution.d3d9DrawPrimitivePostStreamSource;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.deepEqual(draw.call, draw.checkpointCompletion.call);
    assert.equal(draw.call.sequence, 282_865);
    assert.equal(draw.call.symbol, "IDirect3DDevice9.DrawPrimitive");
    assert.equal(draw.call.vtableSlot, 81);
    assert.deepEqual(draw.call.arguments, [0x0101_0004, 5, 0, 2]);
    assert.deepEqual(draw.call.result, { kind: "h-result", value: 0 });
    assert.deepEqual(draw.call.guestWrites, []);
    assert.equal(draw.call.graphicsDevice.id, 3);
    assert.equal(draw.call.graphicsDevice.generation, 72);
    assert.equal(draw.call.graphicsDevice.activeScene.sequence, 4);
    assert.equal(draw.call.graphicsDevice.backBuffer.contentGeneration, 11);
    assert.equal(
      draw.call.graphicsDevice.backBuffer.drawSubmissions.length,
      1,
    );
    const submission =
      draw.call.graphicsDevice.backBuffer.drawSubmissions[0];
    assert.equal(submission.sequence, 4);
    assert.equal(submission.sceneSequence, 4);
    assert.equal(submission.vertexData.length, 112);
    assert.equal(
      sha256(Buffer.from(submission.vertexData)),
      "405bea74541d2aece445d0dac693da55fb404e777c91ff81d87d4c6272581362",
    );
    assert.equal(submission.texture.textureId, 8);
    assert.equal(submission.texture.content.contentGeneration, 4);
    assertD3d9DrawPrimitivePostStreamSourceCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9SetStreamSourcePostFvfReport) {
    const streamSource = nodeExecution.d3d9SetStreamSourcePostFvf;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.deepEqual(streamSource.call, streamSource.checkpointCompletion.call);
    assert.equal(streamSource.call.sequence, 282_864);
    assert.equal(
      streamSource.call.symbol,
      "IDirect3DDevice9.SetStreamSource",
    );
    assert.equal(streamSource.call.vtableSlot, 100);
    assert.deepEqual(
      streamSource.call.arguments,
      [0x0101_0004, 0, 0x0105_0004, 0, 28],
    );
    assert.deepEqual(streamSource.call.result, { kind: "h-result", value: 0 });
    assert.deepEqual(streamSource.call.guestWrites, []);
    assert.equal(streamSource.call.graphicsDevice.id, 3);
    assert.equal(streamSource.call.graphicsDevice.generation, 71);
    assert.equal(streamSource.call.graphicsDevice.activeScene.sequence, 4);
    assert.deepEqual(streamSource.call.graphicsDevice.textureBindings, {
      stageZero: 8,
    });
    assert.equal(
      streamSource.call.graphicsDevice.flexibleVertexFormat,
      "xyz-rhw-diffuse-texture1",
    );
    assert.deepEqual(
      streamSource.call.graphicsDevice.streamSources.streamZero,
      { vertexBufferId: 3, offsetInBytes: 0, stride: 28 },
    );
    assertD3d9SetStreamSourcePostFvfCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9SetFvfPostTextureBindingReport) {
    const fvf = nodeExecution.d3d9SetFvfPostTextureBinding;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.deepEqual(fvf.call, fvf.checkpointCompletion.call);
    assert.equal(fvf.call.sequence, 282_863);
    assert.equal(fvf.call.symbol, "IDirect3DDevice9.SetFVF");
    assert.equal(fvf.call.vtableSlot, 89);
    assert.deepEqual(fvf.call.arguments, [0x0101_0004, 0x144]);
    assert.deepEqual(fvf.call.result, { kind: "h-result", value: 0 });
    assert.deepEqual(fvf.call.guestWrites, []);
    assert.equal(fvf.call.graphicsDevice.id, 3);
    assert.equal(fvf.call.graphicsDevice.generation, 71);
    assert.equal(fvf.call.graphicsDevice.activeScene.sequence, 4);
    assert.deepEqual(fvf.call.graphicsDevice.textureBindings, {
      stageZero: 8,
    });
    assert.equal(
      fvf.call.graphicsDevice.flexibleVertexFormat,
      "xyz-rhw-diffuse-texture1",
    );
    assert.deepEqual(fvf.call.graphicsDevice.streamSources.streamZero, {
      vertexBufferId: 3,
      offsetInBytes: 0,
      stride: 28,
    });
    assertD3d9SetFvfPostTextureBindingCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9SetTexturePostTextureStageStateReport) {
    const texture = nodeExecution.d3d9SetTexturePostTextureStageState;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.deepEqual(texture.call, texture.checkpointCompletion.call);
    assert.equal(texture.call.sequence, 282_862);
    assert.equal(texture.call.symbol, "IDirect3DDevice9.SetTexture");
    assert.equal(texture.call.vtableSlot, 65);
    assert.deepEqual(texture.call.arguments, [0x0101_0004, 0, 0x0103_0004]);
    assert.deepEqual(texture.call.result, { kind: "h-result", value: 0 });
    assert.deepEqual(texture.call.guestWrites, []);
    assert.equal(texture.call.graphicsDevice.id, 3);
    assert.equal(texture.call.graphicsDevice.generation, 71);
    assert.equal(texture.call.graphicsDevice.activeScene.sequence, 4);
    assert.deepEqual(texture.call.graphicsDevice.textureBindings, {
      stageZero: 8,
    });
    assert.equal(
      texture.call.graphicsDevice.flexibleVertexFormat,
      "xyz-rhw-diffuse-texture1",
    );
    assert.deepEqual(texture.call.graphicsDevice.streamSources.streamZero, {
      vertexBufferId: 3,
      offsetInBytes: 0,
      stride: 28,
    });
    assertD3d9SetTexturePostTextureStageStateCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9SetTextureStageStatePostSamplerStateReport) {
    const textureStage =
      nodeExecution.d3d9SetTextureStageStatePostSamplerState;
    assert.equal(nodeStop.fault, null);
    assert.equal(
      nodeStop.terminal.instructionCount,
      expectedNext.instructionCount,
    );
    assert.deepEqual(
      textureStage.calls,
      textureStage.checkpointCompletion.calls,
    );
    assert.deepEqual(
      textureStage.calls.map((call) => call.sequence),
      [282_856, 282_857, 282_858, 282_859, 282_860, 282_861],
    );
    assert.deepEqual(
      textureStage.calls.map((call) => call.arguments),
      [
        [0x0101_0004, 0, 1, 4],
        [0x0101_0004, 0, 2, 2],
        [0x0101_0004, 0, 3, 0],
        [0x0101_0004, 0, 4, 2],
        [0x0101_0004, 0, 5, 2],
        [0x0101_0004, 0, 6, 0],
      ],
    );
    for (const call of textureStage.calls) {
      assert.equal(call.symbol, "IDirect3DDevice9.SetTextureStageState");
      assert.deepEqual(call.result, { kind: "h-result", value: 0 });
      assert.deepEqual(call.guestWrites, []);
      assert.equal(call.graphicsDevice.id, 3);
      assert.equal(call.graphicsDevice.generation, 71);
      assert.equal(call.graphicsDevice.activeScene.sequence, 4);
      assert.deepEqual(call.graphicsDevice.textureStageStates.stageZero, {
        colorOperation: "modulate",
        colorArgumentOne: "texture",
        colorArgumentTwo: "diffuse",
        alphaOperation: "select-argument-one",
        alphaArgumentOne: "texture",
        alphaArgumentTwo: "diffuse",
      });
    }
    assertD3d9SetTextureStageStatePostSamplerStateCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9SetSamplerStatePostBeginSceneReport) {
    const sampler = nodeExecution.d3d9SetSamplerStatePostBeginScene;
    assert.equal(nodeStop.fault, null);
    assert.equal(nodeStop.terminal.instructionCount, expectedNext.instructionCount);
    assert.deepEqual(sampler.calls, sampler.checkpointCompletion.calls);
    assert.deepEqual(
      sampler.postSamplerRenderStateRejections,
      sampler.checkpointCompletion.postSamplerRenderStateRejections,
    );
    assert.deepEqual(
      sampler.calls.map((call) => call.sequence),
      [282_851, 282_852],
    );
    assert.deepEqual(
      sampler.calls.map((call) => call.arguments),
      [
        [0x0101_0004, 0, 6, 2],
        [0x0101_0004, 0, 5, 2],
      ],
    );
    for (const call of sampler.calls) {
      assert.equal(call.symbol, "IDirect3DDevice9.SetSamplerState");
      assert.deepEqual(call.result, { kind: "h-result", value: 0 });
      assert.equal(call.graphicsDevice.id, 3);
      assert.equal(call.graphicsDevice.generation, 71);
      assert.equal(call.graphicsDevice.activeScene.sequence, 4);
      assert.equal(call.graphicsDevice.samplerStates.samplerZero.minFilter, "linear");
      assert.equal(call.graphicsDevice.samplerStates.samplerZero.magFilter, "linear");
    }
    assert.deepEqual(
      sampler.postSamplerRenderStateRejections.map((call) => call.arguments),
      [
        [0x0101_0004, 171, 1],
        [0x0101_0004, 20, 6],
        [0x0101_0004, 19, 5],
      ],
    );
    for (const call of sampler.postSamplerRenderStateRejections) {
      assert.equal(call.symbol, "IDirect3DDevice9.SetRenderState");
      assert.deepEqual(call.result, {
        kind: "h-result",
        value: -2_005_530_518,
      });
      assert.equal(call.graphicsDevice, undefined);
      assert.deepEqual(call.guestWrites, []);
    }
    assertD3d9SetSamplerStatePostBeginSceneCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9BeginScenePostPartialDirtyRegionReport) {
    const update = nodeExecution.d3d9UpdateTexturePartialDirtyRegion;
    const begin = nodeExecution.d3d9BeginScenePostPartialDirtyRegion;
    assert.equal(nodeStop.fault, null);
    assert.equal(nodeStop.terminal.instructionCount, expectedNext.instructionCount);
    assert.equal(update.call.sequence, 282_849);
    assert.equal(update.call.symbol, "IDirect3DDevice9.UpdateTexture");
    assert.deepEqual(
      update.call.graphicsTextureUpdate.destination.content.lastUpdate.dirtyRegion,
      {
        kind: "rect",
        rect: { left: 583, top: 530, right: 1_260, bottom: 582 },
      },
    );
    assert.deepEqual(begin.call, begin.checkpointCompletion.call);
    assert.equal(begin.call.sequence, 282_850);
    assert.equal(begin.call.symbol, "IDirect3DDevice9.BeginScene");
    assert.equal(begin.call.vtableSlot, 41);
    assert.deepEqual(begin.call.arguments, [0x0101_0004]);
    assert.deepEqual(begin.call.result, { kind: "h-result", value: 0 });
    assert.deepEqual(begin.call.guestWrites, []);
    assert.equal(begin.call.graphicsDevice.id, 3);
    assert.equal(begin.call.graphicsDevice.comObjectId, "21");
    assert.deepEqual(begin.call.graphicsDevice.activeScene, { sequence: 4 });
    assert.equal(begin.call.graphicsDevice.generation, 71);
    assert.equal(begin.call.graphicsDevice.backBuffer.contentGeneration, 10);
    assert.equal(begin.call.graphicsDevice.backBuffer.uniformClear.sequence, 4);
    assert.equal(begin.call.graphicsDevice.presentedFrame.sequence, 3);
    assertD3d9BeginScenePostPartialDirtyRegionCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else if (d3d9UpdateTexturePartialDirtyRegionReport) {
    const update = nodeExecution.d3d9UpdateTexturePartialDirtyRegion;
    assert.equal(nodeStop.fault, null);
    assert.equal(nodeStop.terminal.instructionCount, expectedNext.instructionCount);
    assert.deepEqual(update.call, update.checkpointCompletion.call);
    assert.equal(update.call.sequence, 282_849);
    assert.equal(update.call.symbol, "IDirect3DDevice9.UpdateTexture");
    assert.equal(update.call.vtableSlot, 31);
    assert.deepEqual(update.call.arguments, [
      0x0101_0004,
      0x0102_0004,
      0x0103_0004,
    ]);
    assert.deepEqual(update.call.result, { kind: "h-result", value: 0 });
    assert.deepEqual(update.call.guestWrites, []);
    assert.equal(
      update.call.graphicsTextureUpdate.source.content.contentGeneration,
      4,
    );
    assert.deepEqual(update.call.graphicsTextureUpdate.source.dirtyRegion, {
      kind: "clean",
    });
    assert.equal(
      update.call.graphicsTextureUpdate.destination.content.contentGeneration,
      4,
    );
    assert.deepEqual(
      update.call.graphicsTextureUpdate.destination.content.lastUpdate.dirtyRegion,
      {
        kind: "rect",
        rect: { left: 583, top: 530, right: 1_260, bottom: 582 },
      },
    );
    assertD3d9UpdateTexturePartialDirtyRegionCheckpoint(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
  } else {
    assert.equal(nodeStop.fault.capabilityCluster, expectedNext.capabilityCluster);
    expectedFaultCode =
      getCursorInfoReport
        ? expectedNext.reason
        : nodeReport.schema ===
            "vine.v86-bgi-outline-text-metrics-224x112-primary"
          ? "endpoint-contract-missing"
          : nodeReport.schema ===
              "vine.v86-bgi-create-dib-section-448x336-primary"
            ? "outline-text-metrics-font-variant-unsupported"
            : nodeReport.schema ===
                "vine.v86-bgi-create-font-a-224x112-primary"
              ? "create-dib-section-variant-unsupported"
              : nodeReport.schema ===
                  "vine.v86-bgi-private-heap-ordinary-segment-growth-primary"
                ? "create-font-a-variant-unsupported"
                : nodeReport.schema === "vine.v86-bgi-delete-file-a-primary"
                  ? "heap-backing-capacity-exceeded"
                  : "endpoint-contract-missing";
    assert.equal(nodeStop.fault.fault.code, expectedFaultCode);
    assert.equal(nodeStop.terminal.instructionCount, expectedNext.instructionCount);
    assert.equal(
      nodeStop.fault.memoryContentSha256,
      expected.memoryContentSha256,
    );
  }
  if (
    !user32GetCursorPosPostTestCooperativeLevelTimerCallbackTimeGetTimeWindowRectReport &&
    !user32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTimeReport &&
    !winmmTimeGetTimePostTestCooperativeLevelTimerCallbackReport &&
    !user32PeekMessageAPostTestCooperativeLevelTimerCallbackReport &&
    !kernel32GetTickCountPostTestCooperativeLevelTimerDispatchReport &&
    !user32DispatchMessageAPostTestCooperativeLevelTimerTranslateReport &&
    !user32TranslateMessagePostTestCooperativeLevelTimerPeekReport &&
    !user32PeekMessageAPostTestCooperativeLevelTimerWaitReport &&
    !kernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArmReport &&
    !kernel32SetWaitableTimerPostTestCooperativeLevelReport &&
    !d3d9TestCooperativeLevelPostTimeGetTimeReport &&
    !winmmTimeGetTimePostPresentReport &&
    !d3d9PresentPostRasterStatusReport &&
    !d3d9GetRasterStatusPostEndSceneReport &&
    !d3d9EndScenePostDrawPrimitiveReport &&
    !d3d9DrawPrimitivePostStreamSourceReport &&
    !d3d9SetStreamSourcePostFvfReport &&
    !d3d9SetFvfPostTextureBindingReport &&
    !d3d9SetTexturePostTextureStageStateReport &&
    !d3d9SetTextureStageStatePostSamplerStateReport &&
    !d3d9SetSamplerStatePostBeginSceneReport &&
    !d3d9BeginScenePostPartialDirtyRegionReport &&
    !d3d9UpdateTexturePartialDirtyRegionReport &&
    getCursorInfoReport
  ) {
    const target = nodeStop.fault.endpoint.target;
    assert.equal(nodeStop.fault.endpoint.kind, "com-method");
    assert.equal(target.interface.name, expectedNext.interface);
    assert.equal(target.interface.id, expectedNext.interfaceId);
    assert.equal(target.object.id, expectedNext.objectId);
    assert.equal(target.method.slot, expectedNext.vtableSlot);
    assert.equal(target.method.name, expectedNext.method);
    assert.equal(hex32(target.method.endpoint), expectedNext.endpointAddress);
    assert.equal(
      hex32(nodeStop.terminal.terminal.returnAddress),
      expectedNext.returnAddress,
    );
    assert.deepEqual(nodeStop.arguments.map(hex32), [
      expectedNext.thisPointer,
      ...expectedNext.arguments,
    ]);
    assertCompletedCapabilityBeforeStructuredStop(
      nodeCheckpoint,
      expectedNext.completion,
      expectedNext,
    );
  } else if (
    !user32GetCursorPosPostTestCooperativeLevelTimerCallbackTimeGetTimeWindowRectReport &&
    !user32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTimeReport &&
    !winmmTimeGetTimePostTestCooperativeLevelTimerCallbackReport &&
    !user32PeekMessageAPostTestCooperativeLevelTimerCallbackReport &&
    !kernel32GetTickCountPostTestCooperativeLevelTimerDispatchReport &&
    !user32DispatchMessageAPostTestCooperativeLevelTimerTranslateReport &&
    !user32TranslateMessagePostTestCooperativeLevelTimerPeekReport &&
    !user32PeekMessageAPostTestCooperativeLevelTimerWaitReport &&
    !kernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArmReport &&
    !kernel32SetWaitableTimerPostTestCooperativeLevelReport &&
    !d3d9TestCooperativeLevelPostTimeGetTimeReport &&
    !winmmTimeGetTimePostPresentReport &&
    !d3d9PresentPostRasterStatusReport &&
    !d3d9GetRasterStatusPostEndSceneReport &&
    !d3d9EndScenePostDrawPrimitiveReport &&
    !d3d9DrawPrimitivePostStreamSourceReport &&
    !d3d9SetStreamSourcePostFvfReport &&
    !d3d9SetFvfPostTextureBindingReport &&
    !d3d9SetTexturePostTextureStageStateReport &&
    !d3d9SetTextureStageStatePostSamplerStateReport &&
    !d3d9SetSamplerStatePostBeginSceneReport &&
    !d3d9BeginScenePostPartialDirtyRegionReport &&
    !d3d9UpdateTexturePartialDirtyRegionReport
  ) {
    const nodeEndpoint = nodeStop.fault.endpoint.endpoint;
    const expectedEndpoint = expectedNext.nextEndpoint;
    assert.equal(nodeStop.fault.endpoint.kind, expectedEndpoint.endpointKind);
    assert.equal(nodeEndpoint.id, expectedEndpoint.endpointId);
    assert.equal(hex32(nodeEndpoint.address), expectedEndpoint.endpointAddress);
    assert.equal(
      optionalHex32(nodeEndpoint.iatAddress),
      expectedEndpoint.iatAddress,
    );
    assert.equal(
      `${nodeEndpoint.module}!${formatRuntimeEndpointSymbol(nodeEndpoint)}`.toLowerCase(),
      expectedEndpoint.api.toLowerCase(),
    );
    assert.equal(
      hex32(nodeStop.terminal.terminal.returnAddress),
      expectedEndpoint.returnAddress,
    );
    assert.deepEqual(nodeStop.arguments.map(hex32), expectedEndpoint.arguments);
    assertCapabilityCheckpoint(nodeCheckpoint, expectedNext);
  }
  assert.equal(nodeCheckpoint.memoryContentSha256, expected.memoryContentSha256);

  assert.equal(browserReport.runtimeProtocol, checkpoint.browserCheckpoint.browserRuntimeProtocol);
  assert.equal(browserReport.schema, BROWSER_BGI_REPORT_SCHEMA);
  assert.equal(browserReport.schemaVersion, BROWSER_BGI_REPORT_SCHEMA_VERSION);
  assert.equal(browser.outcome, "checkpoint-complete");
  assert.equal(browser.evidenceLevel, "summary");
  assert.equal(browser.supportedCallCount, nodeExecution.completedSupportedCalls);
  assert.equal(browser.supportedCalls.length, 0);
  assert.equal(browser.supportedCallSemanticsAlgorithm, "sha256-canonical-json-v1");
  assert.equal(
    browser.supportedCallSemanticsSha256,
    nodeExecution.supportedCallSemanticsSha256,
  );
  assert.equal(browser.instructions, expectedNext.instructionCount);
  assert.equal(
    browser.finalGeneration,
    scheduled.browserBoundaryEvidence.privatePrimaryFinalMemoryGeneration,
  );
  assert.equal(
    browser.mappingDeltas,
    scheduled.browserBoundaryEvidence.privatePrimaryMappingDeltas,
  );
  assert.equal(
    browser.writebackBatches,
    scheduled.browserBoundaryEvidence.privatePrimaryWritebackBatches,
  );
  assert.equal(browser.pendingHostCalls, 0);
  assert.equal(browser.pendingCallbacks, 0);

  const browserUnsupported = browser.firstUnsupported;
  if (
    user32GetCursorPosPostTestCooperativeLevelTimerCallbackTimeGetTimeWindowRectReport
  ) {
    assert.equal(browserUnsupported, null);
    assertPrimaryMainLoopPostCallbackWindowInputScanBatchCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assertPrimaryMainLoopPostCallbackWindowInputScanBatchCheckpoint(
      browser.checkpointCompletion,
      nodeCheckpoint,
      expectedNext.nextEndpoint,
    );
    assert.equal(browser.pointerInput.receivedMessages, 3);
    assert.equal(browser.pointerInput.receipts.length, 3);
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (
    user32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTimeReport
  ) {
    assert.equal(browserUnsupported, null);
    assertUser32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTimeCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
      expectedNext.prerequisiteTimeGetTimeCheckpoint,
    );
    const normalizeCall = (call) => ({
      ...call,
      module: call.module.toLowerCase(),
      guestWrites: call.guestWrites.map(({ address, bytes }) => ({
        address,
        bytes,
      })),
    });
    assert.deepEqual(
      normalizeCall(browser.checkpointCompletion.call),
      normalizeCall(nodeCheckpoint.call),
    );
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (winmmTimeGetTimePostTestCooperativeLevelTimerCallbackReport) {
    assert.equal(browserUnsupported, null);
    assertWinmmTimeGetTimePostTestCooperativeLevelTimerCallbackCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.equal(
      browser.checkpointCompletion.call.module.toLowerCase(),
      nodeCheckpoint.call.module.toLowerCase(),
    );
    assert.deepEqual(
      { ...browser.checkpointCompletion.call, module: undefined },
      { ...nodeCheckpoint.call, module: undefined },
    );
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (user32PeekMessageAPostTestCooperativeLevelTimerCallbackReport) {
    assert.equal(browserUnsupported, null);
    assertUser32PeekMessageAPostTestCooperativeLevelTimerCallbackCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.equal(
      browser.checkpointCompletion.call.module.toLowerCase(),
      nodeCheckpoint.call.module.toLowerCase(),
    );
    assert.deepEqual(
      { ...browser.checkpointCompletion.call, module: undefined },
      { ...nodeCheckpoint.call, module: undefined },
    );
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (kernel32GetTickCountPostTestCooperativeLevelTimerDispatchReport) {
    assert.equal(browserUnsupported, null);
    assertKernel32GetTickCountPostTestCooperativeLevelTimerDispatchCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    for (const field of ["call", "dispatchCompletion"]) {
      assert.equal(
        browser.checkpointCompletion[field].module.toLowerCase(),
        nodeCheckpoint[field].module.toLowerCase(),
      );
      assert.deepEqual(
        { ...browser.checkpointCompletion[field], module: undefined },
        { ...nodeCheckpoint[field], module: undefined },
      );
    }
    assert.deepEqual(
      browser.checkpointCompletion.callbackReturn,
      nodeCheckpoint.callbackReturn,
    );
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (user32DispatchMessageAPostTestCooperativeLevelTimerTranslateReport) {
    assert.equal(browserUnsupported, null);
    assertUser32DispatchMessageAPostTestCooperativeLevelTimerTranslateCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(
      browser.checkpointCompletion.callbackEntry,
      nodeCheckpoint.callbackEntry,
    );
    assert.equal(
      browser.checkpointCompletion.dispatchEntry.module.toLowerCase(),
      nodeCheckpoint.dispatchEntry.module.toLowerCase(),
    );
    assert.deepEqual(
      {
        ...browser.checkpointCompletion.dispatchEntry,
        module: undefined,
      },
      { ...nodeCheckpoint.dispatchEntry, module: undefined },
    );
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (user32TranslateMessagePostTestCooperativeLevelTimerPeekReport) {
    assert.equal(browserUnsupported, null);
    assertUser32TranslateMessagePostTestCooperativeLevelTimerPeekCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(
      normalizeSemanticCall(browser.checkpointCompletion.call),
      normalizeSemanticCall(nodeCheckpoint.call),
    );
    assert.equal(
      browser.checkpointCompletion.call.occurrence,
      nodeCheckpoint.call.occurrence,
    );
    assert.equal(
      browser.checkpointCompletion.call.trapInstructionCount,
      nodeCheckpoint.call.trapInstructionCount,
    );
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (user32PeekMessageAPostTestCooperativeLevelTimerWaitReport) {
    assert.equal(browserUnsupported, null);
    assertUser32PeekMessageAPostTestCooperativeLevelTimerWaitCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(
      normalizeSemanticCall(browser.checkpointCompletion.call),
      normalizeSemanticCall(nodeCheckpoint.call),
    );
    assert.equal(
      browser.checkpointCompletion.call.occurrence,
      nodeCheckpoint.call.occurrence,
    );
    assert.equal(
      browser.checkpointCompletion.call.trapInstructionCount,
      nodeCheckpoint.call.trapInstructionCount,
    );
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (kernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArmReport) {
    assert.equal(browserUnsupported, null);
    assertKernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArmCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(
      normalizeSemanticCall(browser.checkpointCompletion.call),
      normalizeSemanticCall(nodeCheckpoint.call),
    );
    assert.equal(
      browser.checkpointCompletion.call.occurrence,
      nodeCheckpoint.call.occurrence,
    );
    assert.equal(
      browser.checkpointCompletion.call.trapInstructionCount,
      nodeCheckpoint.call.trapInstructionCount,
    );
    assert.deepEqual(browser.checkpointCompletion.pacing, {
      ...nodeCheckpoint.pacing,
      pacing: expectedNext.completion.browserPacingRole,
    });
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (kernel32SetWaitableTimerPostTestCooperativeLevelReport) {
    assert.equal(browserUnsupported, null);
    assertKernel32SetWaitableTimerPostTestCooperativeLevelCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(
      normalizeSemanticCall(browser.checkpointCompletion.call),
      normalizeSemanticCall(nodeCheckpoint.call),
    );
    assert.equal(
      browser.checkpointCompletion.call.occurrence,
      nodeCheckpoint.call.occurrence,
    );
    assert.equal(
      browser.checkpointCompletion.call.trapInstructionCount,
      nodeCheckpoint.call.trapInstructionCount,
    );
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (d3d9TestCooperativeLevelPostTimeGetTimeReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9TestCooperativeLevelPostTimeGetTimeCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(
      normalizeSemanticCall(browser.checkpointCompletion.call),
      normalizeSemanticCall(nodeCheckpoint.call),
    );
    assert.equal(
      browser.checkpointCompletion.call.occurrence,
      nodeCheckpoint.call.occurrence,
    );
    assert.equal(
      browser.checkpointCompletion.call.trapInstructionCount,
      nodeCheckpoint.call.trapInstructionCount,
    );
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (winmmTimeGetTimePostPresentReport) {
    assert.equal(browserUnsupported, null);
    assertWinmmTimeGetTimePostPresentCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(
      normalizeSemanticCall(browser.checkpointCompletion.call),
      normalizeSemanticCall(nodeCheckpoint.call),
    );
    assert.equal(
      browser.checkpointCompletion.call.occurrence,
      nodeCheckpoint.call.occurrence,
    );
    assert.equal(
      browser.checkpointCompletion.call.trapInstructionCount,
      nodeCheckpoint.call.trapInstructionCount,
    );
    assert.equal(
      browser.checkpointCompletion.call.implementationId,
      nodeCheckpoint.call.implementationId,
    );
    assert.deepEqual(
      browser.checkpointCompletion.call.tickCount,
      nodeCheckpoint.call.tickCount,
    );
    assert.equal(browser.d3d9Present.call.sequence, 282_868);
    assert.equal(
      browser.d3d9Present.call.browserPresentation.resource.id,
      "browser-resource:1",
    );
    assert.equal(browser.d3d9Present.call.browserPresentation.frameSequence, 4);
  } else if (d3d9PresentPostRasterStatusReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9PresentPostRasterStatusCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    const {
      browserPresentation: _browserPresentation,
      ...browserLogicalPresentCall
    } = browser.checkpointCompletion.call;
    assert.deepEqual(browserLogicalPresentCall, nodeCheckpoint.call);
  } else if (d3d9GetRasterStatusPostEndSceneReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9GetRasterStatusPostEndSceneCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(browser.checkpointCompletion.call, nodeCheckpoint.call);
  } else if (d3d9EndScenePostDrawPrimitiveReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9EndScenePostDrawPrimitiveCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(browser.checkpointCompletion.call, nodeCheckpoint.call);
  } else if (d3d9DrawPrimitivePostStreamSourceReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9DrawPrimitivePostStreamSourceCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(browser.checkpointCompletion.call, nodeCheckpoint.call);
  } else if (d3d9SetStreamSourcePostFvfReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9SetStreamSourcePostFvfCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(browser.checkpointCompletion.call, nodeCheckpoint.call);
  } else if (d3d9SetFvfPostTextureBindingReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9SetFvfPostTextureBindingCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(browser.checkpointCompletion.call, nodeCheckpoint.call);
  } else if (d3d9SetTexturePostTextureStageStateReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9SetTexturePostTextureStageStateCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(browser.checkpointCompletion.call, nodeCheckpoint.call);
  } else if (d3d9SetTextureStageStatePostSamplerStateReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9SetTextureStageStatePostSamplerStateCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(browser.checkpointCompletion.calls, nodeCheckpoint.calls);
  } else if (d3d9SetSamplerStatePostBeginSceneReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9SetSamplerStatePostBeginSceneCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    assert.deepEqual(browser.checkpointCompletion.calls, nodeCheckpoint.calls);
    assert.deepEqual(
      browser.checkpointCompletion.postSamplerRenderStateRejections,
      nodeCheckpoint.postSamplerRenderStateRejections,
    );
  } else if (d3d9BeginScenePostPartialDirtyRegionReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9BeginScenePostPartialDirtyRegionCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    const {
      occurrence: _nodeOccurrence,
      trapInstructionCount: _nodeTrapInstructionCount,
      ...nodeCheckpointCall
    } = nodeExecution.d3d9BeginScenePostPartialDirtyRegion.call;
    assert.deepEqual(browser.checkpointCompletion.call, nodeCheckpointCall);
  } else if (d3d9UpdateTexturePartialDirtyRegionReport) {
    assert.equal(browserUnsupported, null);
    assertD3d9UpdateTexturePartialDirtyRegionCheckpoint(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext.nextEndpoint,
    );
    const {
      occurrence: _nodeOccurrence,
      trapInstructionCount: _nodeTrapInstructionCount,
      ...nodeCheckpointCall
    } = nodeExecution.d3d9UpdateTexturePartialDirtyRegion.call;
    assert.deepEqual(browser.checkpointCompletion.call, nodeCheckpointCall);
  } else {
    assert.equal(
      browserUnsupported.capabilityCluster,
      expectedNext.capabilityCluster,
    );
    assert.equal(browserUnsupported.fault.code, expectedFaultCode);
    assert.equal(browserUnsupported.instructionCount, expectedNext.instructionCount);
    assert.equal(
      browserUnsupported.memoryContentSha256,
      expected.memoryContentSha256,
    );
  }
  if (
    !user32GetCursorPosPostTestCooperativeLevelTimerCallbackTimeGetTimeWindowRectReport &&
    !user32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTimeReport &&
    !winmmTimeGetTimePostTestCooperativeLevelTimerCallbackReport &&
    !user32PeekMessageAPostTestCooperativeLevelTimerCallbackReport &&
    !kernel32GetTickCountPostTestCooperativeLevelTimerDispatchReport &&
    !user32DispatchMessageAPostTestCooperativeLevelTimerTranslateReport &&
    !user32TranslateMessagePostTestCooperativeLevelTimerPeekReport &&
    !user32PeekMessageAPostTestCooperativeLevelTimerWaitReport &&
    !kernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArmReport &&
    !kernel32SetWaitableTimerPostTestCooperativeLevelReport &&
    !d3d9TestCooperativeLevelPostTimeGetTimeReport &&
    !winmmTimeGetTimePostPresentReport &&
    !d3d9PresentPostRasterStatusReport &&
    !d3d9GetRasterStatusPostEndSceneReport &&
    !d3d9EndScenePostDrawPrimitiveReport &&
    !d3d9DrawPrimitivePostStreamSourceReport &&
    !d3d9SetStreamSourcePostFvfReport &&
    !d3d9SetFvfPostTextureBindingReport &&
    !d3d9SetTexturePostTextureStageStateReport &&
    !d3d9SetTextureStageStatePostSamplerStateReport &&
    !d3d9SetSamplerStatePostBeginSceneReport &&
    !d3d9BeginScenePostPartialDirtyRegionReport &&
    !d3d9UpdateTexturePartialDirtyRegionReport &&
    getCursorInfoReport
  ) {
    assert.equal(browserUnsupported.endpoint.kind, "com-method");
    assert.equal(
      hex32(browserUnsupported.endpoint.endpointAddress),
      expectedNext.endpointAddress,
    );
    assert.equal(
      browserUnsupported.endpoint.target.interface,
      expectedNext.interface,
    );
    assert.equal(
      browserUnsupported.endpoint.target.interfaceId,
      expectedNext.interfaceId,
    );
    assert.equal(browserUnsupported.endpoint.target.objectId, expectedNext.objectId);
    assert.equal(
      browserUnsupported.endpoint.target.vtableSlot,
      expectedNext.vtableSlot,
    );
    assert.equal(browserUnsupported.endpoint.target.method, expectedNext.method);
    assert.equal(hex32(browserUnsupported.fault.callSite), expectedNext.returnAddress);
    assertCompletedCapabilityBeforeStructuredStop(
      browser.checkpointCompletion,
      expectedNext.completion,
      expectedNext,
    );
  } else if (
    !user32GetCursorPosPostTestCooperativeLevelTimerCallbackTimeGetTimeWindowRectReport &&
    !user32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTimeReport &&
    !winmmTimeGetTimePostTestCooperativeLevelTimerCallbackReport &&
    !user32PeekMessageAPostTestCooperativeLevelTimerCallbackReport &&
    !kernel32GetTickCountPostTestCooperativeLevelTimerDispatchReport &&
    !user32DispatchMessageAPostTestCooperativeLevelTimerTranslateReport &&
    !user32TranslateMessagePostTestCooperativeLevelTimerPeekReport &&
    !user32PeekMessageAPostTestCooperativeLevelTimerWaitReport &&
    !kernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArmReport &&
    !kernel32SetWaitableTimerPostTestCooperativeLevelReport &&
    !d3d9TestCooperativeLevelPostTimeGetTimeReport &&
    !winmmTimeGetTimePostPresentReport &&
    !d3d9PresentPostRasterStatusReport &&
    !d3d9GetRasterStatusPostEndSceneReport &&
    !d3d9EndScenePostDrawPrimitiveReport &&
    !d3d9DrawPrimitivePostStreamSourceReport &&
    !d3d9SetStreamSourcePostFvfReport &&
    !d3d9SetFvfPostTextureBindingReport &&
    !d3d9SetTexturePostTextureStageStateReport &&
    !d3d9SetTextureStageStatePostSamplerStateReport &&
    !d3d9SetSamplerStatePostBeginSceneReport &&
    !d3d9BeginScenePostPartialDirtyRegionReport &&
    !d3d9UpdateTexturePartialDirtyRegionReport
  ) {
    const expectedEndpoint = expectedNext.nextEndpoint;
    assert.equal(browserUnsupported.endpoint.id, expectedEndpoint.endpointId);
    assert.equal(
      hex32(browserUnsupported.endpoint.address),
      expectedEndpoint.endpointAddress,
    );
    assert.equal(
      optionalHex32(browserUnsupported.endpoint.iatAddress),
      expectedEndpoint.iatAddress,
    );
    assert.equal(
      `${browserUnsupported.endpoint.module}!${formatRuntimeEndpointSymbol(
        browserUnsupported.endpoint,
      )}`.toLowerCase(),
      expectedEndpoint.api.toLowerCase(),
    );
    assertCapabilityCheckpoint(browser.checkpointCompletion, expectedNext);
  }
  assert.equal(
    browser.checkpointCompletion.memoryContentSha256,
    expected.memoryContentSha256,
  );
  assert.equal(
    browser.checkpointCompletion.memoryContentSha256,
    nodeCheckpoint.memoryContentSha256,
    "Node/browser final memory mismatch",
  );
  assert.equal(
    nodeReport.synchronization.finalGeneration,
    scheduled.nodePrimaryEvidence.finalMemoryGeneration,
  );
  assert.equal(
    nodeReport.synchronization.writebackBatches,
    scheduled.nodePrimaryEvidence.writebackBatches,
  );
  assert.equal(
    nodeReport.synchronization.hostDeltaBatches,
    scheduled.nodePrimaryEvidence.hostDeltaBatches,
  );
  assert.equal(
    nodeReport.synchronization.activeCallbackDepth,
    user32DispatchMessageAPostTestCooperativeLevelTimerTranslateReport
      ? expectedNext.completion.activeCallbackDepthAtEntry
      : 0,
  );

  assert.equal(browserReport.cancellation.report.pendingHostCalls, 0);
  assert.equal(browserReport.cancellation.report.pendingCallbacks, 0);
  assert.equal(browserReport.watchdog.report.pendingHostCalls, 0);
  assert.equal(browserReport.watchdog.report.pendingCallbacks, 0);
  assertExecutionStateReport(browser, "success");
  assertExecutionStateReport(browserReport.cancellation.report, "cancellation");
  assertExecutionStateReport(browserReport.watchdog.report, "watchdog");
  for (const report of [
    browser,
    browserReport.cancellation.report,
    browserReport.watchdog.report,
  ]) {
    assert.deepEqual(report.executionEngine, executionEngine);
  }
  assert.equal(
    browser.executionEngineLinearMemoryBytes,
    expected.executionEngineLinearMemoryBytes,
  );
}

function assertReportSemantics({
  checkpoint,
  nodeReport,
  browserReport,
  policy,
  executionEngine,
  source,
}) {
  if (isScheduledSharedClockNodeReport(nodeReport)) {
    assertScheduledSharedClockReportSemantics({
      checkpoint,
      nodeReport,
      browserReport,
      policy,
      executionEngine,
      source,
    });
    return;
  }
  const active = checkpoint.activeRuntimeCheckpoint;
  const expectedBrowser = checkpoint.browserCheckpoint.bgiRuntimeEvidence;
  const expectedNext = checkpoint.nextStructuredStop;
  const expectedPacingCount =
    expectedBrowser.waitForSingleObjectExPacingRequestIds.length;
  const nodePacing = projectWaitableTimerPacingRequests(
    nodeReport.waitableTimerWait,
  );
  const nodePacingRequests = nodePacing.sampledRequests;
  const sourceIdentity = { bytes: source.bytes, sha256: source.sha256 };
  assert.deepEqual(nodeReport.source, sourceIdentity, "Node source fingerprint mismatch");
  assert.deepEqual(browserReport.source, sourceIdentity, "browser source fingerprint mismatch");
  assert.deepEqual(
    nodeReport.contract.compatibilityPolicy,
    policy,
    "runtime policy mismatch",
  );
  assert.equal(nodeReport.contract.enabledCapability, active.profile);
  assert.equal(nodeReport.contract.evidenceLevel, "summary");
  assert.equal(nodeReport.supportedContinuation.length, 0);
  assert.equal(nodeReport.supportedCallCount, active.genericDispatcherCallOccurrenceCount);
  assert.equal(nodeReport.supportedCallSemanticsAlgorithm, "sha256-canonical-json-v1");
  assert.match(nodeReport.supportedCallSemanticsSha256, /^[0-9a-f]{64}$/u);
  assert.equal(
    nodeReport.supportedCallSemanticsSha256,
    expectedBrowser.callSemanticsSha256,
  );
  assert.equal(
    nodeReport.process.implementedApiContracts,
    active.genericDispatcherContractCount,
  );
  assert.equal(
    nodeReport.guestThreadBootstrap.schedulingPolicy,
    expectedBrowser.guestThreadSchedulingPolicy,
  );
  assert.deepEqual(
    nodeReport.threadExecution.registeredThreadIds,
    expectedBrowser.registeredThreadIds,
  );
  assert.deepEqual(
    nodeReport.threadExecution.executedThreadIds,
    expectedBrowser.executedThreadIds,
  );
  assert.equal(
    nodeReport.threadExecution.contextSwitches,
    expectedBrowser.contextSwitches,
  );
  assert.equal(
    nodeReport.waitableTimerWait.pacingService,
    expectedBrowser.waitForSingleObjectExPacingService,
  );
  assert.equal(
    nodeReport.waitableTimerWait.pacingPayloadVersion,
    expectedBrowser.waitForSingleObjectExPacingPayloadVersion,
  );
  assert.equal(
    nodePacing.totalCount,
    expectedBrowser.waitForSingleObjectExBrowserPacingRequests,
  );
  assert.equal(nodePacingRequests.length, expectedPacingCount);
  assert.deepEqual(
    nodePacingRequests.map((request) => request.requestId),
    expectedBrowser.waitForSingleObjectExPacingRequestIds,
  );
  assert.deepEqual(
    nodePacingRequests.map((request) => request.registrationId),
    expectedBrowser.waitForSingleObjectExPacingRegistrationIds,
  );
  assert.deepEqual(
    nodePacingRequests.map((request) => request.delayMilliseconds),
    expectedBrowser.waitForSingleObjectExPacingDelayMilliseconds,
  );
  assert.deepEqual(
    nodePacingRequests.map((request) => request.timerId),
    expectedBrowser.waitForSingleObjectExPacingTimerIds,
  );
  assert.deepEqual(
    nodePacingRequests.map((request) => request.targetTime100ns),
    expectedBrowser.waitForSingleObjectExPacingTargetTimes100ns,
  );
  assert.deepEqual(
    nodePacingRequests.map((request) => request.timerClockGeneration),
    expectedBrowser.waitForSingleObjectExPacingClockGenerations,
  );
  assert.deepEqual(
    nodePacingRequests.map((request) => request.armGeneration),
    expectedBrowser.waitForSingleObjectExPacingArmGenerations,
  );
  assert.deepEqual(
    nodePacingRequests.map((request) => request.outcome),
    Array(expectedPacingCount).fill(expectedBrowser.waitForSingleObjectExPacingOutcome),
  );
  assert.deepEqual(
    nodePacingRequests.map((request) => request.pacing),
    Array(expectedPacingCount).fill(expectedBrowser.waitForSingleObjectExNodePacingMode),
  );
  assert.equal(
    nodeReport.waitableTimerWait.logicalCompletionAuthority,
    expectedBrowser.waitForSingleObjectExLogicalCompletionAuthority,
  );
  assert.equal(
    nodeReport.waitableTimerWait.pageLifecyclePolicy,
    expectedBrowser.waitForSingleObjectExPageLifecyclePolicy,
  );
  assert.equal(nodeReport.waitableTimerWait.nodeTimerCreated, false);
  assert.equal(nodeReport.waitableTimerWait.browserTimerRequired, true);
  assert.equal(nodeReport.waitableTimerWait.hostClockImported, false);
  assert.equal(nodeReport.synchronization.finalGeneration, expectedBrowser.nodeFinalGeneration);
  if (expectedNext.kind === "capability-checkpoint") {
    assert.equal(nodeReport.firstUnsupported, null);
    assertCapabilityCheckpoint(nodeReport.checkpointCompletion, expectedNext, {
      requireWriteReceipt: true,
    });
    assert.equal(
      nodeReport.checkpointCompletion.memoryContentSha256,
      expectedBrowser.memoryContentSha256,
    );
  } else {
    assert.equal(nodeReport.checkpointCompletion ?? null, null);
    assert.equal(nodeReport.firstUnsupported.instructionCount, expectedNext.instructionCount);
    assert.equal(
      nodeReport.firstUnsupported.memoryContentSha256,
      expectedBrowser.memoryContentSha256,
    );
    assert.equal(nodeReport.firstUnsupported.capabilityCluster, expectedNext.capabilityCluster);
    assert.equal(nodeReport.firstUnsupported.fault.code, expectedNext.reason);
  }

  if (expectedNext.kind === "execution-fault") {
    assert.equal(nodeReport.firstUnsupported.endpoint, null);
    assert.equal(
      nodeReport.firstUnsupported.fault.instructionPointer,
      Number.parseInt(expectedNext.fault.instructionPointer, 16),
    );
    assert.equal(
      nodeReport.firstUnsupported.fault.previousInstructionPointer,
      Number.parseInt(expectedNext.fault.previousInstructionPointer, 16),
    );
    assert.equal(
      nodeReport.firstUnsupported.fault.faultAddress,
      Number.parseInt(expectedNext.fault.faultAddress, 16),
    );
    assert.equal(
      nodeReport.firstUnsupported.fault.pageFaultErrorCode,
      expectedNext.fault.pageFaultErrorCode,
    );
  } else if (expectedNext.kind === "com-method") {
    const nodeEndpoint = nodeReport.firstUnsupported.endpoint;
    const nodeTarget = nodeEndpoint.target;
    assert.equal(nodeEndpoint.kind, "com-method");
    assert.equal(nodeTarget.interface.name, expectedNext.interface);
    assert.equal(nodeTarget.method.slot, expectedNext.vtableSlot);
    assert.equal(nodeTarget.method.name, expectedNext.method);
    assert.equal(hex32(nodeTarget.method.endpoint), expectedNext.endpointAddress);
    assert.equal(
      hex32(nodeReport.firstUnsupported.terminal.stackWords[1]),
      expectedNext.thisPointer,
    );
    assert.deepEqual(
      nodeReport.firstUnsupported.terminal.stackWords
        .slice(2, 2 + expectedNext.arguments.length)
        .map(hex32),
      expectedNext.arguments,
    );
    assert.equal(
      hex32(nodeReport.firstUnsupported.terminal.returnAddress),
      expectedNext.returnAddress,
    );
  } else if (expectedNext.kind === "api-endpoint") {
    const nodeEndpoint = nodeReport.firstUnsupported.endpoint.endpoint;
    assert.equal(nodeReport.firstUnsupported.endpoint.kind, expectedNext.endpointKind);
    assert.equal(nodeEndpoint.id, expectedNext.endpointId);
    assert.equal(hex32(nodeEndpoint.address), expectedNext.endpointAddress);
    assert.equal(optionalHex32(nodeEndpoint.iatAddress), expectedNext.iatAddress);
    assert.equal(
      `${nodeEndpoint.module}!${formatRuntimeEndpointSymbol(
        nodeEndpoint,
      )}`.toLowerCase(),
      expectedNext.api.toLowerCase(),
    );
    assert.equal(
      hex32(nodeReport.firstUnsupported.terminal.returnAddress),
      expectedNext.returnAddress,
    );
  }

  assert.equal(browserReport.runtimeProtocol, checkpoint.browserCheckpoint.browserRuntimeProtocol);
  assert.equal(browserReport.schema, BROWSER_BGI_REPORT_SCHEMA);
  assert.equal(browserReport.schemaVersion, BROWSER_BGI_REPORT_SCHEMA_VERSION);
  assert.equal(
    browserReport.success.report.outcome,
    expectedNext.kind === "capability-checkpoint"
      ? "checkpoint-complete"
      : "structured-stop",
  );
  assert.equal(browserReport.success.report.evidenceLevel, "summary");
  assert.equal(
    browserReport.success.report.supportedCallCount,
    active.genericDispatcherCallOccurrenceCount,
  );
  assert.equal(browserReport.success.report.supportedCalls.length, 0);
  assert.equal(
    browserReport.success.report.supportedCallSemanticsAlgorithm,
    "sha256-canonical-json-v1",
  );
  assert.equal(
    browserReport.success.report.supportedCallSemanticsSha256,
    nodeReport.supportedCallSemanticsSha256,
    "Node/browser call semantics digest mismatch",
  );
  assert.equal(browserReport.success.report.instructions, expectedNext.instructionCount);
  assert.equal(browserReport.success.report.finalGeneration, expectedBrowser.finalGeneration);
  const browserUnsupported = browserReport.success.report.firstUnsupported;
  const browserCheckpoint = browserReport.success.report.checkpointCompletion;
  if (expectedNext.kind === "capability-checkpoint") {
    assert.equal(browserUnsupported, null);
    assertCapabilityCheckpoint(browserCheckpoint, expectedNext);
    assert.equal(
      browserCheckpoint.memoryContentSha256,
      expectedBrowser.memoryContentSha256,
    );
  } else {
    assert.equal(browserCheckpoint, null);
    assert.equal(
      browserUnsupported.memoryContentSha256,
      expectedBrowser.memoryContentSha256,
    );
    assert.equal(browserUnsupported.capabilityCluster, expectedNext.capabilityCluster);
    assert.equal(browserUnsupported.fault.code, expectedNext.reason);
  }
  if (expectedNext.kind === "execution-fault") {
    assert.equal(browserUnsupported.endpoint, null);
    assert.equal(
      browserUnsupported.fault.instructionPointer,
      Number.parseInt(expectedNext.fault.instructionPointer, 16),
    );
    assert.equal(
      browserUnsupported.fault.previousInstructionPointer,
      Number.parseInt(expectedNext.fault.previousInstructionPointer, 16),
    );
    assert.equal(
      browserUnsupported.fault.faultAddress,
      Number.parseInt(expectedNext.fault.faultAddress, 16),
    );
    assert.equal(
      browserUnsupported.fault.pageFaultErrorCode,
      expectedNext.fault.pageFaultErrorCode,
    );
  } else if (expectedNext.kind === "com-method") {
    assert.equal(browserUnsupported.endpoint.kind, "com-method");
    assert.equal(
      hex32(browserUnsupported.endpoint.endpointAddress),
      expectedNext.endpointAddress,
    );
    assert.equal(browserUnsupported.endpoint.target.interface, expectedNext.interface);
    assert.equal(browserUnsupported.endpoint.target.vtableSlot, expectedNext.vtableSlot);
    assert.equal(browserUnsupported.endpoint.target.method, expectedNext.method);
    assert.equal(
      hex32(browserUnsupported.endpoint.target.endpointAddress),
      expectedNext.endpointAddress,
    );
    assert.equal(hex32(browserUnsupported.fault.callSite), expectedNext.returnAddress);
  } else if (expectedNext.kind === "api-endpoint") {
    assert.equal(browserUnsupported.endpoint.id, expectedNext.endpointId);
    assert.equal(hex32(browserUnsupported.endpoint.address), expectedNext.endpointAddress);
    assert.equal(
      optionalHex32(browserUnsupported.endpoint.iatAddress),
      expectedNext.iatAddress,
    );
    assert.equal(
      `${browserUnsupported.endpoint.module}!${formatRuntimeEndpointSymbol(
        browserUnsupported.endpoint,
      )}`.toLowerCase(),
      expectedNext.api.toLowerCase(),
    );
  }
  assert.equal(browserReport.success.report.pendingHostCalls, 0);
  assert.equal(browserReport.success.report.pendingCallbacks, 0);
  assert.deepEqual(
    browserReport.success.report.registeredThreadIds,
    expectedBrowser.registeredThreadIds,
  );
  assert.deepEqual(
    browserReport.success.report.executedThreadIds,
    expectedBrowser.executedThreadIds,
  );
  assert.equal(
    browserReport.success.report.contextSwitches,
    expectedBrowser.contextSwitches,
  );
  assert.deepEqual(browserReport.success.report.timerPacing, {
    requests: expectedBrowser.waitForSingleObjectExBrowserPacingRequests,
    completions: expectedBrowser.waitForSingleObjectExBrowserPacingCompletions,
    cancellations: expectedBrowser.waitForSingleObjectExBrowserPacingCancellations,
    freezes: 0,
    resumes: 0,
    staleCallbacks: expectedBrowser.waitForSingleObjectExBrowserPacingStaleCallbacks,
    boundedCatchUps: 0,
    pending: expectedBrowser.waitForSingleObjectExBrowserPacingPending,
  });
  assert.equal(browserReport.cancellation.report.pendingHostCalls, 0);
  assert.equal(browserReport.cancellation.report.pendingCallbacks, 0);
  assert.equal(browserReport.watchdog.report.pendingHostCalls, 0);
  assert.equal(browserReport.watchdog.report.pendingCallbacks, 0);
  assertExecutionStateReport(browserReport.success.report, "success");
  assertExecutionStateReport(browserReport.cancellation.report, "cancellation");
  assertExecutionStateReport(browserReport.watchdog.report, "watchdog");
  for (const report of [
    browserReport.success.report,
    browserReport.cancellation.report,
    browserReport.watchdog.report,
  ]) {
    assert.deepEqual(report.executionEngine, executionEngine);
  }
  assert.equal(
    browserReport.success.report.executionEngineLinearMemoryBytes,
    expectedBrowser.executionEngineLinearMemoryBytes,
  );
  assert.equal(
    expectedNext.kind === "capability-checkpoint"
      ? nodeReport.checkpointCompletion.memoryContentSha256
      : nodeReport.firstUnsupported.memoryContentSha256,
    expectedNext.kind === "capability-checkpoint"
      ? browserCheckpoint.memoryContentSha256
      : browserUnsupported.memoryContentSha256,
    "Node/browser final memory mismatch",
  );
}

function invocation(allowedDirtyPaths, outputRelative) {
  const parts = ["npm run check:private-target"];
  if (allowedDirtyPaths.length > 0 || outputRelative !== "docs/private-target-evidence.json") {
    parts.push("--");
  }
  for (const entry of allowedDirtyPaths) {
    parts.push(`--allow-dirty ${JSON.stringify(entry)}`);
  }
  if (outputRelative !== "docs/private-target-evidence.json") {
    parts.push(`--output ${JSON.stringify(outputRelative)}`);
  }
  return parts.join(" ");
}

const options = parseArguments(process.argv.slice(2));
const checkpoint = loadCurrentCheckpoint(path.join(root, "docs", "current-checkpoint.json"));
const headCommit = git(["rev-parse", "HEAD"]).trim();
const implementationTree = git(["rev-parse", "HEAD^{tree}"]).trim();
const beforeDirtyPaths = dirtyPaths();
const allowed = new Set([...options.allowedDirtyPaths, options.outputRelative]);
const disallowedDirtyPaths = beforeDirtyPaths.filter(
  (entry) => !allowed.has(entry) || isProtectedInput(entry),
);
if (disallowedDirtyPaths.length > 0) {
  fail(
    `execution inputs are dirty:\n${disallowedDirtyPaths.map((entry) => `- ${entry}`).join("\n")}`,
  );
}

const bgiPath = process.env.VINE_BGI_EXE ?? defaultBgi;
if (bgiPath === null || !fs.existsSync(bgiPath)) {
  fail("BGI.exe is unavailable; set VINE_BGI_EXE to the private target path");
}
const bgiBytes = fs.readFileSync(bgiPath);
const source = {
  fileName: path.basename(bgiPath),
  bytes: bgiBytes.byteLength,
  sha256: sha256(bgiBytes),
};
const expectedSource = projectPrivateTargetExpectedBrowserEvidence(checkpoint);
if (
  source.bytes !== expectedSource.sourceBytes ||
  source.sha256 !== expectedSource.sourceSha256
) {
  fail(
    `BGI source fingerprint mismatch: expected ${expectedSource.sourceBytes} bytes/${expectedSource.sourceSha256}, got ${source.bytes} bytes/${source.sha256}`,
  );
}

console.log(
  `private target preflight passed: commit ${headCommit.slice(0, 12)}, ` +
    `${beforeDirtyPaths.length} recorded dirty path(s), source ${source.sha256}`,
);
if (process.platform === "win32") {
  runInherited(process.env.ComSpec ?? "cmd.exe", [
    "/d",
    "/s",
    "/c",
    "npm run check:runtime:bgi-browser",
  ]);
} else {
  runInherited("npm", ["run", "check:runtime:bgi-browser"]);
}

if (git(["rev-parse", "HEAD"]).trim() !== headCommit) {
  fail("HEAD changed while the private target gate was running");
}
const afterDirtyPaths = dirtyPaths();
const afterDisallowed = afterDirtyPaths.filter(
  (entry) => !allowed.has(entry) || isProtectedInput(entry),
);
if (afterDisallowed.length > 0) {
  fail(
    `the gate changed execution inputs:\n${afterDisallowed.map((entry) => `- ${entry}`).join("\n")}`,
  );
}

const browserReportPath = path.join(root, ".work", "browser-runtime", "bgi-browser-report.json");
const browserReport = readJson(browserReportPath);
const nodeReportPath = path.resolve(root, browserReport.nodeOracle.path);
const nodeReport = readJson(nodeReportPath);
const toolchainReportPath = path.join(root, ".work", "toolchain-spike", "report.json");
const v86AdapterReportPath = path.join(root, ".work", "v86-adapter-spike", "report.json");
const browserBuildManifestPath = path.join(
  root,
  ".work",
  "browser-runtime",
  "build-manifest.json",
);
const toolchainReport = readJson(toolchainReportPath);
const v86AdapterReport = readJson(v86AdapterReportPath);
const browserBuildManifest = readJson(browserBuildManifestPath);
const policyPath = path.join(
  root,
  checkpoint.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source,
);
const policy = readJson(policyPath);
const executionEngineProfilePath = path.join(
  root,
  checkpoint.browserCheckpoint.executionEngineProfile,
);
const executionEngineProfile = readJson(executionEngineProfilePath);
const builtAdapterArtifact = browserBuildManifest.assets["v86-adapter.wasm"];
const executionEngine = executionEngineIdentity(
  executionEngineProfile,
  builtAdapterArtifact,
);
assertReportSemantics({
  checkpoint,
  nodeReport,
  browserReport,
  policy,
  executionEngine,
  source,
});

const runtimeCoreWasmPath = path.join(root, ".work", "browser-runtime", "runtime-core.wasm");
const v86AdapterWasmPath = path.join(root, ".work", "browser-runtime", "v86-adapter.wasm");
const artifacts = {
  nodeReport: artifact(nodeReportPath),
  browserReport: artifact(browserReportPath),
  toolchainReport: artifact(toolchainReportPath),
  v86AdapterReport: artifact(v86AdapterReportPath),
  browserBuildManifest: artifact(browserBuildManifestPath),
  runtimeCoreWasm: artifact(runtimeCoreWasmPath),
  v86AdapterWasm: artifact(v86AdapterWasmPath),
  cargoLock: artifact(path.join(root, "Cargo.lock")),
  npmLock: artifact(path.join(root, "package-lock.json")),
};
assert(
  artifacts.nodeReport.bytes <= maximumSemanticReportBytes,
  `summary Node report exceeds ${maximumSemanticReportBytes} bytes`,
);
assert(
  artifacts.browserReport.bytes <= maximumSemanticReportBytes,
  `summary browser report exceeds ${maximumSemanticReportBytes} bytes`,
);
assert.deepEqual(
  browserBuildManifest.assets["runtime-core.wasm"],
  { bytes: artifacts.runtimeCoreWasm.bytes, sha256: artifacts.runtimeCoreWasm.sha256 },
);
assert.equal(browserBuildManifest.schemaVersion, 2);
assert.deepEqual(browserBuildManifest.executionEngine, {
  profile: checkpoint.browserCheckpoint.executionEngineProfile,
  ...executionEngine,
});
assert.deepEqual(
  browserBuildManifest.assets["v86-adapter.wasm"],
  { bytes: artifacts.v86AdapterWasm.bytes, sha256: artifacts.v86AdapterWasm.sha256 },
);
assert.equal(browserReport.nodeOracle.bytes, artifacts.nodeReport.bytes);
assert.equal(browserReport.nodeOracle.sha256, artifacts.nodeReport.sha256);

const expectedBrowser = projectPrivateTargetExpectedBrowserEvidence(checkpoint);
const benchmark = checkpoint.browserCheckpoint.bgiBenchmarkEvidence;
const actualAllowedDirtyPaths = beforeDirtyPaths.filter((entry) => allowed.has(entry));
const scheduledNodeReport = isScheduledSharedClockNodeReport(nodeReport);
const completedCapabilityBeforeStructuredStop =
  scheduledNodeReport &&
  nodeReport.schema === "vine.v86-bgi-get-cursor-info-primary" &&
  checkpoint.nextStructuredStop.kind === "com-method";
const nodeCheckpointCompletion =
  checkpoint.nextStructuredStop.kind !== "capability-checkpoint"
    ? null
    : scheduledNodeReport
      ? projectScheduledSharedClockCheckpointCompletion(
          nodeReport,
          checkpoint.nextStructuredStop,
        )
      : nodeReport.checkpointCompletion;
const nodeSupportedCalls = scheduledNodeReport
  ? nodeReport.execution.completedSupportedCalls
  : nodeReport.supportedCallCount;
const nodeCallSemanticsSha256 = scheduledNodeReport
  ? nodeReport.execution.supportedCallSemanticsSha256
  : nodeReport.supportedCallSemanticsSha256;
const nodeInstructions = scheduledNodeReport
  ? nodeReport.stop.terminal.instructionCount
  : checkpoint.nextStructuredStop.kind === "capability-checkpoint"
    ? nodeReport.checkpointCompletion.instructionCount
    : nodeReport.firstUnsupported.instructionCount;
const nodeMemoryContentSha256 = scheduledNodeReport
  ? nodeReport.schema ===
    "vine.v86-bgi-user32-get-cursor-pos-post-test-cooperative-level-timer-callback-time-get-time-window-rect-primary"
    ? nodeReport.execution
        .user32GetCursorPosPostTestCooperativeLevelTimerCallbackTimeGetTimeWindowRect
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-user32-get-window-rect-post-test-cooperative-level-timer-callback-time-get-time-primary"
    ? nodeReport.execution
        .user32GetWindowRectPostTestCooperativeLevelTimerCallbackTimeGetTime
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-winmm-time-get-time-post-test-cooperative-level-timer-callback-primary"
    ? nodeReport.execution
        .winmmTimeGetTimePostTestCooperativeLevelTimerCallback
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
  "vine.v86-bgi-user32-peek-message-a-post-test-cooperative-level-timer-callback-primary"
    ? nodeReport.execution
        .user32PeekMessageAPostTestCooperativeLevelTimerCallback
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-kernel32-get-tick-count-post-test-cooperative-level-timer-dispatch-primary"
    ? nodeReport.execution
        .kernel32GetTickCountPostTestCooperativeLevelTimerDispatch
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-user32-dispatch-message-a-post-test-cooperative-level-timer-translate-primary"
    ? nodeReport.execution
        .user32DispatchMessageAPostTestCooperativeLevelTimerTranslate
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-user32-translate-message-post-test-cooperative-level-timer-peek-primary"
    ? nodeReport.execution
        .user32TranslateMessagePostTestCooperativeLevelTimerPeek
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-user32-peek-message-a-post-test-cooperative-level-timer-wait-primary"
    ? nodeReport.execution.user32PeekMessageAPostTestCooperativeLevelTimerWait
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-kernel32-wait-for-single-object-ex-post-test-cooperative-level-timer-arm-primary"
    ? nodeReport.execution
        .kernel32WaitForSingleObjectExPostTestCooperativeLevelTimerArm
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-kernel32-set-waitable-timer-post-test-cooperative-level-primary"
    ? nodeReport.execution.kernel32SetWaitableTimerPostTestCooperativeLevel
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-d3d9-test-cooperative-level-post-time-get-time-primary"
    ? nodeReport.execution.d3d9TestCooperativeLevelPostTimeGetTime
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-winmm-time-get-time-post-present-primary"
    ? nodeReport.execution.winmmTimeGetTimePostPresent.checkpointCompletion
        .memoryContentSha256
    : nodeReport.schema ===
      "vine.v86-bgi-d3d9-present-post-raster-status-primary"
    ? nodeReport.execution.d3d9PresentPostRasterStatus.checkpointCompletion
        .memoryContentSha256
    : nodeReport.schema ===
      "vine.v86-bgi-d3d9-get-raster-status-post-end-scene-primary"
    ? nodeReport.execution.d3d9GetRasterStatusPostEndScene
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
      "vine.v86-bgi-d3d9-end-scene-post-draw-primitive-primary"
    ? nodeReport.execution.d3d9EndScenePostDrawPrimitive
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
      "vine.v86-bgi-d3d9-draw-primitive-post-stream-source-primary"
    ? nodeReport.execution.d3d9DrawPrimitivePostStreamSource
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
      "vine.v86-bgi-d3d9-set-stream-source-post-fvf-primary"
    ? nodeReport.execution.d3d9SetStreamSourcePostFvf.checkpointCompletion
        .memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-fvf-post-texture-binding-primary"
    ? nodeReport.execution.d3d9SetFvfPostTextureBinding.checkpointCompletion
        .memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-texture-post-texture-stage-state-primary"
    ? nodeReport.execution.d3d9SetTexturePostTextureStageState
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-texture-stage-state-post-sampler-state-primary"
    ? nodeReport.execution.d3d9SetTextureStageStatePostSamplerState
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-d3d9-set-sampler-state-post-begin-scene-primary"
    ? nodeReport.execution.d3d9SetSamplerStatePostBeginScene
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
    "vine.v86-bgi-d3d9-begin-scene-post-partial-dirty-region-primary"
    ? nodeReport.execution.d3d9BeginScenePostPartialDirtyRegion
        .checkpointCompletion.memoryContentSha256
    : nodeReport.schema ===
        "vine.v86-bgi-d3d9-update-texture-partial-dirty-region-primary"
    ? nodeReport.execution.d3d9UpdateTexturePartialDirtyRegion
        .checkpointCompletion.memoryContentSha256
    : nodeReport.stop.fault.memoryContentSha256
  : checkpoint.nextStructuredStop.kind === "capability-checkpoint"
    ? nodeReport.checkpointCompletion.memoryContentSha256
    : nodeReport.firstUnsupported.memoryContentSha256;
const evidence = finalizePrivateTargetEvidence({
  schema: PRIVATE_TARGET_EVIDENCE_SCHEMA,
  schemaVersion: PRIVATE_TARGET_EVIDENCE_VERSION,
  status: "passed",
  recordedAt: new Date().toISOString(),
  repository: {
    implementationCommit: headCommit,
    implementationTree,
    headCommit,
    dirty: beforeDirtyPaths.length > 0,
    dirtyPaths: beforeDirtyPaths,
    allowedDirtyPaths: actualAllowedDirtyPaths,
    executionInputsClean: true,
    disallowedDirtyPaths: [],
  },
  command: {
    canonical: "npm run check:private-target",
    invocation: invocation(options.allowedDirtyPaths, options.outputRelative),
    semanticGate: "required",
    performanceGate: "separate-observation",
  },
  source,
  profile: {
    currentCheckpointSchema: checkpoint.schema,
    currentCheckpointSchemaVersion: checkpoint.schemaVersion,
    runtimeCheckpoint: checkpoint.activeRuntimeCheckpoint.runtimeKernelCheckpoint,
    runtimeProfile: checkpoint.activeRuntimeCheckpoint.profile,
    policySchema: policy.schema,
    policySchemaVersion: policy.schemaVersion,
    policyId: policy.id,
    policySource: checkpoint.activeRuntimeCheckpoint.kernel32CompatibilityPolicy.source,
    nodeReportSchema: nodeReport.schema,
    nodeReportSchemaVersion: nodeReport.schemaVersion,
    browserRuntimeProtocol: browserReport.runtimeProtocol,
    executionEngineProfile: checkpoint.browserCheckpoint.executionEngineProfile,
  },
  environment: {
    platform: `${process.platform}-${os.version()}`,
    osRelease: os.release(),
    architecture: process.arch,
    node: process.version,
    rustc: firstLine("rustc", ["--version"]),
    cargo: firstLine("cargo", ["--version"]),
    chrome: browserReport.chromeVersion,
    llvm: {
      clang: toolchainReport.tools.clang,
      wasmClang: toolchainReport.tools.wasmClang,
      lldLink: toolchainReport.tools.lldLink,
      llvmReadobj: toolchainReport.tools.llvmReadobj,
    },
    executionEngine: {
      engine: executionEngine.engine,
      sourceRevision: v86AdapterReport.sourceRevision,
      cpuContextAbi: {
        version: v86AdapterReport.results.cpuState.version,
        bytes: v86AdapterReport.results.cpuState.size,
      },
      adapterArtifact: {
        bytes: artifacts.v86AdapterWasm.bytes,
        sha256: artifacts.v86AdapterWasm.sha256,
      },
    },
  },
  semantic: {
    outcome: "passed",
    node: {
      outcome:
        checkpoint.nextStructuredStop.kind === "capability-checkpoint" ||
        completedCapabilityBeforeStructuredStop
          ? "checkpoint-complete"
          : "structured-stop",
      implementedApiContracts: nodeReport.process.implementedApiContracts,
      supportedCalls: nodeSupportedCalls,
      callSemanticsSha256: nodeCallSemanticsSha256,
      instructions: nodeInstructions,
      finalGeneration: nodeReport.synchronization.finalGeneration,
      memoryContentSha256: nodeMemoryContentSha256,
      ...(checkpoint.nextStructuredStop.kind === "capability-checkpoint"
        ? {
            checkpointCompletion: projectCapabilityCheckpointCompletion(
              nodeCheckpointCompletion,
            ),
          }
        : {}),
      writebackBatches: nodeReport.synchronization.writebackBatches,
      ...(nodeReport.synchronization.dirtyPages === undefined
        ? {}
        : { dirtyPages: nodeReport.synchronization.dirtyPages }),
      ...(nodeReport.synchronization.changedBytes === undefined
        ? {}
        : { changedBytes: nodeReport.synchronization.changedBytes }),
      hostDeltaBatches: nodeReport.synchronization.hostDeltaBatches,
    },
    browser: {
      outcome: browserReport.success.report.outcome,
      supportedCalls: browserReport.success.report.supportedCallCount,
      callSemanticsSha256:
        browserReport.success.report.supportedCallSemanticsSha256,
      instructions: browserReport.success.report.instructions,
      finalGeneration: browserReport.success.report.finalGeneration,
      memoryContentSha256:
        checkpoint.nextStructuredStop.kind === "capability-checkpoint"
          ? browserReport.success.report.checkpointCompletion.memoryContentSha256
          : browserReport.success.report.firstUnsupported.memoryContentSha256,
      ...(checkpoint.nextStructuredStop.kind === "capability-checkpoint"
        ? {
            checkpointCompletion: projectCapabilityCheckpointCompletion(
              browserReport.success.report.checkpointCompletion,
            ),
          }
        : {}),
      wasmBridgeCalls: browserReport.success.report.wasmBridgeCalls,
      executionStateMachine: {
        success: executionStateEvidence(browserReport.success.report),
        cancellation: executionStateEvidence(browserReport.cancellation.report),
        watchdog: executionStateEvidence(browserReport.watchdog.report),
      },
      executionEngine: structuredClone(browserReport.success.report.executionEngine),
      executionEngineLinearMemoryBytes:
        browserReport.success.report.executionEngineLinearMemoryBytes,
    },
    memoryEquivalent: true,
    nextStructuredStop: structuredClone(checkpoint.nextStructuredStop),
    cleanup: {
      pendingHostCalls: browserReport.success.report.pendingHostCalls,
      pendingCallbacks: browserReport.success.report.pendingCallbacks,
      cancelInstructions: browserReport.cancellation.report.instructions,
      cancelPendingHostCalls: browserReport.cancellation.report.pendingHostCalls,
      cancelPendingCallbacks: browserReport.cancellation.report.pendingCallbacks,
      watchdogInstructions: browserReport.watchdog.report.instructions,
      watchdogPendingHostCalls: browserReport.watchdog.report.pendingHostCalls,
      watchdogPendingCallbacks: browserReport.watchdog.report.pendingCallbacks,
    },
  },
  artifacts,
  performanceObservation: {
    classification: "observation-not-semantic-gate",
    status: checkpoint.browserCheckpoint.bgiBenchmark,
    configuration: benchmark.configuration,
    benchmarkClassification: benchmark.performanceClassification,
    decisionStatus: benchmark.decisionStatus,
    decisionReason: benchmark.decisionReason,
    thresholdEvaluationCount: benchmark.thresholdEvaluationCount,
    durationMilliseconds: benchmark.durationMilliseconds,
    completedSessions: benchmark.completedSessions,
    failedSessions: benchmark.failedSessions,
    thresholdFailureIds: benchmark.thresholdFailureIds,
    reportSha256: benchmark.reportSha256,
  },
});
assertValidPrivateTargetEvidence(evidence, checkpoint);

fs.mkdirSync(path.dirname(options.output), { recursive: true });
const temporary = `${options.output}.tmp-${process.pid}`;
fs.writeFileSync(temporary, `${JSON.stringify(evidence, null, 2)}\n`, { flag: "wx" });
fs.renameSync(temporary, options.output);
console.log(
  `private target gate passed: ${evidence.semantic.node.supportedCalls} Node/Chrome calls, ` +
    `${
      evidence.semantic.nextStructuredStop.kind === "capability-checkpoint"
        ? evidence.semantic.nextStructuredStop.completion.capability
        : evidence.semantic.nextStructuredStop.kind === "execution-fault"
        ? evidence.semantic.nextStructuredStop.capabilityCluster
        : evidence.semantic.nextStructuredStop.kind === "com-method"
          ? `${evidence.semantic.nextStructuredStop.interface}::${evidence.semantic.nextStructuredStop.method}`
          : evidence.semantic.nextStructuredStop.api
    } terminal; ` +
    `${options.outputRelative} payload ${evidence.payloadSha256}`,
);
