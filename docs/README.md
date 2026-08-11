# Vine documentation

## Active source order

1. [Current state](current-checkpoint.json): active target、user-visible milestone、latest generic stop、commands
2. [Requirements](requirements.md): product behavior and scope
3. [Target architecture](target-architecture.md): stable authority and safety boundaries
4. [Roadmap](roadmap.md): scenario order and exit conditions
5. [Testing and acceptance](testing-and-acceptance.md): leaf、gateway、milestone verification
6. [Checks and CI](checks-and-ci.md): canonical commands
7. [Structural risks](risks-and-open-questions.md): feasibility and long-lived architecture questions
8. [Corpus inventory](corpus-inventory.md): private target fingerprints and dependencies

`AGENTS.md` defines the forward-first working loop. The root README status is generated from the compact current state.

## Stable subsystem references

The following documents remain useful when their subsystem is reached:

- [Guest memory](guest-memory-ingress.md) and [memory transactions](memory-delta-and-transactions.md)
- [Process and thread lifecycle](runtime-process-thread-lifecycle.md)
- [Runtime object model](runtime-object-model.md)
- [VFS](runtime-vfs.md)
- [Message queue](runtime-message-queue.md) and [input state](runtime-input-state.md)
- [GDI frontend](gdi-frontend.md) and [D3D9 frontend](d3d9-frontend.md)
- [Browser Runtime Worker](browser-runtime-worker.md)
- [Browser native storage ADR](adr-010-browser-native-storage.md)

These describe stable boundaries, not the active implementation queue. Where a historical statement conflicts with the compact current state or active docs, the active source order above wins.

## Historical material

Files named `bgi-*.md`, `v86-bgi-*.md`, old checkpoint manifests, native trace narratives, and per-occurrence evidence are historical engineering records. They are frozen, not active requirements, not roadmap order, and not a reason to add another profile、schema、validator、or document.

Do not spend forward-development time moving or editing them one by one. Consult a specific historical file only when diagnosing the same reached behavior or recovering a non-obvious oracle.

## Update policy

- Update current state and generated README at a user-visible milestone or true gateway, not for every API.
- Update architecture only for a stable boundary decision.
- Update requirements or roadmap only when product scope or scenario order changes.
- Put raw and run-specific evidence in `.work/`.
