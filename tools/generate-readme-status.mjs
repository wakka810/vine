import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import {
  loadCurrentCheckpoint,
  renderReadmeStatus,
  replaceReadmeStatus,
} from "./current-checkpoint.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifest = loadCurrentCheckpoint(
  path.join(root, "docs", "current-checkpoint.json"),
);
const readmePath = path.join(root, "README.md");
const current = fs.readFileSync(readmePath, "utf8");
const expected = replaceReadmeStatus(current, renderReadmeStatus(manifest));

if (process.argv.slice(2).includes("--check")) {
  if (current !== expected) {
    console.error("README.md: generated current status is stale; run npm run docs:generate");
    process.exit(1);
  }
  console.log("README current status is up to date");
} else {
  fs.writeFileSync(readmePath, expected);
  console.log("README current status generated from docs/current-checkpoint.json");
}
