import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import {
  loadCurrentCheckpoint,
  renderReadmeStatus,
  replaceReadmeStatus,
} from "./current-checkpoint.mjs";
import { validateCurrentStateDocuments } from "./current-state-documents.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const docsDirectory = path.join(root, "docs");
const tick = String.fromCharCode(96);
const errors = [];
let currentCheckpoint;

try {
  const checkpoint = loadCurrentCheckpoint(
    path.join(docsDirectory, "current-checkpoint.json"),
  );
  currentCheckpoint = checkpoint;
  const historicalManifest = path.resolve(
    root,
    checkpoint.historicalBaseline.checkpointManifest,
  );
  if (!historicalManifest.startsWith(`${root}${path.sep}`)) {
    errors.push("docs/current-checkpoint.json: checkpoint manifest leaves the repository");
  } else if (!fs.existsSync(historicalManifest)) {
    errors.push("docs/current-checkpoint.json: historical checkpoint manifest is missing");
  }
  const engineConfiguration = path.resolve(
    root,
    checkpoint.engineDecisionEvidence.v86EngineGate.configuration,
  );
  if (!fs.existsSync(engineConfiguration)) {
    errors.push("docs/current-checkpoint.json: engine configuration is missing");
  }

  const readmePath = path.join(root, "README.md");
  const readme = fs.readFileSync(readmePath, "utf8");
  const expected = replaceReadmeStatus(readme, renderReadmeStatus(checkpoint));
  if (readme !== expected) {
    errors.push("README.md: generated current status is stale");
  }
} catch (error) {
  errors.push(`docs/current-checkpoint.json: ${error.message}`);
}

const files = fs
  .readdirSync(docsDirectory)
  .filter((file) => file.endsWith(".md"))
  .sort()
  .map((file) => path.join(docsDirectory, file));

const currentStateDocuments = new Map(
  files.map((file) => [
    path.relative(root, file).split(path.sep).join("/"),
    fs.readFileSync(file, "utf8"),
  ]),
);
errors.push(
  ...validateCurrentStateDocuments(currentStateDocuments, currentCheckpoint),
);

for (const file of files) {
  const content = fs.readFileSync(file, "utf8");
  const relativeFile = path.relative(root, file);
  if (!content.endsWith("\n")) {
    errors.push(`${relativeFile}: missing final newline`);
  }

  const lines = content.split("\n");
  lines.forEach((line, index) => {
    if (/[ \t]+$/.test(line)) {
      errors.push(`${relativeFile}:${index + 1}: trailing whitespace`);
    }
  });

  const fenceCount = lines.filter((line) => line.startsWith(tick.repeat(3))).length;
  if (fenceCount % 2 !== 0) {
    errors.push(`${relativeFile}: unbalanced code fences (${fenceCount})`);
  }

  let table = [];
  let inFence = false;
  const flushTable = () => {
    if (table.length === 0) {
      return;
    }
    const pipeCounts = new Set(
      table.map(({ line }) => (line.match(/\|/g) ?? []).length),
    );
    if (pipeCounts.size > 1) {
      errors.push(
        `${relativeFile}:${table[0].lineNumber}: inconsistent table columns`,
      );
    }
    table = [];
  };

  lines.forEach((line, index) => {
    if (line.startsWith(tick.repeat(3))) {
      flushTable();
      inFence = !inFence;
      return;
    }
    if (!inFence && line.startsWith("|") && line.endsWith("|")) {
      table.push({ line, lineNumber: index + 1 });
      return;
    }
    flushTable();
  });
  flushTable();

  for (const match of content.matchAll(/\]\(([^)]+)\)/g)) {
    const target = match[1];
    if (/^(?:https?:|mailto:|#)/.test(target)) {
      continue;
    }
    const targetPath = target.split("#", 1)[0];
    if (targetPath.length === 0) {
      continue;
    }
    const resolved = path.resolve(path.dirname(file), targetPath);
    if (!fs.existsSync(resolved)) {
      errors.push(`${relativeFile}: broken link ${target}`);
    }
  }
}

const definitions = new Map();
const addDefinition = (id, location) => {
  const locations = definitions.get(id) ?? [];
  locations.push(location);
  definitions.set(id, locations);
};

for (const file of files) {
  const relativeFile = path.relative(root, file);
  fs.readFileSync(file, "utf8")
    .split("\n")
    .forEach((line, index) => {
      let match = line.match(/^\| ((?:REQ|NFR|CON)-[A-Z0-9-]+) \|/);
      match ??= line.match(/^\| ((?:A|Q|R)-[A-Z0-9-]+) \|/);
      match ??= line.match(/^\| (ADR-[A-Z0-9-]+) \|/);
      match ??= line.match(new RegExp(`^- ${tick}(AC-[A-Z0-9-]+)${tick}:`));
      if (match) {
        addDefinition(match[1], `${relativeFile}:${index + 1}`);
      }
    });
}

for (const [id, locations] of definitions) {
  if (locations.length > 1) {
    errors.push(`duplicate definition ${id}: ${locations.join(", ")}`);
  }
}

const references = new Map();
for (const file of files) {
  const relativeFile = path.relative(root, file);
  fs.readFileSync(file, "utf8")
    .split("\n")
    .forEach((line, index) => {
      for (const match of line.matchAll(
        /(?<!-)\b(?:REQ|NFR|CON|A|Q|R|ADR|AC)-[A-Z0-9]+(?:-[A-Z0-9]+)*/g,
      )) {
        const locations = references.get(match[0]) ?? [];
        locations.push(`${relativeFile}:${index + 1}`);
        references.set(match[0], locations);
      }
    });
}

for (const [id, locations] of references) {
  const isGroupReference = [...definitions.keys()].some((definition) =>
    definition.startsWith(`${id}-`),
  );
  if (!definitions.has(id) && !isGroupReference) {
    errors.push(
      `undefined reference ${id}: ${[...new Set(locations)].slice(0, 4).join(", ")}`,
    );
  }
}

const tasks = new Map();
const roadmap = path.join(docsDirectory, "roadmap.md");
fs.readFileSync(roadmap, "utf8")
  .split("\n")
  .forEach((line, index) => {
    const match = line.match(/^\| ((?:P\d+[A-Z]?|F)-[A-Z0-9-]+) \|/);
    if (!match) {
      return;
    }
    const locations = tasks.get(match[1]) ?? [];
    locations.push(index + 1);
    tasks.set(match[1], locations);
  });

for (const [id, locations] of tasks) {
  if (locations.length > 1) {
    errors.push(`duplicate task ${id}: docs/roadmap.md:${locations.join(",")}`);
  }
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}

console.log(
  `documentation check passed: ${files.length} files, ${definitions.size} IDs, ${tasks.size} tasks`,
);
