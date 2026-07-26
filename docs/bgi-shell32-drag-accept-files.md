# BGI `DragAcceptFiles` checkpoint

## Status

- Status: semantic checkpoint verified; browser performance gate blocked
- Date: 2026-07-26
- Runtime-kernel checkpoint: `shell32-drag-accept-files`
- Previous checkpoint:
  [`kernel32-leave-critical-section-continuation`](bgi-leave-critical-section-continuation.md)
- Next owned endpoint: `USER32.dll!MessageBoxA`

This checkpoint closes the reached `SHELL32.dll!DragAcceptFiles` leaf without
creating a separate shell or browser window-state authority. The API toggles
the existing `WS_EX_ACCEPTFILES` bit in the Rust-owned logical window's
effective extended style. The reached call passes `FALSE`, so the already
clear bit remains clear and the idempotent plan does not advance the window
generation.

## Native oracle

Two independent focused BGI traces contain one balanced `DragAcceptFiles` pair
each and no diagnostics. The primary trace is 23,308 bytes／77 lines with
SHA-256
`939c2094e929ba80e867ec73586e41b0d411d6f657def08d11c44212795ae412`;
the confirmation is 23,236 bytes／77 lines with SHA-256
`05924e0335698b3081a79b36d62f0a9a9f3c519e673d61a63229b2fad52b1834`.
The source executable fingerprint remains unchanged.

The BGI caller returns to RVA `0x0005F272`, passes its created main HWND and
`FALSE`, observes LastError `0 -> 0`, and does not consume a return value.
A separate live-window oracle applies `TRUE` and then `FALSE`. `GWL_EXSTYLE`
shows the accept-files bit set and then cleared. Windows clears both LastError
sentinels to `0`; therefore this reached contract sets LastError to `0` rather
than claiming preservation.

Raw traces and summaries remain under ignored `.work/` paths. Their artifact
identities and the structured oracle values are recorded in
[current-checkpoint.json](current-checkpoint.json).

## Runtime contract

`shell32.drag-accept-files.logical-window-style-v1` is an x86 stdcall contract
with two arguments, callee cleanup, `VOID` result semantics, and EAX
preservation. Dispatch validates the exact live logical HWND and created
lifecycle, asks `RuntimeUser32` for a mutation-free style plan, rejects stale
or duplicate commits, and updates only the owner record. Unknown, stale, or
wrong-lifecycle handles leave window state, generation, allocators, guest
memory, and LastError unchanged.

The effective extended style is the sole file-drop acceptance authority.
TypeScript receives an immutable observation only. No DOM drag listener,
browser resource, native HWND, or second acceptance flag is created.

## Node and Chrome evidence

Node and clean-profile Chrome both complete the call at sequence `89,249` and
instruction `15,601,759`, with arguments
`0x48010001, 0x00000000`, return address `0x1005F272`, preserved EAX,
LastError `0`, and zero guest writes. Protocol version 20 carries a compact
typed `dragAcceptFiles` snapshot containing the call, logical window, style
authority, browser-resource non-claim, and `WM_DROPFILES` non-claim.

The complete profile reaches 89,283 supported calls and stops at instruction
15,610,457 on endpoint ID `221`, `USER32.dll!MessageBoxA`. Node and Chrome
match semantic SHA-256
`61a51e57987cc3644926f1e61615ad630c48477334dd5b9c1e9a32cf985e6977`
and final memory SHA-256
`2f041d4df7be96665dfa3bcdce1c6c21aea92e1e486ef025ab834b7e692da31e`.
Exact report and artifact identities remain manifest-owned.

## Performance blocker

The bounded timer-driver remediation replaces repeated timeout churn with one
`MessageChannel` deadline driver and coalesces only synchronous API-boundary
yields at a quantum of 64. Cancel, watchdog, and asynchronous host-wait
boundaries remain immediate. The ordinary Chrome semantic run improved from
about 73.2 seconds to about 15.3 seconds without changing either semantic
digest.

The one accepted 180-second benchmark observation used a valid environment and
zero semantic failures, but completed 14 sessions against a minimum of 24 and
recorded session P95 15,868.955 ms against a maximum of 8,000 ms. The other 15
versioned thresholds passed. A preceding run was discarded because the
environment monitor found sustained external CPU; it is diagnostic evidence,
not a confirmation or pass.

This is a structured feasibility blocker. Thresholds are unchanged. Resumption
requires profiling a correctness-preserving Rust／Wasm dispatch fast path, or
an explicit versioned performance-contract decision, followed by the same
180-second gate.

## Reproduction

```powershell
npm run check:runtime:bgi -- --profile shell32-drag-accept-files
node tools/run-browser-runtime-check.mjs --bgi `
  --bgi-profile shell32-drag-accept-files `
  --bgi-policy fixtures/runtime-policies/kernel32-wait-for-single-object-ex-v1.json
npm run check:runtime:bgi-browser-benchmark:prepared
```

The benchmark command is expected to fail while the structured blocker is
active. Do not retry it as a pass-producing mechanism or relax its thresholds.

## Non-claims

- `DragAcceptFiles` acceptance state does not implement `WM_DROPFILES`,
  `HDROP`, `DragQueryFile`, browser drag events, file permission, or VFS import.
- The idempotent `FALSE` call is not evidence for arbitrary window style
  mutation or multiple-window file-drop routing.
- Semantic equivalence does not override the active performance blocker.
- This checkpoint does not establish rendering, input consumption, audible
  output, save／restart／load, representative flow, or full-play acceptance.
