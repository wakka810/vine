# Checks and CI

## Canonical commands

### Forward development

```sh
npm run dev:target
```

Builds the release Wasm／browser runtime, launches the private target in Chrome, ignores frozen capability-checkpoint terminals, and writes a bounded report under `.work/target-forward/`. It stops at the next real unsupported／fault／milestone. It does not run a Node oracle or generate tracked evidence.

### Fast gate

```sh
npm run check:fast
```

The normal leaf-development gate covers compact manifest／README consistency, active documentation links and formatting, JavaScript syntax, browser TypeScript contracts, and focused Rust model tests. It does not inspect tracked private evidence freshness.

### Milestone acceptance

```sh
npm run check:milestone
```

Run only after a user-visible milestone or authority／storage architecture change. It adds public fixture E2E, broader Rust regression, and validation of the bounded private-target artifact already produced by the forward run. It does not rerun a correct guest solely to regenerate evidence. Full Node／Chrome historical evidence remains an explicit manual choice when the milestone needs it.

### Full／historical

```sh
npm run check:full
```

Adds optional historical checks after the milestone gate. Performance／stress、full private replay、or fuzz checks remain explicitly selected manual commands. It is not a routine semantic-leaf gate.

## Focused commands

Use package scripts or direct `cargo test -p vine-core <test-name> --locked` for the owner being changed. Rebuild the browser Worker only when Rust/Wasm or browser adapter code changed. Reuse prepared v86 artifacts in `.work/` when their source identity is unchanged.

## CI policy

CI keeps fast static/model checks separate from public browser fixture acceptance and optional observations. Private target data is not required on public CI. Historical report hashes and tracked private evidence are not merge gates.

Performance and stress jobs are observations unless a versioned milestone workload has an explicit threshold. A failure may not be hidden by retries、skips、or relaxed thresholds; classify environment invalidity separately from workload failure.

## Artifact policy

Commit source、fixtures safe for the public repository、compact current state、and stable docs. Do not commit private executables／DLLs／assets、save data、raw traces、full reports、Chrome profiles、`.work/` caches、or dependency build output.
