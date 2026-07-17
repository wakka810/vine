use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::memory::GuestAddress;

use super::{RuntimeHandle, RuntimeStateError};

pub const RUNTIME_SOURCE_SNAPSHOT_SCHEMA: &str = "vine.source-snapshot-manifest";
pub const RUNTIME_SOURCE_SNAPSHOT_VERSION: u32 = 1;
pub const RUNTIME_SOURCE_FILE_NAMESPACE: &str = "vine.runtime-source-file.v1";
pub const RUNTIME_SOURCE_READ_SERVICE: &str = "storage.source-read.v1";
pub const RUNTIME_SOURCE_READ_TRANSPORT_VERSION: u32 = 1;
pub const RUNTIME_SOURCE_READ_REQUEST_BYTES: usize = 24;
pub const RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES: usize = 32;
pub const RUNTIME_SOURCE_FILE_LIMIT: usize = 4096;
pub const RUNTIME_SOURCE_READ_MAX_BYTES: u32 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSourceSnapshotManifest {
    pub schema: String,
    pub schema_version: u32,
    pub title_fingerprint: String,
    pub snapshot_id: String,
    pub mount: RuntimeSourceMountManifest,
    pub files: Vec<RuntimeSourceFileManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSourceMountManifest {
    pub source_id: u32,
    pub root_path_a: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSourceFileManifest {
    pub object_id: u32,
    pub relative_path_a: Vec<u8>,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSourceSnapshot {
    pub title_fingerprint: String,
    pub snapshot_id: String,
    pub source_id: u32,
    pub root_path_a: Vec<u8>,
    pub files: BTreeMap<u32, RuntimeSourceFileDescriptor>,
    #[serde(skip)]
    path_index: BTreeMap<Vec<u8>, u32>,
    #[serde(skip)]
    root_identity: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSourceFileDescriptor {
    pub object_id: u32,
    pub relative_path_a: Vec<u8>,
    pub byte_length: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeSourceFileId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSourceFile {
    pub id: RuntimeSourceFileId,
    pub source_id: u32,
    pub object_id: u32,
    pub normalized_path_a: Vec<u8>,
    pub byte_length: u64,
    pub cursor: u64,
    pub desired_access: u32,
    pub share_mode: u32,
    pub flags_and_attributes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOpenSourceFileResult {
    pub file: RuntimeSourceFile,
    pub handle: RuntimeHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSourceReadRequest {
    pub request_id: u64,
    pub file_id: RuntimeSourceFileId,
    pub source_id: u32,
    pub object_id: u32,
    pub offset: u64,
    pub requested_bytes: u32,
}

impl RuntimeSourceReadRequest {
    pub fn encode_host_payload(self) -> [u8; RUNTIME_SOURCE_READ_REQUEST_BYTES] {
        let words = [
            RUNTIME_SOURCE_READ_TRANSPORT_VERSION,
            self.source_id,
            self.object_id,
            self.offset as u32,
            (self.offset >> 32) as u32,
            self.requested_bytes,
        ];
        let mut payload = [0; RUNTIME_SOURCE_READ_REQUEST_BYTES];
        for (word, destination) in words.into_iter().zip(payload.chunks_exact_mut(4)) {
            destination.copy_from_slice(&word.to_le_bytes());
        }
        payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSourceReadHostResponse {
    pub request_id: u64,
    pub source_id: u32,
    pub object_id: u32,
    pub offset: u64,
    pub requested_bytes: u32,
    pub transferred_bytes: u32,
    pub win32_error: u32,
    pub bytes: Vec<u8>,
}

impl RuntimeSourceReadHostResponse {
    pub fn decode_binary(
        request_id: u64,
        control: &[u8],
        payload: &[u8],
    ) -> Result<Self, RuntimeStateError> {
        if control.len() != RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES {
            return Err(vfs_invalid(
                "source-read-response-control-size-invalid",
                "source read response control must contain eight little-endian words",
            ));
        }
        let words = control
            .chunks_exact(4)
            .map(|word| {
                u32::from_le_bytes(
                    word.try_into()
                        .expect("four-byte source read response control chunk"),
                )
            })
            .collect::<Vec<_>>();
        if words[0] != RUNTIME_SOURCE_READ_TRANSPORT_VERSION {
            return Err(vfs_invalid(
                "source-read-response-version-unsupported",
                "source read response uses an unsupported transport version",
            ));
        }
        Ok(Self {
            request_id,
            source_id: words[1],
            object_id: words[2],
            offset: u64::from(words[3]) | (u64::from(words[4]) << 32),
            requested_bytes: words[5],
            transferred_bytes: words[6],
            win32_error: words[7],
            bytes: payload.to_vec(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSourceReadCompletion {
    pub request: RuntimeSourceReadRequest,
    pub succeeded: bool,
    pub transferred_bytes: u32,
    pub win32_error: u32,
    pub cursor_before: u64,
    pub cursor_after: u64,
    pub content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeFiles {
    snapshot: Option<RuntimeSourceSnapshot>,
    open_files: BTreeMap<RuntimeSourceFileId, RuntimeSourceFile>,
    pending_reads: BTreeMap<u64, RuntimePendingSourceRead>,
    next_file_id: u64,
}

impl Default for RuntimeFiles {
    fn default() -> Self {
        Self {
            snapshot: None,
            open_files: BTreeMap::new(),
            pending_reads: BTreeMap::new(),
            next_file_id: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceSnapshotRegistrationPlan {
    next: RuntimeSourceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceFileOpenPlan {
    expected_next_file_id: u64,
    next_file_id: u64,
    file: RuntimeSourceFile,
}

impl RuntimeSourceFileOpenPlan {
    pub(super) const fn file_id(&self) -> RuntimeSourceFileId {
        self.file.id
    }

    #[cfg(test)]
    pub(super) fn file(&self) -> &RuntimeSourceFile {
        &self.file
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceFileRetirementPlan {
    previous: RuntimeSourceFile,
    pending_reads: Vec<RuntimePendingSourceRead>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceFileSeekPlan {
    previous: RuntimeSourceFile,
    next: RuntimeSourceFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimePendingSourceRead {
    request: RuntimeSourceReadRequest,
    buffer: GuestAddress,
    bytes_read: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceReadBeginPlan {
    pending: RuntimePendingSourceRead,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceReadCompletionPlan {
    pending: RuntimePendingSourceRead,
    previous_file: RuntimeSourceFile,
    next_file: RuntimeSourceFile,
    response: RuntimeSourceReadHostResponse,
    completion: RuntimeSourceReadCompletion,
}

impl RuntimeSourceReadCompletionPlan {
    pub(super) fn response(&self) -> &RuntimeSourceReadHostResponse {
        &self.response
    }

    pub(super) const fn buffer(&self) -> GuestAddress {
        self.pending.buffer
    }

    pub(super) const fn bytes_read(&self) -> GuestAddress {
        self.pending.bytes_read
    }

    pub(super) fn completion(&self) -> &RuntimeSourceReadCompletion {
        &self.completion
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceReadCancellationPlan {
    requests: Vec<RuntimePendingSourceRead>,
}

impl RuntimeSourceReadCancellationPlan {
    pub(super) fn request_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.requests
            .iter()
            .map(|pending| pending.request.request_id)
    }
}

impl RuntimeFiles {
    pub(super) fn snapshot(&self) -> Option<&RuntimeSourceSnapshot> {
        self.snapshot.as_ref()
    }

    pub(super) fn open_file(
        &self,
        id: RuntimeSourceFileId,
    ) -> Result<&RuntimeSourceFile, RuntimeStateError> {
        self.open_files.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-vfs",
                "source-file-id-unknown",
                "logical source file identity is not open",
            )
        })
    }

    pub(super) fn open_file_ids(&self) -> impl Iterator<Item = RuntimeSourceFileId> + '_ {
        self.open_files.keys().copied()
    }

    pub(super) fn plan_snapshot_registration(
        &self,
        manifest: RuntimeSourceSnapshotManifest,
    ) -> Result<RuntimeSourceSnapshotRegistrationPlan, RuntimeStateError> {
        if self.snapshot.is_some() || !self.open_files.is_empty() || !self.pending_reads.is_empty()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-snapshot-already-registered",
                "runtime source snapshot registration is process-static",
            ));
        }
        if manifest.schema != RUNTIME_SOURCE_SNAPSHOT_SCHEMA
            || manifest.schema_version != RUNTIME_SOURCE_SNAPSHOT_VERSION
            || manifest.title_fingerprint.is_empty()
            || manifest.title_fingerprint.len() > 128
            || manifest.snapshot_id.is_empty()
            || manifest.snapshot_id.len() > 128
            || manifest.mount.source_id == 0
            || manifest.files.is_empty()
            || manifest.files.len() > RUNTIME_SOURCE_FILE_LIMIT
        {
            return Err(vfs_invalid(
                "source-snapshot-manifest-invalid",
                "source snapshot manifest identity or bounds are invalid",
            ));
        }
        let root = normalize_absolute_path(&manifest.mount.root_path_a)?;
        let mut files = BTreeMap::new();
        let mut path_index = BTreeMap::new();
        for file in manifest.files {
            if file.object_id == 0
                || file.byte_length > u64::from(u32::MAX)
                || !is_sha256(&file.sha256)
            {
                return Err(vfs_invalid(
                    "source-file-manifest-invalid",
                    "source file manifest identity, size, or hash is invalid",
                ));
            }
            let relative = normalize_relative_path(&file.relative_path_a)?;
            let identity = windows_932_ascii_fold(&relative);
            if files.contains_key(&file.object_id) || path_index.contains_key(&identity) {
                return Err(vfs_invalid(
                    "source-file-manifest-duplicate",
                    "source file object or case-insensitive path identity is duplicated",
                ));
            }
            path_index.insert(identity, file.object_id);
            files.insert(
                file.object_id,
                RuntimeSourceFileDescriptor {
                    object_id: file.object_id,
                    relative_path_a: relative,
                    byte_length: file.byte_length,
                    sha256: file.sha256.to_ascii_lowercase(),
                },
            );
        }
        Ok(RuntimeSourceSnapshotRegistrationPlan {
            next: RuntimeSourceSnapshot {
                title_fingerprint: manifest.title_fingerprint,
                snapshot_id: manifest.snapshot_id,
                source_id: manifest.mount.source_id,
                root_path_a: root.bytes_with_nul,
                files,
                path_index,
                root_identity: root.identity,
            },
        })
    }

    pub(super) fn commit_snapshot_registration(
        &mut self,
        plan: RuntimeSourceSnapshotRegistrationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.snapshot.is_some() || !self.open_files.is_empty() || !self.pending_reads.is_empty()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-snapshot-registration-plan-stale",
                "source file state changed after snapshot registration planning",
            ));
        }
        self.snapshot = Some(plan.next);
        Ok(())
    }

    pub(super) fn plan_open(
        &self,
        requested_path: &[u8],
        current_directory: &[u8],
        desired_access: u32,
        share_mode: u32,
        flags_and_attributes: u32,
    ) -> Result<Option<RuntimeSourceFileOpenPlan>, RuntimeStateError> {
        let snapshot = self.snapshot.as_ref().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-vfs",
                "source-snapshot-unregistered",
                "runtime process has no immutable source snapshot",
            )
        })?;
        let normalized = resolve_path(requested_path, current_directory)?;
        let relative = relative_to_mount(&normalized.identity, &snapshot.root_identity)?;
        let Some(&object_id) = snapshot.path_index.get(relative) else {
            return Ok(None);
        };
        let descriptor = snapshot.files.get(&object_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "source-path-index-corrupt",
                "source path index points to a missing object descriptor",
            )
        })?;
        if self.open_files.len() >= RUNTIME_SOURCE_FILE_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-vfs",
                "source-open-file-limit-exceeded",
                "logical source file count exceeds the runtime bound",
            ));
        }
        for open in self
            .open_files
            .values()
            .filter(|open| open.object_id == object_id)
        {
            if (desired_access & 0x8000_0000 != 0 && open.share_mode & 1 == 0)
                || (open.desired_access & 0x8000_0000 != 0 && share_mode & 1 == 0)
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-vfs",
                    "source-file-sharing-violation",
                    "logical source file read access conflicts with an existing share mode",
                ));
            }
        }
        let id = RuntimeSourceFileId(self.next_file_id);
        let next_file_id = self.next_file_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-vfs",
                "source-file-id-space-exhausted",
                "logical source file identifier space is exhausted",
            )
        })?;
        Ok(Some(RuntimeSourceFileOpenPlan {
            expected_next_file_id: self.next_file_id,
            next_file_id,
            file: RuntimeSourceFile {
                id,
                source_id: snapshot.source_id,
                object_id,
                normalized_path_a: normalized.bytes_with_nul,
                byte_length: descriptor.byte_length,
                cursor: 0,
                desired_access,
                share_mode,
                flags_and_attributes,
            },
        }))
    }

    pub(super) fn preflight_open(
        &self,
        plan: &RuntimeSourceFileOpenPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.next_file_id != plan.expected_next_file_id
            || self.open_files.contains_key(&plan.file.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-file-open-plan-stale",
                "source file state changed after open planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_open(&mut self, plan: RuntimeSourceFileOpenPlan) -> RuntimeSourceFile {
        debug_assert!(self.preflight_open(&plan).is_ok());
        self.next_file_id = plan.next_file_id;
        let file = plan.file;
        let previous = self.open_files.insert(file.id, file.clone());
        debug_assert!(previous.is_none());
        file
    }

    pub(super) fn plan_retirement(
        &self,
        id: RuntimeSourceFileId,
    ) -> Result<RuntimeSourceFileRetirementPlan, RuntimeStateError> {
        let previous = self.open_file(id)?.clone();
        if self
            .pending_reads
            .values()
            .any(|pending| pending.request.file_id == id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-file-read-pending",
                "logical source file cannot close while a read is pending",
            ));
        }
        Ok(RuntimeSourceFileRetirementPlan {
            previous,
            pending_reads: Vec::new(),
        })
    }

    pub(super) fn plan_process_retirement(
        &self,
        id: RuntimeSourceFileId,
        cancelling_request_ids: &BTreeSet<u64>,
    ) -> Result<RuntimeSourceFileRetirementPlan, RuntimeStateError> {
        let previous = self.open_file(id)?.clone();
        let pending_reads = self
            .pending_reads
            .values()
            .filter(|pending| pending.request.file_id == id)
            .cloned()
            .collect::<Vec<_>>();
        if pending_reads
            .iter()
            .any(|pending| !cancelling_request_ids.contains(&pending.request.request_id))
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-file-read-survives-process-teardown",
                "process teardown cannot retire a source file with an uncancelled read",
            ));
        }
        Ok(RuntimeSourceFileRetirementPlan {
            previous,
            pending_reads,
        })
    }

    pub(super) fn commit_retirement(&mut self, plan: RuntimeSourceFileRetirementPlan) {
        debug_assert_eq!(self.open_files.get(&plan.previous.id), Some(&plan.previous));
        debug_assert!(
            !self
                .pending_reads
                .values()
                .any(|pending| pending.request.file_id == plan.previous.id)
        );
        self.open_files.remove(&plan.previous.id);
    }

    pub(super) fn preflight_retirement(
        &self,
        plan: &RuntimeSourceFileRetirementPlan,
    ) -> Result<(), RuntimeStateError> {
        let pending_reads = self
            .pending_reads
            .values()
            .filter(|pending| pending.request.file_id == plan.previous.id)
            .collect::<Vec<_>>();
        if self.open_files.get(&plan.previous.id) != Some(&plan.previous)
            || pending_reads.len() != plan.pending_reads.len()
            || pending_reads
                .iter()
                .zip(&plan.pending_reads)
                .any(|(actual, expected)| *actual != expected)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-file-retirement-plan-stale",
                "source file changed after close planning",
            ));
        }
        Ok(())
    }

    pub(super) fn plan_read_begin(
        &self,
        id: RuntimeSourceFileId,
        request_id: u64,
        buffer: GuestAddress,
        bytes_read: GuestAddress,
        requested_bytes: u32,
    ) -> Result<RuntimeSourceReadBeginPlan, RuntimeStateError> {
        let file = self.open_file(id)?;
        if requested_bytes == 0 || requested_bytes > RUNTIME_SOURCE_READ_MAX_BYTES {
            return Err(vfs_invalid(
                "source-read-length-unsupported",
                "source reads require a nonzero bounded byte count",
            ));
        }
        if self.pending_reads.contains_key(&request_id)
            || self
                .pending_reads
                .values()
                .any(|pending| pending.request.file_id == id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-read-already-pending",
                "logical source file already owns a pending read",
            ));
        }
        Ok(RuntimeSourceReadBeginPlan {
            pending: RuntimePendingSourceRead {
                request: RuntimeSourceReadRequest {
                    request_id,
                    file_id: id,
                    source_id: file.source_id,
                    object_id: file.object_id,
                    offset: file.cursor,
                    requested_bytes,
                },
                buffer,
                bytes_read,
            },
        })
    }

    pub(super) fn preflight_read_begin(
        &self,
        plan: &RuntimeSourceReadBeginPlan,
    ) -> Result<(), RuntimeStateError> {
        let file = self.open_file(plan.pending.request.file_id)?;
        if file.cursor != plan.pending.request.offset
            || self
                .pending_reads
                .contains_key(&plan.pending.request.request_id)
            || self
                .pending_reads
                .values()
                .any(|pending| pending.request.file_id == file.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-read-plan-stale",
                "source file cursor or pending request state changed after planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_read_begin(
        &mut self,
        plan: RuntimeSourceReadBeginPlan,
    ) -> RuntimeSourceReadRequest {
        debug_assert!(self.preflight_read_begin(&plan).is_ok());
        let request = plan.pending.request;
        let previous = self.pending_reads.insert(request.request_id, plan.pending);
        debug_assert!(previous.is_none());
        request
    }

    pub(super) fn plan_read_completion(
        &self,
        response: RuntimeSourceReadHostResponse,
    ) -> Result<RuntimeSourceReadCompletionPlan, RuntimeStateError> {
        let pending = self
            .pending_reads
            .get(&response.request_id)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-vfs",
                    "source-read-request-unknown",
                    "source read completion does not identify a pending request",
                )
            })?;
        let request = pending.request;
        let previous_file = self.open_file(request.file_id)?.clone();
        if response.source_id != request.source_id
            || response.object_id != request.object_id
            || response.offset != request.offset
            || response.requested_bytes != request.requested_bytes
            || previous_file.cursor != request.offset
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-read-completion-identity-mismatch",
                "source read completion identity or cursor does not match the pending request",
            ));
        }
        let succeeded = response.win32_error == 0;
        if succeeded {
            if response.transferred_bytes as usize != response.bytes.len()
                || response.transferred_bytes > response.requested_bytes
                || response
                    .offset
                    .checked_add(u64::from(response.transferred_bytes))
                    .is_none_or(|end| end > previous_file.byte_length)
            {
                return Err(vfs_invalid(
                    "source-read-completion-length-invalid",
                    "successful source read completion has invalid byte bounds",
                ));
            }
        } else if response.transferred_bytes != 0 || !response.bytes.is_empty() {
            return Err(vfs_invalid(
                "source-read-failure-payload-invalid",
                "failed source read completion may not carry bytes",
            ));
        }
        let mut next_file = previous_file.clone();
        if succeeded {
            next_file.cursor += u64::from(response.transferred_bytes);
        }
        let completion = RuntimeSourceReadCompletion {
            request,
            succeeded,
            transferred_bytes: response.transferred_bytes,
            win32_error: response.win32_error,
            cursor_before: previous_file.cursor,
            cursor_after: next_file.cursor,
            content_sha256: hex_sha256(&response.bytes),
        };
        Ok(RuntimeSourceReadCompletionPlan {
            pending: pending.clone(),
            previous_file,
            next_file,
            response,
            completion,
        })
    }

    pub(super) fn preflight_read_completion(
        &self,
        plan: &RuntimeSourceReadCompletionPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.pending_reads.get(&plan.pending.request.request_id) != Some(&plan.pending)
            || self.open_files.get(&plan.previous_file.id) != Some(&plan.previous_file)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-read-completion-plan-stale",
                "source read state changed after completion planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_read_completion(
        &mut self,
        plan: RuntimeSourceReadCompletionPlan,
    ) -> RuntimeSourceReadCompletion {
        debug_assert!(self.preflight_read_completion(&plan).is_ok());
        self.pending_reads.remove(&plan.pending.request.request_id);
        self.open_files.insert(plan.next_file.id, plan.next_file);
        plan.completion
    }

    pub(super) fn plan_read_cancellation(
        &self,
        request_ids: &[u64],
    ) -> RuntimeSourceReadCancellationPlan {
        let selected = request_ids.iter().copied().collect::<BTreeSet<_>>();
        RuntimeSourceReadCancellationPlan {
            requests: self
                .pending_reads
                .iter()
                .filter_map(|(request_id, pending)| {
                    selected.contains(request_id).then_some(pending.clone())
                })
                .collect(),
        }
    }

    pub(super) fn commit_read_cancellation(&mut self, plan: RuntimeSourceReadCancellationPlan) {
        for pending in plan.requests {
            let removed = self.pending_reads.remove(&pending.request.request_id);
            debug_assert_eq!(removed, Some(pending));
        }
    }

    pub(super) fn plan_seek(
        &self,
        id: RuntimeSourceFileId,
        distance: i64,
        origin: u32,
    ) -> Result<RuntimeSourceFileSeekPlan, RuntimeStateError> {
        let file = self.open_file(id)?;
        if self
            .pending_reads
            .values()
            .any(|pending| pending.request.file_id == id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-file-read-pending",
                "logical source file cannot seek while a read is pending",
            ));
        }
        let base = match origin {
            0 => 0i128,
            1 => i128::from(file.cursor),
            2 => i128::from(file.byte_length),
            _ => {
                return Err(vfs_invalid(
                    "source-seek-origin-invalid",
                    "source file seek origin is outside FILE_BEGIN/CURRENT/END",
                ));
            }
        };
        let cursor = base + i128::from(distance);
        if cursor < 0 || cursor > i128::from(file.byte_length) {
            return Err(vfs_invalid(
                "source-seek-range-invalid",
                "source file seek result is outside the immutable file",
            ));
        }
        let cursor = cursor as u64;
        let previous = file.clone();
        let mut next = previous.clone();
        next.cursor = cursor;
        Ok(RuntimeSourceFileSeekPlan { previous, next })
    }

    pub(super) fn commit_seek(
        &mut self,
        plan: RuntimeSourceFileSeekPlan,
    ) -> Result<u64, RuntimeStateError> {
        if self.open_files.get(&plan.previous.id) != Some(&plan.previous) {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-file-seek-plan-stale",
                "source file changed after seek planning",
            ));
        }
        let cursor = plan.next.cursor;
        self.open_files.insert(plan.next.id, plan.next);
        Ok(cursor)
    }
}

