import { type GamePresentation, texturePixels, validatePresentation } from "./game-presentation.js";

// Development-only presentation of the same Rust snapshots as the WebGL2 shader.
// This budget includes clear, texture initialization/fills, and candidate fragments,
// even for overlapping draws. No loop is bounded by untrusted offscreen coordinates.
const MAX_PIXEL_WORK = 64 * 1024 * 1024;
interface Vertex {
  x: number; y: number; rhw: number;
  u: number; v: number; r: number; g: number; b: number;
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
    points.push({ x: vertices[index]! + 0.5, y: vertices[index + 1]! + 0.5, rhw,
      u: vertices[index + 4]! * rhw, v: vertices[index + 5]! * rhw,
      r: vertices[index + 6]! * rhw, g: vertices[index + 7]! * rhw, b: vertices[index + 8]! * rhw });
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
    texture: Uint8Array, textureWidth: number, textureHeight: number): void {
  const { a, b, c, left, top, right, bottom } = triangle;
  const includeA = inclusive(b, c), includeB = inclusive(c, a), includeC = inclusive(a, b);
  for (let y = top; y < bottom; y++) {
    for (let x = left; x < right; x++) {
      const ea = edge(b, c, x + 0.5, y + 0.5);
      const eb = edge(c, a, x + 0.5, y + 0.5);
      const ec = edge(a, b, x + 0.5, y + 0.5);
      if (ea < 0 || (ea === 0 && !includeA) || eb < 0 || (eb === 0 && !includeB) ||
          ec < 0 || (ec === 0 && !includeC)) continue;
      // The shader's clip w is 1/rhw. Area cancels between the perspective
      // numerator and denominator, so use edge weights without normalizing.
      const inverseW = ea * a.rhw + eb * b.rhw + ec * c.rhw;
      const u = (ea * a.u + eb * b.u + ec * c.u) / inverseW;
      const v = (ea * a.v + eb * b.v + ec * c.v) / inverseW;
      // GL_LINEAR + GL_REPEAT: texel centers are at (n + 0.5) / size.
      const tx = (u - Math.floor(u)) * textureWidth - 0.5;
      const ty = (v - Math.floor(v)) * textureHeight - 0.5;
      const floorX = Math.floor(tx), floorY = Math.floor(ty);
      const fx = tx - floorX, fy = ty - floorY;
      const x0 = floorX < 0 ? textureWidth - 1 : floorX;
      const y0 = floorY < 0 ? textureHeight - 1 : floorY;
      const x1 = x0 + 1 === textureWidth ? 0 : x0 + 1;
      const y1 = y0 + 1 === textureHeight ? 0 : y0 + 1;
      const p00 = (y0 * textureWidth + x0) * 4, p10 = (y0 * textureWidth + x1) * 4;
      const p01 = (y1 * textureWidth + x0) * 4, p11 = (y1 * textureWidth + x1) * 4;
      const offset = (y * width + x) * 4;
      for (let channel = 0; channel < 3; channel++) {
        const tint = channel === 0 ? (ea * a.r + eb * b.r + ec * c.r) / inverseW :
          channel === 1 ? (ea * a.g + eb * b.g + ec * c.g) / inverseW :
          (ea * a.b + eb * b.b + ec * c.b) / inverseW;
        const upper = texture[p00 + channel]! * (1 - fx) + texture[p10 + channel]! * fx;
        const lower = texture[p01 + channel]! * (1 - fx) + texture[p11 + channel]! * fx;
        target[offset + channel] = (upper * (1 - fy) + lower * fy) * tint;
      }
      // X8R8G8B8 sampling is always opaque. The shader ignores diffuse alpha.
      target[offset + 3] = 255;
    }
  }
}

export class SoftwareGameSurface {
  readonly #canvas: OffscreenCanvas;
  readonly #context: OffscreenCanvasRenderingContext2D;

  constructor(canvas: OffscreenCanvas) {
    const context = canvas.getContext("2d", { alpha: false });
    if (context === null) throw new Error("Canvas2D is unavailable for the development game surface");
    this.#canvas = canvas;
    this.#context = context;
  }

  present(frame: GamePresentation, payload: Uint8Array): void {
    const vertices = validatePresentation(frame, payload, 4096);
    let work = frame.width * frame.height;
    const geometry = frame.draws.map((draw, index) => {
      const texture = draw.texture;
      work += texture.width * texture.height;
      for (const fill of texture.fills) work += (fill.right - fill.left) * (fill.bottom - fill.top);
      const result = triangles(vertices[index]!, frame.width, frame.height);
      for (const triangle of result) work += (triangle.right - triangle.left) * (triangle.bottom - triangle.top);
      if (work > MAX_PIXEL_WORK) throw new Error("Presentation exceeds the software raster budget");
      return result;
    });
    // Finish the entire CPU frame off-surface. Validation/allocation failures
    // must not resize, clear, or partially publish the previous presentation.
    const image = this.#context.createImageData(frame.width, frame.height);
    const background = frame.colorArgb ?? 0xff000000;
    const red = background >>> 16 & 255, green = background >>> 8 & 255, blue = background & 255;
    for (let offset = 0; offset < image.data.length; offset += 4) {
      image.data[offset] = red; image.data[offset + 1] = green; image.data[offset + 2] = blue;
      image.data[offset + 3] = 255;
    }
    for (let index = 0; index < frame.draws.length; index++) {
      const source = frame.draws[index]!.texture;
      const pixels = texturePixels(source, payload);
      for (const triangle of geometry[index]!) rasterize(image.data, frame.width, triangle, pixels, source.width, source.height);
    }
    if (this.#canvas.width !== frame.width) this.#canvas.width = frame.width;
    if (this.#canvas.height !== frame.height) this.#canvas.height = frame.height;
    this.#context.putImageData(image, 0, 0);
  }
}
