# Deterministic test and evidence classification

## Status

- Status: verified
- Date: 2026-07-13
- Contract: `tools/evidence-classification.mjs`
- Public check: `npm run check:evidence-classification`

Vine separates guest-visible regression gates from observations that identify a
particular host run. A field is not promoted to exact semantic ABI merely
because it appears in deterministic JSON.

| Evidence class | Gate |
|---|---|
| Guest-visible semantic value | exact value or an explicit allowed set |
| Ordering required by Win32／BGI | exact sequence or required partial order |
| Opaque identity | equality, stability, and lifetime relations |
| Performance | repeated distribution and versioned thresholds |
| Browser／native nondeterministic value | range, shape, or presence |
| Whole-report hash | artifact identity only |

Input integrity remains separate from output classification. The private BGI
source fingerprint and selected execution-engine Wasm digest are checked before
execution because they identify what code is being run. A generated report hash
identifies the resulting artifact but does not define guest semantics.

## Semantic and ordering gates

The current BGI Node／Chrome comparison checks all 82 guest-visible call results,
arguments, writes, LastError values, instruction stop, final memory content, and
structured fault independently of report serialization. That call stream is an
exact BGI-required sequence.

Scheduler timing uses a partial order. Independent request lanes may be
interleaved, but each request must precede its own completion and every required
completion must precede terminal stop. The public contract accepts two valid
interleavings and rejects completion-before-request. The Runtime Worker state
machine separately rejects illegal adjacent transitions before mutation.

## Historical artifact identity

`fixtures/checkpoints/bgi-startup-win32.json` retains capture-time report bytes
and SHA-256 for provenance. The historical runner now records whether a newly
serialized report matches that artifact and prints `matched` or `changed`.
Mismatch is not a hard failure after the checkpoint's source, calls, writes,
instruction count, next stop, and other semantic assertions pass. The manifest
declares report hashes as `artifact-identity-only`; its public contract no longer
hard-codes a historical report digest as production ABI.

The controlled entry and bootstrap runners use the same observation rule. Their
CPU state, memory ownership, ABI, stop, and lifecycle assertions remain exact;
adding or reordering non-semantic report metadata does not fail solely because
the serialized bytes changed.

## Performance decision

BGI benchmark report schema version 2 stores host／Chrome／Node environment,
configuration identity, raw session and memory samples, sample counts, p50／p95／
p99／maximum distributions, regression slopes, all threshold evaluations, and a
structured decision. The decision records classification
`repeated-distribution-threshold`, pass／fail status, an explicit reason, total
evaluation count, and failure IDs. The whole report hash remains run identity.

The commit-bound private target gate references the latest benchmark as a
separate observation. It does not rerun performance or turn a single timing
sample into the semantic result.
