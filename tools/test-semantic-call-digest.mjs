import assert from "node:assert/strict";
import crypto from "node:crypto";

import {
  SEMANTIC_CALL_DIGEST_ALGORITHM,
  SemanticCallDigest,
  normalizeSemanticCall,
} from "../browser/runtime-worker/semantic-call.mjs";

const call = {
  sequence: 1,
  module: "KERNEL32.dll",
  symbol: "GetLastError",
  arguments: [],
  result: { action: "set", value: 87 },
  implementationId: "internal-handler-name-is-not-evidence",
  returnAddress: 0x1000,
  stackPointerBefore: 0x2000,
  stackPointerAfter: 0x2004,
  lastError: 87,
  guestWrites: [],
};
const normalized = normalizeSemanticCall(call);
assert.equal(SEMANTIC_CALL_DIGEST_ALGORITHM, "sha256-canonical-json-v1");
assert.equal(normalized.module, "kernel32.dll");
assert.equal("implementationId" in normalized, false);

const expectedLine =
  '{"arguments":[],"currentDirectory":null,"cursor":null,"gdi":null,"guestWrites":[],"icon":null,' +
  '"interface":null,"lastError":87,"module":"kernel32.dll","mutex":null,' +
  '"objectRetired":null,"referenceCount":null,"result":{"action":"set","value":87},' +
  '"returnAddress":4096,"sequence":1,"sourceRead":null,"stackPointerAfter":8196,' +
  '"stackPointerBefore":8192,"symbol":"GetLastError","vtableSlot":null,' +
  '"waitableTimer":null,"window":null,"windowClass":null}\n';
const digest = new SemanticCallDigest();
digest.update(call);
assert.equal(digest.count(), 1);
assert.equal(
  digest.digestHex(),
  crypto.createHash("sha256").update(expectedLine).digest("hex"),
);

const reordered = {
  guestWrites: [],
  stackPointerAfter: 0x2004,
  returnAddress: 0x1000,
  result: { value: 87, action: "set" },
  symbol: "GetLastError",
  sequence: 1,
  arguments: [],
  module: "kernel32.DLL",
  stackPointerBefore: 0x2000,
  lastError: 87,
  implementationId: "another-internal-name",
};
const reorderedDigest = new SemanticCallDigest();
reorderedDigest.update(reordered);
assert.equal(reorderedDigest.digestHex(), crypto.createHash("sha256").update(expectedLine).digest("hex"));

const empty = new SemanticCallDigest();
assert.equal(empty.count(), 0);
assert.equal(
  empty.digestHex(),
  "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
);

const secondCall = { ...call, sequence: 2, symbol: "GetCurrentThreadId", result: 8192 };
const forward = new SemanticCallDigest();
forward.update(call);
forward.update(secondCall);
const reverseCompletion = new SemanticCallDigest();
reverseCompletion.update(secondCall);
reverseCompletion.update(call);
assert.equal(
  reverseCompletion.digestHex(),
  forward.digestHex(),
  "completion order changed the guest-visible sequence digest",
);
const duplicate = new SemanticCallDigest();
duplicate.update(call);
assert.throws(() => duplicate.update(call), /sequence 1 is duplicated/u);

console.log("semantic call digest contract passed");
