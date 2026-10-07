export interface TextureFill {
  left: number; top: number; right: number; bottom: number; colorRef: number;
}
export interface QuadPresentation {
  vertices: number[];
  texture: { width: number; height: number; fills: TextureFill[];
    pixels: { offset: number; length: number; pitch: number } | null };
}
export interface GamePresentation {
  width: number;
  height: number;
  colorArgb: number | null;
  draws: QuadPresentation[];
}

function dimension(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= 4096;
}
function color(value: number): boolean {
  return Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff;
}

// Decode the complete Rust snapshot before either renderer changes browser resources.
export function validatePresentation(frame: GamePresentation, payload: Uint8Array, maximum: number): Float32Array[] {
  if (frame === null || typeof frame !== "object" || !dimension(frame.width) || !dimension(frame.height) ||
      (frame.colorArgb !== null && !color(frame.colorArgb)) ||
      !Array.isArray(frame.draws) || frame.draws.length > 4096 ||
      !(payload instanceof Uint8Array) || payload.length > 16 * 1024 * 1024) {
    throw new Error("Invalid game presentation");
  }
  if (frame.width > maximum || frame.height > maximum) throw new Error("Game surface exceeds the renderer size limit");
  let byteCount = 0, fillCount = 0;
  // Validate the complete boundary before changing the surface or allocating texture pixels.
  return Array.from(frame.draws, draw => {
    if (draw === null || typeof draw !== "object") throw new Error("Invalid presentation draw");
    const texture = draw.texture;
    if (texture === null || typeof texture !== "object" || !dimension(texture.width) || !dimension(texture.height) ||
        texture.width > maximum || texture.height > maximum || !Array.isArray(texture.fills)) {
      throw new Error("Invalid presentation texture");
    }
    byteCount += texture.width * texture.height * 4;
    fillCount += texture.fills.length;
    if (byteCount > 16 * 1024 * 1024 || fillCount > 16384) throw new Error("Presentation exceeds the upload budget");
    if (texture.pixels !== null) {
      const pixels = texture.pixels;
      if (pixels === undefined || !Number.isSafeInteger(pixels.offset) || pixels.offset < 0 ||
          !Number.isSafeInteger(pixels.length) || pixels.length < 1 ||
          !Number.isSafeInteger(pixels.pitch) || pixels.pitch < texture.width * 4 ||
          pixels.length !== pixels.pitch * texture.height ||
          pixels.offset > payload.length || pixels.length > payload.length - pixels.offset) {
        throw new Error("Invalid presentation pixel range");
      }
    }
    for (const fill of texture.fills) {
      if (fill === null || typeof fill !== "object" || ![fill.left, fill.top, fill.right, fill.bottom].every(Number.isInteger) ||
          fill.left < 0 || fill.top < 0 || fill.right <= fill.left || fill.bottom <= fill.top ||
          fill.right > texture.width || fill.bottom > texture.height || !color(fill.colorRef)) {
        throw new Error("Invalid presentation texture fill");
      }
    }
    if (!Array.isArray(draw.vertices) || draw.vertices.length !== 112 ||
        Array.from(draw.vertices).some(value => !Number.isInteger(value) || value < 0 || value > 255)) {
      throw new Error("Invalid presentation quad bytes");
    }
    const bytes = Uint8Array.from(draw.vertices);
    const view = new DataView(bytes.buffer);
    const result = new Float32Array(40);
    for (let vertex = 0; vertex < 4; vertex++) {
      const base = vertex * 28, out = vertex * 10;
      for (const [offset, index] of [[0, 0], [4, 1], [8, 2], [12, 3], [20, 4], [24, 5]]) {
        const value = view.getFloat32(base + offset!, true);
        if (!Number.isFinite(value)) throw new Error("Non-finite presentation vertex");
        result[out + index!] = value;
      }
      const w = 1 / result[out + 3]!;
      if (result[out + 2]! < 0 || result[out + 2]! > 1 || result[out + 3]! <= 0 ||
          !Number.isFinite(w) || w > 3.402823466e38 ||
          Math.abs(((result[out]! + 0.5) / frame.width * 2 - 1) * w) > 3.402823466e38 ||
          Math.abs(((result[out + 1]! + 0.5) / frame.height * 2 - 1) * w) > 3.402823466e38) {
        throw new Error("Invalid transformed presentation vertex");
      }
      const diffuse = view.getUint32(base + 16, true);
      result.set([(diffuse >>> 16 & 255) / 255, (diffuse >>> 8 & 255) / 255,
        (diffuse & 255) / 255, (diffuse >>> 24) / 255], out + 6);
    }
    return result;
  });
}

export function texturePixels(source: QuadPresentation["texture"], payload: Uint8Array): Uint8Array {
  const pixels = new Uint8Array(source.width * source.height * 4);
  // X8R8G8B8 has an opaque alpha channel, including previously undefined texels.
  for (let offset = 3; offset < pixels.length; offset += 4) pixels[offset] = 255;
  if (source.pixels !== null) {
    const snapshot = source.pixels;
    for (let y = 0; y < source.height; y++) {
      for (let x = 0; x < source.width; x++) {
        const input = snapshot.offset + y * snapshot.pitch + x * 4;
        const output = (y * source.width + x) * 4;
        pixels[output] = payload[input + 2]!;
        pixels[output + 1] = payload[input + 1]!;
        pixels[output + 2] = payload[input]!;
      }
    }
  }
  for (const fill of source.fills) {
    for (let y = fill.top; y < fill.bottom; y++) {
      for (let x = fill.left; x < fill.right; x++) {
        const offset = (y * source.width + x) * 4;
        pixels[offset] = fill.colorRef & 255;
        pixels[offset + 1] = fill.colorRef >>> 8 & 255;
        pixels[offset + 2] = fill.colorRef >>> 16 & 255;
      }
    }
  }
  return pixels;
}
