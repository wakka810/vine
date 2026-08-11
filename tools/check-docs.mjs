import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const active = [
  "README.md",
  "AGENTS.md",
  "docs/README.md",
  "docs/requirements.md",
  "docs/target-architecture.md",
  "docs/roadmap.md",
  "docs/testing-and-acceptance.md",
  "docs/risks-and-open-questions.md",
  "docs/checks-and-ci.md",
];
const errors = [];

for (const relative of active) {
  const file = path.join(root, relative);
  const content = fs.readFileSync(file, "utf8");
  if (!content.endsWith("\n")) {
    errors.push(`${relative}: missing final newline`);
  }
  for (const [index, line] of content.split("\n").entries()) {
    if (/[ \t]+$/u.test(line)) {
      errors.push(`${relative}:${index + 1}: trailing whitespace`);
    }
  }
  const fences = content.split("\n").filter((line) => line.startsWith("```")).length;
  if (fences % 2 !== 0) {
    errors.push(`${relative}: unbalanced code fences`);
  }
  for (const match of content.matchAll(/\]\(([^)]+)\)/gu)) {
    const target = match[1];
    if (/^(?:https?:|mailto:|#)/u.test(target)) {
      continue;
    }
    const local = target.split("#", 1)[0];
    if (local.length > 0 && !fs.existsSync(path.resolve(path.dirname(file), local))) {
      errors.push(`${relative}: broken link ${target}`);
    }
  }
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log(`active documentation check passed: ${active.length} files`);
