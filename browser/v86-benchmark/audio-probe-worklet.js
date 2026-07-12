class VineAudioDeadlineProbe extends AudioWorkletProcessor {
  constructor() {
    super();
    this.quantumCount = 0;
  }

  process(_inputs, outputs) {
    this.quantumCount += 1;
    for (const output of outputs) {
      for (const channel of output) {
        channel.fill(0);
      }
    }
    if (this.quantumCount % 16 === 0) {
      this.port.postMessage({ currentFrame, currentTime, quantumCount: this.quantumCount });
    }
    return true;
  }
}

registerProcessor("vine-audio-deadline-probe", VineAudioDeadlineProbe);
