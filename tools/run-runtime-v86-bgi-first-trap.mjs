import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import readline from "node:readline";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const commandArguments = process.argv.slice(2);
const dispatchSystemTimeOnly = commandArguments.includes("--dispatch-system-time");
const dispatchProcessIdOnly = commandArguments.includes("--dispatch-process-id");
const dispatchThreadIdOnly = commandArguments.includes("--dispatch-thread-id");
const dispatchTickCountOnly = commandArguments.includes("--dispatch-tick-count");
const dispatchPerformanceCounter = commandArguments.includes("--dispatch-performance-counter");
const dispatchTickCount = dispatchTickCountOnly || dispatchPerformanceCounter;
const dispatchThreadId = dispatchThreadIdOnly || dispatchTickCount;
const dispatchProcessId = dispatchProcessIdOnly || dispatchThreadId;
const dispatchSystemTime = dispatchSystemTimeOnly || dispatchProcessId;
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-system-time").length,
  Number(dispatchSystemTimeOnly),
  "--dispatch-system-time may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-process-id").length,
  Number(dispatchProcessIdOnly),
  "--dispatch-process-id may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-thread-id").length,
  Number(dispatchThreadIdOnly),
  "--dispatch-thread-id may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-tick-count").length,
  Number(dispatchTickCountOnly),
  "--dispatch-tick-count may be specified only once",
);
assert.equal(
  commandArguments.filter((argument) => argument === "--dispatch-performance-counter").length,
  Number(dispatchPerformanceCounter),
  "--dispatch-performance-counter may be specified only once",
);
assert(
  Number(dispatchSystemTimeOnly) +
    Number(dispatchProcessIdOnly) +
    Number(dispatchThreadIdOnly) +
    Number(dispatchTickCountOnly) +
    Number(dispatchPerformanceCounter) <=
    1,
  "dispatch modes are mutually exclusive",
);
const positionalArguments = commandArguments.filter(
  (argument) =>
    argument !== "--dispatch-system-time" &&
    argument !== "--dispatch-process-id" &&
    argument !== "--dispatch-thread-id" &&
    argument !== "--dispatch-tick-count" &&
    argument !== "--dispatch-performance-counter",
);
assert(
  positionalArguments.length <= 2 && positionalArguments.every((argument) => !argument.startsWith("--")),
  "usage: run-runtime-v86-bgi-first-trap.mjs [--dispatch-system-time | --dispatch-process-id | --dispatch-thread-id | --dispatch-tick-count | --dispatch-performance-counter] [WASM] [BGI.exe]",
);
const wasmPath = path.resolve(
  positionalArguments[0] ??
    path.join(root, ".work", "v86-adapter-source", "build", "v86-fallback.wasm"),
);
const bgiPath = path.resolve(
  positionalArguments[1] ??
    "V:\\softs\\\u30b5\u30af\u30e9\u30ce\u8a69\\BGI.exe",
);
const outputDirectory = path.join(
  root,
  ".work",
  dispatchPerformanceCounter
    ? "v86-bgi-fifth-win32-call"
    : dispatchTickCount
      ? "v86-bgi-fourth-win32-call"
      : dispatchThreadId
        ? "v86-bgi-third-win32-call"
        : dispatchProcessId
          ? "v86-bgi-second-win32-call"
          : dispatchSystemTime
            ? "v86-bgi-first-win32-call"
            : "v86-bgi-first-trap",
);
const reportPath = path.join(outputDirectory, "report.json");

const controlledClock = {
  owner: "checkpoint-host-clock",
  sampleId: "utc-2026-07-11T00:00:00.000Z",
  utcIso: "2026-07-11T00:00:00.000Z",
  fileTimeTicks: 134_282_016_000_000_000n,
  fileTimeLow: 929_513_472,
  fileTimeHigh: 31_264_968,
};
const controlledProcess = {
  owner: "checkpoint-process-identity",
  processId: 4_096,
};
const controlledThread = {
  owner: "checkpoint-thread-identity",
  threadId: 8_192,
};
const controlledMonotonicClock = {
  owner: "checkpoint-monotonic-clock",
  sampleId: "uptime-ms-3600000",
  pairedWallClockSampleId: controlledClock.sampleId,
  tickCount: 3_600_000,
};
const controlledPerformanceCounter = {
  owner: controlledMonotonicClock.owner,
  sampleId: "qpc-ticks-12345678901",
  pairedTickSampleId: controlledMonotonicClock.sampleId,
  ticks: 12_345_678_901n,
  low: 3_755_744_309,
  high: 2,
};

const expected = {
  source: {
    bytes: 1_193_472,
    sha256: "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1",
  },
  wasm: {
    bytes: 1_882_836,
    sha256: "f68a706655c81bde9b12982bd67cb00f19b1dae0e9ca5ddfaf94e0e74c4e7e26",
    imports: 22,
    exports: 809,
  },
  image: {
    owner: 1,
    loadBase: 0x10000000,
    entry: 0x100a383e,
    pages: 380,
    readablePages: 380,
    writablePages: 100,
    executablePages: 208,
  },
  imports: {
    owner: 2,
    pages: 2,
    total: 330,
    declared: dispatchPerformanceCounter
      ? 6
      : dispatchTickCount
        ? 5
      : dispatchThreadId
        ? 4
        : dispatchProcessId
          ? 3
          : dispatchSystemTime
            ? 2
            : 1,
    unsupported: dispatchPerformanceCounter
      ? 324
      : dispatchTickCount
        ? 325
      : dispatchThreadId
        ? 326
        : dispatchProcessId
          ? 327
          : dispatchSystemTime
            ? 328
            : 329,
    first: {
      id: 1,
      address: 0x7f000000,
      iatAddress: 0x100d14f4,
      module: "d3d9.dll",
      name: "Direct3DCreate9",
    },
    second: {
      id: 2,
      address: 0x7f000010,
      iatAddress: 0x100d1050,
      module: "DINPUT8.dll",
      name: "DirectInput8Create",
    },
    last: {
      id: 330,
      address: 0x7f001490,
      iatAddress: 0x100d1058,
      module: "DSOUND.dll",
      ordinal: 11,
    },
  },
  control: {
    owner: 3,
    page: 0x7f010000,
    callbackId: 1,
    callbackAddress: 0x7f010000,
    processEntryId: 2,
    processEntryAddress: 0x7f010010,
  },
  stack: {
    owner: 4,
    base: 0x006ff000,
    top: 0x00700000,
    initialEsp: 0x006ffffc,
  },
  processEnvironment: {
    tebOwner: 5,
    pebOwner: 6,
    processParametersOwner: 7,
    commandLineOwner: 8,
    tebBase: 0x7ffde000,
    pebBase: 0x7ffdf000,
    processParametersBase: 0x7ffdc000,
    commandLineBase: 0x7ffdd000,
    commandLine: "BGI.exe",
    commandLineLengthBytes: 14,
    commandLineMaximumLengthBytes: 16,
    tebContentSha256: "5d4ad643f294d6976304f87d2912ec4a0b1f774fae1fd9a94de59749bfdcd67e",
    pebContentSha256: "780207feb049a4c658816bf0c986bb386ec04214722a21c47936cdec01ee4d39",
    processParametersContentSha256:
      "ac17565d250da1d27dc2429b2c81bd242fe26d67373495b2238eadbb762b6d33",
    commandLineContentSha256:
      "c9b83dbe193708a3418517c43a4dadd26ca50a4006b00bd11701f3bb6ef0e6cc",
  },
  memory: {
    owners: 8,
    reservedPages: 417,
    reflectedPages: 388,
    generation: 24,
  },
  reflectionContentSha256:
    "f68f79bb13b3c5a11a2d145a8ee772a0bfd5c04dfec7502f50065cc748bc3cb0",
  offerLineBytes: 4_904_602,
  receiptLineBytes: 1_494,
  terminal: {
    thunk: {
      id: 104,
      address: 0x7f000670,
      iatAddress: 0x100d1248,
      module: "KERNEL32.dll",
      name: "GetSystemTimeAsFileTime",
    },
    instructionCount: 19,
    executeStops: 19,
    budgetStops: 18,
    returnAddress: 0x100a88d2,
    stackWords: [0x100a88d2, 0x006fffec, 0, 0, 0, 0, 0, 0],
  },
  report: {
    bytes: 9_592,
    sha256: "4ed39b15de85c188b52dbafd0eda9c35e4d88f3c4aa8a9079c98f6d9d9f03840",
  },
  firstCall: {
    offerLineBytes: 4_904_569,
    requestLineBytes: 8_670,
    deltaLineBytes: 9_584,
    deltaReceiptLineBytes: 361,
    deltaGeneration: 25,
    stackPageContentSha256: "46d99e572b1fb4f18aeb340362f31a5264cfc745cb01d22e32bc00bbb19dc785",
    reconciledChangedBytes: 11,
    deltaContentSha256: "dfad2cdbcc57bb6c05b66962dc185dbb6139a458f4da3e725536168474653840",
    nextThunk: {
      identity: {
        id: 103,
        address: 0x7f000660,
        iatAddress: 0x100d1244,
        module: "KERNEL32.dll",
        name: "GetCurrentProcessId",
      },
      unsupported: true,
      returnAddress: 0x100a88de,
      stackWords: [0x100a88de, 0, 0, 0, 0, 0, 929_513_472, 31_264_968],
    },
    instructionCount: 23,
    executeStops: 4,
    budgetStops: 3,
    reportBytes: 14_540,
    reportSha256: "7457cdfb50affd0003a3860e6959d5803cc9e7edd1c9bb0358454a6046984c16",
  },
  secondCall: {
    offerLineBytes: 4_904_536,
    requestLineBytes: 8_552,
    receiptLineBytes: 845,
    stackPageContentSha256: "5aa539018b6830e5fb6ebdd82880174b37e6edd05f6933008b68d6d994c8223f",
    reconciledChangedBytes: 4,
    nextThunk: {
      identity: {
        id: 149,
        address: 0x7f000940,
        iatAddress: 0x100d12fc,
        module: "KERNEL32.dll",
        name: "GetCurrentThreadId",
      },
      unsupported: true,
      returnAddress: 0x100a88e6,
      stackWords: [0x100a88e6, 0, 0, 0, 0, 0, 929_513_472, 31_264_968],
    },
    instructionCount: 26,
    executeStops: 3,
    budgetStops: 2,
    reportBytes: 18_472,
    reportSha256: "bbce0252bd7215b0643118c49bef61c724e3c8c0f80279ede1cd5c3ba1b94e67",
  },
  thirdCall: {
    offerLineBytes: 4_904_503,
    requestLineBytes: 8_548,
    receiptLineBytes: 840,
    stackPageContentSha256: "b22f529af561901c5156e388732e0e79954527b3650fc733ae5b780319791617",
    reconciledChangedBytes: 1,
    nextThunk: {
      identity: {
        id: 13,
        address: 0x7f0000c0,
        iatAddress: 0x100d10dc,
        module: "KERNEL32.dll",
        name: "GetTickCount",
      },
      unsupported: true,
      returnAddress: 0x100a88ee,
      stackWords: [0x100a88ee, 0, 0, 0, 0, 0, 929_513_472, 31_264_968],
    },
    instructionCount: 29,
    executeStops: 3,
    budgetStops: 2,
    reportBytes: 22_383,
    reportSha256: "d6bf15bec208ed1ced26970ff96a9d9f0faebbcec55da19cfbf27588b452d4fa",
  },
  fourthCall: {
    offerLineBytes: 4_904_470,
    requestLineBytes: 8_640,
    receiptLineBytes: 926,
    stackPageContentSha256: "f70a02d880af738aa49ba1233b61151929abf67f5e370e3479858c20f312453b",
    reconciledChangedBytes: 1,
    nextThunk: {
      identity: {
        id: 157,
        address: 0x7f0009c0,
        iatAddress: 0x100d131c,
        module: "KERNEL32.dll",
        name: "QueryPerformanceCounter",
      },
      unsupported: true,
      returnAddress: 0x100a88fa,
      stackWords: [0x100a88fa, 0x006fffe4, 0, 0, 0, 0, 0, 929_513_472],
    },
    instructionCount: 34,
    executeStops: 5,
    budgetStops: 4,
    reportBytes: 26_558,
    reportSha256: "79924bf3817104a9d2762b730534d8f74dc1ca1adfd17aa75c3855bcdf9bf111",
  },
  fifthCall: {
    offerLineBytes: 4_904_437,
    requestLineBytes: 8_681,
    deltaLineBytes: 9_583,
    deltaReceiptLineBytes: 366,
    stackPageContentSha256: "a581c7ce28aa0c683d1b0489e93fbe29d24924e35cd2abe3e966d2fc7c65c4bc",
    reconciledChangedBytes: 5,
    deltaGeneration: 26,
    deltaContentSha256: "e466741f272179ad06d4777d23b91fa11b4fa526a47063c0adfa62c842c3d951",
    nextThunk: {
      identity: {
        id: 83,
        address: 0x7f000520,
        iatAddress: 0x100d11f4,
        module: "KERNEL32.dll",
        name: "GetStartupInfoW",
      },
      unsupported: true,
      returnAddress: 0x100a36e7,
      stackWords: [0x100a36e7, 0x006fff90, 0xe93f6d87, 0, 0, 0, 0, 0],
    },
    instructionCount: 78,
    executeStops: 44,
    budgetStops: 43,
    reportBytes: 31_427,
    reportSha256: "eb1e83e3a2a75483d2606ce7e92eca36acfdc8d15b69a8381813e8c738974ad1",
  },
};

