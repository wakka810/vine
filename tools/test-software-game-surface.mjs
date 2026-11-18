import assert from "node:assert/strict";
import { build } from "esbuild";

const { outputFiles } = await build({
  entryPoints: ["browser/runtime-worker/game-surface.ts"],
  bundle: true, write: false, platform: "node", format: "esm"
});
const { GameSurface } = await import(`data:text/javascript;base64,${Buffer.from(outputFiles[0].contents).toString("base64")}`);
const presentation = await build({
  entryPoints: ["browser/runtime-worker/game-presentation.ts"],
  bundle: true, write: false, platform: "node", format: "esm"
});
const { texturePixels } = await import(`data:text/javascript;base64,${Buffer.from(presentation.outputFiles[0].contents).toString("base64")}`);

function canvas({ webgl = null, twoD = true } = {}) {
  const result = {
    width: 7, height: 9, contexts: [], writes: 0, allocations: 0, image: null,
    getContext(kind) { this.contexts.push(kind); return kind === "webgl2" ? webgl : twoD ? context : null; }
  };
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
const preferred = canvas({
  webgl: {
    createShader: () => ({}), shaderSource() { }, compileShader() { }, getShaderParameter: () => true,
    deleteShader() { }, createProgram: () => ({}), attachShader() { }, linkProgram() { },
    getProgramParameter: () => true, createBuffer: () => ({}), createVertexArray: () => ({}),
    getUniformLocation: () => ({}), bindVertexArray() { }, bindBuffer() { }, enableVertexAttribArray() { },
    vertexAttribPointer() { }, useProgram() { }, uniform1i() { }, isContextLost: () => true,
  }
});
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

const depthQuad = (colorRef, z, write = true) => ({
  ...quad(4, 4, solid(colorRef),
    [[0, 0, 0, 0, 1, 0xffffffff, z], [4, 0, 1, 0, 1, 0xffffffff, z], [0, 4, 0, 1, 1, 0xffffffff, z], [4, 4, 1, 1, 1, 0xffffffff, z]]), depth: { write, compare: 4 }
});
surface.present({ ...frame, depthClears: [{ beforeDraw: 0, value: 1 }], draws: [depthQuad(0xff, 0.25), depthQuad(0xff00, 0.75)] });
assert.deepEqual(pixel(target, 1, 1), [255, 0, 0, 255], "D16 depth rejects a later farther fragment");
surface.present({ ...frame, depthClears: [{ beforeDraw: 1, value: 1 }], draws: [depthQuad(0xff, 0.25), depthQuad(0xff00, 0.75)] });
assert.deepEqual(pixel(target, 1, 1), [0, 255, 0, 255], "depth-only clear occurs between draws");
surface.present({ ...frame, draws: [depthQuad(0xff, 0.25, false), depthQuad(0xff00, 0.75)] });
assert.deepEqual(pixel(target, 1, 1), [0, 255, 0, 255], "disabled depth writes preserve later visibility");

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
const pointClamp = { minFilter: "point", magFilter: "point", addressU: "clamp", addressV: "clamp" };
surface.present({ ...frame, draws: [{ ...uploaded, sampling: pointClamp }] }, payload);
assert.deepEqual(pixel(target, 0, 0), [255, 0, 0, 255], "clamp preserves the edge texel");
assert.deepEqual(pixel(target, 2, 2), [255, 255, 255, 255], "point selects one texel without blending");
const outside = quad(4, 4, texture, [[0, 0, -0.25, 1.25], [4, 0, -0.25, 1.25],
[0, 4, -0.25, 1.25], [4, 4, -0.25, 1.25]]);
for (const filter of ["point", "linear"]) {
  surface.present({ ...frame, draws: [{ ...outside, sampling: { ...pointClamp, minFilter: filter, magFilter: filter } }] }, payload);
  assert.deepEqual(pixel(target, 1, 1), [0, 0, 255, 255], "out-of-range coordinates clamp on both axes");
}
surface.present({ ...frame, draws: [{ ...outside, sampling: { ...pointClamp, addressU: "wrap", addressV: "wrap" } }] }, payload);
assert.deepEqual(pixel(target, 1, 1), [0, 255, 0, 255], "point repeat wraps both coordinates");
surface.present({
  ...frame, draws: [{
    ...uploaded, texture: {
      ...texture,
      fills: [{ left: 0, top: 0, right: 1, bottom: 1, colorRef: 0x785634 }]
    }
  }]
}, payload);
assert.deepEqual(pixel(target, 1, 1), [0x34, 0x56, 0x78, 255], "COLORREF fill overrides BGRX bytes");

// BGRX decoding uses the payload view's absolute byte offset and each row's pitch.
const expectedPixels = [3, 2, 1, 255, 6, 5, 4, 255, 9, 8, 7, 255, 12, 11, 10, 255];
for (const [byteOffset, offset, pitch] of [[0, 0, 8], [0, 4, 12], [1, 3, 12], [1, 0, 12], [0, 2, 12], [0, 0, 10]]) {
  const bytes = new Uint8Array(new ArrayBuffer(byteOffset + offset + pitch * 2), byteOffset);
  bytes.fill(0xee);
  bytes.set([1, 2, 3, 0, 4, 5, 6, 1], offset);
  bytes.set([7, 8, 9, 128, 10, 11, 12, 255], offset + pitch);
  const source = { width: 2, height: 2, pixels: { offset, length: pitch * 2, pitch }, fills: [] };
  const original = bytes.slice();
  const rgba = texturePixels(source, bytes);
  assert.deepEqual([...rgba], expectedPixels, "row padding and every X byte leave RGB ordering and opaque alpha exact");
  assert.deepEqual(bytes, original, "texture decoding never changes the source snapshot");
  assert.notEqual(rgba.buffer, bytes.buffer, "decoded pixels own their output buffer");
  bytes.fill(0);
  assert.deepEqual([...rgba], expectedPixels, "later source mutations cannot change decoded pixels");
}
const fillOnly = { width: 2, height: 1, pixels: null, fills: [] };
assert.deepEqual([...texturePixels(fillOnly, new Uint8Array())], [0, 0, 0, 255, 0, 0, 0, 255]);
fillOnly.fills = [{ left: 0, top: 0, right: 2, bottom: 1, colorRef: 0x112233 },
{ left: 1, top: 0, right: 2, bottom: 1, colorRef: 0x445566 }];
assert.deepEqual([...texturePixels(fillOnly, new Uint8Array())], [0x33, 0x22, 0x11, 255, 0x66, 0x55, 0x44, 255],
  "fills remain ordered after opaque pixel initialization");
const withFills = { width: 2, height: 1, pixels: { offset: 0, length: 8, pitch: 8 }, fills: fillOnly.fills };
assert.deepEqual([...texturePixels(withFills, Uint8Array.of(1, 2, 3, 4, 5, 6, 7, 8))],
  [0x33, 0x22, 0x11, 255, 0x66, 0x55, 0x44, 255], "ordered fills override decoded BGRX texels");

// Constant UV outside [0,1], RGB diffuse modulation, and ignored diffuse alpha.
const tinted = quad(4, 4, texture, [[0, 0, -0.75, 1.25, 1, 0x00804020],
[4, 0, -0.75, 1.25, 1, 0x00804020], [0, 4, -0.75, 1.25, 1, 0x00804020], [4, 4, -0.75, 1.25, 1, 0x00804020]]);
surface.present({ ...frame, draws: [tinted] }, payload);
assert.deepEqual(pixel(target, 2, 2), [128, 0, 0, 255]);
surface.present({
  ...frame, draws: [quad(4, 4, solid(0xffffff),
    [[0, 0, 0.5, 0.5, 1, 0x00804020], [4, 0, 0.5, 0.5, 1, 0x00804020],
    [0, 4, 0.5, 0.5, 1, 0x00804020], [4, 4, 0.5, 0.5, 1, 0x00804020]])]
});
assert.deepEqual(pixel(target, 2, 2), [128, 64, 32, 255]);

// A +0.5 position adjustment means the sample at (0.5,0.5) is outside a
// quad whose first guest vertex is (0.25,0.25).
surface.present({
  ...frame, colorArgb: 0xff000000, draws: [quad(4, 4, solid(0xffffff),
    [[0.25, 0.25], [3.25, 0.25], [0.25, 3.25], [3.25, 3.25]])]
});
assert.deepEqual(pixel(target, 0, 0), [0, 0, 0, 255]);
assert.deepEqual(pixel(target, 1, 1), [255, 255, 255, 255]);

// Rotated geometry, reversed winding, and offscreen clipping are generic.
const rotatedPoints = [[2, -1], [5, 2], [-1, 2], [2, 5]];
surface.present({ ...frame, colorArgb: 0xff000000, draws: [quad(4, 4, solid(0x0000ff), rotatedPoints)] });
assert.deepEqual(pixel(target, 0, 0), [0, 0, 0, 255]);
assert.deepEqual(pixel(target, 2, 2), [255, 0, 0, 255]);
const rotation = [...target.image.data];
surface.present({
  ...frame, colorArgb: 0xff000000,
  draws: [quad(4, 4, solid(0x0000ff), [rotatedPoints[0], rotatedPoints[2], rotatedPoints[1], rotatedPoints[3]])]
});
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

const alphaTexture = { width: 1, height: 1, alpha: true, fills: [], pixels: { offset: 0, length: 4, pitch: 4 } };
const alphaPixels = Uint8Array.from([0, 0, 255, 128]);
const alphaDraw = { ...quad(4, 4, alphaTexture), fragment: { textureAlpha: true, diffuseAlpha: false, blend: true, alphaTest: null } };
surface.present({ ...frame, colorArgb: 0xff0000ff, draws: [alphaDraw] }, alphaPixels);
close(pixel(target, 1, 1), [128, 0, 127, 255], "BGRA sample alpha blends over blue");
const tested = { ...alphaDraw, depth: { write: true, compare: 4 }, fragment: { ...alphaDraw.fragment, alphaTest: { compare: 5, reference: 200 } } };
surface.present({ ...frame, colorArgb: 0xff0000ff, draws: [tested, depthQuad(0xff00, 0.75)] }, alphaPixels);
assert.deepEqual(pixel(target, 1, 1), [0, 255, 0, 255], "alpha rejection must not write D16 depth");
for (const compare of [1, 2, 3, 4, 5, 6, 7, 8]) {
  const pass = [false, false, true, true, false, false, true, true][compare - 1];
  surface.present({ ...frame, colorArgb: 0xff0000ff, draws: [{ ...alphaDraw, fragment: { ...alphaDraw.fragment, blend: false, alphaTest: { compare, reference: 128 } } }] }, alphaPixels);
  assert.deepEqual(pixel(target, 1, 1), pass ? [255, 0, 0, 255] : [0, 0, 255, 255], "alpha comparison " + compare);
}
const diffuseAlpha = quad(4, 4, alphaTexture, [[0, 0, 0, 0, 1, 0x80ffffff], [4, 0, 1, 0, 1, 0x80ffffff], [0, 4, 0, 1, 1, 0x80ffffff], [4, 4, 1, 1, 1, 0x80ffffff]]);
surface.present({ ...frame, colorArgb: 0xff000000, draws: [{ ...diffuseAlpha, fragment: { ...alphaDraw.fragment, diffuseAlpha: true } }] }, alphaPixels);
close(pixel(target, 1, 1), [64, 0, 0, 255], "texture and diffuse alpha modulation");

surface.present({ ...frame, colorArgb: 0xff0000ff, draws: [{ ...quad(4, 4, solid(0xff)), clip: { left: 1, top: 1, right: 3, bottom: 3 } }] });
assert.deepEqual(pixel(target, 1, 1), [255, 0, 0, 255], "viewport interior is drawn");
assert.deepEqual(pixel(target, 0, 1), [0, 0, 255, 255], "viewport clips the left edge");
assert.deepEqual(pixel(target, 3, 2), [0, 0, 255, 255], "viewport excludes its right edge");
assert.deepEqual(pixel(target, 2, 3), [0, 0, 255, 255], "viewport excludes its bottom edge");

const old = {
  width: target.width, height: target.height, writes: target.writes,
  allocations: target.allocations, pixels: [...target.image.data]
};
function reject(frame, payload = new Uint8Array()) {
  assert.throws(() => surface.present(frame, payload));
  assert.deepEqual({
    width: target.width, height: target.height, writes: target.writes,
    allocations: target.allocations, pixels: [...target.image.data]
  }, old, "invalid complete frame cannot allocate or mutate the surface");
}
const valid = quad();
for (const clip of [{}, { left: 0, top: 0, right: 5, bottom: 4 }, { left: 2, top: 1, right: 1, bottom: 3 }, { left: 0.5, top: 0, right: 3, bottom: 3 }]) reject({ ...frame, draws: [valid, { ...valid, clip }] });
for (const bad of [null, {}, { ...frame, width: 0 }, { ...frame, height: 4097 }, { ...frame, colorArgb: -1 },
  { ...frame, draws: Array(1) }, { ...frame, draws: [null] }, { ...frame, draws: [valid, { ...valid, texture: null }] },
  { ...frame, draws: Array(4097).fill(valid) }, { ...frame, draws: [{ ...valid, vertices: [1] }] },
  { ...frame, draws: [{ ...valid, vertices: Array(112) }] },
  { ...frame, draws: [{ ...valid, texture: { width: 4096, height: 4096, fills: [], pixels: null } }] },
  { ...frame, draws: [{ ...valid, texture: { ...solid(0), fills: Array(16385).fill(solid(0).fills[0]) } }] },
  { ...frame, draws: [{ ...valid, texture: { ...solid(0), fills: [{ left: 0, top: 0, right: 2, bottom: 1, colorRef: 0 }] } }] },
]) reject(bad);
for (const fragment of [{}, { ...alphaDraw.fragment, textureAlpha: 1 }, { ...alphaDraw.fragment, alphaTest: { compare: 0, reference: 0 } }, { ...alphaDraw.fragment, alphaTest: { compare: 8, reference: 256 } }]) {
  reject({ ...frame, draws: [valid, { ...valid, fragment }] });
}
for (const sampling of [null, {}, { ...pointClamp, minFilter: "anisotropic" },
  { ...pointClamp, magFilter: 1 }, { ...pointClamp, addressU: "mirror" }, { ...pointClamp, addressV: null }]) {
  reject({ ...frame, draws: [valid, { ...valid, sampling }] });
}
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
reject({
  ...frame, draws: [quad(4, 4, {
    width: 1024, height: 1024, pixels: null,
    fills: Array(65).fill({ left: 0, top: 0, right: 1024, bottom: 1024, colorRef: 0 })
  })]
});

// Huge coordinates do not enlarge raster loops, and a wholly offscreen draw is empty.
surface.present({
  ...frame, colorArgb: 0xff000000, draws: [quad(4, 4, solid(0xffffff),
    [[-1e20, -1e20], [1e20, -1e20], [-1e20, 1e20], [1e20, 1e20]])]
});
assert.deepEqual(pixel(target, 2, 2), [255, 255, 255, 255]);
surface.present({
  ...frame, colorArgb: 0xff123456, draws: [quad(4, 4, solid(0xffffff),
    [[1e20, 1e20], [2e20, 1e20], [1e20, 2e20], [2e20, 2e20]])]
});
assert.deepEqual(pixel(target, 2, 2), [0x12, 0x34, 0x56, 255]);

// Incremental Copy presentation preserves color and depth, including trailing post-scene depth clears.
surface.present({
  ...frame, colorArgb: 0xff102030, depthClears: [{ beforeDraw: 0, value: 1 }],
  draws: [{ ...depthQuad(0x0000ff, 0.25), clip: { left: 0, top: 0, right: 2, bottom: 4 } }]
});
assert.deepEqual(pixel(target, 0, 1), [255, 0, 0, 255]);
assert.deepEqual(pixel(target, 3, 1), [0x10, 0x20, 0x30, 255]);
surface.present({
  ...frame, preserve: true, colorArgb: null,
  depthClears: [{ beforeDraw: 1, value: 1 }],
  draws: [{ ...depthQuad(0x00ff00, 0.75), clip: { left: 0, top: 0, right: 2, bottom: 4 } }]
});
assert.deepEqual(pixel(target, 0, 1), [255, 0, 0, 255], "preserved depth rejects farther draw before trailing clear");
assert.deepEqual(pixel(target, 3, 1), [0x10, 0x20, 0x30, 255], "preserved color retains untouched border");
surface.present({
  ...frame, preserve: true, colorArgb: null,
  draws: [{ ...depthQuad(0x00ff00, 0.75), clip: { left: 0, top: 0, right: 2, bottom: 4 } }]
});
assert.deepEqual(pixel(target, 0, 1), [0, 255, 0, 255], "trailing depth clear at draws.length resets preserved depth for next frame");
assert.deepEqual(pixel(target, 3, 1), [0x10, 0x20, 0x30, 255]);

surface.present({
  ...frame, colorClearBeforeDraw: 1,
  draws: [depthQuad(0xff, 0.25), depthQuad(0xff00, 0.75)]
});
assert.deepEqual(pixel(target, 1, 1), [0x34, 0x56, 0x78, 255], "target clear erases the near draw's color but retains its depth");
surface.present({
  ...frame, preserve: true, colorArgb: 0xff102030,
  draws: [depthQuad(0xff00, 0.75)]
});
assert.deepEqual(pixel(target, 1, 1), [0x10, 0x20, 0x30, 255], "a target-only clear also retains depth from the previous frame");

// Additive blend, stage-0 colorAdd, and linear W-fog.
surface.present({
  ...frame, colorArgb: 0xff204060, draws: [{
    ...quad(4, 4, alphaTexture, [[0, 0, 0, 0, 1, 0xff102030], [4, 0, 1, 0, 1, 0xff102030],
    [0, 4, 0, 1, 1, 0xff102030], [4, 4, 1, 1, 1, 0xff102030]]),
    fragment: { textureAlpha: true, diffuseAlpha: false, blend: true, additive: true, colorAdd: true, alphaTest: null },
  }]
}, alphaPixels);
// baseRgb=(1,0,0), tint=(0x10,0x20,0x30)/255 -> lit=(255,32,48), alpha=128/255, additive over (0x20,0x40,0x60)
close(pixel(target, 1, 1), [0x20 + 128, 0x40 + 16, 0x60 + 24, 255], "colorAdd + additive blend");

// eyeW = 1 / rhw = 1 / 0.005 = 200; fog [100, 300] => f = 0.5 between fogColor and litRgb.
surface.present({
  ...frame, colorArgb: 0xff000000, draws: [{
    ...quad(4, 4, solid(0x0000ff), [[0, 0, 0, 0, 0.005], [4, 0, 1, 0, 0.005], [0, 4, 0, 1, 0.005], [4, 4, 1, 1, 0.005]]),
    fragment: {
      textureAlpha: false, diffuseAlpha: false, blend: false, alphaTest: null,
      fog: { colorArgb: 0xff0080ff, start: 100, end: 300 }
    },
  }]
});
close(pixel(target, 1, 1), [128, 64, 128, 255], "linear W-fog mixes fogColor with litRgb by 1/rhw");

// Shared snapshot reuse in validatePresentation does not multiply upload budget.
const sharedSheet = new Uint8Array(1024 * 1024 * 4);
const sharedTex = { width: 1024, height: 1024, alpha: true, fills: [], pixels: { offset: 0, length: sharedSheet.length, pitch: 4096 } };
surface.present({ ...frame, colorArgb: 0xff000000, draws: Array(8).fill(quad(4, 4, sharedTex)) }, sharedSheet);

const oldExtended = {
  width: target.width, height: target.height, writes: target.writes,
  allocations: target.allocations, pixels: [...target.image.data]
};
for (const badExt of [
  { ...frame, preserve: 1, draws: [] },
  { ...frame, colorClearBeforeDraw: 1, draws: [] },
  { ...frame, colorClearBeforeDraw: null, draws: [] },
  { ...frame, draws: [{ ...valid, fragment: { textureAlpha: false, diffuseAlpha: false, blend: true, additive: "yes", alphaTest: null } }] },
  { ...frame, draws: [{ ...valid, fragment: { textureAlpha: false, diffuseAlpha: false, blend: false, colorAdd: 1, alphaTest: null } }] },
  { ...frame, draws: [{ ...valid, fragment: { textureAlpha: false, diffuseAlpha: false, blend: false, alphaTest: null, fog: { colorArgb: -1, start: 0, end: 1 } } }] },
  { ...frame, draws: [{ ...valid, fragment: { textureAlpha: false, diffuseAlpha: false, blend: false, alphaTest: null, fog: { colorArgb: 0, start: NaN, end: 1 } } }] },
]) {
  assert.throws(() => surface.present(badExt));
  assert.deepEqual({
    width: target.width, height: target.height, writes: target.writes,
    allocations: target.allocations, pixels: [...target.image.data]
  }, oldExtended);
}
console.log("Software game surface: explicit opt-in, shared validation, clear/BGRX/fills, point/linear wrap/clamp, tint, strip/rotation/perspective, preserve/depth/fog/additive, CPU bounds and atomic rejection passed");