#[derive(Debug)]
struct NormalizedPath {
    bytes_with_nul: Vec<u8>,
    identity: Vec<u8>,
}

fn resolve_path(
    requested: &[u8],
    current_directory: &[u8],
) -> Result<NormalizedPath, RuntimeStateError> {
    let body = nul_terminated_body(requested, "source-file-path-shape-invalid")?;
    if is_absolute(body) {
        return normalize_absolute_path(requested);
    }
    let current = normalize_absolute_path(current_directory)?;
    let mut joined = current.bytes_with_nul[..current.bytes_with_nul.len() - 1].to_vec();
    if joined.last() != Some(&b'\\') {
        joined.push(b'\\');
    }
    joined.extend_from_slice(body);
    joined.push(0);
    normalize_absolute_path(&joined)
}

fn normalize_absolute_path(bytes: &[u8]) -> Result<NormalizedPath, RuntimeStateError> {
    let body = nul_terminated_body(bytes, "source-file-path-shape-invalid")?;
    if !is_absolute(body) {
        return Err(vfs_invalid(
            "source-file-path-not-absolute",
            "source file path requires an absolute DOS drive after resolution",
        ));
    }
    let components = normalize_components(&body[3..], true)?;
    let mut normalized = vec![body[0].to_ascii_uppercase(), b':', b'\\'];
    for (index, component) in components.iter().enumerate() {
        if index != 0 {
            normalized.push(b'\\');
        }
        normalized.extend_from_slice(component);
    }
    normalized.push(0);
    let identity = windows_932_ascii_fold(&normalized[..normalized.len() - 1]);
    Ok(NormalizedPath {
        bytes_with_nul: normalized,
        identity,
    })
}

