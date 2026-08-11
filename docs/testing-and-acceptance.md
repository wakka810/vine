# Testing and acceptance

## Principle

Verification effort scales with blast radius. A reached semantic leaf does not require full Node replay、private Chrome equivalence、manifest rewrite、or a new evidence profile. Heavy acceptance is reserved for user-visible milestones and genuine authority／storage architecture changes.

## Leaf development checks

Use while implementing reached API／COM leaves:

- owner-local Rust unit or model test for the successful reached variant
- the smallest negative case needed to protect pointer、owner、lifetime、range、or failure atomicity
- ABI／dispatch fixture when calling convention、stack cleanup、guest output、or callback entry changes
- TypeScript typecheck only when browser adapter code changes
- generic forward run after the focused check

Do not add all negative combinations, a native trace, a full browser replay, or a long document for a straightforward leaf.

## Subsystem gateway checks

For a new file、graphics、audio、input-device、storage、or threading boundary, verify the smallest vertical slice:

- correct Rust authority and identity namespace
- input and output pointer／range validation
- lifecycle and stale completion rejection
- cross-owner preflight and failure atomicity
- one controlled fixture through the boundary
- release browser adapter behavior when a browser resource is involved
- cancel／timeout cleanup for async host work

Then return to the target forward run. A gateway is not a reason to build unreached framework breadth.

## Milestone acceptance checks

Run once after a real boundary is reached:

- readable frame or visible input response
- primary audible output
- save／restart／load
- representative flow
- full play／long-run
- architecture change affecting authority or persistent data safety

Select only relevant checks, then run the canonical milestone command from[current-checkpoint.json](current-checkpoint.json). A milestone run may include:

- full Node semantic replay
- private release Chrome run and browser adapter validation
- broader Rust workspace regression and public fixture E2E
- source hash before／after and writable-overlay isolation
- bounded user-visible artifact such as a frame or audio diagnostic
- current manifest and generated README update

If guest execution already produced a reusable artifact, validator／projection fixes must reuse it. Do not rerun the guest solely for report formatting、regex、whole-report hash、or pacing-dependent terminal metadata.

## Optional full and historical checks

Stress、benchmark、fuzz、historical capability profiles、and exhaustive report projection are manual tools. Run them for measured performance／memory risk、long-run acceptance、or investigation of a regression in that historical boundary. They are not default development gates.

## Forward report acceptance

The normal report must remain bounded, targeting hundreds of KiB and at most a few MiB. It contains:

- stop reason and endpoint／COM identity
- caller、arguments、return address、minimal registers／stack、LastError、current thread
- last 256-512 semantic completions with guest writes reduced to address and length／digest
- latest browser presentation and Present count
- input、audio、source read、writable commit／directory summary
- pending callback、host request、message information
- elapsed time、small optional digest、and reproduction command

It does not contain every call、expanded large guest writes、recursive prerequisite checkpoints、or raw private bytes.

## Oracle selection

- Microsoft or subsystem documentation for ordinary API contracts
- Rust model tests for authority、identity、lifetime、and transaction invariants
- controlled PE fixtures for ABI、guest memory、callback、and loader integration
- Windows native evidence only when documented behavior is ambiguous、caller uses subtle result／LastError／ordering、identity／lifetime is unclear、or a real divergence exists
- Node and Chrome full comparison only at milestone acceptance

## Default gates

`npm run check:fast` is the normal local gate. `npm run check:milestone` is the user-visible acceptance gate. `npm run check:full` adds optional broad and historical checks. Exact definitions live in[checks-and-ci.md](checks-and-ci.md) and `package.json`.

## Definition of done

A development batch is done when it reaches a user-visible milestone、a subsystem gateway with a working minimal slice、or a true blocker; its reached semantics are correct in the owning subsystem; focused regression is green; source／save and authority invariants are preserved; and the next forward command is reproducible.

API count、report generation、or one newly supported endpoint alone is not done.
