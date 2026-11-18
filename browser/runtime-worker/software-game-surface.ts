import { DEFAULT_TEXTURE_SAMPLING, type TextureSampling, type FragmentState, type GamePresentation, texturePixels, validatePresentation } from "./game-presentation.js";

// Development-only presentation of the same Rust snapshots as the WebGL2 shader.
// This budget includes clear, texture initialization/fills, and candidate fragments,
// even for overlapping draws. No loop is bounded by untrusted offscreen coordinates.
const MAX_PIXEL_WORK = 64 * 1024 * 1024;
interface Vertex {
  x: number; y: number; rhw: number; z: number;
  u: number; v: number; r: number; g: number; b: number; alpha: number;
}
interface Triangle {
  a: Vertex; b: Vertex; c: Vertex;
  left: number; top: number; right: number; bottom: number;
}

function edge(a: Vertex, b: Vertex, x: number, y: number): number {
  return (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x);
}
function inclusive(a: Vertex, b: Vertex): boolean {
  return b.y < a.y || (b.y === a.y && b.x > a.x);
}
function triangles(vertices: Float32Array, width: number, height: number): Triangle[] {
  const points: Vertex[] = [];
  for (let index = 0; index < 40; index += 10) {
    const rhw = vertices[index + 3]!;
    points.push({ x: vertices[index]! + 0.5, y: vertices[index + 1]! + 0.5, rhw, z: vertices[index + 2]!,
      u: vertices[index + 4]! * rhw, v: vertices[index + 5]! * rhw,
      r: vertices[index + 6]! * rhw, g: vertices[index + 7]! * rhw, b: vertices[index + 8]! * rhw, alpha: vertices[index + 9]! * rhw });
  }
  const result: Triangle[] = [];
  // WebGL TRIANGLE_STRIP alternates winding. There is no culling or depth test.
  for (const indices of [[0, 1, 2], [2, 1, 3]]) {
    const a = points[indices[0]!]!;
    let b = points[indices[1]!]!, c = points[indices[2]!]!;
    const area = edge(a, b, c.x, c.y);
    if (area === 0) continue;
    if (area < 0) [b, c] = [c, b];
    const left = Math.max(0, Math.ceil(Math.min(a.x, b.x, c.x) - 0.5));
    const top = Math.max(0, Math.ceil(Math.min(a.y, b.y, c.y) - 0.5));
    const right = Math.min(width, Math.floor(Math.max(a.x, b.x, c.x) - 0.5) + 1);
    const bottom = Math.min(height, Math.floor(Math.max(a.y, b.y, c.y) - 0.5) + 1);
    if (right > left && bottom > top) result.push({ a, b, c, left, top, right, bottom });
  }
  return result;
}

