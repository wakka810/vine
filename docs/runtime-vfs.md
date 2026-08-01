# Runtime VFS, immutable sources, and writable profiles

## Status

- State owner: Rust `runtime/files.rs::RuntimeFiles`
- Aggregate coordinator: `runtime/kernel.rs::RuntimeKernel`
- Immutable adapter: same-origin `storage.source-read.v1`
- Writable adapters: same-origin `storage.profile-read.v1`,
  `storage.profile-commit.v1`, and `storage.profile-directory-create.v1`
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

`RuntimeFiles` privately owns normalized Windows-932 path identity, monotonic file,
directory, and search IDs, open-file access/share/cursor state, immutable search
snapshots and cursors, profile file／directory generations, committed and staged
bytes, dirty revisions, and pending read/commit/create records. `RuntimeObjectTable`
owns typed handle generation and staleness, including the distinct file-search
handle kind. The Worker and local server never own a Win32 handle, search cursor,
guest pointer, LastError, or dirty-file decision.

An immutable `vine.source-snapshot-manifest` version 1 binds a title and snapshot
to source/object IDs, paths, byte lengths, and SHA-256 values. The source server
validates and caches the configured host bytes and accepts no guest-supplied host
path. The primary title registers the versioned
`fixtures/source-snapshots/primary-game-archives-v2.json` snapshot — the three boot
archives `system.arc`, `sysprg.arc`, and `sysgrp.arc` plus the 45 reached
`data0????.arc` archives — under mount root `C:\Vine` beside the `BGI.gdb`
writable profile. The two mounts share one title fingerprint and root, and writable
paths may not shadow immutable objects. Registration streams and hashes each large
source object while checking stable host identity before and after the read; runtime
range service then addresses only verified source/object IDs. Unregistered paths,
including the engine's loose-file member probes, resolve to not-found exactly as on
the native disk.

A `vine.writable-profile-snapshot` version 3 binds a title fingerprint, profile ID,
root, quota, and disjoint file／directory allowlists to per-object existence and
generation, plus file length and hash. A directory allowlist entry grants only the
ability to create that logical directory; it does not imply existence. The snapshot
is metadata-only: committed bytes are never expanded into JSON or
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

- `GetFileAttributesA`: exact registered existing source／writable files project
  the policy-owned regular-file attributes, while the mount root and ancestors of
  registered paths and committed writable-profile directories project directory
  attributes;
- `CreateDirectoryA`: one allowlisted absent writable-profile directory with NULL
  security attributes, durable SQLite publication, and exact duplicate／missing
  parent／file-conflict errors;
- `FindFirstFileA`／`FindNextFileA`: a bounded `*`／`?` wildcard over one resolved
  directory, ASCII-case-insensitive matching without splitting Windows-932 pairs,
  deterministic lexical order, and exact 320-byte PE32 `WIN32_FIND_DATAA` output;
- `FindClose`: retire only a live file-search handle; ordinary `CloseHandle` rejects
  that handle kind;
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
`GetFileAttributesA` is a mutation-free query over this same path authority. A
missing final component returns `ERROR_FILE_NOT_FOUND`; a missing parent or mount
escape returns `ERROR_PATH_NOT_FOUND`. Success preserves LastError. It never probes
the Node／browser host filesystem, and an absent writable object does not become an
existing file or directory merely because its path is allowlisted.
Sparse writes zero-fill the gap. Immutable writes and source/profile shadowing are
denied before mutation. Recursive enumeration, short-name generation, broad DOS
wildcards and attributes, rename/delete, mapped files, locks, directory mutation
outside the explicit create allowlist, volume resolution, access/share/disposition
combinations, and registry projection are not claimed.

An enumeration captures matching registered source and existing writable entries at
successful `FindFirstFileA` commit. Later catalog changes cannot alter that open
search. The owner preflights the complete first result, search capacity, typed object
capacity, and guest output before publishing either search or handle; failures do not
consume either allocator. `FindNextFileA` commits the cursor only with its complete
guest write. Exhaustion returns FALSE／`ERROR_NO_MORE_FILES` without touching the
output, and successful `FindClose` preserves LastError. No enumeration asks Node,
Chrome, or the source server to list a host directory.

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
profile-v3 endpoint. Session startup first acquires a SQLite-backed monotonic epoch
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

Directory creation follows the same host-first publication rule without borrowing
file-object state. Rust validates path classification, parent existence, file
conflict, directory generation, request identity, and lifecycle, then suspends the
owner thread with an opaque create plan. SQLite revalidates the epoch lease and base
generation inside one immediate transaction, records the idempotency result, and
publishes the directory generation. Only a matching durable success completion lets
`RuntimeFiles` commit existence and generation. Duplicate, stale, malformed,
cancelled, or failed requests leave both authorities unchanged. Successful
`CreateDirectoryA` preserves the thread's incoming LastError and writes no guest
bytes.

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
metadata-only atomic hydration, process-exit discard, and read-only file／derived
directory attribute classification without host filesystem import. They also cover
case-insensitive wildcard enumeration, lexical ordering, exact find-data layout,
immutable open-search snapshots, terminal error/output preservation, distinct
`FindClose` lifetime, stale/wrong-kind handles, capacity, and failure atomicity. They also cover
allowlisted directory identity, host-first durable creation, duplicate／missing-parent／
file-conflict classification, stale plans, malformed completion, failure retry, and
cancellation. The SQLite tests cover
first creation, restart, idempotent duplicate, stale generation, identity/hash
rejection, transaction rollback, post-commit response loss, quota, a held SQLite
write lock, lease revoke and replacement, rejected late commits, a 4 MiB
high-expansion profile, and the same-origin HTTP-v3 range/commit/directory protocol.
The public immutable fixture continues to cover open/size/read/seek/close and browser
read cancellation/watchdog cleanup.

The private Node and Chrome checks import `BGI.gdb` through the writable profile,
serve it from SQLite, create the run-local allowlisted `UserData` directory, enumerate
the same 45 immutable `data0????.arc` entries as the native oracle, and verify that
the distribution files and shared profile are unchanged. This checkpoint establishes
the storage authority, durable file／directory commit paths, and host-independent
primary archive enumeration. It does not
yet claim that a controlled guest fixture has performed a browser commit and loaded
it after a browser restart, nor that the primary title's full save/load UI,
multi-slot lifecycle, registry settings, migration UI, crash recovery matrix, or
long-run quota behavior has passed. Those remain scenario milestones under
[`ADR-006`](adr-006-vfs-local-server.md).
