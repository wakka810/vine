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

const comCall = {
  ...call,
  module: "<com>",
  symbol: "IDirect3DSurface9.GetDC",
  interface: "IDirect3DSurface9",
  vtableSlot: 15,
  guestWrites: [{ address: 0x3000, bytes: [1, 2, 3, 4], internal: "ignored" }],
};
delete comCall.lastError;
const expectedComLine =
  '{"arguments":[],"currentDirectory":null,"cursor":null,"gdi":null,' +
  '"guestWrites":[{"address":12288,"bytes":[1,2,3,4]}],"icon":null,' +
  '"interface":"IDirect3DSurface9","module":"<com>","mutex":null,' +
  '"objectRetired":null,"referenceCount":null,"result":{"action":"set","value":87},' +
  '"returnAddress":4096,"sequence":1,"sourceRead":null,"stackPointerAfter":8196,' +
  '"stackPointerBefore":8192,"symbol":"IDirect3DSurface9.GetDC","vtableSlot":15,' +
  '"waitableTimer":null,"window":null,"windowClass":null}\n';
const comDigest = new SemanticCallDigest();
comDigest.update(comCall);
assert.equal(
  comDigest.digestHex(),
  crypto.createHash("sha256").update(expectedComLine).digest("hex"),
  "direct semantic-call streaming changed optional-field or guest-write normalization",
);

const compactWriteCall = {
  ...comCall,
  guestWrites: [
    {
      address: 0x3000,
      byteLength: 65_537,
      contentSha256:
        "a3253bfa7d790dc6f5d41b75421bcc2c606e8043f9fad4331e8aae0a6c504cfa",
      internal: "ignored",
    },
  ],
};
const expectedCompactWriteLine = expectedComLine.replace(
  '{"address":12288,"bytes":[1,2,3,4]}',
  '{"address":12288,"byteLength":65537,"contentSha256":' +
    '"a3253bfa7d790dc6f5d41b75421bcc2c606e8043f9fad4331e8aae0a6c504cfa"}',
);
assert.deepEqual(normalizeSemanticCall(compactWriteCall).guestWrites, [
  {
    address: 0x3000,
    byteLength: 65_537,
    contentSha256:
      "a3253bfa7d790dc6f5d41b75421bcc2c606e8043f9fad4331e8aae0a6c504cfa",
  },
]);
const compactWriteDigest = new SemanticCallDigest();
compactWriteDigest.update(compactWriteCall);
assert.equal(
  compactWriteDigest.digestHex(),
  crypto.createHash("sha256").update(expectedCompactWriteLine).digest("hex"),
  "compact guest-write metadata changed semantic-call normalization",
);

const unicodeSymbol = "\u30c6\u30b9\u30c8";
const unicodeCall = { ...call, symbol: unicodeSymbol };
const unicodeDigest = new SemanticCallDigest();
unicodeDigest.update(unicodeCall);
assert.equal(
  unicodeDigest.digestHex(),
  crypto
    .createHash("sha256")
    .update(expectedLine.replace('"GetLastError"', JSON.stringify(unicodeSymbol)))
    .digest("hex"),
  "non-ASCII semantic-call fragments changed UTF-8 encoding",
);

const duplicate = new SemanticCallDigest();
duplicate.update(call);
assert.throws(() => duplicate.update(call), /sequence 1 is duplicated/u);

console.log("semantic call digest contract passed");
