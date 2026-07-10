import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const traceSchema = "vine.win32-trace.v1";
const summarySchema = "vine.win32-trace-summary.v1";

const categoryApis = {
  file: new Set(["CreateFileA", "CreateFileW"]),
  window: new Set(["CreateWindowExA", "CreateWindowExW"]),
  graphics: new Set([
    "BitBlt",
    "StretchDIBits",
    "GdiplusStartup",
    "Direct3DCreate9",
  ]),
  input: new Set(["DirectInput8Create"]),
  audio: new Set([
    "DirectSoundCreate",
    "DirectSoundCreate8",
    "mciSendCommandA",
    "mciSendCommandW",
    "waveOutOpen",
  ]),
  com: new Set(["CoInitialize", "CoInitializeEx", "CoCreateInstance"]),
};

const compareOrdinal = (left, right) => (left < right ? -1 : left > right ? 1 : 0);

const exactlyOne = (events, kind) => {
  const matches = events.filter((event) => event.kind === kind);
  if (matches.length !== 1) {
    throw new Error(`expected one ${kind} event, found ${matches.length}`);
  }
  return matches[0];
};

const sortedCounts = (values) =>
  Object.fromEntries(
    [...values.entries()].sort(([left], [right]) => compareOrdinal(left, right)),
  );

const firstSequence = (event) =>
  Number.isInteger(event.sequence) ? event.sequence : null;

const apiEvent = (event) => ({
  sequence: firstSequence(event),
  module: event.module,
  api: event.api,
  arguments: Array.isArray(event.arguments) ? event.arguments : [],
});

const pairApiCalls = (events) => {
  const stacks = new Map();
  const pairs = [];
  let enters = 0;
  let leaves = 0;
  let maxDepth = 0;

  for (const event of events) {
    if (event.kind !== "api-enter" && event.kind !== "api-leave") {
      continue;
    }
    if (!Number.isInteger(event.threadId)) {
      throw new Error(`${event.kind} sequence ${event.sequence} has no threadId`);
    }

    const stack = stacks.get(event.threadId) ?? [];
    stacks.set(event.threadId, stack);
    if (event.kind === "api-enter") {
      enters += 1;
      stack.push({ enter: event, parent: stack.at(-1)?.enter ?? null });
      maxDepth = Math.max(maxDepth, stack.length);
      continue;
    }

    leaves += 1;
    const frame = stack.pop();
    if (frame === undefined) {
      throw new Error(`api-leave sequence ${event.sequence} has no matching enter`);
    }
    if (
      frame.enter.module.toLowerCase() !== event.module.toLowerCase() ||
      frame.enter.api !== event.api
    ) {
      throw new Error(
        `api-leave sequence ${event.sequence} closes ${event.module}!${event.api}, ` +
          `expected ${frame.enter.module}!${frame.enter.api}`,
      );
    }
    pairs.push({ enter: frame.enter, leave: event, parent: frame.parent });
  }

  for (const [threadId, stack] of stacks) {
    if (stack.length !== 0) {
      const frame = stack.at(-1);
      throw new Error(
        `thread ${threadId} has unclosed ${frame.enter.module}!${frame.enter.api}`,
      );
    }
  }

  pairs.sort((left, right) => left.enter.sequence - right.enter.sequence);
  return { pairs, enters, leaves, maxDepth };
};

const normalizedWindowsPath = (value) => value.replaceAll("/", "\\");
const windowsPathKey = (value) => normalizedWindowsPath(value).toLowerCase();
const isSuccessfulFileResult = (value) => {
  const result = String(value).toLowerCase();
  return result !== "0xffffffff" && result !== "0xffffffffffffffff";
};

