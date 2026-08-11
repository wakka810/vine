# Vine roadmap

## Planning rule

The roadmap is scenario-first. Work is selected by the shortest path to the next user-visible boundary, not by import order、API inventory、call count、or historical checkpoint sequence. The current stop and commands are in[current-checkpoint.json](current-checkpoint.json).

## 1. Visible action response or readable frame

Reached on 2026-08-12: release Chrome advanced the primary surface from black to a non-black title menu with readable `Start` and `Load` entries while primary audio remained active.

Exit condition:

- primary target shows readable non-black title content, or
- a real pointer／keyboard action causes a repeatable visible frame change

Remaining work:

- retain the same graphics authority and bounded frame diagnostics through later scenes
- confirm input-driven menu selection while progressing the save/restart/load scenario

## 2. Primary audio

Reached on 2026-08-12: the primary title filled a DirectSound streaming buffer, armed position notifications, entered looping playback, and sent non-zero PCM to a running Chrome Web Audio context. Broader format coverage and long-run stability remain later acceptance work.

Exit condition:

- the primary title produces audible BGM or effect audio through Web Audio
- play／stop／loop and logical cursor state remain Rust-authoritative
- cancel and browser suspend leave no stale audio resource

Remaining unknowns:

- format and control variants reached later in the title
- suspend／resume and long-run scheduling stability
- any later WinMM／MCI fallback

## 3. Save, browser restart, and load

Current work:

- select `Start` through the existing pointer path and follow the first scenario transition
- classify the already reached writable directory and commit as configuration or save behavior
- project the next reached persistence endpoint into the bounded forward report

Exit condition:

- primary title creates or updates save／configuration state in the writable overlay
- source hashes remain unchanged
- browser and local service restart, then title reload, restores the committed state
- stale generation、quota failure、and cancel expose no partial commit

Main unknowns:

- exact reached file／registry operations and locking pattern
- title close／flush semantics

## 4. Representative flow

Exit condition:

- a bounded real scenario covers launch、readable rendering、multiple inputs、audio、transition、save、restart、load
- no unsupported stop occurs in that flow
- diagnostics remain bounded and actionable

Main unknowns:

- DirectInput breadth、font／locale variants、movie or transition effects
- resource lifetime across longer scene changes

## 5. Full play and long run

Exit condition:

- all routes、choices、ending、and long-run playback complete
- memory／browser resources remain bounded
- source is immutable and save data survives repeated restart/load
- no required active-scope behavior is silently stubbed

## Deferred

PE32+／x64、D3D11／DXGI、XAudio2、Unity、multi-title generalization、public distribution remain deferred until the primary full-play target is complete or the user explicitly changes scope.

## Stop handling

- semantic leaf: implement in the correct owner, run focused checks, continue immediately
- subsystem gateway: implement the smallest end-to-end slice, then continue
- true blocker: record exact reproduction、first failure、known evidence、options、protected invariant、and resume condition
