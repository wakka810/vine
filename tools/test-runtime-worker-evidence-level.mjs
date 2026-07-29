import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(root, relativePath), "utf8");

const protocol = read("browser/runtime-worker/protocol.ts");
const worker = read("browser/runtime-worker/runtime-worker.ts");
const fullProbe = read("browser/runtime-worker/bgi-check.ts");
const benchmark = read("browser/runtime-worker/bgi-benchmark.ts");

assert.match(protocol, /protocolVersion = 63 as const/u);
assert.match(protocol, /evidenceLevel: "full" \| "summary"/u);
assert.match(protocol, /capabilityProfile\?: BgiCapabilityProfile/u);
assert.match(protocol, /supportedCallCount: number/u);
assert.match(protocol, /kernel32-get-file-attributes-a/u);
assert.match(protocol, /RuntimeGetFileAttributesACallSnapshot/u);
assert.match(protocol, /getFileAttributesA:/u);
assert.match(protocol, /kernel32-create-directory-a/u);
assert.match(protocol, /RuntimeCreateDirectoryACallSnapshot/u);
assert.match(protocol, /createDirectoryA:/u);
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
  /kernel32-set-waitable-timer-post-critical-section-batch/u,
);
assert.match(
  protocol,
  /kernel32-wait-for-single-object-ex-post-timer-arm-batch/u,
);
assert.match(
  protocol,
  /kernel32-wait-for-single-object-ex-post-critical-section/u,
);
assert.match(
  protocol,
  /capability: "winmm-time-get-time-post-heap-reuse"/u,
);
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
  /capability: "kernel32-set-waitable-timer-post-critical-section-batch"/u,
);
assert.match(
  protocol,
  /capability: "kernel32-wait-for-single-object-ex-post-timer-arm-batch"/u,
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
assert.match(worker, /ingestKeyboardInput/u);
assert.match(worker, /#pendingKeyboardEvents/u);
assert.match(fullProbe, /type: "keyboard-input"/u);
assert.match(fullProbe, /virtualKey: 0xa2/u);
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
