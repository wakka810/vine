import type {
  ExecutionEngine,
  ExecutionEngineFactory,
  ExecutionEngineIdentity,
  ExecutionEngineOptions,
} from "./execution-engine-contract.js";
import { V86Mirror } from "./v86-mirror.js";

export * from "./execution-engine-contract.js";

export async function prepareExecutionEngine(baseUrl: URL): Promise<ExecutionEngineFactory> {
  const factory = await V86Mirror.prepare(new URL("./v86-adapter.wasm", baseUrl));
  return (options) => factory(options);
}

export async function instantiateExecutionEngine(
  baseUrl: URL,
  options: ExecutionEngineOptions,
): Promise<ExecutionEngine> {
  return V86Mirror.instantiate(new URL("./v86-adapter.wasm", baseUrl), options);
}

export async function instantiateExecutionEngineBytes(
  bytes: Uint8Array,
  identity?: ExecutionEngineIdentity,
  options?: ExecutionEngineOptions,
): Promise<ExecutionEngine> {
  return V86Mirror.instantiateBytes(bytes, identity, options);
}
