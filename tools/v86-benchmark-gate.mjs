const DECISIONS = new Set(["go", "conditional-go", "no-go"]);

export function evaluateV86BenchmarkGate(decision, { allowConditionalGo = false } = {}) {
  if (!DECISIONS.has(decision)) {
    throw new Error(`unsupported v86 benchmark decision: ${decision}`);
  }
  if (typeof allowConditionalGo !== "boolean") {
    throw new TypeError("allowConditionalGo must be boolean");
  }
  const acceptedDecisions = allowConditionalGo ? ["go", "conditional-go"] : ["go"];
  return {
    mode: allowConditionalGo ? "ci-observation" : "strict-decision",
    acceptedDecisions,
    accepted: acceptedDecisions.includes(decision),
  };
}
