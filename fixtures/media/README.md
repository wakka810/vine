`synthetic-mpeg1-mp2.mpg` is a 14,336-byte synthetic MPEG-1 program stream
with 64×48 test-pattern video at 30 fps and a 440 Hz MP2 stereo tone at
48,000 Hz/128 kbps. It contains no extracted application media. The decoder
test checks actual RGB32/PCM output, presentation timestamps and drained EOF.

It was generated with FFmpeg `N-92722-gf22fcd4483` using:

```sh
ffmpeg -f lavfi -i testsrc=size=64x48:rate=30 \
  -f lavfi -i sine=frequency=440:sample_rate=48000 -t 0.5 \
  -c:v mpeg1video -bf 2 -pix_fmt yuv420p -c:a mp2 -b:a 128k -ac 2 \
  -f mpeg synthetic-mpeg1-mp2.mpg
```
