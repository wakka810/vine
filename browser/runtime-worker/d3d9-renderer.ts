import {
  BrowserResourceTable,
  type BrowserResourceRecord,
  type LogicalComBinding,
} from "./browser-resource-table.js";
import type {
  RuntimeBrowserPresentationReceipt,
  RuntimeGraphicsDeviceSnapshot,
} from "./protocol.js";

interface D3d9WebGlDeviceResource {
  readonly canvas: OffscreenCanvas;
  readonly gl: WebGL2RenderingContext;
  readonly program: WebGLProgram;
  readonly vertexArray: WebGLVertexArrayObject;
  readonly vertexBuffer: WebGLBuffer;
  readonly texture: WebGLTexture;
  readonly viewportUniform: WebGLUniformLocation;
  readonly texturePixels: Uint8Array;
  readonly vertices: Float32Array;
}

const FLOATS_PER_RENDER_VERTEX = 7;
const BYTES_PER_RENDER_VERTEX = FLOATS_PER_RENDER_VERTEX * Float32Array.BYTES_PER_ELEMENT;
const PRESENT_WIDTH = 1_280;
const PRESENT_HEIGHT = 720;
const TEXTURE_WIDTH = 2_048;
const TEXTURE_HEIGHT = 1_024;
const TEXTURE_BYTE_LENGTH = TEXTURE_WIDTH * TEXTURE_HEIGHT * 4;
const VERTEX_COUNT = 4;
const SOURCE_VERTEX_BYTE_LENGTH = VERTEX_COUNT * 28;

/** Browser resources only; logical device and frame state remain authoritative in Rust. */
export class D3d9WebGl2Renderer {
  readonly #canvas: OffscreenCanvas;
  readonly #preflightResource: D3d9WebGlDeviceResource;
  readonly #resources = new BrowserResourceTable<D3d9WebGlDeviceResource>();
  readonly #deviceResources = new Map<
    string,
    BrowserResourceRecord<D3d9WebGlDeviceResource>
  >();
  readonly #probePixels = new Uint8Array(80 * 40 * 4);

  constructor(canvas: OffscreenCanvas) {
    invariant(
      canvas.width === PRESENT_WIDTH && canvas.height === PRESENT_HEIGHT,
      "first-frame renderer canvas dimensions are outside the policy band",
    );
    this.#canvas = canvas;
    // Allocate the bounded browser capacity before guest execution. Present then
    // validates the Rust snapshot before publishing the logical binding or pixels.
    this.#preflightResource = createWebGlResource(canvas);
  }