const page = {
  size: 0x1000,
  present: 1,
  writable: 2,
  user: 4,
};
const protocol = {
  maximumLineBytes: 8 * 1024 * 1024,
  maximumReflectionPages: 400,
};
const guest = {
  physicalPoolBytes: 2 * 1024 * 1024,
  excludedPhysicalRanges: [{ start: 0x000a0000, end: 0x000c0000, role: "vga-mmio" }],
  instructionBudget: 1,
  maximumExecuteStops: 4_096,
};
const adapterPrivateOffset = {
  instructionCounter: 664,
  memorySize: 812,
};
const stateWord = {
  magic: 0,
  version: 1,
  wordCount: 2,
  eax: 4,
  ecx: 5,
  edx: 6,
  ebx: 7,
  esp: 8,
  ebp: 9,
  esi: 10,
  edi: 11,
  instructionPointer: 12,
  previousInstructionPointer: 13,
  flags: 14,
  cpl: 20,
  protectedMode: 21,
  operandSize32: 22,
  stackSize32: 23,
  inHalt: 24,
  segmentSelectorBase: 28,
  segmentOffsetBase: 36,
  segmentLimitBase: 44,
  segmentMetaBase: 52,
  controlRegisterBase: 60,
};
const segmentIndex = {
  cs: 1,
  fs: 4,
};
const stopReason = {
  budget: 1,
  invalidOpcode: 0x106,
  pageFault: 0x10e,
};

function requireFile(file, command) {
  if (!fs.existsSync(file)) {
    throw new Error(`${file} is missing; run ${command} first`);
  }
}

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function checkedEnd(start, size, limit, label) {
  assert(Number.isSafeInteger(start) && start >= 0, `${label} start is invalid`);
  assert(Number.isSafeInteger(size) && size >= 0, `${label} size is invalid`);
  const end = start + size;
  assert(Number.isSafeInteger(end) && end >= start, `${label} overflows`);
  assert(end <= limit, `${label} exceeds its containing range`);
  return end;
}

function executableFetchWindow(instructionPointer, executablePages) {
  for (let offset = 0; offset < 15; offset += 1) {
    const address = instructionPointer + offset;
    if (address > 0xffff_ffff) {
      return { allowed: false, deniedPage: null };
    }
    const virtualPage = (address & ~(page.size - 1)) >>> 0;
    if (!executablePages.has(virtualPage)) {
      return { allowed: false, deniedPage: virtualPage };
    }
  }
  return { allowed: true, deniedPage: null };
}

function checkedGuestEnd(address, size, label) {
  return checkedEnd(address, size, 0x1_0000_0000, label);
}

function reflectionContentSha256(pages) {
  const hasher = crypto.createHash("sha256");
  for (const reflected of pages) {
    const metadata = Buffer.alloc(11);
    metadata.writeUInt32LE(reflected.address, 0);
    metadata.writeUInt32LE(reflected.owner, 4);
    metadata[8] =
      Number(reflected.protection.read) |
      (Number(reflected.protection.write) << 1) |
      (Number(reflected.protection.execute) << 2);
    metadata[9] = Number(reflected.ownerSealed);
    metadata[10] = Number(reflected.dirty);
    hasher.update(metadata);
    hasher.update(Uint8Array.from(reflected.bytes));
  }
  return hasher.digest("hex");
}

function summarizeOwners(pages) {
  const owners = new Map();
  for (const reflected of pages) {
    let summary = owners.get(reflected.owner);
    if (summary === undefined) {
      summary = {
        owner: reflected.owner,
        pages: 0,
        firstAddress: reflected.address,
        lastAddress: reflected.address,
        readablePages: 0,
        writablePages: 0,
        executablePages: 0,
        hasher: crypto.createHash("sha256"),
      };
      owners.set(reflected.owner, summary);
    }
    summary.pages += 1;
    summary.lastAddress = reflected.address;
    summary.readablePages += Number(reflected.protection.read);
    summary.writablePages += Number(reflected.protection.write);
    summary.executablePages += Number(reflected.protection.execute);
    summary.hasher.update(Uint8Array.from(reflected.bytes));
  }
  return [...owners.values()].map(({ hasher, ...summary }) => ({
    ...summary,
    contentSha256: hasher.digest("hex"),
  }));
}

function sourceIdentity(file) {
  const bytes = fs.readFileSync(file);
  return { bytes: bytes.byteLength, sha256: sha256(bytes) };
}