function rasterize(target: Uint8ClampedArray, width: number, triangle: Triangle,
    texture: Uint8Array, textureWidth: number, textureHeight: number, sampling: Readonly<TextureSampling>, depthBuffer: Uint16Array, depth: {write: boolean; compare: number} | null | undefined, fragment: FragmentState | null | undefined): void {
  const { a, b, c, left, top, right, bottom } = triangle;
  const address = (index: number, size: number, mode: "clamp" | "wrap") => mode === "clamp"
    ? Math.max(0, Math.min(size - 1, index)) : ((index % size) + size) % size;
  const derivative = (key: "rhw" | "u" | "v") => [
    (b.y - c.y) * a[key] + (c.y - a.y) * b[key] + (a.y - b.y) * c[key],
    (c.x - b.x) * a[key] + (a.x - c.x) * b[key] + (b.x - a.x) * c[key],
  ];
  const dW = derivative("rhw"), dU = derivative("u"), dV = derivative("v");
  const includeA = inclusive(b, c), includeB = inclusive(c, a), includeC = inclusive(a, b);
  for (let y = top; y < bottom; y++) {
    for (let x = left; x < right; x++) {
      const ea = edge(b, c, x + 0.5, y + 0.5);
      const eb = edge(c, a, x + 0.5, y + 0.5);
      const ec = edge(a, b, x + 0.5, y + 0.5);
      if (ea < 0 || (ea === 0 && !includeA) || eb < 0 || (eb === 0 && !includeB) ||
          ec < 0 || (ec === 0 && !includeC)) continue;
      if (depth) {
        const value = Math.round((ea * a.z + eb * b.z + ec * c.z) / (ea + eb + ec) * 65535);
        const previous = depthBuffer[y * width + x]!;
        const pass = [false, value < previous, value === previous, value <= previous, value > previous, value !== previous, value >= previous, true][depth.compare - 1];
        if (!pass) continue;

      }
      // The shader's clip w is 1/rhw. Area cancels between the perspective
      // numerator and denominator, so use edge weights without normalizing.
      const inverseW = ea * a.rhw + eb * b.rhw + ec * c.rhw;
      const u = (ea * a.u + eb * b.u + ec * c.u) / inverseW;
      const v = (ea * a.v + eb * b.v + ec * c.v) / inverseW;
      let filter = sampling.magFilter;
      if (sampling.minFilter !== sampling.magFilter) {
        const rho = Math.max(...[0, 1].map(axis => Math.hypot(
          (dU[axis]! - u * dW[axis]!) * textureWidth / inverseW,
          (dV[axis]! - v * dW[axis]!) * textureHeight / inverseW)));
        if (rho > 1) filter = sampling.minFilter;
      }
      // Normalize before integer addressing, preserving exact clamp endpoints.
      const tu = sampling.addressU === "clamp" ? Math.max(0, Math.min(1, u)) : u - Math.floor(u);
      const tv = sampling.addressV === "clamp" ? Math.max(0, Math.min(1, v)) : v - Math.floor(v);
      const tx = tu * textureWidth - (filter === "linear" ? 0.5 : 0);
      const ty = tv * textureHeight - (filter === "linear" ? 0.5 : 0);
      const floorX = Math.floor(tx), floorY = Math.floor(ty);
      const fx = filter === "linear" ? tx - floorX : 0, fy = filter === "linear" ? ty - floorY : 0;
      const x0 = address(floorX, textureWidth, sampling.addressU);
      const y0 = address(floorY, textureHeight, sampling.addressV);
      const x1 = address(floorX + 1, textureWidth, sampling.addressU);
      const y1 = address(floorY + 1, textureHeight, sampling.addressV);
      const p00 = (y0 * textureWidth + x0) * 4, p10 = (y0 * textureWidth + x1) * 4;
      const p01 = (y1 * textureWidth + x0) * 4, p11 = (y1 * textureWidth + x1) * 4;
      const offset = (y * width + x) * 4;
      const sample = (channel: number) => {
        const upper = texture[p00 + channel]! * (1 - fx) + texture[p10 + channel]! * fx;
        const lower = texture[p01 + channel]! * (1 - fx) + texture[p11 + channel]! * fx;
        return upper * (1 - fy) + lower * fy;
      };
      let alpha = fragment?.textureAlpha ? sample(3) / 255 : 1;
      if (fragment?.diffuseAlpha) alpha *= (ea * a.alpha + eb * b.alpha + ec * c.alpha) / inverseW;
      alpha = Math.max(0, Math.min(1, alpha));
      if (fragment?.alphaTest) {
        const value = Math.round(alpha * 255), ref = fragment.alphaTest.reference;
        if (![false,value<ref,value===ref,value<=ref,value>ref,value!==ref,value>=ref,true][fragment.alphaTest.compare-1]) continue;
      }
      if (depth?.write) depthBuffer[y * width + x] = Math.round((ea * a.z + eb * b.z + ec * c.z) / (ea + eb + ec) * 65535);
      const fog = fragment?.fog ?? null;
      let fogFactor = 1;
      if (fog !== null) {
        const eyeW = (ea + eb + ec) / inverseW;
        const denom = fog.end - fog.start;
        fogFactor = Math.abs(denom) > 1e-6 ? Math.max(0, Math.min(1, (fog.end - eyeW) / denom)) : (eyeW <= fog.end ? 1 : 0);
      }
      for (let channel = 0; channel < 3; channel++) {
        const tint = channel === 0 ? (ea * a.r + eb * b.r + ec * c.r) / inverseW :
          channel === 1 ? (ea * a.g + eb * b.g + ec * c.g) / inverseW :
          (ea * a.b + eb * b.b + ec * c.b) / inverseW;
        let color = fragment?.colorAdd ? Math.min(255, sample(channel) + tint * 255) : sample(channel) * tint;
        if (fog !== null) {
          const fogCh = channel === 0 ? ((fog.colorArgb >>> 16) & 255) : channel === 1 ? ((fog.colorArgb >>> 8) & 255) : (fog.colorArgb & 255);
          color = fogCh * (1 - fogFactor) + color * fogFactor;
        }
        target[offset + channel] = fragment?.blend
          ? (fragment.additive ? Math.min(255, color * alpha + target[offset + channel]!) : color * alpha + target[offset + channel]! * (1 - alpha))
          : color;
      }
      // X8R8G8B8 sampling is always opaque. The shader ignores diffuse alpha.
      target[offset + 3] = 255;
    }
  }
}

export class SoftwareGameSurface {
  readonly #canvas: OffscreenCanvas;
  readonly #context: OffscreenCanvasRenderingContext2D;
  #lastImage: ImageData | null = null;
  #lastDepth: Uint16Array | null = null;

