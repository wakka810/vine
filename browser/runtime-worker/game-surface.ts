import { DEFAULT_TEXTURE_SAMPLING, type GamePresentation, texturePixels, validatePresentation } from "./game-presentation.js";
import { SoftwareGameSurface } from "./software-game-surface.js";
export type { GamePresentation } from "./game-presentation.js";

export class GameSurface {
  readonly backend: "webgl2" | "software";
  readonly #surface: WebGLGameSurface | SoftwareGameSurface;

  constructor(canvas: OffscreenCanvas, options: { allowSoftware?: boolean } = {}) {
    const gl = canvas.getContext("webgl2", {
      alpha: false, antialias: false, depth: true, stencil: false,
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
  readonly #fragment: WebGLUniformLocation;
  readonly #textureAlpha: WebGLUniformLocation;
  readonly #colorAdd: WebGLUniformLocation;
  readonly #fogEnabled: WebGLUniformLocation;
  readonly #fogColor: WebGLUniformLocation;
  readonly #fogRange: WebGLUniformLocation;
  readonly #freeTextures = new Map<string, WebGLTexture[]>();
  #maximum: number | null = null;
  #pooledTextureCount = 0;
  #hasPresented = false;

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
      out float eyeW;
      void main() {
        vec2 clip = (position.xy + vec2(0.5)) / resolution * 2.0 - 1.0;
        float w = 1.0 / position.w;
        gl_Position = vec4(clip.x * w, -clip.y * w, (position.z * 2.0 - 1.0) * w, w);
        uv = texcoord; tint = diffuse; eyeW = w;
      }`);
    const fragment = shader(gl.FRAGMENT_SHADER, `#version 300 es
      precision highp float;
      uniform sampler2D image;
      uniform bool bgrx;
      uniform bool textureAlpha;
      uniform bool colorAdd;
      uniform bool fogEnabled;
      uniform vec3 fogColor;
      uniform vec2 fogRange;
      uniform ivec4 fragmentState;
      in vec2 uv;
      in vec4 tint;
      in float eyeW;
      out vec4 pixel;
      void main() {
        vec4 sampled = texture(image, uv);
        float alpha = fragmentState.x != 0 && textureAlpha ? sampled.a : 1.0;
        if (fragmentState.y != 0) alpha *= tint.a;
        int value = int(floor(clamp(alpha,0.0,1.0)*255.0+0.5));
        int ref = fragmentState.w;
        bool pass = fragmentState.z == 8 ||
          (fragmentState.z == 2 && value < ref) || (fragmentState.z == 3 && value == ref) ||
          (fragmentState.z == 4 && value <= ref) || (fragmentState.z == 5 && value > ref) ||
          (fragmentState.z == 6 && value != ref) || (fragmentState.z == 7 && value >= ref);
        if (!pass) discard;
        vec3 baseRgb = bgrx ? sampled.bgr : sampled.rgb;
        vec3 litRgb = colorAdd ? min(baseRgb + tint.rgb, vec3(1.0)) : baseRgb * tint.rgb;
        if (fogEnabled) {
          float denom = fogRange.y - fogRange.x;
          float f = abs(denom) > 1e-6 ? clamp((fogRange.y - eyeW) / denom, 0.0, 1.0) : (eyeW <= fogRange.y ? 1.0 : 0.0);
          litRgb = mix(fogColor, litRgb, f);
        }
        pixel = vec4(litRgb, alpha);
        gl_FragDepth = floor(gl_FragCoord.z * 65535.0 + 0.5) / 65535.0;
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
    const fragmentUniform = gl.getUniformLocation(program, "fragmentState");
    const textureAlpha = gl.getUniformLocation(program, "textureAlpha");
    const colorAdd = gl.getUniformLocation(program, "colorAdd");
    const fogEnabled = gl.getUniformLocation(program, "fogEnabled");
    const fogColor = gl.getUniformLocation(program, "fogColor");
    const fogRange = gl.getUniformLocation(program, "fogRange");
    if (vertices === null || layout === null || resolution === null || bgrx === null || fragmentUniform === null || textureAlpha === null || colorAdd === null || fogEnabled === null || fogColor === null || fogRange === null) {
      throw new Error("WebGL2 quad resources are unavailable");
    }
    this.#program = program; this.#vertices = vertices;
    this.#layout = layout; this.#resolution = resolution;
    this.#bgrx = bgrx; this.#fragment = fragmentUniform; this.#textureAlpha = textureAlpha;
    this.#colorAdd = colorAdd; this.#fogEnabled = fogEnabled; this.#fogColor = fogColor; this.#fogRange = fogRange;
    gl.bindVertexArray(layout); gl.bindBuffer(gl.ARRAY_BUFFER, vertices);
    for (const [index, size, offset] of [[0, 4, 0], [1, 2, 16], [2, 4, 24]]) {
      gl.enableVertexAttribArray(index!);
      gl.vertexAttribPointer(index!, size!, gl.FLOAT, false, 40, offset!);
    }
    gl.useProgram(program);
    gl.uniform1i(gl.getUniformLocation(program, "image"), 0);
  }

  #releaseTexture(sizeKey: string, texture: WebGLTexture): void {
    if (this.#pooledTextureCount < 16) {
      let bucket = this.#freeTextures.get(sizeKey);
      if (bucket === undefined) {
        bucket = [];
        this.#freeTextures.set(sizeKey, bucket);
      }
      bucket.push(texture);
      this.#pooledTextureCount++;
    } else {
      this.#gl.deleteTexture(texture);
    }
  }

  present(frame: GamePresentation, payload: Uint8Array = new Uint8Array(0)): void {
    const gl = this.#gl;
    if (gl.isContextLost()) throw new Error("The game WebGL2 context was lost");
    const maximum = (this.#maximum ??= Math.min(gl.getParameter(gl.MAX_TEXTURE_SIZE), gl.getParameter(gl.MAX_RENDERBUFFER_SIZE)));
    const vertices = validatePresentation(frame, payload, maximum);
    const resized = this.#canvas.width !== frame.width || this.#canvas.height !== frame.height;
    if (this.#canvas.width !== frame.width) this.#canvas.width = frame.width;
    if (this.#canvas.height !== frame.height) this.#canvas.height = frame.height;
    gl.viewport(0, 0, frame.width, frame.height);
    gl.disable(gl.DEPTH_TEST); gl.disable(gl.CULL_FACE); gl.disable(gl.SCISSOR_TEST);
    const color = frame.colorArgb;
    const colorClearAt = color === null ? -1 : frame.colorClearBeforeDraw ?? 0;
    const reset = !frame.preserve || resized || !this.#hasPresented;
    const initialColor = reset && colorClearAt === 0;
    const setClearColor = (argb: number) => gl.clearColor(
      ((argb >>> 16) & 255) / 255, ((argb >>> 8) & 255) / 255, (argb & 255) / 255, 1,
    );
    if (reset) {
      setClearColor(initialColor ? color! : 0);
      gl.depthMask(true);
      gl.clearDepth(1);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }
    this.#hasPresented = true;
    gl.useProgram(this.#program); gl.bindVertexArray(this.#layout);
    gl.uniform2f(this.#resolution, frame.width, frame.height);
    gl.enable(gl.BLEND); gl.blendEquation(gl.FUNC_ADD); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.activeTexture(gl.TEXTURE0);
    let depthClearIndex = 0;
    const cachedTextures = new Map<string, { texture: WebGLTexture; bgrx: boolean; sizeKey: string }>();
    try {
      for (let index = 0; index <= frame.draws.length; index++) {
        if (color !== null && colorClearAt === index && !initialColor) {
          gl.disable(gl.SCISSOR_TEST);
          setClearColor(color);
          gl.clear(gl.COLOR_BUFFER_BIT);
        }
        while (frame.depthClears?.[depthClearIndex]?.beforeDraw === index) {
          gl.disable(gl.SCISSOR_TEST);
          gl.depthMask(true); gl.clearDepth(Math.round(frame.depthClears[depthClearIndex++]!.value * 65535) / 65535); gl.clear(gl.DEPTH_BUFFER_BIT);
        }
        if (index === frame.draws.length) break;
        const depth = frame.draws[index]!.depth;
        const clip = frame.draws[index]!.clip;
        if (clip) {
          gl.enable(gl.SCISSOR_TEST);
          gl.scissor(clip.left, frame.height - clip.bottom, clip.right - clip.left, clip.bottom - clip.top);
        } else gl.disable(gl.SCISSOR_TEST);
        if (depth) {gl.enable(gl.DEPTH_TEST); gl.depthMask(depth.write); gl.depthFunc([gl.NEVER,gl.LESS,gl.EQUAL,gl.LEQUAL,gl.GREATER,gl.NOTEQUAL,gl.GEQUAL,gl.ALWAYS][depth.compare - 1]!);}
        else {gl.disable(gl.DEPTH_TEST); gl.depthMask(false);}
        const fragment = frame.draws[index]!.fragment;
        if (fragment?.blend) {
          gl.enable(gl.BLEND);
          gl.blendFunc(gl.SRC_ALPHA, fragment.additive ? gl.ONE : gl.ONE_MINUS_SRC_ALPHA);
        } else {
          gl.disable(gl.BLEND);
        }
        gl.uniform4i(this.#fragment, Number(fragment?.textureAlpha ?? false), Number(fragment?.diffuseAlpha ?? false), fragment?.alphaTest?.compare ?? 8, fragment?.alphaTest?.reference ?? 0);
        gl.uniform1i(this.#colorAdd, Number(fragment?.colorAdd ?? false));
        const fog = fragment?.fog ?? null;
        gl.uniform1i(this.#fogEnabled, Number(fog !== null));
        if (fog !== null) {
          gl.uniform3f(this.#fogColor, ((fog.colorArgb >>> 16) & 255) / 255, ((fog.colorArgb >>> 8) & 255) / 255, (fog.colorArgb & 255) / 255);
          gl.uniform2f(this.#fogRange, fog.start, fog.end);
        }
        const source = frame.draws[index]!.texture;
        gl.uniform1i(this.#textureAlpha, Number(source.alpha ?? false));
        const sampling = frame.draws[index]!.sampling ?? DEFAULT_TEXTURE_SAMPLING;
        const snapshot = source.pixels;
        const cacheKey = snapshot !== null && source.fills.length === 0
          ? `${snapshot.offset}:${snapshot.length}:${snapshot.pitch}:${source.width}:${source.height}:${Boolean(source.alpha)}`
          : null;
        let cached = cacheKey !== null ? cachedTextures.get(cacheKey) ?? null : null;
        let ephemeral: { texture: WebGLTexture; sizeKey: string } | null = null;
        try {
          if (cached === null) {
            // Undefined X8R8G8B8 texels and black fills are opaque black at every coordinate.
            const black = source.alpha !== true && snapshot === null && source.fills.every(fill => (fill.colorRef & 0xffffff) === 0);
            const direct = snapshot !== null && source.fills.length === 0 && snapshot.pitch % 4 === 0;
            // Upload consumes this validated, borrowed snapshot synchronously before the Wasm acknowledgement.
            // X8R8G8B8 channel order and opacity are applied after filtering in the shader.
            const pixels = black ? new Uint8Array([0, 0, 0, 255]) :
              direct ? payload.subarray(snapshot.offset, snapshot.offset + snapshot.length) : texturePixels(source, payload);
            const texWidth = black ? 1 : source.width;
            const texHeight = black ? 1 : source.height;
            const sizeKey = `${texWidth}x${texHeight}`;
            const bucket = this.#freeTextures.get(sizeKey);
            let reused = false;
            let texture: WebGLTexture | null = null;
            if (bucket !== undefined && bucket.length > 0) {
              texture = bucket.pop()!;
              this.#pooledTextureCount--;
              reused = true;
            } else {
              texture = gl.createTexture();
              if (texture === null) throw new Error("WebGL2 texture allocation failed");
            }
            if (cacheKey !== null) {
              cached = { texture, bgrx: direct, sizeKey };
              cachedTextures.set(cacheKey, cached);
            } else {
              ephemeral = { texture, sizeKey };
              cached = { texture, bgrx: direct, sizeKey };
            }
            gl.bindTexture(gl.TEXTURE_2D, texture);
            gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
            gl.pixelStorei(gl.UNPACK_ROW_LENGTH, direct ? snapshot!.pitch / 4 : 0);
            if (reused) {
              gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, texWidth, texHeight, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
            } else {
              gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, texWidth, texHeight, 0, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
            }
            gl.pixelStorei(gl.UNPACK_ROW_LENGTH, 0);
            gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
          } else {
            gl.bindTexture(gl.TEXTURE_2D, cached.texture);
          }
          gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, sampling.minFilter === "point" ? gl.NEAREST : gl.LINEAR);
          gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, sampling.magFilter === "point" ? gl.NEAREST : gl.LINEAR);
          gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, sampling.addressU === "clamp" ? gl.CLAMP_TO_EDGE : gl.REPEAT);
          gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, sampling.addressV === "clamp" ? gl.CLAMP_TO_EDGE : gl.REPEAT);
          gl.uniform1i(this.#bgrx, cached.bgrx ? 1 : 0);
          gl.bindBuffer(gl.ARRAY_BUFFER, this.#vertices);
          gl.bufferData(gl.ARRAY_BUFFER, vertices[index]!, gl.STREAM_DRAW);
          gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
        } finally {
          if (ephemeral !== null) this.#releaseTexture(ephemeral.sizeKey, ephemeral.texture);
        }
      }
    } finally {
      gl.pixelStorei(gl.UNPACK_ROW_LENGTH, 0);
      gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
      for (const entry of cachedTextures.values()) this.#releaseTexture(entry.sizeKey, entry.texture);
    }
    if (gl.isContextLost()) throw new Error("The game WebGL2 context was lost");
  }
}
