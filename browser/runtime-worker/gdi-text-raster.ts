/// <reference lib="webworker" />

const requestWordCount = 18;
const maximumPixelBytes = 2 * 1024 * 1024;
const windows932 = new TextDecoder("shift_jis", { fatal: true });

interface TextRasterRequest {
  fontHeight: number;
  fontWidth: number;
  fontWeight: number;
  dibWidth: number;
  dibHeight: number;
  strideBytes: number;
  pixelByteLength: number;
  characterCount: number;
  packedText: number;
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function signed(word: number): number {
  return word | 0;
}

function decodeRequest(arguments_: readonly number[]): TextRasterRequest {
  invariant(
    arguments_.length === requestWordCount &&
      arguments_.every((word) => Number.isInteger(word) && word >= 0 && word <= 0xffffffff),
    "GDI text raster request has invalid words or size",
  );
  const request: TextRasterRequest = {
    fontHeight: signed(arguments_[9]!),
    fontWidth: signed(arguments_[10]!),
    fontWeight: signed(arguments_[11]!),
    dibWidth: signed(arguments_[12]!),
    dibHeight: signed(arguments_[13]!),
    strideBytes: arguments_[14]!,
    pixelByteLength: arguments_[15]!,
    characterCount: arguments_[16]!,
    packedText: arguments_[17]!,
  };
  const height = Math.abs(request.dibHeight);
  invariant(
    request.characterCount === 1 || request.characterCount === 2,
    "GDI text raster request has an unsupported Windows-932 glyph length",
  );
  invariant(
    request.fontHeight !== 0 && Math.abs(request.fontHeight) <= 4096 &&
      request.fontWidth > 0 && request.fontWidth <= 4096 &&
      request.fontWeight >= 0 && request.fontWeight <= 1000 &&
      request.dibWidth > 0 && request.dibWidth <= 4096 &&
      request.dibHeight < 0 && height <= 4096 &&
      request.strideBytes === Math.ceil(request.dibWidth / 4) * 4 &&
      request.pixelByteLength === request.strideBytes * height &&
      request.pixelByteLength <= maximumPixelBytes,
    "GDI text raster request has an invalid bounded 8bpp pixel plane",
  );
  return request;
}

function decodeGlyph(request: TextRasterRequest): string {
  const packed = new Uint8Array(4);
  new DataView(packed.buffer).setUint32(0, request.packedText, true);
  invariant(
    packed.subarray(request.characterCount).every((byte) => byte === 0),
    "GDI text raster request has nonzero bytes beyond the captured glyph",
  );
  const glyph = windows932.decode(packed.subarray(0, request.characterCount));
  invariant(
    Array.from(glyph).length === 1,
    "GDI text raster request did not decode to exactly one character",
  );
  return glyph;
}

export function rasterizeGdiText(arguments_: readonly number[]): Uint8Array {
  const request = decodeRequest(arguments_);
  const glyph = decodeGlyph(request);
  const height = Math.abs(request.dibHeight);
  const canvas = new OffscreenCanvas(request.dibWidth, height);
  const context = canvas.getContext("2d", {
    alpha: false,
    willReadFrequently: true,
  });
  invariant(context !== null, "Chrome did not provide an OffscreenCanvas 2D context");
  context.fillStyle = "black";
  context.fillRect(0, 0, request.dibWidth, height);
  context.fillStyle = "white";
  context.textBaseline = "top";
  context.textAlign = "left";
  context.font = `400 ${Math.abs(request.fontHeight)}px "MS Gothic", monospace`;

  const measuredWidth = context.measureText(glyph).width;
  const requestedWidth = Math.abs(request.fontWidth) * request.characterCount;
  invariant(
    Number.isFinite(measuredWidth) && measuredWidth > 0 && requestedWidth > 0,
    "Chrome could not measure the reached GDI glyph",
  );
  context.save();
  context.scale(requestedWidth / measuredWidth, 1);
  context.fillText(glyph, 0, 0);
  context.restore();

  const rgba = context.getImageData(0, 0, request.dibWidth, height).data;
  const pixels = new Uint8Array(request.pixelByteLength);
  for (let row = 0; row < height; row += 1) {
    for (let column = 0; column < request.dibWidth; column += 1) {
      pixels[row * request.strideBytes + column] = rgba[(row * request.dibWidth + column) * 4]!;
    }
  }
  return pixels;
}