fn normalize_relative_path(bytes: &[u8]) -> Result<Vec<u8>, RuntimeStateError> {
    if bytes.is_empty() || bytes.len() > 32 * 1024 || bytes.contains(&0) || is_absolute(bytes) {
        return Err(vfs_invalid(
            "source-relative-path-invalid",
            "source manifest relative path is empty, absolute, NUL-containing, or too long",
        ));
    }
    let components = normalize_components(bytes, false)?;
    if components.is_empty() {
        return Err(vfs_invalid(
            "source-relative-path-invalid",
            "source manifest relative path resolves to the mount root",
        ));
    }
    let mut normalized = Vec::new();
    for (index, component) in components.iter().enumerate() {
        if index != 0 {
            normalized.push(b'\\');
        }
        normalized.extend_from_slice(component);
    }
    Ok(normalized)
}

fn normalize_components(
    bytes: &[u8],
    allow_parent: bool,
) -> Result<Vec<Vec<u8>>, RuntimeStateError> {
    let mut components = Vec::<Vec<u8>>::new();
    let mut component = Vec::new();
    let mut index = 0;
    while index <= bytes.len() {
        if index == bytes.len() || is_separator(bytes[index]) {
            if !component.is_empty() {
                if component == b"." {
                    component.clear();
                } else if component == b".." {
                    if !allow_parent || components.pop().is_none() {
                        return Err(vfs_invalid(
                            "source-path-parent-escape",
                            "source path parent component escapes its normalized root",
                        ));
                    }
                    component.clear();
                } else {
                    validate_component(&component)?;
                    components.push(std::mem::take(&mut component));
                }
            }
            index += 1;
            continue;
        }
        let byte = bytes[index];
        if is_windows_932_lead_byte(byte) {
            let Some(&trail) = bytes.get(index + 1) else {
                return Err(vfs_invalid(
                    "source-path-windows-932-invalid",
                    "source path ends in a Windows-932 lead byte",
                ));
            };
            if !is_windows_932_trail_byte(trail) {
                return Err(vfs_invalid(
                    "source-path-windows-932-invalid",
                    "source path contains an invalid Windows-932 trail byte",
                ));
            }
            component.extend_from_slice(&[byte, trail]);
            index += 2;
        } else {
            if byte < 0x20
                || byte == 0x7f
                || matches!(byte, b'"' | b'<' | b'>' | b'|' | b'?' | b'*' | b':')
            {
                return Err(vfs_invalid(
                    "source-path-character-invalid",
                    "source path contains a disallowed DOS path character",
                ));
            }
            component.push(byte);
            index += 1;
        }
    }
    Ok(components)
}

