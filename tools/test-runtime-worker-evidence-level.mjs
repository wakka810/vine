import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");

const protocol = read("browser/runtime-worker/protocol.ts");
const worker = read("browser/runtime-worker/runtime-worker.ts");
const fullProbe = read("browser/runtime-worker/bgi-check.ts");
const benchmark = read("browser/runtime-worker/bgi-benchmark.ts");

assert.match(protocol, /protocolVersion = 7 as const/u);
assert.match(protocol, /evidenceLevel: "full" \| "summary"/u);
assert.match(protocol, /supportedCallCount: number/u);

assert.match(worker, /readonly #semanticCallDigest = new SemanticCallDigest\(\)/u);
assert.doesNotMatch(worker, /completedBgiCallSequences/u);
assert.match(worker, /this\.#semanticCallDigest\.update\(call\)/u);
assert.match(worker, /this\.#report\.supportedCallCount === this\.#semanticCallDigest\.count\(\)/u);
assert.match(worker, /this\.#semanticCallDigest\.digestHex\(\)/u);
assert.doesNotMatch(worker, /supportedCalls\.some\(/u);
assert.equal(worker.match(/supportedCalls\.sort\(/gu)?.length, 1);
assert.match(worker, /summary runtime evidence is only supported for the private BGI workload/u);
assert.match(worker, /summary BGI evidence retained detailed call payloads/u);

assert.match(fullProbe, /searchParams\.get\("evidence"\) === "full" \? "full" : "summary"/u);
assert.match(fullProbe, /instructionBudget: scenario === "watchdog" \? 4 : 16_384/u);
assert.match(benchmark, /evidenceLevel: "summary"/u);
assert.match(benchmark, /instructionBudget: 16_384/u);
assert.match(benchmark, /report\.supportedCallCount === expected\.supportedCalls/u);
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
