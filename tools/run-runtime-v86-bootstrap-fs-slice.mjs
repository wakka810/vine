import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import readline from "node:readline";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { resolveV86AdapterProfile } from "./v86-adapter-profile.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const commandArguments = process.argv.slice(2);
const installProcessEnvironmentRequested = commandArguments.includes(
  "--install-process-environment",
);
const processParametersFirstStop = commandArguments.includes(
  "--process-parameters-first-stop",
);
const installProcessParameters = commandArguments.includes(
  "--install-process-parameters",
);
assert(
  Number(installProcessEnvironmentRequested) +
    Number(processParametersFirstStop) +
    Number(installProcessParameters) <=
    1,
  "choose only one bootstrap mode",
);
const installProcessEnvironment =
  installProcessEnvironmentRequested ||
  processParametersFirstStop ||
  installProcessParameters;
const useProcessParametersFixture =
  processParametersFirstStop || installProcessParameters;
const positionalArguments = commandArguments.filter(
  (value) =>
    value !== "--install-process-environment" &&
    value !== "--process-parameters-first-stop" &&
    value !== "--install-process-parameters",
);
assert(
  positionalArguments.length <= 2,
  "usage: run-runtime-v86-bootstrap-fs-slice.mjs [--install-process-environment | --process-parameters-first-stop | --install-process-parameters] [WASM] [EXE]",
);
const wasmPath = path.resolve(
  positionalArguments[0] ??
    path.join(root, ".work", "v86-adapter-source", "build", "v86-fallback.wasm"),
);
const fixturePath = path.resolve(
  positionalArguments[1] ??
    path.join(
      root,
      ".work",
      "toolchain-spike",
      useProcessParametersFixture ? "bootstrap-params.exe" : "bootstrap-fs.exe",
    ),
);
const outputDirectory = path.join(
  root,
  ".work",
  useProcessParametersFixture
    ? installProcessParameters
      ? "v86-bootstrap-params-ingress"
      : "v86-bootstrap-params-first-stop"
    : installProcessEnvironment
    ? "v86-bootstrap-fs-ingress"
    : "v86-bootstrap-fs-first-stop",
);
const reportPath = path.join(outputDirectory, "report.json");

const expected = {
  fixture: {
    bytes: 2_560,
    sha256: "5712377ff8a8bd2be292f8678902d19d3b4b2f1c1919922cce9d373f0696637c",
    loadBase: 0x10000000,
    entry: 0x10001000,
    imageOwner: 1,
  },
  processParametersFixture: {
    bytes: 2_560,
    sha256: "50a5f27f9ce65071f3d8668213eea67f67ccef7b09dc1a72d9192100de4f0c40",
    loadBase: 0x10000000,
    entry: 0x10001000,
    imageOwner: 1,
  },
  control: {
    owner: 2,
    callbackId: 1,
    callbackAddress: 0x7f010000,
    processEntryId: 2,
    processEntryAddress: 0x7f010010,
    arenaStart: 0x7f010000,
    arenaBytes: 0x1_0000,
    committedPages: 1,
    slotSize: 0x10,
    contentSha256: "bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc",
    generation: 6,
  },
  stack: {
    owner: 3,
    base: 0x006ff000,
    bytes: 0x1000,
    committedPages: 1,
    top: 0x00700000,
    initialEsp: 0x006ffffc,
    contentSha256: "3e944a15f97096aba42964f3628826766b970bbaeb98d85962f679fd725de079",
    generation: 9,
  },
  processEnvironment: {
    tebOwner: 4,
    pebOwner: 5,
    tebBase: 0x7ffde000,
    pebBase: 0x7ffdf000,
    pageBytes: 0x1000,
    committedPages: 2,
    tebSelfPointerOffset: 0x18,
    tebPebPointerOffset: 0x30,
    tebContentSha256: "5d4ad643f294d6976304f87d2912ec4a0b1f774fae1fd9a94de59749bfdcd67e",
    pebContentSha256: "ad7facb2586fc6e966c004d7d1d16b024f5805ff7cb47c7a85dabd8b48892ca7",
    generation: 15,
  },
  processParameters: {
    processParametersOwner: 6,
    commandLineOwner: 7,
    processParametersBase: 0x7ffdc000,
    commandLineBase: 0x7ffdd000,
    pageBytes: 0x1000,
    committedPages: 2,
    pebProcessParametersPointerOffset: 0x10,
    processParametersCommandLineOffset: 0x40,
    unicodeStringBufferOffset: 0x04,
    commandLine: "bootstrap-params.exe",
    commandLineUtf16Units: 20,
    commandLineLengthBytes: 40,
    commandLineMaximumLengthBytes: 42,
    pebContentSha256: "780207feb049a4c658816bf0c986bb386ec04214722a21c47936cdec01ee4d39",
    processParametersContentSha256:
      "5fb4eed3eb10240c39a428c43621e7198ccf0c3089ffff6e6aea233759533f75",
    commandLineContentSha256:
      "f6e0abfe79cb7fafa42be5f1450e91539b3d0386f7140523e44109249f1e19bd",
    generation: 21,
    executeStops: 94,
    budgetStops: 93,
  },
  reflectionContentSha256:
    "bb476b06336e93654df562ce78054e0927d5b2527ac12b54241d84a1d50b0696",
  processEnvironmentReflectionContentSha256:
    "14a986f7f10229da07f54d11fd5eda29be02e0639f06aba6151307e35f0206f3",
  processParametersFirstStopReflectionContentSha256:
    "51357b546920ec78aa535488ce8d280a8027805e1d1d24e138b85264fbab9c39",
  processParametersReflectionContentSha256:
    "f9a59953a06999030e40e962a007f1ec5f0ecb73b077c4df760e212ba3d3acbb",
  report: {
    bytes: 5_982,
    sha256: "0276cc927a3d870b10a632cf95995e79ec96cd8af5558e964b7635d2d2b7941b",
  },
  processEnvironmentReport: {
    bytes: 8_214,
    sha256: "3567ff39474f72781010fdb810b793f58424483a7663b89f2cea4b73a28748b9",
  },
  processParametersFirstStopReport: {
    bytes: 8_351,
    sha256: "b1c05c3e9df1529196276c8905f3b97aee306610c870461d458a857c80aadde5",
  },
  processParametersReport: {
    bytes: 10_521,
    sha256: "c1e76294d4bba6760cbc8d022e022ef66b557d370b567d9a498798b8b2f7ac66",
  },
};
const activeFixture = useProcessParametersFixture
  ? expected.processParametersFixture
  : expected.fixture;

