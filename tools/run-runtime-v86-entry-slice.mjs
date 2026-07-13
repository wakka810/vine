import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import readline from "node:readline";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

import { observeArtifactIdentity } from "./evidence-classification.mjs";
import { resolveV86AdapterProfile } from "./v86-adapter-profile.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const commandArguments = process.argv.slice(2);
const dispatchAddRequested = commandArguments.includes("--dispatch-add");
const dispatchCallbackRequested = commandArguments.includes("--dispatch-callback");
const dispatchOuterReturnRequested = commandArguments.includes("--dispatch-outer-return");
const dispatchEntryReturnRequested = commandArguments.includes("--dispatch-entry-return");
const dispatchStackBootstrap = commandArguments.includes("--dispatch-stack-bootstrap");
assert(
  Number(dispatchAddRequested) +
    Number(dispatchCallbackRequested) +
    Number(dispatchOuterReturnRequested) +
    Number(dispatchEntryReturnRequested) +
    Number(dispatchStackBootstrap) <=
    1,
  "choose only one dispatch mode",
);
const dispatchEntryReturn = dispatchEntryReturnRequested || dispatchStackBootstrap;
const dispatchOuterReturn = dispatchOuterReturnRequested || dispatchEntryReturn;
const dispatchCallback = dispatchCallbackRequested || dispatchOuterReturn;
const dispatchAdd = dispatchAddRequested || dispatchCallback;
const positionalArguments = commandArguments.filter(
  (value) =>
    value !== "--dispatch-add" &&
    value !== "--dispatch-callback" &&
    value !== "--dispatch-outer-return" &&
    value !== "--dispatch-entry-return" &&
    value !== "--dispatch-stack-bootstrap",
);
assert(
  positionalArguments.length <= 2,
  "usage: run-runtime-v86-entry-slice.mjs [--dispatch-add | --dispatch-callback | --dispatch-outer-return | --dispatch-entry-return | --dispatch-stack-bootstrap] [WASM] [EXE]",
);
const wasmPath = path.resolve(
  positionalArguments[0] ??
    path.join(root, ".work", "v86-adapter-source", "build", "v86-fallback.wasm"),
);
const fixturePath = path.resolve(
  positionalArguments[1] ?? path.join(root, ".work", "toolchain-spike", "abi-smoke.exe"),
);
const outputDirectory = path.join(
  root,
  ".work",
  dispatchStackBootstrap
    ? "v86-stack-bootstrap-ingress"
    : dispatchEntryReturn
    ? "v86-entry-return-ingress"
    : dispatchOuterReturn
      ? "v86-outer-return-ingress"
      : dispatchCallback
        ? "v86-callback-return-ingress"
        : dispatchAdd
          ? "v86-cdecl-ingress"
          : "v86-entry-ingress",
);
const reportPath = path.join(outputDirectory, "report.json");

const expected = {
  source: {
    bytes: 2_560,
    sha256: "60f882a22d7d44e0ebe734f284dc8676e29035207ff004ed0cdf6e97c629b9c7",
  },
  imageLoadBase: 0x10000000,
  entry: 0x10001000,
  resultAddress: 0x10003000,
  firstThunk: {
    id: 1,
    address: 0x7f000000,
    iatAddress: 0x10002034,
    returnAddress: 0x1000100a,
    arguments: [19, 23],
  },
  secondThunk: {
    id: 2,
    address: 0x7f000010,
    iatAddress: 0x10002038,
    returnAddress: 0x10001019,
    callbackTarget: 0x10001030,
    value: 42,
  },
  callbackReturnSentinel: {
    id: 1,
    address: 0x7f010000,
    owner: 3,
    arenaStart: 0x7f010000,
    arenaBytes: 0x1_0000,
    committedPages: 1,
    slotSize: 0x10,
    contentSha256: "4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059",
    generation: 9,
  },
  processEntryReturnSentinel: {
    id: 2,
    address: 0x7f010010,
    owner: 3,
    arenaStart: 0x7f010000,
    arenaBytes: 0x1_0000,
    committedPages: 1,
    slotSize: 0x10,
    contentSha256: "bf852b2d7a8fb6faddf0c43f182d3298746eeeb644c2fe35f8413b0b9a6de7dc",
    generation: 9,
  },
  initialThreadStack: {
    owner: 4,
    base: 0x006ff000,
    bytes: 0x1000,
    committedPages: 1,
    top: 0x00700000,
    initialEsp: 0x006ffffc,
    entryReturnSentinelId: 2,
    entryReturnAddress: 0x7f010010,
    contentSha256: "3e944a15f97096aba42964f3628826766b970bbaeb98d85962f679fd725de079",
    generation: 12,
  },
  reflectionContentSha256:
    "3a3510468616980e99ccf36ccba29083acd8edffb01f1bbfdfd60e3b4a1d297e",
  entryReport: {
    bytes: 4_831,
    sha256: "c254b3be79eccbfd944dbae3d00376e9db81597eaccdec3488670408611a817c",
  },
  cdeclReport: {
    bytes: 5_501,
    sha256: "199923cb0e0b03e0fdc042db095a22f3ef697d78010834280dca1eb2b8116c7f",
  },
  callbackReflectionContentSha256:
    "45d980f7ab5aa3c98942777693438a9c780f5d6a4f0e86f2f8db63211a3af406",
  callbackReport: {
    bytes: 7_028,
    sha256: "29ede77d61369e560197dedb35311e662e0d3c56a3c6ce5dadc9743c02c10771",
  },
  outerReturnReport: {
    bytes: 8_124,
    sha256: "de6ce28c1aa6f990fe211abd81c71415a0c908fb7a47ba6651ad0aa355cdd841",
  },
  entryReturnReflectionContentSha256:
    "b184b076d28abbf59401e35113aaeb30ca1f29b8cee590ff1d71897313bfb9db",
  entryReturnReport: {
    bytes: 8_674,
    sha256: "c5edbb527fad5fc0907d8784cd22df9d0c5a93bd26aa63fd413c0a6c5b75ce59",
  },
  stackBootstrapReflectionContentSha256:
    "d7471bcc744e2f769861e1a40cc5520f5c0d452f2ea8539233a8573315b6f1cd",
  stackBootstrapReport: {
    bytes: 9_430,
    sha256: "a4e0fbb559f98e32218d5c49595ed47a113f6dd18b9f3bd4411e26efcc8592c2",
  },
};

