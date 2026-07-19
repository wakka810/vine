# Runtime VFS and immutable source reads

## Status

- Status: immutable-source read vertical implemented and verified
- State owner: Rust `runtime/files.rs::RuntimeFiles`
- Aggregate coordinator: `runtime/kernel.rs::RuntimeKernel`
- Browser adapter: same-origin `storage.source-read.v1` byte service
- Writable overlay／save／registry persistence: not implemented by this slice

This document defines the stable contract established by the first VFS gateway.
Current call counts, report identities, and the exact next stop remain in
[`current-checkpoint.json`](current-checkpoint.json).

## Authority and identity

`RuntimeFiles` privately owns the registered source snapshot, normalized path index,
monotonic file IDs, open-file state, cursor, and pending-read records. A source file is
also represented by a typed `RuntimeObjectKind::File` object and a
`vine.runtime-source-file.v1` handle. The object table owns handle generation and
staleness; `RuntimeFiles` owns the file payload and lifetime. Neither the Worker nor
the HTTP service owns a Win32 handle, cursor, guest pointer, or LastError value.

One process-static `vine.source-snapshot-manifest` version 1 binds a title
fingerprint and snapshot ID to one source mount and bounded object descriptors. Each
descriptor contains a stable object ID, normalized relative Windows-932 path, byte
length, and SHA-256. The loopback source service registers explicit host files,
verifies the complete length and hash once, and serves an immutable in-memory copy;
it never accepts a host path from the guest or Worker.
The Worker rejects configured source-manifest or source-byte URLs whose origin differs
from the Runtime Worker origin before issuing a fetch.

Source executable／DLL／asset identity and a future writable overlay remain separate
namespaces. This slice cannot create, overwrite, truncate, delete, or shadow source
objects.

## Path and API support band

Path resolution accepts a bounded trailing-NUL `CreateFileA` path. It resolves
relative input against the Rust-owned process current directory, canonicalizes drive
letter and separators, removes `.`, processes `..` without allowing an escape, and
performs ASCII case folding without folding bytes inside valid Windows-932 pairs.
Invalid pairs, DOS-invalid characters, and trailing dot／space variants stop before
mutation. The normalized result must identify a manifest object below the registered
mount.

The versioned Kernel32 policy enables only the reached immutable-source band:

- `CreateFileA`: `GENERIC_READ`, share-read, `OPEN_EXISTING`, null security／template,
  and the measured flags／attributes value;
- `GetFileSize`: low DWORD only;
- `ReadFile`: bounded nonzero synchronous read with explicit output count;
- `SetFilePointer`: signed low-DWORD seek with `FILE_BEGIN`／`FILE_CURRENT`／`FILE_END`;
- `CloseHandle`: typed live file handle.

Missing files and expected Win32 failures return the bounded LastError result.
Unreached access, creation, overlapped, high-size, high-distance, path, and seek
variants are explicit unsupported or error outcomes before state mutation. Directory
enumeration, metadata, writes, mapped files, delete semantics, locks, general volume
resolution, and writable overlay precedence are not claimed.

## Async read transaction

A `ReadFile` request is a cross-subsystem transaction:

1. Rust validates the typed handle, policy, guest output ranges, cursor, scheduler
   lane, and absence of another pending read.
2. `RuntimeFiles` and the scheduler create matching request records. The binary
   request exposes only transport version, source／object IDs, offset, and bounded
   length.
3. The Worker binds those IDs to the already validated title／snapshot manifest and
   performs a same-origin range request. The URL carries title fingerprint,
   snapshot, source, and object identity; the response repeats them in headers.
4. Rust first decodes an exact 32-byte response control record. Before copying host
   bytes, the decoder rejects zero or over-1-MiB requests, success payloads whose
   length differs from the transferred count, transfers larger than the request,
   and error responses carrying bytes. It then validates request, source, object,
   offset, file lifetime, and cursor. The dispatcher preflights the byte-count and
   payload writes as one guest-memory transaction.
5. Only after guest writes succeed does Rust retire the pending request and advance
   the cursor. Wrong, stale, duplicate, oversized, or failed completions preserve the
   cursor, guest bytes, scheduler request, and allocators.

Thread／process cancellation removes the matching file and scheduler request before
file retirement. Normal `CloseHandle` rejects a file with a pending read; process
exit cancels the read, retires all file handles and payloads, and leaves no browser
fetch controller or Rust pending record.

## Verification and non-claims

Rust model tests cover mount bounding, Windows-932-aware case identity, stale
snapshot registration, share conflict, cursor plan／completion ordering, wrong and
duplicate completion, guest-memory atomicity, close, and process-exit cancellation.
The source-response decoder has valid success／error and malformed length／result
tests, plus a coverage-guided arbitrary control／payload fuzz target. The same fuzz
tier covers the CPU writeback binary decoder separately from the PE loader.
The public PE32 fixture verifies open／size／read／seek／read／close and validates the
returned bytes itself. Clean-profile Chrome covers success, pending cancellation,
watchdog cleanup, same-origin delivery, and cross-origin adapter rejection. The private target additionally proves
two complete reads of one fingerprinted asset and source hash invariance.

The service currently caches registered source bytes for deterministic evidence. It
does not establish large-corpus streaming memory bounds, source replacement recovery,
server restart, writable durability, save／load, SQLite transactions, or full VFS
coverage. Those remain the next storage milestones under
[ADR-006](adr-006-vfs-local-server.md).