const page = {
  size: 0x1000,
  present: 1,
  writable: 2,
  user: 4,
};
const guest = {
  physicalPoolBytes: 2 * 1024 * 1024,
  instructionBudget: 3,
  fsSelfOffset: 0x18,
  fsPebOffset: 0x30,
};
const adapterPrivateOffset = {
  memorySize: 812,
};
const stateWord = {
  magic: 0,
  version: 1,
  wordCount: 2,
  gprBase: 4,
  eax: 4,
  edx: 6,
  esp: 8,
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
    throw new Error(`${path.relative(root, file)} is missing; run ${command} first`);
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

function assertExecutableFetchWindow(instructionPointer, executablePages) {
  for (let offset = 0; offset < 15; offset += 1) {
    const address = instructionPointer + offset;
    assert(address <= 0xffff_ffff, "instruction fetch window crosses the guest address limit");
    const virtualPage = (address & ~(page.size - 1)) >>> 0;
    assert(
      executablePages.has(virtualPage),
      `execute gate rejected 15-byte fetch window at EIP 0x${instructionPointer.toString(16)}; ` +
        `page 0x${virtualPage.toString(16)} is not executable`,
    );
  }
}

function checkedGuestEnd(address, size, label) {
  return checkedEnd(address, size, 0x1_0000_0000, label);
}

function expectedReflectedPages() {
  const pages = [
    [0x006ff000, 3, { read: true, write: true, execute: false }],
    [0x10000000, 1, { read: true, write: false, execute: false }],
    [0x10001000, 1, { read: true, write: false, execute: true }],
    [0x10002000, 1, { read: true, write: false, execute: false }],
    [0x10003000, 1, { read: true, write: true, execute: false }],
    [0x10004000, 1, { read: true, write: false, execute: false }],
    [0x7f010000, 2, { read: true, write: false, execute: true }],
  ];
  if (installProcessParameters) {
    pages.push(
      [0x7ffdc000, 6, { read: true, write: true, execute: false }],
      [0x7ffdd000, 7, { read: true, write: true, execute: false }],
      [0x7ffde000, 4, { read: true, write: true, execute: false }],
      [0x7ffdf000, 5, { read: true, write: true, execute: false }],
    );
  } else if (installProcessEnvironment) {
    pages.push(
      [0x7ffde000, 4, { read: true, write: true, execute: false }],
      [0x7ffdf000, 5, { read: true, write: true, execute: false }],
    );
  }
  return pages;
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

function runNativeOracle() {
  if (process.platform !== "win32") {
    return {
      platform: "Windows i386 process",
      exitCode: 0,
      provenance: "recorded-windows-i386-self-oracle",
      fixtureSha256: activeFixture.sha256,
    };
  }
  const result = spawnSync(fixturePath, [], {
    cwd: path.dirname(fixturePath),
    encoding: "utf8",
    timeout: 10_000,
    windowsHide: true,
  });
  if (result.error) {
    throw result.error;
  }
  assert.equal(result.signal, null, `native fixture terminated by ${result.signal}`);
  assert.equal(
    result.status,
    0,
    `native bootstrap oracle failed: ${result.stderr.trim()}`,
  );
  return { platform: "Windows i386 process", exitCode: result.status };
}

async function run() {
  requireFile(wasmPath, "npm run prepare:engine");
  requireFile(fixturePath, "npm run check:toolchain");
  const fixtureBytes = fs.readFileSync(fixturePath);
  assert.equal(fixtureBytes.byteLength, activeFixture.bytes);
  assert.equal(sha256(fixtureBytes), activeFixture.sha256);
  const nativeOracle = runNativeOracle();

  const probeArguments = [
    "run",
    "--quiet",
    "--locked",
    "-p",
    "vine-pe-probe",
    "--",
    "--v86-reflection-session",
    "--expect-sha256",
    activeFixture.sha256,
    "--prepare-memory-pages",
    installProcessParameters ? "11" : installProcessEnvironment ? "9" : "7",
    "--materialize-image-base",
    "0x10000000",
    "--install-callback-return-sentinel",
    "--install-process-entry-return-sentinel",
    "--install-controlled-initial-stack",
  ];
  if (installProcessEnvironment) {
    probeArguments.push("--install-controlled-process-environment");
  }
  if (installProcessParameters) {
    probeArguments.push("--install-controlled-process-parameters");
  }
  probeArguments.push(fixturePath);
  const child = spawn(
    "cargo",
    probeArguments,
    {
      cwd: root,
      windowsHide: true,
      stdio: ["pipe", "pipe", "pipe"],
    },
  );
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
    assert(next.value.length <= 512 * 1024, `${label} exceeds the 512 KiB limit`);
    return JSON.parse(next.value);
  };

  try {
    const offer = await nextMessage("reflection offer");
    validateOffer(offer);
    const context = await prepareV86(offer);
    const invalidationCalls = [
      "vine_cpu_state_restore/full_clear_tlb",
      "full_clear_tlb",
      "jit_clear_cache_js",
    ];
    context.wasm.full_clear_tlb();
    context.wasm.jit_clear_cache_js();

    child.stdin.end(
      `${JSON.stringify({
        schema: "vine.v86-memory-reflection-ack",
        schemaVersion: 1,
        generation: offer.reflection.invalidation.generation,
        dataTlb: true,
        instructionFetch: true,
        generatedCode: true,
      })}\n`,
    );
    const receipt = await nextMessage("reflection receipt");
    validateReceipt(receipt, offer, context);
    const processExit = await exit;
    assert.equal(processExit.signal, null, `reflection probe terminated by ${processExit.signal}`);
    assert.equal(processExit.code, 0, `reflection probe failed: ${stderr.trim()}`);
    sessionCompleted = true;

    const execution = useProcessParametersFixture
      ? executeProcessParameters(offer, context)
      : installProcessEnvironment
        ? executeWithProcessEnvironment(offer, context)
        : executeFirstStop(offer, context);
    const report = {
      schema: useProcessParametersFixture
        ? installProcessParameters
          ? "vine.v86-bootstrap-params-ingress"
          : "vine.v86-bootstrap-params-first-stop"
        : installProcessEnvironment
          ? "vine.v86-bootstrap-fs-ingress"
          : "vine.v86-bootstrap-fs-first-stop",
      schemaVersion: 1,
      source: offer.source,
      nativeOracle,
      contract: useProcessParametersFixture
        ? {
            pebProcessParameters: "PEB+0x10",
            commandLine: "ProcessParameters+0x40",
            expectedBasename: "bootstrap-params.exe",
            entryBytes: receipt.entryBytes,
          }
        : {
            tebSelf: "FS:[0x18]",
            peb: "FS:[0x30]",
            entryBytes: receipt.entryBytes,
          },
      bootstrap: {
        imageLoadBase: offer.imageLoadBase,
        entryPoint: offer.entryPointAddress,
        initialThreadStack: offer.initialThreadStack,
        ...(installProcessEnvironment
          ? { initialProcessEnvironment: offer.initialProcessEnvironment }
          : {}),
        fsSelector: 0,
        fsBase: installProcessEnvironment
          ? offer.initialProcessEnvironment.tebAddress
          : 0,
      },
      reflection: {
        invalidation: offer.reflection.invalidation,
        contentSha256: offer.reflection.contentSha256,
        pages: offer.reflection.pages.map((reflected) => ({
          address: reflected.address,
          owner: reflected.owner,
          ownerSealed: reflected.ownerSealed,
          protection: reflected.protection,
          dirty: reflected.dirty,
          contentSha256: sha256(Uint8Array.from(reflected.bytes)),
        })),
        acknowledgement: {
          generation: receipt.acknowledgedGeneration,
          callsBeforeAck: invalidationCalls,
          pendingAfterAck: receipt.stats.pendingInvalidation ?? null,
        },
      },
      adapter: {
        wasm: context.adapterProfile.wasm,
        physicalPoolBytes: guest.physicalPoolBytes,
        pageDirectory: context.pageDirectory,
        pageTables: [...context.pageTables.entries()].map(
          ([directoryIndex, physicalAddress]) => ({ directoryIndex, physicalAddress }),
        ),
        allocatedPages: context.allocations,
        hostImportCalls: context.hostImportCalls,
      },
      ...(processParametersFirstStop || !installProcessEnvironment
        ? { firstStop: execution }
        : { execution }),
      stop: installProcessParameters
        ? {
            stage: "process-bootstrap",
            code: "broader-process-parameters-unimplemented",
            message:
              "the measured process-parameters pointer and command-line UNICODE_STRING are Rust-owned and verified through exact entry completion; current directory, environment, image path, loader lists, TLS, LastError, and general lifecycle were not attempted",
          }
        : processParametersFirstStop
        ? {
            stage: "process-bootstrap",
            code: "process-parameters-pointer-unimplemented",
            message:
              "the native-oracled fixture returned its explicit missing-process-parameters result from PEB+0x10; process-parameters storage and command-line bytes were not installed",
          }
        : installProcessEnvironment
          ? {
            stage: "process-bootstrap",
            code: "process-parameters-unimplemented",
            message:
              "the Rust-owned TEB/PEB fields and adopted FS base are verified through exact entry completion; process parameters, loader lists, TLS, LastError, and general lifecycle were not attempted",
          }
          : {
              stage: "process-bootstrap",
              code: "fs-base-teb-unimplemented",
              message:
                "the import-free fixture reached its first FS-based TEB read with a flat zero FS base; Rust-owned TEB/PEB pages and the adopted FS base were not installed",
            },
    };
    fs.mkdirSync(outputDirectory, { recursive: true });
    const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
    const expectedReport = useProcessParametersFixture
      ? installProcessParameters
        ? expected.processParametersReport
        : expected.processParametersFirstStopReport
      : installProcessEnvironment
        ? expected.processEnvironmentReport
        : expected.report;
    if (
      expectedReport.bytes !== null &&
      context.adapterProfile.hasCanonicalReportFingerprints
    ) {
      assert.equal(reportBytes.byteLength, expectedReport.bytes);
      assert.equal(sha256(reportBytes), expectedReport.sha256);
    }
    fs.writeFileSync(reportPath, reportBytes);
    if (installProcessParameters) {
      console.log(
        `v86 bootstrap params ingress passed: native exit ${nativeOracle.exitCode}, ` +
          `process parameters 0x${execution.processParametersAddress.toString(16)}, ` +
          `command line 0x${execution.fixtureObservations.commandLineBuffer.toString(16)}, ` +
          `entry result ${execution.entryResult}`,
      );
    } else if (processParametersFirstStop) {
      console.log(
        `v86 bootstrap params first stop passed: native exit ${nativeOracle.exitCode}, ` +
          `PEB 0x${execution.pebAddress.toString(16)}, process parameters ` +
          `${execution.processParametersAddress}, entry result ${execution.entryResult}`,
      );
    } else if (installProcessEnvironment) {
      console.log(
        `v86 bootstrap FS ingress passed: native exit ${nativeOracle.exitCode}, ` +
          `TEB 0x${execution.tebAddress.toString(16)}, PEB ` +
          `0x${execution.pebAddress.toString(16)}, entry result ${execution.entryResult}`,
      );
    } else {
      console.log(
        `v86 bootstrap FS first stop passed: native exit ${nativeOracle.exitCode}, ` +
          `#PF 0x${execution.faultAddress.toString(16)}, PFEC ${execution.errorCode}, ` +
          `FS base 0x${execution.fsBase.toString(16)}`,
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

function validateOffer(offer) {
  assert.equal(offer.schema, "vine.v86-memory-reflection");
  assert.equal(offer.schemaVersion, 1);
  assert.deepEqual(offer.source, {
    bytes: activeFixture.bytes,
    sha256: activeFixture.sha256,
  });
  assert.equal(offer.imageLoadBase, activeFixture.loadBase);
  assert.equal(offer.entryPointAddress, activeFixture.entry);
  assert.equal(offer.imageOwner, activeFixture.imageOwner);
  assert.equal(offer.firstThunk, undefined);
  assert.deepEqual(offer.thunks, []);

  const callback = offer.callbackReturnSentinel;
  const processEntry = offer.processEntryReturnSentinel;
  assert(callback && processEntry, "bootstrap fixture requires both control sentinels");
  assert.equal(callback.id, expected.control.callbackId);
  assert.equal(callback.address, expected.control.callbackAddress);
  assert.equal(processEntry.id, expected.control.processEntryId);
  assert.equal(processEntry.address, expected.control.processEntryAddress);
  for (const sentinel of [callback, processEntry]) {
    assert.equal(sentinel.owner, expected.control.owner);
    assert.equal(sentinel.arenaRange.start, expected.control.arenaStart);
    assert.equal(sentinel.arenaRange.byteLength, expected.control.arenaBytes);
    assert.equal(sentinel.committedPages, expected.control.committedPages);
    assert.equal(sentinel.slotSize, expected.control.slotSize);
    assert.equal(sentinel.contentSha256, expected.control.contentSha256);
    assert.equal(sentinel.ownerSealed, true);
    assert.deepEqual(sentinel.invalidation, {
      generation: expected.control.generation,
      dataTlb: true,
      instructionFetch: true,
      generatedCode: true,
    });
  }

  const stack = offer.initialThreadStack;
  assert(stack, "bootstrap fixture requires a production-owned stack");
  assert.equal(stack.owner, expected.stack.owner);
  assert.equal(stack.stackRange.start, expected.stack.base);
  assert.equal(stack.stackRange.byteLength, expected.stack.bytes);
  assert.equal(stack.committedPages, expected.stack.committedPages);
  assert.equal(stack.stackTop, expected.stack.top);
  assert.equal(stack.initialEsp, expected.stack.initialEsp);
  assert.equal(stack.entryReturnSentinelId, expected.control.processEntryId);
  assert.equal(stack.entryReturnAddress, expected.control.processEntryAddress);
  assert.equal(stack.contentSha256, expected.stack.contentSha256);
  assert.equal(stack.ownerSealed, true);
  assert.deepEqual(stack.invalidation, {
    generation: expected.stack.generation,
    dataTlb: true,
    instructionFetch: true,
    generatedCode: true,
  });

  const environment = offer.initialProcessEnvironment;
  if (installProcessEnvironment) {
    assert(environment, "process-environment mode requires TEB/PEB metadata");
    assert.equal(environment.tebOwner, expected.processEnvironment.tebOwner);
    assert.equal(environment.pebOwner, expected.processEnvironment.pebOwner);
    assert.equal(environment.tebRegionKind, "thread-environment-block");
    assert.equal(environment.pebRegionKind, "process-environment-block");
    assert.equal(environment.tebRange.start, expected.processEnvironment.tebBase);
    assert.equal(environment.tebRange.byteLength, expected.processEnvironment.pageBytes);
    assert.equal(environment.pebRange.start, expected.processEnvironment.pebBase);
    assert.equal(environment.pebRange.byteLength, expected.processEnvironment.pageBytes);
    assert.equal(
      environment.committedPages,
      expected.processEnvironment.committedPages,
    );
    assert.equal(environment.tebAddress, expected.processEnvironment.tebBase);
    assert.equal(environment.pebAddress, expected.processEnvironment.pebBase);
    assert.equal(
      environment.tebSelfPointerOffset,
      expected.processEnvironment.tebSelfPointerOffset,
    );
    assert.equal(
      environment.tebPebPointerOffset,
      expected.processEnvironment.tebPebPointerOffset,
    );
    assert.equal(
      environment.tebContentSha256,
      expected.processEnvironment.tebContentSha256,
    );
    const expectedPebContentSha256 = installProcessParameters
      ? expected.processParameters.pebContentSha256
      : expected.processEnvironment.pebContentSha256;
    if (expectedPebContentSha256 !== null) {
      assert.equal(environment.pebContentSha256, expectedPebContentSha256);
    }
    const parameters = environment.processParameters;
    if (installProcessParameters) {
      assert(parameters, "process-parameters mode requires owned metadata");
      assert.equal(
        parameters.processParametersOwner,
        expected.processParameters.processParametersOwner,
      );
      assert.equal(
        parameters.commandLineOwner,
        expected.processParameters.commandLineOwner,
      );
      assert.equal(parameters.processParametersRegionKind, "process-parameters");
      assert.equal(parameters.commandLineRegionKind, "command-line");
      assert.equal(
        parameters.processParametersRange.start,
        expected.processParameters.processParametersBase,
      );
      assert.equal(
        parameters.processParametersRange.byteLength,
        expected.processParameters.pageBytes,
      );
      assert.equal(
        parameters.commandLineRange.start,
        expected.processParameters.commandLineBase,
      );
      assert.equal(
        parameters.commandLineRange.byteLength,
        expected.processParameters.pageBytes,
      );
      assert.equal(parameters.committedPages, expected.processParameters.committedPages);
      assert.equal(
        parameters.processParametersAddress,
        expected.processParameters.processParametersBase,
      );
      assert.equal(
        parameters.commandLineBufferAddress,
        expected.processParameters.commandLineBase,
      );
      assert.equal(
        parameters.pebProcessParametersPointerOffset,
        expected.processParameters.pebProcessParametersPointerOffset,
      );
      assert.equal(
        parameters.processParametersCommandLineOffset,
        expected.processParameters.processParametersCommandLineOffset,
      );
      assert.equal(
        parameters.unicodeStringBufferOffset,
        expected.processParameters.unicodeStringBufferOffset,
      );
      assert.equal(parameters.commandLine, expected.processParameters.commandLine);
      assert.equal(
        parameters.commandLineUtf16Units,
        expected.processParameters.commandLineUtf16Units,
      );
      assert.equal(
        parameters.commandLineLengthBytes,
        expected.processParameters.commandLineLengthBytes,
      );
      assert.equal(
        parameters.commandLineMaximumLengthBytes,
        expected.processParameters.commandLineMaximumLengthBytes,
      );
      if (expected.processParameters.processParametersContentSha256 !== null) {
        assert.equal(
          parameters.processParametersContentSha256,
          expected.processParameters.processParametersContentSha256,
        );
      }
      if (expected.processParameters.commandLineContentSha256 !== null) {
        assert.equal(
          parameters.commandLineContentSha256,
          expected.processParameters.commandLineContentSha256,
        );
      }
      assert.equal(parameters.ownersSealed, true);
      assert.deepEqual(parameters.invalidation, environment.invalidation);
    } else {
      assert.equal(parameters, undefined);
    }
    assert.equal(environment.ownersSealed, true);
    assert.deepEqual(environment.invalidation, {
      generation: installProcessParameters
        ? expected.processParameters.generation
        : expected.processEnvironment.generation,
      dataTlb: true,
      instructionFetch: true,
      generatedCode: true,
    });
  } else {
    assert.equal(environment, undefined);
  }

  assert.deepEqual(
    offer.reflection.invalidation,
    installProcessEnvironment ? environment.invalidation : stack.invalidation,
  );
  assert.equal(offer.reflection.pageSize, page.size);
  assert.deepEqual(
    offer.reflection.pages.map(({ address, owner, protection }) => [
      address,
      owner,
      protection,
    ]),
    expectedReflectedPages(),
  );
  for (const reflected of offer.reflection.pages) {
    assert.equal(reflected.ownerSealed, true);
    assert.equal(reflected.dirty, true);
    assert.equal(reflected.bytes.length, page.size);
  }
  assert.equal(
    reflectionContentSha256(offer.reflection.pages),
    offer.reflection.contentSha256,
  );
  const expectedReflectionSha256 = useProcessParametersFixture
    ? installProcessParameters
      ? expected.processParametersReflectionContentSha256
      : expected.processParametersFirstStopReflectionContentSha256
    : installProcessEnvironment
      ? expected.processEnvironmentReflectionContentSha256
      : expected.reflectionContentSha256;
  if (expectedReflectionSha256 !== null) {
    assert.equal(offer.reflection.contentSha256, expectedReflectionSha256);
  }
}

async function prepareV86(offer) {
  const wasmBytes = fs.readFileSync(wasmPath);
  const adapterProfile = resolveV86AdapterProfile(wasmBytes);
  const module = new WebAssembly.Module(wasmBytes);
  const imports = WebAssembly.Module.imports(module);
  const exportsDescription = WebAssembly.Module.exports(module);
  assert.equal(imports.length, adapterProfile.wasm.imports);
  assert.equal(exportsDescription.length, adapterProfile.wasm.exports);
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
      assert.equal(imported.name, "microtick", `unexpected host import ${imported.name}`);
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
  const readVirtualBytes = (address, size, label) => {
    checkedGuestEnd(address, size, label);
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
  const readVirtualU16 = (address, label) => {
    const bytes = readVirtualBytes(address, 2, label);
    return bytes[0] | (bytes[1] << 8);
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
  words.fill(0, stateWord.gprBase, stateWord.gprBase + 8);
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
  words[stateWord.segmentOffsetBase + segmentIndex.fs] = installProcessEnvironment
    ? offer.initialProcessEnvironment.tebAddress
    : 0;
  words[stateWord.segmentMetaBase + segmentIndex.cs] = 0xfa;
  words[stateWord.controlRegisterBase] |= 0x80010001;
  words[stateWord.controlRegisterBase + 3] = pageDirectory;
  words[stateWord.controlRegisterBase + 4] |= 0x200;
  assert.equal(wasm.vine_cpu_state_restore(), 0);
  assert.equal(
    allocations.length,
    installProcessParameters ? 16 : installProcessEnvironment ? 14 : 11,
  );
  assert.deepEqual(
    [...pageTables.keys()],
    installProcessEnvironment ? [1, 64, 508, 511] : [1, 64, 508],
  );

  return {
    adapterProfile,
    allocations,
    hostImportCalls,
    mappings,
    pageDirectory,
    pageTables,
    readVirtualBytes,
    readVirtualU16,
    readVirtualU32,
    wasm,
  };
}

function validateReceipt(receipt, offer, context) {
  assert.equal(receipt.schema, "vine.v86-memory-reflection-receipt");
  assert.equal(receipt.schemaVersion, 1);
  const generation = installProcessParameters
    ? expected.processParameters.generation
    : installProcessEnvironment
      ? expected.processEnvironment.generation
    : expected.stack.generation;
  assert.equal(receipt.acknowledgedGeneration, generation);
  assert.equal(receipt.stats.owners, installProcessParameters ? 7 : installProcessEnvironment ? 5 : 3);
  assert.equal(
    receipt.stats.sealedOwners,
    installProcessParameters ? 7 : installProcessEnvironment ? 5 : 3,
  );
  assert.equal(receipt.stats.regions, installProcessParameters ? 7 : installProcessEnvironment ? 5 : 3);
  assert.equal(
    receipt.stats.reservedPages,
    installProcessParameters ? 26 : installProcessEnvironment ? 24 : 22,
  );
  assert.equal(
    receipt.stats.committedPages,
    installProcessParameters ? 11 : installProcessEnvironment ? 9 : 7,
  );
  assert.equal(
    receipt.stats.physicalPages,
    installProcessParameters ? 11 : installProcessEnvironment ? 9 : 7,
  );
  assert.equal(
    receipt.stats.dirtyPages,
    0,
  );
  assert.equal(receipt.stats.generation, generation);
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  assert.equal(receipt.iatValue, undefined);
  assert.equal(receipt.thunkBytes, undefined);
  assert.deepEqual(
    receipt.entryBytes,
    [...context.readVirtualBytes(activeFixture.entry, 16, "entry bytes")],
  );
  assert.deepEqual(
    receipt.entryBytes,
    useProcessParametersFixture
      ? [
          0x55, 0x53, 0x57, 0x56, 0x50, 0x64, 0xa1, 0x30,
          0x00, 0x00, 0x00, 0xa3, 0x00, 0x30, 0x00, 0x10,
        ]
      : [
          0x64, 0x8b, 0x15, 0x18, 0x00, 0x00, 0x00, 0x64,
          0x8b, 0x0d, 0x30, 0x00, 0x00, 0x00, 0x89, 0x15,
        ],
  );
  assert.deepEqual(receipt.callbackReturnBytes, [
    0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
    0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
  ]);
  assert.deepEqual(receipt.processEntryReturnBytes, receipt.callbackReturnBytes);
  assert.deepEqual(receipt.initialStackTopBytes, [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10, 0x00, 0x01, 0x7f,
  ]);
  if (installProcessEnvironment) {
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
    assert.equal(
      context.readVirtualU32(
        expected.processEnvironment.tebBase +
          expected.processEnvironment.tebSelfPointerOffset,
        "TEB self field",
      ),
      expected.processEnvironment.tebBase,
    );
    assert.equal(
      context.readVirtualU32(
        expected.processEnvironment.tebBase +
          expected.processEnvironment.tebPebPointerOffset,
        "TEB PEB field",
      ),
      expected.processEnvironment.pebBase,
    );
    const expectedPebBytes = new Uint8Array(64);
    if (installProcessParameters) {
      new DataView(expectedPebBytes.buffer).setUint32(
        expected.processParameters.pebProcessParametersPointerOffset,
        expected.processParameters.processParametersBase,
        true,
      );
    }
    assert.deepEqual(receipt.pebBytes, [...expectedPebBytes]);
  } else {
    assert.equal(receipt.tebBytes, undefined);
    assert.equal(receipt.pebBytes, undefined);
  }
  if (installProcessParameters) {
    const expectedProcessParametersBytes = new Uint8Array(0x48);
    const processParametersView = new DataView(expectedProcessParametersBytes.buffer);
    processParametersView.setUint16(
      expected.processParameters.processParametersCommandLineOffset,
      expected.processParameters.commandLineLengthBytes,
      true,
    );
    processParametersView.setUint16(
      expected.processParameters.processParametersCommandLineOffset + 2,
      expected.processParameters.commandLineMaximumLengthBytes,
      true,
    );
    processParametersView.setUint32(
      expected.processParameters.processParametersCommandLineOffset +
        expected.processParameters.unicodeStringBufferOffset,
      expected.processParameters.commandLineBase,
      true,
    );
    assert.deepEqual(receipt.processParametersBytes, [
      ...expectedProcessParametersBytes,
    ]);
    const expectedCommandLineBytes = Buffer.concat([
      Buffer.from(expected.processParameters.commandLine, "utf16le"),
      Buffer.from([0, 0]),
    ]);
    assert.equal(
      expectedCommandLineBytes.byteLength,
      expected.processParameters.commandLineMaximumLengthBytes,
    );
    assert.deepEqual(receipt.commandLineBytes, [...expectedCommandLineBytes]);
  } else {
    assert.equal(receipt.processParametersBytes, undefined);
    assert.equal(receipt.commandLineBytes, undefined);
  }
  assert.equal(receipt.stop.stage, "adapter-execution");
  assert.equal(receipt.stop.code, "entry-execution-pending");
  assert.equal(offer.reflection.invalidation.generation, receipt.acknowledgedGeneration);
}

function executeFirstStop(offer, context) {
  const executablePages = new Set(
    offer.reflection.pages
      .filter(({ ownerSealed, protection }) => ownerSealed && protection.execute)
      .map(({ address }) => address),
  );
  assert(executablePages.has(activeFixture.entry & ~(page.size - 1)));
  context.wasm.vine_cpu_state_save();
  let words = new Uint32Array(
    context.wasm.memory.buffer,
    context.wasm.vine_cpu_state_ptr(),
    context.wasm.vine_cpu_state_size() / 4,
  );
  assert.equal(words[stateWord.segmentOffsetBase + segmentIndex.fs], 0);
  assert.equal(words[stateWord.instructionPointer], activeFixture.entry);
  assert.equal(words[stateWord.esp], expected.stack.initialEsp);

  const reason = context.wasm.vine_execute_budget(guest.instructionBudget);
  assert.equal(reason, stopReason.pageFault);
  context.wasm.vine_cpu_state_save();
  words = new Uint32Array(
    context.wasm.memory.buffer,
    context.wasm.vine_cpu_state_ptr(),
    context.wasm.vine_cpu_state_size() / 4,
  );
  const result = {
    reason,
    errorCode: context.wasm.vine_get_stop_error_code(),
    faultAddress: words[stateWord.controlRegisterBase + 2],
    instructionPointer: words[stateWord.instructionPointer],
    previousInstructionPointer: words[stateWord.previousInstructionPointer],
    esp: words[stateWord.esp],
    edx: words[stateWord.edx],
    fsSelector: words[stateWord.segmentSelectorBase + segmentIndex.fs],
    fsBase: words[stateWord.segmentOffsetBase + segmentIndex.fs],
    instructionBudget: guest.instructionBudget,
    executeStops: 1,
    budgetStops: 0,
  };
  assert.equal(result.errorCode, 4);
  assert.equal(result.faultAddress, guest.fsSelfOffset);
  assert.equal(result.instructionPointer, activeFixture.entry + 7);
  assert.equal(result.previousInstructionPointer, activeFixture.entry);
  assert.equal(result.esp, expected.stack.initialEsp);
  assert.equal(result.edx, 0);
  assert.equal(result.fsSelector, 0);
  assert.equal(result.fsBase, 0);
  assert.deepEqual(context.hostImportCalls, { microtick: 1 });
  return result;
}

function executeWithProcessEnvironment(offer, context) {
  const environment = offer.initialProcessEnvironment;
  const processEntry = offer.processEntryReturnSentinel;
  assert(environment && processEntry);
  const executablePages = new Set(
    offer.reflection.pages
      .filter(({ ownerSealed, protection }) => ownerSealed && protection.execute)
      .map(({ address }) => address),
  );
  const saveState = (reason) => {
    context.wasm.vine_cpu_state_save();
    const words = Uint32Array.from(
      new Uint32Array(
        context.wasm.memory.buffer,
        context.wasm.vine_cpu_state_ptr(),
        context.wasm.vine_cpu_state_size() / 4,
      ),
    );
    return {
      reason,
      errorCode: context.wasm.vine_get_stop_error_code(),
      words,
      eax: words[stateWord.eax],
      esp: words[stateWord.esp],
      instructionPointer: words[stateWord.instructionPointer],
      previousInstructionPointer: words[stateWord.previousInstructionPointer],
      fsSelector: words[stateWord.segmentSelectorBase + segmentIndex.fs],
      fsBase: words[stateWord.segmentOffsetBase + segmentIndex.fs],
    };
  };

  const initial = saveState(0);
  assert.equal(initial.instructionPointer, activeFixture.entry);
  assert.equal(initial.esp, expected.stack.initialEsp);
  assert.equal(initial.fsSelector, 0);
  assert.equal(initial.fsBase, environment.tebAddress);
  let executeStops = 0;
  let budgetStops = 0;
  let completion = null;
  for (let index = 0; index < 32; index += 1) {
    const before = saveState(0);
    assertExecutableFetchWindow(before.instructionPointer, executablePages);
    const reason = context.wasm.vine_execute_budget(guest.instructionBudget);
    executeStops += 1;
    const state = saveState(reason);
    if (reason === stopReason.budget) {
      budgetStops += 1;
      continue;
    }
    assert.equal(reason, stopReason.invalidOpcode);
    assert.equal(state.errorCode, -1);
    assert.equal(state.instructionPointer, processEntry.address);
    assert.equal(state.previousInstructionPointer, processEntry.address);
    completion = state;
    break;
  }
  assert(completion, "bootstrap fixture did not reach the process-entry sentinel");

  const fixtureTebAddress = activeFixture.loadBase + 0x3000;
  const fixturePebAddress = fixtureTebAddress + 4;
  const fixtureTebSelfAddress = fixtureTebAddress + 8;
  const fixtureTebPebAddress = fixtureTebAddress + 12;
  const fixtureObservations = {
    teb: context.readVirtualU32(fixtureTebAddress, "fixture TEB result"),
    peb: context.readVirtualU32(fixturePebAddress, "fixture PEB result"),
    tebSelf: context.readVirtualU32(
      fixtureTebSelfAddress,
      "fixture TEB self result",
    ),
    tebPeb: context.readVirtualU32(fixtureTebPebAddress, "fixture TEB PEB result"),
  };
  const tebSelfField = context.readVirtualU32(
    environment.tebAddress + environment.tebSelfPointerOffset,
    "post-execution TEB self field",
  );
  const tebPebField = context.readVirtualU32(
    environment.tebAddress + environment.tebPebPointerOffset,
    "post-execution TEB PEB field",
  );
  const result = {
    reason: completion.reason,
    errorCode: completion.errorCode,
    instructionPointer: completion.instructionPointer,
    previousInstructionPointer: completion.previousInstructionPointer,
    entryResult: completion.eax,
    finalEsp: completion.esp,
    fsSelector: completion.fsSelector,
    fsBase: completion.fsBase,
    tebAddress: environment.tebAddress,
    pebAddress: environment.pebAddress,
    tebSelfField,
    tebPebField,
    fixtureObservations,
    processEntrySentinelId: processEntry.id,
    processEntrySentinelAddress: processEntry.address,
    instructionBudget: guest.instructionBudget,
    executeStops,
    budgetStops,
  };
  assert.equal(result.entryResult, 0);
  assert.equal(result.finalEsp, expected.stack.top);
  assert.equal(result.fsSelector, 0);
  assert.equal(result.fsBase, expected.processEnvironment.tebBase);
  assert.equal(result.tebAddress, expected.processEnvironment.tebBase);
  assert.equal(result.pebAddress, expected.processEnvironment.pebBase);
  assert.equal(result.tebSelfField, result.tebAddress);
  assert.equal(result.tebPebField, result.pebAddress);
  assert.equal(result.executeStops, 9);
  assert.equal(result.budgetStops, 8);
  assert.deepEqual(result.fixtureObservations, {
    teb: result.tebAddress,
    peb: result.pebAddress,
    tebSelf: result.tebAddress,
    tebPeb: result.pebAddress,
  });
  assert.deepEqual(context.hostImportCalls, { microtick: 1 });
  return result;
}

function executeProcessParameters(offer, context) {
  const environment = offer.initialProcessEnvironment;
  const processEntry = offer.processEntryReturnSentinel;
  assert(environment && processEntry);
  const executablePages = new Set(
    offer.reflection.pages
      .filter(({ ownerSealed, protection }) => ownerSealed && protection.execute)
      .map(({ address }) => address),
  );
  const saveState = (reason) => {
    context.wasm.vine_cpu_state_save();
    const words = Uint32Array.from(
      new Uint32Array(
        context.wasm.memory.buffer,
        context.wasm.vine_cpu_state_ptr(),
        context.wasm.vine_cpu_state_size() / 4,
      ),
    );
    return {
      reason,
      errorCode: context.wasm.vine_get_stop_error_code(),
      eax: words[stateWord.eax],
      esp: words[stateWord.esp],
      instructionPointer: words[stateWord.instructionPointer],
      previousInstructionPointer: words[stateWord.previousInstructionPointer],
      fsSelector: words[stateWord.segmentSelectorBase + segmentIndex.fs],
      fsBase: words[stateWord.segmentOffsetBase + segmentIndex.fs],
    };
  };

  const initial = saveState(0);
  assert.equal(initial.instructionPointer, activeFixture.entry);
  assert.equal(initial.esp, expected.stack.initialEsp);
  assert.equal(initial.fsBase, environment.tebAddress);
  let executeStops = 0;
  let budgetStops = 0;
  let completion = null;
  const maximumExecuteStops = installProcessParameters ? 512 : 32;
  for (let index = 0; index < maximumExecuteStops; index += 1) {
    const before = saveState(0);
    assertExecutableFetchWindow(before.instructionPointer, executablePages);
    const reason = context.wasm.vine_execute_budget(guest.instructionBudget);
    executeStops += 1;
    const state = saveState(reason);
    if (reason === stopReason.budget) {
      budgetStops += 1;
      continue;
    }
    assert.equal(reason, stopReason.invalidOpcode);
    assert.equal(state.errorCode, -1);
    assert.equal(state.instructionPointer, processEntry.address);
    assert.equal(state.previousInstructionPointer, processEntry.address);
    completion = state;
    break;
  }
  assert(completion, "process-parameters fixture did not reach the entry sentinel");

  const resultBase = activeFixture.loadBase + 0x3000;
  const fixtureObservations = {
    peb: context.readVirtualU32(resultBase, "fixture PEB result"),
    processParameters: context.readVirtualU32(
      resultBase + 4,
      "fixture process parameters result",
    ),
    commandLineLength: context.readVirtualU32(
      resultBase + 8,
      "fixture command line length result",
    ),
    commandLineMaximumLength: context.readVirtualU32(
      resultBase + 12,
      "fixture command line maximum length result",
    ),
    commandLineBuffer: context.readVirtualU32(
      resultBase + 16,
      "fixture command line buffer result",
    ),
    commandLineMatchIndex: context.readVirtualU32(
      resultBase + 20,
      "fixture command line match result",
    ),
  };
  const processParametersAddress = context.readVirtualU32(
    environment.pebAddress + expected.processParameters.pebProcessParametersPointerOffset,
    "PEB process parameters field",
  );
  const result = {
    reason: completion.reason,
    errorCode: completion.errorCode,
    instructionPointer: completion.instructionPointer,
    previousInstructionPointer: completion.previousInstructionPointer,
    entryResult: completion.eax,
    finalEsp: completion.esp,
    fsSelector: completion.fsSelector,
    fsBase: completion.fsBase,
    pebAddress: environment.pebAddress,
    processParametersFieldOffset:
      expected.processParameters.pebProcessParametersPointerOffset,
    processParametersAddress,
    fixtureObservations,
    processEntrySentinelId: processEntry.id,
    processEntrySentinelAddress: processEntry.address,
    instructionBudget: guest.instructionBudget,
    executeStops,
    budgetStops,
  };
  assert.equal(result.finalEsp, expected.stack.top);
  assert.equal(result.fsSelector, 0);
  assert.equal(result.fsBase, expected.processEnvironment.tebBase);
  assert.equal(result.pebAddress, expected.processEnvironment.pebBase);
  if (installProcessParameters) {
    const parameters = environment.processParameters;
    assert(parameters);
    assert.equal(result.entryResult, 0);
    assert.equal(
      result.processParametersAddress,
      expected.processParameters.processParametersBase,
    );
    if (expected.processParameters.executeStops !== null) {
      assert.equal(result.executeStops, expected.processParameters.executeStops);
      assert.equal(result.budgetStops, expected.processParameters.budgetStops);
    }
    assert.deepEqual(result.fixtureObservations, {
      peb: result.pebAddress,
      processParameters: expected.processParameters.processParametersBase,
      commandLineLength: expected.processParameters.commandLineLengthBytes,
      commandLineMaximumLength:
        expected.processParameters.commandLineMaximumLengthBytes,
      commandLineBuffer: expected.processParameters.commandLineBase,
      commandLineMatchIndex: 0,
    });
    const commandLineAddress =
      expected.processParameters.processParametersBase +
      expected.processParameters.processParametersCommandLineOffset;
    assert.equal(
      context.readVirtualU16(commandLineAddress, "command-line Length"),
      expected.processParameters.commandLineLengthBytes,
    );
    assert.equal(
      context.readVirtualU16(commandLineAddress + 2, "command-line MaximumLength"),
      expected.processParameters.commandLineMaximumLengthBytes,
    );
    assert.equal(
      context.readVirtualU32(
        commandLineAddress + expected.processParameters.unicodeStringBufferOffset,
        "command-line Buffer",
      ),
      expected.processParameters.commandLineBase,
    );
  } else {
    assert.equal(result.entryResult, 2);
    assert.equal(result.processParametersAddress, 0);
    assert.equal(result.executeStops, 8);
    assert.equal(result.budgetStops, 7);
    assert.deepEqual(result.fixtureObservations, {
      peb: result.pebAddress,
      processParameters: 0,
      commandLineLength: 0,
      commandLineMaximumLength: 0,
      commandLineBuffer: 0,
      commandLineMatchIndex: 0,
    });
  }
  assert.deepEqual(context.hostImportCalls, { microtick: 1 });
  return result;
}

run().catch((error) => {
  console.error(error.stack ?? error.message);
  process.exitCode = 1;
});
