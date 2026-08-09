const u32Modulus = 0x1_0000_0000;
const minimumI32 = -0x8000_0000;
const maximumU32 = 0xffff_ffff;

export function createMonotonicV86InstructionCounter(readRawCounter) {
  let previousRaw = null;
  let wrapBase = 0;

  return () => {
    const exported = readRawCounter();
    if (
      !Number.isInteger(exported) ||
      exported < minimumI32 ||
      exported > maximumU32
    ) {
      throw new Error("v86 instruction counter export is outside the i32/u32 range");
    }
    const raw = exported >>> 0;
    if (previousRaw !== null && raw < previousRaw) {
      wrapBase += u32Modulus;
    }
    previousRaw = raw;
    const count = wrapBase + raw;
    if (!Number.isSafeInteger(count)) {
      throw new Error("v86 instruction counter exceeds the JavaScript safe integer range");
    }
    return count;
  };
}
