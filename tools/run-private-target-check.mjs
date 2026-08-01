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
  return report?.schema === "vine.v86-bgi-scheduled-shared-clock-primary";
}

function projectScheduledSharedClockCheckpointCompletion(
  nodeReport,
  expectedNext,
) {
  const execution = nodeReport.execution;
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

  assert.equal(expectedNext.kind, "capability-checkpoint");
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
  assert.equal(policy.clockScheduleSource, scheduled.scheduleSource);
  assert.equal(
    nodeReport.contract.retiredInstructionsPerSecond,
    scheduled.retiredInstructionsPerSecond,
  );
  assert.equal(
    policy.clockScheduledRetiredInstructionsPerSecond,
    scheduled.retiredInstructionsPerSecond,
  );
  assert.equal(nodeReport.contract.resolution100ns, scheduled.resolution100ns);
  assert.equal(policy.clockResolution100ns, scheduled.resolution100ns);
  assert.equal(nodeReport.contract.apiReadsAdvanceClock, false);
  assert.equal(policy.clockApiReadsAdvance, false);
  assert.equal(nodeReport.contract.waitDeadlinesShareClock, true);
  assert.equal(policy.clockWaitDeadlinesShareAuthority, true);
  assert.equal(nodeReport.contract.hostTimeImported, false);
  assert.equal(policy.clockHostTimeImported, false);
  assert.equal(policy.clockBrowserTimeImported, false);
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
  assert.equal(nodeExecution.presentations.thirdPresentReached, false);
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

  const nodeEndpoint = nodeStop.fault.endpoint.endpoint;
  const expectedEndpoint = expectedNext.nextEndpoint;
  assert.equal(nodeStop.fault.capabilityCluster, expectedNext.capabilityCluster);
  assert.equal(nodeStop.fault.fault.code, "endpoint-contract-missing");
  assert.equal(nodeStop.fault.endpoint.kind, expectedEndpoint.endpointKind);
  assert.equal(nodeEndpoint.id, expectedEndpoint.endpointId);
  assert.equal(hex32(nodeEndpoint.address), expectedEndpoint.endpointAddress);
  assert.equal(optionalHex32(nodeEndpoint.iatAddress), expectedEndpoint.iatAddress);
  assert.equal(
    `${nodeEndpoint.module}!${formatRuntimeEndpointSymbol(nodeEndpoint)}`.toLowerCase(),
    expectedEndpoint.api.toLowerCase(),
  );
  assert.equal(nodeStop.terminal.instructionCount, expectedNext.instructionCount);
  assert.equal(
    hex32(nodeStop.terminal.terminal.returnAddress),
    expectedEndpoint.returnAddress,
  );
  assert.deepEqual(nodeStop.arguments.map(hex32), expectedEndpoint.arguments);
  assert.equal(nodeStop.fault.memoryContentSha256, expected.memoryContentSha256);

  const nodeCheckpoint = projectScheduledSharedClockCheckpointCompletion(
    nodeReport,
    expectedNext,
  );
  assertCapabilityCheckpoint(nodeCheckpoint, expectedNext);
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
  assert.equal(browserUnsupported.capabilityCluster, expectedNext.capabilityCluster);
  assert.equal(browserUnsupported.fault.code, "endpoint-contract-missing");
  assert.equal(browserUnsupported.instructionCount, expectedNext.instructionCount);
  assert.equal(browserUnsupported.memoryContentSha256, expected.memoryContentSha256);
  assert.equal(browserUnsupported.endpoint.id, expectedEndpoint.endpointId);
  assert.equal(hex32(browserUnsupported.endpoint.address), expectedEndpoint.endpointAddress);
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
  assert.equal(nodeReport.synchronization.activeCallbackDepth, 0);

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
  ? nodeReport.stop.fault.memoryContentSha256
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
        checkpoint.nextStructuredStop.kind === "capability-checkpoint"
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
