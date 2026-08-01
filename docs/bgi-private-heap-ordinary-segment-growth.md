# BGI ordinary private-heap segment growth

## Outcome

Primary BGIのgrowable private heapは、二つの16 MiB backingを使い切った後も
process-wide controlled slot budget内で第三のordinary segmentを作成できる。
Reached `HeapAlloc(0x68010001, 0, 0x00406000)`はallocation ID 1847、
guest address `0x3e000000`、controlled slot 31として成功する。単一requestの
16 MiB上限、fixed heap、process全体32 slot上限は変更しない。

この成功後、deterministic Node／Chrome replayは397 supported callsと追加1件の
source readを進み、`GDI32.dll!CreateFontA`の未対応224 x 112 variantで停止する。
Presentは引き続き2回である。したがって旧二segment capacityは実行を止めていた
gatewayではあるが、third Present停滞の近因ではない。

## Authority and transaction boundary

`runtime/heaps.rs::RuntimeHeaps`がlogical heap、per-heap allocation-ID cursor、
primary backing、slot-keyed extension backing collectionを一意に所有する。
新しいextensionはhighest free controlled slotを選び、address-allocation identity、
heap generation、slot availability、GuestMemory reserve／commit capacityを全て
preflightしてからtargeted commitする。既存allocationの探索はprimary、続いて
extensionをguest address昇順で行う。

Fixed heapのgrowth、32 segment超過、process slot exhaustion、stale／duplicate slot、
GuestMemory collisionはheap metadata、allocation cursor、slot relation、memory owner、
mapping generationを変更せず失敗する。Free、size lookup、reuse、heap destruction、
process teardownは全extensionを同じowner境界で処理する。

## Oracle

Focused native Frida evidenceは旧heap stopの同一caller RVA `0x000A4A33`で
4,218,880-byte allocationが成功することを確認済みである。Runtime replayはその
return address、heap handle、flags、size、success addressを固定する。

次のstopについては追加のfocused Frida trace
`.work/create-font-variant-native-oracle.json`を取得した。Source executableは
1,193,472 bytes、SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
のまま不変である。Caller return RVA `0x0002C838`からのexact requestはheight 224、
width 112、weight 100、charset 128、output precision 4、quality 2、CP932
`ＭＳ ゴシック`で、nativeはnon-NULL HFONTを返しLastError 0を保持する。Decompile
`V:\softs\サクラノ詩\decompiled\BGI.exe.c`の`sub_42C5F0`は、handleをobject+152へ
保存してnonzeroを検査した後にfont／DIB pipelineへ進むconsumerを示す。このvariantは
次checkpointのsemantic leafであり、本checkpointでは実装しない。

## Deterministic evidence

Node report:

- schema `vine.v86-bgi-private-heap-ordinary-segment-growth-primary` v1
- 261,027 supported calls／342,873,024 instructions
- semantic digest `c92d8866b1e9355c076db7126a3ab286b7d67289ca4601a6f9cfb749c23b3a16`
- 100,534 shared-clock samples／39 TIMERPROC callbacks／2 Present
- 139 source reads／7,721 timer-pacing completions
- terminal memory digest `c214835487a1ed382c3ea9cbb459d9c09faa1f329fba2c60c492587443d52806`

Chrome Worker protocol v104／browser report schema 70 reproduces the same call
count, instruction count, semantic digest, endpoint, and terminal memory digest.
The successful report records 128,181 mapping deltas, final adapter generation
316,418, clean cancel／watchdog completion, and report SHA-256
`fbd295af2dd63d76ec8e90bc6e1749e909c1234b97dd46a65c8bcc9a9b16c207`.
Adapter generation and writeback counts are transport evidence, not guest semantics.

## Acceptance and non-claims

Representative checks:

```powershell
cargo test -p vine-core --lib runtime::heaps::tests --locked
cargo test -p vine-core --test runtime_heap_backing --locked
npm run check:runtime:bgi -- --profile kernel32-private-heap-ordinary-segment-growth
npm run build:runtime:browser-worker:prepared
node tools/run-browser-runtime-check.mjs --bgi
```

This checkpoint does not claim a request larger than 16 MiB, more than 32
ordinary process backings, arbitrary heap layout compatibility, concurrent heap
serialization, decommit／compact／walk／LFH behavior, or that capacity growth
causes rendering progress. It also does not implement the newly reached
CreateFontA variant or prove a third Present.
