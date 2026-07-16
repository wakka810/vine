# BGI `SwapMouseButton` checkpoint

## Status

- Status: verified
- Date: 2026-07-16
- Runtime-kernel checkpoint: `bgi-swap-mouse-button`
- Previous capability checkpoint: `bgi-get-drive-type-a`
- First unsupported capability: `winmm-time-get-dev-caps`

This checkpoint introduces the minimal logical input-state vertical slice. It
implements the reached mouse-button normalization／restore transaction without
importing host or browser mapping and without claiming DOM or DirectInput
delivery.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-swap-mouse-button-20260716-r3.jsonl `
  --seconds 30 --kill-after `
  --stop-on-event swap-mouse-button-complete `
  --observe swap-mouse-button
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r2 | 31,879 | 72 | 9 | `eca20bd9a661ffd1b04b9dad776711c38002f18ab130e65c166fd58d4033d459` |
| Confirmation r3 | 31,930 | 72 | 9 | `1dea669569a73ddc7eb5a9cf49068114865bf206c905204cec6733071f630a34` |

Both traces have contiguous agent sequences, balanced enter／leave IDs, and
zero hook, vtable, call-stack, agent-message, or cleanup diagnostics. The
observer records `GetSystemMetrics(SM_SWAPBUTTON)` before the BGI calls and
does not use that native value as runtime policy. The completion event is
emitted only after both calls and verifies that the host mapping is restored.

BGI performs two calls through the same ESI endpoint:

```asm
push 0
call esi                 ; SwapMouseButton(FALSE)
push eax                 ; preserve previous mapping for call 2
mov [process_global], eax
call esi                 ; SwapMouseButton(previous)
pop esi
ret
```

The return RVAs are `0x00088A3B` and `0x00088A43`. On both accepted runs the
host started unswapped, so arguments and results were `[0,0]`, state remained
false, and LastError remained 0. The semantic relation is more important than
those host values: call 1 returns the previous mapping and call 2 receives that
result, restoring the initial state.

## Rust authority and API contract

`runtime/input.rs::RuntimeInput` owns profile identity, initial mapping,
current mapping, and owner-local generation. The profile is configured once;
swap mutation is plan／stale-check／targeted commit. Zero means normal mapping
and any nonzero BOOL means swapped. The return is the previous BOOL state.
Idempotent requests do not advance the input generation.

`user32.swap-mouse-button.v1` is one-argument x86 stdcall, returns BOOL,
preserves LastError, writes no guest bytes, and never calls the browser. The
version-2 User32 policy selects `normal-mouse-buttons-v1`, initially false.
Public tests also start from true and prove the exact normalize／restore pair,
so the second call remains correct even when the initial mapping differs from
the controlled primary profile.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile user32-swap-mouse-button
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

Node completes 1,333 calls across 54 contracts. Swap calls are sequences
1,322／1,323 at instructions 46,215／46,219 and return to
`0x10088A3B`／`0x10088A43`. Both return false, preserve LastError 0, and write
no guest bytes. The existing LastError／TLS／FLS continuation and two later
`InitializeCriticalSection` calls remain green.

Node stops at instruction 49,731 with 1,339 writeback batches, 2,530 dirty
pages, 8,967 changed bytes, 82 host deltas, and generation 1,450. Chrome uses
6,820 slices, 5,481 yields, 4,951 writeback batches／6,695 pages, 6,375 Wasm
crossings, and generation 3,586. Both have memory SHA-256
`f6030b2d43bbbc6fe43a2ede52b1ecc6e4962822e39235032f25ca560a58d0e4`.

The 5,437,068-byte Node report has SHA-256
`29e5061c7943140c48f5b5c0f5e0ccf22c930fa77b499ed8e5081fae65cc3f4a`.
The implementation-run 799,262-byte browser report has SHA-256
`3135d564d3a5d9727301bc3fe2e9a436ce3b084b2fdd81524b8efa0bcfb9c3f1`.
Runtime-core Wasm is 2,295,595 bytes with SHA-256
`5c94f7dbb904829a2a6e623d502d66aa39fdf5f7f563f0260bffaa39bef2d874`.

The first unchanged benchmark attempt exposed benchmark-main retention of the
full completed 1,333-call report across its pacing delay: main heap peaked at
19,007,386 bytes and failed only `maximum-main-heap`; final heap and slope,
Worker metrics, semantic sessions, and the other 16 thresholds passed. The raw
failed report was replaced by the passing rerun; its recorded identity was
1,061,256 bytes with SHA-256
`ed23f3cd92a5ec50150dfc64743fa18ac49814998e19d83ab7d8e9c63c8593c3`.

After compact scalar metrics are retained, the benchmark now releases the
completed report's large arrays before the 20 ms delay. No threshold or sample
rule changed. The passing 180-second run completed 346 sessions with zero
failure and all 17 thresholds green. Main heap maximum is 11,661,344 bytes and
slope -263,411 bytes/minute. The 1,061,226-byte report has SHA-256
`5293a80666e1148f7aca57183b1d967fb705a00379484ca3388cf839026afa2f`.

## Non-claims

This checkpoint does not implement host mouse settings, DOM event ingestion,
mouse messages, cursor coordinates, capture, focus, button count, double-click
timing, keyboard input, or DirectInput. The native initial-state read is oracle
metadata only. The controlled false initial mapping is compatibility policy,
not evidence that every host is unswapped.

## Next structured stop

```text
WINMM.dll!timeGetDevCaps(0x1014C8E8, 8)
endpoint ID:       3
endpoint address:  0x7F000020
IAT:               0x100D14D0
return:            0x10091E6D
instruction:       49731
fault:             endpoint-contract-missing
cluster:           winmm-api-contract
```

The next checkpoint must capture the written `TIMECAPS` bytes, size handling,
return code, LastError behavior, and caller consumption before choosing the
timer-resolution owner and browser scheduling boundary.
