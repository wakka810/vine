import type LibAV from "@libav.js/types";

export interface DecodedMediaFormat {
  width: number;
  height: number;
  duration100ns: number;
  frameDuration100ns: number;
  audioChannels: number;
  audioSamplesPerSecond: number;
}

export type DecodedMediaSample = {
  kind: "frame";
  start100ns: number;
  end100ns: number;
  bytes: Uint8Array;
} | {
  kind: "audio";
  start100ns: number;
  end100ns: number;
  channels: number;
  samplesPerSecond: number;
  bytes: Uint8Array;
};

type DecoderStream = {
  stream: LibAV.Stream;
  context: number;
  packet: number;
  frame: number;
  queued: Array<{ pointer: number; start: bigint; end: bigint }>;
  nextTime: bigint | null;
  videoPictures: bigint;
};

type SelectedFrame = { state: DecoderStream; item: DecoderStream["queued"][number] };

const noTimestamp = -(1n << 63n);
const ticksPerSecond = 10_000_000n;

export class UnsupportedMediaFormat extends Error {}

function signed64(low: number, high: number): bigint {
  return BigInt.asIntN(64, BigInt(low >>> 0) | (BigInt(high >>> 0) << 32n));
}

function safeTicks(ticks: bigint): number {
  const value = Number(ticks);
  if (ticks < 0n || !Number.isSafeInteger(value)) throw new Error("Media timestamp exceeds the browser boundary");
  return value;
}

// This owns codec resources only. The read callback obtains bytes through the
// live Rust source/guest IAsyncReader handoff; it never receives a guest pointer.
export class MediaDecoder {
  #av: LibAV.LibAV | null;
  readonly #read: (offset: number, length: number) => Promise<Uint8Array>;
  readonly #byteLength: number;
  #formatContext = 0;
  #packet = 0;
  #streams: DecoderStream[] = [];
  #rawVideo = false;
  #eof = false;
  #busy = false;
  #origin: bigint | null = null;
  #scaler = 0;
  #scaledFrame = 0;
  #audioFilter: [number, number, number] | null = null;
  #audioFilterFrame = 0;
  #audioDrained = false;
  #format: DecodedMediaFormat | null = null;
  #readerCreated = false;
  #sourceFailure: unknown = null;

  private constructor(av: LibAV.LibAV, byteLength: number,
    read: (offset: number, length: number) => Promise<Uint8Array>) {
    this.#av = av;
    this.#read = read;
    this.#byteLength = byteLength;
  }

