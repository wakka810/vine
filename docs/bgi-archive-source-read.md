# BGI primary boot-archive source read checkpoint

## Status

- Status: semantic checkpoint verified; browser performance gate blocked
- Date: 2026-07-26
- Runtime-kernel checkpoint: `kernel32-archive-source-read`
- Previous checkpoint:
  [`shell32-drag-accept-files`](bgi-shell32-drag-accept-files.md)
- Next owned endpoint: `USER32.dll!SetWindowPos` (resize variant)

This checkpoint reclassifies the previously recorded `USER32.dll!MessageBoxA`
stop and closes the first primary immutable-source asset access. Guest memory
inspection at the stop showed the engine formatting
`指定されたファイル [ system.arc : ipl._bp ] は存在しません` into the
`Error!!` dialog: MessageBoxA was the failure branch taken because the
immutable mount did not expose `system.arc`. The correct work item was the
`immutable-source-asset-access` scenario milestone, not a MessageBoxA
contract.

## Primary boot-archive snapshot

`fixtures/source-snapshots/primary-boot-archives-v1.json` is the versioned
immutable snapshot catalog for the primary title. It registers exactly the
three boot archives whose successful opens are already recorded in the native
file-behavior evidence (`windows-native-evidence.md` §3.4): `system.arc`
(21,464 bytes), `sysprg.arc` (195,670 bytes), and `sysgrp.arc` (11,665,375
bytes), each pinned by byte length and SHA-256 under mount root `C:\Vine`
with source ID 1. `BGI.hvl` and the `data*.arc` archives are not registered:
the native capture did not observe them being opened during boot, and the
multi-gigabyte data archives require the lazy range model rather than
registration-time whole-file buffers.

The snapshot registers alongside the existing `BGI.gdb` writable profile.
Rust `RuntimeFiles` remains the sole path, mount, share, cursor, and handle
authority; writable-profile paths may not shadow immutable objects, and
immutable objects reject write, truncate, and create dispositions before any
mutation.

## Reached behavior

Node and clean-profile Chrome complete the same sequence:

1. `CreateFileA("C:\Vine\ipl._bp")` — the engine's loose-file probe — still
   returns `INVALID_HANDLE_VALUE` with LastError 2, exactly as on native
   Windows where that loose file does not exist.
2. `CreateFileA("C:\Vine\system.arc")` opens logical handle `0x46030001`
   (sequence 89,270), reads the 16-byte archive header and the 384-byte
   member index through the bounded `storage.source-read.v1` transport, and
   closes the handle.
3. A second open (`0x46040001`, sequence 89,283) seeks to offset 400 and
   reads the 2,328-byte `ipl._bp` member, then closes.
4. The BGI script interpreter consumes the boot program: the run continues
   through heap traffic, a window fill, and `SetWindowLongA` to a
   `WS_POPUP`-style frame, and stops at instruction 15,951,782 on endpoint
   ID 202, `USER32.dll!SetWindowPos(0x48010001, HWND_NOTOPMOST, 0, 0, 1286,
   749, 0x10A)` — the engine resizing its window for the 1280x720 client
   resolution read from the boot program. The stop is the typed
   `set-window-pos-size-unsupported` variant fault of the existing
   same-geometry activation contract.

The complete profile reaches 89,386 supported calls. Node and Chrome match
semantic SHA-256
`c4e32952ed36c7d0580356e82d2e56ba93b143a390cd6f36a77925a366602732`
and final memory SHA-256
`233209a71d74ab8893e91efc9404c8d493d1ea438beba06ae626013b6cbb2deb`.
Exact report and artifact identities remain manifest-owned.

## Browser synchronous host-completion order

The archive reads are the first primary storage traffic that overlaps the
recurring waitable-timer pacing on the second guest thread. The Node oracle
services every host request synchronously, so host completions always reach
Rust in request-issue order; wall-clock fetch latency in Chrome could reorder
them and diverge from the frozen scheduling stream.

The Runtime Worker therefore treats `runtime.timer-delay.v1`,
`storage.source-read.v1`, `storage.profile-read.v1`, and
`storage.profile-commit.v1` as synchronous-stream services: the issuing
thread parks on a completion barrier that validates request and thread
identity (not FIFO position), and completions are delivered to Rust strictly
in request-issue order, deferring a younger completion until the older
in-flight request has resolved. The controlled fixture's browser host
services keep the existing offer-the-slot scheduling path, which the fixture
E2E (`hostWaitExecutions`) continues to exercise. Chrome reproduces the Node
context-switch count (1,032) and both digests with this discipline.

## Performance blocker

The structured browser throughput blocker remains active and was re-measured
for this profile in a valid environment: 14 completed sessions against a
minimum of 24 and session P95 13,108.180 ms against a maximum of 8,000 ms;
the other 15 versioned thresholds passed. Thresholds are unchanged and the
benchmark command is expected to fail while the blocker is active. Resumption
still requires a profiled correctness-preserving Rust/Wasm dispatch fast path
or an explicit versioned performance-contract decision.

## Reproduction

```powershell
npm run check:runtime:bgi -- --profile kernel32-archive-source-read
node tools/run-browser-runtime-check.mjs --bgi `
  --bgi-profile kernel32-archive-source-read `
  --bgi-policy fixtures/runtime-policies/kernel32-wait-for-single-object-ex-v1.json
npm run check:runtime:bgi-browser-benchmark:prepared
```

## Non-claims

- The runtime does not parse the BGI archive format; the engine's own code
  performs all header, index, and member interpretation.
- Registering the three boot archives is bounded data availability, not a
  general VFS directory listing, search-path, or loose-file overlay claim.
- `sysprg.arc` and `sysgrp.arc` are registered but their reads are not yet
  reached; no claim is made beyond identity-pinned availability.
- The SetWindowPos resize stop, rendering, input consumption, audible
  output, save／restart／load, representative flow, and full-play acceptance
  are not claimed.
- Semantic equivalence does not override the active performance blocker.
