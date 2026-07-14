# BGI InitializeCriticalSection checkpoint

## Status

- Status: implemented and semantically equivalent in Node and Chrome
- Date: 2026-07-14
- Runtime-kernel checkpoint: `bgi-initialize-critical-section`
- Generic dispatcher: 44 API contracts／267 completed calls
- Compatibility policy: `kernel32-initialize-critical-section-v1`
- Next structured stop: initial-thread stack-growth page fault at `0x006FE000`

This checkpoint implements the reached one-argument
`KERNEL32.dll!InitializeCriticalSection(0x100FD9CC)` call. It reuses the
process-owned critical-section subsystem; it does not introduce a BGI-specific
handler or a second synchronization authority.

## Native evidence

Two focused captures used `--observe critical-section` and completed with 236
balanced API pairs and zero diagnostics each.

| Capture | Bytes / lines | SHA-256 |
|---|---:|---|
| primary r4 | 295,301 / 524 | `62758a647a37e270c4d2f969055c814d7559d1c398f0324ed0243b29135bc689` |
| confirmation r5 | 294,495 / 524 | `ecc73e513eefdee295dc324bd0bff8a699ea8819bc99b6cad7694b336599ebc4` |

The BGI call returns to RVA `0x0009B2C3` and initializes image storage RVA
`0x000FD9CC`, guest address `0x100FD9CC`. The reached storage begins as 24 zero
bytes. Both captures observe the following x86 `RTL_CRITICAL_SECTION` bytes
after return:

```text
FF FF FF FF FF FF FF FF 00 00 00 00 00 00 00 00
00 00 00 00 D0 07 00 02
```

The raw `SpinCount` field is `0x020007D0` (`33,556,432`). LastError remains
`0 -> 0`; the VOID API raises no successful-path exception. A controlled oracle
sets LastError to sentinel `86` immediately before the call and confirms that it
is preserved.

The direct wrapper continuation is `mov eax, esi; pop esi; ret`. Its outer
consumer at RVA `0x000A37BE` compares the wrapper-returned EAX with ESI. This is
wrapper behavior: the one-argument API's EAX is not consumed. The runtime must
therefore preserve EAX for the VOID ABI rather than fabricating a BOOL result.

`InitializeCriticalSection` exposes no caller spin-count argument and is the
zero-spin semantic entry point. The Windows 11 raw structure nevertheless
contains internal dynamic state in its high byte and the measured low value
`0x07D0`. Vine does not reinterpret that private layout as general Win32
semantics or write it into the handler as a title constant.

Raw JSONL traces remain under `.work/windows-traces/` and are not committed.

## Semantics and compatibility policy

`kernel32.initialize-critical-section.v1` is a one-argument stdcall contract
with a checked 24-byte output, VOID return, EAX preservation, callee stack
cleanup, and LastError preservation. The handler:

1. validates the output pointer and policy before mutation;
2. creates one process-owned critical-section record through the existing
   synchronization subsystem;
3. serializes that record to the checked guest output;
4. leaves EAX and selected-thread LastError unchanged.

The versioned policy file
[`kernel32-initialize-critical-section-v1.json`](../fixtures/runtime-policies/kernel32-initialize-critical-section-v1.json)
owns `initializeCriticalSection.spinCountField = 33556432`. A policy without
this value stops with
`initialize-critical-section-policy-unsupported` before runtime or guest-memory
mutation. Policy validation accepts the measured dynamic-state flag family and
rejects unknown high-byte state flags.

The public semantic test uses non-BGI raw field `0x02001234`. It verifies the
VOID ABI, exact checked bytes, LastError preservation, policy-disabled atomic
failure, process-owned state, and mutation-free duplicate-initialization fault.
This keeps caller RVA, BGI address, and reached call order out of the Win32
implementation.

## Generic runtime result

```powershell
npm run check:runtime:bgi -- --profile kernel32-initialize-critical-section
```

Call 211 is the reached initialization:

| Field | Result |
|---|---|
| Instruction | 19,185 |
| Argument | `0x100FD9CC` |
| Return address | `0x1009B2C3` |
| EAX | preserved |
| LastError | `0` |
| Guest write | the measured 24 bytes |
| Host delta | generation `299 -> 300`, page `0x100FD000`, owner `1` |
| Delta SHA-256 | `80eb4797e4759ad5c9f4c484abf155aa7008a2ef8bfdffad9c17319ae36ef0fb` |

The same session continues to 267 completed calls. It includes 5 `HeapSize`,
12 `DecodePointer`, 27 `EncodePointer`, 9 `EnterCriticalSection`, 9
`LeaveCriticalSection`, and 30 each of `GetLastError`, `TlsGetValue`,
`FlsGetValue`, and `SetLastError`. This long continuation is intentionally
checked through the generic dispatcher rather than an API-specific replay list.

The Node report records 268 writeback batches, 406 dirty pages, 5,855 changed
bytes, 67 host deltas, and final generation `365`. Final memory SHA-256 is
`5a7b35e013f2ac108f5d7394dc908ba91a76aaa77dc4e5288d3ee827460c146c`.
The 1,154,575-byte report SHA-256 is
`c396de1128554bad61efabc52e1e10f29896a14e923a75d349751cde2eda7686`.
The hash is artifact identity; semantic assertions are the gate.

## Chrome equivalence and workload gate

`npm run check:runtime:bgi-browser` reproduces all 267 ordered completions and
the endpoint-null execution fault in Chrome `150.0.7871.115`. Chrome uses 2,926
CPU slices, 2,658 budget yields, 2,247 writeback batches／2,921 pages, 67 host
deltas, 8,778 state transitions, and 2,585 Rust/Wasm crossings. Its generation
ends at `1,994`; Node ends at `365` because their reconciliation boundaries
differ. Both produce the same final memory digest and zero pending host calls or
callbacks after success, cancellation, and watchdog cleanup.

The 261,407-byte Chrome report SHA-256 from the checkpoint run is
`d520414931f960e15a6e6feb4e5583d45fee93aef31ee34187505665e1eb04a7`.

The separate 180-second benchmark completes 347 fresh sessions with zero
failure. All 17 versioned thresholds pass. Session P95 is `619.620 ms`, slice
P99 is `1.845 ms`, heartbeat P99 is `15.570 ms`, and maximum heartbeat gap is
`53.280 ms`. The 953,228-byte benchmark report SHA-256 is
`8f9b07aad63170b69c3cef72c45400f2cfbbf7c8e799cccf9e1c3e6c56e008df`.

## Next structured stop

The next stop is not an unimplemented API endpoint:

```text
completed calls:       267
instruction:           22754
EIP:                   0x100A47A9
previous EIP:          0x100A47A7
fault address:         0x006FE000
page-fault error code: 4
cluster:               initial-thread-stack-growth
endpoint:              null
```

`0x006FE000` is the page immediately below the current initial-thread stack
mapping. Node and Chrome both reconcile CPU writes before recording the fault,
then stop the process and DLL lifecycle cleanly. The next checkpoint must define
initial stack reserve／commit／guard growth in the Rust memory authority and
apply its mapping delta through the existing ExecutionEngine boundary.

## Non-claims

- the measured raw spin field is not a portable Windows implementation rule;
- critical-section contention, deletion, waiter scheduling, and cross-thread
  blocking remain outside this checkpoint;
- general stack growth, guard-page exception delivery, additional guest-thread
  stacks, and stack overflow are not implemented;
- no rendering, input, audio, VFS, COM, or full-play completion is claimed.