export const summarizeTrace = (buffer, sourcePath = null) => {
  const text = buffer.toString("utf8");
  const lines = text.split(/\r?\n/);
  if (lines.at(-1) === "") {
    lines.pop();
  }
  if (lines.length === 0) {
    throw new Error("trace is empty");
  }

  const events = lines.map((line, index) => {
    if (line.trim() === "") {
      throw new Error(`trace line ${index + 1} is empty`);
    }
    let event;
    try {
      event = JSON.parse(line);
    } catch (error) {
      throw new Error(`trace line ${index + 1} is not valid JSON: ${error.message}`);
    }
    if (event.schema !== traceSchema) {
      throw new Error(
        `trace line ${index + 1} has schema ${event.schema ?? "<missing>"}`,
      );
    }
    return event;
  });

  const traceStart = exactlyOne(events, "trace-start");
  const traceStop = exactlyOne(events, "trace-stop");
  const agentReady = exactlyOne(events, "agent-ready");
  if (events[0] !== traceStart) {
    throw new Error("trace-start must be the first record");
  }
  if (events.at(-1) !== traceStop) {
    throw new Error("trace-stop must be the last record");
  }
  if (!Number.isInteger(agentReady.sequence)) {
    throw new Error("agent-ready must have an integer sequence");
  }
  const sequenced = events.filter((event) => event.sequence !== undefined);
  let previousSequence = -1;
  for (const event of sequenced) {
    if (!Number.isInteger(event.sequence) || event.sequence < 0) {
      throw new Error(`invalid trace sequence: ${event.sequence}`);
    }
    if (event.sequence <= previousSequence) {
      throw new Error(
        `trace sequence ${event.sequence} follows ${previousSequence}`,
      );
    }
    if (event.sequence !== previousSequence + 1) {
      throw new Error(
        `trace sequence ${event.sequence} is not contiguous after ${previousSequence}`,
      );
    }
    previousSequence = event.sequence;
  }

  const eventCounts = new Map();
  for (const event of events) {
    eventCounts.set(event.kind, (eventCounts.get(event.kind) ?? 0) + 1);
  }
  const apiPairing = pairApiCalls(events);

  const modulesByName = new Map();
  for (const event of events.filter((item) => item.kind === "module-load")) {
    const key = event.module.toLowerCase();
    const existing = modulesByName.get(key);
    if (existing === undefined) {
      modulesByName.set(key, {
        name: event.module,
        firstSequence: event.sequence,
        loadCount: 1,
        paths: new Set([event.path]),
      });
    } else {
      existing.loadCount += 1;
      existing.paths.add(event.path);
    }
  }
  const modules = [...modulesByName.values()]
    .map((module) => ({ ...module, paths: [...module.paths].sort() }))
    .sort((left, right) => left.firstSequence - right.firstSequence);

  const apiEntries = events.filter((event) => event.kind === "api-enter");
  const apisByName = new Map();
  for (const event of apiEntries) {
    const key = `${event.module.toLowerCase()}\0${event.api}`;
    const existing = apisByName.get(key);
    if (existing === undefined) {
      apisByName.set(key, {
        module: event.module,
        api: event.api,
        calls: 1,
        firstSequence: event.sequence,
        lastSequence: event.sequence,
      });
    } else {
      existing.calls += 1;
      existing.lastSequence = event.sequence;
    }
  }
  const apis = [...apisByName.values()].sort(
    (left, right) => left.firstSequence - right.firstSequence,
  );

  const categories = {};
  for (const [category, names] of Object.entries(categoryApis)) {
    const matchingEntries = apiEntries.filter((event) => names.has(event.api));
    const matchingApis = apis.filter((api) => names.has(api.api));
    categories[category] = {
      calls: matchingEntries.length,
      first: matchingEntries.length === 0 ? null : apiEvent(matchingEntries[0]),
      apis: matchingApis,
    };
  }

  const logicalFileCalls = apiPairing.pairs.filter(
    (pair) =>
      categoryApis.file.has(pair.enter.api) &&
      (pair.parent === null || !categoryApis.file.has(pair.parent.api)),
  );
  const filePaths = new Map();
  const titleRoot = normalizedWindowsPath(path.win32.dirname(traceStart.executable));
  const titleRootKey = windowsPathKey(titleRoot);
  const titlePaths = new Map();
  for (const pair of logicalFileCalls) {
    const filePath = pair.enter.arguments?.[0];
    if (typeof filePath === "string") {
      const normalizedPath = normalizedWindowsPath(filePath);
      const key = windowsPathKey(normalizedPath);
      const file = filePaths.get(key) ?? { path: normalizedPath, calls: 0 };
      file.calls += 1;
      filePaths.set(key, file);

      const titlePrefix = `${titleRootKey}\\`;
      if (key.startsWith(titlePrefix)) {
        const relativePath = normalizedPath.slice(titleRoot.length + 1);
        const relativeKey = windowsPathKey(relativePath);
        const titleFile = titlePaths.get(relativeKey) ?? {
          path: relativePath,
          calls: 0,
          successfulCalls: 0,
          failedCalls: 0,
        };
        titleFile.calls += 1;
        if (isSuccessfulFileResult(pair.leave.result)) {
          titleFile.successfulCalls += 1;
        } else {
          titleFile.failedCalls += 1;
        }
        titlePaths.set(relativeKey, titleFile);
      }
    }
  }
  const topFilePaths = [...filePaths.values()]
    .sort(
      (left, right) =>
        right.calls - left.calls || compareOrdinal(left.path, right.path),
    )
    .slice(0, 20);
  const titleFileList = [...titlePaths.values()].sort((left, right) =>
    compareOrdinal(left.path, right.path),
  );
  const failedSearchRoots = new Map();
  for (const file of titleFileList.filter((item) => item.failedCalls > 0)) {
    const separator = file.path.indexOf("\\");
    const root = separator === -1 ? "." : file.path.slice(0, separator);
    const key = root.toLowerCase();
    const searchRoot = failedSearchRoots.get(key) ?? {
      root,
      calls: 0,
      paths: new Set(),
    };
    searchRoot.calls += file.failedCalls;
    searchRoot.paths.add(windowsPathKey(file.path));
    failedSearchRoots.set(key, searchRoot);
  }

  const startedAtMs = Date.parse(traceStart.timestamp);
  const stoppedAtMs = Date.parse(traceStop.timestamp);
  if (!Number.isFinite(startedAtMs) || !Number.isFinite(stoppedAtMs)) {
    throw new Error("trace start or stop timestamp is invalid");
  }
  if (stoppedAtMs < startedAtMs) {
    throw new Error("trace stop timestamp precedes trace start");
  }

  const diagnostics = {};
  for (const kind of ["hook-error", "agent-message", "cleanup-error"]) {
    diagnostics[kind] = events.filter((event) => event.kind === kind);
  }

  return {
    schema: summarySchema,
    source: {
      path: sourcePath,
      bytes: buffer.length,
      lines: lines.length,
      sha256: crypto.createHash("sha256").update(buffer).digest("hex"),
    },
    trace: {
      executable: traceStart.executable,
      executableBytes: traceStart.executableBytes ?? null,
      executableSha256: traceStart.executableSha256 ?? null,
      startedAt: traceStart.timestamp,
      stoppedAt: traceStop.timestamp,
      durationMs: stoppedAtMs - startedAtMs,
      stopReason: traceStop.reason ?? null,
      detachedReason: traceStop.detachedReason ?? null,
      crash: traceStop.crash ?? null,
      tools: {
        python: traceStart.python ?? null,
        frida: traceStart.frida ?? null,
        platform: traceStart.platform ?? null,
      },
      captureArguments: traceStart.arguments ?? null,
      architecture: agentReady.architecture,
      pointerSize: agentReady.pointerSize,
      processId: agentReady.processId,
      sequence: {
        events: sequenced.length,
        first: sequenced.length === 0 ? null : sequenced[0].sequence,
        last: sequenced.length === 0 ? null : sequenced.at(-1).sequence,
        contiguous: true,
      },
      apiPairing: {
        enters: apiPairing.enters,
        leaves: apiPairing.leaves,
        pairs: apiPairing.pairs.length,
        maxDepth: apiPairing.maxDepth,
        balanced: apiPairing.enters === apiPairing.leaves,
      },
    },
    eventCounts: sortedCounts(eventCounts),
    diagnostics,
    modules,
    apis,
    categories,
    files: {
      hookCalls: categories.file.calls,
      logicalCalls: logicalFileCalls.length,
      uniquePaths: filePaths.size,
      topPaths: topFilePaths,
      titleRelative: {
        calls: titleFileList.reduce((total, file) => total + file.calls, 0),
        uniquePaths: titleFileList.length,
        successfulCalls: titleFileList.reduce(
          (total, file) => total + file.successfulCalls,
          0,
        ),
        failedCalls: titleFileList.reduce(
          (total, file) => total + file.failedCalls,
          0,
        ),
        successfulPaths: titleFileList
          .filter((file) => file.successfulCalls > 0)
          .map((file) => file.path),
        failedSearchRoots: [...failedSearchRoots.values()]
          .map((searchRoot) => ({
            root: searchRoot.root,
            calls: searchRoot.calls,
            uniquePaths: searchRoot.paths.size,
          }))
          .sort(
            (left, right) =>
              right.calls - left.calls || compareOrdinal(left.root, right.root),
          ),
        topFailedPaths: titleFileList
          .filter((file) => file.failedCalls > 0)
          .sort(
            (left, right) =>
              right.failedCalls - left.failedCalls ||
              compareOrdinal(left.path, right.path),
          )
          .slice(0, 20)
          .map((file) => ({ path: file.path, calls: file.failedCalls })),
      },
    },
  };
};

