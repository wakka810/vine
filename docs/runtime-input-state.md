# Runtime logical input state

## Status

- Status: keyboard input gateway and mouse-button mapping implemented
- Authority: `runtime/input.rs::RuntimeInput`
- Reached User32 APIs: `SwapMouseButton`, `GetAsyncKeyState`
- DirectInput gateway: root identity and reached empty game-controller enumeration implemented

Logical input state is guest-visible Windows state and therefore belongs to the
Rust runtime. Browser main-thread adapters normalize DOM keyboard, focus, and
pointer events into a versioned Worker message, but they do not own focus,
mouse-button mapping, virtual-key state, pressed-since-query state, generation,
or a second Windows input clock.

## Keyboard state and browser ingress

The bounded keyboard profile `foreground-focused-virtual-key-state-v1` owns:

- source-focus state and a strictly increasing source-event sequence;
- current-down and pressed-since-last-query bits for virtual keys 0 through 255;
- side-specific and generic Shift／Control／Alt aggregation;
- owner-local generation and the last accepted event sequence.

The browser adapter translates stable W3C `KeyboardEvent.code` values and
pointer buttons to Win32 virtual keys. It tags each focus, key, or pointer event
with one monotonic session-local sequence and sends only the normalized event to
the Worker. Up to 256 events may queue before runtime preparation. The Worker
validates the protocol shape, preserves order, and calls the Wasm bridge; Rust
then validates and commits the authoritative state. Queue capacity, malformed
messages, stale sequences, unsupported virtual keys, and profile mismatch fail
before Rust state mutation.

Key-down sets the current-down bit and, unless it is a repeat or duplicate,
arms the pressed-since-query bit. Key-up clears current-down without clearing an
already armed pressed bit. Side-specific modifiers also update their generic
modifier relation. Querying one virtual key consumes only that key's pressed
bit. Source-focus loss clears all down and pressed bits atomically; events while
unfocused cannot create down state. Browser focus is an input source fact, not
the authority for the runtime foreground-window relation.

`GetAsyncKeyState` returns a signed x86 `SHORT`: bit 15 reports current-down and
bit 0 reports pressed since the previous query. The dispatcher sign-extends the
16-bit result into EAX, preserves LastError, and writes no guest memory. A
matching focused source and focused foreground logical window are required.
Invalid virtual keys, an unfocused source, or a missing foreground relation
return zero without consuming the pressed bit or mutating state.

The primary checkpoint posts one deterministic `ControlLeft` key-down event
through the same browser protocol used by live DOM input. Rust aggregates it to
`VK_CONTROL`; the first primary `GetAsyncKeyState(VK_CONTROL)` consumes the
pressed bit and returns sign-extended `0xFFFF8001`. With no second input event,
a later primary poll of the same generic key returns `0xFFFF8000`. Node and
Chrome therefore demonstrate that the pressed bit is one-shot while the
current-down bit remains set across repeated polling. The continuation performs
three additional immutable reads from source object 3 (`sysgrp.arc`) in both
runs. Because no zero-input counterfactual was run, this records ordered source
activity without claiming that the key event caused those reads or produced a
visible gameplay response. Key-up and DirectInput keyboard-device state also
remain outside this checkpoint.

## Mouse-button mapping

The input owner also keeps one configured mouse-button mapping record:

- bounded profile identity;
- initial primary／secondary mapping;
- current swapped state;
- owner-local generation.

Configuration is a mutation-free plan followed by a configure-once commit.
`SwapMouseButton` plans against the current generation and swapped value,
rejects stale commits, returns the previous mapping, and commits only the
requested current value. An idempotent request does not increment generation.
The controlled browser profile begins unswapped and imports neither the native
host setting nor a DOM/browser preference.

Future User32 message translation and DirectInput mouse state must consult this
same owner; they must not install independent button-remapping or virtual-key
tables.

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

The primary workload reaches slot 4 `IDirectInput8A::EnumDevices` with device
class 4 (`DI8DEVCLASS_GAMECTRL`), callback `0x1005BEF0`, reference 0, and flags
1 (`DIEDFL_ATTACHEDONLY`). Two native runs return `S_OK` without invoking the
callback. `Dinput8CompatibilityPolicy` version 1 therefore exposes only this
exact empty attached-game-controller catalog. Invalid class／flags／NULL callback
return `E_INVALIDARG`; valid but unreached variants remain structured
unsupported. The reached completion writes no guest bytes, schedules no
callback, and mutates neither COM nor input state.

## Failure ordering

User32 input operations validate, before mutation:

1. compatibility-policy identity, enablement, and exact input-profile shape;
2. decoded x86 arguments and virtual-key range;
3. configured Rust owner and exact profile match;
4. source-event sequence, source-focus state, and foreground-window relation;
5. owner-local generation and expected before-image at targeted commit.

Policy-disabled, missing-owner, mismatched-profile, stale-event, stale-plan,
capacity, and generation-overflow faults leave guest memory, LastError,
allocator state, input bits, event sequence, and generation unchanged. Browser
resources do not participate in the synchronous query transaction.

## Policy version

`vine.user32-compatibility-policy` version 27 adds the bounded keyboard profile
and `GetAsyncKeyState` contract to the existing display, window, paint, and
mouse-mapping policy. Older versions remain historical evidence; the policy is
an internal typed runtime profile rather than a persisted user format, so no
cross-version deserialization or migration is claimed.

## Verification

Owner tests cover configure-once validation, strict source-event ordering,
repeat／duplicate behavior, side-to-generic modifier aggregation, per-key
pressed-bit consumption, key-up, focus loss, and stale commits. Dispatcher
tests cover x86 stdcall cleanup, signed-SHORT extension, first／repeat queries,
side-specific state independence, key-up, invalid and unfocused queries,
LastError preservation, zero guest writes, and policy／authority failure
atomicity.

The native primary and confirmation traces, controlled two-query Node semantic
run, and Chrome Worker run are recorded in
[windows-native-evidence.md](windows-native-evidence.md). Mouse-mapping evidence
remains in [bgi-swap-mouse-button.md](bgi-swap-mouse-button.md).

## Non-claims

This checkpoint does not claim `GetKeyboardState`, message-queue keyboard or
mouse translation, capture, raw input, callback-producing DirectInput variants,
keyboard／mouse DirectInput devices, acquire／unacquire, buffered input, or
general device state. Pointer normalization is present at the adapter boundary,
but primary pointer consumption is not demonstrated. The bounded controlled
Control event proves transport, one User32 consumer, and pressed-bit consumption
across two held-key queries only; arbitrary live gameplay input, key release,
IME text composition, browser shortcut suppression, and game response remain
future scenario evidence. It does not change host OS input state, and TypeScript
remains an adapter rather than Windows-state authority.