  present(
    device: RuntimeGraphicsDeviceSnapshot,
    binding: LogicalComBinding,
    textureBacking?: Uint8Array,
    verifyPixels = true,
  ): RuntimeBrowserPresentationReceipt {
    invariant(
      binding.interfaceName === "IDirect3DDevice9" &&
        binding.objectId === device.comObjectId,
      "browser renderer COM binding does not match the Rust logical device",
    );
    const frame = device.presentedFrame;
    invariant(frame !== undefined, "Present completion omitted its published frame");
    invariant(
      device.presentation.presentMode === "discard" &&
        device.presentation.backBufferCount === 1 &&
        device.presentation.sampleCount === 1 &&
        device.presentation.windowed,
      "browser renderer received an unsupported presentation band",
    );
    invariant(
      frame.backBuffer.contentGeneration + 1 ===
        device.backBuffer.contentGeneration &&
        device.backBuffer.uniformClear === undefined &&
        device.backBuffer.drawSubmissions === undefined,
      "discard Present did not publish and replace the back buffer atomically",
    );

    const { width, height } = device.presentation;
    invariant(
      Number.isSafeInteger(width) && width > 0 &&
        Number.isSafeInteger(height) && height > 0,
      "presentation dimensions are invalid",
    );
    invariant(
      width === PRESENT_WIDTH &&
        height === PRESENT_HEIGHT &&
        this.#canvas.width === width &&
        this.#canvas.height === height,
      "first-frame presentation geometry is outside the policy band",
    );

    const clear = frame.backBuffer.uniformClear;
    invariant(
      clear !== undefined &&
        clear.rect.left === 0 &&
        clear.rect.top === 0 &&
        clear.rect.right === width &&
        clear.rect.bottom === height,
      "first-frame renderer requires the policy-bounded full back-buffer clear",
    );
    const clearArgb = clear.colorArgb >>> 0;
    const submissions = frame.backBuffer.drawSubmissions ?? [];
    invariant(submissions.length === 1, "first Present requires one draw submission");
    const submission = submissions[0];
    invariant(submission !== undefined, "first Present draw submission is missing");
    invariant(
      submission.primitiveTopology === "triangle-strip" &&
        submission.startVertex === 0 &&
        submission.primitiveCount === 2 &&
        submission.vertexCount === VERTEX_COUNT &&
        submission.flexibleVertexFormat === "xyz-rhw-diffuse-texture1" &&
        submission.streamSource.offsetInBytes === 0 &&
        submission.streamSource.stride === 28,
      "first-frame renderer received an unsupported draw band",
    );
    invariant(
      submission.renderStates.cullMode === "none" &&
        submission.renderStates.depthTestEnabled === false &&
        submission.renderStates.lightingEnabled === false &&
        submission.renderStates.alphaBlendEnabled === true &&
        submission.samplerStates.samplerZero?.minFilter === "linear" &&
        submission.samplerStates.samplerZero.magFilter === "linear" &&
        submission.textureStageStates.stageZero?.colorOperation === "modulate" &&
        submission.textureStageStates.stageZero.colorArgumentOne === "texture" &&
        submission.textureStageStates.stageZero.colorArgumentTwo === "diffuse" &&
        submission.textureStageStates.stageZero.alphaOperation ===
          "select-argument-one" &&
        submission.textureStageStates.stageZero.alphaArgumentOne === "texture" &&
        submission.textureStageStates.stageZero.alphaArgumentTwo === "diffuse",
      "first-frame renderer received unsupported fixed-function state",
    );
    const { texturePixels, vertices } = this.#preflightResource;
    const uploadPixels = materializeTexture(
      submission.texture,
      texturePixels,
      textureBacking,
    );
    decodeVertices(submission.vertexData, submission.vertexCount, vertices);

    // All policy, identity, capacity, and immutable input validation is complete.
    // Publishing the browser binding and issuing GPU commands are the adapter commit.
    const record = this.#resourceForDevice(binding);
    const { gl, program, vertexArray, vertexBuffer, texture, viewportUniform } =
      record.resource;
    gl.viewport(0, 0, width, height);
    gl.disable(gl.CULL_FACE);
    gl.disable(gl.DEPTH_TEST);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.useProgram(program);
    gl.uniform2f(viewportUniform, width, height);
    gl.bindVertexArray(vertexArray);
    gl.bindBuffer(gl.ARRAY_BUFFER, vertexBuffer);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, vertices);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
    gl.texSubImage2D(
      gl.TEXTURE_2D,
      0,
      0,
      0,
      TEXTURE_WIDTH,
      TEXTURE_HEIGHT,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      uploadPixels,
    );
    gl.clearColor(
      ((clearArgb >>> 16) & 0xff) / 255,
      ((clearArgb >>> 8) & 0xff) / 255,
      (clearArgb & 0xff) / 255,
      ((clearArgb >>> 24) & 0xff) / 255,
    );
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, submission.vertexCount);

    const centerPixel = new Uint8Array(4);
    if (verifyPixels) {
      gl.flush();
      gl.readPixels(
        Math.floor(width / 2),
        Math.floor(height / 2),
        1,
        1,
        gl.RGBA,
        gl.UNSIGNED_BYTE,
        centerPixel,
      );
      const webGlError = gl.getError();
      invariant(webGlError === gl.NO_ERROR, `WebGL2 Present failed with error ${webGlError}`);
    }

    return {
      backend: "webgl2-offscreen-canvas",
      resource: {
        id: record.id,
        kind: "d3d9-webgl2-device",
        binding: {
          objectId: record.binding.objectId,
          interfaceId: record.binding.interfaceId,
          interfaceName: "IDirect3DDevice9",
        },
      },
      browserResourceCount: this.#resources.size,
      deviceId: device.id,
      deviceGeneration: device.generation,
      frameSequence: frame.sequence,
      presentedContentGeneration: frame.backBuffer.contentGeneration,
      discardedBackBufferContentGeneration: device.backBuffer.contentGeneration,
      width,
      height,
      drawSubmissionCount: submissions.length,
      uploadedTextureBytes: uploadPixels.byteLength,
      uploadedVertexBytes: submission.vertexData.length,
      centerPixelRgba: [
        requireByte(centerPixel[0]),
        requireByte(centerPixel[1]),
        requireByte(centerPixel[2]),
        requireByte(centerPixel[3]),
      ],
      webGlError: 0,
      capacityPreflightBeforeGuestExecution: true,
      bindingPublishedAfterSnapshotValidation: true,
      renderedBeforeGuestResume: true,
    };
  }

  countDarkPixels(
    left: number,
    top: number,
    width: 80,
    height: 40,
    channelLimit: number,
  ): number {
    invariant(
      Number.isInteger(left) &&
        Number.isInteger(top) &&
        left >= 0 &&
        top >= 0 &&
        left + width <= PRESENT_WIDTH &&
        top + height <= PRESENT_HEIGHT &&
        Number.isInteger(channelLimit) &&
        channelLimit > 0 &&
        channelLimit <= 0xff,
      "presentation probe is outside the bounded frame",
    );
    const { gl } = this.#preflightResource;
    gl.readPixels(
      left,
      PRESENT_HEIGHT - top - height,
      width,
      height,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      this.#probePixels,
    );
    const webGlError = gl.getError();
    invariant(
      webGlError === gl.NO_ERROR,
      `WebGL2 presentation probe failed with error ${webGlError}`,
    );
    let darkPixels = 0;
    for (let offset = 0; offset < this.#probePixels.length; offset += 4) {
      if (
        requireByte(this.#probePixels[offset]) < channelLimit &&
        requireByte(this.#probePixels[offset + 1]) < channelLimit &&
        requireByte(this.#probePixels[offset + 2]) < channelLimit
      ) {
        darkPixels += 1;
      }
    }
    return darkPixels;
  }

  readPixelRgba(x: number, y: number): [number, number, number, number] {
    invariant(
      Number.isInteger(x) &&
        Number.isInteger(y) &&
        x >= 0 &&
        x < PRESENT_WIDTH &&
        y >= 0 &&
        y < PRESENT_HEIGHT,
      "presentation pixel probe is outside the bounded frame",
    );
    const { gl } = this.#preflightResource;
    const pixel = new Uint8Array(4);
    gl.readPixels(
      x,
      PRESENT_HEIGHT - y - 1,
      1,
      1,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      pixel,
    );
    const webGlError = gl.getError();
    invariant(
      webGlError === gl.NO_ERROR,
      `WebGL2 presentation pixel probe failed with error ${webGlError}`,
    );
    return [
      requireByte(pixel[0]),
      requireByte(pixel[1]),
      requireByte(pixel[2]),
      requireByte(pixel[3]),
    ];
  }

  #resourceForDevice(
    binding: LogicalComBinding,
  ): BrowserResourceRecord<D3d9WebGlDeviceResource> {
    const existing = this.#deviceResources.get(binding.objectId);
    if (existing !== undefined) {
      invariant(
        existing.binding.interfaceId === binding.interfaceId &&
          existing.binding.interfaceName === binding.interfaceName,
        "logical COM object changed renderer binding",
      );
      return existing;
    }
    invariant(this.#deviceResources.size === 0, "first-frame renderer supports one device");
    const record = this.#resources.register(
      "d3d9-webgl2-device",
      binding,
      this.#preflightResource,
    );
    this.#deviceResources.set(binding.objectId, record);
    return record;
  }
}

function createWebGlResource(canvas: OffscreenCanvas): D3d9WebGlDeviceResource {
  const gl = canvas.getContext("webgl2", {
    alpha: false,
    antialias: false,
    depth: false,
    preserveDrawingBuffer: true,
    premultipliedAlpha: false,
    stencil: false,
  });
  invariant(gl !== null, "OffscreenCanvas WebGL2 is unavailable");
  const program = linkProgram(
    gl,
    `#version 300 es
layout(location = 0) in vec2 aPosition;
layout(location = 1) in vec2 aUv;
layout(location = 2) in vec3 aDiffuse;
uniform vec2 uViewport;
out vec2 vUv;
out vec3 vDiffuse;
void main() {
  vec2 clip = vec2((aPosition.x / uViewport.x) * 2.0 - 1.0,
                   1.0 - (aPosition.y / uViewport.y) * 2.0);
  gl_Position = vec4(clip, 0.0, 1.0);
  vUv = aUv;
  vDiffuse = aDiffuse;
}`,
    `#version 300 es
precision mediump float;
uniform sampler2D uTexture;
in vec2 vUv;
in vec3 vDiffuse;
out vec4 outColor;
void main() {
  vec4 sampled = texture(uTexture, vUv);
  outColor = vec4(sampled.bgr * vDiffuse, 1.0);
}`,
  );
  const vertexArray = gl.createVertexArray();
  const vertexBuffer = gl.createBuffer();
  const texture = gl.createTexture();
  invariant(
    vertexArray !== null && vertexBuffer !== null && texture !== null,
    "WebGL2 resource allocation failed",
  );
  gl.useProgram(program);
  gl.bindVertexArray(vertexArray);
  gl.bindBuffer(gl.ARRAY_BUFFER, vertexBuffer);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, BYTES_PER_RENDER_VERTEX, 0);
  gl.enableVertexAttribArray(1);
  gl.vertexAttribPointer(
    1,
    2,
    gl.FLOAT,
    false,
    BYTES_PER_RENDER_VERTEX,
    2 * Float32Array.BYTES_PER_ELEMENT,
  );
  gl.enableVertexAttribArray(2);
  gl.vertexAttribPointer(
    2,
    3,
    gl.FLOAT,
    false,
    BYTES_PER_RENDER_VERTEX,
    4 * Float32Array.BYTES_PER_ELEMENT,
  );
  const textureUniform = gl.getUniformLocation(program, "uTexture");
  const viewportUniform = gl.getUniformLocation(program, "uViewport");
  invariant(
    textureUniform !== null && viewportUniform !== null,
    "WebGL2 shader uniforms are unavailable",
  );
  gl.uniform1i(textureUniform, 0);
  gl.bufferData(gl.ARRAY_BUFFER, SOURCE_VERTEX_BYTE_LENGTH, gl.STREAM_DRAW);
  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texImage2D(
    gl.TEXTURE_2D,
    0,
    gl.RGBA,
    TEXTURE_WIDTH,
    TEXTURE_HEIGHT,
    0,
    gl.RGBA,
    gl.UNSIGNED_BYTE,
    null,
  );
  const webGlError = gl.getError();
  invariant(
    webGlError === gl.NO_ERROR,
    `WebGL2 renderer preflight failed with error ${webGlError}`,
  );
  return {
    canvas,
    gl,
    program,
    vertexArray,
    vertexBuffer,
    texture,
    viewportUniform,
    texturePixels: new Uint8Array(TEXTURE_BYTE_LENGTH),
    vertices: new Float32Array(VERTEX_COUNT * FLOATS_PER_RENDER_VERTEX),
  };
}

