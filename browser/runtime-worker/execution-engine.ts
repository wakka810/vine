import type {
  ExecutionEngine,
  ExecutionEngineOptions,
} from "./execution-engine-contract.js";
import { V86Mirror } from "./v86-mirror.js";

export * from "./execution-engine-contract.js";

export async function instantiateExecutionEngine(
  baseUrl: URL,
  options: ExecutionEngineOptions,
): Promise<ExecutionEngine> {
  return V86Mirror.instantiate(new URL("./v86-adapter.wasm", baseUrl), options);
}

export async function instantiateExecutionEngineBytes(
  bytes: Uint8Array,
  options: ExecutionEngineOptions,
): Promise<ExecutionEngine> {
  return V86Mirror.instantiateBytes(bytes, options);
}
