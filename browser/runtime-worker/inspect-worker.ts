import type { SourceDirectory } from "../storage/dev-source-directory.js";
import { gameStorageFingerprint } from "../storage/source-directory.js";
import { RuntimeBridge } from "./runtime-bridge.js";
import { openSourceExecutable } from "./source-explorer.js";
import { openWritableTree } from "./writable-tree.js";

export interface InspectExecutableMessage {
  type: "inspect-executable";
  requestId: number;
  root: SourceDirectory;
  segments: string[];
  name: string;
}

export interface InspectedExecutableMessage {
  type: "inspected-executable";
  requestId: number;
  result: { sha256: string; imageBase: number; entryPoint: number; importModules: number } | null;
  storageFingerprint: string | null;
  error: string | null;
}

const worker = self as DedicatedWorkerGlobalScope;
let inspector: Promise<RuntimeBridge> | null = null;
worker.addEventListener("message", (event: MessageEvent<InspectExecutableMessage>) => {
  const { requestId, root, segments, name } = event.data;
  if (event.data.type !== "inspect-executable") return;
  void (async () => {
    let result: InspectedExecutableMessage["result"] = null;
    try {
      if (root?.kind !== "directory" || !Array.isArray(segments)) {
        throw new Error("Invalid source directory");
      }
      const file = await openSourceExecutable(root, segments, name);
      if (file.size > 16 * 1024 * 1024) throw new Error("Executable exceeds the bridge input limit");
      const bytes = new Uint8Array(await file.arrayBuffer());
      const after = await openSourceExecutable(root, segments, name);
      if (bytes.length !== file.size || after.size !== file.size ||
          after.lastModified !== file.lastModified ||
          after.sourceVersion !== file.sourceVersion) throw new Error("Executable changed while reading");
      inspector ??= RuntimeBridge.instantiate(new URL("./runtime-core.wasm", import.meta.url));
      result = (await inspector).inspectExecutable(bytes, [...segments, name].join("/"));
      const storageFingerprint = await gameStorageFingerprint(root, segments, name, result.sha256);
      await openWritableTree(storageFingerprint);
      worker.postMessage({ type: "inspected-executable", requestId, result, storageFingerprint,
        error: null } satisfies InspectedExecutableMessage);
    } catch (error) {
      worker.postMessage({ type: "inspected-executable", requestId, result,
        storageFingerprint: null, error: error instanceof Error ? error.message : String(error)
      } satisfies InspectedExecutableMessage);
    }
  })();
});