function materializeTexture(
  texture: NonNullable<
    NonNullable<RuntimeGraphicsDeviceSnapshot["presentedFrame"]>["backBuffer"]["drawSubmissions"]
  >[number]["texture"],
  pixels: Uint8Array,
  backing?: Uint8Array,
): Uint8Array {
  const { width, height, colorFormat } = texture.descriptor;
  invariant(
    colorFormat === "b8g8r8x8-unorm" &&
      width === TEXTURE_WIDTH &&
      height === TEXTURE_HEIGHT &&
      texture.descriptor.levels === 1 &&
      texture.descriptor.usage === "none" &&
      texture.descriptor.pool === "default",
    "renderer texture descriptor is unsupported",
  );
  const byteLength = width * height * 4;
  invariant(
    byteLength === TEXTURE_BYTE_LENGTH && pixels.byteLength === byteLength,
    "renderer texture staging capacity does not match the policy band",
  );
  const pixelSource = texture.content.pixelSource;
  if (pixelSource !== undefined) {
    invariant(
      backing !== undefined &&
        pixelSource.range.byteLength === byteLength &&
        pixelSource.rowPitch === width * 4 &&
        backing.byteLength === byteLength,
      "renderer texture pixel source does not match its checked guest backing",
    );
    return backing;
  }
  invariant(backing === undefined, "renderer received guest backing without a pixel source");
  pixels.fill(0);
  for (let offset = 3; offset < pixels.length; offset += 4) {
    pixels[offset] = 0xff;
  }
  let previousSequence = 0;
  for (const fill of texture.content.fills) {
    const { left, top, right, bottom } = fill.rect;
    invariant(
      fill.sequence > previousSequence &&
        left >= 0 && top >= 0 && right > left && bottom > top &&
        right <= width && bottom <= height,
      "renderer texture fill is invalid",
    );
    previousSequence = fill.sequence;
    const colorRef = fill.colorRef >>> 0;
    const red = colorRef & 0xff;
    const green = (colorRef >>> 8) & 0xff;
    const blue = (colorRef >>> 16) & 0xff;
    for (let y = top; y < bottom; y += 1) {
      for (let x = left; x < right; x += 1) {
        const offset = (y * width + x) * 4;
        pixels[offset] = blue;
        pixels[offset + 1] = green;
        pixels[offset + 2] = red;
        pixels[offset + 3] = 0xff;
      }
    }
  }
  return pixels;
}

