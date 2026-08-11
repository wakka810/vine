# Structural risks and open questions

This file contains only issues that can change feasibility or long-lived architecture. Current endpoints and ordinary unsupported APIs belong in the forward report and[current-checkpoint.json](current-checkpoint.json), not here.

## R1. Browser execution throughput

Risk: full-play may fail real-time CPU、memory、or responsiveness requirements once rendering and audio are active together.

Known evidence: release Chrome execution has exceeded the logical 2M guest-instruction-per-second clock model in prior measured runs, but long-run gameplay is not proven.

Decision trigger: a versioned representative workload misses an explicit frame／audio deadline or shows unbounded memory. Profile before changing v86、adding DBT/JIT、or requiring SharedArrayBuffer.

## R2. Graphics completeness

Risk: current bounded D3D9 fixed-function path may not cover later texture formats、blend states、device loss／Reset、or presentation patterns needed by the title.

Decision trigger: a reached call or visible divergence identifies a missing state or resource lifecycle. Extend the existing Rust frontend／logical resource／browser renderer split with the smallest vertical slice.

## R3. Long-run audio transport

Risk: the reached synchronous DirectSound path produces non-zero primary PCM through a running Web Audio context, but suspend／resume, later formats, and long-running scheduling may still diverge.

Decision trigger: a representative flow exhibits measured underrun, drift, stale playback after cancel/suspend, or unbounded buffering. Preserve Rust buffer／clock authority and evaluate bounded ring-buffer or worklet transport only from that measured behavior.

## R4. Persistent storage safety

Risk: browser permission、quota、site-data deletion、crash recovery、or concurrent sessions may violate save durability expectations.

Decision trigger: primary save／load reaches the browser service. Acceptance must include source hash invariance、generation conflict、cancel／crash atomicity、restart/load、and wrong-title／wrong-profile rejection.

## R5. Long-run lifecycle

Risk: thread、COM、graphics、audio、timer、or browser resource leaks may only appear across many scenes or save/load cycles.

Decision trigger: representative flow is complete. Add bounded counters and a scenario soak; do not build a general proof system before the workload exists.

## R6. Japanese text and locale breadth

Risk: Windows-932 conversion、font metrics、IME、or glyph rendering may diverge in later text paths.

Decision trigger: readable-frame work reaches incorrect text or a new locale/font variant. Use a native oracle only for ambiguous metrics or caller-sensitive behavior.

## True blocker format

When a risk becomes a blocker, record:

- exact reproduction command and first failure
- endpoint／caller requirement and acquired evidence
- attempted bounded options
- source／save／authority invariant that must be preserved
- alternatives that change long-term architecture
- concrete resume condition

Call budget、stale manifest、old validator、report mismatch、or a normal unsupported endpoint is never a structural blocker.