const page = {
  size: 0x1000,
  present: 1,
  writable: 2,
  user: 4,
};
const guest = {
  physicalPoolBytes: 2 * 1024 * 1024,
  stackBase: 0x006ff000,
  stackTop: 0x00700000,
  entryReturn: 0xdeadc0de,
  instructionBudget: 3,
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
const stopReason = {
  budget: 1,
  invalidOpcode: 0x106,
  pageFault: 0x10e,
};
const calleeSaved = {
  ebx: 0x11223344,
  ebp: 0x55667788,
  esi: 0x99aabbcc,
  edi: 0xddeeff00,
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
    [0x10000000, 1, { read: true, write: false, execute: false }],
    [0x10001000, 1, { read: true, write: false, execute: true }],
    [0x10002000, 1, { read: true, write: false, execute: false }],
    [0x10003000, 1, { read: true, write: true, execute: false }],
    [0x10004000, 1, { read: true, write: false, execute: false }],
    [0x7f000000, 2, { read: true, write: false, execute: true }],
  ];
  if (dispatchStackBootstrap) {
    pages.unshift([0x006ff000, 4, { read: true, write: true, execute: false }]);
  }
  if (dispatchCallback) {
    pages.push([0x7f010000, 3, { read: true, write: false, execute: true }]);
  }
  return pages;
}

function reflectionContentSha256(pages) {
  const hasher = crypto.createHash("sha256");
  for (const reflected of pages) {
    const metadata = Buffer.alloc(11);
    metadata.writeUInt32LE(reflected.address, 0);
    metadata.writeUInt32LE(reflected.owner, 4);
    const protectionBits =
      Number(reflected.protection.read) |
      (Number(reflected.protection.write) << 1) |
      (Number(reflected.protection.execute) << 2);
    metadata[8] = protectionBits;
    metadata[9] = Number(reflected.ownerSealed);
    metadata[10] = Number(reflected.dirty);
    hasher.update(metadata);
    hasher.update(Uint8Array.from(reflected.bytes));
  }
  return hasher.digest("hex");
}