  constructor(canvas: OffscreenCanvas) {
    const context = canvas.getContext("2d", { alpha: false });
    if (context === null) throw new Error("Canvas2D is unavailable for the development game surface");
    this.#canvas = canvas;
    this.#context = context;
  }

  present(frame: GamePresentation, payload: Uint8Array): void {
    const vertices = validatePresentation(frame, payload, 4096);
    const color = frame.colorArgb;
    const colorClearAt = color === null ? -1 : frame.colorClearBeforeDraw ?? 0;
    let work = frame.width * frame.height * (1 + (frame.depthClears?.length ?? 0) + Number(colorClearAt > 0));
    if (work > MAX_PIXEL_WORK) throw new Error("Presentation exceeds the software raster budget");
    const seenSoftwareSnapshots = new Set<string>();
    const geometry = frame.draws.map((draw, index) => {
      const texture = draw.texture;
      const snapKey = texture.pixels !== null && texture.fills.length === 0
        ? `${texture.pixels.offset}:${texture.pixels.length}:${texture.pixels.pitch}:${texture.width}:${texture.height}:${Boolean(texture.alpha)}`
        : null;
      if (snapKey === null || !seenSoftwareSnapshots.has(snapKey)) {
        if (snapKey !== null) seenSoftwareSnapshots.add(snapKey);
        work += texture.width * texture.height;
      }
      for (const fill of texture.fills) work += (fill.right - fill.left) * (fill.bottom - fill.top);
      const result = triangles(vertices[index]!, frame.width, frame.height);
      if (draw.clip) for (const triangle of result) {
        triangle.left = Math.max(triangle.left, draw.clip.left);
        triangle.top = Math.max(triangle.top, draw.clip.top);
        triangle.right = Math.max(triangle.left, Math.min(triangle.right, draw.clip.right));
        triangle.bottom = Math.max(triangle.top, Math.min(triangle.bottom, draw.clip.bottom));
      }
      for (const triangle of result) work += (triangle.right - triangle.left) * (triangle.bottom - triangle.top);
      if (work > MAX_PIXEL_WORK) throw new Error("Presentation exceeds the software raster budget");
      return result;
    });
    // Finish the entire CPU frame off-surface. Validation/allocation failures
    // must not resize, clear, or partially publish the previous presentation.
    const image = this.#context.createImageData(frame.width, frame.height);
    const depthBuffer = new Uint16Array(frame.width * frame.height);
    const clearColor = (background: number) => {
      const red = background >>> 16 & 255, green = background >>> 8 & 255, blue = background & 255;
      for (let offset = 0; offset < image.data.length; offset += 4) {
        image.data[offset] = red; image.data[offset + 1] = green; image.data[offset + 2] = blue;
        image.data[offset + 3] = 255;
      }
    };
    if (frame.preserve && this.#lastImage !== null && this.#lastDepth !== null &&
        this.#lastImage.width === frame.width && this.#lastImage.height === frame.height) {
      if (colorClearAt !== 0) image.data.set(this.#lastImage.data);
      depthBuffer.set(this.#lastDepth);
    } else {
      depthBuffer.fill(65535);
      if (colorClearAt !== 0) clearColor(0);
    }
    let depthClearIndex = 0;
    const decodedTextures = new Map<string, Uint8Array>();
    for (let index = 0; index <= frame.draws.length; index++) {
      if (color !== null && colorClearAt === index) clearColor(color);
      while (frame.depthClears?.[depthClearIndex]?.beforeDraw === index) depthBuffer.fill(Math.round(frame.depthClears[depthClearIndex++]!.value * 65535));
      if (index === frame.draws.length) break;
      const source = frame.draws[index]!.texture;
      const snapKey = source.pixels !== null && source.fills.length === 0
        ? `${source.pixels.offset}:${source.pixels.length}:${source.pixels.pitch}:${source.width}:${source.height}:${Boolean(source.alpha)}`
        : null;
      let pixels = snapKey !== null ? decodedTextures.get(snapKey) : undefined;
      if (pixels === undefined) {
        pixels = texturePixels(source, payload);
        if (snapKey !== null) decodedTextures.set(snapKey, pixels);
      }
      for (const triangle of geometry[index]!) rasterize(image.data, frame.width, triangle, pixels, source.width, source.height,
        frame.draws[index]!.sampling ?? DEFAULT_TEXTURE_SAMPLING, depthBuffer, frame.draws[index]!.depth, frame.draws[index]!.fragment);
    }
    if (this.#canvas.width !== frame.width) this.#canvas.width = frame.width;
    if (this.#canvas.height !== frame.height) this.#canvas.height = frame.height;
    this.#lastImage = image;
    this.#lastDepth = depthBuffer;
    this.#context.putImageData(image, 0, 0);
  }
}