fn validate_component(component: &[u8]) -> Result<(), RuntimeStateError> {
    if component
        .last()
        .is_some_and(|byte| matches!(byte, b'.' | b' '))
    {
        return Err(vfs_invalid(
            "source-path-trailing-dot-space-unsupported",
            "source path trailing dot/space normalization is outside the first VFS slice",
        ));
    }
    Ok(())
}

fn relative_to_mount<'a>(path: &'a [u8], root: &[u8]) -> Result<&'a [u8], RuntimeStateError> {
    if path == root {
        return Err(vfs_invalid(
            "source-path-identifies-directory",
            "source file open path identifies the mount root directory",
        ));
    }
    if path.len() <= root.len() || !path.starts_with(root) || path.get(root.len()) != Some(&b'\\') {
        return Err(RuntimeStateError::not_found(
            "runtime-vfs",
            "source-path-outside-mount",
            "source file path is outside the registered immutable mount",
        ));
    }
    Ok(&path[root.len() + 1..])
}

fn nul_terminated_body<'a>(
    bytes: &'a [u8],
    code: &'static str,
) -> Result<&'a [u8], RuntimeStateError> {
    if bytes.len() < 2
        || bytes.len() > 32 * 1024
        || bytes.last() != Some(&0)
        || bytes[..bytes.len() - 1].contains(&0)
    {
        return Err(vfs_invalid(
            code,
            "source path requires one bounded trailing-NUL ANSI string",
        ));
    }
    Ok(&bytes[..bytes.len() - 1])
}

