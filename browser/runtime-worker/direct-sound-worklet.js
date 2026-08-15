"use strict";

class VineDirectSoundStreamProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.channels = [];
    this.frameCount = 0;
    this.frequencyHz = sampleRate;
    this.playback = "stopped";
    this.position = 0;
    this.ended = false;
    // AudioWorklet may render before the main thread's first start message.
    // Keep the processor alive through that initial silent quantum.
    this.alive = true;
    this.port.onmessage = (event) => this.receive(event.data);
  }

  receive(message) {
    if (message === null || typeof message !== "object") {
      return;
    }
    if (message.type === "stop") {
      this.playback = "stopped";
      this.alive = false;
      return;
    }
    if (message.type !== "start" && message.type !== "update") {
      return;
    }
    if (
      !Array.isArray(message.channels) ||
      message.channels.length < 1 ||
      message.channels.length > 2 ||
      !Number.isInteger(message.frameCount) ||
      message.frameCount < 1 ||
      !Number.isFinite(message.frequencyHz) ||
      message.frequencyHz <= 0 ||
      (message.playback !== "playing-once" &&
        message.playback !== "playing-looping")
    ) {
      this.playback = "stopped";
      this.alive = false;
      return;
    }
    const channels = message.channels.map((buffer) => new Float32Array(buffer));
    const regionOffsetFrames = message.regionOffsetFrames;
    if (
      !Number.isInteger(regionOffsetFrames) ||
      regionOffsetFrames < 0 ||
      channels.some((channel) => channel.length < 1) ||
      (message.type === "start" &&
        (regionOffsetFrames !== 0 ||
          channels.some((channel) => channel.length !== message.frameCount))) ||
      (message.type === "update" &&
        (message.frameCount !== this.frameCount ||
          channels.length !== this.channels.length ||
          channels.some(
            (channel) => regionOffsetFrames + channel.length > this.frameCount,
          )))
    ) {
      this.playback = "stopped";
      this.alive = false;
      return;
    }
    if (message.type === "start") {
      this.channels = channels;
      this.frameCount = message.frameCount;
    } else {
      for (let channelIndex = 0; channelIndex < channels.length; channelIndex += 1) {
        this.channels[channelIndex].set(channels[channelIndex], regionOffsetFrames);
      }
    }
    this.frequencyHz = message.frequencyHz;
    this.playback = message.playback;
    this.alive = true;
    if (message.type === "start") {
      this.position = Number.isFinite(message.startFrame)
        ? Math.max(0, Math.min(message.frameCount - 1, message.startFrame))
        : 0;
      this.ended = false;
    } else if (this.position >= this.frameCount) {
      this.position %= this.frameCount;
    }
  }

  process(_inputs, outputs) {
    const output = outputs[0];
    if (output === undefined) {
      return this.alive;
    }
    for (const channel of output) {
      channel.fill(0);
    }
    if (
      this.playback === "stopped" ||
      this.frameCount === 0 ||
      this.channels.length === 0
    ) {
      return this.alive;
    }

    const renderedFrames = output[0]?.length ?? 0;
    const step = this.frequencyHz / sampleRate;
    for (let outputFrame = 0; outputFrame < renderedFrames; outputFrame += 1) {
      const sourceFrame = Math.floor(this.position);
      const fraction = this.position - sourceFrame;
      const nextFrame =
        sourceFrame + 1 < this.frameCount
          ? sourceFrame + 1
          : this.playback === "playing-looping"
            ? 0
            : sourceFrame;
      for (let channelIndex = 0; channelIndex < output.length; channelIndex += 1) {
        const source = this.channels[channelIndex] ?? this.channels[0];
        if (source === undefined || output[channelIndex] === undefined) {
          continue;
        }
        const first = source[sourceFrame] ?? 0;
        const second = source[nextFrame] ?? first;
        output[channelIndex][outputFrame] = first + (second - first) * fraction;
      }
      this.position += step;
      if (this.position < this.frameCount) {
        continue;
      }
      if (this.playback === "playing-looping") {
        this.position %= this.frameCount;
        continue;
      }
      this.playback = "stopped";
      this.alive = false;
      if (!this.ended) {
        this.ended = true;
        this.port.postMessage({ type: "ended" });
      }
      return false;
    }
    return true;
  }
}

registerProcessor("vine-direct-sound-stream", VineDirectSoundStreamProcessor);
