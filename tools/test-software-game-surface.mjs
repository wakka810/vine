import assert from "node:assert/strict";
import { build } from "esbuild";

const { outputFiles } = await build({ entryPoints: ["browser/runtime-worker/game-surface.ts"],
  bundle: true, write: false, platform: "node", format: "esm" });
const { GameSurface } = await import(`data:text/javascript;base64,${Buffer.from(outputFiles[0].contents).toString("base64")}`);

function canvas({ webgl = null, twoD = true } = {}) {
  const result = { width: 7, height: 9, contexts: [], writes: 0, allocations: 0, image: null,
    getContext(kind) { this.contexts.push(kind); return kind === "webgl2" ? webgl : twoD ? context : null; } };
  const context = {
    createImageData(width, height) {
      result.allocations++;
      return { width, height, data: new Uint8ClampedArray(width * height * 4) };
    },
    putImageData(image, x, y) {
      assert.equal(x, 0); assert.equal(y, 0);
      assert.equal(image.width, result.width); assert.equal(image.height, result.height);
      result.writes++; result.image = { ...image, data: image.data.slice() };
    },
  };
  return result;
}
function vertices(points) {
  const bytes = new Uint8Array(112), view = new DataView(bytes.buffer);
  points.forEach(([x, y, u = 0.5, v = 0.5, rhw = 1, diffuse = 0xffffffff, z = 0], index) => {
    const base = index * 28;
    for (const [offset, value] of [[0, x], [4, y], [8, z], [12, rhw], [20, u], [24, v]]) {
      view.setFloat32(base + offset, value, true);
    }
    view.setUint32(base + 16, diffuse, true);
  });
  return [...bytes];
}
function quad(width = 4, height = 4, texture = solid(0xffffff), points) {
  return { vertices: vertices(points ?? [[0, 0, 0, 0], [width, 0, 1, 0], [0, height, 0, 1], [width, height, 1, 1]]), texture };
}
function solid(colorRef) {
  return { width: 1, height: 1, pixels: null, fills: [{ left: 0, top: 0, right: 1, bottom: 1, colorRef }] };
}
function pixel(target, x, y) {
  return [...target.image.data.slice((y * target.width + x) * 4, (y * target.width + x) * 4 + 4)];
}
function close(actual, expected, message) {
  actual.forEach((value, index) => assert(Math.abs(value - expected[index]) <= 1, `${message}: ${actual} != ${expected}`));
}
const unavailable = canvas();
assert.throws(() => new GameSurface(unavailable), /WebGL2 is unavailable/);
assert.deepEqual(unavailable.contexts, ["webgl2"], "production does not silently select software");
assert.throws(() => new GameSurface(canvas({ twoD: false }), { allowSoftware: true }), /Canvas2D is unavailable/);
assert.throws(() => new GameSurface(canvas(), { allowSoftware: "true" }), /WebGL2 is unavailable/);
const preferred = canvas({ webgl: {
  createShader: () => ({}), shaderSource() {}, compileShader() {}, getShaderParameter: () => true,
  deleteShader() {}, createProgram: () => ({}), attachShader() {}, linkProgram() {},
  getProgramParameter: () => true, createBuffer: () => ({}), createVertexArray: () => ({}),
  getUniformLocation: () => ({}), bindVertexArray() {}, bindBuffer() {}, enableVertexAttribArray() {},
  vertexAttribPointer() {}, useProgram() {}, uniform1i() {}, isContextLost: () => true,
} });
const preferredSurface = new GameSurface(preferred, { allowSoftware: true });
assert.equal(preferredSurface.backend, "webgl2");
assert.deepEqual(preferred.contexts, ["webgl2"]);
assert.throws(() => preferredSurface.present({ width: 4, height: 4, colorArgb: null, draws: [] }), /context was lost/);
assert.deepEqual(preferred.contexts, ["webgl2"], "context loss never silently swaps rendering backend");

const target = canvas(), surface = new GameSurface(target, { allowSoftware: true });
assert.equal(surface.backend, "software");
const frame = { width: 4, height: 4, colorArgb: 0x12345678, draws: [] };
surface.present(frame);
assert.equal(target.width, 4); assert.equal(target.height, 4);
assert.deepEqual(pixel(target, 1, 2), [0x34, 0x56, 0x78, 255], "clear ignores input alpha like the opaque WebGL canvas");
surface.present({ ...frame, colorArgb: null });
assert.deepEqual(pixel(target, 1, 2), [0, 0, 0, 255]);

