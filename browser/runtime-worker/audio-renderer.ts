import type { AudioPlaybackMessage, AudioStopMessage } from "./protocol.js";

export interface BrowserAudioPlaybackReceipt {
  bufferId: number;
  bufferGeneration: number;
  contentGeneration: number;
  frameCount: number;
  durationSeconds: number;
  peakAbsoluteSample: number;
  playback: AudioPlaybackMessage["playback"];
  sourceReplaced: boolean;
  contentUpdated: boolean;
  staleMessageIgnored: boolean;
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
  contentGeneration: number;
  playback: AudioPlaybackMessage["playback"];
  channels: number;
  samplesPerSecond: number;
  bitsPerSample: number;
  frameCount: number;
  node: AudioWorkletNode;
  gain: GainNode;
  pan: StereoPannerNode;
}

interface DecodedPcm {
  channels: Float32Array[];
  peakAbsoluteSample: number;
}

const workletName = "vine-direct-sound-stream";

export class BrowserPcmRenderer {
  #context: AudioContext | null = null;
  #moduleReady: Promise<void> | null = null;
  #operationTail: Promise<void> = Promise.resolve();
  readonly #sources = new Map<number, ActiveAudioSource>();
  readonly #latestStopGeneration = new Map<number, number>();

  play(message: AudioPlaybackMessage): Promise<BrowserAudioPlaybackReceipt> {
    const operation = this.#operationTail.then(() => this.#play(message));
    this.#operationTail = operation.then(
      () => undefined,
      () => undefined,
    );
    return operation;
  }

  async #play(
    message: AudioPlaybackMessage,
  ): Promise<BrowserAudioPlaybackReceipt> {
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
    const context = await this.#readyContext();
    // Context/worklet initialization can yield while a stop for this buffer is
    // delivered. Re-read authoritative generations only after that boundary so
    // a stopped source cannot be resurrected by an older queued play message.
    const previous = this.#sources.get(message.bufferId);
    const stoppedGeneration = this.#latestStopGeneration.get(message.bufferId);
    const staleMessageIgnored =
      (stoppedGeneration !== undefined &&
        message.bufferGeneration <= stoppedGeneration) ||
      (previous !== undefined &&
        (message.bufferGeneration < previous.bufferGeneration ||
          (message.bufferGeneration === previous.bufferGeneration &&
            message.contentGeneration <= previous.contentGeneration)));
    if (staleMessageIgnored) {
      return this.#receipt(message, frameCount, 0, false, false, true, context);
    }

    const decoded = decodePcm(message, frameCount);
    const canUpdate =
      previous !== undefined &&
      previous.playback === "playing-looping" &&
      message.playback === "playing-looping" &&
      previous.channels === format.channels &&
      previous.samplesPerSecond === format.samplesPerSecond &&
      previous.bitsPerSample === format.bitsPerSample &&
      previous.frameCount === frameCount;
    if (canUpdate) {
      this.#updateControls(previous, message);
      this.#postSamples(previous.node, "update", message, decoded.channels);
      previous.bufferGeneration = message.bufferGeneration;
      previous.contentGeneration = message.contentGeneration;
      return this.#receipt(
        message,
        frameCount,
        decoded.peakAbsoluteSample,
        false,
        true,
        false,
        context,
      );
    }

    if (previous !== undefined) {
      this.#retire(previous);
    }
    this.#latestStopGeneration.delete(message.bufferId);
    const gain = new GainNode(context, {
      gain: gainForVolume(message.controls.volumeHundredthsDb),
    });
    const pan = new StereoPannerNode(context, {
      pan: panForHundredthsDb(message.controls.panHundredthsDb),
    });
    const node = new AudioWorkletNode(context, workletName, {
      numberOfInputs: 0,
      numberOfOutputs: 1,
      outputChannelCount: [format.channels],
      channelCount: format.channels,
      channelCountMode: "explicit",
      channelInterpretation: "speakers",
    });
    node.connect(pan).connect(gain).connect(context.destination);
    const active: ActiveAudioSource = {
      bufferGeneration: message.bufferGeneration,
      contentGeneration: message.contentGeneration,
      playback: message.playback,
      channels: format.channels,
      samplesPerSecond: format.samplesPerSecond,
      bitsPerSample: format.bitsPerSample,
      frameCount,
      node,
      gain,
      pan,
    };
    this.#sources.set(message.bufferId, active);
    node.port.addEventListener("message", (event: MessageEvent<unknown>) => {
      if (
        isEndedMessage(event.data) &&
        this.#sources.get(message.bufferId)?.node === node
      ) {
        this.#sources.delete(message.bufferId);
        this.#disconnect(active);
      }
    });
    node.port.start();
    this.#postSamples(node, "start", message, decoded.channels);
    return this.#receipt(
      message,
      frameCount,
      decoded.peakAbsoluteSample,
      previous !== undefined,
      false,
      false,
      context,
    );
  }

  stop(message: AudioStopMessage): BrowserAudioStopReceipt {
    const previousStop = this.#latestStopGeneration.get(message.bufferId) ?? 0;
    if (message.bufferGeneration > previousStop) {
      this.#latestStopGeneration.set(message.bufferId, message.bufferGeneration);
    }
    const active = this.#sources.get(message.bufferId);
    const staleMessageIgnored =
      active !== undefined && message.bufferGeneration < active.bufferGeneration;
    if (active !== undefined && !staleMessageIgnored) {
      this.#sources.delete(message.bufferId);
      this.#retire(active);
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

  async #readyContext(): Promise<AudioContext> {
    const context =
      this.#context ?? new AudioContext({ latencyHint: "interactive" });
    this.#context = context;
    this.#moduleReady ??= context.audioWorklet.addModule(
      new URL("./direct-sound-worklet.js", import.meta.url),
    );
    await Promise.all([
      this.#moduleReady,
      context.state === "suspended" ? context.resume() : Promise.resolve(),
    ]);
    return context;
  }

  #postSamples(
    node: AudioWorkletNode,
    type: "start" | "update",
    message: AudioPlaybackMessage,
    channels: Float32Array[],
  ): void {
    const channelBuffers = channels.map((channel) => channel.buffer);
    node.port.postMessage(
      {
        type,
        channels: channelBuffers,
        frameCount: channels[0]?.length ?? 0,
        sourceSamplesPerSecond: message.format.samplesPerSecond,
        frequencyHz:
          message.controls.frequencyHz ?? message.format.samplesPerSecond,
        playback: message.playback,
        startFrame:
          message.controls.currentPositionBytes / message.format.blockAlign,
      },
      channelBuffers,
    );
  }

  #updateControls(
    active: ActiveAudioSource,
    message: AudioPlaybackMessage,
  ): void {
    const context = this.#context;
    if (context === null) {
      throw new Error("DirectSound browser context disappeared during playback");
    }
    active.gain.gain.setValueAtTime(
      gainForVolume(message.controls.volumeHundredthsDb),
      context.currentTime,
    );
    active.pan.pan.setValueAtTime(
      panForHundredthsDb(message.controls.panHundredthsDb),
      context.currentTime,
    );
  }

  #retire(active: ActiveAudioSource): void {
    active.node.port.postMessage({ type: "stop" });
    this.#disconnect(active);
  }

  #disconnect(active: ActiveAudioSource): void {
    active.node.disconnect();
    active.pan.disconnect();
    active.gain.disconnect();
    active.node.port.close();
  }

  #receipt(
    message: AudioPlaybackMessage,
    frameCount: number,
    peakAbsoluteSample: number,
    sourceReplaced: boolean,
    contentUpdated: boolean,
    staleMessageIgnored: boolean,
    context: AudioContext,
  ): BrowserAudioPlaybackReceipt {
    return {
      bufferId: message.bufferId,
      bufferGeneration: message.bufferGeneration,
      contentGeneration: message.contentGeneration,
      frameCount,
      durationSeconds: frameCount / message.format.samplesPerSecond,
      peakAbsoluteSample,
      playback: message.playback,
      sourceReplaced,
      contentUpdated,
      staleMessageIgnored,
      contextState: context.state,
    };
  }
}

