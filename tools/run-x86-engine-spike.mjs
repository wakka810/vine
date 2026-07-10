import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = path.join(root, ".work", "x86-engine-spike");
const v86Entry = fileURLToPath(import.meta.resolve("v86"));
const v86Root = path.resolve(path.dirname(v86Entry), "..");
const packageManifestPath = path.join(v86Root, "package.json");
const fallbackPath = path.join(v86Root, "build", "v86-fallback.wasm");
const jitPath = path.join(v86Root, "build", "v86.wasm");
const debugPath = path.join(v86Root, "build", "v86-debug.wasm");

const artifacts = {
  fallback: {
    path: fallbackPath,
    expectedSize: 2_113_455,
    expectedSha256:
      "0859dc359024ee2ac04a1eade930043065a86a1d041d9c5115669ab3eba7cbed",
  },
  jit: {
    path: jitPath,
    expectedSize: 2_084_417,
    expectedSha256:
      "aec2c16bb0a1618aa641bb44d9c0fe14681f8c1459fa08c32e3e0562020884e8",
  },
  debug: {
    path: debugPath,
    expectedSize: 11_325_416,
    expectedSha256:
      "e5407e92a85fa7b5c4b2a97729aed863f7976539f0708eec0beb551f25598eda",
  },
};

const expectedImports = [
  "env.__indirect_function_table:table",
  "env.codegen_finalize:function",
  "env.console_log_from_wasm:function",
  "env.cpu_event_halt:function",
  "env.get_rand_int:function",
  "env.io_port_read16:function",
  "env.io_port_read32:function",
  "env.io_port_read8:function",
  "env.io_port_write16:function",
  "env.io_port_write32:function",
  "env.io_port_write8:function",
  "env.jit_clear_func:function",
  "env.microtick:function",
  "env.mmap_read32:function",
  "env.mmap_read8:function",
  "env.mmap_write128:function",
  "env.mmap_write16:function",
  "env.mmap_write32:function",
  "env.mmap_write64:function",
  "env.mmap_write8:function",
  "env.run_hardware_timers:function",
  "env.stop_idling:function",
];

const requiredExports = [
  "allocate_memory",
  "full_clear_tlb",
  "main_loop",
  "memory",
  "reset_cpu",
  "rust_init",
  "update_state_flags",
];

const stateOffset = {
  registers: 64,
  flags: 120,
  instructionPointer: 556,
  previousInstructionPointer: 560,
  controlRegisters: 580,
  cpl: 612,
  inHalt: 616,
  lastVirtualInstructionPointer: 620,
  instructionCounter: 664,
  segmentAccessBytes: 512,
  segmentIsNull: 724,
  segmentOffsets: 736,
  segmentLimits: 768,
  protectedMode: 800,
  operandSize32: 804,
  stackSize32: 808,
  memorySize: 812,
};

const guest = {
  memorySize: 2 * 1024 * 1024,
  codeAddress: 0x1000,
  resultAddress: 0x2000,
  stackAddress: 0x1ff000,
};

// mov/add/store, stack/call/ret, x87, SSE, then a ring-0 hlt sentinel.
const program = Uint8Array.from([
  0xb8, 0x78, 0x56, 0x34, 0x12,
  0x05, 0x01, 0x00, 0x00, 0x00,
  0xa3, 0x00, 0x20, 0x00, 0x00,
  0xbc, 0x00, 0xf0, 0x1f, 0x00,
  0x6a, 0x20,
  0x6a, 0x16,
  0xe8, 0x33, 0x00, 0x00, 0x00,
  0x83, 0xc4, 0x08,
  0xa3, 0x04, 0x20, 0x00, 0x00,
  0xd9, 0xe8,
  0xd9, 0xe8,
  0xde, 0xc1,
  0xdb, 0x1d, 0x08, 0x20, 0x00, 0x00,
  0xc7, 0x05, 0x10, 0x20, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3f,
  0xf3, 0x0f, 0x10, 0x05, 0x10, 0x20, 0x00, 0x00,
  0xf3, 0x0f, 0x58, 0xc0,
  0xf3, 0x0f, 0x11, 0x05, 0x14, 0x20, 0x00, 0x00,
  0xf4,
  0x8b, 0x44, 0x24, 0x04,
  0x03, 0x44, 0x24, 0x08,
  0xc3,
]);

const cpl3TrapProgram = Uint8Array.from([
  0xb8, 0x2a, 0x00, 0x00, 0x00,
  0x0f, 0x0b,
]);

function inspectArtifact(artifact) {
  const bytes = fs.readFileSync(artifact.path);
  const sha256 = crypto.createHash("sha256").update(bytes).digest("hex");

  assert.equal(bytes.byteLength, artifact.expectedSize);
  assert.equal(sha256, artifact.expectedSha256);

  return { bytes, size: bytes.byteLength, sha256 };
}

function describeImports(module) {
  return WebAssembly.Module.imports(module)
    .map(({ module: namespace, name, kind }) => `${namespace}.${name}:${kind}`)
    .sort();
}