// Padded BGRX snapshots become RGBA, with fills applied in order after the pixels.
const payload = Uint8Array.from([9, 9, 0, 0, 255, 0, 0, 255, 0, 1, 9, 9,
  255, 0, 0, 2, 255, 255, 255, 3, 9, 9]);
const texture = { width: 2, height: 2, pixels: { offset: 2, length: 20, pitch: 10 }, fills: [] };
const uploaded = quad(4, 4, texture);
surface.present({ ...frame, draws: [uploaded] }, payload);
assert.deepEqual(pixel(target, 1, 1), [255, 0, 0, 255]);
assert.deepEqual(pixel(target, 3, 1), [0, 255, 0, 255]);
assert.deepEqual(pixel(target, 1, 3), [0, 0, 255, 255]);
assert.deepEqual(pixel(target, 3, 3), [255, 255, 255, 255], "second triangle of the strip is rendered");
close(pixel(target, 2, 2), [128, 128, 128, 255], "bilinear sampling blends four texels");
close(pixel(target, 0, 0), [128, 128, 128, 255], "repeat blends across both texture seams");
surface.present({ ...frame, draws: [{ ...uploaded, texture: { ...texture,
  fills: [{ left: 0, top: 0, right: 1, bottom: 1, colorRef: 0x785634 }] } }] }, payload);
assert.deepEqual(pixel(target, 1, 1), [0x34, 0x56, 0x78, 255], "COLORREF fill overrides BGRX bytes");

// Constant UV outside [0,1], RGB diffuse modulation, and ignored diffuse alpha.
const tinted = quad(4, 4, texture, [[0, 0, -0.75, 1.25, 1, 0x00804020],
  [4, 0, -0.75, 1.25, 1, 0x00804020], [0, 4, -0.75, 1.25, 1, 0x00804020], [4, 4, -0.75, 1.25, 1, 0x00804020]]);
surface.present({ ...frame, draws: [tinted] }, payload);
assert.deepEqual(pixel(target, 2, 2), [128, 0, 0, 255]);
surface.present({ ...frame, draws: [quad(4, 4, solid(0xffffff),
  [[0, 0, 0.5, 0.5, 1, 0x00804020], [4, 0, 0.5, 0.5, 1, 0x00804020],
    [0, 4, 0.5, 0.5, 1, 0x00804020], [4, 4, 0.5, 0.5, 1, 0x00804020]])] });
assert.deepEqual(pixel(target, 2, 2), [128, 64, 32, 255]);

// A +0.5 position adjustment means the sample at (0.5,0.5) is outside a
// quad whose first guest vertex is (0.25,0.25).
surface.present({ ...frame, colorArgb: 0xff000000, draws: [quad(4, 4, solid(0xffffff),
  [[0.25, 0.25], [3.25, 0.25], [0.25, 3.25], [3.25, 3.25]])] });
assert.deepEqual(pixel(target, 0, 0), [0, 0, 0, 255]);
assert.deepEqual(pixel(target, 1, 1), [255, 255, 255, 255]);

// Rotated geometry, reversed winding, and offscreen clipping are generic.
const rotatedPoints = [[2, -1], [5, 2], [-1, 2], [2, 5]];
surface.present({ ...frame, colorArgb: 0xff000000, draws: [quad(4, 4, solid(0x0000ff), rotatedPoints)] });
assert.deepEqual(pixel(target, 0, 0), [0, 0, 0, 255]);
assert.deepEqual(pixel(target, 2, 2), [255, 0, 0, 255]);
const rotation = [...target.image.data];
surface.present({ ...frame, colorArgb: 0xff000000,
  draws: [quad(4, 4, solid(0x0000ff), [rotatedPoints[0], rotatedPoints[2], rotatedPoints[1], rotatedPoints[3]])] });
assert.deepEqual([...target.image.data], rotation, "winding never culls a triangle");

// At (1,1), first-triangle screen barycentrics are [1/2,1/4,1/4].
// RHW [1,2,1] yields perspective UV (0.4,0.2), unlike affine (0.25,0.25).
const perspectivePoints = [[0, 0, 0, 0, 1], [4, 0, 1, 0, 2], [0, 4, 0, 1, 1], [4, 4, 1, 1, 2]];
surface.present({ ...frame, draws: [quad(4, 4, texture, perspectivePoints)] }, payload);
close(pixel(target, 1, 1), [168, 77, 26, 255], "perspective-correct UV interpolation");
const gradient = perspectivePoints.map((point, index) => [...point, index === 1 || index === 3 ? 0xffff0000 : 0xff000000]);
surface.present({ ...frame, draws: [quad(4, 4, solid(0xffffff), gradient)] });
close(pixel(target, 1, 1), [102, 0, 0, 255], "perspective-correct diffuse interpolation");

