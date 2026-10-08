const headerBytes = 48;
const pcmLimit = 16 * 1024 * 1024;

function pcmSample(voice, index) {
  return voice.bits === 8 ? (voice.pcm.getUint8(index) - 128) / 128 :
    voice.pcm.getInt16(index * 2, true) / 32768;
}

class VinePcmProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.voices = new Map();
    this.media = new Map();
    this.port.onmessage = ({ data }) => {
      if (data?.type === "media-command") {
        let accepted = false;
        try {
          if (this.mediaCommand(data)) return;
          accepted = true;
        } catch { /* Validate the bounded resource command before mutation. */ }
        this.port.postMessage({ type: "media-ack", requestId: data.requestId, accepted });
        return;
      }
      if (data?.type === "retire") {
        if (typeof data.resourceId === "string") this.voices.delete(data.resourceId);
        return;
      }
      if (data?.type !== "command" || typeof data.requestId !== "string") return;
      let accepted = false;
      try { this.command(data.bytes); accepted = true; } catch { /* Reject before changing any voice. */ }
      this.port.postMessage({ type: "ack", requestId: data.requestId, accepted });
    };
  }

  mediaCommand({ requestId, resourceId, operation, volume, bytes }) {
    if (typeof requestId !== "string" || !/^[1-9][0-9]*$/u.test(requestId) ||
        typeof resourceId !== "string" || !/^[1-9][0-9]*$/u.test(resourceId) ||
        !Number.isInteger(volume) || volume < -10000 || volume > 0) throw new Error("Invalid media identity/control");
    const voice = this.media.get(resourceId);
    if (operation === "create") {
      if (voice !== undefined || this.media.size >= 32 || bytes !== undefined) throw new Error("Invalid media creation");
      this.media.set(resourceId, { queue: [], bytes: 0, cursor: 0, running: false,
        gain: Math.pow(10, volume / 2000), drain: null });
      return false;
    }
    if (voice === undefined) throw new Error("Media audio resource is absent");
    if (operation === "sample") {
      if (!(bytes instanceof ArrayBuffer) || bytes.byteLength === 0 || bytes.byteLength % 8 !== 0 ||
          bytes.byteLength + voice.bytes > 512 * 1024 || voice.drain !== null) throw new Error("Media PCM queue exceeds its bound");
      const samples = new Float32Array(bytes);
      if (samples.some(value => !Number.isFinite(value))) throw new Error("Invalid media PCM sample");
      voice.queue.push(samples);
      voice.bytes += bytes.byteLength;
      return false;
    }
    if (bytes !== undefined) throw new Error("Media control has unexpected PCM");
    if (operation === "drain") {
      if (voice.drain !== null) throw new Error("Media audio already awaits its drain");
      if (voice.queue.length === 0) return false;
      voice.drain = requestId;
      return true;
    }
    if (operation === "run") voice.running = true;
    else if (operation === "pause") voice.running = false;
    else if (operation === "volume") voice.gain = Math.pow(10, volume / 2000);
    else if (operation === "stop" || operation === "release") {
      voice.running = false;
      voice.queue = []; voice.bytes = 0; voice.cursor = 0;
      if (voice.drain !== null) {
        this.port.postMessage({ type: "media-ack", requestId: voice.drain, accepted: false });
        voice.drain = null;
      }
      if (operation === "release") this.media.delete(resourceId);
    } else throw new Error("Unknown media audio operation");
    return false;
  }

  command(bytes) {
    if (!(bytes instanceof ArrayBuffer) || bytes.byteLength < headerBytes ||
        bytes.byteLength > headerBytes + pcmLimit) throw new Error("Invalid PCM command length");
    const view = new DataView(bytes);
    const word = offset => view.getUint32(offset * 4, true);
    const operation = word(0);
    const id = view.getBigUint64(4, true).toString();
    const length = word(11);
    if (length !== bytes.byteLength - headerBytes) throw new Error("Invalid PCM payload length");
    if (operation === 6) {
      if (length % 8 !== 0) throw new Error("Invalid resource retirement");
      const ids = [];
      for (let offset = headerBytes; offset < bytes.byteLength; offset += 8) {
        ids.push(view.getBigUint64(offset, true).toString());
      }
      for (const resourceId of ids) this.voices.delete(resourceId);
      return;
    }
    if (id === "0") throw new Error("Missing audio resource identity");
    if (operation === 5) {
      if (length !== 0) throw new Error("Stop has a PCM payload");
      this.voices.delete(id);
      return;
    }
    const channels = word(3), rate = word(4), bits = word(5);
    const volume = view.getInt32(24, true), pan = view.getInt32(28, true);
    const frequency = word(8), position = word(9), looping = word(10);
    const alignment = channels * bits / 8;
    if (![1, 2].includes(channels) || ![8, 16].includes(bits) ||
        rate < 8000 || rate > 192000 || frequency < 100 || frequency > 200000 ||
        volume < -10000 || volume > 0 || pan < -10000 || pan > 10000 ||
        looping > 1 || position % alignment !== 0) throw new Error("Invalid PCM format or controls");
    const previousId = operation === 4 && length === 8 ? view.getBigUint64(headerBytes, true).toString() : id;
    const previous = this.voices.get(previousId);
    let pcm, frames;
    if (operation === 1 || operation === 2) {
      if (length === 0 || length % alignment !== 0) throw new Error("Unaligned PCM samples");
      frames = length / alignment;
      // The transferred packet belongs to this resource. Keep its PCM bytes;
      // converting the full ring here blocks the real-time audio thread.
      pcm = new DataView(bytes, headerBytes, length);
      if (operation === 2 && previous === undefined) throw new Error("PCM update has no playback resource");
    } else if (operation === 3 || operation === 4) {
      if (length !== (operation === 4 ? 8 : 0) || previous === undefined) throw new Error("Control has no playback resource");
      pcm = previous.pcm;
      frames = previous.frames;
    } else throw new Error("Unknown PCM operation");
    if (position / alignment >= frames || (previous !== undefined &&
        (previous.channels !== channels || previous.rate !== rate || previous.bits !== bits || previous.frames !== frames))) {
      throw new Error("PCM resource layout changed");
    }
    const gain = Math.pow(10, volume / 2000);
    const left = gain * (pan > 0 ? Math.pow(10, -pan / 2000) : 1);
    const right = gain * (pan < 0 ? Math.pow(10, pan / 2000) : 1);
    // The Chrome cursor belongs only to the audio resource. Windows position and
    // notification state remain in Rust; an explicit seek replaces this cursor.
    const cursor = operation === 4 || previous === undefined ? position / alignment : previous.cursor;
    this.voices.set(id, { pcm, frames, channels, rate, bits, frequency, left, right,
      cursor, looping: looping === 1, ended: operation === 1 || operation === 4 ? false : previous?.ended ?? false });
    if (operation === 4 && previousId !== id) this.voices.delete(previousId);
  }

  process(_inputs, outputs) {
    const output = outputs[0];
    if (output === undefined || output.length === 0) return true;
    for (const channel of output) channel.fill(0);
    const left = output[0], right = output[1] ?? left;
    for (const [resourceId, voice] of this.voices) {
      if (voice.ended) continue;
      const step = voice.frequency / sampleRate;
      for (let frame = 0; frame < left.length; frame++) {
        if (voice.cursor >= voice.frames) {
          if (voice.looping) voice.cursor %= voice.frames;
          else {
            voice.ended = true;
            this.port.postMessage({ type: "ended", resourceId });
            break;
          }
        }
        const index = Math.floor(voice.cursor), fraction = voice.cursor - index;
        const next = index + 1 < voice.frames ? index + 1 : voice.looping ? 0 : index;
        const a = pcmSample(voice, index * voice.channels);
        const b = pcmSample(voice, next * voice.channels);
        const l = a + (b - a) * fraction;
        const rightSample = voice.channels === 1 ? l : pcmSample(voice, index * 2 + 1);
        const r = voice.channels === 1 ? l : rightSample +
          (pcmSample(voice, next * 2 + 1) - rightSample) * fraction;
        left[frame] += l * voice.left;
        if (right !== left) right[frame] += r * voice.right;
        voice.cursor += step;
      }
    }
    for (const voice of this.media.values()) {
      if (!voice.running) continue;
      const step = 48000 / sampleRate;
      for (let frame = 0; frame < left.length; frame++) {
        while (voice.queue.length > 0 && voice.cursor >= voice.queue[0].length / 2) {
          const consumed = voice.queue.shift();
          voice.cursor -= consumed.length / 2;
          voice.bytes -= consumed.byteLength;
        }
        if (voice.queue.length === 0) {
          if (voice.drain !== null) {
            this.port.postMessage({ type: "media-ack", requestId: voice.drain, accepted: true });
            voice.drain = null;
          }
          break;
        }
        const samples = voice.queue[0];
        const index = Math.floor(voice.cursor), fraction = voice.cursor - index;
        const next = Math.min(index + 1, samples.length / 2 - 1);
        left[frame] += (samples[index * 2] + (samples[next * 2] - samples[index * 2]) * fraction) * voice.gain;
        if (right !== left) right[frame] += (samples[index * 2 + 1] +
          (samples[next * 2 + 1] - samples[index * 2 + 1]) * fraction) * voice.gain;
        voice.cursor += step;
      }
    }
    return true;
  }
}

registerProcessor("vine-pcm", VinePcmProcessor);
