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
    this.streaming = false;
    this.writeVersions = new Uint32Array(0);
    this.consumedVersions = new Uint32Array(0);
    this.nextWriteVersion = 1;
    this.staleReplayPreventedFrames = 0;
    this.lastReportedPreventedFrames = 0;
    this.underrunActive = false;
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
      channels.some((channel) => channel.length !== channels[0].length) ||
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
      this.streaming = false;
      this.writeVersions = new Uint32Array(this.frameCount);
      this.writeVersions.fill(1);
      this.consumedVersions = new Uint32Array(this.frameCount);
      this.nextWriteVersion = 1;
      this.staleReplayPreventedFrames = 0;
      this.lastReportedPreventedFrames = 0;
      this.underrunActive = false;
    } else {
      this.streaming = true;
      this.nextWriteVersion += 1;
      for (let channelIndex = 0; channelIndex < channels.length; channelIndex += 1) {
        this.channels[channelIndex].set(channels[channelIndex], regionOffsetFrames);
      }
      this.writeVersions.fill(
        this.nextWriteVersion,
        regionOffsetFrames,
        regionOffsetFrames + channels[0].length,
      );
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
    let staleReplayPreventedFrames = 0;
    for (let outputFrame = 0; outputFrame < renderedFrames; outputFrame += 1) {
      const sourceFrame = Math.floor(this.position);
      // An incrementally updated looping buffer is a stream. Replaying a frame
      // that Rust has not refreshed turns producer underrun into a short audio
      // loop, so hold the browser cursor in silence until a new version arrives.
      // Static looping buffers never enter this path.
      if (
        this.streaming &&
        this.writeVersions[sourceFrame] ===
          this.consumedVersions[sourceFrame]
      ) {
        staleReplayPreventedFrames += 1;
        continue;
      }
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
        const nextFrameFresh =
          !this.streaming ||
          nextFrame === sourceFrame ||
          this.writeVersions[nextFrame] !== this.consumedVersions[nextFrame];
        const second = nextFrameFresh ? (source[nextFrame] ?? first) : first;
        output[channelIndex][outputFrame] = first + (second - first) * fraction;
      }
      this.position += step;
      if (Math.floor(this.position) !== sourceFrame) {
        this.consumedVersions[sourceFrame] = this.writeVersions[sourceFrame];
      }
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
    this.reportStreamPacing(staleReplayPreventedFrames);
    return true;
  }

  reportStreamPacing(preventedFrames) {
    if (preventedFrames > 0) {
      this.staleReplayPreventedFrames += preventedFrames;
      const reportInterval = Math.max(128, Math.floor(sampleRate / 4));
      if (
        !this.underrunActive ||
        this.staleReplayPreventedFrames - this.lastReportedPreventedFrames >=
          reportInterval
      ) {
        this.underrunActive = true;
        this.lastReportedPreventedFrames = this.staleReplayPreventedFrames;
        this.port.postMessage({
          type: "stream-underrun",
          staleReplayPreventedFrames: this.staleReplayPreventedFrames,
        });
      }
      return;
    }
    if (this.underrunActive) {
      this.underrunActive = false;
      this.lastReportedPreventedFrames = this.staleReplayPreventedFrames;
      this.port.postMessage({
        type: "stream-recovered",
        staleReplayPreventedFrames: this.staleReplayPreventedFrames,
      });
    }
  }
}

registerProcessor("vine-direct-sound-stream", VineDirectSoundStreamProcessor);
