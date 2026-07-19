const SHA256_INITIAL_STATE = [
  0x6a09e667,
  0xbb67ae85,
  0x3c6ef372,
  0xa54ff53a,
  0x510e527f,
  0x9b05688c,
  0x1f83d9ab,
  0x5be0cd19,
];

const SHA256_ROUND_CONSTANTS = [
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
  0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
  0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
  0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
  0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
  0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
  0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
  0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
  0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

export const SEMANTIC_CALL_DIGEST_ALGORITHM = "sha256-canonical-json-v1";

function rotateRight(value, bits) {
  return (value >>> bits) | (value << (32 - bits));
}

function canonicalJson(value) {
  if (value === null || typeof value !== "object") {
    const encoded = JSON.stringify(value);
    if (encoded === undefined) {
      throw new TypeError("semantic call contains a non-serializable scalar");
    }
    return encoded;
  }
  if (Array.isArray(value)) {
    return `[${value.map((entry) => canonicalJson(entry ?? null)).join(",")}]`;
  }
  const fields = Object.keys(value)
    .filter((key) => value[key] !== undefined)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`);
  return `{${fields.join(",")}}`;
}

class Sha256 {
  #state = Uint32Array.from(SHA256_INITIAL_STATE);
  #block = new Uint8Array(64);
  #blockBytes = 0;
  #bytesHashed = 0;
  #finished = false;

  update(bytes) {
    if (this.#finished) {
      throw new Error("SHA-256 digest is already finalized");
    }
    this.#bytesHashed += bytes.byteLength;
    let offset = 0;
    while (offset < bytes.byteLength) {
      const copied = Math.min(64 - this.#blockBytes, bytes.byteLength - offset);
      this.#block.set(bytes.subarray(offset, offset + copied), this.#blockBytes);
      this.#blockBytes += copied;
      offset += copied;
      if (this.#blockBytes === 64) {
        this.#compress(this.#block);
        this.#blockBytes = 0;
      }
    }
  }

  digestHex() {
    if (this.#finished) {
      throw new Error("SHA-256 digest is already finalized");
    }
    this.#finished = true;
    const bitLength = this.#bytesHashed * 8;
    if (!Number.isSafeInteger(bitLength)) {
      throw new RangeError("semantic call digest input is too large");
    }
    this.#block[this.#blockBytes++] = 0x80;
    if (this.#blockBytes > 56) {
      this.#block.fill(0, this.#blockBytes);
      this.#compress(this.#block);
      this.#blockBytes = 0;
    }
    this.#block.fill(0, this.#blockBytes, 56);
    const view = new DataView(this.#block.buffer);
    view.setUint32(56, Math.floor(bitLength / 0x1_0000_0000), false);
    view.setUint32(60, bitLength >>> 0, false);
    this.#compress(this.#block);
    return [...this.#state]
      .map((word) => word.toString(16).padStart(8, "0"))
      .join("");
  }

  #compress(block) {
    const words = new Uint32Array(64);
    const view = new DataView(block.buffer, block.byteOffset, block.byteLength);
    for (let index = 0; index < 16; index += 1) {
      words[index] = view.getUint32(index * 4, false);
    }
    for (let index = 16; index < 64; index += 1) {
      const previous15 = words[index - 15];
      const previous2 = words[index - 2];
      const sigma0 =
        rotateRight(previous15, 7) ^ rotateRight(previous15, 18) ^ (previous15 >>> 3);
      const sigma1 =
        rotateRight(previous2, 17) ^ rotateRight(previous2, 19) ^ (previous2 >>> 10);
      words[index] =
        (words[index - 16] + sigma0 + words[index - 7] + sigma1) >>> 0;
    }

    let [a, b, c, d, e, f, g, h] = this.#state;
    for (let index = 0; index < 64; index += 1) {
      const sum1 = rotateRight(e, 6) ^ rotateRight(e, 11) ^ rotateRight(e, 25);
      const choice = (e & f) ^ (~e & g);
      const temporary1 =
        (h + sum1 + choice + SHA256_ROUND_CONSTANTS[index] + words[index]) >>> 0;
      const sum0 = rotateRight(a, 2) ^ rotateRight(a, 13) ^ rotateRight(a, 22);
      const majority = (a & b) ^ (a & c) ^ (b & c);
      const temporary2 = (sum0 + majority) >>> 0;
      h = g;
      g = f;
      f = e;
      e = (d + temporary1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (temporary1 + temporary2) >>> 0;
    }
    const values = [a, b, c, d, e, f, g, h];
    for (let index = 0; index < this.#state.length; index += 1) {
      this.#state[index] = (this.#state[index] + values[index]) >>> 0;
    }
  }
}

export function normalizeSemanticCall(call) {
  return {
    sequence: call.sequence,
    module: call.module.toLowerCase(),
    symbol: call.symbol,
    arguments: call.arguments,
    result: call.result,
    returnAddress: call.returnAddress,
    stackPointerBefore: call.stackPointerBefore,
    stackPointerAfter: call.stackPointerAfter,
    lastError: call.lastError,
    guestWrites: call.guestWrites.map(({ address, bytes }) => ({ address, bytes })),
    currentDirectory: call.currentDirectory ?? null,
    mutex: call.mutex ?? null,
    waitableTimer: call.waitableTimer ?? null,
    icon: call.icon ?? null,
    cursor: call.cursor ?? null,
    windowClass: call.windowClass ?? null,
    window: call.window ?? null,
    sourceRead: call.sourceRead ?? null,
    gdi: call.gdi ?? null,
    interface: call.interface ?? null,
    vtableSlot: call.vtableSlot ?? null,
    referenceCount: call.referenceCount ?? null,
    objectRetired: call.objectRetired ?? null,
  };
}

export class SemanticCallDigest {
  #encoder = new TextEncoder();
  #canonicalCalls = new Map();
  #finished = false;

  update(call) {
    if (this.#finished) {
      throw new Error("semantic call digest is already finalized");
    }
    if (!Number.isSafeInteger(call.sequence) || call.sequence < 1) {
      throw new RangeError("semantic call sequence must be a positive integer");
    }
    if (this.#canonicalCalls.has(call.sequence)) {
      throw new Error(`semantic call sequence ${call.sequence} is duplicated`);
    }
    this.#canonicalCalls.set(
      call.sequence,
      `${canonicalJson(normalizeSemanticCall(call))}\n`,
    );
  }

  count() {
    return this.#canonicalCalls.size;
  }

  digestHex() {
    if (this.#finished) {
      throw new Error("semantic call digest is already finalized");
    }
    this.#finished = true;
    const sha256 = new Sha256();
    for (const sequence of [...this.#canonicalCalls.keys()].sort((left, right) => left - right)) {
      sha256.update(this.#encoder.encode(this.#canonicalCalls.get(sequence)));
    }
    return sha256.digestHex();
  }
}
