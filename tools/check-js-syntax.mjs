import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const ignoredDirectories = new Set([".git", ".work", "node_modules", "target"]);
const files = [];
const visit = (directory) => {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (entry.isDirectory() && ignoredDirectories.has(entry.name)) {
      continue;
    }
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      visit(absolute);
    } else if (entry.isFile() && /\.(?:mjs|js)$/u.test(entry.name)) {
      files.push(path.relative(root, absolute));
    }
  }
};
visit(root);
files.sort();
assert(files.length > 0, "no JavaScript files were found");

for (const file of files) {
  const checked = spawnSync(process.execPath, ["--check", file], {
    cwd: root,
    encoding: "utf8",
  });
  if (checked.status !== 0) {
    process.stderr.write(checked.stdout);
    process.stderr.write(checked.stderr);
    throw new Error(`JavaScript syntax check failed for ${file}`);
  }
}

console.log(`JavaScript syntax check passed for ${files.length} files`);
