const pageSize = 0x1000;
const ascii = new TextDecoder("ascii", { fatal: true });

export interface MemoryProtection {
  read: boolean;
  write: boolean;
  execute: boolean;
}

export type MemoryRegionKind =
  | "image"
  | "stack"
  | "thread-environment-block"
  | "process-environment-block"
  | "process-parameters"
  | "command-line"
  | "heap"
  | "anonymous"
  | "runtime"
  | "surface"
  | "graphics-resource"
  | "audio-resource"
  | "gdi-resource";

export interface MemoryRange {
  base: number;
  byteLength: number;
}

export interface MemoryRegionSnapshot {
  addressAllocationId?: number;
  owner: number;
  kind: MemoryRegionKind;
  range: {
    start: number;
    byteLength: number;
  };
}

export type MappingOperation =
  | {
      operation: "reserve";
      addressAllocationId?: number;
      owner: number;
      kind: MemoryRegionKind;
      range: MemoryRange;
    }
  | {
      operation: "commit";
      addressAllocationId?: number;
      owner: number;
      range: MemoryRange;
      protection: MemoryProtection;
    }
  | {
      operation: "protect";
      addressAllocationId?: number;
      owner: number;
      range: MemoryRange;
      protection: MemoryProtection;
    }
  | { operation: "decommit"; addressAllocationId?: number; owner: number; range: MemoryRange }
  | { operation: "release"; addressAllocationId?: number; owner: number; range: MemoryRange };

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
  mappingOperations: MappingOperation[];
  pages: ReflectedPage[];
}

export interface DirtyMirrorPage {
  address: number;
  owner: number;
  bytes: Uint8Array;
}

const cpuWritebackMagic = Uint8Array.from([0x56, 0x49, 0x4e, 0x45, 0x57, 0x42, 0x43, 0x4b]);

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
  invariant((flags & ~7) === 0, "memory protection contains unknown flags");
  return {
    read: Boolean(flags & 1),
    write: Boolean(flags & 2),
    execute: Boolean(flags & 4),
  };
}

function regionKind(code: number): MemoryRegionKind {
  const kinds: MemoryRegionKind[] = [
    "image",
    "stack",
    "thread-environment-block",
    "process-environment-block",
    "process-parameters",
    "command-line",
    "heap",
    "anonymous",
    "runtime",
    "surface",
    "graphics-resource",
    "audio-resource",
    "gdi-resource",
  ];
  const kind = kinds[code - 1];
  invariant(kind !== undefined, `unknown memory region kind ${code}`);
  return kind;
}

function mappingRange(controlView: DataView, offset: number): MemoryRange {
  const base = controlView.getUint32(offset + 16, true);
  const byteLength64 = controlView.getBigUint64(offset + 20, true);
  invariant(byteLength64 > 0n && byteLength64 <= 0x1_0000_0000n, "mapping range length is invalid");
  const byteLength = Number(byteLength64);
  invariant((base & (pageSize - 1)) === 0, "mapping range base is not page aligned");
  invariant(byteLength % pageSize === 0, "mapping range length is not page aligned");
  invariant(base + byteLength <= 0x1_0000_0000, "mapping range crosses the x86 address limit");
  return { base, byteLength };
}

export function decodeReflection(control: Uint8Array, payload: Uint8Array): MemoryReflection {
  const headerBytes = 36;
  const descriptorBytes = 16;
  expectMagic(control, "VINEREFL");
  invariant(control.byteLength >= headerBytes, "reflection control header is truncated");
  const controlView = view(control);
  invariant(controlView.getUint32(8, true) === pageSize, "unexpected reflection page size");
  const generation = controlView.getBigUint64(12, true);
  const count = controlView.getUint32(20, true);
  invariant(
    controlView.getUint32(24, true) === descriptorBytes,
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
      protection: protection(flags & 7),
      ownerSealed: Boolean(flags & 8),
      dirty: Boolean(flags & 16),
      bytes: payload.subarray(index * pageSize, (index + 1) * pageSize),
    });
  }
  invariant(
    pages.every((page) => page.dirty && (!page.protection.execute || page.ownerSealed)),
    "invalid reflected page state",
  );
  return {
    generation,
    cacheEffects: cacheEffects(controlView.getUint32(28, true)),
    pages,
  };
}

