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
  if (arguments_[0] === 0x32545854) return rasterizePositionedText(arguments_);
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

function rasterizePositionedText(words: readonly number[]): Uint8Array {
  invariant(words.length >= 14 && words.every(word => Number.isInteger(word) && word >= 0 && word <= 0xffffffff), "Invalid positioned GDI raster request");
  const [width, height, fontHeight, weight, italic, underline, strikeout, faceLength, count] = words.slice(1, 10) as number[];
  invariant(width! > 0 && width! <= 32768 && height! > 0 && height! <= 32768 && width! * height! <= maximumPixelBytes && fontHeight! > 0 && fontHeight! <= 4096 && weight! <= 1000 && italic! <= 1 && underline! <= 1 && strikeout! <= 1 && faceLength! <= 32 && count! <= 8192 && words.length === 14 + faceLength! + count! * 5, "Positioned GDI raster exceeds its bounds");
  const faceUnits = words.slice(14, 14 + faceLength!);
  invariant(faceUnits.every(unit => unit <= 0xffff), "Invalid GDI font face");
  const face = String.fromCharCode(...faceUnits);
  const canvas = new OffscreenCanvas(width!, height!);
  const context = canvas.getContext("2d", {alpha: false, willReadFrequently: true});
  invariant(context !== null, "Chrome did not provide a text canvas");
  context.fillStyle = "black"; context.fillRect(0, 0, width!, height!);
  context.beginPath(); context.rect(signed(words[10]!), signed(words[11]!), Math.max(0, signed(words[12]!) - signed(words[10]!)), Math.max(0, signed(words[13]!) - signed(words[11]!))); context.clip();
  context.fillStyle = "white"; context.textBaseline = "top";
  context.font = `${italic ? "italic " : ""}${weight || 400} ${fontHeight}px ${JSON.stringify(face || "MS Gothic")}, monospace`;
  for (let index = 14 + faceLength!; index < words.length; index += 5) {
    const scalar = words[index]!;
    const x = signed(words[index + 1]!); const y = signed(words[index + 2]!); const advance = words[index + 3]!;
    invariant(scalar <= 0x10ffff && !(scalar >= 0xd800 && scalar <= 0xdfff) && advance > 0 && advance <= 8192 && words[index + 4]! <= 1, "Invalid positioned GDI glyph");
    const glyph = String.fromCodePoint(scalar);
    const measured = context.measureText(glyph).width;
    invariant(Number.isFinite(measured) && measured >= 0, "Chrome could not measure a GDI glyph");
    if (measured > 0) {context.save(); context.translate(x, y); context.scale(advance / measured, 1); context.fillText(glyph, 0, 0); context.restore();}
    if (underline || words[index + 4]) context.fillRect(x, y + fontHeight! - 1, advance, 1);
    if (strikeout) context.fillRect(x, y + Math.floor(fontHeight! / 2), advance, 1);
  }
  const rgba = context.getImageData(0, 0, width!, height!).data;
  const result = new Uint8Array(width! * height!);
  for (let index = 0; index < result.length; index += 1) result[index] = rgba[index * 4]!;
  return result;
}