async function run() {
  requireFile(wasmPath, "npm run prepare:engine");
  requireFile(fixturePath, "npm run check:toolchain");
  const fixtureBytes = fs.readFileSync(fixturePath);
  assert.equal(fixtureBytes.byteLength, expected.source.bytes);
  assert.equal(sha256(fixtureBytes), expected.source.sha256);

  const probeArguments = [
    "run",
    "--quiet",
    "--locked",
    "-p",
    "vine-pe-probe",
    "--",
    "--v86-reflection-session",
    "--expect-sha256",
    expected.source.sha256,
    "--prepare-memory-pages",
    dispatchStackBootstrap ? "8" : dispatchCallback ? "7" : "6",
    "--materialize-image-base",
    "0x10000000",
    "--bind-name",
    "vine_fixture.dll",
    "fixture_add",
    "--bind-name",
    "vine_fixture.dll",
    "fixture_call_callback",
  ];
  if (dispatchCallback) {
    probeArguments.push("--install-callback-return-sentinel");
  }
  if (dispatchEntryReturn) {
    probeArguments.push("--install-process-entry-return-sentinel");
  }
  if (dispatchStackBootstrap) {
    probeArguments.push("--install-controlled-initial-stack");
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
    assert(next.value.length <= 512 * 1024, `${label} exceeds the 512 KiB protocol limit`);
    return JSON.parse(next.value);
  };

  try {
    const offer = await nextMessage("reflection offer");
    validateOffer(offer);

    const wasmContext = await prepareV86(offer);
    const invalidationCalls = [
      "vine_cpu_state_restore/full_clear_tlb",
      "full_clear_tlb",
      "jit_clear_cache_js",
    ];
    wasmContext.wasm.full_clear_tlb();
    wasmContext.wasm.jit_clear_cache_js();

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
    validateReceipt(receipt, offer, wasmContext);
    const processExit = await exit;
    assert.equal(processExit.signal, null, `reflection probe terminated by ${processExit.signal}`);
    assert.equal(processExit.code, 0, `reflection probe failed: ${stderr.trim()}`);
    sessionCompleted = true;

    const execution = executeControlled(offer, wasmContext);
    const report = {
      schema: dispatchStackBootstrap
        ? "vine.v86-stack-bootstrap-ingress"
        : dispatchEntryReturn
          ? "vine.v86-entry-return-ingress"
        : dispatchOuterReturn
          ? "vine.v86-outer-return-ingress"
          : dispatchCallback
            ? "vine.v86-callback-return-ingress"
            : dispatchAdd
              ? "vine.v86-cdecl-service-ingress"
              : "vine.v86-entry-ingress",
      schemaVersion: 1,
      source: offer.source,
      ...(dispatchCallback
        ? {
            runtimeControl: {
              callbackReturnSentinel: offer.callbackReturnSentinel,
              ...(dispatchEntryReturn
                ? { processEntryReturnSentinel: offer.processEntryReturnSentinel }
                : {}),
            },
          }
        : {}),
      adapter: {
        wasm: wasmContext.adapterProfile.wasm,
        physicalPoolBytes: guest.physicalPoolBytes,
        pageDirectory: wasmContext.pageDirectory,
        pageTables: [...wasmContext.pageTables.entries()].map(
          ([directoryIndex, physicalAddress]) => ({ directoryIndex, physicalAddress }),
        ),
        allocatedPages: wasmContext.allocations,
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
      bootstrap: dispatchStackBootstrap
        ? {
            kind: "rust-owned-one-page-flat-cpl3",
            initialThreadStack: offer.initialThreadStack,
            entryPoint: offer.entryPointAddress,
          }
        : {
            kind: "harness-only-one-page-flat-cpl3",
            stackBase: guest.stackBase,
            stackTop: guest.stackTop,
            initialEsp: guest.stackTop - 4,
            entryReturn: dispatchEntryReturn
              ? offer.processEntryReturnSentinel.address
              : guest.entryReturn,
            entryPoint: offer.entryPointAddress,
          },
      execution,
      stop: dispatchStackBootstrap
        ? {
            stage: "process-bootstrap",
            code: "peb-teb-process-parameters-unimplemented",
            message:
              "the Rust-owned initial stack and exact entry completion are verified; PEB, TEB, process parameters, and general stack lifecycle were not attempted",
          }
        : dispatchEntryReturn
          ? {
            stage: "process-lifecycle",
            code: "entry-completion-lifecycle-unimplemented",
            message:
              "the guest entry returned result 0 to the exact production-owned sentinel with a balanced stack; process bootstrap ownership and completion lifecycle were not attempted",
          }
        : dispatchOuterReturn
          ? {
              stage: "process-entry-return",
              code: "production-entry-return-sentinel-unimplemented",
              message:
                "outer cdecl thunk ID 2 returned 43 and the guest entry completed; its RET reached the harness-only unmapped return address before a production-owned entry sentinel was installed",
            }
          : dispatchCallback
            ? {
                stage: "callback-return",
                code: "outer-cdecl-return-unimplemented",
                message:
                  "guest callback returned 43 to the exact production-owned sentinel; returning through outer thunk ID 2 was not attempted",
              }
            : dispatchAdd
              ? {
                stage: "callback-dispatch",
                code: "guest-callback-frame-unimplemented",
                message:
                  "fixed cdecl thunk ID 1 returned to guest execution and reached thunk ID 2; callback frame construction and invocation were not attempted",
              }
              : {
                stage: "thunk-dispatch",
                code: "owned-import-service-unimplemented",
                message:
                  "the controlled entry reached exact runtime-owned thunk ID 1; host service dispatch and guest resume were not attempted",
              },
    };
    fs.mkdirSync(outputDirectory, { recursive: true });
    const reportBytes = Buffer.from(`${JSON.stringify(report, null, 2)}\n`);
    const expectedReport = dispatchStackBootstrap
      ? expected.stackBootstrapReport
      : dispatchEntryReturn
        ? expected.entryReturnReport
        : dispatchOuterReturn
          ? expected.outerReturnReport
          : dispatchCallback
            ? expected.callbackReport
            : dispatchAdd
              ? expected.cdeclReport
              : expected.entryReport;
    const reportArtifact = observeArtifactIdentity(reportBytes, expectedReport);
    fs.writeFileSync(reportPath, reportBytes);
    if (dispatchStackBootstrap) {
      console.log(
        `v86 stack bootstrap ingress passed: owner ` +
          `${offer.initialThreadStack.owner}, initial ESP ` +
          `0x${offer.initialThreadStack.initialEsp.toString(16)}, entry result=` +
          `${execution.entryCompletion.entryResult}, ${execution.budgetStops} budget stops`,
      );
    } else if (dispatchEntryReturn) {
      console.log(
        `v86 entry return ingress passed: sentinel ID ` +
          `${execution.entryCompletion.sentinelId}, entry result=` +
          `${execution.entryCompletion.entryResult}, ${execution.budgetStops} budget stops, ` +
          `EIP 0x${execution.entryCompletion.instructionPointer.toString(16)}`,
      );
    } else if (dispatchOuterReturn) {
      console.log(
        `v86 outer return ingress passed: frame ID ${execution.callbackFrame.id} ` +
          `${execution.callbackFrame.state}, result=${execution.entryContinuation.result}, ` +
          `${execution.budgetStops} budget stops, #PF ` +
          `0x${execution.entryContinuation.faultAddress.toString(16)}`,
      );
    } else if (dispatchCallback) {
      console.log(
        `v86 callback return ingress passed: frame ID ${execution.callbackFrame.id}, ` +
          `result=${execution.callbackReturn.result}, ${execution.budgetStops} budget stops, ` +
          `sentinel 0x${execution.callbackReturn.instructionPointer.toString(16)}`,
      );
    } else if (dispatchAdd) {
      console.log(
        `v86 cdecl ingress passed: result=${execution.services[0].result}, ` +
          `${execution.budgetStops} budget stops, callback thunk ID ` +
          `${execution.callbackIngress.thunkId} at ` +
          `0x${execution.callbackIngress.instructionPointer.toString(16)}`,
      );
    } else {
      console.log(
        `v86 entry ingress passed: generation ${receipt.acknowledgedGeneration}, ` +
          `${execution.budgetStops} budget stops, thunk ID ${execution.thunkId} at ` +
          `0x${execution.instructionPointer.toString(16)}`,
      );
    }
    console.log(
      `report: ${path.relative(root, reportPath)}, ${reportBytes.byteLength} bytes, ` +
        `sha256 ${sha256(reportBytes)}`,
    );
    console.log(
      `report artifact ${reportArtifact.matches ? "matched" : "changed"}; ` +
        `classification=${reportArtifact.classification}, semantic gate=independent`,
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
  assert.equal(offer.source.bytes, expected.source.bytes);
  assert.equal(offer.source.sha256, expected.source.sha256);
  assert.equal(offer.imageLoadBase, expected.imageLoadBase);
  assert.equal(offer.entryPointAddress, expected.entry);
  assert.equal(offer.imageOwner, 1);
  assert.equal(offer.firstThunk.id, expected.firstThunk.id);
  assert.equal(offer.firstThunk.address, expected.firstThunk.address);
  assert.equal(offer.firstThunk.iatAddress, expected.firstThunk.iatAddress);
  assert.equal(offer.firstThunk.module, "vine_fixture.dll");
  assert.equal(offer.firstThunk.kind, "name");
  assert.equal(offer.firstThunk.name, "fixture_add");
  assert.equal(offer.thunks.length, 2);
  assert.deepEqual(offer.thunks[0], offer.firstThunk);
  assert.equal(offer.thunks[1].id, expected.secondThunk.id);
  assert.equal(offer.thunks[1].address, expected.secondThunk.address);
  assert.equal(offer.thunks[1].iatAddress, expected.secondThunk.iatAddress);
  assert.equal(offer.thunks[1].module, "vine_fixture.dll");
  assert.equal(offer.thunks[1].kind, "name");
  assert.equal(offer.thunks[1].name, "fixture_call_callback");
  if (dispatchCallback) {
    const sentinel = offer.callbackReturnSentinel;
    assert(sentinel, "callback mode requires a production-owned return sentinel");
    assert.equal(sentinel.id, expected.callbackReturnSentinel.id);
    assert.equal(sentinel.address, expected.callbackReturnSentinel.address);
    assert.equal(sentinel.owner, expected.callbackReturnSentinel.owner);
    assert.equal(sentinel.arenaRange.start, expected.callbackReturnSentinel.arenaStart);
    assert.equal(
      sentinel.arenaRange.byteLength,
      expected.callbackReturnSentinel.arenaBytes,
    );
    assert.equal(sentinel.committedPages, expected.callbackReturnSentinel.committedPages);
    assert.equal(sentinel.slotSize, expected.callbackReturnSentinel.slotSize);
    assert.equal(
      sentinel.contentSha256,
      dispatchEntryReturn
        ? expected.processEntryReturnSentinel.contentSha256
        : expected.callbackReturnSentinel.contentSha256,
    );
    assert.equal(sentinel.ownerSealed, true);
    assert.deepEqual(sentinel.invalidation, {
      generation: expected.callbackReturnSentinel.generation,
      dataTlb: true,
      instructionFetch: true,
      generatedCode: true,
    });
  } else {
    assert.equal(offer.callbackReturnSentinel, undefined);
  }
  if (dispatchEntryReturn) {
    const sentinel = offer.processEntryReturnSentinel;
    assert(sentinel, "entry-return mode requires a production-owned return sentinel");
    assert.equal(sentinel.id, expected.processEntryReturnSentinel.id);
    assert.equal(sentinel.address, expected.processEntryReturnSentinel.address);
    assert.equal(sentinel.owner, expected.processEntryReturnSentinel.owner);
    assert.equal(sentinel.arenaRange.start, expected.processEntryReturnSentinel.arenaStart);
    assert.equal(
      sentinel.arenaRange.byteLength,
      expected.processEntryReturnSentinel.arenaBytes,
    );
    assert.equal(sentinel.committedPages, expected.processEntryReturnSentinel.committedPages);
    assert.equal(sentinel.slotSize, expected.processEntryReturnSentinel.slotSize);
    assert.equal(sentinel.contentSha256, expected.processEntryReturnSentinel.contentSha256);
    assert.equal(sentinel.ownerSealed, true);
    assert.deepEqual(sentinel.invalidation, {
      generation: expected.processEntryReturnSentinel.generation,
      dataTlb: true,
      instructionFetch: true,
      generatedCode: true,
    });
    assert.equal(sentinel.owner, offer.callbackReturnSentinel.owner);
    assert.notEqual(sentinel.address, offer.callbackReturnSentinel.address);
  } else {
    assert.equal(offer.processEntryReturnSentinel, undefined);
  }
  if (dispatchStackBootstrap) {
    const stack = offer.initialThreadStack;
    assert(stack, "stack-bootstrap mode requires production stack metadata");
    assert.equal(stack.owner, expected.initialThreadStack.owner);
    assert.equal(stack.stackRange.start, expected.initialThreadStack.base);
    assert.equal(stack.stackRange.byteLength, expected.initialThreadStack.bytes);
    assert.equal(stack.committedPages, expected.initialThreadStack.committedPages);
    assert.equal(stack.stackTop, expected.initialThreadStack.top);
    assert.equal(stack.initialEsp, expected.initialThreadStack.initialEsp);
    assert.equal(
      stack.entryReturnSentinelId,
      expected.initialThreadStack.entryReturnSentinelId,
    );
    assert.equal(stack.entryReturnAddress, expected.initialThreadStack.entryReturnAddress);
    assert.equal(stack.contentSha256, expected.initialThreadStack.contentSha256);
    assert.equal(stack.ownerSealed, true);
    assert.deepEqual(stack.invalidation, {
      generation: expected.initialThreadStack.generation,
      dataTlb: true,
      instructionFetch: true,
      generatedCode: true,
    });
  } else {
    assert.equal(offer.initialThreadStack, undefined);
  }
  assert.deepEqual(offer.reflection.invalidation, {
    generation: dispatchStackBootstrap
      ? expected.initialThreadStack.generation
      : dispatchCallback
        ? expected.callbackReturnSentinel.generation
        : 6,
    dataTlb: true,
    instructionFetch: true,
    generatedCode: true,
  });
  assert.equal(offer.reflection.pageSize, page.size);
  assert.equal(offer.reflection.pages.length, expectedReflectedPages().length);
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
    for (const byte of reflected.bytes) {
      assert(Number.isInteger(byte) && byte >= 0 && byte <= 0xff);
    }
  }
  assert.equal(
    reflectionContentSha256(offer.reflection.pages),
    offer.reflection.contentSha256,
  );
  assert.equal(
    offer.reflection.contentSha256,
    dispatchStackBootstrap
      ? expected.stackBootstrapReflectionContentSha256
      : dispatchEntryReturn
        ? expected.entryReturnReflectionContentSha256
      : dispatchCallback
      ? expected.callbackReflectionContentSha256
      : expected.reflectionContentSha256,
  );
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
  privateState.setUint32(
    adapterPrivateOffset.memorySize,
    guest.physicalPoolBytes,
    true,
  );
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
  const mapPage = (virtualBase, protection, bytes, role) => {
    checkedGuestEnd(virtualBase, page.size, role);
    assert.equal(virtualBase & (page.size - 1), 0);
    assert.equal(bytes.byteLength, page.size);
    assert(!mappings.has(virtualBase), `duplicate mapping 0x${virtualBase.toString(16)}`);
    const table = ensurePageTable(virtualBase);
    const tableIndex = (virtualBase >>> 12) & 0x3ff;
    const pteAddress = table + tableIndex * 4;
    const physicalBase = allocatePhysicalPage(role);
    writePhysicalU32(
      pteAddress,
      physicalBase |
        page.present |
        page.user |
        (protection.write ? page.writable : 0),
      `${role} page table entry`,
    );
    physicalMemory.set(bytes, physicalBase);
    mappings.set(virtualBase, {
      virtualBase,
      physicalBase,
      pteAddress,
      protection: { ...protection },
      role,
    });
  };

  for (const reflected of offer.reflection.pages) {
    mapPage(
      reflected.address,
      reflected.protection,
      Uint8Array.from(reflected.bytes),
      `reflected-owner-${reflected.owner}`,
    );
  }
  if (!dispatchStackBootstrap) {
    mapPage(
      guest.stackBase,
      { read: true, write: true, execute: false },
      new Uint8Array(page.size),
      "harness-stack",
    );
  }

  const mappingFor = (address, label) => {
    checkedGuestEnd(address, 1, label);
    const virtualBase = (address & ~(page.size - 1)) >>> 0;
    const mapping = mappings.get(virtualBase);
    assert(mapping, `${label} uses unmapped guest address 0x${address.toString(16)}`);
    return mapping;
  };
  const writeVirtualU32 = (address, value, label) => {
    checkedGuestEnd(address, 4, label);
    for (let index = 0; index < 4; index += 1) {
      const current = (address + index) >>> 0;
      const mapping = mappingFor(current, label);
      assert(mapping.protection.write, `${label} writes a read-only guest page`);
      physicalMemory[mapping.physicalBase + (current & (page.size - 1))] =
        value >>> (index * 8);
    }
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

  const initialEsp = dispatchStackBootstrap
    ? offer.initialThreadStack.initialEsp
    : guest.stackTop - 4;
  const entryReturnAddress = dispatchEntryReturn
    ? offer.processEntryReturnSentinel.address
    : guest.entryReturn;
  if (dispatchStackBootstrap) {
    assert.equal(initialEsp, expected.initialThreadStack.initialEsp);
    assert.equal(
      readVirtualU32(initialEsp, "production initial entry return sentinel"),
      entryReturnAddress,
    );
    const stackMapping = mappingFor(initialEsp, "production initial stack mapping");
    assert.equal(stackMapping.role, `reflected-owner-${expected.initialThreadStack.owner}`);
    assert.deepEqual(stackMapping.protection, {
      read: true,
      write: true,
      execute: false,
    });
  } else {
    writeVirtualU32(initialEsp, entryReturnAddress, "entry return sentinel");
  }
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
  words[stateWord.ebx] = calleeSaved.ebx;
  words[stateWord.esp] = initialEsp;
  words[stateWord.ebp] = calleeSaved.ebp;
  words[stateWord.esi] = calleeSaved.esi;
  words[stateWord.edi] = calleeSaved.edi;
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
  words[stateWord.segmentMetaBase + 1] = 0xfa;
  words[stateWord.controlRegisterBase] |= 0x80010001;
  words[stateWord.controlRegisterBase + 3] = pageDirectory;
  words[stateWord.controlRegisterBase + 4] |= 0x200;
  assert.equal(wasm.vine_cpu_state_restore(), 0);
  assert.equal(
    allocations.filter(({ role }) => role === "harness-stack").length,
    dispatchStackBootstrap ? 0 : 1,
  );
  if (dispatchStackBootstrap) {
    assert.equal(
      allocations.filter(
        ({ role }) => role === `reflected-owner-${expected.initialThreadStack.owner}`,
      ).length,
      1,
    );
  }

  return {
    adapterProfile,
    allocations,
    hostImportCalls,
    mappings,
    pageDirectory,
    pageTables,
    physicalMemory,
    readVirtualBytes,
    readVirtualU32,
    wasm,
    writeVirtualU32,
  };
}

function validateReceipt(receipt, offer, context) {
  assert.equal(receipt.schema, "vine.v86-memory-reflection-receipt");
  assert.equal(receipt.schemaVersion, 1);
  assert.equal(
    receipt.acknowledgedGeneration,
    offer.reflection.invalidation.generation,
  );
  assert.equal(
    receipt.stats.generation,
    dispatchStackBootstrap
      ? expected.initialThreadStack.generation
      : dispatchCallback
        ? expected.callbackReturnSentinel.generation
        : 6,
  );
  assert.equal(receipt.stats.pendingInvalidation, undefined);
  if (dispatchStackBootstrap) {
    assert.equal(receipt.stats.owners, 4);
    assert.equal(receipt.stats.sealedOwners, 4);
    assert.equal(receipt.stats.regions, 4);
    assert.equal(receipt.stats.reservedPages, 38);
    assert.equal(receipt.stats.committedPages, 8);
    assert.equal(receipt.stats.physicalPages, 8);
    assert.equal(receipt.stats.dirtyPages, 0);
  } else if (dispatchCallback) {
    assert.equal(receipt.stats.owners, 3);
    assert.equal(receipt.stats.sealedOwners, 3);
    assert.equal(receipt.stats.regions, 3);
    assert.equal(receipt.stats.reservedPages, 37);
    assert.equal(receipt.stats.committedPages, 7);
    assert.equal(receipt.stats.physicalPages, 7);
    assert.equal(receipt.stats.dirtyPages, 0);
  }
  assert.equal(receipt.iatValue, expected.firstThunk.address);
  assert.deepEqual(
    receipt.entryBytes,
    [...context.readVirtualBytes(expected.entry, 16, "reflected entry bytes")],
  );
  assert.deepEqual(
    receipt.thunkBytes,
    [...context.readVirtualBytes(expected.firstThunk.address, 16, "reflected thunk bytes")],
  );
  assert.deepEqual(receipt.thunkBytes, [
    0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
    0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
  ]);
  if (dispatchCallback) {
    assert.deepEqual(
      receipt.callbackReturnBytes,
      [
        ...context.readVirtualBytes(
          expected.callbackReturnSentinel.address,
          16,
          "reflected callback return bytes",
        ),
      ],
    );
    assert.deepEqual(receipt.callbackReturnBytes, [
      0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
      0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
    ]);
  } else {
    assert.equal(receipt.callbackReturnBytes, undefined);
  }
  if (dispatchEntryReturn) {
    assert.deepEqual(
      receipt.processEntryReturnBytes,
      [
        ...context.readVirtualBytes(
          expected.processEntryReturnSentinel.address,
          16,
          "reflected process entry return bytes",
        ),
      ],
    );
    assert.deepEqual(receipt.processEntryReturnBytes, [
      0x0f, 0x0b, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
      0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4, 0xf4,
    ]);
  } else {
    assert.equal(receipt.processEntryReturnBytes, undefined);
  }
  if (dispatchStackBootstrap) {
    assert.deepEqual(
      receipt.initialStackTopBytes,
      [
        ...context.readVirtualBytes(
          expected.initialThreadStack.top - 16,
          16,
          "reflected initial stack top bytes",
        ),
      ],
    );
    assert.deepEqual(receipt.initialStackTopBytes, [
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10, 0x00, 0x01, 0x7f,
    ]);
  } else {
    assert.equal(receipt.initialStackTopBytes, undefined);
  }
  assert.equal(receipt.stop.stage, "adapter-execution");
  assert.equal(receipt.stop.code, "entry-to-first-owned-thunk-pending");
}

function executeControlled(offer, context) {
  let budgetStops = 0;
  let executeStops = 0;
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
      faultAddress: words[stateWord.controlRegisterBase + 2],
      instructionPointer: words[stateWord.instructionPointer],
      previousInstructionPointer: words[stateWord.previousInstructionPointer],
    };
  };
  const ownedThunks = new Map(offer.thunks.map((thunk) => [thunk.address, thunk]));
  assert.equal(ownedThunks.size, offer.thunks.length, "owned thunk addresses must be unique");
  const callbackSentinel = offer.callbackReturnSentinel;
  if (callbackSentinel) {
    assert(!ownedThunks.has(callbackSentinel.address));
  }
  const processEntrySentinel = offer.processEntryReturnSentinel;
  if (processEntrySentinel) {
    assert(!ownedThunks.has(processEntrySentinel.address));
    assert.notEqual(processEntrySentinel.address, callbackSentinel?.address);
  }
  const classifyTrap = (address) => {
    const thunk = ownedThunks.get(address);
    if (thunk) {
      return { kind: "import-thunk", thunk };
    }
    if (callbackSentinel?.address === address) {
      return { kind: "callback-return", sentinel: callbackSentinel };
    }
    if (processEntrySentinel?.address === address) {
      return { kind: "process-entry-return", sentinel: processEntrySentinel };
    }
    return null;
  };
  const runUntilExactTrap = (expectedAddress, label) => {
    for (let index = 0; index < 16; index += 1) {
      const before = saveState(0);
      assertExecutableFetchWindow(before.instructionPointer, executablePages);
      const reason = context.wasm.vine_execute_budget(guest.instructionBudget);
      executeStops += 1;
      const state = saveState(reason);
      assert.equal(state.words[stateWord.ebx], calleeSaved.ebx);
      assert.equal(state.words[stateWord.ebp], calleeSaved.ebp);
      assert.equal(state.words[stateWord.esi], calleeSaved.esi);
      assert.equal(state.words[stateWord.edi], calleeSaved.edi);
      if (reason === stopReason.budget) {
        budgetStops += 1;
        continue;
      }
      assert.equal(reason, stopReason.invalidOpcode);
      assert.equal(state.errorCode, -1);
      const trap = classifyTrap(state.instructionPointer);
      assert(trap, `#UD at unowned address 0x${state.instructionPointer.toString(16)}`);
      assert.equal(state.instructionPointer, expectedAddress, `${label} reached the wrong trap`);
      assert.equal(state.previousInstructionPointer, expectedAddress);
      return { state, trap };
    }
    assert.fail(`${label} did not reach its exact owned trap within 16 slices`);
  };
  const runUntilOwnedThunk = (expectedThunk, label) => {
    const { state, trap } = runUntilExactTrap(expectedThunk.address, label);
    assert.equal(trap.kind, "import-thunk", `${label} reached a non-import trap`);
    assert.equal(trap.thunk.id, expectedThunk.id, `${label} reached the wrong thunk`);
    return state;
  };
  const runUntilCallbackReturn = () => {
    assert(callbackSentinel, "callback return execution requires sentinel metadata");
    const { state, trap } = runUntilExactTrap(
      callbackSentinel.address,
      "guest callback return",
    );
    assert.equal(trap.kind, "callback-return");
    assert.equal(trap.sentinel.id, callbackSentinel.id);
    return state;
  };
  const runUntilProcessEntryReturn = () => {
    assert(processEntrySentinel, "entry return execution requires sentinel metadata");
    const { state, trap } = runUntilExactTrap(
      processEntrySentinel.address,
      "process entry return",
    );
    assert.equal(trap.kind, "process-entry-return");
    assert.equal(trap.sentinel.id, processEntrySentinel.id);
    return state;
  };
  const runUntilPageFault = (expectedAddress, label) => {
    for (let index = 0; index < 16; index += 1) {
      const before = saveState(0);
      assertExecutableFetchWindow(before.instructionPointer, executablePages);
      const reason = context.wasm.vine_execute_budget(guest.instructionBudget);
      executeStops += 1;
      const state = saveState(reason);
      assert.equal(state.words[stateWord.ebx], calleeSaved.ebx);
      assert.equal(state.words[stateWord.ebp], calleeSaved.ebp);
      assert.equal(state.words[stateWord.esi], calleeSaved.esi);
      assert.equal(state.words[stateWord.edi], calleeSaved.edi);
      if (reason === stopReason.budget) {
        budgetStops += 1;
        continue;
      }
      assert.equal(reason, stopReason.pageFault);
      assert.equal(state.errorCode, 4);
      assert.equal(state.faultAddress, expectedAddress, `${label} faulted at the wrong address`);
      return state;
    }
    assert.fail(`${label} did not reach its page-fault boundary within 16 slices`);
  };
  const resume = (state, { eax, esp, instructionPointer }) => {
    context.wasm.vine_cpu_state_save();
    const words = new Uint32Array(
      context.wasm.memory.buffer,
      context.wasm.vine_cpu_state_ptr(),
      context.wasm.vine_cpu_state_size() / 4,
    );
    assert.equal(words[stateWord.instructionPointer], state.instructionPointer);
    assert.equal(words[stateWord.esp], state.esp);
    words[stateWord.eax] = eax >>> 0;
    words[stateWord.esp] = esp >>> 0;
    words[stateWord.instructionPointer] = instructionPointer >>> 0;
    words[stateWord.previousInstructionPointer] = instructionPointer >>> 0;
    assert.equal(context.wasm.vine_cpu_state_restore(), 0);
  };

  const firstState = runUntilOwnedThunk(offer.thunks[0], "fixture_add");
  assert.equal(firstState.esp, guest.stackTop - 16);
  const returnAddress = context.readVirtualU32(firstState.esp, "fixture_add return address");
  const left = context.readVirtualU32(firstState.esp + 4, "fixture_add left argument");
  const right = context.readVirtualU32(firstState.esp + 8, "fixture_add right argument");
  assert.equal(returnAddress, expected.firstThunk.returnAddress);
  assert.deepEqual([left, right], expected.firstThunk.arguments);
  assert.equal(
    context.readVirtualU32(expected.firstThunk.iatAddress, "bound fixture_add IAT"),
    expected.firstThunk.address,
  );
  const entryResult = {
    instructionBudget: guest.instructionBudget,
    executeStops,
    budgetStops,
    stopReason: firstState.reason,
    stopErrorCode: firstState.errorCode,
    thunkId: offer.firstThunk.id,
    instructionPointer: firstState.instructionPointer,
    previousInstructionPointer: firstState.previousInstructionPointer,
    esp: firstState.esp,
    returnAddress,
    arguments: [left, right],
    service: "vine_fixture.dll!fixture_add",
  };
  if (!dispatchAdd) {
    assert.deepEqual(context.hostImportCalls, { microtick: 1 });
    return entryResult;
  }

  const result = (left + right) >>> 0;
  const resumedEsp = firstState.esp + 4;
  resume(firstState, {
    eax: result,
    esp: resumedEsp,
    instructionPointer: returnAddress,
  });
  const secondState = runUntilOwnedThunk(offer.thunks[1], "fixture_call_callback");
  assert.equal(secondState.eax, result);
  assert.equal(secondState.esp, guest.stackTop - 16);
  const callbackReturn = context.readVirtualU32(
    secondState.esp,
    "fixture_call_callback return address",
  );
  const callbackTarget = context.readVirtualU32(
    secondState.esp + 4,
    "fixture_call_callback target",
  );
  const callbackValue = context.readVirtualU32(
    secondState.esp + 8,
    "fixture_call_callback value",
  );
  assert.equal(callbackReturn, expected.secondThunk.returnAddress);
  assert.equal(callbackTarget, expected.secondThunk.callbackTarget);
  assert.equal(callbackValue, expected.secondThunk.value);
  assertExecutableFetchWindow(callbackTarget, executablePages);
  assert.equal(
    context.readVirtualU32(expected.secondThunk.iatAddress, "bound callback IAT"),
    expected.secondThunk.address,
  );
  assert.deepEqual(context.hostImportCalls, { microtick: 1 });

  const cdeclResult = {
    instructionBudget: guest.instructionBudget,
    executeStops,
    budgetStops,
    services: [
      {
        thunkId: offer.thunks[0].id,
        thunkAddress: offer.thunks[0].address,
        service: "vine_fixture.dll!fixture_add",
        esp: firstState.esp,
        returnAddress,
        arguments: [left, right],
        result,
        resume: {
          eax: result,
          esp: resumedEsp,
          instructionPointer: returnAddress,
          previousInstructionPointer: returnAddress,
        },
      },
    ],
    callbackIngress: {
      stopReason: secondState.reason,
      stopErrorCode: secondState.errorCode,
      thunkId: offer.thunks[1].id,
      thunkAddress: offer.thunks[1].address,
      service: "vine_fixture.dll!fixture_call_callback",
      instructionPointer: secondState.instructionPointer,
      previousInstructionPointer: secondState.previousInstructionPointer,
      eax: secondState.eax,
      esp: secondState.esp,
      returnAddress: callbackReturn,
      callbackTarget,
      callbackValue,
    },
    hostImportCalls: { ...context.hostImportCalls },
  };
  if (!dispatchCallback) {
    assert.equal(executeStops, 4);
    assert.equal(budgetStops, 2);
    return cdeclResult;
  }

  let nextCallbackFrameId = 1;
  const callbackStackPointer = secondState.esp - 8;
  checkedEnd(
    callbackStackPointer,
    8,
    secondState.esp,
    "guest callback frame stack space",
  );
  assert(callbackStackPointer >= guest.stackBase);
  const callbackFrame = {
    id: nextCallbackFrameId,
    parentThunkId: offer.thunks[1].id,
    outerEsp: secondState.esp,
    outerReturnAddress: callbackReturn,
    target: callbackTarget,
    value: callbackValue,
    returnSentinelId: callbackSentinel.id,
    returnSentinelAddress: callbackSentinel.address,
    stackPointer: callbackStackPointer,
    state: "callback-return-trapped",
  };
  nextCallbackFrameId += 1;
  assert.equal(nextCallbackFrameId, 2);
  context.writeVirtualU32(
    callbackStackPointer,
    callbackSentinel.address,
    "callback return sentinel",
  );
  context.writeVirtualU32(
    callbackStackPointer + 4,
    callbackValue,
    "callback value argument",
  );
  resume(secondState, {
    eax: secondState.eax,
    esp: callbackStackPointer,
    instructionPointer: callbackTarget,
  });
  const callbackReturnState = runUntilCallbackReturn();
  assert.equal(callbackReturnState.eax, 43);
  assert.equal(callbackReturnState.esp, secondState.esp - 4);
  assert.equal(
    context.readVirtualU32(callbackStackPointer, "consumed callback sentinel slot"),
    callbackSentinel.address,
  );
  assert.equal(
    context.readVirtualU32(callbackStackPointer + 4, "callback argument after return"),
    callbackValue,
  );
  assert.equal(executeStops, 6);
  assert.equal(budgetStops, 3);
  assert.deepEqual(context.hostImportCalls, { microtick: 1 });

  const callbackResult = {
    instructionBudget: guest.instructionBudget,
    executeStops,
    budgetStops,
    services: cdeclResult.services,
    callbackIngress: cdeclResult.callbackIngress,
    callbackFrame,
    callbackReturn: {
      stopReason: callbackReturnState.reason,
      stopErrorCode: callbackReturnState.errorCode,
      sentinelId: callbackSentinel.id,
      sentinelAddress: callbackSentinel.address,
      instructionPointer: callbackReturnState.instructionPointer,
      previousInstructionPointer: callbackReturnState.previousInstructionPointer,
      eax: callbackReturnState.eax,
      result: callbackReturnState.eax,
      esp: callbackReturnState.esp,
    },
    hostImportCalls: { ...context.hostImportCalls },
  };
  if (!dispatchOuterReturn) {
    return callbackResult;
  }

  const outerResumeEsp = callbackFrame.outerEsp + 4;
  assert.equal(outerResumeEsp, guest.stackTop - 12);
  const outerResume = {
    eax: callbackReturnState.eax,
    esp: outerResumeEsp,
    instructionPointer: callbackFrame.outerReturnAddress,
    previousInstructionPointer: callbackFrame.outerReturnAddress,
  };
  resume(callbackReturnState, outerResume);
  const entryContinuationState = dispatchEntryReturn
    ? runUntilProcessEntryReturn()
    : runUntilPageFault(guest.entryReturn, "fixture entry return");
  const expectedEntryReturnAddress = dispatchEntryReturn
    ? processEntrySentinel.address
    : guest.entryReturn;
  assert.equal(entryContinuationState.instructionPointer, expectedEntryReturnAddress);
  assert.equal(entryContinuationState.previousInstructionPointer, expectedEntryReturnAddress);
  assert.equal(entryContinuationState.eax, 0);
  assert.equal(entryContinuationState.esp, guest.stackTop);
  const fixtureResult = context.readVirtualU32(
    expected.resultAddress,
    "fixture callback result",
  );
  assert.equal(fixtureResult, callbackReturnState.eax);
  assert.equal(executeStops, 9);
  assert.equal(budgetStops, 5);
  assert.deepEqual(context.hostImportCalls, { microtick: 1 });

  if (dispatchEntryReturn) {
    return {
      instructionBudget: guest.instructionBudget,
      executeStops,
      budgetStops,
      services: [
        ...cdeclResult.services,
        {
          thunkId: offer.thunks[1].id,
          thunkAddress: offer.thunks[1].address,
          service: "vine_fixture.dll!fixture_call_callback",
          esp: secondState.esp,
          returnAddress: callbackReturn,
          arguments: [callbackTarget, callbackValue],
          callbackFrameId: callbackFrame.id,
          result: callbackReturnState.eax,
          resume: outerResume,
        },
      ],
      callbackIngress: cdeclResult.callbackIngress,
      callbackFrame: {
        ...callbackFrame,
        state: "retired",
      },
      callbackReturn: callbackResult.callbackReturn,
      outerReturn: {
        thunkId: offer.thunks[1].id,
        callbackFrameId: callbackFrame.id,
        result: callbackReturnState.eax,
        resume: outerResume,
      },
      entryCompletion: {
        stopReason: entryContinuationState.reason,
        stopErrorCode: entryContinuationState.errorCode,
        sentinelId: processEntrySentinel.id,
        sentinelAddress: processEntrySentinel.address,
        instructionPointer: entryContinuationState.instructionPointer,
        previousInstructionPointer: entryContinuationState.previousInstructionPointer,
        eax: entryContinuationState.eax,
        entryResult: entryContinuationState.eax,
        esp: entryContinuationState.esp,
        fixtureResultAddress: expected.resultAddress,
        fixtureResult,
      },
      hostImportCalls: { ...context.hostImportCalls },
    };
  }

  return {
    instructionBudget: guest.instructionBudget,
    executeStops,
    budgetStops,
    services: [
      ...cdeclResult.services,
      {
        thunkId: offer.thunks[1].id,
        thunkAddress: offer.thunks[1].address,
        service: "vine_fixture.dll!fixture_call_callback",
        esp: secondState.esp,
        returnAddress: callbackReturn,
        arguments: [callbackTarget, callbackValue],
        callbackFrameId: callbackFrame.id,
        result: callbackReturnState.eax,
        resume: outerResume,
      },
    ],
    callbackIngress: cdeclResult.callbackIngress,
    callbackFrame: {
      ...callbackFrame,
      state: "retired",
    },
    callbackReturn: callbackResult.callbackReturn,
    outerReturn: {
      thunkId: offer.thunks[1].id,
      callbackFrameId: callbackFrame.id,
      result: callbackReturnState.eax,
      resume: outerResume,
    },
    entryContinuation: {
      stopReason: entryContinuationState.reason,
      stopErrorCode: entryContinuationState.errorCode,
      faultAddress: entryContinuationState.faultAddress,
      instructionPointer: entryContinuationState.instructionPointer,
      previousInstructionPointer: entryContinuationState.previousInstructionPointer,
      eax: entryContinuationState.eax,
      esp: entryContinuationState.esp,
      resultAddress: expected.resultAddress,
      result: fixtureResult,
    },
    hostImportCalls: { ...context.hostImportCalls },
  };
}

await run();
