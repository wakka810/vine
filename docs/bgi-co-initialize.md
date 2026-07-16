# BGI `CoInitialize` checkpoint

## Status

- Status: verified
- Date: 2026-07-16
- Runtime-kernel checkpoint: `bgi-co-initialize`
- Previous capability checkpoint: `bgi-enum-display-monitors`
- First unsupported capability: `kernel32-get-system-default-lang-id`

This checkpoint connects the first reached Ole32 API to the existing COM ABI
foundation. COM object/interface identity remains owned by `RuntimeCom`; the
apartment model and successful initialization count are thread-local state
owned by `RuntimeThreads`.

## Native evidence

The focused observer records the BGI call, its immediate HRESULT consumer, and
a balanced same-thread `CoInitialize`／`CoInitializeEx`／`CoUninitialize`
oracle:

```powershell
py tools/windows/trace.py `
  --exe 'V:\softs\サクラノ詩\BGI.exe' `
  --output .work/windows-traces/bgi-co-initialize-20260716-r4.jsonl `
  --seconds 1.05 --kill-after --observe com-initialization
```

| Capture | Bytes | Lines | Balanced API pairs | SHA-256 |
|---|---:|---:|---:|---|
| Primary r1 | 154,928 | 457 | 196 | `a85aea76149237c60508993ec1dd3e421ba69fc89bb2d4cfb6c3b59dae65e70e` |
| Confirmation r4 | 60,989 | 151 | 49 | `b47ad8ae39f7451852bac6e2c59945a01599b94af970cbb450d9d10af8ed52cb` |

Both accepted captures are balanced and have zero diagnostics. Shorter r2／r3
captures were rejected because process termination left unrelated D3D／dynamic
export calls open; they are not evidence for this checkpoint.

BGI calls `CoInitialize(NULL)`, returns to RVA `0x00087399`, and consumes EAX
with `test eax,eax; js failure`. It therefore distinguishes failed HRESULTs by
the sign bit and does not read LastError. The balanced oracle fixes these
same-thread transitions:

| State / call | HRESULT | State after call |
|---|---:|---|
| uninitialized -> STA | `S_OK` (`0x00000000`) | STA count 1 |
| STA -> STA | `S_FALSE` (`0x00000001`) | STA count 2 |
| STA -> MTA | `RPC_E_CHANGED_MODE` (`0x80010106`) | unchanged |
| MTA -> MTA | `S_FALSE` | MTA count incremented |
| MTA -> STA | `RPC_E_CHANGED_MODE` | unchanged |
| balanced final uninitialize | void | apartment removed at count zero |

Every successful initialization is balanced by one `CoUninitialize` in the
oracle. Repeat and changed-mode calls preserve LastError sentinels. First
successful initialization was observed both clearing a sentinel to zero and
preserving a sentinel in the same execution context, so first-success LastError
is treated as unspecified API-external state rather than imported semantics.

## Rust authority and failure ordering

`RuntimeThreads` owns `RuntimeComApartmentState { model,
initialization_count }` inside each thread record. Initialization and
uninitialization use mutation-free plans and reject stale commits. The rules
are:

- first initialization creates count 1;
- same-model repeat uses checked increment and rejects overflow before mutation;
- changed model returns the existing apartment as an outcome without mutation;
- uninitialize decrements a successful count and removes state at zero;
- duplicate uninitialize and non-running thread operations are structured
  errors without mutation;
- thread exit removes its apartment without affecting another thread.

`RuntimeKernel` only selects the current thread and coordinates the owner plan
and commit. It does not hold a second apartment collection. `RuntimeCom` remains
the independent authority for COM object, interface, endpoint, and reference
count identity.

`ole32.co-initialize.v1` is one-argument x86 stdcall with a typed HRESULT return
schema and LastError preservation. The reached bounded variant accepts only a
NULL reserved pointer and requests STA. Non-NULL reserved values stop before
runtime or guest-memory mutation. The handler contains no BGI address, hash, or
occurrence branch.

The Runtime Worker serializes the Rust-owned apartment snapshot only on the
matching completion. TypeScript does not create or mutate apartment state.

## Node and Chrome evidence

```powershell
npm run check:runtime:bgi -- --profile ole32-co-initialize
npm run check:runtime:bgi-browser
npm run check:runtime:bgi-browser-benchmark
```

| Metric | Node oracle | Chrome Runtime Worker |
|---|---:|---:|
| Implemented contracts / completed calls | 51 / 1,294 | 51 / 1,294 |
| Instructions at next stop | 45,691 | 45,691 |
| Writeback batches / pages | 1,300 / 2,460 | 4,525 / 6,223 |
| Host / mapping deltas | 80 | 80 |
| Callback entries / maximum depth | 1 / 1 | 1 / 1 |
| Final generation | 1,409 | 3,251 |
| Memory SHA-256 | `b7e5005417aa49c642ded6fdb078be8fe1750e9853cbbcf19e1996d9fd4fad4b` | same |

Call 1,294 runs at instruction 45,685 with argument NULL, returns `S_OK`,
preserves LastError 0, writes no guest bytes, performs stdcall cleanup, and
reports STA count 1 for thread 8192. Instruction 45,691 reaches the same next
endpoint and memory digest in Node and Chrome.

The 5,283,584-byte Node report has SHA-256
`9b3bffce1eedcdb39f0d243b588b1e488ad98f75198e610dbd7dc617576e73dd`.
Chrome `150.0.7871.115` uses 6,305 slices, 5,005 budget yields, and 5,908 Wasm
bridge calls. The 776,262-byte browser report has SHA-256
`77634dd1739ccdbd0423abfed455d6ed6cc71b13d7be6cbce53d04b438073235`;
success, cancellation, and watchdog all retire pending callbacks and host work.

The runtime-core Wasm is 2,276,914 bytes with SHA-256
`39f3ddde2d4e7e168e3ec9d059e3f5d47e52b205f5887408b29020fbbc16ec1e`.
The unchanged 180-second configuration completes 198 sessions with zero
failures and passes all 17 thresholds. Session p95 is `944.460 ms`, slice p99
`4.900 ms`, heartbeat p99 `10.255 ms`, and maximum heartbeat gap `37.580 ms`.
The 949,698-byte benchmark report has SHA-256
`436f347bb8b12e1dd0c2afb14cd0f02cd51bb8400dfc10558d074cb69e58bacf`.

## Non-claims

This checkpoint does not expose guest `CoInitializeEx` or `CoUninitialize`,
implement neutral-threaded or application STA variants, pump apartment
messages, marshal interfaces across threads, or provide class registration,
class factories, `CoCreateInstance`, BSTR, VARIANT, or automation. The native
oracle exercises `CoInitializeEx` and `CoUninitialize` only to establish the
state contract behind reached `CoInitialize`.

It does not connect a COM apartment to D3D9, DirectInput, DirectSound, browser
resources, or host COM. Apartment serialization is diagnostic evidence, not a
TypeScript authority or persistence format.

## Next structured stop

```text
KERNEL32.dll!GetSystemDefaultLangID()
endpoint ID:       39
endpoint address:  0x7F000260
IAT:               0x100D1144
return:            0x1006A556
instruction:       45691
fault:             endpoint-contract-missing
cluster:           kernel32-api-contract
```

This is a semantic leaf in the existing locale compatibility boundary. The next
checkpoint must establish the native LANGID, caller use, LastError policy, and
whether the value belongs in the existing versioned Kernel32 locale policy.