surface.present({ ...frame, draws: [quad(4, 4, solid(0x0000ff)), quad(4, 4, solid(0x00ff00))] });
assert.deepEqual(pixel(target, 1, 1), [0, 255, 0, 255], "draw order is retained");
surface.present({ ...frame, colorArgb: 0xffffffff, draws: [quad(4, 4, { width: 1, height: 1, pixels: null, fills: [] })] });
assert.deepEqual(pixel(target, 2, 2), [0, 0, 0, 255], "undefined XRGB texels initialize as opaque black");

const old = { width: target.width, height: target.height, writes: target.writes,
  allocations: target.allocations, pixels: [...target.image.data] };
function reject(frame, payload = new Uint8Array()) {
  assert.throws(() => surface.present(frame, payload));
  assert.deepEqual({ width: target.width, height: target.height, writes: target.writes,
    allocations: target.allocations, pixels: [...target.image.data] }, old, "invalid complete frame cannot allocate or mutate the surface");
}
const valid = quad();
for (const bad of [null, {}, { ...frame, width: 0 }, { ...frame, height: 4097 }, { ...frame, colorArgb: -1 },
  { ...frame, draws: Array(1) }, { ...frame, draws: [null] }, { ...frame, draws: [valid, { ...valid, texture: null }] },
  { ...frame, draws: Array(4097).fill(valid) }, { ...frame, draws: [{ ...valid, vertices: [1] }] },
  { ...frame, draws: [{ ...valid, vertices: Array(112) }] },
  { ...frame, draws: [{ ...valid, texture: { width: 4096, height: 4096, fills: [], pixels: null } }] },
  { ...frame, draws: [{ ...valid, texture: { ...solid(0), fills: Array(16385).fill(solid(0).fills[0]) } }] },
  { ...frame, draws: [{ ...valid, texture: { ...solid(0), fills: [{ left: 0, top: 0, right: 2, bottom: 1, colorRef: 0 }] } }] },
]) reject(bad);
for (const [offset, value] of [[0, NaN], [4, Infinity], [8, -0.1], [8, 1.1], [12, 0], [12, -1], [12, 1e-44]]) {
  const bytes = Uint8Array.from(valid.vertices);
  new DataView(bytes.buffer).setFloat32(offset, value, true);
  reject({ ...frame, width: 3, height: 2, draws: [valid, { ...valid, vertices: [...bytes] }] });
}
for (const pixels of [undefined, { offset: 3, length: 20, pitch: 10 }, { offset: 2, length: 19, pitch: 10 },
  { offset: 2, length: 20, pitch: 7 }, { offset: -1, length: 20, pitch: 10 }]) {
  reject({ ...frame, draws: [{ ...uploaded, texture: { ...texture, pixels } }] }, payload);
}
reject(frame, new Uint8Array(16 * 1024 * 1024 + 1));
reject(frame, []);
// CPU rejection happens before an image allocation, independently of upload size.
reject({ ...frame, width: 4096, height: 4096, draws: Array(4).fill(quad(4096, 4096)) });
reject({ ...frame, draws: [quad(4, 4, { width: 1024, height: 1024, pixels: null,
  fills: Array(65).fill({ left: 0, top: 0, right: 1024, bottom: 1024, colorRef: 0 }) })] });

// Huge coordinates do not enlarge raster loops, and a wholly offscreen draw is empty.
surface.present({ ...frame, colorArgb: 0xff000000, draws: [quad(4, 4, solid(0xffffff),
  [[-1e20, -1e20], [1e20, -1e20], [-1e20, 1e20], [1e20, 1e20]])] });
assert.deepEqual(pixel(target, 2, 2), [255, 255, 255, 255]);
surface.present({ ...frame, colorArgb: 0xff123456, draws: [quad(4, 4, solid(0xffffff),
  [[1e20, 1e20], [2e20, 1e20], [1e20, 2e20], [2e20, 2e20]])] });
assert.deepEqual(pixel(target, 2, 2), [0x12, 0x34, 0x56, 255]);
console.log("Software game surface: explicit opt-in, shared validation, clear/BGRX/fills, bilinear repeat, tint, strip/rotation/perspective, CPU bounds and atomic rejection passed");