function describeExports(module) {
  return WebAssembly.Module.exports(module)
    .map(({ name, kind }) => ({ name, kind }))
    .sort((left, right) => left.name.localeCompare(right.name));
}

function countCall(calls, name) {
  calls[name] = (calls[name] ?? 0) + 1;
}

function createEnvironment(module, calls, handlers = {}) {
  let now = 0;
  const environment = {
    __indirect_function_table: new WebAssembly.Table({
      element: "anyfunc",
      initial: 1_924,
    }),
  };

  for (const imported of WebAssembly.Module.imports(module)) {
    if (imported.kind !== "function") {
      continue;
    }

    environment[imported.name] = (...args) => {
      countCall(calls, imported.name);
      if (handlers[imported.name]) {
        return handlers[imported.name](...args);
      }
      if (imported.name === "microtick") {
        now += 0.125;
        return now;
      }
      if (imported.name === "run_hardware_timers") {
        return 100;
      }
      return 0;
    };
  }

  return { env: environment };
}

function configureFlat32State(exports, state, cpl = 0) {
  for (let index = 0; index < 8; index += 1) {
    state.setUint8(stateOffset.segmentIsNull + index, 0);
    state.setUint32(stateOffset.segmentOffsets + index * 4, 0, true);
    state.setUint32(stateOffset.segmentLimits + index * 4, 0xffffffff, true);
    state.setUint8(
      stateOffset.segmentAccessBytes + index,
      0x80 | (cpl << 5) | 0x10 | 0x02,
    );
  }
  state.setUint8(
    stateOffset.segmentAccessBytes + 1,
    0x80 | (cpl << 5) | 0x10 | 0x08 | 0x02,
  );

  state.setUint8(stateOffset.protectedMode, 1);
  state.setUint8(stateOffset.operandSize32, 1);
  state.setUint8(stateOffset.stackSize32, 1);
  state.setUint8(stateOffset.cpl, cpl);
  state.setUint8(stateOffset.inHalt, 0);
  state.setUint32(stateOffset.instructionPointer, guest.codeAddress, true);
  state.setUint32(
    stateOffset.previousInstructionPointer,
    guest.codeAddress,
    true,
  );
  state.setInt32(stateOffset.lastVirtualInstructionPointer, -1, true);
  state.setUint32(stateOffset.flags, 2, true);

  const cr0 = state.getUint32(stateOffset.controlRegisters, true);
  state.setUint32(stateOffset.controlRegisters, cr0 | 1, true);
  const cr4Offset = stateOffset.controlRegisters + 4 * 4;
  const cr4 = state.getUint32(cr4Offset, true);
  state.setUint32(cr4Offset, cr4 | 0x200, true);

  exports.update_state_flags();
  exports.full_clear_tlb();
}

async function runSmoke(module, moduleBytes) {
  const calls = {};
  const { instance } = await WebAssembly.instantiate(moduleBytes, createEnvironment(module, calls));
  const exports = instance.exports;

  exports.rust_init();
  let state = new DataView(exports.memory.buffer);
  state.setUint32(stateOffset.memorySize, guest.memorySize, true);
  const guestMemoryBase = exports.allocate_memory(guest.memorySize);

  // allocate_memory can grow Wasm memory and detach earlier views.
  state = new DataView(exports.memory.buffer);
  const memory = new Uint8Array(
    exports.memory.buffer,
    guestMemoryBase,
    guest.memorySize,
  );
  exports.zero_memory(0, guest.memorySize);
  exports.reset_cpu();
  memory.set(program, guest.codeAddress);
  configureFlat32State(exports, state);

  const timerDelay = exports.main_loop();
  const results = new DataView(memory.buffer, memory.byteOffset);
  const smoke = {
    cpl: state.getUint8(stateOffset.cpl),
    timerDelay,
    instructionCount: state.getUint32(stateOffset.instructionCounter, true),
    instructionPointer: state.getUint32(
      stateOffset.instructionPointer,
      true,
    ),
    eax: state.getUint32(stateOffset.registers, true),
    esp: state.getUint32(stateOffset.registers + 4 * 4, true),
    integerResult: results.getUint32(guest.resultAddress, true),
    callResult: results.getUint32(guest.resultAddress + 4, true),
    x87Result: results.getInt32(guest.resultAddress + 8, true),
    sseResultBits: results.getUint32(guest.resultAddress + 0x14, true),
    hostImportCalls: calls,
  };

  assert.deepEqual(smoke, {
    cpl: 0,
    timerDelay: 100,
    instructionCount: 21,
    instructionPointer: guest.codeAddress + 80,
    eax: 54,
    esp: guest.stackAddress,
    integerResult: 0x12345679,
    callResult: 54,
    x87Result: 2,
    sseResultBits: 0x40000000,
    hostImportCalls: {
      microtick: 3,
      cpu_event_halt: 1,
      run_hardware_timers: 1,
    },
  });

  return { guestMemoryBase, programSize: program.byteLength, ...smoke };
}

