interface TextureFill {
  left: number; top: number; right: number; bottom: number; colorRef: number;
}
interface QuadPresentation {
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

// Browser resources only. Windows devices, render state and draw snapshots belong to Rust.
export class GameSurface {
  readonly #canvas: OffscreenCanvas;
  readonly #gl: WebGL2RenderingContext;
  readonly #program: WebGLProgram;
  readonly #vertices: WebGLBuffer;
  readonly #layout: WebGLVertexArrayObject;
  readonly #resolution: WebGLUniformLocation;

  constructor(canvas: OffscreenCanvas) {
    const gl = canvas.getContext("webgl2", {
      alpha: false, antialias: false, depth: false, stencil: false,
      preserveDrawingBuffer: true,
    });
    if (gl === null) throw new Error("WebGL2 is unavailable for the game surface");
    this.#canvas = canvas;
    this.#gl = gl;
    const shader = (type: number, source: string): WebGLShader => {
      const result = gl.createShader(type);
      if (result === null) throw new Error("WebGL2 shader allocation failed");
      gl.shaderSource(result, source);
      gl.compileShader(result);
      if (!gl.getShaderParameter(result, gl.COMPILE_STATUS)) {
        const message = gl.getShaderInfoLog(result);
        gl.deleteShader(result);
        throw new Error(`WebGL2 shader compilation failed: ${message}`);
      }
      return result;
    };
    const vertex = shader(gl.VERTEX_SHADER, `#version 300 es
      layout(location=0) in vec4 position;
      layout(location=1) in vec2 texcoord;
      layout(location=2) in vec4 diffuse;
      uniform vec2 resolution;
      out vec2 uv;
      out vec4 tint;
      void main() {
        vec2 clip = (position.xy + vec2(0.5)) / resolution * 2.0 - 1.0;
        float w = 1.0 / position.w;
        gl_Position = vec4(clip.x * w, -clip.y * w, (position.z * 2.0 - 1.0) * w, w);
        uv = texcoord; tint = diffuse;
      }`);
    const fragment = shader(gl.FRAGMENT_SHADER, `#version 300 es
      precision highp float;
      uniform sampler2D image;
      in vec2 uv;
      in vec4 tint;
      out vec4 pixel;
      void main() {
        vec4 sampled = texture(image, uv);
        pixel = vec4(sampled.rgb * tint.rgb, sampled.a);
      }`);
    const program = gl.createProgram();
    if (program === null) throw new Error("WebGL2 program allocation failed");
    gl.attachShader(program, vertex); gl.attachShader(program, fragment); gl.linkProgram(program);
    gl.deleteShader(vertex); gl.deleteShader(fragment);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      throw new Error(`WebGL2 program linking failed: ${gl.getProgramInfoLog(program)}`);
    }
    const vertices = gl.createBuffer(), layout = gl.createVertexArray();
    const resolution = gl.getUniformLocation(program, "resolution");
    if (vertices === null || layout === null || resolution === null) {
      throw new Error("WebGL2 quad resources are unavailable");
    }
    this.#program = program; this.#vertices = vertices;
    this.#layout = layout; this.#resolution = resolution;
    gl.bindVertexArray(layout); gl.bindBuffer(gl.ARRAY_BUFFER, vertices);
    for (const [index, size, offset] of [[0, 4, 0], [1, 2, 16], [2, 4, 24]]) {
      gl.enableVertexAttribArray(index!);
      gl.vertexAttribPointer(index!, size!, gl.FLOAT, false, 40, offset!);
    }
    gl.useProgram(program);
    gl.uniform1i(gl.getUniformLocation(program, "image"), 0);
  }

  present(frame: GamePresentation, payload: Uint8Array = new Uint8Array(0)): void {
    const gl = this.#gl;
    if (!dimension(frame.width) || !dimension(frame.height) ||
        (frame.colorArgb !== null && !color(frame.colorArgb)) ||
        !Array.isArray(frame.draws) || frame.draws.length > 4096 ||
        !(payload instanceof Uint8Array) || payload.length > 16 * 1024 * 1024) {
      throw new Error("Invalid game presentation");
    }
    if (gl.isContextLost()) throw new Error("The game WebGL2 context was lost");
    const maximum = Math.min(gl.getParameter(gl.MAX_TEXTURE_SIZE), gl.getParameter(gl.MAX_RENDERBUFFER_SIZE));
    if (frame.width > maximum || frame.height > maximum) throw new Error("Game surface exceeds the WebGL2 size limit");
    let byteCount = 0, fillCount = 0;
    // Validate the complete boundary before changing the surface or allocating texture pixels.
    const vertices = frame.draws.map(draw => {
      const texture = draw.texture;
      if (!dimension(texture.width) || !dimension(texture.height) ||
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
        if (![fill.left, fill.top, fill.right, fill.bottom].every(Number.isInteger) ||
            fill.left < 0 || fill.top < 0 || fill.right <= fill.left || fill.bottom <= fill.top ||
            fill.right > texture.width || fill.bottom > texture.height || !color(fill.colorRef)) {
          throw new Error("Invalid presentation texture fill");
        }
      }
      if (!Array.isArray(draw.vertices) || draw.vertices.length !== 112 ||
          draw.vertices.some(value => !Number.isInteger(value) || value < 0 || value > 255)) {
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
    if (this.#canvas.width !== frame.width) this.#canvas.width = frame.width;
    if (this.#canvas.height !== frame.height) this.#canvas.height = frame.height;
    gl.viewport(0, 0, frame.width, frame.height);
    gl.disable(gl.DEPTH_TEST); gl.disable(gl.CULL_FACE); gl.disable(gl.SCISSOR_TEST);
    const background = frame.colorArgb ?? 0xff000000;
    gl.clearColor(((background >>> 16) & 255) / 255, ((background >>> 8) & 255) / 255, (background & 255) / 255, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(this.#program); gl.bindVertexArray(this.#layout);
    gl.uniform2f(this.#resolution, frame.width, frame.height);
    gl.enable(gl.BLEND); gl.blendEquation(gl.FUNC_ADD); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.activeTexture(gl.TEXTURE0);
    for (let index = 0; index < frame.draws.length; index++) {
      const source = frame.draws[index]!.texture;
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
      const texture = gl.createTexture();
      if (texture === null) throw new Error("WebGL2 texture allocation failed");
      try {
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.REPEAT);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.REPEAT);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, source.width, source.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.#vertices);
        gl.bufferData(gl.ARRAY_BUFFER, vertices[index]!, gl.STREAM_DRAW);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      } finally { gl.deleteTexture(texture); }
    }
    if (gl.getError() !== gl.NO_ERROR) throw new Error("WebGL2 game presentation failed");
  }
}
