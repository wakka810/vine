import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(root, relativePath), "utf8");

const protocol = read("browser/runtime-worker/protocol.ts");
const worker = read("browser/runtime-worker/runtime-worker.ts");
const bridge = read("browser/runtime-worker/runtime-bridge.ts");
const fullProbe = read("browser/runtime-worker/bgi-check.ts");
const benchmark = read("browser/runtime-worker/bgi-benchmark.ts");

assert.match(protocol, /protocolVersion = 125 as const/u);
assert.match(protocol, /"d3d9-begin-scene-post-partial-dirty-region"/u);
assert.match(protocol, /"d3d9-set-sampler-state-post-begin-scene"/u);
assert.match(
  protocol,
  /"d3d9-set-texture-stage-state-post-sampler-state"/u,
);
assert.match(protocol, /"d3d9-set-texture-post-texture-stage-state"/u);
assert.match(protocol, /"d3d9-set-fvf-post-texture-binding"/u);
assert.match(protocol, /"d3d9-set-stream-source-post-fvf"/u);
assert.match(protocol, /"d3d9-draw-primitive-post-stream-source"/u);
assert.match(protocol, /"d3d9-end-scene-post-draw-primitive"/u);
assert.match(protocol, /"d3d9-get-raster-status-post-end-scene"/u);
assert.match(protocol, /"d3d9-present-post-raster-status"/u);
assert.match(protocol, /"winmm-time-get-time-post-present"/u);
assert.match(
  protocol,
  /capability: "winmm-time-get-time-post-present";[\s\S]*?implementationId: "runtime\.winmm-time-get-time-post-present\.v1";[\s\S]*?semanticImplementationId: "winmm\.time-get-time\.v1";[\s\S]*?nextArguments: number\[\];/u,
);
assert.match(
  protocol,
  /capability: "d3d9-test-cooperative-level-post-time-get-time";[\s\S]*?implementationId:[\s\S]*?"runtime\.d3d9-test-cooperative-level-post-time-get-time\.v1";[\s\S]*?testCooperativeLevelProfileId: "primary-live-operational-device-query-v1";[\s\S]*?nextArguments: number\[\];/u,
);
assert.match(
  protocol,
  /capability: "kernel32-set-waitable-timer-post-test-cooperative-level";[\s\S]*?implementationId: "runtime\.kernel32-set-waitable-timer-post-test-cooperative-level\.v1";[\s\S]*?semanticImplementationId: "kernel32\.set-waitable-timer\.relative-one-shot\.v1";[\s\S]*?browserTimerCreated: false;/u,
);
assert.match(
  protocol,
  /capability: "user32-translate-message-post-test-cooperative-level-timer-peek";[\s\S]*?implementationId: "runtime\.user32-translate-message-post-test-cooperative-level-timer-peek\.v1";[\s\S]*?semanticImplementationId: "user32\.translate-message\.window-timer-pointer-noop\.v2";[\s\S]*?generalKeyboardTranslationImplemented: false;/u,
);
assert.match(protocol, /evidenceLevel: "full" \| "summary"/u);
assert.match(protocol, /capabilityProfile\?: BgiCapabilityProfile/u);
assert.match(protocol, /supportedCallCount: number/u);
assert.match(
  protocol,
  /"primary-main-loop-recurring-timer-multi-cycle-batch"/u,
);
assert.match(protocol, /"primary-main-loop-recurring-timer-window-batch"/u);
assert.match(protocol, /"primary-main-loop-presentation-progress-gateway"/u);
assert.match(protocol, /"primary-main-loop-input-progress-gateway"/u);
assert.match(
  protocol,
  /"primary-main-loop-action-input-progress-gateway"/u,
);
assert.match(
  protocol,
  /"primary-main-loop-window-timer-callback-delivery-gateway"/u,
);
assert.match(
  protocol,
  /"primary-main-loop-window-timer-callback-action-input-progress-gateway"/u,
);
assert.match(
  protocol,
  /"primary-main-loop-pointer-message-action-progress-gateway"/u,
);
assert.match(protocol, /"user32-set-focus-action-continuation"/u);
assert.match(protocol, /"winmm-time-get-time-action-continuation"/u);
assert.match(
  protocol,
  /"user32-def-window-proc-a-action-continuation"/u,
);
assert.match(
  protocol,
  /"user32-peek-message-a-pointer-release-continuation"/u,
);
assert.match(
  protocol,
  /"user32-translate-message-pointer-release-continuation"/u,
);
assert.match(
  protocol,
  /"user32-dispatch-message-a-pointer-release-continuation"/u,
);
assert.match(
  protocol,
  /"user32-def-window-proc-a-pointer-release-continuation"/u,
);
assert.match(
  protocol,
  /"user32-peek-message-a-post-pointer-release-continuation"/u,
);
assert.match(
  protocol,
  /"winmm-time-get-time-post-pointer-release-repeat-batch"/u,
);
assert.match(
  protocol,
  /"user32-get-window-rect-post-pointer-release-continuation"/u,
);
assert.match(
  protocol,
  /"user32-get-cursor-pos-post-pointer-release-continuation"/u,
);
assert.match(
  protocol,
  /"user32-get-system-metrics-post-pointer-release-continuation"/u,
);
assert.match(
  protocol,
  /"user32-get-system-metrics-caption-post-pointer-release-continuation"/u,
);
assert.match(
  protocol,
  /"user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation"/u,
);
assert.match(
  protocol,
  /"user32-is-iconic-post-pointer-release-continuation"/u,
);
assert.match(
  protocol,
  /"winmm-time-get-time-post-is-iconic-continuation"/u,
);
assert.match(
  protocol,
  /"user32-get-async-key-state-post-is-iconic-clock-continuation"/u,
);
assert.match(protocol, /"d3d9-post-input-scan-waitable-timer-cycle"/u);
assert.match(
  protocol,
  /"user32-window-timer-callback-post-waitable-timer-cycle"/u,
);
assert.match(
  protocol,
  /"user32-peek-message-a-post-window-timer-callback-continuation"/u,
);
assert.match(
  protocol,
  /capability: "winmm-time-get-time-action-continuation";[\s\S]*?setFocusActiveCallbackDepthAtStop: 1;[\s\S]*?activeCallbackDepthAtStop: 1;[\s\S]*?setFocus: RuntimeThreadCallSnapshot;[\s\S]*?lastCompletionSequence: number;[\s\S]*?lastCompletionInstructionCount: number;[\s\S]*?timeGetTime: RuntimeThreadCallSnapshot;[\s\S]*?nextArguments: \[number, number, number, number\];/u,
);
assert.match(
  protocol,
  /capability: "user32-def-window-proc-a-action-continuation";[\s\S]*?timeGetTimeActiveCallbackDepthAtCompletion: 1;[\s\S]*?defWindowProcAActiveCallbackDepthAtCompletion: 1;[\s\S]*?activeCallbackDepthAtStop: 0;[\s\S]*?timeGetTime: RuntimeThreadCallSnapshot;[\s\S]*?lastCompletionSequence: number;[\s\S]*?lastCompletionInstructionCount: number;[\s\S]*?defWindowProcA: RuntimeThreadCallSnapshot;[\s\S]*?nextArguments: \[number, number, number, number, number\];/u,
);
assert.match(
  protocol,
  /capability: "user32-peek-message-a-pointer-release-continuation";[\s\S]*?releaseMessageQueued: false;[\s\S]*?peekMessageReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?activeCallbackDepthAtStop: 0;[\s\S]*?releaseMessage: RuntimeThreadMessageSnapshot;[\s\S]*?queueDepthBefore: 1;[\s\S]*?queueDepthAfter: 0;[\s\S]*?lastCompletionSequence: number;[\s\S]*?lastCompletionInstructionCount: number;[\s\S]*?peekMessageRelease: RuntimeThreadCallSnapshot;[\s\S]*?nextArguments: \[number\];/u,
);
assert.match(
  protocol,
  /capability: "user32-translate-message-pointer-release-continuation";[\s\S]*?releaseMessageQueued: false;[\s\S]*?peekMessageReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?translateMessageReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?queueDepthAtTranslateCompletion: 0;[\s\S]*?translateMessageReleaseBytesPreserved: true;[\s\S]*?activeCallbackDepthAtStop: 0;[\s\S]*?releaseMessage: RuntimeThreadMessageSnapshot;[\s\S]*?queueDepthBefore: 1;[\s\S]*?queueDepthAfter: 0;[\s\S]*?lastCompletionSequence: number;[\s\S]*?lastCompletionInstructionCount: number;[\s\S]*?peekMessageRelease: RuntimeThreadCallSnapshot;[\s\S]*?translateMessageRelease: RuntimeThreadCallSnapshot;[\s\S]*?nextArguments: \[number\];/u,
);
assert.match(
  protocol,
  /capability: "user32-dispatch-message-a-pointer-release-continuation";[\s\S]*?releaseMessageQueued: false;[\s\S]*?translateMessageReleaseBytesPreserved: true;[\s\S]*?requiredEntries: 1;[\s\S]*?entryCount: number;[\s\S]*?lastEntrySequence: number;[\s\S]*?lastEntryInstructionCount: number;[\s\S]*?dispatchMessageReleaseActiveCallbackDepthAtEntry: 1;[\s\S]*?queueDepthAtDispatchEntry: 0;[\s\S]*?dispatchMessageReleaseBytesPreserved: true;[\s\S]*?dispatchMessageRelease: \{[\s\S]*?runtimeThreadId: number;[\s\S]*?symbol: "DispatchMessageA";[\s\S]*?pendingCallback: true;[\s\S]*?releaseCallbackEntry: \{[\s\S]*?runtimeThreadId: number;[\s\S]*?depth: 1;[\s\S]*?allocationStart: number;[\s\S]*?scratchAddress: number;[\s\S]*?scratchBytes: number;[\s\S]*?frameBytes: 20;[\s\S]*?returnSentinel: number;[\s\S]*?activeCallbackDepthAtStop: 1;[\s\S]*?nextArguments: \[number, number, number, number\];/u,
);
assert.match(
  protocol,
  /capability: "user32-def-window-proc-a-pointer-release-continuation";[\s\S]*?requiredCompletions: 1;[\s\S]*?requiredEntries: 1;[\s\S]*?defWindowProcAReleaseActiveCallbackDepthAtCompletion: 1;[\s\S]*?defWindowProcARelease: RuntimeThreadCallSnapshot;[\s\S]*?releaseCallbackReturn: \{[\s\S]*?action: "return";[\s\S]*?dispatchMessageReleaseResult: number;[\s\S]*?activeCallbackDepthAtStop: 0;[\s\S]*?nextArguments: \[number, number, number, number, number\];/u,
);
assert.match(
  protocol,
  /requiredEntries: 1;[\s\S]*?defWindowProcAReleaseActiveCallbackDepthAtCompletion: 1;[\s\S]*?peekMessagePostReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?postReleaseQueueDepthBefore: 0;[\s\S]*?postReleaseQueueDepthAfter: 0;[\s\S]*?postReleaseMessageBytesPreserved: true;[\s\S]*?peekMessagePostRelease: RuntimeThreadCallSnapshot;[\s\S]*?activeCallbackDepthAtStop: 0;[\s\S]*?nextArguments:[\s\S]*?\[\][\s\S]*?\[number\][\s\S]*?\[number, number\][\s\S]*?\[number, number, number, number, number\][\s\S]*?\[number, number, number, number, number, number\];[\s\S]*?capability:[\s\S]*?"user32-peek-message-a-post-pointer-release-continuation";[\s\S]*?requiredCompletions: 1;/u,
);
assert.match(
  protocol,
  /capability:[\s\S]*?"winmm-time-get-time-post-pointer-release-continuation";[\s\S]*?implementationId:[\s\S]*?"runtime\.winmm-time-get-time-post-pointer-release-continuation\.v1";[\s\S]*?timeGetTimePostReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?timeGetTimePostRelease: RuntimeThreadCallSnapshot;/u,
);
assert.match(
  protocol,
  /capability:[\s\S]*?"winmm-time-get-time-post-pointer-release-repeat-batch";[\s\S]*?implementationId:[\s\S]*?"runtime\.winmm-time-get-time-post-pointer-release-repeat-batch\.v1";[\s\S]*?requiredCompletions: 4;[\s\S]*?completionCount: 4;[\s\S]*?maximumRepeatCompletions: 16;[\s\S]*?timeGetTimePostReleaseCompletions: RuntimeThreadCallSnapshot\[\];/u,
);
assert.match(
  protocol,
  /capability:[\s\S]*?"user32-get-window-rect-post-pointer-release-continuation";[\s\S]*?implementationId:[\s\S]*?"runtime\.user32-get-window-rect-post-pointer-release-continuation\.v1";[\s\S]*?requiredCompletions: 1;[\s\S]*?completionCount: 1;[\s\S]*?maximumRepeatCompletions: 16;[\s\S]*?timeGetTimePostReleaseCompletions: RuntimeThreadCallSnapshot\[\];[\s\S]*?getWindowRectPostReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?getWindowRectPostRelease: RuntimeThreadCallSnapshot;/u,
);
assert.match(
  protocol,
  /capability:[\s\S]*?"user32-get-cursor-pos-post-pointer-release-continuation";[\s\S]*?implementationId:[\s\S]*?"runtime\.user32-get-cursor-pos-post-pointer-release-continuation\.v1";[\s\S]*?requiredCompletions: 1;[\s\S]*?completionCount: 1;[\s\S]*?maximumRepeatCompletions: 16;[\s\S]*?timeGetTimePostReleaseCompletions: RuntimeThreadCallSnapshot\[\];[\s\S]*?getWindowRectPostReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?getWindowRectPostRelease: RuntimeThreadCallSnapshot;[\s\S]*?getCursorPosPostReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?getCursorPosPostRelease: RuntimeThreadCallSnapshot;/u,
);
assert.match(
  protocol,
  /capability:[\s\S]*?"user32-get-system-metrics-post-pointer-release-continuation";[\s\S]*?implementationId:[\s\S]*?"runtime\.user32-get-system-metrics-post-pointer-release-continuation\.v1";[\s\S]*?requiredCompletions: 1;[\s\S]*?completionCount: 1;[\s\S]*?maximumRepeatCompletions: 16;[\s\S]*?timeGetTimePostReleaseCompletions: RuntimeThreadCallSnapshot\[\];[\s\S]*?getWindowRectPostRelease: RuntimeThreadCallSnapshot;[\s\S]*?getCursorPosPostRelease: RuntimeThreadCallSnapshot;[\s\S]*?getSystemMetricsPostReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?getSystemMetricsPostRelease: RuntimeThreadCallSnapshot;/u,
);
assert.match(
  protocol,
  /capability:[\s\S]*?"user32-get-system-metrics-caption-post-pointer-release-continuation";[\s\S]*?implementationId:[\s\S]*?"runtime\.user32-get-system-metrics-caption-post-pointer-release-continuation\.v1";[\s\S]*?requiredCompletions: 1;[\s\S]*?completionCount: 1;[\s\S]*?getSystemMetricsPostRelease: RuntimeThreadCallSnapshot;[\s\S]*?getSystemMetricsCaptionPostReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?getSystemMetricsCaptionPostRelease: RuntimeThreadCallSnapshot;/u,
);
assert.match(
  protocol,
  /capability:[\s\S]*?"user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation";[\s\S]*?implementationId:[\s\S]*?"runtime\.user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation\.v1";[\s\S]*?requiredCompletions: 1;[\s\S]*?completionCount: 1;[\s\S]*?getSystemMetricsCaptionPostRelease: RuntimeThreadCallSnapshot;[\s\S]*?getSystemMetricsFixedFrameHeightPostReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?getSystemMetricsFixedFrameHeightPostRelease: RuntimeThreadCallSnapshot;/u,
);
assert.match(
  protocol,
  /capability: "user32-is-iconic-post-pointer-release-continuation";[\s\S]*?implementationId: "runtime\.user32-is-iconic-post-pointer-release-continuation\.v1";[\s\S]*?requiredCompletions: 1;[\s\S]*?completionCount: 1;[\s\S]*?getSystemMetricsFixedFrameHeightPostRelease: RuntimeThreadCallSnapshot;[\s\S]*?isIconicPostReleaseActiveCallbackDepthAtCompletion: 0;[\s\S]*?isIconicPostRelease: RuntimeThreadCallSnapshot;/u,
);
assert.match(
  protocol,
  /capability: "winmm-time-get-time-post-is-iconic-continuation";[\s\S]*?implementationId: "runtime\.winmm-time-get-time-post-is-iconic-continuation\.v1";[\s\S]*?requiredCompletions: 1;[\s\S]*?completionCount: 1;[\s\S]*?isIconicPostRelease: RuntimeThreadCallSnapshot;[\s\S]*?timeGetTimePostIsIconicActiveCallbackDepthAtCompletion: 0;[\s\S]*?timeGetTimePostIsIconic: RuntimeThreadCallSnapshot;/u,
);
assert.match(
  protocol,
  /capability:[\s\S]*?"user32-get-async-key-state-post-is-iconic-clock-continuation";[\s\S]*?implementationId:[\s\S]*?"runtime\.user32-get-async-key-state-post-is-iconic-clock-continuation\.v1";[\s\S]*?requiredCompletions: 38;[\s\S]*?completionCount: 38;[\s\S]*?firstCompletionSequence: number;[\s\S]*?scanVirtualKeys: number\[\];[\s\S]*?getAsyncKeyStatePostIsIconicClockActiveCallbackDepthAtCompletion: 0;[\s\S]*?getAsyncKeyStatePostIsIconicClockFirst: RuntimeThreadCallSnapshot;[\s\S]*?getAsyncKeyStatePostIsIconicClockLast: RuntimeThreadCallSnapshot;/u,
);
assert.match(
  protocol,
  /capability: "d3d9-post-input-scan-waitable-timer-cycle";[\s\S]*?implementationId:[\s\S]*?"kernel32\.wait-for-single-object-ex\.waitable-timer\.v1";[\s\S]*?prerequisiteTestCooperativeLevelSequence: number;[\s\S]*?timerArmActiveCallbackDepthAtCompletion: 0;[\s\S]*?timerWaitActiveCallbackDepthAtCompletion: 0;[\s\S]*?timerArm: RuntimeThreadCallSnapshot;[\s\S]*?timerWait: RuntimeThreadCallSnapshot;[\s\S]*?cumulativeTimerArmCompletions: number;[\s\S]*?cumulativeTimerWaitCompletions: number;/u,
);
assert.match(
  protocol,
  /capability:[\s\S]*?"user32-window-timer-callback-post-waitable-timer-cycle";[\s\S]*?implementationId:[\s\S]*?"runtime\.user32-window-timer-callback-post-waitable-timer-cycle\.v1";[\s\S]*?requiredCompletions: 3;[\s\S]*?requiredDeliveries: 1;[\s\S]*?deliveryCount: 3;[\s\S]*?windowTimerPeek: RuntimeThreadCallSnapshot;[\s\S]*?windowTimerTranslate: RuntimeThreadCallSnapshot;[\s\S]*?windowTimerDispatch: RuntimeThreadCallSnapshot[\s\S]*?callbackArguments: \[number, number, number, number\];[\s\S]*?windowTimerCallbackNestedCalls: \[RuntimeThreadCallSnapshot\];[\s\S]*?windowTimerCallbackNestedCompletionCount: 1;[\s\S]*?windowTimerCallbackActiveDepthAtCompletion: 1;/u,
);
assert.match(
  protocol,
  /capability:[\s\S]*?"user32-peek-message-a-post-window-timer-callback-continuation";[\s\S]*?implementationId:[\s\S]*?"runtime\.user32-peek-message-a-post-window-timer-callback-continuation\.v1";[\s\S]*?prerequisiteWindowTimerDispatchSequence: number;[\s\S]*?postWindowTimerCallbackQueueDepthBefore: 0;[\s\S]*?postWindowTimerCallbackQueueDepthAfter: 0;[\s\S]*?postWindowTimerCallbackMessageBytesPreserved: true;[\s\S]*?peekMessagePostWindowTimerCallback: RuntimeThreadCallSnapshot;/u,
);
assert.match(protocol, /windowTimerDelivery\?: \{/u);
assert.match(
  protocol,
  /"input-consumed-with-post-input-timer-no-presentation-progress"/u,
);
assert.match(protocol, /RuntimeWindowTimerSnapshot/u);
assert.match(protocol, /RuntimeRecurringTimerCycleSummary/u);
assert.match(protocol, /RuntimeRecurringTimerWindowCycleSummary/u);
assert.match(protocol, /RuntimeRecurringTimerInterCycleSummary/u);
assert.match(protocol, /runtimeThreadId: number/u);
assert.match(worker, /recurringTimerMultiCycleExpectation/u);
assert.match(worker, /recurringTimerWindowExpectation/u);
assert.match(worker, /#recurringTimerMultiCycleSummaries/u);
assert.match(worker, /completedCycleCount >= configuration\.requiredCycles/u);
assert.match(fullProbe, /primary-main-loop-recurring-timer-multi-cycle-batch/u);
assert.match(fullProbe, /primary-main-loop-recurring-timer-window-batch/u);
assert.match(fullProbe, /primary-main-loop-presentation-progress-gateway/u);
assert.match(fullProbe, /primary-main-loop-input-progress-gateway/u);
assert.match(
  fullProbe,
  /primary-main-loop-window-timer-callback-delivery-gateway/u,
);
assert.match(
  fullProbe,
  /primary-main-loop-window-timer-callback-action-input-progress-gateway/u,
);
assert.match(
  fullProbe,
  /primary-main-loop-pointer-message-action-progress-gateway/u,
);
assert.match(fullProbe, /winmm-time-get-time-action-continuation/u);
assert.match(
  fullProbe,
  /user32-window-timer-callback-post-waitable-timer-cycle/u,
);
assert.match(
  fullProbe,
  /user32-peek-message-a-post-window-timer-callback-continuation/u,
);
assert.match(
  fullProbe,
  /user32-def-window-proc-a-action-continuation/u,
);
assert.match(
  fullProbe,
  /user32-peek-message-a-pointer-release-continuation/u,
);
assert.match(
  fullProbe,
  /user32-translate-message-pointer-release-continuation/u,
);
assert.match(
  fullProbe,
  /user32-dispatch-message-a-pointer-release-continuation/u,
);
assert.match(
  fullProbe,
  /user32-def-window-proc-a-pointer-release-continuation/u,
);
assert.match(
  fullProbe,
  /user32-peek-message-a-post-pointer-release-continuation/u,
);
assert.match(
  fullProbe,
  /winmm-time-get-time-post-pointer-release-repeat-batch/u,
);
assert.match(
  fullProbe,
  /user32-get-window-rect-post-pointer-release-continuation/u,
);
assert.match(
  fullProbe,
  /user32-get-cursor-pos-post-pointer-release-continuation/u,
);
assert.match(
  fullProbe,
  /user32-get-system-metrics-post-pointer-release-continuation/u,
);
assert.match(
  worker,
  /hasTimeGetTimeActionContinuation\([\s\S]{0,200}#timeGetTimeActionContinuationDispatching &&\s*completion\.implementationId === "winmm\.time-get-time\.v1"/u,
);
assert.match(
  worker,
  /#defWindowProcAActionContinuationAwaitingDispatch[\s\S]*?ownedEndpoint\.symbol\.name === "DefWindowProcA"[\s\S]*?#defWindowProcAActionContinuationDispatching = true/u,
);
assert.match(
  worker,
  /#defWindowProcAActionContinuationDispatching &&\s*completion\.implementationId === "user32\.def-window-proc-a\.v1"/u,
);
assert.match(
  worker,
  /#peekMessageAPointerReleaseContinuationAwaitingDispatch[\s\S]*?ownedEndpoint\.symbol\.name === "PeekMessageA"[\s\S]*?#peekMessageAPointerReleaseContinuationDispatching = true/u,
);
assert.match(
  worker,
  /#peekMessageAPointerReleaseContinuationDispatching &&\s*completion\.implementationId ===\s*"user32\.peek-message-a\.current-thread\.v1"/u,
);
assert.match(
  worker,
  /#translateMessagePointerReleaseContinuationAwaitingDispatch[\s\S]*?ownedEndpoint\.symbol\.name === "TranslateMessage"[\s\S]*?#translateMessagePointerReleaseContinuationDispatching = true/u,
);
assert.match(
  worker,
  /#translateMessagePointerReleaseContinuationDispatching &&\s*completion\.implementationId ===\s*"user32\.translate-message\.window-timer-pointer-noop\.v2"/u,
);
assert.match(
  worker,
  /#dispatchMessageAPointerReleaseContinuationAwaitingDispatch[\s\S]*?ownedEndpoint\.symbol\.name === "DispatchMessageA"[\s\S]*?#dispatchMessageAPointerReleaseContinuationDispatching = true/u,
);
assert.match(
  worker,
  /#dispatchMessageAPointerReleaseContinuationDispatching[\s\S]*?metadata\.depth === 1[\s\S]*?metadata\.queuedThreadMessages\.length === 0[\s\S]*?releaseBytesAfter\.every\([\s\S]*?dispatchMessageReleaseActiveCallbackDepthAtEntry: 1[\s\S]*?dispatchMessageReleaseBytesPreserved: true[\s\S]*?releaseCallbackEntry: \{[\s\S]*?activeCallbackDepthAtStop: 1[\s\S]*?#bgiCheckpointPending = true/u,
);
assert.match(
  worker,
  /#defWindowProcAPointerReleaseContinuationEntry !== null[\s\S]*?completion\.implementationId === "user32\.def-window-proc-a\.v1"[\s\S]*?defWindowProcAReleaseActiveCallbackDepthAtCompletion: 1[\s\S]*?defWindowProcARelease: \{[\s\S]*?#defWindowProcAPointerReleaseContinuationCompletion !== null[\s\S]*?releaseCallbackReturn: \{[\s\S]*?dispatchMessageReleaseResult: result,[\s\S]*?activeCallbackDepthAtStop: 0[\s\S]*?#bgiCheckpointPending = true/u,
);
assert.match(
  worker,
  /#peekMessageAPostPointerReleaseContinuationEntry !== null[\s\S]*?ownedEndpoint\?\.id === 164[\s\S]*?#peekMessageAPostPointerReleaseContinuationDispatching = true/u,
);
assert.match(
  worker,
  /#peekMessageAPostPointerReleaseContinuationDispatching &&[\s\S]*?completion\.implementationId ===[\s\S]*?"user32\.peek-message-a\.current-thread\.v1"[\s\S]*?queuedThreadMessages\.length === 0[\s\S]*?postReleaseQueueDepthBefore: 0 as const[\s\S]*?postReleaseQueueDepthAfter: 0 as const[\s\S]*?postReleaseMessageBytesPreserved: true as const[\s\S]*?peekMessagePostRelease: \{[\s\S]*?#bgiCheckpointPending = true/u,
);
assert.match(
  worker,
  /#timeGetTimePostPointerReleaseContinuationDispatching &&[\s\S]*?completion\.implementationId === "winmm\.time-get-time\.v1"[\s\S]*?timeGetTimePostPointerReleaseRepeatCompletions\.push\(completedCall\)[\s\S]*?maximumRepeatCompletions: 16[\s\S]*?timeGetTimePostReleaseCompletions: completions[\s\S]*?#bgiCheckpointPending = true/u,
);
assert.match(
  bridge,
  /user32DefWindowProcAPointerDownEnabled: boolean[\s\S]*?user32DefWindowProcAPointerReleaseEnabled: boolean[\s\S]*?user32DefWindowProcAPointerReleaseEnabled[\s\S]*?\? 2[\s\S]*?: Number\(user32DefWindowProcAPointerDownEnabled\)/u,
);
assert.match(protocol, /kernel32-get-file-attributes-a/u);
assert.match(protocol, /RuntimeGetFileAttributesACallSnapshot/u);
assert.match(protocol, /getFileAttributesA:/u);
assert.match(protocol, /kernel32-create-directory-a/u);
assert.match(protocol, /kernel32-find-first-file-a/u);
assert.match(protocol, /kernel32-private-heap-ordinary-segment-growth/u);
assert.match(protocol, /RuntimeCreateDirectoryACallSnapshot/u);
assert.match(protocol, /createDirectoryA:/u);
assert.match(
  worker,
  /profile === "kernel32-find-first-file-a"/u,
);
assert.match(
  worker,
  /profile === "kernel32-private-heap-ordinary-segment-growth"/u,
);
assert.match(protocol, /user32-set-window-pos-repeat-activation/u);
assert.match(protocol, /RuntimeSetWindowPosRepeatActivationCallSnapshot/u);
assert.match(protocol, /setWindowPosRepeatActivation:/u);
assert.match(protocol, /d3d9-recreated-textures/u);
assert.match(protocol, /d3d9RecreatedTextures:/u);
assert.match(protocol, /user32-set-window-pos-frame-change-move/u);
assert.match(protocol, /RuntimeSetWindowPosFrameChangeMoveCallSnapshot/u);
assert.match(protocol, /setWindowPosFrameChangeMove:/u);
assert.match(protocol, /user32-show-window-normal/u);
assert.match(protocol, /RuntimeShowWindowNormalCallSnapshot/u);
assert.match(protocol, /showWindowNormal:/u);
assert.match(protocol, /user32-update-window/u);
assert.match(protocol, /RuntimeBeginPaintCallSnapshot/u);
assert.match(protocol, /RuntimeEndPaintCallSnapshot/u);
assert.match(protocol, /updateWindow:/u);
assert.match(protocol, /d3d9-clear/u);
assert.match(protocol, /RuntimeGraphicsDeviceSnapshot/u);
assert.match(protocol, /d3d9Clear:/u);
assert.match(protocol, /user32-set-rect/u);
assert.match(protocol, /setRect:/u);
assert.match(protocol, /d3d9-add-dirty-rect/u);
assert.match(protocol, /d3d9-update-texture/u);
assert.match(protocol, /RuntimeGraphicsTextureSnapshot/u);
assert.match(protocol, /d3d9AddDirtyRect:/u);
assert.match(protocol, /d3d9-draw-primitive/u);
assert.match(protocol, /d3d9DrawPrimitive:/u);
assert.match(protocol, /d3d9-end-scene/u);
assert.match(protocol, /d3d9EndScene:/u);
assert.match(protocol, /d3d9-post-end-scene-raster-status/u);
assert.match(protocol, /d3d9PostEndSceneRasterStatus:/u);
assert.match(protocol, /d3d9-post-raster-status-waitable-timer/u);
assert.match(protocol, /d3d9-post-scene-present-gateway/u);
assert.match(protocol, /d3d9-post-present-continuation/u);
assert.match(protocol, /postPresentContinuation:/u);
assert.match(protocol, /user32-async-key-state-input/u);
assert.match(protocol, /user32-async-key-state-initial-scan/u);
assert.match(protocol, /user32-async-key-state-initial-scan-continuation/u);
assert.match(protocol, /user32-peek-message-a/u);
assert.match(protocol, /RuntimeKeyboardInputEvent/u);
assert.match(protocol, /RuntimeCursorPositionSnapshot/u);
assert.match(protocol, /keyboardInput:/u);
assert.match(protocol, /getWindowRect:/u);
assert.match(protocol, /getCursorPos:/u);
assert.match(protocol, /getAsyncKeyState:/u);
assert.match(protocol, /getAsyncKeyStateInitialScan:/u);
assert.match(protocol, /initialScanContinuation:/u);
assert.match(protocol, /peekMessageA:/u);
assert.match(protocol, /isIconic:/u);
assert.match(protocol, /runtime-trapless-execution-budget/u);
assert.match(protocol, /kernel32-heap-free-large-buffer/u);
assert.match(protocol, /heapFree\?: \{/u);
assert.match(protocol, /allocationRetired: boolean/u);
assert.match(protocol, /capability: "kernel32-heap-free-large-buffer"/u);
assert.match(protocol, /freeDelta: \{/u);
assert.match(protocol, /kernel32-heap-alloc-24-byte-buffer/u);
assert.match(protocol, /heapAllocation\?: \{/u);
assert.match(protocol, /capability: "kernel32-heap-alloc-24-byte-buffer"/u);
assert.match(protocol, /allocationDelta: \{/u);
assert.match(protocol, /winmm-time-get-time-post-heap-reuse/u);
assert.match(protocol, /kernel32-enter-critical-section-post-clock/u);
assert.match(protocol, /kernel32-enter-critical-section-post-wait/u);
assert.match(
  protocol,
  /kernel32-enter-critical-section-post-timer-wait-batch/u,
);
assert.match(
  protocol,
  /kernel32-enter-critical-section-post-timer-rearm-wait-batch/u,
);
assert.match(
  protocol,
  /kernel32-set-waitable-timer-post-critical-section-batch/u,
);
assert.match(protocol, /kernel32-set-waitable-timer-post-timer-wait-batch/u);
assert.match(
  protocol,
  /kernel32-set-waitable-timer-post-timer-rearm-wait-batch/u,
);
assert.match(protocol, /primary-main-loop-recurring-timer-cycle-batch/u);
assert.match(
  protocol,
  /kernel32-wait-for-single-object-ex-post-timer-arm-batch/u,
);
assert.match(
  protocol,
  /kernel32-wait-for-single-object-ex-post-timer-rearm-batch/u,
);
assert.match(
  protocol,
  /kernel32-wait-for-single-object-ex-post-critical-section/u,
);
assert.match(protocol, /capability: "winmm-time-get-time-post-heap-reuse"/u);
assert.match(
  protocol,
  /capability: "kernel32-enter-critical-section-post-clock"/u,
);
assert.match(
  protocol,
  /capability: "kernel32-enter-critical-section-post-wait"/u,
);
assert.match(
  protocol,
  /"kernel32-enter-critical-section-post-timer-wait-batch"/u,
);
assert.match(
  protocol,
  /"kernel32-enter-critical-section-post-timer-rearm-wait-batch"/u,
);
assert.match(
  protocol,
  /"kernel32-set-waitable-timer-post-critical-section-batch"/u,
);
assert.match(protocol, /"kernel32-set-waitable-timer-post-timer-wait-batch"/u);
assert.match(
  protocol,
  /"kernel32-set-waitable-timer-post-timer-rearm-wait-batch"/u,
);
assert.match(
  protocol,
  /capability: "primary-main-loop-recurring-timer-cycle-batch"/u,
);
assert.match(
  protocol,
  /"kernel32-wait-for-single-object-ex-post-timer-arm-batch"/u,
);
assert.match(
  protocol,
  /"kernel32-wait-for-single-object-ex-post-timer-rearm-batch"/u,
);
assert.match(
  protocol,
  /capability: "kernel32-wait-for-single-object-ex-post-critical-section"/u,
);
assert.match(
  protocol,
  /memoryAfter: \{ generation: number; acknowledgedGeneration: number \}/u,
);
assert.match(protocol, /d3d9TestCooperativeLevel:/u);

assert.match(
  worker,
  /readonly #semanticCallDigest = new SemanticCallDigest\(\)/u,
);
assert.doesNotMatch(worker, /completedBgiCallSequences/u);
assert.match(worker, /this\.#semanticCallDigest\.update\(call\)/u);
assert.match(
  worker,
  /this\.#report\.supportedCallCount === this\.#semanticCallDigest\.count\(\)/u,
);
assert.match(worker, /this\.#semanticCallDigest\.digestHex\(\)/u);
assert.doesNotMatch(worker, /supportedCalls\.some\(/u);
assert.equal(worker.match(/supportedCalls\.sort\(/gu)?.length, 1);
assert.match(
  worker,
  /summary runtime evidence is only supported for the private BGI workload/u,
);
assert.match(worker, /summary BGI evidence retained detailed call payloads/u);
assert.match(worker, /user32\.get-async-key-state\.v1/u);
assert.match(worker, /user32\.get-window-rect\.v1/u);
assert.match(worker, /user32\.get-cursor-pos\.v1/u);
assert.match(worker, /user32\.peek-message-a\.current-thread\.v1/u);
assert.match(
  worker,
  /#recordUser32TranslateMessagePostTestCooperativeLevelTimerPeekCall/u,
);
assert.match(
  worker,
  /user32\.translate-message\.window-timer-pointer-noop\.v2/u,
);
assert.match(worker, /ingestKeyboardInput/u);
assert.match(worker, /#pendingInputEvents/u);
assert.match(fullProbe, /type: "keyboard-input"/u);
assert.match(fullProbe, /pointerSourceEvent\(\s*1,\s*"button-down"/u);
assert.match(fullProbe, /pointerSourceEvent\(\s*2,\s*"button-up"/u);
assert.match(fullProbe, /type: "pointer-input"/u);
assert.match(fullProbe, /primary-main-loop-action-input-progress-gateway/u);
assert.match(
  fullProbe,
  /user32-translate-message-post-test-cooperative-level-timer-peek/u,
);
assert.match(worker, /terminal\.outcome === "structured-stop"/u);
assert.match(
  worker,
  /this\.#configuration\.capabilityProfile ===\s*"kernel32-wait-for-single-object-ex"/u,
);
assert.match(
  worker,
  /this\.#configuration\.capabilityProfile ===\s*"kernel32-enter-critical-section"/u,
);
assert.match(worker, /kernel32-enter-critical-section-continuation/u);
assert.match(worker, /kernel32-leave-critical-section-continuation/u);
assert.match(worker, /isPostTimerArmWaitCheckpoint/u);
assert.match(worker, /isPostTimerWaitCriticalSectionCheckpoint/u);
assert.match(worker, /shell32-drag-accept-files/u);
assert.match(
  worker,
  /completion\.implementationId === "user32\.set-window-pos\.v5"/u,
);
assert.match(worker, /user32\.show-window\.visible-normal-noop\.v1/u);
assert.match(worker, /user32\.begin-paint\.full-client-logical-dc\.v1/u);
assert.match(worker, /user32\.end-paint\.matching-session\.v1/u);
assert.match(worker, /user32\.set-rect\.v1/u);
assert.match(worker, /IDirect3DTexture9\.AddDirtyRect/u);
assert.match(worker, /IDirect3DDevice9\.DrawPrimitive/u);
assert.match(worker, /IDirect3DDevice9\.EndScene/u);
assert.match(worker, /IDirect3DDevice9\.TestCooperativeLevel/u);

assert.match(
  fullProbe,
  /searchParams\.get\("evidence"\) === "full" \? "full" : "summary"/u,
);
assert.match(
  fullProbe,
  /parseCapabilityProfile\(searchParams\.get\("profile"\)\)/u,
);
assert.match(
  fullProbe,
  /instructionBudget: scenario === "watchdog" \? 4 : 16_384/u,
);
assert.match(fullProbe, /maximumRetainedStatuses = 4_096/u);
assert.match(fullProbe, /statusMessagesObserved \+= 1/u);
assert.match(fullProbe, /statusMessagesRetained: statuses\.length/u);
assert.match(benchmark, /evidenceLevel: "summary"/u);
assert.match(benchmark, /instructionBudget: 16_384/u);
assert.match(
  benchmark,
  /report\.supportedCallCount === expected\.supportedCalls/u,
);
assert.match(benchmark, /report\.supportedCalls\.length === 0/u);
assert.match(
  benchmark,
  /report\.supportedCallSemanticsAlgorithm === expected\.callSemanticsAlgorithm/u,
);
assert.match(
  benchmark,
  /report\.supportedCallSemanticsSha256 === expected\.callSemanticsSha256/u,
);

console.log("Runtime Worker evidence-level contract passed");
