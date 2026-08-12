import type { AudioPlaybackMessage, AudioStopMessage } from "./protocol.js";

export interface BrowserAudioPlaybackReceipt {
  bufferId: number;
  bufferGeneration: number;
  contentGeneration: number;
  frameCount: number;
  durationSeconds: number;
  peakAbsoluteSample: number;
  playback: AudioPlaybackMessage["playback"];
  contextState: AudioContextState;
}

export interface BrowserAudioStopReceipt {
  bufferId: number;
  bufferGeneration: number;
  contentGeneration: number;
  sourceStopped: boolean;
  staleMessageIgnored: boolean;
  contextState: AudioContextState | "uninitialized";
}

interface ActiveAudioSource {
  bufferGeneration: number;
  source: AudioBufferSourceNode;
}

export class BrowserPcmRenderer {
  #context: AudioContext | null = null;
  readonly #sources = new Map<number, ActiveAudioSource>();

  async play(message: AudioPlaybackMessage): Promise<BrowserAudioPlaybackReceipt> {
    const context = this.#context ?? new AudioContext({ latencyHint: "interactive" });
    this.#context = context;
    const resume = context.state === "suspended" ? context.resume() : Promise.resolve();
    const { format } = message;
    if (
      !Number.isInteger(format.channels) ||
      format.channels < 1 ||
      format.channels > 2 ||
      !Number.isInteger(format.samplesPerSecond) ||
      format.samplesPerSecond < 8_000 ||
      format.samplesPerSecond > 192_000 ||
      (format.bitsPerSample !== 8 && format.bitsPerSample !== 16) ||
      format.blockAlign !== (format.channels * format.bitsPerSample) / 8 ||
      format.averageBytesPerSecond !==
        format.samplesPerSecond * format.blockAlign ||
      message.samples.byteLength === 0 ||
      message.samples.byteLength % format.blockAlign !== 0
    ) {
      throw new Error("DirectSound playback message has an invalid PCM shape");
    }

    const frameCount = message.samples.byteLength / format.blockAlign;
    const audioBuffer = context.createBuffer(
      format.channels,
      frameCount,
      format.samplesPerSecond,
    );
    const samples = new DataView(message.samples);
    const bytesPerSample = format.bitsPerSample / 8;
    let peakAbsoluteSample = 0;
    for (let channel = 0; channel < format.channels; channel += 1) {
      const output = audioBuffer.getChannelData(channel);
      for (let frame = 0; frame < frameCount; frame += 1) {
        const offset = frame * format.blockAlign + channel * bytesPerSample;
        const sample =
          format.bitsPerSample === 8
            ? (samples.getUint8(offset) - 128) / 128
            : samples.getInt16(offset, true) / 32_768;
        output[frame] = sample;
        peakAbsoluteSample = Math.max(peakAbsoluteSample, Math.abs(sample));
      }
    }

    const previous = this.#sources.get(message.bufferId);
    if (previous !== undefined) {
      previous.source.stop();
      previous.source.disconnect();
    }
    const source = new AudioBufferSourceNode(context, {
      buffer: audioBuffer,
      loop: message.playback === "playing-looping",
      playbackRate:
        (message.controls.frequencyHz ?? format.samplesPerSecond) /
        format.samplesPerSecond,
    });
    const gain = new GainNode(context, {
      gain:
        message.controls.volumeHundredthsDb <= -10_000
          ? 0
          : 10 ** (message.controls.volumeHundredthsDb / 2_000),
    });
    const pan = new StereoPannerNode(context, {
      pan: message.controls.panHundredthsDb / 10_000,
    });
    source.connect(pan).connect(gain).connect(context.destination);
    this.#sources.set(message.bufferId, {
      bufferGeneration: message.bufferGeneration,
      source,
    });
    source.addEventListener(
      "ended",
      () => {
        if (this.#sources.get(message.bufferId)?.source === source) {
          this.#sources.delete(message.bufferId);
        }
        source.disconnect();
        pan.disconnect();
        gain.disconnect();
      },
      { once: true },
    );
    source.start(
      0,
      message.controls.currentPositionBytes /
        format.averageBytesPerSecond,
    );
    await resume;
    return {
      bufferId: message.bufferId,
      bufferGeneration: message.bufferGeneration,
      contentGeneration: message.contentGeneration,
      frameCount,
      durationSeconds: frameCount / format.samplesPerSecond,
      peakAbsoluteSample,
      playback: message.playback,
      contextState: context.state,
    };
  }

  stop(message: AudioStopMessage): BrowserAudioStopReceipt {
    const active = this.#sources.get(message.bufferId);
    const staleMessageIgnored =
      active !== undefined && message.bufferGeneration < active.bufferGeneration;
    if (active !== undefined && !staleMessageIgnored) {
      this.#sources.delete(message.bufferId);
      active.source.stop();
    }
    return {
      bufferId: message.bufferId,
      bufferGeneration: message.bufferGeneration,
      contentGeneration: message.contentGeneration,
      sourceStopped: active !== undefined && !staleMessageIgnored,
      staleMessageIgnored,
      contextState: this.#context?.state ?? "uninitialized",
    };
  }
}
