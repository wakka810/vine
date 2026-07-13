import type {
  ExecutionEngine,
  ExecutionEngineIdentity,
} from "./execution-engine-contract.js";
import { V86Mirror } from "./v86-mirror.js";

export * from "./execution-engine-contract.js";

export async function instantiateExecutionEngine(baseUrl: URL): Promise<ExecutionEngine> {
  return V86Mirror.instantiate(new URL("./v86-adapter.wasm", baseUrl));
}

export async function instantiateExecutionEngineBytes(
  bytes: Uint8Array,
  identity?: ExecutionEngineIdentity,
): Promise<ExecutionEngine> {
  return V86Mirror.instantiateBytes(bytes, identity);
}
