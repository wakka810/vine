import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const docsDirectory = path.join(root, "docs");
const tick = String.fromCharCode(96);
const errors = [];

const checkpointPath = path.join(docsDirectory, "current-checkpoint.json");
try {
  const checkpoint = JSON.parse(fs.readFileSync(checkpointPath, "utf8"));
  if (
    checkpoint.schema !== "vine.current-checkpoint" ||
    checkpoint.schemaVersion !== 1 ||
    checkpoint.checkpoint !== "bgi-ninth-win32-call" ||
    checkpoint.entryInstructionCount !== 131 ||
    checkpoint.supportedCallCount !== 9 ||
    checkpoint.lastSupportedCallId !== 72 ||
    checkpoint.lastSupportedCall !== "KERNEL32.dll!GetModuleHandleW" ||
    checkpoint.memoryGeneration !== 27 ||
    checkpoint.controlledProcessId !== 4096 ||
    checkpoint.controlledThreadId !== 8192 ||
    checkpoint.controlledTickCount !== 3600000 ||
    checkpoint.controlledPerformanceCounter !== 12345678901 ||
    checkpoint.controlledStartupInfoProfile !==
      "startupinfow-v68-zero-optional-fields" ||
    checkpoint.controlledHeapPolicy !== "heap-enable-termination-on-corruption" ||
    checkpoint.controlledPrivateHeapVariant !== "heapcreate-growable-initial-4096" ||
    checkpoint.logicalPrivateHeapHandleNamespace !== "vine.logical-private-heap.v1" ||
    checkpoint.logicalPrivateHeapHandle !== 0x68010001 ||
    checkpoint.controlledLoadedModuleVariant !==
      "getmodulehandlew-kernel32-loaded-runtime-module" ||
    checkpoint.logicalLoadedModuleHandleNamespace !== "vine.logical-loaded-module.v1" ||
    checkpoint.logicalLoadedModuleHandle !== 0x4d010001 ||
    checkpoint.ownedTrapId !== 150 ||
    checkpoint.ownedTrap !== "KERNEL32.dll!GetProcAddress" ||
    checkpoint.nextCheckpoint !== "tenth-supported-win32-call-return"
  ) {
    errors.push("docs/current-checkpoint.json: unsupported or inconsistent checkpoint values");
  }
  const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
  if (!readme.includes(`checkpoint is \`${checkpoint.checkpoint}\``)) {
    errors.push("README.md: current checkpoint does not match the checkpoint manifest");
  }
  if (!readme.includes(`thunk ID \`${checkpoint.ownedTrapId}\``)) {
    errors.push("README.md: owned trap ID does not match the checkpoint manifest");
  }
} catch (error) {
  errors.push(`docs/current-checkpoint.json: ${error.message}`);
}

const files = fs
  .readdirSync(docsDirectory)
  .filter((file) => file.endsWith(".md"))
  .sort()
  .map((file) => path.join(docsDirectory, file));

for (const file of files) {
  const text = fs.readFileSync(file, "utf8");
  const relativeFile = path.relative(root, file);

  if (!text.endsWith("\n")) {
    errors.push(`${relativeFile}: missing final newline`);
  }

  const lines = text.split("\n");
  lines.forEach((line, index) => {
    if (/[ \t]+$/.test(line)) {
      errors.push(`${relativeFile}:${index + 1}: trailing whitespace`);
    }
  });

  const fenceCount = lines.filter((line) =>
    line.startsWith(tick.repeat(3)),
  ).length;
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

  for (const match of text.matchAll(/\]\(([^)]+)\)/g)) {
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
      match ??= line.match(
        new RegExp(`^- ${tick}(AC-[A-Z0-9-]+)${tick}:`),
      );
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
    errors.push(
      `duplicate task ${id}: docs/roadmap.md:${locations.join(",")}`,
    );
  }
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}

console.log(
  `documentation check passed: ${files.length} files, ${definitions.size} IDs, ${tasks.size} tasks`,
);