function decodePcm(
  message: AudioPlaybackMessage,
  frameCount: number,
): DecodedPcm {
  const { format } = message;
  const input = new DataView(message.samples);
  const bytesPerSample = format.bitsPerSample / 8;
  const channels = Array.from(
    { length: format.channels },
    () => new Float32Array(frameCount),
  );
  let peakAbsoluteSample = 0;
  for (let channel = 0; channel < format.channels; channel += 1) {
    const output = channels[channel];
    if (output === undefined) {
      throw new Error("DirectSound PCM channel allocation failed");
    }
    for (let frame = 0; frame < frameCount; frame += 1) {
      const offset = frame * format.blockAlign + channel * bytesPerSample;
      const sample =
        format.bitsPerSample === 8
          ? (input.getUint8(offset) - 128) / 128
          : input.getInt16(offset, true) / 32_768;
      output[frame] = sample;
      peakAbsoluteSample = Math.max(peakAbsoluteSample, Math.abs(sample));
    }
  }
  return { channels, peakAbsoluteSample };
}

function gainForVolume(volumeHundredthsDb: number): number {
  return volumeHundredthsDb <= -10_000
    ? 0
    : 10 ** (volumeHundredthsDb / 2_000);
}

function panForHundredthsDb(panHundredthsDb: number): number {
  return Math.max(-1, Math.min(1, panHundredthsDb / 10_000));
}

function isEndedMessage(value: unknown): value is { type: "ended" } {
  return (
    typeof value === "object" &&
    value !== null &&
    "type" in value &&
    value.type === "ended"
  );
}
