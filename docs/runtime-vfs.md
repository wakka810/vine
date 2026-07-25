# Runtime VFS, immutable sources, and writable profiles

## Status

- State owner: Rust `runtime/files.rs::RuntimeFiles`
- Aggregate coordinator: `runtime/kernel.rs::RuntimeKernel`
- Immutable adapter: same-origin `storage.source-read.v1`
- Writable adapters: same-origin `storage.profile-read.v1` and
  `storage.profile-commit.v1`
- Durable backend: title/profile-scoped SQLite under ignored `.work/`
- Primary save/browser-restart/load acceptance: not yet complete

This document defines the stable storage contract. Current call counts, artifact
identities, and the next structured stop remain in
[`current-checkpoint.json`](current-checkpoint.json).

## Classification and authority

Executable, DLL, and asset bytes are immutable source objects. Save data, settings,
and global state files are writable profile objects. A path can belong to only one
class: registration rejects overlap between the source and profile path indexes.
When both mounts exist they must have the same title fingerprint and normalized
Windows root.

`RuntimeFiles` privately owns normalized Windows-932 path identity, monotonic file
IDs, open-file access/share/cursor state, profile generations, committed and staged
bytes, dirty revisions, and pending read/commit records. `RuntimeObjectTable` owns
typed handle generation and staleness. The Worker and local server never own a
Win32 handle, cursor, guest pointer, LastError, or dirty-file decision.

An immutable `vine.source-snapshot-manifest` version 1 binds a title and snapshot
to source/object IDs, paths, byte lengths, and SHA-256 values. The source server
validates and caches the configured host bytes and accepts no guest-supplied host
path.

A `vine.writable-profile-snapshot` version 2 binds a title fingerprint, profile ID,
root, quota, and path allowlist to per-object existence, generation, length, and
hash. It is metadata-only: committed bytes are never expanded into JSON or
concatenated with the executable and policy Wasm bootstrap. Before guest entry,
the Worker reads each existing object through generation-bound binary ranges of at
most 1 MiB, verifies the complete SHA-256, and hydrates that object into Rust in one
plan/commit operation. Source ID zero is reserved for profile reads; object identity
remains explicit. Rust owns the hydrated committed bytes and all staged mutations
until a durable commit succeeds.

`BGI.gdb` is a writable profile object, not an immutable asset. On creation of a
new profile database, an existing distribution copy may be read once as migration
seed so current user state is retained. An existing database is never reseeded.
If neither the database nor seed exists, the object begins absent and the guest can
generate it through `OPEN_ALWAYS`. Runtime reads and writes after initialization
target the profile only; Vine never writes the distribution `BGI.gdb`.

## Path and Win32 support band

Path resolution accepts a bounded trailing-NUL `CreateFileA` path, resolves
relative input against the Rust-owned current directory, canonicalizes drive and
separators, processes `.` and `..` without mount escape, and folds ASCII case
without folding bytes inside valid Windows-932 pairs. Invalid encodings, DOS-invalid
characters, and trailing dot/space variants fail before mutation.

The versioned Kernel32 policy supports these bounded variants:

- immutable open: `GENERIC_READ`, share-read, `OPEN_EXISTING`;
- writable open: `GENERIC_WRITE`, share none, `CREATE_ALWAYS` or
  `OPEN_ALWAYS`;
- common open constraints: null security/template and the measured
  flags/attributes value;
- `GetFileSize`: low DWORD only;
- `ReadFile`: bounded nonzero synchronous read with an explicit output count;
- `WriteFile`: bounded nonzero synchronous write with an explicit output count;
- `SetFilePointer`: signed low-DWORD seek with
  `FILE_BEGIN`/`FILE_CURRENT`/`FILE_END`;
- `FlushFileBuffers` and `CloseHandle`: durable profile commit boundaries.

`CREATE_ALWAYS` stages truncation; `OPEN_ALWAYS` stages creation only when the
profile object is absent. Both return `ERROR_ALREADY_EXISTS` when appropriate.
Successful deferred `CloseHandle` preserves the owner thread's LastError exactly
like the clean immediate path; a failed commit returns its concrete Win32 error and
keeps both the handle and dirty staging available for retry.
Sparse writes zero-fill the gap. Immutable writes and source/profile shadowing are
denied before mutation. Directory enumeration, rename/delete, mapped files, locks,
general volume resolution, arbitrary access/share/disposition combinations, and
registry projection are not claimed.