const parseArguments = (args) => {
  const options = {};
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--force") {
      options.force = true;
      continue;
    }
    if (argument !== "--input" && argument !== "--output") {
      throw new Error(`unknown argument: ${argument}`);
    }
    const value = args[index + 1];
    if (value === undefined) {
      throw new Error(`${argument} requires a value`);
    }
    options[argument.slice(2)] = value;
    index += 1;
  }
  if (options.input === undefined) {
    throw new Error("--input is required");
  }
  return options;
};

const main = () => {
  const options = parseArguments(process.argv.slice(2));
  const input = path.resolve(options.input);
  const buffer = fs.readFileSync(input);
  const summary = summarizeTrace(buffer, options.input);
  const json = `${JSON.stringify(summary, null, 2)}\n`;
  if (options.output === undefined) {
    process.stdout.write(json);
    return;
  }

  const output = path.resolve(options.output);
  if (input === output) {
    throw new Error("trace input and summary output must differ");
  }
  fs.mkdirSync(path.dirname(output), { recursive: true });
  try {
    fs.writeFileSync(output, json, { flag: options.force ? "w" : "wx" });
  } catch (error) {
    if (error.code === "EEXIST") {
      throw new Error(`summary output already exists (use --force): ${output}`);
    }
    throw error;
  }
  console.log(`trace summary written: ${output}`);
};

const isMain =
  process.argv[1] !== undefined &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  try {
    main();
  } catch (error) {
    console.error(`trace summary failed: ${error.message}`);
    process.exitCode = 1;
  }
}
