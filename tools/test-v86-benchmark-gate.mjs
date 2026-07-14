import assert from "node:assert/strict";

import { evaluateV86BenchmarkGate } from "./v86-benchmark-gate.mjs";

assert.deepEqual(evaluateV86BenchmarkGate("go"), {
  mode: "strict-decision",
  acceptedDecisions: ["go"],
  accepted: true,
});
assert.equal(evaluateV86BenchmarkGate("conditional-go").accepted, false);
assert.equal(evaluateV86BenchmarkGate("no-go").accepted, false);

assert.deepEqual(evaluateV86BenchmarkGate("conditional-go", { allowConditionalGo: true }), {
  mode: "ci-observation",
  acceptedDecisions: ["go", "conditional-go"],
  accepted: true,
});
assert.equal(evaluateV86BenchmarkGate("go", { allowConditionalGo: true }).accepted, true);
assert.equal(evaluateV86BenchmarkGate("no-go", { allowConditionalGo: true }).accepted, false);
assert.throws(
  () => evaluateV86BenchmarkGate("unknown"),
  /unsupported v86 benchmark decision/u,
);
assert.throws(
  () => evaluateV86BenchmarkGate("go", { allowConditionalGo: "yes" }),
  /allowConditionalGo must be boolean/u,
);

console.log("v86 benchmark gate policy passed");