## Read, write, and commit transactions

A clean file read is a scheduler/VFS/guest-memory transaction. Rust validates the
handle, cursor, request identity, generation, and guest outputs before suspending the
thread. The same-origin adapter returns bounded bytes identified by title, profile or
snapshot, object, offset, and generation. Rust validates the exact binary response
and preflights all guest writes before advancing the cursor. Dirty profile reads use
Rust-owned staged bytes directly and cannot observe stale SQLite content.

`WriteFile` first validates guest input and output spans, access, cursor, quota, and
the absence of a pending operation. `RuntimeFiles` builds a mutation-free plan for
the new staged bytes, length, hash, revision, and cursor. Guest byte-count output and
the owner plan commit together. Failure leaves bytes, cursor, revision, generation,
and guest output unchanged.

A dirty flush or close creates a versioned commit request containing object ID, base
generation, length, hash, and close intent. The payload remains Rust-owned. The
adapter adds a session/request idempotency key and sends it to the same-origin
profile-v2 endpoint. Session startup first acquires a SQLite-backed monotonic epoch
lease. Every snapshot, object read, and commit carries the opaque session key and
epoch. A commit revalidates that lease inside the same `BEGIN IMMEDIATE`
transaction that performs generation compare-and-swap, hash/length verification,
whole-profile quota accounting, and publication. `WAL` and `synchronous=FULL` are
enabled.

Rust advances committed generation, clears dirty state, and closes a requested
handle only after a valid successful response. A storage failure removes the pending
request but preserves the dirty handle and staged bytes for retry. A lost response is
retried with the same idempotency key; the SQLite commit ledger returns the original
generation without applying the bytes twice. Stale generations, duplicate keys with
different content, malformed responses, and quota failures do not partially update
either authority.

Quota exhaustion, generation conflict, a revoked lease, SQLite busy/locked, and
other ordinary SQLite write failures return a validated 32-byte commit failure
completion with a concrete Win32 error. They do not fault the Worker session.
Malformed controls, identity mismatches, and impossible protocol responses remain
session-fatal. A transport failure with no definitive response is also fatal because
the Worker cannot distinguish a request that never arrived from a committed
transaction whose response was lost. Cancellation, watchdog, fault, structured stop, and clean completion
all stop new work, drain every in-flight commit into Rust, revoke the lease, and only
then publish the terminal session report. A later session acquisition therefore
serializes after any old transaction and invalidates every old epoch before loading
its snapshot.

Process/thread cancellation retires matching scheduler and VFS requests before file
retirement. Process exit deliberately discards uncommitted staged bytes and leaves
the durable generation unchanged. It does not copy dirty process memory into browser
state during teardown.

## Verification and non-claims

Rust model and integration tests cover path separation, create/truncate/open
semantics, sharing, sparse writes, quota, guest-memory atomicity, successful durable
close, malformed completion, failed-close retry, stale/duplicate completion,
metadata-only atomic hydration, and process-exit discard. The SQLite tests cover
first creation, restart, idempotent duplicate, stale generation, identity/hash
rejection, transaction rollback, post-commit response loss, quota, a held SQLite
write lock, lease revoke and replacement, rejected late commits, a 4 MiB
high-expansion profile, and the same-origin HTTP-v2 range/commit protocol.
The public immutable fixture continues to cover open/size/read/seek/close and browser
read cancellation/watchdog cleanup.

The private Node and Chrome checks import `BGI.gdb` through the writable profile,
serve it from SQLite, and verify that the distribution file is unchanged. This
checkpoint establishes the storage authority and durable commit path. It does not
yet claim that a controlled guest fixture has performed a browser commit and loaded
it after a browser restart, nor that the primary title's full save/load UI,
multi-slot lifecycle, registry settings, migration UI, crash recovery matrix, or
long-run quota behavior has passed. Those remain scenario milestones under
[`ADR-006`](adr-006-vfs-local-server.md).
