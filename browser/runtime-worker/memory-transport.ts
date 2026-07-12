const pageSize = 0x1000;
const ascii = new TextDecoder("ascii", { fatal: true });

export interface MemoryProtection {
  read: boolean;
  write: boolean;
  execute: boolean;
}

export interface ReflectedPage {
  address: number;
  owner: number;
  protection: MemoryProtection;
  ownerSealed: boolean;
  dirty: boolean;
  bytes: Uint8Array;
}

export interface CacheEffects {
  flushTlb: boolean;
  invalidateInstructionFetch: boolean;
  invalidateGeneratedCode: boolean;
}

export interface MemoryReflection {
  generation: bigint;
  cacheEffects: CacheEffects;
  pages: ReflectedPage[];
}

export interface HostMemoryDelta {
  fromGeneration: bigint;
  generation: bigint;
  cacheEffects: CacheEffects;
  pages: ReflectedPage[];
}

export interface DirtyMirrorPage {
  address: number;
  owner: number;
  bytes: Uint8Array;
}

export interface CpuWritebackBuffers {
  control: Uint8Array;
  payload: Uint8Array;
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function view(bytes: Uint8Array): DataView {
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
}

function expectMagic(bytes: Uint8Array, expected: string): void {
  invariant(bytes.byteLength >= expected.length, `${expected} control buffer is truncated`);
  invariant(ascii.decode(bytes.subarray(0, expected.length)) === expected, `expected ${expected}`);
}

function cacheEffects(flags: number): CacheEffects {
  return {
    flushTlb: Boolean(flags & 1),
    invalidateInstructionFetch: Boolean(flags & 2),
    invalidateGeneratedCode: Boolean(flags & 4),
  };
}

function protection(flags: number): MemoryProtection {
  return {
    read: Boolean(flags & 1),
    write: Boolean(flags & 2),
    execute: Boolean(flags & 4),
  };
}

export function decodeReflection(control: Uint8Array, payload: Uint8Array): MemoryReflection {
  const headerBytes = 40;
  const descriptorBytes = 16;
  expectMagic(control, "VINERF01");
  const controlView = view(control);
  invariant(controlView.getUint32(8, true) === 1, "unsupported reflection version");
  invariant(controlView.getUint32(12, true) === pageSize, "unexpected reflection page size");
  const generation = controlView.getBigUint64(16, true);
  const count = controlView.getUint32(24, true);
  invariant(
    controlView.getUint32(28, true) === descriptorBytes,
    "unexpected reflection descriptor size",
  );
  invariant(
    control.byteLength === headerBytes + count * descriptorBytes,
    "reflection control length mismatch",
  );
  invariant(payload.byteLength === count * pageSize, "reflection payload length mismatch");
  const pages: ReflectedPage[] = [];
  for (let index = 0; index < count; index += 1) {
    const offset = headerBytes + index * descriptorBytes;
    const flags = controlView.getUint32(offset + 8, true);
    pages.push({
      address: controlView.getUint32(offset, true),
      owner: controlView.getUint32(offset + 4, true),
      protection: protection(flags),
      ownerSealed: Boolean(flags & 8),
      dirty: Boolean(flags & 16),
      bytes: payload.subarray(index * pageSize, (index + 1) * pageSize),
    });
  }
  invariant(pages.every((page) => page.ownerSealed && page.dirty), "invalid initial page state");
  return {
    generation,
    cacheEffects: cacheEffects(controlView.getUint32(32, true)),
    pages,
  };
}

export function decodeHostDelta(control: Uint8Array, payload: Uint8Array): HostMemoryDelta {
  const headerBytes = 56;
  const mappingDescriptorBytes = 32;
  const pageDescriptorBytes = 16;
  expectMagic(control, "VINEDL01");
  const controlView = view(control);
  invariant(controlView.getUint32(8, true) === 1, "unsupported host delta version");
  invariant(controlView.getUint32(12, true) === pageSize, "unexpected host delta page size");
  const fromGeneration = controlView.getBigUint64(16, true);
  const generation = controlView.getBigUint64(24, true);
  const mappingCount = controlView.getUint32(32, true);
  const dirtyCount = controlView.getUint32(40, true);
  invariant(
    controlView.getUint32(36, true) === mappingDescriptorBytes,
    "unexpected mapping descriptor size",
  );
  invariant(
    controlView.getUint32(44, true) === pageDescriptorBytes,
    "unexpected delta page descriptor size",
  );
  invariant(mappingCount === 0, "browser vertical slice does not accept mapping deltas");
  invariant(
    control.byteLength ===
      headerBytes + mappingCount * mappingDescriptorBytes + dirtyCount * pageDescriptorBytes,
    "host delta control length mismatch",
  );
  invariant(payload.byteLength === dirtyCount * pageSize, "host delta payload length mismatch");
  const pages: ReflectedPage[] = [];
  const dirtyBase = headerBytes + mappingCount * mappingDescriptorBytes;
  for (let index = 0; index < dirtyCount; index += 1) {
    const offset = dirtyBase + index * pageDescriptorBytes;
    const flags = controlView.getUint32(offset + 8, true);
    pages.push({
      address: controlView.getUint32(offset, true),
      owner: controlView.getUint32(offset + 4, true),
      protection: protection(flags),
      ownerSealed: Boolean(flags & 8),
      dirty: true,
      bytes: payload.subarray(index * pageSize, (index + 1) * pageSize),
    });
  }
  return {
    fromGeneration,
    generation,
    cacheEffects: cacheEffects(controlView.getUint32(48, true)),
    pages,
  };
}

export function encodeCpuWriteback(
  generation: bigint,
  pages: DirtyMirrorPage[],
): CpuWritebackBuffers {
  const headerBytes = 32;
  const descriptorBytes = 8;
  const control = new Uint8Array(headerBytes + pages.length * descriptorBytes);
  control.set(new TextEncoder().encode("VINEWB01"), 0);
  const controlView = view(control);
  controlView.setUint32(8, 1, true);
  controlView.setUint32(12, pageSize, true);
  controlView.setBigUint64(16, generation, true);
  controlView.setUint32(24, pages.length, true);
  controlView.setUint32(28, descriptorBytes, true);
  const payload = new Uint8Array(pages.length * pageSize);
  pages.forEach((page, index) => {
    invariant(page.bytes.byteLength === pageSize, "writeback page is not 4 KiB");
    const offset = headerBytes + index * descriptorBytes;
    controlView.setUint32(offset, page.address, true);
    controlView.setUint32(offset + 4, page.owner, true);
    payload.set(page.bytes, index * pageSize);
  });
  return { control, payload };
}

export const memoryPageSize = pageSize;