async function runCpl3TrapSmoke(module, moduleBytes) {
  const calls = {};
  let exports;
  let trappedException;
  const imports = createEnvironment(module, calls, {
    cpu_exception_hook: (exception) => {
      trappedException = exception;
      const state = new DataView(exports.memory.buffer);
      state.setUint8(stateOffset.inHalt, 1);
      state.setUint32(
        stateOffset.instructionPointer,
        guest.codeAddress + cpl3TrapProgram.byteLength,
        true,
      );
      return 1;
    },
  });
  const result = await WebAssembly.instantiate(moduleBytes, imports);
  exports = result.instance.exports;

  exports.rust_init();
  let state = new DataView(exports.memory.buffer);
  state.setUint32(stateOffset.memorySize, guest.memorySize, true);
  const guestMemoryBase = exports.allocate_memory(guest.memorySize);
  state = new DataView(exports.memory.buffer);
  const memory = new Uint8Array(
    exports.memory.buffer,
    guestMemoryBase,
    guest.memorySize,
  );
  exports.zero_memory(0, guest.memorySize);
  exports.reset_cpu();
  memory.set(cpl3TrapProgram, guest.codeAddress);
  configureFlat32State(exports, state, 3);

  const timerDelay = exports.main_loop();
  const smoke = {
    guestMemoryBase,
    programSize: cpl3TrapProgram.byteLength,
    cpl: state.getUint8(stateOffset.cpl),
    trappedException,
    timerDelay,
    instructionCount: state.getUint32(stateOffset.instructionCounter, true),
    instructionPointer: state.getUint32(
      stateOffset.instructionPointer,
      true,
    ),
    eax: state.getUint32(stateOffset.registers, true),
    hostImportCalls: calls,
  };

  assert.deepEqual(smoke, {
    guestMemoryBase,
    programSize: 7,
    cpl: 3,
    trappedException: 6,
    timerDelay: 100,
    instructionCount: 2,
    instructionPointer: guest.codeAddress + 7,
    eax: 42,
    hostImportCalls: {
      log_from_wasm: 3,
      microtick: 3,
      dbg_trace_from_wasm: 1,
      cpu_exception_hook: 1,
      run_hardware_timers: 1,
    },
  });

  return smoke;
}

fs.rmSync(outputDirectory, { recursive: true, force: true });
fs.mkdirSync(outputDirectory, { recursive: true });

const packageManifest = JSON.parse(fs.readFileSync(packageManifestPath, "utf8"));
assert.equal(packageManifest.name, "v86");
assert.equal(packageManifest.version, "0.5.424+g2f1346b");

const fallbackArtifact = inspectArtifact(artifacts.fallback);
const jitArtifact = inspectArtifact(artifacts.jit);
const debugArtifact = inspectArtifact(artifacts.debug);
const fallbackModule = new WebAssembly.Module(fallbackArtifact.bytes);
const jitModule = new WebAssembly.Module(jitArtifact.bytes);
const debugModule = new WebAssembly.Module(debugArtifact.bytes);
const fallbackImports = describeImports(fallbackModule);
const fallbackExports = describeExports(fallbackModule);
const fallbackExportNames = new Set(fallbackExports.map(({ name }) => name));

assert.deepEqual(fallbackImports, expectedImports);
assert(!fallbackImports.includes("env.cpu_exception_hook:function"));
assert(
  describeImports(debugModule).includes("env.cpu_exception_hook:function"),
);
for (const requiredExport of requiredExports) {
  assert(fallbackExportNames.has(requiredExport), `missing export ${requiredExport}`);
}

const smoke = await runSmoke(fallbackModule, fallbackArtifact.bytes);
const cpl3TrapSmoke = await runCpl3TrapSmoke(
  debugModule,
  debugArtifact.bytes,
);
const report = {
  schemaVersion: 1,
  candidate: {
    package: "v86",
    requestedVersion: "0.5.424",
    embeddedVersion: packageManifest.version,
    status: "adapter-feasibility-candidate",
  },
  artifacts: {
    fallback: {
      size: fallbackArtifact.size,
      sha256: fallbackArtifact.sha256,
      importCount: fallbackImports.length,
      exportCount: fallbackExports.length,
    },
    jit: {
      size: jitArtifact.size,
      sha256: jitArtifact.sha256,
      importCount: WebAssembly.Module.imports(jitModule).length,
      exportCount: WebAssembly.Module.exports(jitModule).length,
    },
    debug: {
      size: debugArtifact.size,
      sha256: debugArtifact.sha256,
      importCount: WebAssembly.Module.imports(debugModule).length,
      exportCount: WebAssembly.Module.exports(debugModule).length,
    },
  },
  adapterContract: {
    imports: fallbackImports,
    requiredExports,
  },
  smoke,
  cpl3TrapSmoke,
};

fs.writeFileSync(
  path.join(outputDirectory, "report.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);

console.log(
  `x86 engine spike passed: v86 ${report.candidate.requestedVersion}, ` +
    `${smoke.instructionCount} instructions, EAX=${smoke.eax}, ` +
    `x87=${smoke.x87Result}, SSE=0x${smoke.sseResultBits.toString(16)}, ` +
    `CPL3 trap=#${cpl3TrapSmoke.trappedException}`,
);