fn windows_932_ascii_fold(bytes: &[u8]) -> Vec<u8> {
    let mut folded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if is_windows_932_lead_byte(byte)
            && bytes
                .get(index + 1)
                .is_some_and(|trail| is_windows_932_trail_byte(*trail))
        {
            folded.extend_from_slice(&bytes[index..index + 2]);
            index += 2;
        } else {
            folded.push(byte.to_ascii_lowercase());
            index += 1;
        }
    }
    folded
}

fn is_absolute(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && is_separator(bytes[2])
}

const fn is_separator(byte: u8) -> bool {
    matches!(byte, b'\\' | b'/')
}

const fn is_windows_932_lead_byte(byte: u8) -> bool {
    matches!(byte, 0x81..=0x9f | 0xe0..=0xfc)
}

const fn is_windows_932_trail_byte(byte: u8) -> bool {
    matches!(byte, 0x40..=0x7e | 0x80..=0xfc)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut result, "{byte:02x}").expect("writing a digest to String cannot fail");
    }
    result
}

fn vfs_invalid(code: &'static str, message: &'static str) -> RuntimeStateError {
    RuntimeStateError::invalid_input("runtime-vfs", code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> RuntimeSourceSnapshotManifest {
        RuntimeSourceSnapshotManifest {
            schema: RUNTIME_SOURCE_SNAPSHOT_SCHEMA.to_owned(),
            schema_version: RUNTIME_SOURCE_SNAPSHOT_VERSION,
            title_fingerprint: "sha256:test-title".to_owned(),
            snapshot_id: "source-fixture-v1".to_owned(),
            mount: RuntimeSourceMountManifest {
                source_id: 7,
                root_path_a: b"C:\\Vine\0".to_vec(),
            },
            files: vec![RuntimeSourceFileManifest {
                object_id: 11,
                relative_path_a: b"Data\\BGI.gdb".to_vec(),
                byte_length: 9,
                sha256: "0".repeat(64),
            }],
        }
    }

    #[test]
    fn source_snapshot_paths_are_case_insensitive_and_mount_bounded() {
        let mut files = RuntimeFiles::default();
        let plan = files.plan_snapshot_registration(manifest()).unwrap();
        let stale_plan = files.plan_snapshot_registration(manifest()).unwrap();
        files.commit_snapshot_registration(plan).unwrap();
        let after_registration = files.clone();
        let stale = files.commit_snapshot_registration(stale_plan).unwrap_err();
        assert_eq!(stale.code, "source-snapshot-registration-plan-stale");
        assert_eq!(files, after_registration);
        let opened = files
            .plan_open(
                b"c:/vine/data/../DATA/bgi.GDB\0",
                b"C:\\Vine\0",
                0x8000_0000,
                1,
                0x0800_0020,
            )
            .unwrap()
            .unwrap();
        assert_eq!(opened.file().object_id, 11);
        let before = files.clone();
        assert_eq!(
            files
                .plan_open(
                    b"D:\\Vine\\Data\\BGI.gdb\0",
                    b"C:\\Vine\0",
                    0x8000_0000,
                    1,
                    0x0800_0020,
                )
                .unwrap_err()
                .code,
            "source-path-outside-mount"
        );
        assert_eq!(files, before);
    }

    #[test]
    fn read_plans_advance_cursor_only_after_matching_completion() {
        let mut files = RuntimeFiles::default();
        let registration = files.plan_snapshot_registration(manifest()).unwrap();
        files.commit_snapshot_registration(registration).unwrap();
        let open = files
            .plan_open(
                b"C:\\Vine\\Data\\BGI.gdb\0",
                b"C:\\Vine\0",
                0x8000_0000,
                1,
                0x0800_0020,
            )
            .unwrap()
            .unwrap();
        let id = open.file_id();
        files.commit_open(open);
        let begin = files
            .plan_read_begin(id, 3, GuestAddress(0x1000), GuestAddress(0x2000), 4)
            .unwrap();
        assert_eq!(files.open_file(id).unwrap().cursor, 0);
        files.commit_read_begin(begin);

        let before = files.clone();
        let mut wrong = RuntimeSourceReadHostResponse {
            request_id: 3,
            source_id: 7,
            object_id: 11,
            offset: 0,
            requested_bytes: 4,
            transferred_bytes: 4,
            win32_error: 0,
            bytes: vec![1, 2, 3, 4],
        };
        wrong.object_id = 12;
        assert_eq!(
            files.plan_read_completion(wrong).unwrap_err().code,
            "source-read-completion-identity-mismatch"
        );
        assert_eq!(files, before);

        let plan = files
            .plan_read_completion(RuntimeSourceReadHostResponse {
                request_id: 3,
                source_id: 7,
                object_id: 11,
                offset: 0,
                requested_bytes: 4,
                transferred_bytes: 4,
                win32_error: 0,
                bytes: vec![1, 2, 3, 4],
            })
            .unwrap();
        assert_eq!(plan.buffer(), GuestAddress(0x1000));
        assert_eq!(plan.bytes_read(), GuestAddress(0x2000));
        let completion = files.commit_read_completion(plan);
        assert!(completion.succeeded);
        assert_eq!(completion.cursor_after, 4);
        assert_eq!(files.open_file(id).unwrap().cursor, 4);
    }
}
