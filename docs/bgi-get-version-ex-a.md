# BGI `GetVersionExA` checkpoint

## Status

- Status: verified
- Date: 2026-07-17
- Runtime-kernel checkpoint: `bgi-get-version-ex-a`
- Previous capability checkpoint: `bgi-time-begin-period`
- First unsupported capability: `kernel32-create-waitable-timer-a`

This checkpoint adds the reached size-versioned ANSI operating-system version
contract. Rust owns the guest-visible fields through a versioned Kernel32
compatibility policy. The browser does not contribute host OS identity or keep
a second copy of Windows state.

## Native evidence

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-get-version-ex-a-20260717-r2.jsonl `
  --seconds 30 --kill-after `
  --stop-on-event get-version-ex-a-complete `
  --observe get-version-ex-a
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r1 | 287,884 | 791 | 360 | `ee021e4fb44315a342746bfd5908b36b5a2f319d2cd1c588155e115eed15ddc7` |
| Confirmation r2 | 290,851 | 800 | 364 | `0ae028967ca9e815e15d6ce109310e8b20d57a19bc09fded832a7a2c3a59a90b` |

Both captures have contiguous agent sequences, balanced call IDs, zero
diagnostics, and 250 ms of hook quiescence. The source executable remained
1,193,472 bytes with SHA-256
`f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1`
before and after collection.

The guarded x86 oracle establishes the size and write relations:

- declared sizes 0 and 147 return `FALSE`, set LastError to
  `ERROR_INSUFFICIENT_BUFFER` (`122`), and leave output and both guards
  unchanged;
- declared size 148 succeeds, preserves independent LastError sentinels, and
  changes only offsets 4 through 20;
- declared size 156 succeeds, preserves LastError, and additionally changes
  offsets 148 through 155;
- all hooked results agree with the direct `SystemFunction` result and bytes
  beyond the declared size remain unchanged.

The observed host fields are version 10.0 build 26200, NT platform 2, empty
service-pack text, suite mask `0x0300`, and workstation product type 1. These
identify the native oracle host; the build and edition fields are not imported
as runtime policy.

BGI passes a zero-initialized 148-byte structure and returns to RVA
`0x00050526` with result 1 and LastError 0. Its immediate consumer compares
`dwPlatformId` with `VER_PLATFORM_WIN32_NT` (`2`). It does not branch on the
major, minor, build, service-pack, suite, or product fields in this reached
path.

## Policy, ABI, and failure ordering

`Kernel32CompatibilityPolicy.os_version_info_a` is the sole authority for the
virtual profile. Version 1 exposes Windows NT 10.0 build 19045, an empty
service-pack label, suite mask `0x0100`, and workstation product type 1. The
controlled build deliberately differs from the captured host build so host
identity cannot silently become guest semantics.

Policy validation requires schema v2, the existing PE32 system-information
layer, nonzero major version, platform ID 2, a bounded ASCII service-pack label,
and product type 1 through 3. The active contract accepts only the measured
148-byte `OSVERSIONINFOA` and 156-byte `OSVERSIONINFOEXA` layouts.

`kernel32.get-version-ex-a.v1` is a one-argument x86 stdcall returning BOOL with
handler-owned LastError. Dispatch preflights the four-byte size field. A valid
size then reads the full declared span, merges only policy-owned fields into a
before-image, and commits that span once. Native-unwritten service-pack padding
therefore remains byte-identical. NULL, unmapped, cross-page-invalid, or
read-only storage faults before mutation. Unsupported policy and invalid size
also leave guest bytes and memory generation unchanged; invalid size alone
performs the required LastError transition to 122.

The core contract test covers both accepted layouts, preserved padding,
invalid size, disabled policy, NULL, cross-page truncation, and read-only
storage. The compatibility-policy tests reject a non-NT platform and an
overlong service-pack label.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile kernel32-get-version-ex-a
npm run check:runtime:bgi-browser
```

Node completes 1,337 calls across 58 contracts. `GetVersionExA` is sequence
1,337 at instruction 49,890. It writes the declared 148-byte merged span, with
the actual memory delta advancing generation 1,455 to 1,456 on page
`0x006FF000`. The delta payload SHA-256 is
`c5d77359f1995928fdd5e32f1b62753fab61fd3ac16c785b11a1faaf4dfbb92e`.

Execution reaches `CreateWaitableTimerA(NULL, TRUE, NULL)` seven instructions
later. Node records 1,343 writeback batches, 2,537 dirty pages, 9,110 changed
bytes, 85 host deltas, final generation 1,457, and memory SHA-256
`6d5a550c0f16b5097465e1af2f3ffe0c97097d9223b8d2991757ba73a87b0280`.
The 5,458,183-byte report has SHA-256
`55b78e0e2d0d3bcf82de81f25a9e8cf190db41b9b97dc06ab3d2980e4f1410f8`.

Chrome Runtime Worker reproduces the same ordered calls, output, stop, and
memory digest with 6,842 slices, 5,499 yields, 4,969 writeback batches／6,716
pages, 85 mapping deltas, final generation 3,605, and 6,400 Wasm crossings.
Cancel and watchdog cleanup leave no pending host call or callback. The
804,637-byte browser report has SHA-256
`0c58dcbedc816141f8c5575b8accc1f0c17dcaadf4c670cca35ee2ddd359e2ef`.
Runtime-core Wasm is 2,328,559 bytes with SHA-256
`fcd30ec0554ad3b209cad63f324976a24bc84639846fe3f39a2631fc74e422a0`.

The three-minute performance gate was not rerun: this semantic leaf changes no
Worker transport, scheduler, memory topology, graphics, audio, or lifecycle
boundary. The last performance observation remains the explicitly labeled
`winmm-time-begin-period` run and is not claimed as a measurement of this
artifact.

## Non-claims

- The virtual build is compatibility policy, not browser or host OS detection.
- No broad Windows version-lie, application-manifest, compatibility-mode, or
  `VerifyVersionInfo` behavior is implemented.
- Service-pack text is currently bounded ASCII; unobserved Windows-932 labels
  are not generalized.
- Accepted layouts are exactly 148 and 156 bytes. Other sizes are not treated
  as truncated success.

## Next structured stop

```text
KERNEL32.dll!CreateWaitableTimerA(NULL, TRUE, NULL)
endpoint ID:       116
endpoint address:  0x7F000730
IAT address:       0x100D1278
return address:    0x1005053B
instruction count: 49,897
fault:             endpoint-contract-missing
```