function decodeVertices(
  vertexData: number[],
  vertexCount: number,
  vertices: Float32Array,
): void {
  invariant(
    vertexCount === VERTEX_COUNT &&
      vertexData.length === SOURCE_VERTEX_BYTE_LENGTH &&
      vertices.length === vertexCount * FLOATS_PER_RENDER_VERTEX &&
      vertexData.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 0xff),
    "renderer vertex snapshot has an invalid size or byte",
  );
  const bytes = Uint8Array.from(vertexData);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  for (let index = 0; index < vertexCount; index += 1) {
    const source = index * 28;
    const destination = index * FLOATS_PER_RENDER_VERTEX;
    const x = view.getFloat32(source, true);
    const y = view.getFloat32(source + 4, true);
    const z = view.getFloat32(source + 8, true);
    const rhw = view.getFloat32(source + 12, true);
    const diffuse = view.getUint32(source + 16, true);
    const u = view.getFloat32(source + 20, true);
    const v = view.getFloat32(source + 24, true);
    invariant(
      [x, y, z, rhw, u, v].every(Number.isFinite) && z === 0 && rhw === 1,
      "renderer transformed vertex is outside the policy band",
    );
    vertices[destination] = x;
    vertices[destination + 1] = y;
    vertices[destination + 2] = u;
    vertices[destination + 3] = v;
    vertices[destination + 4] = ((diffuse >>> 16) & 0xff) / 255;
    vertices[destination + 5] = ((diffuse >>> 8) & 0xff) / 255;
    vertices[destination + 6] = (diffuse & 0xff) / 255;
  }
}

