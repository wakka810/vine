/// <reference lib="webworker" />

const requestVersion = 1;
const requestWordCount = 21;
const maximumPixelBytes = 448 * 336;
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
  initialPixel: number;
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
    arguments_.length === requestWordCount && arguments_[0] === requestVersion,
    "GDI text raster request version or size mismatch",
  );
  const request: TextRasterRequest = {
    fontHeight: signed(arguments_[10]!),
    fontWidth: signed(arguments_[11]!),
    fontWeight: signed(arguments_[12]!),
    dibWidth: signed(arguments_[13]!),
    dibHeight: signed(arguments_[14]!),
    strideBytes: arguments_[15]!,
    pixelByteLength: arguments_[16]!,
    characterCount: arguments_[17]!,
    packedText: arguments_[18]!,
    initialPixel: arguments_[19]!,
  };
  const height = Math.abs(request.dibHeight);
  invariant(
    request.characterCount === 1 || request.characterCount === 2,
    "GDI text raster request has an unsupported Windows-932 glyph length",
  );
  invariant(
    request.initialPixel === 0 &&
      request.fontWeight === 100 &&
      ((request.fontHeight === 64 &&
        request.fontWidth === 32 &&
        request.dibWidth === 128 &&
        request.dibHeight === -96 &&
        request.strideBytes === 128 &&
        request.pixelByteLength === 12_288) ||
        (request.fontHeight === 224 &&
          request.fontWidth === 112 &&
          request.dibWidth === 448 &&
          request.dibHeight === -336 &&
          request.strideBytes === 448 &&
          request.pixelByteLength === 150_528) ||
        (request.fontHeight === 80 &&
          request.fontWidth === 40 &&
          request.dibWidth === 160 &&
          request.dibHeight === -120 &&
          request.strideBytes === 160 &&
          request.pixelByteLength === 19_200)),
    "GDI text raster request is outside the reached font/DIB profile pairs",
  );
  invariant(
    height > 0 &&
      request.strideBytes === request.dibWidth &&
      request.pixelByteLength === request.strideBytes * height &&
      request.pixelByteLength <= maximumPixelBytes,
    "GDI text raster request has an invalid bounded pixel plane",
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
  context.font = `${request.fontWeight} ${Math.abs(request.fontHeight)}px "MS Gothic", monospace`;

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
  for (let index = 0; index < pixels.length; index += 1) {
    pixels[index] = rgba[index * 4]!;
  }
  return pixels;
}