async function run() {
  requireFile(wasmPath, "npm run prepare:engine");
  requireFile(bgiPath, "the primary title must be installed at its configured path");
  const sourceBefore = sourceIdentity(bgiPath);
  assert.deepEqual(sourceBefore, expected.source);

  const probeArguments = [
    "run",
    "--quiet",
    "--locked",
    "-p",
    "vine-pe-probe",
    "--",
    "--v86-reflection-session",
    "--large-image-reflection",
    "--expect-sha256",
    expected.source.sha256,
    "--prepare-memory-pages",
    String(expected.memory.reflectedPages),
    "--materialize-image-base",
    `0x${expected.image.loadBase.toString(16)}`,
    "--bind-name",
    "d3d9.dll",
    "Direct3DCreate9",
    ...(dispatchSystemTime
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "GetSystemTimeAsFileTime",
          "--system-time-delta-session",
        ]
      : []),
    ...(dispatchProcessId
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "GetCurrentProcessId",
          "--process-id-return-session",
        ]
      : []),
    ...(dispatchThreadId
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "GetCurrentThreadId",
          "--thread-id-return-session",
        ]
      : []),
    ...(dispatchTickCount
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "GetTickCount",
          "--tick-count-return-session",
        ]
      : []),
    ...(dispatchPerformanceCounter
      ? [
          "--bind-name",
          "KERNEL32.dll",
          "QueryPerformanceCounter",
          "--performance-counter-return-session",
        ]
      : []),
    "--trap-unsupported-imports",
    "--install-callback-return-sentinel",
    "--install-process-entry-return-sentinel",
    "--install-controlled-initial-stack",
    "--install-controlled-process-environment",
    "--install-controlled-process-parameters",
    "--controlled-command-line",
    expected.processEnvironment.commandLine,
    bgiPath,
  ];
  const child = spawn("cargo", probeArguments, {
    cwd: root,
    windowsHide: true,
    stdio: ["pipe", "pipe", "pipe"],
  });
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  const lineReader = readline.createInterface({ input: child.stdout, crlfDelay: Infinity });
  const lines = lineReader[Symbol.asyncIterator]();
  const exit = new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => resolve({ code, signal }));
  });
  let sessionCompleted = false;

  const nextMessage = async (label) => {
    const next = await lines.next();
    assert(!next.done, `${label} was not emitted; probe stderr: ${stderr.trim()}`);
    const lineBytes = Buffer.byteLength(next.value);
    assert(
      lineBytes <= protocol.maximumLineBytes,
      `${label} exceeds the ${protocol.maximumLineBytes}-byte protocol limit`,
    );
    return { message: JSON.parse(next.value), lineBytes };
  };

  try {
    const offerMessage = await nextMessage("reflection offer");
    const offer = offerMessage.message;
    validateOffer(offer, offerMessage.lineBytes);
    const context = await prepareV86(offer);
    const invalidationCalls = [
      "vine_cpu_state_restore/full_clear_tlb",
      "full_clear_tlb",
      "jit_clear_cache_js",
    ];
    context.wasm.full_clear_tlb();
    context.wasm.jit_clear_cache_js();

    const initialAcknowledgement = `${JSON.stringify({
      schema: "vine.v86-memory-reflection-ack",
      schemaVersion: 1,
      generation: offer.reflection.invalidation.generation,
      dataTlb: true,
      instructionFetch: true,
      generatedCode: true,
    })}\n`;
    if (dispatchSystemTime) {
      child.stdin.write(initialAcknowledgement);
    } else {
      child.stdin.end(initialAcknowledgement);
    }
    const receiptMessage = await nextMessage("reflection receipt");
    const receipt = receiptMessage.message;
    validateReceipt(receipt, offer, context, receiptMessage.lineBytes);

    const execution = executeToFirstOwnedTrap(offer, context);
    assert.equal(execution.terminal.kind, "owned-import-thunk");
    validateThunk(execution.terminal.thunk, expected.terminal.thunk, !dispatchSystemTime);
    assert.equal(execution.instructionCount, expected.terminal.instructionCount);
    assert.equal(execution.executeStops, expected.terminal.executeStops);
    assert.equal(execution.budgetStops, expected.terminal.budgetStops);
    assert.equal(execution.terminal.returnAddress, expected.terminal.returnAddress);
    assert.deepEqual(execution.terminal.stackWords, expected.terminal.stackWords);
    const systemTime = dispatchSystemTime
      ? await dispatchFirstSystemTimeCall({
          child,
          context,
          execution,
          nextMessage,
          offer,
          continueSession: dispatchProcessId,
        })
      : null;
    const processIdentity = dispatchProcessId
      ? await dispatchSecondProcessIdCall({
          child,
          context,
          execution: systemTime.continuation,
          nextMessage,
          offer,
          continueSession: dispatchThreadId,
        })
      : null;
    const threadIdentity = dispatchThreadId
      ? await dispatchThirdThreadIdCall({
          child,
          context,
          execution: processIdentity.continuation,
          nextMessage,
          offer,
          continueSession: dispatchTickCount,
        })
      : null;
    const monotonicClock = dispatchTickCount
      ? await dispatchFourthTickCountCall({
          child,
          context,
          execution: threadIdentity.continuation,
          nextMessage,
          offer,
          continueSession: dispatchPerformanceCounter,
        })
      : null;
    const performanceCounter = dispatchPerformanceCounter
      ? await dispatchFifthPerformanceCounterCall({
          child,
          context,
          execution: monotonicClock.continuation,
          nextMessage,
          offer,
        })
      : null;
    const processExit = await exit;
    assert.equal(processExit.signal, null, `reflection probe terminated by ${processExit.signal}`);
    assert.equal(processExit.code, 0, `reflection probe failed: ${stderr.trim()}`);
    sessionCompleted = true;
    const sourceAfter = sourceIdentity(bgiPath);
    assert.deepEqual(sourceAfter, sourceBefore);
    const ownerSummaries = summarizeOwners(offer.reflection.pages);
    const report = dispatchPerformanceCounter
      ? buildFifthPerformanceCounterReport({
          context,
          execution,
          invalidationCalls,
          monotonicClock,
          offer,
          offerMessage,
          ownerSummaries,
          performanceCounter,
          processIdentity,
          receipt,
          receiptMessage,
          sourceAfter,
          sourceBefore,
          systemTime,
          threadIdentity,
        })
      : dispatchTickCount
        ? buildFourthTickCountReport({
            context,
            execution,
            invalidationCalls,
            monotonicClock,
            offer,
            offerMessage,
            ownerSummaries,
            processIdentity,
            receipt,
            receiptMessage,
            sourceAfter,
            sourceBefore,
            systemTime,
            threadIdentity,
          })
        : dispatchThreadId
          ? buildThirdThreadIdReport({
              context,
              execution,
              invalidationCalls,
              offer,
              offerMessage,
              ownerSummaries,
              processIdentity,
              receipt,
              receiptMessage,
              sourceAfter,
              sourceBefore,
              systemTime,
              threadIdentity,
            })
          : dispatchProcessId
            ? buildSecondProcessIdReport({
                context,
                execution,
                invalidationCalls,
                offer,
                offerMessage,
                ownerSummaries,
                processIdentity,
                receipt,
                receiptMessage,
                sourceAfter,
                sourceBefore,
                systemTime,
              })
            : dispatchSystemTime
              ? buildFirstSystemTimeReport({
                  context,
                  execution,
                  invalidationCalls,
                  offer,
                  offerMessage,
                  ownerSummaries,
                  receipt,
                  receiptMessage,
                  sourceAfter,
                  sourceBefore,
                  systemTime,
                })
              : {
      schema: "vine.v86-bgi-first-trap",
      schemaVersion: 1,
      source: offer.source,
      sourceSnapshot: {
        immutableAcrossRun: true,
        before: sourceBefore,
        after: sourceAfter,
      },
      contract: {
        largeImageReflection: {
          maximumReflectionPages: protocol.maximumReflectionPages,
          maximumProtocolLineBytes: protocol.maximumLineBytes,
        },
        commandLine: expected.processEnvironment.commandLine,
        stopBeforeDispatchOrReturn: true,
      },
      bootstrap: {
        imageLoadBase: offer.imageLoadBase,
        entryPoint: offer.entryPointAddress,
        initialThreadStack: offer.initialThreadStack,
        initialProcessEnvironment: offer.initialProcessEnvironment,
        fsSelector: 0,
        fsBase: offer.initialProcessEnvironment.tebAddress,
      },
      imports: {
        total: offer.thunks.length,
        declared: offer.thunks.filter(({ disposition }) => disposition === undefined).length,
        unsupported: offer.thunks.filter(
          ({ disposition }) => disposition === "unsupported-trap",
        ).length,
        first: offer.thunks[0],
        last: offer.thunks.at(-1),
      },
      adapter: {
        wasm: expected.wasm,
        physicalPoolBytes: guest.physicalPoolBytes,
        excludedPhysicalRanges: guest.excludedPhysicalRanges,
        pageDirectory: context.pageDirectory,
        pageTables: [...context.pageTables.entries()].map(
          ([directoryIndex, physicalAddress]) => ({ directoryIndex, physicalAddress }),
        ),
        allocatedPages: context.allocations.length,
        reflectedPageAllocations: context.allocations.filter(({ role }) =>
          role.startsWith("reflected-owner-"),
        ).length,
      },
      reflection: {
        offerLineBytes: offerMessage.lineBytes,
        receiptLineBytes: receiptMessage.lineBytes,
        pageSize: offer.reflection.pageSize,
        pages: offer.reflection.pages.length,
        contentSha256: offer.reflection.contentSha256,
        ownerSummaries,
        invalidation: offer.reflection.invalidation,
        acknowledgement: {
          generation: receipt.acknowledgedGeneration,
          callsBeforeAck: invalidationCalls,
          pendingAfterAck: receipt.stats.pendingInvalidation ?? null,
        },
      },
      execution,
      stop: {
        stage: "thunk-dispatch",
        code: "runtime-reachable-owned-import-trap",
        message:
          "BGI entry reached an exact runtime-owned import thunk; API dispatch, return-value fabrication, stack cleanup, and guest resume were not attempted",
      },
      };
    fs.mkdirSync(outputDirectory, { recursive: true });
    const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
    const expectedReport = dispatchPerformanceCounter
      ? { bytes: expected.fifthCall.reportBytes, sha256: expected.fifthCall.reportSha256 }
      : dispatchTickCount
        ? { bytes: expected.fourthCall.reportBytes, sha256: expected.fourthCall.reportSha256 }
        : dispatchThreadId
          ? { bytes: expected.thirdCall.reportBytes, sha256: expected.thirdCall.reportSha256 }
          : dispatchProcessId
            ? { bytes: expected.secondCall.reportBytes, sha256: expected.secondCall.reportSha256 }
            : dispatchSystemTime
              ? { bytes: expected.firstCall.reportBytes, sha256: expected.firstCall.reportSha256 }
              : expected.report;
    if (expectedReport.bytes !== null) {
      assert.equal(reportBytes.byteLength, expectedReport.bytes);
      assert.equal(sha256(reportBytes), expectedReport.sha256);
    }
    fs.writeFileSync(reportPath, reportBytes);
    if (dispatchPerformanceCounter) {
      console.log(
        `v86 BGI fifth Win32 call passed: performance counter ` +
          `${controlledPerformanceCounter.ticks}, next thunk ID ` +
          `${performanceCounter.continuation.terminal.thunk.id} after ` +
          `${performanceCounter.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchTickCount) {
      console.log(
        `v86 BGI fourth Win32 call passed: tick count ${controlledMonotonicClock.tickCount}, ` +
          `next thunk ID ${monotonicClock.continuation.terminal.thunk.id} after ` +
          `${monotonicClock.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchThreadId) {
      console.log(
        `v86 BGI third Win32 call passed: thread ID ${controlledThread.threadId}, ` +
          `next thunk ID ${threadIdentity.continuation.terminal.thunk.id} after ` +
          `${threadIdentity.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchProcessId) {
      console.log(
        `v86 BGI second Win32 call passed: process ID ${controlledProcess.processId}, ` +
          `next thunk ID ${processIdentity.continuation.terminal.thunk.id} after ` +
          `${processIdentity.continuation.instructionCount} total instructions`,
      );
    } else if (dispatchSystemTime) {
      console.log(
        `v86 BGI first Win32 call passed: FILETIME generation ` +
          `${systemTime.delta.delta.generation}, next thunk ID ` +
          `${systemTime.continuation.terminal.thunk.id} after ` +
          `${systemTime.continuation.instructionCount} total instructions`,
      );
    } else {
      console.log(
        `v86 BGI first trap passed: ${offer.reflection.pages.length} pages, ` +
          `${execution.instructionCount} instructions, thunk ID ${execution.terminal.thunk.id} ` +
          `${execution.terminal.thunk.module}!${execution.terminal.thunk.name ?? `#${execution.terminal.thunk.ordinal}`}`,
      );
    }
    console.log(
      `reflection sha256 ${offer.reflection.contentSha256}; report: ` +
        `${path.relative(root, reportPath)}, ${reportBytes.byteLength} bytes, ` +
        `sha256 ${sha256(reportBytes)}`,
    );
  } finally {
    lineReader.close();
    if (!sessionCompleted && child.exitCode === null) {
      child.kill();
    }
  }
}

function validateOffer(offer, offerLineBytes) {
  assert.equal(offer.schema, "vine.v86-memory-reflection");
  assert.equal(offer.schemaVersion, 1);
  assert.deepEqual(offer.source, expected.source);
  assert.equal(offer.imageLoadBase, expected.image.loadBase);
  assert.equal(offer.entryPointAddress, expected.image.entry);
  assert.equal(offer.imageOwner, expected.image.owner);
  assert.equal(offer.thunks.length, expected.imports.total);
  assert.deepEqual(offer.firstThunk, offer.thunks[0]);
  validateThunk(offer.thunks[0], expected.imports.first, false);
  validateThunk(offer.thunks[1], expected.imports.second, true);
  validateThunk(offer.thunks.at(-1), expected.imports.last, true);
  validateThunk(
    offer.thunks[expected.terminal.thunk.id - 1],
    expected.terminal.thunk,
    !dispatchSystemTime,
  );
  validateThunk(
    offer.thunks[expected.firstCall.nextThunk.identity.id - 1],
    expected.firstCall.nextThunk.identity,
    !dispatchProcessId,
  );
  validateThunk(
    offer.thunks[expected.secondCall.nextThunk.identity.id - 1],
    expected.secondCall.nextThunk.identity,
    !dispatchThreadId,
  );
  validateThunk(
    offer.thunks[expected.thirdCall.nextThunk.identity.id - 1],
    expected.thirdCall.nextThunk.identity,
    !dispatchTickCount,
  );
  validateThunk(
    offer.thunks[expected.fourthCall.nextThunk.identity.id - 1],
    expected.fourthCall.nextThunk.identity,
    !dispatchPerformanceCounter,
  );
  assert.equal(
    offer.thunks.filter(({ disposition }) => disposition === undefined).length,
    expected.imports.declared,
  );
  assert.equal(
    offer.thunks.filter(({ disposition }) => disposition === "unsupported-trap").length,
    expected.imports.unsupported,
  );
  for (let index = 0; index < offer.thunks.length; index += 1) {
    const thunk = offer.thunks[index];
    assert.equal(thunk.id, index + 1);
    assert.equal(thunk.address, 0x7f000000 + index * 0x10);
    assert.equal(thunk.requesterOwner, expected.image.owner);
  }

  assert.equal(offer.callbackReturnSentinel.id, expected.control.callbackId);
  assert.equal(offer.callbackReturnSentinel.address, expected.control.callbackAddress);
  assert.equal(offer.callbackReturnSentinel.owner, expected.control.owner);
  assert.equal(offer.processEntryReturnSentinel.id, expected.control.processEntryId);
  assert.equal(
    offer.processEntryReturnSentinel.address,
    expected.control.processEntryAddress,
  );
  assert.equal(offer.processEntryReturnSentinel.owner, expected.control.owner);
  assert.equal(offer.initialThreadStack.owner, expected.stack.owner);
  assert.equal(offer.initialThreadStack.stackRange.start, expected.stack.base);
  assert.equal(offer.initialThreadStack.stackTop, expected.stack.top);
  assert.equal(offer.initialThreadStack.initialEsp, expected.stack.initialEsp);

  const environment = offer.initialProcessEnvironment;
  const parameters = environment.processParameters;
  assert.equal(environment.tebOwner, expected.processEnvironment.tebOwner);
  assert.equal(environment.pebOwner, expected.processEnvironment.pebOwner);
  assert.equal(environment.tebAddress, expected.processEnvironment.tebBase);
  assert.equal(environment.pebAddress, expected.processEnvironment.pebBase);
  assert.equal(parameters.processParametersOwner, expected.processEnvironment.processParametersOwner);
  assert.equal(parameters.commandLineOwner, expected.processEnvironment.commandLineOwner);
  assert.equal(
    parameters.processParametersAddress,
    expected.processEnvironment.processParametersBase,
  );
  assert.equal(
    parameters.commandLineBufferAddress,
    expected.processEnvironment.commandLineBase,
  );
  assert.equal(parameters.commandLine, expected.processEnvironment.commandLine);
  assert.equal(
    parameters.commandLineLengthBytes,
    expected.processEnvironment.commandLineLengthBytes,
  );
  assert.equal(
    parameters.commandLineMaximumLengthBytes,
    expected.processEnvironment.commandLineMaximumLengthBytes,
  );
  assert.equal(
    environment.tebContentSha256,
    expected.processEnvironment.tebContentSha256,
  );
  assert.equal(
    environment.pebContentSha256,
    expected.processEnvironment.pebContentSha256,
  );
  assert.equal(
    parameters.processParametersContentSha256,
    expected.processEnvironment.processParametersContentSha256,
  );
  assert.equal(
    parameters.commandLineContentSha256,
    expected.processEnvironment.commandLineContentSha256,
  );
  assert.deepEqual(environment.invalidation, {
    generation: expected.memory.generation,
    dataTlb: true,
    instructionFetch: true,
    generatedCode: true,
  });

  const reflected = offer.reflection;
  assert.equal(reflected.pageSize, page.size);
  assert(reflected.pages.length <= protocol.maximumReflectionPages);
  assert.equal(reflected.pages.length, expected.memory.reflectedPages);
  assert.deepEqual(reflected.invalidation, environment.invalidation);
  assert.equal(reflectionContentSha256(reflected.pages), reflected.contentSha256);
  if (expected.reflectionContentSha256 !== null) {
    assert.equal(reflected.contentSha256, expected.reflectionContentSha256);
  }
  const expectedOfferLineBytes = dispatchPerformanceCounter
    ? expected.fifthCall.offerLineBytes
    : dispatchTickCount
      ? expected.fourthCall.offerLineBytes
      : dispatchThreadId
        ? expected.thirdCall.offerLineBytes
        : dispatchProcessId
          ? expected.secondCall.offerLineBytes
          : dispatchSystemTime
            ? expected.firstCall.offerLineBytes
            : expected.offerLineBytes;
  if (expectedOfferLineBytes !== null) {
    assert.equal(offerLineBytes, expectedOfferLineBytes);
  }
  for (const reflectedPage of reflected.pages) {
    assert.equal(reflectedPage.ownerSealed, true);
    assert.equal(reflectedPage.dirty, true);
    assert.equal(reflectedPage.bytes.length, page.size);
  }
  validateReflectedLayout(reflected.pages);
}

function validateThunk(thunk, identity, unsupported) {
  for (const [key, value] of Object.entries(identity)) {
    assert.equal(thunk[key], value);
  }
  assert.equal(thunk.kind, identity.ordinal === undefined ? "name" : "ordinal");
  assert.equal(thunk.disposition, unsupported ? "unsupported-trap" : undefined);
}

function validateReflectedLayout(pages) {
  let index = 0;
  assertReflectedPage(pages[index], expected.stack.base, expected.stack.owner, true, false);
  index += 1;
  let imageReadable = 0;
  let imageWritable = 0;
  let imageExecutable = 0;
  for (let pageIndex = 0; pageIndex < expected.image.pages; pageIndex += 1) {
    const reflected = pages[index];
    assert.equal(reflected.address, expected.image.loadBase + pageIndex * page.size);
    assert.equal(reflected.owner, expected.image.owner);
    imageReadable += Number(reflected.protection.read);
    imageWritable += Number(reflected.protection.write);
    imageExecutable += Number(reflected.protection.execute);
    index += 1;
  }
  assert.equal(imageReadable, expected.image.readablePages);
  assert.equal(imageWritable, expected.image.writablePages);
  assert.equal(imageExecutable, expected.image.executablePages);
  assertReflectedPage(pages[index], 0x7f000000, expected.imports.owner, false, true);
  index += 1;
  assertReflectedPage(pages[index], 0x7f001000, expected.imports.owner, false, true);
  index += 1;
  assertReflectedPage(pages[index], expected.control.page, expected.control.owner, false, true);
  index += 1;
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.processParametersBase,
    expected.processEnvironment.processParametersOwner,
    true,
    false,
  );
  index += 1;
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.commandLineBase,
    expected.processEnvironment.commandLineOwner,
    true,
    false,
  );
  index += 1;
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.tebBase,
    expected.processEnvironment.tebOwner,
    true,
    false,
  );
  index += 1;
  assertReflectedPage(
    pages[index],
    expected.processEnvironment.pebBase,
    expected.processEnvironment.pebOwner,
    true,
    false,
  );
  index += 1;
  assert.equal(index, pages.length);
}

function assertReflectedPage(reflected, address, owner, writable, executable) {
  assert.equal(reflected.address, address);
  assert.equal(reflected.owner, owner);
  assert.deepEqual(reflected.protection, { read: true, write: writable, execute: executable });
}

async function prepareV86(offer) {
  const wasmBytes = fs.readFileSync(wasmPath);
  assert.equal(wasmBytes.byteLength, expected.wasm.bytes);
  assert.equal(sha256(wasmBytes), expected.wasm.sha256);
  const module = new WebAssembly.Module(wasmBytes);
  const imports = WebAssembly.Module.imports(module);
  const exportsDescription = WebAssembly.Module.exports(module);
  assert.equal(imports.length, expected.wasm.imports);
  assert.equal(exportsDescription.length, expected.wasm.exports);
  const exportNames = new Set(exportsDescription.map(({ name }) => name));
  for (const name of [
    "allocate_memory",
    "full_clear_tlb",
    "jit_clear_cache_js",
    "memory",
    "reset_cpu",
    "rust_init",
    "vine_cpu_state_ptr",
    "vine_cpu_state_restore",
    "vine_cpu_state_save",
    "vine_cpu_state_size",
    "vine_cpu_state_version",
    "vine_execute_budget",
    "vine_get_stop_error_code",
    "zero_memory",
  ]) {
    assert(exportNames.has(name), `missing adapter export ${name}`);
  }

  const hostImportCalls = {};
  const environment = {
    __indirect_function_table: new WebAssembly.Table({ element: "anyfunc", initial: 1_924 }),
  };
  for (const imported of imports) {
    if (imported.kind !== "function") {
      continue;
    }
    environment[imported.name] = (...args) => {
      hostImportCalls[imported.name] = (hostImportCalls[imported.name] ?? 0) + 1;
      assert.equal(
        imported.name,
        "microtick",
        `unexpected host import ${imported.name}(${args.join(", ")})`,
      );
      assert.equal(args.length, 0);
      return 0;
    };
  }

  const { instance } = await WebAssembly.instantiate(wasmBytes, { env: environment });
  const wasm = instance.exports;
  wasm.rust_init();
  let privateState = new DataView(wasm.memory.buffer);
  privateState.setUint32(adapterPrivateOffset.memorySize, guest.physicalPoolBytes, true);
  const guestMemoryBase = wasm.allocate_memory(guest.physicalPoolBytes);
  privateState = new DataView(wasm.memory.buffer);
  const physicalMemory = new Uint8Array(
    wasm.memory.buffer,
    guestMemoryBase,
    guest.physicalPoolBytes,
  );
  const physicalView = new DataView(
    physicalMemory.buffer,
    physicalMemory.byteOffset,
    physicalMemory.byteLength,
  );
  wasm.zero_memory(0, guest.physicalPoolBytes);

  const allocations = [];
  let nextPhysicalPage = page.size;
  const allocatePhysicalPage = (role) => {
    for (const excluded of guest.excludedPhysicalRanges) {
      if (nextPhysicalPage >= excluded.start && nextPhysicalPage < excluded.end) {
        nextPhysicalPage = excluded.end;
      }
    }
    checkedEnd(nextPhysicalPage, page.size, guest.physicalPoolBytes, role);
    const address = nextPhysicalPage;
    nextPhysicalPage += page.size;
    physicalMemory.fill(0, address, address + page.size);
    allocations.push({ address, role });
    return address;
  };
  const writePhysicalU32 = (address, value, label) => {
    checkedEnd(address, 4, guest.physicalPoolBytes, label);
    physicalView.setUint32(address, value >>> 0, true);
  };
  const pageDirectory = allocatePhysicalPage("page-directory");
  const pageTables = new Map();
  const mappings = new Map();
  const ensurePageTable = (virtualBase) => {
    const directoryIndex = virtualBase >>> 22;
    let table = pageTables.get(directoryIndex);
    if (table === undefined) {
      table = allocatePhysicalPage(`page-table-${directoryIndex}`);
      pageTables.set(directoryIndex, table);
      writePhysicalU32(
        pageDirectory + directoryIndex * 4,
        table | page.present | page.writable | page.user,
        "page directory entry",
      );
    }
    return table;
  };
  for (const reflected of offer.reflection.pages) {
    const virtualBase = reflected.address;
    checkedGuestEnd(virtualBase, page.size, "reflected page");
    assert.equal(virtualBase & (page.size - 1), 0);
    assert(!mappings.has(virtualBase), `duplicate mapping 0x${virtualBase.toString(16)}`);
    const table = ensurePageTable(virtualBase);
    const tableIndex = (virtualBase >>> 12) & 0x3ff;
    const physicalBase = allocatePhysicalPage(`reflected-owner-${reflected.owner}`);
    writePhysicalU32(
      table + tableIndex * 4,
      physicalBase |
        page.present |
        page.user |
        (reflected.protection.write ? page.writable : 0),
      "reflected page table entry",
    );
    physicalMemory.set(Uint8Array.from(reflected.bytes), physicalBase);
    mappings.set(virtualBase, {
      owner: reflected.owner,
      physicalBase,
      protection: { ...reflected.protection },
      role: `reflected-owner-${reflected.owner}`,
    });
  }

  const mappingFor = (address, label) => {
    checkedGuestEnd(address, 1, label);
    const virtualBase = (address & ~(page.size - 1)) >>> 0;
    const mapping = mappings.get(virtualBase);
    assert(mapping, `${label} uses unmapped guest address 0x${address.toString(16)}`);
    return mapping;
  };
  const preflightVirtualRange = (address, size, access, label) => {
    const end = checkedGuestEnd(address, size, label);
    const pages = [];
    for (let current = address; current < end; ) {
      const virtualBase = (current & ~(page.size - 1)) >>> 0;
      const mapping = mappingFor(current, label);
      assert(mapping.protection[access], `${label} is not guest-${access}able`);
      pages.push({
        address: virtualBase,
        owner: mapping.owner,
        protection: { ...mapping.protection },
      });
      current = Math.min(end, virtualBase + page.size);
    }
    return pages;
  };
  const readVirtualBytes = (address, size, label) => {
    preflightVirtualRange(address, size, "read", label);
    const bytes = new Uint8Array(size);
    for (let index = 0; index < size; index += 1) {
      const current = (address + index) >>> 0;
      const mapping = mappingFor(current, label);
      bytes[index] = physicalMemory[mapping.physicalBase + (current & (page.size - 1))];
    }
    return bytes;
  };
  const readVirtualU32 = (address, label) => {
    const bytes = readVirtualBytes(address, 4, label);
    return (
      bytes[0] |
      (bytes[1] << 8) |
      (bytes[2] << 16) |
      (bytes[3] << 24)
    ) >>> 0;
  };
  const applyDirtyPage = (dirty, label) => {
    assert.equal(dirty.address & (page.size - 1), 0, `${label} address is not page aligned`);
    assert.equal(dirty.bytes.length, page.size, `${label} is not one guest page`);
    const mapping = mappingFor(dirty.address, label);
    assert.equal(mapping.owner, dirty.owner, `${label} owner changed`);
    assert.deepEqual(mapping.protection, dirty.protection, `${label} protection changed`);
    physicalMemory.set(Uint8Array.from(dirty.bytes), mapping.physicalBase);
  };

  assert.equal(
    readVirtualU32(offer.initialThreadStack.initialEsp, "entry return sentinel"),
    expected.control.processEntryAddress,
  );
  wasm.reset_cpu();
  assert.equal(wasm.vine_cpu_state_version(), 1);
  assert.equal(wasm.vine_cpu_state_size(), 640);
  wasm.vine_cpu_state_save();
  const words = new Uint32Array(
    wasm.memory.buffer,
    wasm.vine_cpu_state_ptr(),
    wasm.vine_cpu_state_size() / 4,
  );
  assert.equal(words[stateWord.magic], 0x56494e45);
  assert.equal(words[stateWord.version], 1);
  assert.equal(words[stateWord.wordCount], 160);
  words.fill(0, stateWord.eax, stateWord.eax + 8);
  words[stateWord.esp] = offer.initialThreadStack.initialEsp;
  words[stateWord.instructionPointer] = offer.entryPointAddress;
  words[stateWord.previousInstructionPointer] = offer.entryPointAddress;
  words[stateWord.flags] = 2;
  words[stateWord.cpl] = 3;
  words[stateWord.protectedMode] = 1;
  words[stateWord.operandSize32] = 1;
  words[stateWord.stackSize32] = 1;
  words[stateWord.inHalt] = 0;
  for (let index = 0; index < 8; index += 1) {
    words[stateWord.segmentSelectorBase + index] = 0;
    words[stateWord.segmentOffsetBase + index] = 0;
    words[stateWord.segmentLimitBase + index] = 0xffffffff;
    words[stateWord.segmentMetaBase + index] = 0xf2;
  }
  words[stateWord.segmentOffsetBase + segmentIndex.fs] =
    offer.initialProcessEnvironment.tebAddress;
  words[stateWord.segmentMetaBase + segmentIndex.cs] = 0xfa;
  words[stateWord.controlRegisterBase] |= 0x80010001;
  words[stateWord.controlRegisterBase + 3] = pageDirectory;
  words[stateWord.controlRegisterBase + 4] |= 0x200;
  assert.equal(wasm.vine_cpu_state_restore(), 0);
  assert.equal(allocations.length, 393);
  assert.deepEqual([...pageTables.keys()], [1, 64, 508, 511]);
  for (const allocation of allocations) {
    for (const excluded of guest.excludedPhysicalRanges) {
      assert(
        allocation.address + page.size <= excluded.start || allocation.address >= excluded.end,
        `${allocation.role} overlaps excluded physical range ${excluded.role}`,
      );
    }
  }

  return {
    allocations,
    hostImportCalls,
    mappings,
    pageDirectory,
    pageTables,
    applyDirtyPage,
    preflightVirtualRange,
    readVirtualBytes,
    readVirtualU32,
    wasm,
  };
}

function validateReceipt(receipt, offer, context, receiptLineBytes) {
  assert.equal(receipt.schema, "vine.v86-memory-reflection-receipt");
  assert.equal(receipt.schemaVersion, 1);
  assert.equal(receipt.acknowledgedGeneration, expected.memory.generation);
  assert.equal(receipt.stats.owners, expected.memory.owners);
  assert.equal(receipt.stats.sealedOwners, expected.memory.owners);
  assert.equal(receipt.stats.regions, expected.memory.owners);
  assert.equal(receipt.stats.reservedPages, expected.memory.reservedPages);
  assert.equal(receipt.stats.committedPages, expected.memory.reflectedPages);
  assert.equal(receipt.stats.physicalPages, expected.memory.reflectedPages);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.generation, expected.memory.generation);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  assert.equal(receipt.iatValue, expected.imports.first.address);
  assert.deepEqual(
    receipt.entryBytes,
    [...context.readVirtualBytes(expected.image.entry, 16, "entry receipt bytes")],
  );
  assert.deepEqual(receipt.thunkBytes, [
    0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
    0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
  ]);
  assert.deepEqual(receipt.callbackReturnBytes, receipt.thunkBytes);
  assert.deepEqual(receipt.processEntryReturnBytes, receipt.thunkBytes);
  assert.deepEqual(receipt.initialStackTopBytes, [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10, 0x00, 0x01, 0x7f,
  ]);
  assert.deepEqual(
    receipt.tebBytes,
    [
      ...context.readVirtualBytes(
        expected.processEnvironment.tebBase,
        64,
        "TEB receipt bytes",
      ),
    ],
  );
  assert.deepEqual(
    receipt.pebBytes,
    [
      ...context.readVirtualBytes(
        expected.processEnvironment.pebBase,
        64,
        "PEB receipt bytes",
      ),
    ],
  );
  assert.deepEqual(
    receipt.processParametersBytes,
    [
      ...context.readVirtualBytes(
        expected.processEnvironment.processParametersBase,
        0x48,
        "process parameters receipt bytes",
      ),
    ],
  );
  const expectedCommandLineBytes = Buffer.concat([
    Buffer.from(expected.processEnvironment.commandLine, "utf16le"),
    Buffer.from([0, 0]),
  ]);
  assert.deepEqual(receipt.commandLineBytes, [...expectedCommandLineBytes]);
  assert.equal(receipt.stop.stage, "adapter-execution");
  assert.equal(receipt.stop.code, "entry-to-first-owned-thunk-pending");
  assert.equal(offer.reflection.invalidation.generation, receipt.acknowledgedGeneration);
  if (expected.receiptLineBytes !== null) {
    assert.equal(receiptLineBytes, expected.receiptLineBytes);
  }
}

async function dispatchFirstSystemTimeCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    8,
    "read",
    "GetSystemTimeAsFileTime stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "GetSystemTimeAsFileTime return address",
  );
  const outputAddress = context.readVirtualU32(
    trapped.esp + 4,
    "GetSystemTimeAsFileTime output pointer",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(outputAddress, execution.terminal.stackWords[1]);

  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "GetSystemTimeAsFileTime indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.terminal.thunk.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.terminal.thunk.iatAddress,
      "GetSystemTimeAsFileTime bound IAT",
    ),
    expected.terminal.thunk.address,
  );

  const outputPages = context.preflightVirtualRange(
    outputAddress,
    8,
    "write",
    "GetSystemTimeAsFileTime FILETIME output",
  );
  assert.equal(outputPages.length, 1);
  assert.equal(outputPages[0].owner, expected.stack.owner);
  assert.equal(outputPages[0].protection.execute, false);
  const stackPageAddress = (outputAddress & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const outputOffset = outputAddress - stackPageAddress;
  const outputBefore = context.readVirtualBytes(
    outputAddress,
    8,
    "FILETIME output before host write",
  );
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page writeback",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const fileTimeBytes = Buffer.alloc(8);
  fileTimeBytes.writeBigUInt64LE(controlledClock.fileTimeTicks);
  assert.equal(Number(controlledClock.fileTimeTicks & 0xffff_ffffn), controlledClock.fileTimeLow);
  assert.equal(
    Number((controlledClock.fileTimeTicks >> 32n) & 0xffff_ffffn),
    controlledClock.fileTimeHigh,
  );

  const request = {
    schema: "vine.v86-system-time-write-request",
    schemaVersion: 1,
    reflectedGeneration: offer.reflection.invalidation.generation,
    thunkId: expected.terminal.thunk.id,
    thunkAddress: expected.terminal.thunk.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes: 4,
    outputAddress,
    stackPageAddress,
    stackPageOwner: outputPages[0].owner,
    stackPageBytes: [...stackPageBytes],
    clockOwner: controlledClock.owner,
    clockSampleId: controlledClock.sampleId,
    clockUtcIso: controlledClock.utcIso,
    fileTimeLow: controlledClock.fileTimeLow,
    fileTimeHigh: controlledClock.fileTimeHigh,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  assert.equal(Buffer.byteLength(requestLine) - 1, expected.firstCall.requestLineBytes);
  child.stdin.write(requestLine);

  const deltaMessage = await nextMessage("GetSystemTimeAsFileTime memory delta");
  const delta = deltaMessage.message;
  assert.equal(delta.schema, "vine.v86-system-time-delta");
  assert.equal(delta.schemaVersion, 1);
  validateThunk(delta.thunk, expected.terminal.thunk, false);
  assert.equal(delta.callingConvention, "stdcall");
  assert.equal(delta.argumentBytes, 4);
  assert.equal(delta.stackPointer, trapped.esp);
  assert.equal(delta.returnAddress, returnAddress);
  assert.equal(delta.outputAddress, outputAddress);
  assert.equal(delta.clockOwner, controlledClock.owner);
  assert.equal(delta.clockSampleId, controlledClock.sampleId);
  assert.equal(delta.clockUtcIso, controlledClock.utcIso);
  assert.equal(delta.fileTimeTicks, String(controlledClock.fileTimeTicks));
  assert.equal(delta.fileTimeLow, controlledClock.fileTimeLow);
  assert.equal(delta.fileTimeHigh, controlledClock.fileTimeHigh);
  assert.deepEqual(delta.stackReconciliation, {
    generation: offer.reflection.invalidation.generation,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: delta.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(delta.stackReconciliation.changedBytes > 0);
  assert.equal(
    delta.stackReconciliation.changedBytes,
    expected.firstCall.reconciledChangedBytes,
  );
  assert.equal(
    delta.stackReconciliation.contentSha256,
    expected.firstCall.stackPageContentSha256,
  );
  assert.equal(delta.write.bytesWritten, 8);
  assert.equal(delta.write.dirtyPages, 1);
  assert.deepEqual(delta.write.invalidation, {
    generation: expected.firstCall.deltaGeneration,
    dataTlb: false,
    instructionFetch: false,
    generatedCode: false,
  });
  assert.equal(delta.delta.fromGeneration, offer.reflection.invalidation.generation);
  assert.equal(delta.delta.generation, expected.firstCall.deltaGeneration);
  assert.equal(delta.delta.pageSize, page.size);
  assert.deepEqual(delta.delta.mappingOperations, []);
  assert.equal(delta.delta.dirtyPages.length, 1);
  const dirty = delta.delta.dirtyPages[0];
  assert.equal(dirty.address, stackPageAddress);
  assert.equal(dirty.owner, expected.stack.owner);
  assert.equal(dirty.ownerSealed, true);
  assert.deepEqual(dirty.protection, { read: true, write: true, execute: false });
  assert.equal(dirty.bytes.length, page.size);
  const expectedStackPage = Uint8Array.from(stackPageBytes);
  expectedStackPage.set(fileTimeBytes, outputOffset);
  assert.deepEqual(dirty.bytes, [...expectedStackPage]);
  assert.equal(delta.delta.transferredBytes, page.size);
  assert.deepEqual(delta.delta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });
  assert.match(delta.delta.contentSha256, /^[0-9a-f]{64}$/);
  assert.equal(delta.delta.contentSha256, expected.firstCall.deltaContentSha256);
  if (expected.firstCall.deltaLineBytes !== null) {
    assert.equal(deltaMessage.lineBytes, expected.firstCall.deltaLineBytes);
  }

  context.applyDirtyPage(dirty, "GetSystemTimeAsFileTime stack delta");
  assert.deepEqual(
    [...context.readVirtualBytes(outputAddress, 8, "applied FILETIME output")],
    [...fileTimeBytes],
  );
  const deltaAcknowledgement = `${JSON.stringify({
    schema: "vine.v86-memory-delta-ack",
    schemaVersion: 1,
    generation: delta.delta.generation,
    dataTlb: false,
    instructionFetch: false,
    generatedCode: false,
  })}\n`;
  if (continueSession) {
    child.stdin.write(deltaAcknowledgement);
  } else {
    child.stdin.end(deltaAcknowledgement);
  }
  const receiptMessage = await nextMessage("GetSystemTimeAsFileTime delta receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-system-time-delta-receipt");
  assert.equal(receipt.schemaVersion, 1);
  assert.equal(receipt.acknowledgedGeneration, delta.delta.generation);
  assert.equal(receipt.outputAddress, outputAddress);
  assert.deepEqual(receipt.outputBytes, [...fileTimeBytes]);
  assert.equal(receipt.stats.generation, delta.delta.generation);
  assert.equal(receipt.stats.acknowledgedGeneration, delta.delta.generation);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.firstCall.deltaReceiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.firstCall.deltaReceiptLineBytes);
  }

  const resume = resumeVoidStdcall(context, trapped, returnAddress, 4);
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.firstCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.firstCall.nextThunk.identity,
      !dispatchProcessId,
    );
    assert.equal(continuation.instructionCount, expected.firstCall.instructionCount);
    assert.equal(continuation.executeStops, expected.firstCall.executeStops);
    assert.equal(continuation.budgetStops, expected.firstCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.firstCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.firstCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      outputAddress: request.outputAddress,
      outputBefore: [...outputBefore],
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    clock: {
      owner: controlledClock.owner,
      sampleId: controlledClock.sampleId,
      utcIso: controlledClock.utcIso,
      fileTimeTicks: String(controlledClock.fileTimeTicks),
      fileTimeLow: controlledClock.fileTimeLow,
      fileTimeHigh: controlledClock.fileTimeHigh,
      bytes: [...fileTimeBytes],
    },
    delta: {
      lineBytes: deltaMessage.lineBytes,
      callingConvention: delta.callingConvention,
      argumentBytes: delta.argumentBytes,
      stackReconciliation: delta.stackReconciliation,
      write: delta.write,
      delta: {
        fromGeneration: delta.delta.fromGeneration,
        generation: delta.delta.generation,
        pageSize: delta.delta.pageSize,
        mappingOperations: delta.delta.mappingOperations.length,
        dirtyPages: delta.delta.dirtyPages.length,
        dirtyPageAddress: dirty.address,
        dirtyPageOwner: dirty.owner,
        transferredBytes: delta.delta.transferredBytes,
        cacheEffects: delta.delta.cacheEffects,
        contentSha256: delta.delta.contentSha256,
      },
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchSecondProcessIdCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4,
    "read",
    "GetCurrentProcessId stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "GetCurrentProcessId return address",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "GetCurrentProcessId indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.firstCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.firstCall.nextThunk.identity.iatAddress,
      "GetCurrentProcessId bound IAT",
    ),
    expected.firstCall.nextThunk.identity.address,
  );

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before process-ID return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-process-id-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.firstCall.deltaGeneration,
    thunkId: expected.firstCall.nextThunk.identity.id,
    thunkAddress: expected.firstCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes: 0,
    stackPageAddress,
    stackPageOwner: framePages[0].owner,
    stackPageBytes: [...stackPageBytes],
    processIdentityOwner: controlledProcess.owner,
    processId: controlledProcess.processId,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.secondCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.secondCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("GetCurrentProcessId return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-process-id-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.firstCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, 0);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.processIdentityOwner, controlledProcess.owner);
  assert.equal(receipt.processId, controlledProcess.processId);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.firstCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  if (expected.secondCall.reconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.secondCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.secondCall.stackPageContentSha256);
  }
  assert.equal(receipt.stats.generation, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.secondCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.secondCall.receiptLineBytes);
  }

  const resume = resumeStdcallResult(
    context,
    trapped,
    returnAddress,
    0,
    controlledProcess.processId,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  if (expected.secondCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.secondCall.nextThunk.identity,
      !dispatchThreadId,
    );
    assert.equal(continuation.instructionCount, expected.secondCall.instructionCount);
    assert.equal(continuation.executeStops, expected.secondCall.executeStops);
    assert.equal(continuation.budgetStops, expected.secondCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.secondCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.secondCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    identity: {
      owner: controlledProcess.owner,
      processId: controlledProcess.processId,
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchThirdThreadIdCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4,
    "read",
    "GetCurrentThreadId stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "GetCurrentThreadId return address",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "GetCurrentThreadId indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.secondCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.secondCall.nextThunk.identity.iatAddress,
      "GetCurrentThreadId bound IAT",
    ),
    expected.secondCall.nextThunk.identity.address,
  );

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before thread-ID return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-thread-id-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.firstCall.deltaGeneration,
    thunkId: expected.secondCall.nextThunk.identity.id,
    thunkAddress: expected.secondCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes: 0,
    stackPageAddress,
    stackPageOwner: framePages[0].owner,
    stackPageBytes: [...stackPageBytes],
    threadIdentityOwner: controlledThread.owner,
    threadId: controlledThread.threadId,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.thirdCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.thirdCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("GetCurrentThreadId return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-thread-id-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.secondCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, 0);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.threadIdentityOwner, controlledThread.owner);
  assert.equal(receipt.threadId, controlledThread.threadId);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.firstCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  if (expected.thirdCall.reconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.thirdCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.thirdCall.stackPageContentSha256);
  }
  assert.equal(receipt.stats.generation, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.thirdCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.thirdCall.receiptLineBytes);
  }

  const resume = resumeThreadIdStdcallResult(
    context,
    trapped,
    returnAddress,
    controlledThread.threadId,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.thirdCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.thirdCall.nextThunk.identity,
      !dispatchTickCount,
    );
    assert.equal(continuation.instructionCount, expected.thirdCall.instructionCount);
    assert.equal(continuation.executeStops, expected.thirdCall.executeStops);
    assert.equal(continuation.budgetStops, expected.thirdCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.thirdCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.thirdCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    identity: {
      owner: controlledThread.owner,
      threadId: controlledThread.threadId,
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchFourthTickCountCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
  continueSession,
}) {
  const trapped = execution.finalState;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    4,
    "read",
    "GetTickCount stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(trapped.esp, "GetTickCount return address");
  assert.equal(returnAddress, execution.terminal.returnAddress);
  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "GetTickCount indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.thirdCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.thirdCall.nextThunk.identity.iatAddress,
      "GetTickCount bound IAT",
    ),
    expected.thirdCall.nextThunk.identity.address,
  );

  const stackPageAddress = (trapped.esp & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before tick-count return",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const request = {
    schema: "vine.v86-tick-count-return-request",
    schemaVersion: 1,
    reflectedGeneration: expected.firstCall.deltaGeneration,
    thunkId: expected.thirdCall.nextThunk.identity.id,
    thunkAddress: expected.thirdCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes: 0,
    stackPageAddress,
    stackPageOwner: framePages[0].owner,
    stackPageBytes: [...stackPageBytes],
    monotonicClockOwner: controlledMonotonicClock.owner,
    sampleId: controlledMonotonicClock.sampleId,
    pairedWallClockSampleId: controlledMonotonicClock.pairedWallClockSampleId,
    tickCount: controlledMonotonicClock.tickCount,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.fourthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.fourthCall.requestLineBytes);
  }
  if (continueSession) {
    child.stdin.write(requestLine);
  } else {
    child.stdin.end(requestLine);
  }

  const receiptMessage = await nextMessage("GetTickCount return receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-tick-count-return-receipt");
  assert.equal(receipt.schemaVersion, 1);
  validateThunk(receipt.thunk, expected.thirdCall.nextThunk.identity, false);
  assert.equal(receipt.callingConvention, "stdcall");
  assert.equal(receipt.argumentBytes, 0);
  assert.equal(receipt.stackPointer, trapped.esp);
  assert.equal(receipt.returnAddress, returnAddress);
  assert.equal(receipt.monotonicClockOwner, controlledMonotonicClock.owner);
  assert.equal(receipt.sampleId, controlledMonotonicClock.sampleId);
  assert.equal(
    receipt.pairedWallClockSampleId,
    controlledMonotonicClock.pairedWallClockSampleId,
  );
  assert.equal(receipt.tickCount, controlledMonotonicClock.tickCount);
  assert.deepEqual(receipt.stackReconciliation, {
    generation: expected.firstCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: receipt.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(receipt.stackReconciliation.changedBytes > 0);
  if (expected.fourthCall.reconciledChangedBytes !== null) {
    assert.equal(
      receipt.stackReconciliation.changedBytes,
      expected.fourthCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.fourthCall.stackPageContentSha256);
  }
  assert.equal(receipt.stats.generation, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.acknowledgedGeneration, expected.firstCall.deltaGeneration);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.fourthCall.receiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.fourthCall.receiptLineBytes);
  }

  const resume = resumeTickCountStdcallResult(
    context,
    trapped,
    returnAddress,
    controlledMonotonicClock.tickCount,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.fourthCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.fourthCall.nextThunk.identity,
      !dispatchPerformanceCounter,
    );
    assert.equal(continuation.instructionCount, expected.fourthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.fourthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.fourthCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.fourthCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.fourthCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    clock: {
      owner: controlledMonotonicClock.owner,
      sampleId: controlledMonotonicClock.sampleId,
      pairedWallClockSampleId: controlledMonotonicClock.pairedWallClockSampleId,
      tickCount: controlledMonotonicClock.tickCount,
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

async function dispatchFifthPerformanceCounterCall({
  child,
  context,
  execution,
  nextMessage,
  offer,
}) {
  const trapped = execution.finalState;
  const framePages = context.preflightVirtualRange(
    trapped.esp,
    8,
    "read",
    "QueryPerformanceCounter stdcall frame",
  );
  assert.equal(framePages.length, 1);
  assert.equal(framePages[0].owner, expected.stack.owner);
  const returnAddress = context.readVirtualU32(
    trapped.esp,
    "QueryPerformanceCounter return address",
  );
  const outputAddress = context.readVirtualU32(
    trapped.esp + 4,
    "QueryPerformanceCounter output pointer",
  );
  assert.equal(returnAddress, execution.terminal.returnAddress);
  assert.equal(outputAddress, execution.terminal.stackWords[1]);

  const callSite = returnAddress - 6;
  const callBytes = context.readVirtualBytes(
    callSite,
    6,
    "QueryPerformanceCounter indirect IAT call",
  );
  assert.deepEqual([...callBytes.subarray(0, 2)], [0xff, 0x15]);
  const callView = new DataView(callBytes.buffer, callBytes.byteOffset, callBytes.byteLength);
  assert.equal(callView.getUint32(2, true), expected.fourthCall.nextThunk.identity.iatAddress);
  assert.equal(
    context.readVirtualU32(
      expected.fourthCall.nextThunk.identity.iatAddress,
      "QueryPerformanceCounter bound IAT",
    ),
    expected.fourthCall.nextThunk.identity.address,
  );

  const outputPages = context.preflightVirtualRange(
    outputAddress,
    8,
    "write",
    "QueryPerformanceCounter LARGE_INTEGER output",
  );
  assert.equal(outputPages.length, 1);
  assert.equal(outputPages[0].owner, expected.stack.owner);
  assert.equal(outputPages[0].protection.execute, false);
  const stackPageAddress = (outputAddress & ~(page.size - 1)) >>> 0;
  assert.equal(stackPageAddress, expected.stack.base);
  const outputOffset = outputAddress - stackPageAddress;
  const outputBefore = context.readVirtualBytes(
    outputAddress,
    8,
    "performance-counter output before host write",
  );
  const stackPageBytes = context.readVirtualBytes(
    stackPageAddress,
    page.size,
    "current v86 stack page before performance-counter write",
  );
  const stackPageContentSha256 = sha256(stackPageBytes);
  const counterBytes = Buffer.alloc(8);
  counterBytes.writeBigUInt64LE(controlledPerformanceCounter.ticks);
  assert.equal(
    Number(controlledPerformanceCounter.ticks & 0xffff_ffffn),
    controlledPerformanceCounter.low,
  );
  assert.equal(
    Number((controlledPerformanceCounter.ticks >> 32n) & 0xffff_ffffn),
    controlledPerformanceCounter.high,
  );

  const request = {
    schema: "vine.v86-performance-counter-write-request",
    schemaVersion: 1,
    reflectedGeneration: expected.firstCall.deltaGeneration,
    thunkId: expected.fourthCall.nextThunk.identity.id,
    thunkAddress: expected.fourthCall.nextThunk.identity.address,
    stackPointer: trapped.esp,
    returnAddress,
    argumentBytes: 4,
    outputAddress,
    stackPageAddress,
    stackPageOwner: outputPages[0].owner,
    stackPageBytes: [...stackPageBytes],
    monotonicClockOwner: controlledPerformanceCounter.owner,
    sampleId: controlledPerformanceCounter.sampleId,
    pairedTickSampleId: controlledPerformanceCounter.pairedTickSampleId,
    counterLow: controlledPerformanceCounter.low,
    counterHigh: controlledPerformanceCounter.high,
  };
  const requestLine = `${JSON.stringify(request)}\n`;
  assert(Buffer.byteLength(requestLine) <= 64 * 1024);
  if (expected.fifthCall.requestLineBytes !== null) {
    assert.equal(Buffer.byteLength(requestLine) - 1, expected.fifthCall.requestLineBytes);
  }
  child.stdin.write(requestLine);

  const deltaMessage = await nextMessage("QueryPerformanceCounter memory delta");
  const delta = deltaMessage.message;
  assert.equal(delta.schema, "vine.v86-performance-counter-delta");
  assert.equal(delta.schemaVersion, 1);
  validateThunk(delta.thunk, expected.fourthCall.nextThunk.identity, false);
  assert.equal(delta.callingConvention, "stdcall");
  assert.equal(delta.argumentBytes, 4);
  assert.equal(delta.stackPointer, trapped.esp);
  assert.equal(delta.returnAddress, returnAddress);
  assert.equal(delta.outputAddress, outputAddress);
  assert.equal(delta.monotonicClockOwner, controlledPerformanceCounter.owner);
  assert.equal(delta.sampleId, controlledPerformanceCounter.sampleId);
  assert.equal(delta.pairedTickSampleId, controlledPerformanceCounter.pairedTickSampleId);
  assert.equal(delta.counterTicks, String(controlledPerformanceCounter.ticks));
  assert.equal(delta.counterLow, controlledPerformanceCounter.low);
  assert.equal(delta.counterHigh, controlledPerformanceCounter.high);
  assert.deepEqual(delta.stackReconciliation, {
    generation: expected.firstCall.deltaGeneration,
    address: stackPageAddress,
    owner: expected.stack.owner,
    bytesReconciled: page.size,
    changedBytes: delta.stackReconciliation.changedBytes,
    contentSha256: stackPageContentSha256,
  });
  assert(delta.stackReconciliation.changedBytes > 0);
  if (expected.fifthCall.reconciledChangedBytes !== null) {
    assert.equal(
      delta.stackReconciliation.changedBytes,
      expected.fifthCall.reconciledChangedBytes,
    );
    assert.equal(stackPageContentSha256, expected.fifthCall.stackPageContentSha256);
  }
  assert.equal(delta.write.bytesWritten, 8);
  assert.equal(delta.write.dirtyPages, 1);
  assert.deepEqual(delta.write.invalidation, {
    generation: expected.fifthCall.deltaGeneration,
    dataTlb: false,
    instructionFetch: false,
    generatedCode: false,
  });
  assert.equal(delta.delta.fromGeneration, expected.firstCall.deltaGeneration);
  assert.equal(delta.delta.generation, expected.fifthCall.deltaGeneration);
  assert.equal(delta.delta.pageSize, page.size);
  assert.deepEqual(delta.delta.mappingOperations, []);
  assert.equal(delta.delta.dirtyPages.length, 1);
  const dirty = delta.delta.dirtyPages[0];
  assert.equal(dirty.address, stackPageAddress);
  assert.equal(dirty.owner, expected.stack.owner);
  assert.equal(dirty.ownerSealed, true);
  assert.deepEqual(dirty.protection, { read: true, write: true, execute: false });
  assert.equal(dirty.bytes.length, page.size);
  const expectedStackPage = Uint8Array.from(stackPageBytes);
  expectedStackPage.set(counterBytes, outputOffset);
  assert.deepEqual(dirty.bytes, [...expectedStackPage]);
  assert.equal(delta.delta.transferredBytes, page.size);
  assert.deepEqual(delta.delta.cacheEffects, {
    flushTlb: false,
    invalidateInstructionFetch: false,
    invalidateGeneratedCode: false,
  });
  assert.match(delta.delta.contentSha256, /^[0-9a-f]{64}$/);
  if (expected.fifthCall.deltaContentSha256 !== null) {
    assert.equal(delta.delta.contentSha256, expected.fifthCall.deltaContentSha256);
  }
  if (expected.fifthCall.deltaLineBytes !== null) {
    assert.equal(deltaMessage.lineBytes, expected.fifthCall.deltaLineBytes);
  }

  context.applyDirtyPage(dirty, "QueryPerformanceCounter stack delta");
  assert.deepEqual(
    [...context.readVirtualBytes(outputAddress, 8, "applied performance-counter output")],
    [...counterBytes],
  );
  const deltaAcknowledgement = `${JSON.stringify({
    schema: "vine.v86-memory-delta-ack",
    schemaVersion: 1,
    generation: delta.delta.generation,
    dataTlb: false,
    instructionFetch: false,
    generatedCode: false,
  })}\n`;
  child.stdin.end(deltaAcknowledgement);

  const receiptMessage = await nextMessage("QueryPerformanceCounter delta receipt");
  const receipt = receiptMessage.message;
  assert.equal(receipt.schema, "vine.v86-performance-counter-delta-receipt");
  assert.equal(receipt.schemaVersion, 1);
  assert.equal(receipt.acknowledgedGeneration, delta.delta.generation);
  assert.equal(receipt.outputAddress, outputAddress);
  assert.deepEqual(receipt.outputBytes, [...counterBytes]);
  assert.equal(receipt.stats.generation, delta.delta.generation);
  assert.equal(receipt.stats.acknowledgedGeneration, delta.delta.generation);
  assert.equal(receipt.stats.dirtyPages, 0);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (expected.fifthCall.deltaReceiptLineBytes !== null) {
    assert.equal(receiptMessage.lineBytes, expected.fifthCall.deltaReceiptLineBytes);
  }

  const resume = resumePerformanceCounterStdcallResult(
    context,
    trapped,
    returnAddress,
    1,
  );
  const continuation = executeToNextOwnedTrap(offer, context, execution.instructionCount);
  assert.equal(continuation.terminal.kind, "owned-import-thunk");
  if (expected.fifthCall.nextThunk !== null) {
    validateThunk(
      continuation.terminal.thunk,
      expected.fifthCall.nextThunk.identity,
      expected.fifthCall.nextThunk.unsupported,
    );
    assert.equal(continuation.instructionCount, expected.fifthCall.instructionCount);
    assert.equal(continuation.executeStops, expected.fifthCall.executeStops);
    assert.equal(continuation.budgetStops, expected.fifthCall.budgetStops);
    assert.equal(
      continuation.terminal.returnAddress,
      expected.fifthCall.nextThunk.returnAddress,
    );
    assert.deepEqual(continuation.terminal.stackWords, expected.fifthCall.nextThunk.stackWords);
  }

  return {
    request: {
      schema: request.schema,
      schemaVersion: request.schemaVersion,
      reflectedGeneration: request.reflectedGeneration,
      thunkId: request.thunkId,
      thunkAddress: request.thunkAddress,
      stackPointer: request.stackPointer,
      returnAddress: request.returnAddress,
      argumentBytes: request.argumentBytes,
      outputAddress: request.outputAddress,
      outputBefore: [...outputBefore],
      stackPageAddress: request.stackPageAddress,
      stackPageOwner: request.stackPageOwner,
      stackPageContentSha256,
      lineBytes: Buffer.byteLength(requestLine) - 1,
    },
    clock: {
      owner: controlledPerformanceCounter.owner,
      sampleId: controlledPerformanceCounter.sampleId,
      pairedTickSampleId: controlledPerformanceCounter.pairedTickSampleId,
      ticks: String(controlledPerformanceCounter.ticks),
      low: controlledPerformanceCounter.low,
      high: controlledPerformanceCounter.high,
      bytes: [...counterBytes],
    },
    delta: {
      lineBytes: deltaMessage.lineBytes,
      callingConvention: delta.callingConvention,
      argumentBytes: delta.argumentBytes,
      stackReconciliation: delta.stackReconciliation,
      write: delta.write,
      delta: {
        fromGeneration: delta.delta.fromGeneration,
        generation: delta.delta.generation,
        pageSize: delta.delta.pageSize,
        mappingOperations: delta.delta.mappingOperations.length,
        dirtyPages: delta.delta.dirtyPages.length,
        dirtyPageAddress: dirty.address,
        dirtyPageOwner: dirty.owner,
        transferredBytes: delta.delta.transferredBytes,
        cacheEffects: delta.delta.cacheEffects,
        contentSha256: delta.delta.contentSha256,
      },
    },
    receipt: { ...receipt, lineBytes: receiptMessage.lineBytes },
    resume,
    continuation,
  };
}

function captureCpuState(context, reason) {
  context.wasm.vine_cpu_state_save();
  const words = Uint32Array.from(
    new Uint32Array(
      context.wasm.memory.buffer,
      context.wasm.vine_cpu_state_ptr(),
      context.wasm.vine_cpu_state_size() / 4,
    ),
  );
  const privateState = new DataView(context.wasm.memory.buffer);
  return {
    reason,
    errorCode: context.wasm.vine_get_stop_error_code(),
    instructionCount: privateState.getUint32(adapterPrivateOffset.instructionCounter, true),
    eax: words[stateWord.eax],
    ecx: words[stateWord.ecx],
    edx: words[stateWord.edx],
    ebx: words[stateWord.ebx],
    esp: words[stateWord.esp],
    ebp: words[stateWord.ebp],
    esi: words[stateWord.esi],
    edi: words[stateWord.edi],
    flags: words[stateWord.flags],
    faultAddress: words[stateWord.controlRegisterBase + 2],
    instructionPointer: words[stateWord.instructionPointer],
    previousInstructionPointer: words[stateWord.previousInstructionPointer],
    fsSelector: words[stateWord.segmentSelectorBase + segmentIndex.fs],
    fsBase: words[stateWord.segmentOffsetBase + segmentIndex.fs],
  };
}

function resumeVoidStdcall(context, trapped, returnAddress, argumentBytes) {
  assert.equal(argumentBytes, 4);
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    trapped.eax,
    "GetSystemTimeAsFileTime",
  );
  assert.equal(restored.resumed.eax, trapped.eax, "void API return must not fabricate EAX");
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    voidResultPolicy: "preserve-eax-checkpoint-selection",
  };
}

function resumeStdcallResult(context, trapped, returnAddress, argumentBytes, result) {
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "GetCurrentProcessId",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledProcess.owner,
  };
}

function resumeThreadIdStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 0;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "GetCurrentThreadId",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledThread.owner,
  };
}

function resumeTickCountStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 0;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "GetTickCount",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: controlledMonotonicClock.owner,
  };
}

function resumePerformanceCounterStdcallResult(context, trapped, returnAddress, result) {
  const argumentBytes = 4;
  const restored = restoreStdcall(
    context,
    trapped,
    returnAddress,
    argumentBytes,
    result,
    "QueryPerformanceCounter",
  );
  assert.equal(restored.resumed.eax, result);
  return {
    callingConvention: "stdcall",
    argumentBytes,
    returnAddress,
    stackPointerBefore: trapped.esp,
    stackPointerAfter: restored.resumedEsp,
    eaxBefore: trapped.eax,
    eaxAfter: restored.resumed.eax,
    resultSource: "BOOL-success-after-controlled-counter-write",
  };
}

function restoreStdcall(context, trapped, returnAddress, argumentBytes, eax, label) {
  context.wasm.vine_cpu_state_save();
  const words = new Uint32Array(
    context.wasm.memory.buffer,
    context.wasm.vine_cpu_state_ptr(),
    context.wasm.vine_cpu_state_size() / 4,
  );
  assert.equal(words[stateWord.instructionPointer], trapped.instructionPointer);
  assert.equal(words[stateWord.previousInstructionPointer], trapped.previousInstructionPointer);
  assert.equal(words[stateWord.esp], trapped.esp);
  assert.equal(words[stateWord.eax], trapped.eax);
  const resumedEsp = checkedGuestEnd(trapped.esp, 4 + argumentBytes, `${label} stdcall cleanup`);
  words[stateWord.eax] = eax >>> 0;
  words[stateWord.esp] = resumedEsp;
  words[stateWord.instructionPointer] = returnAddress;
  words[stateWord.previousInstructionPointer] = returnAddress;
  assert.equal(context.wasm.vine_cpu_state_restore(), 0);
  const resumed = captureCpuState(context, 0);
  assert.equal(resumed.eax, eax >>> 0);
  assert.equal(resumed.esp, resumedEsp);
  assert.equal(resumed.instructionPointer, returnAddress);
  assert.equal(resumed.previousInstructionPointer, returnAddress);
  return { resumed, resumedEsp };
}

function executeToNextOwnedTrap(offer, context, initialInstructionCount) {
  const executablePages = new Set(
    offer.reflection.pages
      .filter(({ ownerSealed, protection }) => ownerSealed && protection.execute)
      .map(({ address }) => address),
  );
  const thunksByAddress = new Map(offer.thunks.map((thunk) => [thunk.address, thunk]));
  const sentinelsByAddress = new Map([
    [offer.callbackReturnSentinel.address, "callback-return-sentinel"],
    [offer.processEntryReturnSentinel.address, "process-entry-return-sentinel"],
  ]);
  let budgetStops = 0;
  let executeStops = 0;
  let terminal = null;
  let finalState = captureCpuState(context, 0);
  assert.equal(finalState.instructionCount, initialInstructionCount);
  for (let index = 0; index < guest.maximumExecuteStops; index += 1) {
    const before = captureCpuState(context, 0);
    const fetchWindow = executableFetchWindow(before.instructionPointer, executablePages);
    if (!fetchWindow.allowed) {
      terminal = {
        kind: "logical-execute-gate",
        stage: "instruction-fetch",
        code: "cross-page-execute-permission-unimplemented",
        instructionPointer: before.instructionPointer,
        deniedPage: fetchWindow.deniedPage,
        checkedBytes: 15,
      };
      finalState = before;
      break;
    }
    const reason = context.wasm.vine_execute_budget(guest.instructionBudget);
    executeStops += 1;
    const state = captureCpuState(context, reason);
    finalState = state;
    if (reason === stopReason.budget) {
      budgetStops += 1;
      continue;
    }
    terminal = classifyTerminal(state, thunksByAddress, sentinelsByAddress, context);
    break;
  }
  assert(terminal, "BGI continuation exhausted its bounded window without a terminal stop");
  assert.equal(terminal.kind, "owned-import-thunk", "BGI continuation did not reach an import");
  return {
    instructionBudget: guest.instructionBudget,
    maximumExecuteStops: guest.maximumExecuteStops,
    maximumAdditionalInstructions: guest.instructionBudget * guest.maximumExecuteStops,
    executeStops,
    budgetStops,
    initialInstructionCount,
    additionalInstructionCount: finalState.instructionCount - initialInstructionCount,
    instructionCount: finalState.instructionCount,
    finalState,
    terminal,
    hostImportCalls: context.hostImportCalls,
  };
}

function buildFirstSystemTimeReport({
  context,
  execution,
  invalidationCalls,
  offer,
  offerMessage,
  ownerSummaries,
  receipt,
  receiptMessage,
  sourceAfter,
  sourceBefore,
  systemTime,
}) {
  return {
    schema: "vine.v86-bgi-first-win32-call",
    schemaVersion: 1,
    source: offer.source,
    sourceSnapshot: {
      immutableAcrossRun: true,
      before: sourceBefore,
      after: sourceAfter,
    },
    contract: {
      largeImageReflection: {
        maximumReflectionPages: protocol.maximumReflectionPages,
        maximumProtocolLineBytes: protocol.maximumLineBytes,
      },
      commandLine: expected.processEnvironment.commandLine,
      clockOwner: controlledClock.owner,
      clockSampleId: controlledClock.sampleId,
      oneSupportedServiceDispatch: true,
      stopAtNextOwnedTrap: true,
    },
    bootstrap: {
      imageLoadBase: offer.imageLoadBase,
      entryPoint: offer.entryPointAddress,
      initialThreadStack: offer.initialThreadStack,
      initialProcessEnvironment: offer.initialProcessEnvironment,
      fsSelector: 0,
      fsBase: offer.initialProcessEnvironment.tebAddress,
    },
    imports: {
      total: offer.thunks.length,
      declared: offer.thunks.filter(({ disposition }) => disposition === undefined).length,
      unsupported: offer.thunks.filter(({ disposition }) => disposition === "unsupported-trap")
        .length,
      first: offer.thunks[0],
      supportedSystemTime: offer.thunks[expected.terminal.thunk.id - 1],
      last: offer.thunks.at(-1),
    },
    adapter: {
      wasm: expected.wasm,
      physicalPoolBytes: guest.physicalPoolBytes,
      excludedPhysicalRanges: guest.excludedPhysicalRanges,
      pageDirectory: context.pageDirectory,
      pageTables: [...context.pageTables.entries()].map(
        ([directoryIndex, physicalAddress]) => ({ directoryIndex, physicalAddress }),
      ),
      allocatedPages: context.allocations.length,
      reflectedPageAllocations: context.allocations.filter(({ role }) =>
        role.startsWith("reflected-owner-"),
      ).length,
    },
    reflection: {
      offerLineBytes: offerMessage.lineBytes,
      receiptLineBytes: receiptMessage.lineBytes,
      pageSize: offer.reflection.pageSize,
      pages: offer.reflection.pages.length,
      contentSha256: offer.reflection.contentSha256,
      ownerSummaries,
      invalidation: offer.reflection.invalidation,
      acknowledgement: {
        generation: receipt.acknowledgedGeneration,
        callsBeforeAck: invalidationCalls,
        pendingAfterAck: receipt.stats.pendingInvalidation ?? null,
      },
    },
    firstTrap: execution,
    systemTime: {
      request: systemTime.request,
      clock: systemTime.clock,
      delta: systemTime.delta,
      receipt: systemTime.receipt,
      resume: systemTime.resume,
    },
    continuation: systemTime.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from one checked GetSystemTimeAsFileTime call and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildSecondProcessIdReport({ processIdentity, ...firstCallArguments }) {
  const firstCall = buildFirstSystemTimeReport(firstCallArguments);
  const { oneSupportedServiceDispatch: _firstCallOnly, ...sharedContract } = firstCall.contract;
  return {
    schema: "vine.v86-bgi-second-win32-call",
    schemaVersion: 1,
    source: firstCall.source,
    sourceSnapshot: firstCall.sourceSnapshot,
    contract: {
      ...sharedContract,
      processIdentityOwner: controlledProcess.owner,
      supportedServiceDispatches: 2,
    },
    bootstrap: firstCall.bootstrap,
    imports: {
      ...firstCall.imports,
      supportedProcessId:
        firstCallArguments.offer.thunks[expected.firstCall.nextThunk.identity.id - 1],
    },
    adapter: firstCall.adapter,
    reflection: firstCall.reflection,
    firstTrap: firstCall.firstTrap,
    systemTime: firstCall.systemTime,
    firstContinuation: firstCall.continuation,
    processId: {
      request: processIdentity.request,
      identity: processIdentity.identity,
      receipt: processIdentity.receipt,
      resume: processIdentity.resume,
    },
    continuation: processIdentity.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from checked GetSystemTimeAsFileTime and GetCurrentProcessId calls and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildThirdThreadIdReport({ threadIdentity, ...secondCallArguments }) {
  const secondCall = buildSecondProcessIdReport(secondCallArguments);
  return {
    schema: "vine.v86-bgi-third-win32-call",
    schemaVersion: 1,
    source: secondCall.source,
    sourceSnapshot: secondCall.sourceSnapshot,
    contract: {
      ...secondCall.contract,
      threadIdentityOwner: controlledThread.owner,
      supportedServiceDispatches: 3,
    },
    bootstrap: secondCall.bootstrap,
    imports: {
      ...secondCall.imports,
      supportedThreadId:
        secondCallArguments.offer.thunks[expected.secondCall.nextThunk.identity.id - 1],
    },
    adapter: secondCall.adapter,
    reflection: secondCall.reflection,
    firstTrap: secondCall.firstTrap,
    systemTime: secondCall.systemTime,
    firstContinuation: secondCall.firstContinuation,
    processId: secondCall.processId,
    secondContinuation: secondCall.continuation,
    threadId: {
      request: threadIdentity.request,
      identity: threadIdentity.identity,
      receipt: threadIdentity.receipt,
      resume: threadIdentity.resume,
    },
    continuation: threadIdentity.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from checked GetSystemTimeAsFileTime, GetCurrentProcessId, and GetCurrentThreadId calls and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildFourthTickCountReport({ monotonicClock, ...thirdCallArguments }) {
  const thirdCall = buildThirdThreadIdReport(thirdCallArguments);
  return {
    schema: "vine.v86-bgi-fourth-win32-call",
    schemaVersion: 1,
    source: thirdCall.source,
    sourceSnapshot: thirdCall.sourceSnapshot,
    contract: {
      ...thirdCall.contract,
      monotonicClockOwner: controlledMonotonicClock.owner,
      monotonicClockSampleId: controlledMonotonicClock.sampleId,
      supportedServiceDispatches: 4,
    },
    bootstrap: thirdCall.bootstrap,
    imports: {
      ...thirdCall.imports,
      supportedTickCount:
        thirdCallArguments.offer.thunks[expected.thirdCall.nextThunk.identity.id - 1],
    },
    adapter: thirdCall.adapter,
    reflection: thirdCall.reflection,
    firstTrap: thirdCall.firstTrap,
    systemTime: thirdCall.systemTime,
    firstContinuation: thirdCall.firstContinuation,
    processId: thirdCall.processId,
    secondContinuation: thirdCall.secondContinuation,
    threadId: thirdCall.threadId,
    thirdContinuation: thirdCall.continuation,
    tickCount: {
      request: monotonicClock.request,
      clock: monotonicClock.clock,
      receipt: monotonicClock.receipt,
      resume: monotonicClock.resume,
    },
    continuation: monotonicClock.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from checked GetSystemTimeAsFileTime, GetCurrentProcessId, GetCurrentThreadId, and GetTickCount calls and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function buildFifthPerformanceCounterReport({ performanceCounter, ...fourthCallArguments }) {
  const fourthCall = buildFourthTickCountReport(fourthCallArguments);
  return {
    schema: "vine.v86-bgi-fifth-win32-call",
    schemaVersion: 1,
    source: fourthCall.source,
    sourceSnapshot: fourthCall.sourceSnapshot,
    contract: {
      ...fourthCall.contract,
      performanceCounterSampleId: controlledPerformanceCounter.sampleId,
      supportedServiceDispatches: 5,
    },
    bootstrap: fourthCall.bootstrap,
    imports: {
      ...fourthCall.imports,
      supportedPerformanceCounter:
        fourthCallArguments.offer.thunks[expected.fourthCall.nextThunk.identity.id - 1],
    },
    adapter: fourthCall.adapter,
    reflection: fourthCall.reflection,
    firstTrap: fourthCall.firstTrap,
    systemTime: fourthCall.systemTime,
    firstContinuation: fourthCall.firstContinuation,
    processId: fourthCall.processId,
    secondContinuation: fourthCall.secondContinuation,
    threadId: fourthCall.threadId,
    thirdContinuation: fourthCall.thirdContinuation,
    tickCount: fourthCall.tickCount,
    fourthContinuation: fourthCall.continuation,
    performanceCounter: {
      request: performanceCounter.request,
      clock: performanceCounter.clock,
      delta: performanceCounter.delta,
      receipt: performanceCounter.receipt,
      resume: performanceCounter.resume,
    },
    continuation: performanceCounter.continuation,
    stop: {
      stage: "thunk-dispatch",
      code: "next-runtime-reachable-owned-import-trap",
      message:
        "BGI returned from checked GetSystemTimeAsFileTime, GetCurrentProcessId, GetCurrentThreadId, GetTickCount, and QueryPerformanceCounter calls and reached the next exact runtime-owned import thunk without dispatching it",
    },
  };
}

function executeToFirstOwnedTrap(offer, context) {
  const executablePages = new Set(
    offer.reflection.pages
      .filter(({ ownerSealed, protection }) => ownerSealed && protection.execute)
      .map(({ address }) => address),
  );
  const thunksByAddress = new Map(offer.thunks.map((thunk) => [thunk.address, thunk]));
  const sentinelsByAddress = new Map([
    [offer.callbackReturnSentinel.address, "callback-return-sentinel"],
    [offer.processEntryReturnSentinel.address, "process-entry-return-sentinel"],
  ]);
  const saveState = (reason) => {
    context.wasm.vine_cpu_state_save();
    const words = Uint32Array.from(
      new Uint32Array(
        context.wasm.memory.buffer,
        context.wasm.vine_cpu_state_ptr(),
        context.wasm.vine_cpu_state_size() / 4,
      ),
    );
    const privateState = new DataView(context.wasm.memory.buffer);
    return {
      reason,
      errorCode: context.wasm.vine_get_stop_error_code(),
      instructionCount: privateState.getUint32(
        adapterPrivateOffset.instructionCounter,
        true,
      ),
      eax: words[stateWord.eax],
      ecx: words[stateWord.ecx],
      edx: words[stateWord.edx],
      ebx: words[stateWord.ebx],
      esp: words[stateWord.esp],
      ebp: words[stateWord.ebp],
      esi: words[stateWord.esi],
      edi: words[stateWord.edi],
      flags: words[stateWord.flags],
      faultAddress: words[stateWord.controlRegisterBase + 2],
      instructionPointer: words[stateWord.instructionPointer],
      previousInstructionPointer: words[stateWord.previousInstructionPointer],
      fsSelector: words[stateWord.segmentSelectorBase + segmentIndex.fs],
      fsBase: words[stateWord.segmentOffsetBase + segmentIndex.fs],
    };
  };

  const initial = saveState(0);
  assert.equal(initial.instructionPointer, expected.image.entry);
  assert.equal(initial.esp, expected.stack.initialEsp);
  assert.equal(initial.fsSelector, 0);
  assert.equal(initial.fsBase, expected.processEnvironment.tebBase);
  let budgetStops = 0;
  let executeStops = 0;
  let terminal = null;
  let finalState = initial;
  for (let index = 0; index < guest.maximumExecuteStops; index += 1) {
    const before = saveState(0);
    const fetchWindow = executableFetchWindow(before.instructionPointer, executablePages);
    if (!fetchWindow.allowed) {
      terminal = {
        kind: "logical-execute-gate",
        stage: "instruction-fetch",
        code: "cross-page-execute-permission-unimplemented",
        instructionPointer: before.instructionPointer,
        deniedPage: fetchWindow.deniedPage,
        checkedBytes: 15,
      };
      finalState = before;
      break;
    }
    const reason = context.wasm.vine_execute_budget(guest.instructionBudget);
    executeStops += 1;
    const state = saveState(reason);
    finalState = state;
    if (reason === stopReason.budget) {
      budgetStops += 1;
      continue;
    }
    terminal = classifyTerminal(state, thunksByAddress, sentinelsByAddress, context);
    break;
  }
  assert(terminal, "BGI exhausted its bounded execution window without a terminal stop");
  return {
    instructionBudget: guest.instructionBudget,
    maximumExecuteStops: guest.maximumExecuteStops,
    maximumInstructions: guest.instructionBudget * guest.maximumExecuteStops,
    executeStops,
    budgetStops,
    instructionCount: finalState.instructionCount,
    finalState: {
      reason: finalState.reason,
      errorCode: finalState.errorCode,
      instructionPointer: finalState.instructionPointer,
      previousInstructionPointer: finalState.previousInstructionPointer,
      faultAddress: finalState.faultAddress,
      eax: finalState.eax,
      ecx: finalState.ecx,
      edx: finalState.edx,
      ebx: finalState.ebx,
      esp: finalState.esp,
      ebp: finalState.ebp,
      esi: finalState.esi,
      edi: finalState.edi,
      flags: finalState.flags,
      fsSelector: finalState.fsSelector,
      fsBase: finalState.fsBase,
    },
    terminal,
    hostImportCalls: context.hostImportCalls,
  };
}

function classifyTerminal(state, thunksByAddress, sentinelsByAddress, context) {
  if (state.reason === stopReason.invalidOpcode) {
    const thunk = thunksByAddress.get(state.instructionPointer);
    if (thunk !== undefined && state.previousInstructionPointer === thunk.address) {
      const stackWords = [];
      for (let index = 0; index < 8; index += 1) {
        stackWords.push(
          context.readVirtualU32(state.esp + index * 4, `trap stack word ${index}`),
        );
      }
      return {
        kind: "owned-import-thunk",
        thunk,
        returnAddress: stackWords[0],
        stackWords,
      };
    }
    const sentinel = sentinelsByAddress.get(state.instructionPointer);
    if (sentinel !== undefined && state.previousInstructionPointer === state.instructionPointer) {
      return { kind: sentinel, address: state.instructionPointer };
    }
    return {
      kind: "unowned-invalid-opcode",
      instructionPointer: state.instructionPointer,
      previousInstructionPointer: state.previousInstructionPointer,
    };
  }
  if (state.reason === stopReason.pageFault) {
    return {
      kind: "page-fault",
      instructionPointer: state.instructionPointer,
      previousInstructionPointer: state.previousInstructionPointer,
      faultAddress: state.faultAddress,
      pageFaultErrorCode: state.errorCode,
    };
  }
  return {
    kind: "cpu-stop",
    reason: state.reason,
    errorCode: state.errorCode,
    instructionPointer: state.instructionPointer,
    previousInstructionPointer: state.previousInstructionPointer,
  };
}

run().catch((error) => {
  console.error(error.stack ?? error.message);
  process.exitCode = 1;
});
