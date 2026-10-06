import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {build} from "esbuild";

// The ES is the same wholly synthetic two-picture source exercised through the
// original guest IAsyncReader/renderer in the PE32 DirectShow fixture.
const fixture = await readFile("fixtures/pe32/directshow/fixture-directshow.c", "utf8");
const initializer = /static const BYTE video\[\] = \{([^}]+)\}/u.exec(fixture)?.[1];
assert(initializer);
const es = Uint8Array.from(initializer.match(/0x[0-9a-f]+/giu).map(value => Number(value)));
// Synthetic color bars +440Hz tone, not extracted source media. MPEG-1 video
// and MP2 stereo were generated with FFmpeg lavfi at64x48/30fps/48000Hz for0.5s.
const ps = new Uint8Array(await readFile("fixtures/media/synthetic-mpeg1-mp2.mpg"));
assert(ps.length < 16 * 1024);
const {outputFiles} = await build({entryPoints:["browser/runtime-worker/media-decoder.ts"],
  bundle:true, write:false, platform:"node", format:"esm"});
const {MediaDecoder, UnsupportedMediaFormat} = await import(
  `data:text/javascript;base64,${Buffer.from(outputFiles[0].contents).toString("base64")}`);
const moduleUrl = new URL("../.work/libav-mpeg-build/dist/libav-6.10.9.0-vine-mpeg.mjs", import.meta.url);

async function decode(bytes) {
  const reads = [];
  const decoder = await MediaDecoder.open(bytes.length, async(offset,length) => {
    assert(Number.isSafeInteger(offset) && offset >= 0);
    assert(Number.isInteger(length) && length > 0 && length <= 2*1024*1024,
      "zero-length EOF must never invoke the Rust source callback");
    assert(offset+length <= bytes.length);
    reads.push([offset,length]);
    return bytes.slice(offset,offset+length);
  }, moduleUrl);
  try {
    const format = decoder.format, frames = [], audio = [];
    let lastStart = 0;
    while (true) {
      const sample = await decoder.next();
      if (sample === null) break;
      assert(Number.isSafeInteger(sample.start100ns) && Number.isSafeInteger(sample.end100ns));
      assert(sample.start100ns >= lastStart && sample.end100ns > sample.start100ns);
      lastStart = sample.start100ns;
      (sample.kind === "frame" ? frames : audio).push(sample);
    }
    assert.equal(await decoder.next(), null, "decoder EOF remains EOF without duplicate samples");
    assert.equal(Math.max(...frames.concat(audio).map(sample=>sample.end100ns)), format.duration100ns,
      "duration comes from actual samples through EOF rather than the raw ES bitrate estimate");
    return {format,frames,audio,reads};
  } finally {
    await decoder.close();
    await decoder.close();
    await assert.rejects(decoder.next(), /closed/u);
  }
}

const first = await decode(es);
assert.deepEqual(first.format, {width:16,height:16,duration100ns:800000,frameDuration100ns:400000,
  audioChannels:0,audioSamplesPerSecond:0});
assert.equal(first.frames.length,2); assert.equal(first.audio.length,0);
assert.deepEqual(first.frames.map(({start100ns,end100ns})=>[start100ns,end100ns]), [[0,400000],[400000,800000]]);
for (const {bytes} of first.frames) {
  assert.equal(bytes.length,1024);
  assert(bytes.every((value,index)=>value === (index%4===3 ? 255 : 129)), "actual BGRA conversion is uniform gray");
}
const multiplexed = await decode(ps);
assert.equal(multiplexed.format.width,64); assert.equal(multiplexed.format.height,48);
assert.equal(multiplexed.frames.length,15); assert.equal(multiplexed.audio.length,21);
assert.equal(multiplexed.format.audioChannels,2); assert.equal(multiplexed.format.audioSamplesPerSecond,48000);
let audibleSamples = 0;
for (const frame of multiplexed.frames) assert.equal(frame.bytes.length,64*48*4);
for (const sample of multiplexed.audio) {
  assert.equal(sample.channels,2); assert.equal(sample.samplesPerSecond,48000);
  assert.equal(sample.bytes.length,1152*2*4);
  const pcm = new Float32Array(sample.bytes.buffer,sample.bytes.byteOffset,sample.bytes.length/4);
  assert(pcm.every(Number.isFinite));
  audibleSamples += pcm.filter(value=>Math.abs(value)>0.001).length;
}
assert(audibleSamples > 40000, "real MP2 decode produces nonzero float PCM");
assert(multiplexed.format.duration100ns >= 5000000 && multiplexed.format.duration100ns < 5200000);
await assert.rejects(MediaDecoder.open(es.length, async()=>es.subarray(0,1), moduleUrl), /disagrees with its range/u);
let reads = 0;
await assert.rejects(MediaDecoder.open(es.length, async(offset,length)=> {
  if (++reads > 1) throw new Error("actual bounded source read failed");
  return es.slice(offset,offset+length);
}, moduleUrl), /actual bounded source read failed/u);
await assert.rejects(MediaDecoder.open(8, async()=>new Uint8Array(8), moduleUrl), UnsupportedMediaFormat);
const truncated = es.slice(0,43);
await assert.rejects(MediaDecoder.open(truncated.length, async(offset,length)=>truncated.slice(offset,offset+length), moduleUrl),
  /Invalid data|decode|damaged/iu, "an incomplete MPEG macroblock must fail actual decode");
// Reopening after both malformed input and a real failed read must still decode.
assert.equal((await decode(es)).frames.length,2);
console.log("Media decoder: actual MPEG ES/PS, RGB32, MP2 PCM, timestamps, duration, EOF, bounded reads and failures passed");
