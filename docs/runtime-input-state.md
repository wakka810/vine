# Runtime logical input state

## Status

- Status: implemented foundation
- Authority: `runtime/input.rs::RuntimeInput`
- First reached API: `USER32.dll!SwapMouseButton`
- DirectInput gateway: `DINPUT8.dll!DirectInput8Create` root identity implemented

Logical input state is guest-visible Windows state and therefore belongs to the
Rust runtime. Browser main-thread adapters may normalize DOM events, but they do
not own mouse-button mapping, focus, capture, key state, timestamps, or a second
Windows input clock.

## Mouse-button mapping

The initial input slice owns one configured mapping record:

- bounded profile identity;
- initial primary／secondary mapping;
- current swapped state;
- owner-local generation.

Configuration is a mutation-free plan followed by a single configure-once
commit. `SwapMouseButton` plans against the current generation and swapped
value, rejects stale commits, returns the previous mapping, and commits only the
requested current value. An idempotent request does not increment generation.
The controlled browser profile begins unswapped and imports neither the native
host setting nor a DOM/browser preference.

The owner exposes immutable mapping inspection and a targeted swap operation to
`RuntimeKernel`. It does not expose its internal optional state for mutation.
Future User32 message translation and DirectInput mouse state must consult this
same owner; they must not install independent button-remapping tables.

## DirectInput 8 root boundary

`DirectInput8Create` is the reached DirectInput subsystem gateway. The semantic
handler in `dinput8.rs` owns the exact five-word stdcall／HRESULT contract and
accepts the reached DirectInput version `0x0800` with `IID_IDirectInput8A` or
`IID_IUnknown`. It validates the output span, IID, version, and aggregation
policy before allocating identity. Invalid variants clear a valid output slot
and return a concrete HRESULT without leaving a COM object behind.

On success, `RuntimeCom` remains the only authority for object identity,
canonical IUnknown identity, interface identity, reference count, backing
allocation, and vtable endpoints. `ExecutionSession` preflights the COM install
and four-byte guest output together, then commits both as one transaction. The
published `IDirectInput8A` interface has the complete 11-slot x86 vtable:
IUnknown slots use the shared COM implementation, while unreached DirectInput
methods remain typed unsupported endpoints. Browser TypeScript creates no
parallel COM object or DirectInput state.

The primary workload now reaches slot 4 `IDirectInput8A::EnumDevices` with
device type 4, callback `0x1005BEF0`, reference 0, and flags 1. Enumeration,
callback delivery, device identities, acquisition, and state polling belong to
the next vertical slice; the root factory does not silently claim them.

## Failure ordering

The User32 handler validates, in order:

1. compatibility-policy identity and input-profile shape;
2. policy enablement for the mapping API;
3. decoded x86 argument availability;
4. configured Rust owner and exact profile match;
5. owner-local generation and expected previous value at commit.

Policy-disabled, missing-owner, mismatched-profile, stale-plan, and generation
overflow faults occur without guest writes, LastError change, partial input
mutation, or allocator consumption. Browser resources do not participate in
this synchronous transaction.

## Policy version

`vine.user32-compatibility-policy` version 2 adds the logical input profile to
the existing display profile. Version 1 remains historical evidence; it is not
deserialized or migrated because User32 policy is currently constructed as an
internal typed profile and is not a persisted user format. Reverting the
checkpoint means selecting the prior implementation commit and version-1
evidence, not interpreting a version-2 record as version 1.

## Verification

Owner tests cover configure-once, invalid profile, idempotent generation,
changed mapping, and stale commit. Dispatcher tests cover x86 stdcall cleanup,
BOOL previous-state results, both possible initial states, the reached two-call
restore sequence, LastError preservation, zero guest writes, policy-disabled,
missing-owner, and mismatched-owner atomicity.

Mouse-mapping evidence is documented in
[bgi-swap-mouse-button.md](bgi-swap-mouse-button.md). Current DirectInput native,
Node, Chrome, and benchmark evidence is recorded in
[windows-native-evidence.md](windows-native-evidence.md),
[testing-and-acceptance.md](testing-and-acceptance.md), and
[bgi-browser-benchmark.md](bgi-browser-benchmark.md).

## Non-claims

This foundation does not yet ingest DOM input, emit mouse messages, implement
focus／capture, expose button count or metrics, provide keyboard state, enumerate
DirectInput devices, create keyboard／mouse devices, acquire them, or serve
device state. It does not change the host OS mapping. Those later slices must
extend this owner or introduce a separately justified input owner by lifetime
and failure domain; TypeScript remains an adapter, not Windows-state authority.