  static async open(byteLength: number,
    read: (offset: number, length: number) => Promise<Uint8Array>,
    moduleUrl = new URL("./libav-media/libav-6.10.9.0-vine-mpeg.mjs", import.meta.url),
  ): Promise<MediaDecoder> {
    if (!Number.isSafeInteger(byteLength) || byteLength < 4) throw new Error("Invalid encoded media length");
    const module = await import(moduleUrl.href) as { default: LibAV.LibAVWrapper };
    const av = await module.default.LibAV({ noworker: true });
    const decoder = new MediaDecoder(av, byteLength, read);
    try {
      await av.av_log_set_level(16);
      const prefix = await decoder.#readRange(0, Math.min(32, byteLength));
      let code: number | null = null;
      for (let index = 0; index + 3 < prefix.length; index += 1) {
        if (prefix[index] === 0 && prefix[index + 1] === 0 && prefix[index + 2] === 1) {
          code = prefix[index + 3]!;
          break;
        }
      }
      if (code !== 0xb3 && code !== 0xba) throw new UnsupportedMediaFormat("Encoded media is not MPEG-1 ES or program stream");
      decoder.#rawVideo = code === 0xb3;
      await av.mkblockreaderdev("input", byteLength);
      decoder.#readerCreated = true;
      av.onblockread = async (name, offset, requested) => {
        // Read ahead in one bounded handoff so FFmpeg's 32KiB AVIO requests do
        // not each require a new guest callback. The block device retains only
        // this current range, and Rust verifies every requested range.
        if (name !== "input" || !Number.isSafeInteger(requested) || requested < 1) {
          throw new Error("Invalid decoder source request");
        }
        const length = Math.min(2 * 1024 * 1024, Math.max(requested, 1024 * 1024));
        const bytes = await decoder.#readRange(offset, Math.min(length, byteLength - offset));
        await av.ff_block_reader_dev_send(name, offset, bytes);
      };
      await decoder.#initialize();
      let duration = 0n;
      // Windows asks for duration before Run. Establish it from actual decoded
      // sample timestamps and EOF, never FFmpeg's raw-video bitrate estimate.
      while (true) {
        const selected = await decoder.#nextPointer();
        if (selected === null) break;
        const { item } = selected;
        duration = item.end - decoder.#origin! > duration ? item.end - decoder.#origin! : duration;
        await av.av_frame_free_js(item.pointer);
      }
      if (decoder.#format === null || duration <= 0n) throw new Error("Encoded media contains no decoded video samples");
      decoder.#format.duration100ns = safeTicks(duration);
      await decoder.#freeDemuxer();
      await decoder.#initialize();
      return decoder;
    } catch (error) {
      await decoder.close();
      throw error;
    }
  }

  get format(): DecodedMediaFormat {
    if (this.#format === null) throw new Error("Media decoder has no validated format");
    return { ...this.#format };
  }

  async #readRange(offset: number, length: number): Promise<Uint8Array> {
    if (!Number.isSafeInteger(offset) || offset < 0 || offset > this.#byteLength ||
        !Number.isInteger(length) || length < 0 || length > 2 * 1024 * 1024 ||
        length > this.#byteLength - offset) throw new Error("Decoder read exceeds the encoded source");
    if (length === 0) return new Uint8Array(0);
    try {
      const bytes = await this.#read(offset, length);
      if (!(bytes instanceof Uint8Array) || bytes.byteLength !== length) {
        throw new Error("Encoded source completion disagrees with its range");
      }
      return bytes;
    } catch (error) {
      // FFmpeg reports ECANCELED as a generic open/read error. Keep the actual
      // source completion failure, including an original capability failure.
      this.#sourceFailure = error;
      throw error;
    }
  }

  async #initialize(): Promise<void> {
    const av = this.#requireAv();
    let context: number, streams: LibAV.Stream[];
    try { [context, streams] = await av.ff_init_demuxer_file("input", this.#rawVideo ? "mpegvideo" : "mpeg"); }
    catch (error) { throw this.#sourceFailure ?? error; }
    this.#formatContext = context;
    if (streams.length < 1 || streams.length > 2 ||
        streams.filter(stream => stream.codec_type === 0 && stream.codec_id === 1).length !== 1 ||
        streams.some(stream => !(stream.codec_type === 0 && stream.codec_id === 1) &&
          !(stream.codec_type === 1 && stream.codec_id === 0x15000))) {
      throw new UnsupportedMediaFormat("Media streams require an unsupported codec");
    }
    this.#packet = await av.av_packet_alloc();
    if (this.#packet === 0) throw new Error("Decoder packet allocation failed");
    this.#streams = [];
    for (const stream of streams) {
      const [, codecContext, packet, frame] = await av.ff_init_decoder(stream.codec_id,
        { codecpar: stream.codecpar, time_base: [stream.time_base_num, stream.time_base_den] });
      // Actual syntax/sample defects must be decode failures rather than
      // concealed frames from FFmpeg's default error-concealment path.
      if (await av.av_opt_set(codecContext, "err_detect", "explode", 0) < 0) {
        await av.ff_free_decoder(codecContext, packet, frame);
        throw new Error("MPEG decoder could not enable explicit syntax failures");
      }
      this.#streams.push({ stream, context: codecContext, packet, frame,
        queued: [], nextTime: null, videoPictures: 0n });
    }
    this.#eof = false;
    this.#origin = null;
  }

  #requireAv(): LibAV.LibAV {
    if (this.#av === null) throw new Error("Media decoder was closed");
    return this.#av;
  }

  async #nextPointer(): Promise<SelectedFrame | null> {
    const av = this.#requireAv();
    while (!this.#eof && this.#streams.some(stream => stream.queued.length === 0)) {
      let result: number, packets: Record<number, LibAV.Packet[]>;
      try { [result, packets] = await av.ff_read_frame_multi(this.#formatContext, this.#packet, { limit: 1 }); }
      catch (error) { throw this.#sourceFailure ?? error; }
      if (result === av.AVERROR_EOF) this.#eof = true;
      else if (result !== 0 && result !== -av.EAGAIN) throw new Error(`MPEG demux failed (${result})`);
      for (const state of this.#streams) {
        const pointers = await av.ff_decode_multi(state.context, state.packet, state.frame,
          packets[state.stream.index] ?? [], { fin: this.#eof, copyoutFrame: "ptr" });
        try {
          for (const pointer of pointers) {
            const item = await this.#describeFrame(state, pointer);
            if (state.queued.length >= 64) throw new Error("Media stream interleave exceeds the bounded decoder queue");
            state.queued.push(item);
          }
        } catch (error) {
          for (const pointer of pointers) {
            if (!state.queued.some(item => item.pointer === pointer)) await av.av_frame_free_js(pointer);
          }
          throw error;
        }
      }
    }
    const active = this.#streams.filter(stream => stream.queued.length !== 0)
      .sort((left, right) => left.queued[0]!.start < right.queued[0]!.start ? -1 : 1);
    const state = active[0];
    if (state === undefined) return null;
    const item = state.queued.shift()!;
    if (this.#origin === null) this.#origin = item.start;
    if (item.start < this.#origin) {
      await av.av_frame_free_js(item.pointer);
      throw new Error("Decoded media timestamps precede their established origin");
    }
    return { state, item };
  }

  async #describeFrame(state: DecoderStream, pointer: number): Promise<DecoderStream["queued"][number]> {
    const av = this.#requireAv();
    const num = BigInt(state.stream.time_base_num);
    const den = BigInt(state.stream.time_base_den);
    if (num <= 0n || den <= 0n) throw new Error("Invalid decoded media time base");
    const stamp = signed64(await av.AVFrame_best_effort_timestamp(pointer),
      await av.AVFrame_best_effort_timestamphi(pointer));
    let start: bigint;
    let end: bigint;
    if (state.stream.codec_type === 0) {
      const width = await av.AVFrame_width(pointer);
      const height = await av.AVFrame_height(pointer);
      const duration = BigInt(await av.AVFrame_duration(pointer));
      if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 ||
          width > 4096 || height > 4096 || width * height * 4 > 16 * 1024 * 1024 || duration <= 0n) {
        throw new Error("Unsupported decoded video dimensions or cadence");
      }
      if (this.#rawVideo) {
        start = state.videoPictures * duration * num * ticksPerSecond / den;
        end = (state.videoPictures + 1n) * duration * num * ticksPerSecond / den;
      } else {
        start = stamp === noTimestamp ? state.nextTime ?? 0n : stamp * num * ticksPerSecond / den;
        end = start + duration * num * ticksPerSecond / den;
      }
      state.videoPictures += 1n;
      const frameDuration = safeTicks(end - start);
      if (this.#format === null) {
        this.#format = { width, height, duration100ns: 0, frameDuration100ns: frameDuration,
          audioChannels: this.#streams.some(stream => stream.stream.codec_type === 1) ? 2 : 0,
          audioSamplesPerSecond: this.#streams.some(stream => stream.stream.codec_type === 1) ? 48000 : 0 };
      } else if (this.#format.width !== width || this.#format.height !== height ||
                 Math.abs(this.#format.frameDuration100ns - frameDuration) > 1) {
        throw new Error("Decoded video changed its negotiated format");
      }
    } else {
      const samples = await av.AVFrame_nb_samples(pointer);
      const rate = await av.AVFrame_sample_rate(pointer);
      const channels = await av.AVFrame_channels(pointer);
      if (!Number.isInteger(samples) || samples < 1 || samples > 65536 ||
          rate !== 48000 || channels !== 2) {
        throw new UnsupportedMediaFormat("Decoded MP2 audio requires 48000 Hz stereo");
      }
      if (stamp === noTimestamp && state.nextTime === null) throw new Error("MP2 audio has no initial presentation timestamp");
      start = stamp === noTimestamp ? state.nextTime! : stamp * num * ticksPerSecond / den;
      end = start + BigInt(samples) * ticksPerSecond / BigInt(rate);
    }
    if (state.nextTime !== null && start < state.nextTime - 1n) throw new Error("Decoded media timestamp regressed");
    if (end <= start) throw new Error("Decoded media has an empty presentation interval");
    state.nextTime = end;
    return { pointer, start, end };
  }

  async next(): Promise<DecodedMediaSample | null> {
    if (this.#busy) throw new Error("Media decoder already has a pending operation");
    const av = this.#requireAv();
    this.#busy = true;
    let selected: SelectedFrame | null = null;
    try {
      selected = await this.#nextPointer();
      if (selected === null) {
        if (this.#audioFilter !== null && !this.#audioDrained) {
          const [, input, output] = this.#audioFilter;
          const remaining = await av.ff_filter_multi(input, output, this.#audioFilterFrame, [], true);
          // Negotiated48k stereo changes sample layout only. Verify that the
          // actual filter has no delayed samples before reporting codec EOF.
          if (remaining.length !== 0) throw new Error("MP2 conversion retained unexpected samples at EOF");
          this.#audioDrained = true;
        }
        return null;
      }
      const { state, item } = selected;
      const start100ns = safeTicks(item.start - this.#origin!);
      const end100ns = safeTicks(item.end - this.#origin!);
      if (state.stream.codec_type === 0) {
        const format = this.format;
        if (this.#scaler === 0) {
          this.#scaler = await av.sws_getContext(format.width, format.height, await av.AVFrame_format(item.pointer),
            format.width, format.height, av.AV_PIX_FMT_BGRA, 2, 0, 0, 0);
          this.#scaledFrame = await av.av_frame_alloc();
          if (this.#scaler === 0 || this.#scaledFrame === 0) throw new Error("Video conversion allocation failed");
          await av.AVFrame_format_s(this.#scaledFrame, av.AV_PIX_FMT_BGRA);
          await av.AVFrame_width_s(this.#scaledFrame, format.width);
          await av.AVFrame_height_s(this.#scaledFrame, format.height);
        }
        if (await av.sws_scale_frame(this.#scaler, this.#scaledFrame, item.pointer) < 0) {
          throw new Error("MPEG RGB32 conversion failed");
        }
        const packed = await av.ff_copyout_frame_video_packed(this.#scaledFrame);
        if (!(packed.data instanceof Uint8Array) || packed.data.length !== format.width * format.height * 4) {
          throw new Error("RGB32 decoder output has an invalid length");
        }
        return { kind: "frame", start100ns, end100ns, bytes: packed.data };
      }
      const source = await av.ff_copyout_frame(item.pointer);
      if (this.#audioFilter === null) {
        this.#audioFilter = await av.ff_init_filter_graph("aformat=sample_fmts=flt:sample_rates=48000:channel_layouts=stereo",
          { sample_rate: source.sample_rate, sample_fmt: source.format,
            channel_layout: source.channels === 1 ? 4 : 3,
            time_base: [state.stream.time_base_num, state.stream.time_base_den] },
          { sample_rate: 48000, sample_fmt: av.AV_SAMPLE_FMT_FLT, channel_layout: 3 });
        this.#audioFilterFrame = await av.av_frame_alloc();
        if (this.#audioFilterFrame === 0) throw new Error("Audio conversion allocation failed");
      }
      const [, input, output] = this.#audioFilter;
      const converted = await av.ff_filter_multi(input, output, this.#audioFilterFrame, [source]);
      if (converted.length !== 1 || !(converted[0]!.data instanceof Float32Array) ||
          converted[0]!.channels !== 2 || converted[0]!.sample_rate !== 48000) {
        throw new Error("MP2 PCM conversion did not produce its negotiated sample");
      }
      const samples = converted[0]!.data as Float32Array;
      if (samples.length === 0 || samples.byteLength > 512 * 1024 || samples.some(value => !Number.isFinite(value))) {
        throw new Error("Decoded MP2 PCM exceeds its bounded format");
      }
      return { kind: "audio", start100ns, end100ns, channels: 2, samplesPerSecond: 48000,
        bytes: new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength) };
    } finally {
      if (selected !== null) await av.av_frame_free_js(selected.item.pointer);
      this.#busy = false;
    }
  }

  async #freeDemuxer(): Promise<void> {
    const av = this.#requireAv();
    for (const state of this.#streams) {
      for (const item of state.queued) await av.av_frame_free_js(item.pointer);
      await av.ff_free_decoder(state.context, state.packet, state.frame);
    }
    this.#streams = [];
    if (this.#packet !== 0) await av.av_packet_free_js(this.#packet);
    this.#packet = 0;
    if (this.#formatContext !== 0) await av.avformat_close_input_js(this.#formatContext);
    this.#formatContext = 0;
  }

  async close(): Promise<void> {
    const av = this.#av;
    if (av === null) return;
    try {
      await this.#freeDemuxer();
      if (this.#scaler !== 0) await av.sws_freeContext(this.#scaler);
      if (this.#scaledFrame !== 0) await av.av_frame_free_js(this.#scaledFrame);
      if (this.#audioFilter !== null) await av.avfilter_graph_free_js(this.#audioFilter[0]);
      if (this.#audioFilterFrame !== 0) await av.av_frame_free_js(this.#audioFilterFrame);
      if (this.#readerCreated) await av.unlink("input");
    } finally {
      this.#av = null;
      av.terminate();
    }
  }
}
