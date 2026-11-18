export interface TextureFill {
  left: number; top: number; right: number; bottom: number; colorRef: number;
}
export interface FogState {
  colorArgb: number; start: number; end: number;
}
export interface FragmentState {
  textureAlpha: boolean; diffuseAlpha: boolean; blend: boolean;
  additive?: boolean; colorAdd?: boolean;
  alphaTest: {compare: number; reference: number} | null;
  fog?: FogState | null;
}
export interface QuadPresentation {
  clip?: {left: number; top: number; right: number; bottom: number} | null;
  fragment?: FragmentState | null;
  depth?: { write: boolean; compare: number } | null;
  vertices: number[];
  sampling?: TextureSampling;
  texture: { alpha?: boolean; width: number; height: number; fills: TextureFill[];
    pixels: { offset: number; length: number; pitch: number } | null };
}
export interface TextureSampling {
  minFilter: "point" | "linear"; magFilter: "point" | "linear";
  addressU: "wrap" | "clamp"; addressV: "wrap" | "clamp";
}
export const DEFAULT_TEXTURE_SAMPLING: Readonly<TextureSampling> = Object.freeze({
  minFilter: "linear", magFilter: "linear", addressU: "wrap", addressV: "wrap",
});
export interface GamePresentation {
  preserve?: boolean;
  depthClears?: {beforeDraw: number; value: number}[];
  width: number;
  height: number;
  colorArgb: number | null;
  colorClearBeforeDraw?: number;
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
      (frame.preserve !== undefined && typeof frame.preserve !== "boolean") ||
      (frame.colorArgb !== null && !color(frame.colorArgb)) ||
      !Array.isArray(frame.draws) || frame.draws.length > 4096 ||
      !(payload instanceof Uint8Array) || payload.length > 16 * 1024 * 1024) {
    throw new Error("Invalid game presentation");
  }
  if (frame.width > maximum || frame.height > maximum) throw new Error("Game surface exceeds the renderer size limit");
  const colorClearAt = frame.colorClearBeforeDraw ?? 0;
  if ((frame.colorClearBeforeDraw !== undefined && !Number.isInteger(frame.colorClearBeforeDraw)) ||
      colorClearAt < 0 || colorClearAt > frame.draws.length || (frame.colorArgb === null && colorClearAt !== 0)) {
    throw new Error("Invalid presentation color clear position");
  }
  if (frame.depthClears !== undefined && (!Array.isArray(frame.depthClears) || frame.depthClears.length > 4096 || frame.depthClears.some((clear, index, all) => clear === null || typeof clear !== "object" || !Number.isInteger(clear.beforeDraw) || clear.beforeDraw < 0 || clear.beforeDraw > frame.draws.length || index > 0 && clear.beforeDraw < all[index - 1]!.beforeDraw || !Number.isFinite(clear.value) || clear.value < 0 || clear.value > 1))) throw new Error("Invalid presentation depth clears");
  let byteCount = 0, fillCount = 0;
  const seenSnapshots = new Set<string>();
  // Validate the complete boundary before changing the surface or allocating texture pixels.
  return Array.from(frame.draws, draw => {
    if (draw === null || typeof draw !== "object") throw new Error("Invalid presentation draw");
    const clip = draw.clip;
    if (clip !== undefined && clip !== null && (typeof clip !== "object" ||
        ![clip.left, clip.top, clip.right, clip.bottom].every(Number.isInteger) ||
        clip.left < 0 || clip.top < 0 || clip.right <= clip.left || clip.bottom <= clip.top ||
        clip.right > frame.width || clip.bottom > frame.height)) throw new Error("Invalid presentation clip");
    if (draw.depth !== undefined && draw.depth !== null && (typeof draw.depth !== "object" || typeof draw.depth.write !== "boolean" || !Number.isInteger(draw.depth.compare) || draw.depth.compare < 1 || draw.depth.compare > 8)) throw new Error("Invalid presentation depth state");
    const fragment = draw.fragment;
    if (fragment !== undefined && fragment !== null &&
        (typeof fragment !== "object" || typeof fragment.textureAlpha !== "boolean" ||
         typeof fragment.diffuseAlpha !== "boolean" || typeof fragment.blend !== "boolean" ||
         (fragment.additive !== undefined && typeof fragment.additive !== "boolean") ||
         (fragment.colorAdd !== undefined && typeof fragment.colorAdd !== "boolean") ||
         fragment.alphaTest !== null && (typeof fragment.alphaTest !== "object" ||
           !Number.isInteger(fragment.alphaTest.compare) || fragment.alphaTest.compare < 1 || fragment.alphaTest.compare > 8 ||
           !Number.isInteger(fragment.alphaTest.reference) || fragment.alphaTest.reference < 0 || fragment.alphaTest.reference > 255) ||
         (fragment.fog !== undefined && fragment.fog !== null && (typeof fragment.fog !== "object" ||
           !color(fragment.fog.colorArgb) || !Number.isFinite(fragment.fog.start) || !Number.isFinite(fragment.fog.end))))) {
      throw new Error("Invalid presentation fragment state");
    }
    const sampling = draw.sampling;
    if (sampling !== undefined && (sampling === null || typeof sampling !== "object" ||
        !["point", "linear"].includes(sampling.minFilter) || !["point", "linear"].includes(sampling.magFilter) ||
        !["wrap", "clamp"].includes(sampling.addressU) || !["wrap", "clamp"].includes(sampling.addressV))) {
      throw new Error("Invalid presentation texture sampling");
    }
    const texture = draw.texture;
    if (texture === null || typeof texture !== "object" || !dimension(texture.width) || !dimension(texture.height) ||
        texture.width > maximum || texture.height > maximum || (texture.alpha !== undefined && typeof texture.alpha !== "boolean") || !Array.isArray(texture.fills)) {
      throw new Error("Invalid presentation texture");
    }
    if (texture.pixels !== null) {
      const pixels = texture.pixels;
      if (pixels === undefined || !Number.isSafeInteger(pixels.offset) || pixels.offset < 0 ||
          !Number.isSafeInteger(pixels.length) || pixels.length < 1 ||
          !Number.isSafeInteger(pixels.pitch) || pixels.pitch < texture.width * 4 ||
          pixels.length !== pixels.pitch * texture.height ||
          pixels.offset > payload.length || pixels.length > payload.length - pixels.offset) {
        throw new Error("Invalid presentation pixel range");
      }
      const key = texture.fills.length === 0
        ? `${pixels.offset}:${pixels.length}:${pixels.pitch}:${texture.width}:${texture.height}:${Boolean(texture.alpha)}`
        : null;
      if (key === null || !seenSnapshots.has(key)) {
        if (key !== null) seenSnapshots.add(key);
        byteCount += texture.width * texture.height * 4;
      }
    } else {
      byteCount += texture.width * texture.height * 4;
    }
    fillCount += texture.fills.length;
    if (byteCount > 16 * 1024 * 1024 || fillCount > 16384) throw new Error("Presentation exceeds the upload budget");
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

const littleEndian = new Uint8Array(new Uint32Array([1]).buffer)[0] === 1;

export function texturePixels(source: QuadPresentation["texture"], payload: Uint8Array): Uint8Array {
  const pixels = new Uint8Array(source.width * source.height * 4);
  const snapshot = source.pixels;
  // Both renderers validate the full snapshot before allocating or decoding pixels.
  if (littleEndian && snapshot !== null && snapshot.pitch % 4 === 0 &&
      (payload.byteOffset + snapshot.offset) % 4 === 0) {
    const input = new Uint32Array(payload.buffer, payload.byteOffset + snapshot.offset, snapshot.length / 4);
    const output = new Uint32Array(pixels.buffer);
    const stride = snapshot.pitch / 4;
    let target = 0;
    for (let y = 0; y < source.height; y++) {
      let from = y * stride;
      for (let x = 0; x < source.width; x++) {
        const word = input[from++]!;
        output[target++] = (source.alpha ? word & 0xff000000 : 0xff000000) | (word & 0xff00) | ((word & 255) << 16) | ((word >>> 16) & 255);
      }
    }
  } else {
    // X8R8G8B8 is opaque, including undefined texels and byte-aligned snapshots.
    for (let offset = 3; offset < pixels.length; offset += 4) pixels[offset] = 255;
    if (snapshot !== null) {
      for (let y = 0; y < source.height; y++) {
        for (let x = 0; x < source.width; x++) {
          const input = snapshot.offset + y * snapshot.pitch + x * 4;
          const output = (y * source.width + x) * 4;
          pixels[output] = payload[input + 2]!;
          pixels[output + 1] = payload[input + 1]!;
          pixels[output + 2] = payload[input]!;
          pixels[output + 3] = source.alpha ? payload[input + 3]! : 255;
        }
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
        pixels[offset + 3] = 255;
      }
    }
  }
  return pixels;
}
