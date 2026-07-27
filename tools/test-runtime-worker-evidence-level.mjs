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

assert.match(protocol, /protocolVersion = 28 as const/u);
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
assert.match(worker, /shell32-drag-accept-files/u);
assert.match(
  worker,
  /completion\.implementationId === "user32\.set-window-pos\.v5"/u,
);
assert.match(worker, /user32\.show-window\.visible-normal-noop\.v1/u);
assert.match(worker, /user32\.begin-paint\.full-client-logical-dc\.v1/u);
assert.match(worker, /user32\.end-paint\.matching-session\.v1/u);

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
