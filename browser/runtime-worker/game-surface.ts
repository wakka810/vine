import { type GamePresentation, texturePixels, validatePresentation } from "./game-presentation.js";
import { SoftwareGameSurface } from "./software-game-surface.js";
export type { GamePresentation } from "./game-presentation.js";

export class GameSurface {
  readonly backend: "webgl2" | "software";
  readonly #surface: WebGLGameSurface | SoftwareGameSurface;

  constructor(canvas: OffscreenCanvas, options: { allowSoftware?: boolean } = {}) {
    const gl = canvas.getContext("webgl2", {
      alpha: false, antialias: false, depth: false, stencil: false,
      preserveDrawingBuffer: true,
    });
    if (gl !== null) {
      this.backend = "webgl2";
      this.#surface = new WebGLGameSurface(canvas, gl);
    } else if (options.allowSoftware === true) {
      // The caller opts in only for the development source-server path.
      this.backend = "software";
      this.#surface = new SoftwareGameSurface(canvas);
    } else {
      throw new Error("WebGL2 is unavailable for the game surface");
    }
  }

  present(frame: GamePresentation, payload: Uint8Array = new Uint8Array(0)): void {
    this.#surface.present(frame, payload);
  }
}

// Browser resources only. Windows devices, render state and draw snapshots belong to Rust.
class WebGLGameSurface {
  readonly #canvas: OffscreenCanvas;
  readonly #gl: WebGL2RenderingContext;
  readonly #program: WebGLProgram;
  readonly #vertices: WebGLBuffer;
  readonly #layout: WebGLVertexArrayObject;
  readonly #resolution: WebGLUniformLocation;
  readonly #bgrx: WebGLUniformLocation;

  constructor(canvas: OffscreenCanvas, gl: WebGL2RenderingContext) {
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
      uniform bool bgrx;
      in vec2 uv;
      in vec4 tint;
      out vec4 pixel;
      void main() {
        vec4 sampled = texture(image, uv);
        pixel = vec4((bgrx ? sampled.bgr : sampled.rgb) * tint.rgb, 1.0);
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
    const bgrx = gl.getUniformLocation(program, "bgrx");
    if (vertices === null || layout === null || resolution === null || bgrx === null) {
      throw new Error("WebGL2 quad resources are unavailable");
    }
    this.#program = program; this.#vertices = vertices;
    this.#layout = layout; this.#resolution = resolution;
    this.#bgrx = bgrx;
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
    if (gl.isContextLost()) throw new Error("The game WebGL2 context was lost");
    const maximum = Math.min(gl.getParameter(gl.MAX_TEXTURE_SIZE), gl.getParameter(gl.MAX_RENDERBUFFER_SIZE));
    const vertices = validatePresentation(frame, payload, maximum);
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
      const snapshot = source.pixels;
      const direct = snapshot !== null && source.fills.length === 0 && snapshot.pitch % 4 === 0;
      // Upload consumes this validated, borrowed snapshot synchronously before the Wasm acknowledgement.
      // X8R8G8B8 channel order and opacity are applied after filtering in the shader.
      const pixels = direct ? payload.subarray(snapshot.offset, snapshot.offset + snapshot.length) : texturePixels(source, payload);
      const texture = gl.createTexture();
      if (texture === null) throw new Error("WebGL2 texture allocation failed");
      try {
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.REPEAT);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.REPEAT);
        gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
        gl.pixelStorei(gl.UNPACK_ROW_LENGTH, direct ? snapshot.pitch / 4 : 0);
        gl.uniform1i(this.#bgrx, direct ? 1 : 0);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, source.width, source.height, 0, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
        gl.bindBuffer(gl.ARRAY_BUFFER, this.#vertices);
        gl.bufferData(gl.ARRAY_BUFFER, vertices[index]!, gl.STREAM_DRAW);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      } finally {
        gl.pixelStorei(gl.UNPACK_ROW_LENGTH, 0);
        gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
        gl.deleteTexture(texture);
      }
    }
    if (gl.getError() !== gl.NO_ERROR) throw new Error("WebGL2 game presentation failed");
  }
}
