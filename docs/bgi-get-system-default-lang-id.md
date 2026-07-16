# BGI `GetSystemDefaultLangID` checkpoint

## Status

- Status: verified
- Date: 2026-07-16
- Runtime-kernel checkpoint: `bgi-get-system-default-lang-id`
- Previous capability checkpoint: `bgi-co-initialize`
- First unsupported capability: `kernel32-get-drive-type-a`

This checkpoint adds the reached system-default language query as a semantic
leaf in the existing Kernel32 locale policy. The guest-visible LANGID is an
explicit Rust policy value; it is not imported from the host or inferred from
the list of locale variants accepted by another API.

## Native evidence

The focused observer records the BGI call, its immediate consumer, and four
same-process oracle calls with distinct LastError sentinels:

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-system-default-lang-id-20260716-r7.jsonl `
  --seconds 1.1 --kill-after --observe system-default-lang-id
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r5 | 81,022 | 266 | 103 | `b7837df03fd27e34c5839116874d1b0858874c5179d3b92bfa94f99a570f0efa` |
| Confirmation r7 | 79,185 | 260 | 100 | `01e67e62be63be76293fbf6d59e8d3173e88f4c1597e5571002cc817406f79e0` |

Both accepted captures have zero diagnostics. Captures r1／r3／r6 were rejected
because termination left an unrelated `Direct3DCreate9` call open. Captures
r2／r4 predate the corrected WORD-return observer and are not final evidence.

The four oracle calls use LastError sentinels `0`, `87`, `122`, and
`0x12345678`. Every call returns LANGID `0x0411` and preserves its sentinel.
The accepted BGI call also returns `0x0411` and returns to RVA `0x0006A556`.

Native EAX is not a DWORD result contract. The low WORD is stable, but the
upper 16 bits differ between accepted runs: primary r5 observes `0x01390411`
and confirmation r7 observes `0x00CD0411`. Those upper bits are unspecified
native artifact identity and are not imported into Vine semantics.

The immediate BGI consumer is:

```asm
mov ecx, 0x3ff
and ax, cx
xor edx, edx
cmp ax, 0x11
sete dl
mov eax, edx
ret
```

It consumes AX only, masks the sublanguage bits, and tests primary language
`0x11` (Japanese). It does not read LastError.

## Rust authority and ABI

`Kernel32CompatibilityPolicy.system_default_lang_id` is the sole authority for
the returned WORD. Historical and controlled policies leave the optional field
disabled. The active policy explicitly stores `1041`; validation requires the
bounded Japanese locale layer and the exact supported LANGID, but the handler
does not derive the default identity from `LCMapStringW.allowed_locales`.

`ApiReturnSchema::U16` and `ApiReturnValue::U16` represent the x86 WORD return.
The generic completion zero-extends it to EAX, so Vine deterministically returns
`0x00000411` instead of reproducing native undefined upper bits. A mismatched
handler result remains an ABI defect and is rejected before completion.

`kernel32.get-system-default-lang-id.v1` is a zero-argument x86 stdcall,
preserves LastError, writes no guest bytes, and mutates no runtime state. A
policy without the explicit field returns
`get-system-default-lang-id-policy-disabled` before memory or runtime mutation.
The handler contains no BGI address, hash, call occurrence, or host-locale
branch.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-system-default-lang-id
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 52 / 1,295 | 52 / 1,295 |
| Instructions at next stop | 45,840 | 45,840 |
| Writeback batches / pages | 1,301 / 2,461 | 4,539 / 6,237 |
| Host / mapping deltas | 80 | 80 |
| Callback entries / maximum depth | 1 / 1 | 1 / 1 |
| Final generation | 1,410 | 3,263 |
| Memory SHA-256 | `05bf22ce52303ba9b0849352ecd4e00460449ac3bbf84fd03e30cac146d8dc0c` | same |

Call 1,295 runs at instruction 45,691, has no arguments, returns
`0x00000411`, preserves LastError 0, performs four-byte stdcall return cleanup,
and writes no guest bytes. Instruction 45,840 reaches the same next endpoint
and memory digest in Node and Chrome.

The 5,287,412-byte Node report has SHA-256
`f376ff987e2f25a25c7824c7768383d60242b05d88fe29df9ed21cebab885341`.
Chrome `150.0.7871.115` uses 6,324 slices, 5,023 budget yields, and 5,923 Wasm
bridge calls. The 776,051-byte browser report has SHA-256
`89981fc901a5404a9a0f7e4020f53877556d962ced7ba67b7c0bab4f03b1f196`;
success, cancellation, and watchdog all retire pending callbacks and host work.

The runtime-core Wasm is 2,279,333 bytes with SHA-256
`a6613c64374e16499597edc9d755b04e8c4b05b350db567de837e6402f570fce`.
The unchanged 180-second configuration completes 205 sessions with zero
failures and passes all 17 thresholds. Session p95 is `949.065 ms`, slice p99
`3.100 ms`, heartbeat p99 `9.995 ms`, and maximum heartbeat gap `40.400 ms`.
The 963,644-byte benchmark report has SHA-256
`cf57ecef464313763a6a4db2eeebc6d1310ad3a44ece424b4778621a42569521`.

## Non-claims

This checkpoint does not implement `GetUserDefaultLangID`, user-default or
thread-default locale identity, LCID fallback, locale enumeration, formatting,
locale mutation, host locale import, or a general NLS subsystem. It does not
claim that every Windows implementation zeroes EAX above AX.

The policy relation to the Japanese locale layer is a bounded compatibility
validation rule, not authority sharing. Changing `allowed_locales` cannot
silently change the default LANGID, and browser code does not own a second
language or locale value.

## Next structured stop

```text
KERNEL32.dll!GetDriveTypeA(0x006FFC4C)
endpoint ID:       19
endpoint address:  0x7F000120
IAT:               0x100D10F4
return:            0x100602B2
instruction:       45840
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

The next checkpoint must decode the reached root-path string, establish native
drive-type and LastError behavior plus caller consumption, and decide whether
the semantic leaf belongs in the existing immutable-source／VFS policy boundary.
