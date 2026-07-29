# Commit-bound private target gate

## Status

- Status: verified
- Date: 2026-07-26
- Canonical command: `npm run check:private-target`
- Evidence schema: `vine.private-target-evidence` version `6`
- Evidence file: [`private-target-evidence.json`](private-target-evidence.json)

## Purpose

Public fixtures cannot prove that the private primary target still reaches the
current structured stop. The private target gate binds one exact Git
implementation commit to the fixed BGI source, the current runtime profile, the
Node semantic oracle, and the Chrome Runtime Worker result. Raw reports and the
private executable remain under local or ignored paths.

The evidence commit follows the implementation commit. A commit cannot contain
its own stable Git object ID, so the workflow deliberately uses two local
commits and pushes them together:

```text
implementation commit
  -> run the private target gate against that exact HEAD
  -> generate the small evidence manifest
  -> evidence commit referring to the implementation commit
  -> push both commits
```

The offline evidence validator confirms that the referenced implementation
commit exists, is an ancestor, and has the recorded tree ID. Every descendant
change through the current HEAD must be limited to
`docs/private-target-evidence.json`; any later runtime, tool, dependency,
fixture, or checkpoint change makes the evidence stale.

The linked evidence file is the current result authority. Human-readable docs
do not duplicate its commit, call, timing, or artifact values.

## Command

On a clean implementation checkout:

```powershell
$env:VINE_BGI_EXE = "V:\softs\サクラノ詩\BGI.exe"
npm run check:private-target
```

Unrelated working-tree changes may be admitted individually and are recorded in
the result. Runtime inputs cannot be admitted this way:

```powershell
npm run check:private-target -- `
  --allow-dirty AGENTS.md
```

Changes below `browser/`, `crates/`, `fixtures/`, or `tools/`, the dependency
locks, `package.json`, the Rust toolchain file, and
`docs/current-checkpoint.json` always fail preflight. This prevents an evidence
manifest from claiming a commit while executing different runtime inputs.

## Evidence contract

The version 6 result records:

- implementation commit, tree, complete dirty-path list, and admitted paths;
- BGI bytes and SHA-256 without its local path or content;
- current-checkpoint, runtime profile, compatibility-policy, Node-report, and
  browser-protocol schema identities;
- Chrome, Node, Rust, Cargo, LLVM, and v86 adapter identities;
- Node and Chrome outcomes, call count, canonical observable-call SHA-256,
  instruction stop, final memory digest, generations, cancellation, watchdog,
  and pending-work counts;
- capability completion identity, required／observed count, last completion,
  final memory identity, and the following owned endpoint when the checkpoint
  ends at a completed capability rather than an unsupported API;
- Chrome success／cancel／watchdog final execution state, transition count,
  pending request records, and watchdog thread／request／last-transition context;
- pinned execution-engine profile, source revision, CPU-context ABI, adapter
  artifact, and report/build identity agreement;
- Node, browser, toolchain, adapter, build, Wasm, and lockfile artifact hashes;
- the referenced benchmark's distribution classification, structured decision
  reason／evaluation count／failure IDs, and artifact hash without promoting it
  into the semantic gate;
- a payload digest covering the evidence fields.

The required command uses summary evidence. Node and Chrome independently hash
the complete guest-visible call stream after sequence ordering, while detailed
call／writeback／host-delta payloads are retained only by an explicit full
diagnostic run. Each committed semantic report must remain at or below 4 MiB.
Chrome progress status records are likewise bounded diagnostic evidence: the
gate retains the initial observations and the latest status while separately
counting every status received.

Source fingerprint mismatch, protected dirty input, Node／Chrome semantic digest
divergence, an oversized summary report, a different structured stop, pending
work, or malformed evidence is a hard failure. The output is written atomically
only after all assertions pass.

For a capability checkpoint, `nextEndpoint.api` records the semantic Win32／COM
identity. If the prepared runtime catalog has not yet assigned that endpoint a
semantic symbol, `nextEndpoint.observedModule` and `observedSymbol` separately
record the exact raw report identity. The online gate and offline evidence
validator compare the raw identity, endpoint ID, and address without treating
the raw adapter symbol as guest-visible API semantics.

## Performance classification

The command runs the required current-session semantic gate. It records the
latest versioned BGI benchmark as
`observation-not-semantic-gate`; it does not rerun the three-minute workload or
promote one timing sample into a semantic result. Performance remains the
separate `npm run check:runtime:bgi-browser-benchmark` observation.

## Non-claims

This gate does not make the private binary available to public CI, replace the
public deterministic fixture suite, prove full play, or turn whole-report hashes
into semantic ABI. Report hashes identify the observed artifacts; semantic
fields are validated independently.
