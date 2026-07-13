import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { loadCurrentCheckpoint } from "./current-checkpoint.mjs";
import { assertValidPrivateTargetEvidence } from "./private-target-evidence.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const evidencePath = path.join(root, "docs", "private-target-evidence.json");
if (!fs.existsSync(evidencePath)) {
  throw new Error("private target evidence is missing; run npm run check:private-target");
}
const evidence = JSON.parse(fs.readFileSync(evidencePath, "utf8"));
const checkpoint = loadCurrentCheckpoint(path.join(root, "docs", "current-checkpoint.json"));
assertValidPrivateTargetEvidence(evidence, checkpoint);

function git(arguments_) {
  const result = spawnSync("git", arguments_, {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`git ${arguments_.join(" ")} failed: ${result.stderr.trim()}`);
  }
  return result.stdout.trim();
}

const implementationCommit = evidence.repository.implementationCommit;
const implementationTree = git(["rev-parse", `${implementationCommit}^{tree}`]);
if (implementationTree !== evidence.repository.implementationTree) {
  throw new Error("private target evidence implementation tree does not match Git");
}
git(["merge-base", "--is-ancestor", implementationCommit, "HEAD"]);
console.log(
  `private target evidence passed: ${implementationCommit.slice(0, 12)}, ` +
    `${evidence.semantic.node.supportedCalls} calls, payload ${evidence.payloadSha256}`,
);
