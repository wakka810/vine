import type {
  ExecutionEngine,
  ExecutionEngineOptions,
} from "./execution-engine-contract.js";
import { V86Mirror } from "./v86-mirror.js";
import { RecompilerMirror } from "./recompiler-mirror.js";

export * from "./execution-engine-contract.js";

function validateBackend(options: ExecutionEngineOptions): void {
  if (options?.backend !== undefined && options.backend !== "v86" && options.backend !== "recompiler") {
    throw new Error("Unknown execution backend");
  }
}

export async function instantiateExecutionEngine(
  baseUrl: URL,
  options: ExecutionEngineOptions,
): Promise<ExecutionEngine> {
  validateBackend(options);
  if (options?.backend === "recompiler") {
    return RecompilerMirror.instantiate(new URL("./vine_recompiler.wasm", baseUrl), options);
  }
  return V86Mirror.instantiate(new URL("./v86-adapter.wasm", baseUrl), options);
}

export async function instantiateExecutionEngineBytes(
  bytes: Uint8Array,
  options: ExecutionEngineOptions,
): Promise<ExecutionEngine> {
  validateBackend(options);
  if (options?.backend === "recompiler") return RecompilerMirror.instantiateBytes(bytes, options);
  return V86Mirror.instantiateBytes(bytes, options);
}