export function decodeHostDelta(control: Uint8Array, payload: Uint8Array): HostMemoryDelta {
  const headerBytes = 52;
  const mappingDescriptorBytes = 40;
  const pageDescriptorBytes = 16;
  expectMagic(control, "VINEDELT");
  invariant(control.byteLength >= headerBytes, "host delta control header is truncated");
  const controlView = view(control);
  invariant(controlView.getUint32(8, true) === pageSize, "unexpected host delta page size");
  const fromGeneration = controlView.getBigUint64(12, true);
  const generation = controlView.getBigUint64(20, true);
  const mappingCount = controlView.getUint32(28, true);
  const dirtyCount = controlView.getUint32(36, true);
  invariant(
    controlView.getUint32(32, true) === mappingDescriptorBytes,
    "unexpected mapping descriptor size",
  );
  invariant(
    controlView.getUint32(40, true) === pageDescriptorBytes,
    "unexpected delta page descriptor size",
  );
  invariant(
    control.byteLength ===
      headerBytes + mappingCount * mappingDescriptorBytes + dirtyCount * pageDescriptorBytes,
    "host delta control length mismatch",
  );
  invariant(payload.byteLength === dirtyCount * pageSize, "host delta payload length mismatch");
  const mappingOperations: MappingOperation[] = [];
  for (let index = 0; index < mappingCount; index += 1) {
    const offset = headerBytes + index * mappingDescriptorBytes;
    const operation = controlView.getUint32(offset, true);
    const owner = controlView.getUint32(offset + 4, true);
    const kind = controlView.getUint32(offset + 8, true);
    const protectionFlags = controlView.getUint32(offset + 12, true);
    const range = mappingRange(controlView, offset);
    const encodedAddressAllocationId = controlView.getBigUint64(offset + 28, true);
    invariant(
      encodedAddressAllocationId <= BigInt(Number.MAX_SAFE_INTEGER),
      "mapping address allocation ID exceeds exact browser accounting",
    );
    const addressAllocationId = Number(encodedAddressAllocationId);
    const allocationIdentity =
      addressAllocationId === 0 ? {} : { addressAllocationId };
    invariant(owner !== 0, "mapping operation owner is zero");
    invariant(
      controlView.getUint32(offset + 36, true) === 0,
      "mapping descriptor reserved field is nonzero",
    );
    switch (operation) {
      case 1:
        invariant(protectionFlags === 0, "reserve operation contains protection flags");
        mappingOperations.push({
          operation: "reserve",
          ...allocationIdentity,
          owner,
          kind: regionKind(kind),
          range,
        });
        break;
      case 2:
        invariant(kind === 0, "commit operation contains a region kind");
        mappingOperations.push({
          operation: "commit",
          ...allocationIdentity,
          owner,
          range,
          protection: protection(protectionFlags),
        });
        break;
      case 3:
        invariant(kind === 0, "protect operation contains a region kind");
        mappingOperations.push({
          operation: "protect",
          ...allocationIdentity,
          owner,
          range,
          protection: protection(protectionFlags),
        });
        break;
      case 4:
        invariant(kind === 0 && protectionFlags === 0, "decommit operation contains extra policy");
        mappingOperations.push({ operation: "decommit", ...allocationIdentity, owner, range });
        break;
      case 5:
        invariant(kind === 0 && protectionFlags === 0, "release operation contains extra policy");
        mappingOperations.push({ operation: "release", ...allocationIdentity, owner, range });
        break;
      default:
        throw new Error(`unknown mapping operation ${operation}`);
    }
  }
  const pages: ReflectedPage[] = [];
  const dirtyBase = headerBytes + mappingCount * mappingDescriptorBytes;
  for (let index = 0; index < dirtyCount; index += 1) {
    const offset = dirtyBase + index * pageDescriptorBytes;
    const flags = controlView.getUint32(offset + 8, true);
    invariant((flags & ~15) === 0, "host delta page contains unknown flags");
    invariant(controlView.getUint32(offset + 12, true) === 0, "delta page reserved field is nonzero");
    pages.push({
      address: controlView.getUint32(offset, true),
      owner: controlView.getUint32(offset + 4, true),
      protection: protection(flags & 7),
      ownerSealed: Boolean(flags & 8),
      dirty: true,
      bytes: payload.subarray(index * pageSize, (index + 1) * pageSize),
    });
  }
  return {
    fromGeneration,
    generation,
    cacheEffects: cacheEffects(controlView.getUint32(44, true)),
    mappingOperations,
    pages,
  };
}

export function encodeCpuWritebackControl(
  generation: bigint,
  pages: DirtyMirrorPage[],
): Uint8Array {
  const headerBytes = 28;
  const descriptorBytes = 8;
  const control = new Uint8Array(headerBytes + pages.length * descriptorBytes);
  encodeCpuWritebackControlInto(control, generation, pages);
  return control;
}

export function cpuWritebackControlByteLength(pageCount: number): number {
  invariant(Number.isSafeInteger(pageCount) && pageCount >= 0, "invalid writeback page count");
  return 28 + pageCount * 8;
}

export function encodeCpuWritebackControlInto(
  control: Uint8Array,
  generation: bigint,
  pages: DirtyMirrorPage[],
): void {
  const headerBytes = 28;
  const descriptorBytes = 8;
  const controlLength = cpuWritebackControlByteLength(pages.length);
  invariant(
    control.byteLength >= controlLength,
    "writeback control destination length mismatch",
  );
  control.fill(0, 0, controlLength);
  control.set(cpuWritebackMagic, 0);
  const controlView = view(control);
  controlView.setUint32(8, pageSize, true);
  controlView.setBigUint64(12, generation, true);
  controlView.setUint32(20, pages.length, true);
  controlView.setUint32(24, descriptorBytes, true);
  pages.forEach((page, index) => {
    invariant(page.bytes.byteLength === pageSize, "writeback page is not 4 KiB");
    const offset = headerBytes + index * descriptorBytes;
    controlView.setUint32(offset, page.address, true);
    controlView.setUint32(offset + 4, page.owner, true);
  });
}

export function encodeCpuWriteback(
  generation: bigint,
  pages: DirtyMirrorPage[],
): CpuWritebackBuffers {
  const control = encodeCpuWritebackControl(generation, pages);
  const payload = new Uint8Array(pages.length * pageSize);
  pages.forEach((page, index) => {
    payload.set(page.bytes, index * pageSize);
  });
  return { control, payload };
}

export const memoryPageSize = pageSize;