function linkProgram(
  gl: WebGL2RenderingContext,
  vertexSource: string,
  fragmentSource: string,
): WebGLProgram {
  const vertexShader = compileShader(gl, gl.VERTEX_SHADER, vertexSource);
  const fragmentShader = compileShader(gl, gl.FRAGMENT_SHADER, fragmentSource);
  const program = gl.createProgram();
  invariant(program !== null, "WebGL2 program allocation failed");
  gl.attachShader(program, vertexShader);
  gl.attachShader(program, fragmentShader);
  gl.linkProgram(program);
  invariant(
    gl.getProgramParameter(program, gl.LINK_STATUS) === true,
    `WebGL2 program link failed: ${gl.getProgramInfoLog(program) ?? "unknown"}`,
  );
  gl.deleteShader(vertexShader);
  gl.deleteShader(fragmentShader);
  return program;
}

function compileShader(
  gl: WebGL2RenderingContext,
  type: number,
  source: string,
): WebGLShader {
  const shader = gl.createShader(type);
  invariant(shader !== null, "WebGL2 shader allocation failed");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  invariant(
    gl.getShaderParameter(shader, gl.COMPILE_STATUS) === true,
    `WebGL2 shader compile failed: ${gl.getShaderInfoLog(shader) ?? "unknown"}`,
  );
  return shader;
}

function requireByte(value: number | undefined): number {
  invariant(value !== undefined, "WebGL2 pixel sample is incomplete");
  return value;
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}
