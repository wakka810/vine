use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::memory::GuestAddress;

use super::{RuntimeHandle, RuntimeStateError};

pub const RUNTIME_SOURCE_SNAPSHOT_SCHEMA: &str = "vine.source-snapshot-manifest";
pub const RUNTIME_SOURCE_SNAPSHOT_VERSION: u32 = 1;
pub const RUNTIME_SOURCE_FILE_NAMESPACE: &str = "vine.runtime-source-file.v1";
pub const RUNTIME_FILE_SEARCH_NAMESPACE: &str = "vine.runtime-file-search.v1";
pub const RUNTIME_FILE_SEARCH_LIMIT: usize = 4096;
pub const RUNTIME_SOURCE_READ_SERVICE: &str = "storage.source-read.v1";
pub const RUNTIME_SOURCE_READ_TRANSPORT_VERSION: u32 = 1;
pub const RUNTIME_SOURCE_READ_REQUEST_BYTES: usize = 24;
pub const RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES: usize = 32;
pub const RUNTIME_SOURCE_FILE_LIMIT: usize = 4096;
pub const RUNTIME_SOURCE_READ_MAX_BYTES: u32 = 1024 * 1024;
pub const RUNTIME_WRITABLE_PROFILE_SCHEMA: &str = "vine.writable-profile-snapshot";
pub const RUNTIME_WRITABLE_PROFILE_VERSION: u32 = 3;
pub const RUNTIME_WRITABLE_PROFILE_FILE_LIMIT: usize = 64;
pub const RUNTIME_WRITABLE_PROFILE_DIRECTORY_LIMIT: usize = 64;
pub const RUNTIME_WRITABLE_PROFILE_MAX_BYTES: u64 = 16 * 1024 * 1024;
pub const RUNTIME_WRITABLE_FILE_MAX_IO_BYTES: u32 = 1024 * 1024;
pub const RUNTIME_WRITABLE_READ_SERVICE: &str = "storage.profile-read.v1";
pub const RUNTIME_WRITABLE_COMMIT_SERVICE: &str = "storage.profile-commit.v1";
pub const RUNTIME_WRITABLE_COMMIT_TRANSPORT_VERSION: u32 = 1;
pub const RUNTIME_WRITABLE_COMMIT_REQUEST_BYTES: usize = 28;
pub const RUNTIME_WRITABLE_COMMIT_RESPONSE_BYTES: usize = 32;
pub const RUNTIME_WRITABLE_DELETE_SERVICE: &str = "storage.profile-delete.v1";
pub const RUNTIME_WRITABLE_DELETE_TRANSPORT_VERSION: u32 = 1;
pub const RUNTIME_WRITABLE_DELETE_REQUEST_BYTES: usize = 16;
pub const RUNTIME_WRITABLE_DIRECTORY_CREATE_SERVICE: &str = "storage.profile-directory-create.v1";
pub const RUNTIME_WRITABLE_DIRECTORY_CREATE_TRANSPORT_VERSION: u32 = 1;
pub const RUNTIME_WRITABLE_DIRECTORY_CREATE_REQUEST_BYTES: usize = 16;
pub const RUNTIME_WRITABLE_DIRECTORY_CREATE_RESPONSE_BYTES: usize = 32;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeWritableProfileManifest {
    pub schema: String,
    pub schema_version: u32,
    pub title_fingerprint: String,
    pub profile_id: String,
    pub root_path_a: Vec<u8>,
    pub quota_bytes: u64,
    pub files: Vec<RuntimeWritableFileManifest>,
    pub directories: Vec<RuntimeWritableDirectoryManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeWritableFileManifest {
    pub object_id: u32,
    pub relative_path_a: Vec<u8>,
    pub exists: bool,
    pub generation: u64,
    pub byte_length: u64,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeWritableDirectoryManifest {
    pub directory_id: u32,
    pub relative_path_a: Vec<u8>,
    pub exists: bool,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWritableProfile {
    pub title_fingerprint: String,
    pub profile_id: String,
    pub root_path_a: Vec<u8>,
    pub quota_bytes: u64,
    pub files: BTreeMap<u32, RuntimeWritableFile>,
    pub directories: BTreeMap<u32, RuntimeWritableDirectory>,
    #[serde(skip)]
    path_index: BTreeMap<Vec<u8>, u32>,
    #[serde(skip)]
    directory_path_index: BTreeMap<Vec<u8>, u32>,
    #[serde(skip)]
    root_identity: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWritableFile {
    pub object_id: u32,
    pub relative_path_a: Vec<u8>,
    pub exists: bool,
    pub generation: u64,
    pub byte_length: u64,
    pub sha256: Option<String>,
    pub dirty: bool,
    #[serde(skip)]
    content_loaded: bool,
    #[serde(skip)]
    committed_bytes: Vec<u8>,
    #[serde(skip)]
    working_bytes: Vec<u8>,
    #[serde(skip)]
    dirty_base_generation: Option<u64>,
    #[serde(skip)]
    revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWritableDirectory {
    pub directory_id: u32,
    pub relative_path_a: Vec<u8>,
    pub exists: bool,
    pub generation: u64,
    #[serde(skip)]
    revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeSourceFileId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeFileSearchId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeFileSearchEntryKind {
    RegularFile,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFileSearchEntry {
    pub file_name_a: Vec<u8>,
    pub kind: RuntimeFileSearchEntryKind,
    pub byte_length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFileSearch {
    pub id: RuntimeFileSearchId,
    pub normalized_pattern_a: Vec<u8>,
    pub entries: Vec<RuntimeFileSearchEntry>,
    pub next_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFindFirstFileResult {
    pub search: RuntimeFileSearch,
    pub handle: RuntimeHandle,
    pub entry: RuntimeFileSearchEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimeFileBacking {
    ImmutableSource { source_id: u32, object_id: u32 },
    WritableProfile { object_id: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimePathResolution {
    RegularFile,
    Directory,
    MissingFile,
    MissingPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSourceFile {
    pub id: RuntimeSourceFileId,
    pub backing: RuntimeFileBacking,
    pub normalized_path_a: Vec<u8>,
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
    pub existed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWritableReadResult {
    pub file_id: RuntimeSourceFileId,
    pub object_id: u32,
    pub cursor_before: u64,
    pub cursor_after: u64,
    pub transferred_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWritableWriteResult {
    pub file_id: RuntimeSourceFileId,
    pub object_id: u32,
    pub cursor_before: u64,
    pub cursor_after: u64,
    pub transferred_bytes: u32,
    pub byte_length: u64,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWritableCommitRequest {
    pub request_id: u64,
    pub file_id: RuntimeSourceFileId,
    pub object_id: u32,
    pub base_generation: u64,
    pub byte_length: u64,
    pub content_sha256: String,
    pub close_after_commit: bool,
}

impl RuntimeWritableCommitRequest {
    pub fn encode_host_payload(&self) -> [u8; RUNTIME_WRITABLE_COMMIT_REQUEST_BYTES] {
        let words = [
            RUNTIME_WRITABLE_COMMIT_TRANSPORT_VERSION,
            self.object_id,
            self.base_generation as u32,
            (self.base_generation >> 32) as u32,
            self.byte_length as u32,
            (self.byte_length >> 32) as u32,
            u32::from(self.close_after_commit),
        ];
        let mut payload = [0; RUNTIME_WRITABLE_COMMIT_REQUEST_BYTES];
        for (word, destination) in words.into_iter().zip(payload.chunks_exact_mut(4)) {
            destination.copy_from_slice(&word.to_le_bytes());
        }
        payload
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWritableCommitHostResponse {
    pub request_id: u64,
    pub object_id: u32,
    pub base_generation: u64,
    pub committed_generation: u64,
    pub win32_error: u32,
}

impl RuntimeWritableCommitHostResponse {
    pub fn decode_binary(request_id: u64, control: &[u8]) -> Result<Self, RuntimeStateError> {
        if control.len() != RUNTIME_WRITABLE_COMMIT_RESPONSE_BYTES {
            return Err(vfs_invalid(
                "writable-commit-response-size-invalid",
                "writable commit response must contain eight little-endian words",
            ));
        }
        let words: [u32; 8] = std::array::from_fn(|index| {
            let offset = index * 4;
            u32::from_le_bytes(
                control[offset..offset + 4]
                    .try_into()
                    .expect("four-byte writable commit response chunk"),
            )
        });
        if words[0] != RUNTIME_WRITABLE_COMMIT_TRANSPORT_VERSION || words[7] != 0 {
            return Err(vfs_invalid(
                "writable-commit-response-version-invalid",
                "writable commit response uses an unsupported version or reserved word",
            ));
        }
        Ok(Self {
            request_id,
            object_id: words[1],
            base_generation: u64::from(words[2]) | (u64::from(words[3]) << 32),
            committed_generation: u64::from(words[4]) | (u64::from(words[5]) << 32),
            win32_error: words[6],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWritableCommitCompletion {
    pub request: RuntimeWritableCommitRequest,
    pub succeeded: bool,
    pub committed_generation: u64,
    pub win32_error: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWritableDeleteRequest {
    pub request_id: u64,
    pub object_id: u32,
    pub relative_path_a: Vec<u8>,
    pub base_generation: u64,
}

impl RuntimeWritableDeleteRequest {
    pub fn encode_host_payload(&self) -> [u8; RUNTIME_WRITABLE_DELETE_REQUEST_BYTES] {
        let words = [
            RUNTIME_WRITABLE_DELETE_TRANSPORT_VERSION,
            self.object_id,
            self.base_generation as u32,
            (self.base_generation >> 32) as u32,
        ];
        let mut payload = [0; RUNTIME_WRITABLE_DELETE_REQUEST_BYTES];
        for (word, destination) in words.into_iter().zip(payload.chunks_exact_mut(4)) {
            destination.copy_from_slice(&word.to_le_bytes());
        }
        payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWritableDeleteCompletion {
    pub request: RuntimeWritableDeleteRequest,
    pub succeeded: bool,
    pub committed_generation: u64,
    pub win32_error: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWritableDirectoryCreateRequest {
    pub request_id: u64,
    pub directory_id: u32,
    pub relative_path_a: Vec<u8>,
    pub base_generation: u64,
}

impl RuntimeWritableDirectoryCreateRequest {
    pub fn encode_host_payload(&self) -> [u8; RUNTIME_WRITABLE_DIRECTORY_CREATE_REQUEST_BYTES] {
        let words = [
            RUNTIME_WRITABLE_DIRECTORY_CREATE_TRANSPORT_VERSION,
            self.directory_id,
            self.base_generation as u32,
            (self.base_generation >> 32) as u32,
        ];
        let mut payload = [0; RUNTIME_WRITABLE_DIRECTORY_CREATE_REQUEST_BYTES];
        for (word, destination) in words.into_iter().zip(payload.chunks_exact_mut(4)) {
            destination.copy_from_slice(&word.to_le_bytes());
        }
        payload
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWritableDirectoryCreateHostResponse {
    pub request_id: u64,
    pub directory_id: u32,
    pub base_generation: u64,
    pub committed_generation: u64,
    pub win32_error: u32,
}

impl RuntimeWritableDirectoryCreateHostResponse {
    pub fn decode_binary(request_id: u64, control: &[u8]) -> Result<Self, RuntimeStateError> {
        if control.len() != RUNTIME_WRITABLE_DIRECTORY_CREATE_RESPONSE_BYTES {
            return Err(vfs_invalid(
                "writable-directory-create-response-size-invalid",
                "writable directory create response must contain eight little-endian words",
            ));
        }
        let words: [u32; 8] = std::array::from_fn(|index| {
            let offset = index * 4;
            u32::from_le_bytes(
                control[offset..offset + 4]
                    .try_into()
                    .expect("four-byte writable directory create response chunk"),
            )
        });
        if words[0] != RUNTIME_WRITABLE_DIRECTORY_CREATE_TRANSPORT_VERSION || words[7] != 0 {
            return Err(vfs_invalid(
                "writable-directory-create-response-version-invalid",
                "writable directory create response uses an unsupported version or reserved word",
            ));
        }
        Ok(Self {
            request_id,
            directory_id: words[1],
            base_generation: u64::from(words[2]) | (u64::from(words[3]) << 32),
            committed_generation: u64::from(words[4]) | (u64::from(words[5]) << 32),
            win32_error: words[6],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWritableDirectoryCreateCompletion {
    pub request: RuntimeWritableDirectoryCreateRequest,
    pub succeeded: bool,
    pub committed_generation: u64,
    pub win32_error: u32,
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
        let words: [u32; 8] = std::array::from_fn(|index| {
            let offset = index * 4;
            u32::from_le_bytes(
                control[offset..offset + 4]
                    .try_into()
                    .expect("four-byte source read response control chunk"),
            )
        });
        if words[0] != RUNTIME_SOURCE_READ_TRANSPORT_VERSION {
            return Err(vfs_invalid(
                "source-read-response-version-unsupported",
                "source read response uses an unsupported transport version",
            ));
        }
        let requested_bytes = words[5];
        let transferred_bytes = words[6];
        let win32_error = words[7];
        if requested_bytes == 0 || requested_bytes > RUNTIME_SOURCE_READ_MAX_BYTES {
            return Err(vfs_invalid(
                "source-read-response-request-size-invalid",
                "source read response request size exceeds the transport contract",
            ));
        }
        let valid_payload = if win32_error == 0 {
            transferred_bytes <= requested_bytes
                && usize::try_from(transferred_bytes) == Ok(payload.len())
        } else {
            transferred_bytes == 0 && payload.is_empty()
        };
        if !valid_payload {
            return Err(vfs_invalid(
                "source-read-response-payload-invalid",
                "source read response payload does not match its result metadata",
            ));
        }
        Ok(Self {
            request_id,
            source_id: words[1],
            object_id: words[2],
            offset: u64::from(words[3]) | (u64::from(words[4]) << 32),
            requested_bytes,
            transferred_bytes,
            win32_error,
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
    writable_profile: Option<RuntimeWritableProfile>,
    open_files: BTreeMap<RuntimeSourceFileId, RuntimeSourceFile>,
    pending_reads: BTreeMap<u64, RuntimePendingSourceRead>,
    pending_commits: BTreeMap<u64, RuntimePendingWritableCommit>,
    pending_deletes: BTreeMap<u64, RuntimePendingWritableDelete>,
    pending_directory_creates: BTreeMap<u64, RuntimePendingWritableDirectoryCreate>,
    searches: BTreeMap<RuntimeFileSearchId, RuntimeFileSearch>,
    next_file_id: u64,
    next_search_id: u64,
    catalog_revision: u64,
}

impl Default for RuntimeFiles {
    fn default() -> Self {
        Self {
            snapshot: None,
            writable_profile: None,
            open_files: BTreeMap::new(),
            pending_reads: BTreeMap::new(),
            pending_commits: BTreeMap::new(),
            pending_deletes: BTreeMap::new(),
            pending_directory_creates: BTreeMap::new(),
            searches: BTreeMap::new(),
            next_file_id: 1,
            next_search_id: 1,
            catalog_revision: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceSnapshotRegistrationPlan {
    next: RuntimeSourceSnapshot,
    expected_catalog_revision: u64,
    next_catalog_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableProfileRegistrationPlan {
    next: RuntimeWritableProfile,
    expected_catalog_revision: u64,
    next_catalog_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableProfileHydrationPlan {
    previous: RuntimeWritableFile,
    next: RuntimeWritableFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceFileOpenPlan {
    expected_next_file_id: u64,
    next_file_id: u64,
    file: RuntimeSourceFile,
    existed: bool,
    previous_writable: Option<RuntimeWritableFile>,
    next_writable: Option<RuntimeWritableFile>,
    expected_catalog_revision: u64,
    next_catalog_revision: u64,
}

impl RuntimeSourceFileOpenPlan {
    pub(super) const fn file_id(&self) -> RuntimeSourceFileId {
        self.file.id
    }

    pub(super) const fn existed(&self) -> bool {
        self.existed
    }

    #[cfg(test)]
    pub(super) fn file(&self) -> &RuntimeSourceFile {
        &self.file
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeFileSearchOpenPlan {
    expected_next_search_id: u64,
    next_search_id: u64,
    expected_catalog_revision: u64,
    search: RuntimeFileSearch,
}

impl RuntimeFileSearchOpenPlan {
    pub(super) const fn search_id(&self) -> RuntimeFileSearchId {
        self.search.id
    }

    pub(super) fn first_entry(&self) -> &RuntimeFileSearchEntry {
        self.search
            .entries
            .first()
            .expect("file search open plans require at least one entry")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeFileSearchAdvancePlan {
    previous: RuntimeFileSearch,
    next: RuntimeFileSearch,
    entry: RuntimeFileSearchEntry,
}

impl RuntimeFileSearchAdvancePlan {
    pub(super) fn entry(&self) -> &RuntimeFileSearchEntry {
        &self.entry
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeFileSearchRetirementPlan {
    previous: RuntimeFileSearch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceFileRetirementPlan {
    previous: RuntimeSourceFile,
    pending_reads: Vec<RuntimePendingSourceRead>,
    pending_commits: Vec<RuntimePendingWritableCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceFileSeekPlan {
    previous: RuntimeSourceFile,
    next: RuntimeSourceFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableReadPlan {
    previous_file: RuntimeSourceFile,
    next_file: RuntimeSourceFile,
    bytes: Vec<u8>,
    result: RuntimeWritableReadResult,
}

impl RuntimeWritableReadPlan {
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(super) const fn result(&self) -> RuntimeWritableReadResult {
        self.result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableWritePlan {
    previous_file: RuntimeSourceFile,
    next_file: RuntimeSourceFile,
    previous_writable: RuntimeWritableFile,
    next_writable: RuntimeWritableFile,
    result: RuntimeWritableWriteResult,
    expected_catalog_revision: u64,
    next_catalog_revision: u64,
}

impl RuntimeWritableWritePlan {
    pub(super) const fn result(&self) -> RuntimeWritableWriteResult {
        self.result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimePendingSourceRead {
    request: RuntimeSourceReadRequest,
    buffer: GuestAddress,
    bytes_read: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimePendingWritableCommit {
    request: RuntimeWritableCommitRequest,
    expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimePendingWritableDelete {
    request: RuntimeWritableDeleteRequest,
    expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimePendingWritableDirectoryCreate {
    request: RuntimeWritableDirectoryCreateRequest,
    expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableCommitBeginPlan {
    pending: RuntimePendingWritableCommit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableCommitCompletionPlan {
    pending: RuntimePendingWritableCommit,
    previous_writable: RuntimeWritableFile,
    next_writable: RuntimeWritableFile,
    completion: RuntimeWritableCommitCompletion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableDeleteBeginPlan {
    pending: RuntimePendingWritableDelete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableDeleteCompletionPlan {
    pending: RuntimePendingWritableDelete,
    previous_writable: RuntimeWritableFile,
    next_writable: RuntimeWritableFile,
    completion: RuntimeWritableDeleteCompletion,
    expected_catalog_revision: u64,
    next_catalog_revision: u64,
}

impl RuntimeWritableDeleteCompletionPlan {
    pub(super) fn completion(&self) -> &RuntimeWritableDeleteCompletion {
        &self.completion
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableDirectoryCreateBeginPlan {
    pending: RuntimePendingWritableDirectoryCreate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableDirectoryCreateCompletionPlan {
    pending: RuntimePendingWritableDirectoryCreate,
    previous_directory: RuntimeWritableDirectory,
    next_directory: RuntimeWritableDirectory,
    completion: RuntimeWritableDirectoryCreateCompletion,
    expected_catalog_revision: u64,
    next_catalog_revision: u64,
}

impl RuntimeWritableDirectoryCreateCompletionPlan {
    pub(super) fn completion(&self) -> &RuntimeWritableDirectoryCreateCompletion {
        &self.completion
    }
}

impl RuntimeWritableCommitCompletionPlan {
    pub(super) fn completion(&self) -> &RuntimeWritableCommitCompletion {
        &self.completion
    }

    pub(super) const fn closes_file(&self) -> bool {
        self.pending.request.close_after_commit && self.completion.succeeded
    }

    pub(super) const fn file_id(&self) -> RuntimeSourceFileId {
        self.pending.request.file_id
    }
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

    pub(super) const fn service(&self) -> &'static str {
        if self.pending.request.source_id == 0 {
            RUNTIME_WRITABLE_READ_SERVICE
        } else {
            RUNTIME_SOURCE_READ_SERVICE
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSourceReadCancellationPlan {
    requests: Vec<RuntimePendingSourceRead>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableCommitCancellationPlan {
    requests: Vec<RuntimePendingWritableCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableDeleteCancellationPlan {
    requests: Vec<RuntimePendingWritableDelete>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWritableDirectoryCreateCancellationPlan {
    requests: Vec<RuntimePendingWritableDirectoryCreate>,
}

impl RuntimeWritableCommitCancellationPlan {
    pub(super) fn request_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.requests
            .iter()
            .map(|pending| pending.request.request_id)
    }
}

impl RuntimeSourceReadCancellationPlan {
    pub(super) fn request_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.requests
            .iter()
            .map(|pending| pending.request.request_id)
    }
}

impl RuntimeFiles {
    fn plan_catalog_change(&self) -> Result<(u64, u64), RuntimeStateError> {
        let next = self.catalog_revision.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-vfs",
                "file-catalog-revision-exhausted",
                "runtime file catalog revision space is exhausted",
            )
        })?;
        Ok((self.catalog_revision, next))
    }

    pub(super) fn snapshot(&self) -> Option<&RuntimeSourceSnapshot> {
        self.snapshot.as_ref()
    }

    pub(super) fn writable_profile(&self) -> Option<&RuntimeWritableProfile> {
        self.writable_profile.as_ref()
    }

    pub(super) fn resolve_path_kind(
        &self,
        requested_path: &[u8],
        current_directory: &[u8],
    ) -> Result<RuntimePathResolution, RuntimeStateError> {
        let normalized = resolve_path(requested_path, current_directory)?;
        let root_identity = self
            .writable_profile
            .as_ref()
            .map(|profile| profile.root_identity.as_slice())
            .or_else(|| {
                self.snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.root_identity.as_slice())
            })
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-vfs",
                    "file-mount-unregistered",
                    "runtime process has no immutable source or writable profile mount",
                )
            })?;
        if normalized.identity == root_identity {
            return Ok(RuntimePathResolution::Directory);
        }
        if normalized.identity.len() <= root_identity.len()
            || !normalized.identity.starts_with(root_identity)
            || normalized.identity.get(root_identity.len()) != Some(&b'\\')
        {
            return Ok(RuntimePathResolution::MissingPath);
        }
        let relative = &normalized.identity[root_identity.len() + 1..];
        if let Some(directory_id) = self
            .writable_profile
            .as_ref()
            .and_then(|profile| profile.directory_path_index.get(relative).copied())
        {
            let profile = self
                .writable_profile
                .as_ref()
                .expect("writable directory path requires profile");
            let directory = profile.directories.get(&directory_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-vfs",
                    "writable-directory-path-index-corrupt",
                    "writable directory path index points to a missing identity",
                )
            })?;
            if directory.exists {
                return Ok(RuntimePathResolution::Directory);
            }
        }
        if let Some(object_id) = self
            .writable_profile
            .as_ref()
            .and_then(|profile| profile.path_index.get(relative).copied())
        {
            let profile = self
                .writable_profile
                .as_ref()
                .expect("writable path requires profile");
            let file = profile.files.get(&object_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-vfs",
                    "writable-path-index-corrupt",
                    "writable path index points to a missing object",
                )
            })?;
            return Ok(if file.exists {
                RuntimePathResolution::RegularFile
            } else {
                RuntimePathResolution::MissingFile
            });
        }
        if let Some(object_id) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.path_index.get(relative).copied())
        {
            let snapshot = self
                .snapshot
                .as_ref()
                .expect("source path requires snapshot");
            if !snapshot.files.contains_key(&object_id) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-vfs",
                    "source-path-index-corrupt",
                    "source path index points to a missing object descriptor",
                ));
            }
            return Ok(RuntimePathResolution::RegularFile);
        }
        if self.registered_directory_exists(relative) {
            return Ok(RuntimePathResolution::Directory);
        }
        let parent = relative
            .iter()
            .rposition(|byte| *byte == b'\\')
            .map_or(&[][..], |separator| &relative[..separator]);
        Ok(
            if parent.is_empty() || self.registered_directory_exists(parent) {
                RuntimePathResolution::MissingFile
            } else {
                RuntimePathResolution::MissingPath
            },
        )
    }

    fn registered_directory_exists(&self, relative: &[u8]) -> bool {
        let is_descendant = |path: &[u8]| {
            path.len() > relative.len()
                && path.starts_with(relative)
                && (relative.is_empty() || path.get(relative.len()) == Some(&b'\\'))
        };
        self.snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.path_index.keys().any(|path| is_descendant(path)))
            || self.writable_profile.as_ref().is_some_and(|profile| {
                profile.path_index.keys().any(|path| is_descendant(path))
                    || profile
                        .directory_path_index
                        .iter()
                        .any(|(path, directory_id)| {
                            profile
                                .directories
                                .get(directory_id)
                                .is_some_and(|directory| directory.exists)
                                && (path.as_slice() == relative || is_descendant(path))
                        })
            })
    }

    fn enumerate_file_search_entries(
        &self,
        pattern: &ResolvedFileSearchPattern,
        maximum_matches: usize,
    ) -> Result<Vec<RuntimeFileSearchEntry>, RuntimeStateError> {
        let root_identity = self
            .writable_profile
            .as_ref()
            .map(|profile| profile.root_identity.as_slice())
            .or_else(|| {
                self.snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.root_identity.as_slice())
            })
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-vfs",
                    "file-mount-unregistered",
                    "file search requires an immutable source or writable profile mount",
                )
            })?;
        let directory_relative = if pattern.directory_identity == root_identity {
            &[][..]
        } else {
            relative_to_mount(&pattern.directory_identity, root_identity)?
        };
        if !directory_relative.is_empty() && !self.registered_directory_exists(directory_relative) {
            return Err(RuntimeStateError::not_found(
                "runtime-vfs",
                "file-search-directory-missing",
                "file search directory is not registered in the runtime VFS",
            ));
        }

        let mut candidates = BTreeMap::<Vec<u8>, RuntimeFileSearchEntry>::new();
        let mut insert_path = |relative_path: &[u8], kind, byte_length| {
            if let Some((name, immediate_kind)) =
                immediate_search_child(relative_path, directory_relative, kind)
            {
                let identity = windows_932_ascii_fold(&name);
                candidates
                    .entry(identity)
                    .or_insert(RuntimeFileSearchEntry {
                        file_name_a: name,
                        kind: immediate_kind,
                        byte_length: if immediate_kind == RuntimeFileSearchEntryKind::RegularFile {
                            byte_length
                        } else {
                            0
                        },
                    });
            }
        };

        if let Some(snapshot) = self.snapshot.as_ref() {
            for descriptor in snapshot.files.values() {
                insert_path(
                    &descriptor.relative_path_a,
                    RuntimeFileSearchEntryKind::RegularFile,
                    descriptor.byte_length,
                );
            }
        }
        if let Some(profile) = self.writable_profile.as_ref() {
            for file in profile.files.values().filter(|file| file.exists) {
                insert_path(
                    &file.relative_path_a,
                    RuntimeFileSearchEntryKind::RegularFile,
                    file.byte_length,
                );
            }
            for directory in profile
                .directories
                .values()
                .filter(|directory| directory.exists)
            {
                insert_path(
                    &directory.relative_path_a,
                    RuntimeFileSearchEntryKind::Directory,
                    0,
                );
            }
        }

        let mut entries = Vec::new();
        for candidate in candidates.into_values() {
            if !windows_932_wildcard_matches(&pattern.file_name_pattern, &candidate.file_name_a)? {
                continue;
            }
            if entries.len() == maximum_matches {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-vfs",
                    "file-search-match-limit-exceeded",
                    "file search result count exceeds the policy bound",
                ));
            }
            entries.push(candidate);
        }
        Ok(entries)
    }

    pub(super) fn plan_writable_delete(
        &self,
        requested_path: &[u8],
        current_directory: &[u8],
        request_id: u64,
    ) -> Result<RuntimeWritableDeleteBeginPlan, RuntimeStateError> {
        let normalized = resolve_path(requested_path, current_directory)?;
        let profile = self.writable_profile.as_ref().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-vfs",
                "writable-profile-missing",
                "file deletion requires a registered writable profile",
            )
        })?;
        let relative = relative_to_mount(&normalized.identity, &profile.root_identity)?;
        let object_id = profile.path_index.get(relative).copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-vfs",
                "writable-file-delete-unregistered",
                "file deletion path is outside the writable profile allowlist",
            )
        })?;
        let writable = profile.files.get(&object_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "writable-file-path-index-corrupt",
                "writable file path index points to a missing identity",
            )
        })?;
        if !writable.exists {
            return Err(RuntimeStateError::not_found(
                "runtime-vfs",
                "writable-file-delete-missing",
                "the writable file does not exist",
            ));
        }
        if writable.dirty
            || self.open_files.values().any(|file| {
                matches!(
                    file.backing,
                    RuntimeFileBacking::WritableProfile { object_id: open_id }
                        if open_id == object_id
                )
            })
            || self
                .pending_commits
                .values()
                .any(|pending| pending.request.object_id == object_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-file-delete-sharing-violation",
                "the writable file is open or owns a pending operation",
            ));
        }
        if self
            .pending_deletes
            .values()
            .any(|pending| pending.request.object_id == object_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-file-delete-pending",
                "the writable file already owns a pending durable deletion",
            ));
        }
        Ok(RuntimeWritableDeleteBeginPlan {
            pending: RuntimePendingWritableDelete {
                request: RuntimeWritableDeleteRequest {
                    request_id,
                    object_id,
                    relative_path_a: writable.relative_path_a.clone(),
                    base_generation: writable.generation,
                },
                expected_revision: writable.revision,
            },
        })
    }

    pub(super) fn preflight_writable_delete(
        &self,
        plan: &RuntimeWritableDeleteBeginPlan,
    ) -> Result<(), RuntimeStateError> {
        let pending = &plan.pending;
        let writable = self.writable_file(pending.request.object_id)?;
        if !writable.exists
            || writable.dirty
            || writable.generation != pending.request.base_generation
            || writable.revision != pending.expected_revision
            || writable.relative_path_a != pending.request.relative_path_a
            || self
                .pending_deletes
                .contains_key(&pending.request.request_id)
            || self
                .pending_deletes
                .values()
                .any(|existing| existing.request.object_id == pending.request.object_id)
            || self.open_files.values().any(|file| {
                matches!(
                    file.backing,
                    RuntimeFileBacking::WritableProfile { object_id }
                        if object_id == pending.request.object_id
                )
            })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-file-delete-plan-stale",
                "writable file state changed after delete planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_writable_delete_begin(
        &mut self,
        plan: RuntimeWritableDeleteBeginPlan,
    ) -> RuntimeWritableDeleteRequest {
        debug_assert!(self.preflight_writable_delete(&plan).is_ok());
        let request = plan.pending.request.clone();
        let previous = self
            .pending_deletes
            .insert(request.request_id, plan.pending);
        debug_assert!(previous.is_none());
        request
    }

    pub(super) fn writable_delete_request(
        &self,
        request_id: u64,
    ) -> Result<&RuntimeWritableDeleteRequest, RuntimeStateError> {
        self.pending_deletes
            .get(&request_id)
            .map(|pending| &pending.request)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-vfs",
                    "writable-file-delete-request-unknown",
                    "file delete metadata does not identify a pending request",
                )
            })
    }

    pub(super) fn plan_writable_delete_completion(
        &self,
        response: RuntimeWritableCommitHostResponse,
    ) -> Result<RuntimeWritableDeleteCompletionPlan, RuntimeStateError> {
        let pending = self
            .pending_deletes
            .get(&response.request_id)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-vfs",
                    "writable-file-delete-request-unknown",
                    "file delete completion does not identify a pending request",
                )
            })?
            .clone();
        let previous_writable = self.writable_file(pending.request.object_id)?.clone();
        if response.object_id != pending.request.object_id
            || response.base_generation != pending.request.base_generation
            || !previous_writable.exists
            || previous_writable.dirty
            || previous_writable.generation != pending.request.base_generation
            || previous_writable.revision != pending.expected_revision
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-file-delete-completion-identity-mismatch",
                "file delete completion identity or generation is stale",
            ));
        }
        let succeeded = response.win32_error == 0;
        let expected_generation = response.base_generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-vfs",
                "writable-generation-exhausted",
                "writable profile generation space is exhausted",
            )
        })?;
        if (succeeded && response.committed_generation != expected_generation)
            || (!succeeded && response.committed_generation != response.base_generation)
        {
            return Err(vfs_invalid(
                "writable-file-delete-completion-generation-invalid",
                "file delete response generation does not match success or failure semantics",
            ));
        }
        let mut next_writable = previous_writable.clone();
        if succeeded {
            next_writable.exists = false;
            next_writable.generation = 0;
            next_writable.byte_length = 0;
            next_writable.sha256 = None;
            next_writable.dirty = false;
            next_writable.content_loaded = true;
            next_writable.committed_bytes.clear();
            next_writable.working_bytes.clear();
            next_writable.dirty_base_generation = None;
            next_writable.revision = next_writable.revision.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-vfs",
                    "writable-file-revision-exhausted",
                    "writable file revision space is exhausted",
                )
            })?;
        }
        let (expected_catalog_revision, next_catalog_revision) = if succeeded {
            self.plan_catalog_change()?
        } else {
            (self.catalog_revision, self.catalog_revision)
        };
        Ok(RuntimeWritableDeleteCompletionPlan {
            pending: pending.clone(),
            previous_writable,
            next_writable,
            completion: RuntimeWritableDeleteCompletion {
                request: pending.request,
                succeeded,
                committed_generation: response.committed_generation,
                win32_error: response.win32_error,
            },
            expected_catalog_revision,
            next_catalog_revision,
        })
    }

    pub(super) fn preflight_writable_delete_completion(
        &self,
        plan: &RuntimeWritableDeleteCompletionPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.pending_deletes.get(&plan.pending.request.request_id) != Some(&plan.pending)
            || self.catalog_revision != plan.expected_catalog_revision
            || self.writable_file(plan.previous_writable.object_id)? != &plan.previous_writable
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-file-delete-completion-plan-stale",
                "writable file state changed after delete completion planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_writable_delete_completion(
        &mut self,
        plan: RuntimeWritableDeleteCompletionPlan,
    ) -> RuntimeWritableDeleteCompletion {
        debug_assert!(self.preflight_writable_delete_completion(&plan).is_ok());
        self.pending_deletes
            .remove(&plan.pending.request.request_id);
        self.catalog_revision = plan.next_catalog_revision;
        self.writable_profile
            .as_mut()
            .expect("validated writable delete profile")
            .files
            .insert(plan.next_writable.object_id, plan.next_writable);
        plan.completion
    }

    pub(super) fn plan_writable_directory_create(
        &self,
        requested_path: &[u8],
        current_directory: &[u8],
        request_id: u64,
    ) -> Result<RuntimeWritableDirectoryCreateBeginPlan, RuntimeStateError> {
        let normalized = resolve_path(requested_path, current_directory)?;
        let profile = self.writable_profile.as_ref().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-vfs",
                "writable-profile-missing",
                "directory creation requires a registered writable profile",
            )
        })?;
        if normalized.identity == profile.root_identity {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-directory-already-exists",
                "the writable profile mount root already exists",
            ));
        }
        let relative = relative_to_mount(&normalized.identity, &profile.root_identity)?;
        if profile.path_index.contains_key(relative)
            || self
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.path_index.contains_key(relative))
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-directory-file-conflict",
                "directory creation path identifies a registered file",
            ));
        }
        let directory_id = profile
            .directory_path_index
            .get(relative)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-vfs",
                    "writable-directory-unregistered",
                    "directory creation path is outside the writable profile allowlist",
                )
            })?;
        let directory = profile.directories.get(&directory_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "writable-directory-path-index-corrupt",
                "writable directory path index points to a missing identity",
            )
        })?;
        if directory.exists {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-directory-already-exists",
                "the writable directory already exists",
            ));
        }
        if self
            .pending_directory_creates
            .values()
            .any(|pending| pending.request.directory_id == directory_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-directory-create-pending",
                "the writable directory already owns a pending durable create",
            ));
        }
        let parent = relative
            .iter()
            .rposition(|byte| *byte == b'\\')
            .map_or(&[][..], |separator| &relative[..separator]);
        if !parent.is_empty() && !self.registered_directory_exists(parent) {
            return Err(RuntimeStateError::not_found(
                "runtime-vfs",
                "writable-directory-parent-missing",
                "the writable directory parent does not exist",
            ));
        }
        Ok(RuntimeWritableDirectoryCreateBeginPlan {
            pending: RuntimePendingWritableDirectoryCreate {
                request: RuntimeWritableDirectoryCreateRequest {
                    request_id,
                    directory_id,
                    relative_path_a: directory.relative_path_a.clone(),
                    base_generation: directory.generation,
                },
                expected_revision: directory.revision,
            },
        })
    }

    pub(super) fn preflight_writable_directory_create(
        &self,
        plan: &RuntimeWritableDirectoryCreateBeginPlan,
    ) -> Result<(), RuntimeStateError> {
        let pending = &plan.pending;
        let directory = self.writable_directory(pending.request.directory_id)?;
        if directory.exists
            || directory.generation != pending.request.base_generation
            || directory.revision != pending.expected_revision
            || directory.relative_path_a != pending.request.relative_path_a
            || self
                .pending_directory_creates
                .contains_key(&pending.request.request_id)
            || self
                .pending_directory_creates
                .values()
                .any(|existing| existing.request.directory_id == pending.request.directory_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-directory-create-plan-stale",
                "writable directory state changed after create planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_writable_directory_create_begin(
        &mut self,
        plan: RuntimeWritableDirectoryCreateBeginPlan,
    ) -> RuntimeWritableDirectoryCreateRequest {
        debug_assert!(self.preflight_writable_directory_create(&plan).is_ok());
        let request = plan.pending.request.clone();
        let previous = self
            .pending_directory_creates
            .insert(request.request_id, plan.pending);
        debug_assert!(previous.is_none());
        request
    }

    pub(super) fn writable_directory_create_request(
        &self,
        request_id: u64,
    ) -> Result<&RuntimeWritableDirectoryCreateRequest, RuntimeStateError> {
        self.pending_directory_creates
            .get(&request_id)
            .map(|pending| &pending.request)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-vfs",
                    "writable-directory-create-request-unknown",
                    "directory create metadata does not identify a pending request",
                )
            })
    }

    pub(super) fn plan_writable_directory_create_completion(
        &self,
        response: RuntimeWritableDirectoryCreateHostResponse,
    ) -> Result<RuntimeWritableDirectoryCreateCompletionPlan, RuntimeStateError> {
        let pending = self
            .pending_directory_creates
            .get(&response.request_id)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-vfs",
                    "writable-directory-create-request-unknown",
                    "directory create completion does not identify a pending request",
                )
            })?
            .clone();
        let previous_directory = self
            .writable_directory(pending.request.directory_id)?
            .clone();
        if response.directory_id != pending.request.directory_id
            || response.base_generation != pending.request.base_generation
            || previous_directory.exists
            || previous_directory.generation != pending.request.base_generation
            || previous_directory.revision != pending.expected_revision
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-directory-create-completion-identity-mismatch",
                "directory create completion identity or generation is stale",
            ));
        }
        let succeeded = response.win32_error == 0;
        let expected_generation = response.base_generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-vfs",
                "writable-directory-generation-exhausted",
                "writable directory generation space is exhausted",
            )
        })?;
        if (succeeded && response.committed_generation != expected_generation)
            || (!succeeded && response.committed_generation != response.base_generation)
        {
            return Err(vfs_invalid(
                "writable-directory-create-completion-generation-invalid",
                "directory create response generation does not match success or failure semantics",
            ));
        }
        let mut next_directory = previous_directory.clone();
        if succeeded {
            next_directory.exists = true;
            next_directory.generation = response.committed_generation;
            next_directory.revision = next_directory.revision.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-vfs",
                    "writable-directory-revision-exhausted",
                    "writable directory revision space is exhausted",
                )
            })?;
        }
        let (expected_catalog_revision, next_catalog_revision) = if succeeded {
            self.plan_catalog_change()?
        } else {
            (self.catalog_revision, self.catalog_revision)
        };
        Ok(RuntimeWritableDirectoryCreateCompletionPlan {
            pending: pending.clone(),
            previous_directory,
            next_directory,
            completion: RuntimeWritableDirectoryCreateCompletion {
                request: pending.request,
                succeeded,
                committed_generation: response.committed_generation,
                win32_error: response.win32_error,
            },
            expected_catalog_revision,
            next_catalog_revision,
        })
    }

    pub(super) fn preflight_writable_directory_create_completion(
        &self,
        plan: &RuntimeWritableDirectoryCreateCompletionPlan,
    ) -> Result<(), RuntimeStateError> {
        if self
            .pending_directory_creates
            .get(&plan.pending.request.request_id)
            != Some(&plan.pending)
            || self.catalog_revision != plan.expected_catalog_revision
            || self.writable_directory(plan.previous_directory.directory_id)?
                != &plan.previous_directory
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-directory-create-completion-plan-stale",
                "writable directory state changed after completion planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_writable_directory_create_completion(
        &mut self,
        plan: RuntimeWritableDirectoryCreateCompletionPlan,
    ) -> RuntimeWritableDirectoryCreateCompletion {
        debug_assert!(
            self.preflight_writable_directory_create_completion(&plan)
                .is_ok()
        );
        self.pending_directory_creates
            .remove(&plan.pending.request.request_id);
        self.catalog_revision = plan.next_catalog_revision;
        self.writable_profile
            .as_mut()
            .expect("validated writable directory profile")
            .directories
            .insert(plan.next_directory.directory_id, plan.next_directory);
        plan.completion
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

    pub(super) fn file_search(
        &self,
        id: RuntimeFileSearchId,
    ) -> Result<&RuntimeFileSearch, RuntimeStateError> {
        self.searches.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-vfs",
                "file-search-id-unknown",
                "logical file search identity is not open",
            )
        })
    }

    pub(super) fn file_search_ids(&self) -> impl Iterator<Item = RuntimeFileSearchId> + '_ {
        self.searches.keys().copied()
    }

    pub(super) fn plan_find_first(
        &self,
        requested_pattern: &[u8],
        current_directory: &[u8],
        maximum_matches: usize,
    ) -> Result<Option<RuntimeFileSearchOpenPlan>, RuntimeStateError> {
        if maximum_matches == 0 || maximum_matches > RUNTIME_FILE_SEARCH_LIMIT {
            return Err(vfs_invalid(
                "file-search-match-limit-invalid",
                "file search requires one bounded nonzero match limit",
            ));
        }
        if self.searches.len() >= RUNTIME_FILE_SEARCH_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-vfs",
                "file-search-limit-exceeded",
                "logical file search count exceeds the runtime bound",
            ));
        }
        let pattern = resolve_file_search_pattern(requested_pattern, current_directory)?;
        let entries = self.enumerate_file_search_entries(&pattern, maximum_matches)?;
        if entries.is_empty() {
            return Ok(None);
        }
        let id = RuntimeFileSearchId(self.next_search_id);
        let next_search_id = self.next_search_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-vfs",
                "file-search-id-space-exhausted",
                "logical file search identifier space is exhausted",
            )
        })?;
        Ok(Some(RuntimeFileSearchOpenPlan {
            expected_next_search_id: self.next_search_id,
            next_search_id,
            expected_catalog_revision: self.catalog_revision,
            search: RuntimeFileSearch {
                id,
                normalized_pattern_a: pattern.normalized_pattern_a,
                entries,
                next_index: 1,
            },
        }))
    }

    pub(super) fn preflight_find_first(
        &self,
        plan: &RuntimeFileSearchOpenPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.next_search_id != plan.expected_next_search_id
            || self.catalog_revision != plan.expected_catalog_revision
            || self.searches.contains_key(&plan.search.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "file-search-open-plan-stale",
                "file catalog or search allocator changed after search planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_find_first(
        &mut self,
        plan: RuntimeFileSearchOpenPlan,
    ) -> RuntimeFileSearch {
        debug_assert!(self.preflight_find_first(&plan).is_ok());
        self.next_search_id = plan.next_search_id;
        let search = plan.search;
        let previous = self.searches.insert(search.id, search.clone());
        debug_assert!(previous.is_none());
        search
    }

    pub(super) fn plan_find_next(
        &self,
        id: RuntimeFileSearchId,
    ) -> Result<Option<RuntimeFileSearchAdvancePlan>, RuntimeStateError> {
        let previous = self.file_search(id)?.clone();
        let Some(entry) = previous.entries.get(previous.next_index).cloned() else {
            return Ok(None);
        };
        let mut next = previous.clone();
        next.next_index = next.next_index.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-vfs",
                "file-search-cursor-overflow",
                "logical file search cursor overflowed host accounting",
            )
        })?;
        Ok(Some(RuntimeFileSearchAdvancePlan {
            previous,
            next,
            entry,
        }))
    }

    pub(super) fn preflight_find_next(
        &self,
        plan: &RuntimeFileSearchAdvancePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.searches.get(&plan.previous.id) != Some(&plan.previous) {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "file-search-advance-plan-stale",
                "file search state changed after advance planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_find_next(
        &mut self,
        plan: RuntimeFileSearchAdvancePlan,
    ) -> RuntimeFileSearchEntry {
        debug_assert!(self.preflight_find_next(&plan).is_ok());
        self.searches.insert(plan.next.id, plan.next);
        plan.entry
    }

    pub(super) fn plan_file_search_retirement(
        &self,
        id: RuntimeFileSearchId,
    ) -> Result<RuntimeFileSearchRetirementPlan, RuntimeStateError> {
        Ok(RuntimeFileSearchRetirementPlan {
            previous: self.file_search(id)?.clone(),
        })
    }

    pub(super) fn preflight_file_search_retirement(
        &self,
        plan: &RuntimeFileSearchRetirementPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.searches.get(&plan.previous.id) != Some(&plan.previous) {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "file-search-retirement-plan-stale",
                "file search state changed after retirement planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_file_search_retirement(
        &mut self,
        plan: RuntimeFileSearchRetirementPlan,
    ) -> RuntimeFileSearch {
        debug_assert!(self.preflight_file_search_retirement(&plan).is_ok());
        self.searches
            .remove(&plan.previous.id)
            .expect("preflighted file search retirement")
    }

    pub(super) fn plan_snapshot_registration(
        &self,
        manifest: RuntimeSourceSnapshotManifest,
    ) -> Result<RuntimeSourceSnapshotRegistrationPlan, RuntimeStateError> {
        if self.snapshot.is_some()
            || !self.open_files.is_empty()
            || !self.pending_reads.is_empty()
            || !self.pending_commits.is_empty()
            || !self.pending_deletes.is_empty()
            || !self.pending_directory_creates.is_empty()
            || !self.searches.is_empty()
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
        let next = RuntimeSourceSnapshot {
            title_fingerprint: manifest.title_fingerprint,
            snapshot_id: manifest.snapshot_id,
            source_id: manifest.mount.source_id,
            root_path_a: root.bytes_with_nul,
            files,
            path_index,
            root_identity: root.identity,
        };
        validate_source_profile_separation(&next, self.writable_profile.as_ref())?;
        let (expected_catalog_revision, next_catalog_revision) = self.plan_catalog_change()?;
        Ok(RuntimeSourceSnapshotRegistrationPlan {
            next,
            expected_catalog_revision,
            next_catalog_revision,
        })
    }

    pub(super) fn commit_snapshot_registration(
        &mut self,
        plan: RuntimeSourceSnapshotRegistrationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.snapshot.is_some()
            || !self.open_files.is_empty()
            || !self.pending_reads.is_empty()
            || !self.pending_commits.is_empty()
            || !self.pending_deletes.is_empty()
            || !self.pending_directory_creates.is_empty()
            || !self.searches.is_empty()
            || self.catalog_revision != plan.expected_catalog_revision
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-snapshot-registration-plan-stale",
                "source file state changed after snapshot registration planning",
            ));
        }
        self.snapshot = Some(plan.next);
        self.catalog_revision = plan.next_catalog_revision;
        Ok(())
    }

    pub(super) fn plan_writable_profile_registration(
        &self,
        manifest: RuntimeWritableProfileManifest,
    ) -> Result<RuntimeWritableProfileRegistrationPlan, RuntimeStateError> {
        if self.writable_profile.is_some()
            || !self.open_files.is_empty()
            || !self.pending_reads.is_empty()
            || !self.pending_commits.is_empty()
            || !self.pending_deletes.is_empty()
            || !self.pending_directory_creates.is_empty()
            || !self.searches.is_empty()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-profile-already-registered",
                "runtime writable profile registration is process-static",
            ));
        }
        if manifest.schema != RUNTIME_WRITABLE_PROFILE_SCHEMA
            || manifest.schema_version != RUNTIME_WRITABLE_PROFILE_VERSION
            || manifest.title_fingerprint.is_empty()
            || manifest.title_fingerprint.len() > 128
            || manifest.profile_id.is_empty()
            || manifest.profile_id.len() > 128
            || manifest.quota_bytes == 0
            || manifest.quota_bytes > RUNTIME_WRITABLE_PROFILE_MAX_BYTES
            || manifest.files.is_empty()
            || manifest.files.len() > RUNTIME_WRITABLE_PROFILE_FILE_LIMIT
            || manifest.directories.len() > RUNTIME_WRITABLE_PROFILE_DIRECTORY_LIMIT
        {
            return Err(vfs_invalid(
                "writable-profile-manifest-invalid",
                "writable profile manifest identity, quota, or file bounds are invalid",
            ));
        }
        let root = normalize_absolute_path(&manifest.root_path_a)?;
        let mut files = BTreeMap::new();
        let mut path_index = BTreeMap::new();
        let mut total_bytes = 0_u64;
        for file in manifest.files {
            let relative = normalize_relative_path(&file.relative_path_a)?;
            let identity = windows_932_ascii_fold(&relative);
            let valid_content = if file.exists {
                file.generation > 0
                    && file.byte_length <= manifest.quota_bytes
                    && file.sha256.as_ref().is_some_and(|hash| is_sha256(hash))
            } else {
                file.generation == 0 && file.byte_length == 0 && file.sha256.is_none()
            };
            if file.object_id == 0 || !valid_content {
                return Err(vfs_invalid(
                    "writable-file-manifest-invalid",
                    "writable file manifest existence, generation, content, or hash is invalid",
                ));
            }
            if files.contains_key(&file.object_id) || path_index.contains_key(&identity) {
                return Err(vfs_invalid(
                    "writable-file-manifest-duplicate",
                    "writable file object or case-insensitive path identity is duplicated",
                ));
            }
            total_bytes = total_bytes.checked_add(file.byte_length).ok_or_else(|| {
                vfs_invalid(
                    "writable-profile-quota-invalid",
                    "writable profile content accounting overflowed",
                )
            })?;
            let sha256 = file.sha256.map(|hash| hash.to_ascii_lowercase());
            path_index.insert(identity, file.object_id);
            files.insert(
                file.object_id,
                RuntimeWritableFile {
                    object_id: file.object_id,
                    relative_path_a: relative,
                    exists: file.exists,
                    generation: file.generation,
                    byte_length: file.byte_length,
                    sha256,
                    dirty: false,
                    content_loaded: !file.exists,
                    committed_bytes: Vec::new(),
                    working_bytes: Vec::new(),
                    dirty_base_generation: None,
                    revision: 0,
                },
            );
        }
        let mut directories = BTreeMap::new();
        let mut directory_path_index = BTreeMap::new();
        for directory in manifest.directories {
            let relative = normalize_relative_path(&directory.relative_path_a)?;
            let identity = windows_932_ascii_fold(&relative);
            let valid_state = if directory.exists {
                directory.generation > 0
            } else {
                directory.generation == 0
            };
            if directory.directory_id == 0 || !valid_state {
                return Err(vfs_invalid(
                    "writable-directory-manifest-invalid",
                    "writable directory manifest identity, existence, or generation is invalid",
                ));
            }
            if directories.contains_key(&directory.directory_id)
                || directory_path_index.contains_key(&identity)
                || path_index.contains_key(&identity)
            {
                return Err(vfs_invalid(
                    "writable-directory-manifest-duplicate",
                    "writable directory identity or case-insensitive path is duplicated or conflicts with a file",
                ));
            }
            directory_path_index.insert(identity, directory.directory_id);
            directories.insert(
                directory.directory_id,
                RuntimeWritableDirectory {
                    directory_id: directory.directory_id,
                    relative_path_a: relative,
                    exists: directory.exists,
                    generation: directory.generation,
                    revision: 0,
                },
            );
        }
        if total_bytes > manifest.quota_bytes {
            return Err(RuntimeStateError::resource_limit(
                "runtime-vfs",
                "writable-profile-quota-exceeded",
                "writable profile snapshot exceeds its declared quota",
            ));
        }
        let next = RuntimeWritableProfile {
            title_fingerprint: manifest.title_fingerprint,
            profile_id: manifest.profile_id,
            root_path_a: root.bytes_with_nul,
            quota_bytes: manifest.quota_bytes,
            files,
            directories,
            path_index,
            directory_path_index,
            root_identity: root.identity,
        };
        validate_source_profile_separation_opt(self.snapshot.as_ref(), &next)?;
        let (expected_catalog_revision, next_catalog_revision) = self.plan_catalog_change()?;
        Ok(RuntimeWritableProfileRegistrationPlan {
            next,
            expected_catalog_revision,
            next_catalog_revision,
        })
    }

    pub(super) fn commit_writable_profile_registration(
        &mut self,
        plan: RuntimeWritableProfileRegistrationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.writable_profile.is_some()
            || !self.open_files.is_empty()
            || !self.pending_reads.is_empty()
            || !self.pending_commits.is_empty()
            || !self.pending_deletes.is_empty()
            || !self.pending_directory_creates.is_empty()
            || !self.searches.is_empty()
            || self.catalog_revision != plan.expected_catalog_revision
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-profile-registration-plan-stale",
                "writable profile state changed after registration planning",
            ));
        }
        validate_source_profile_separation_opt(self.snapshot.as_ref(), &plan.next)?;
        self.writable_profile = Some(plan.next);
        self.catalog_revision = plan.next_catalog_revision;
        Ok(())
    }

    pub(super) fn plan_writable_profile_hydration(
        &self,
        object_id: u32,
        generation: u64,
        bytes: &[u8],
    ) -> Result<RuntimeWritableProfileHydrationPlan, RuntimeStateError> {
        if !self.open_files.is_empty()
            || !self.pending_reads.is_empty()
            || !self.pending_commits.is_empty()
            || !self.pending_deletes.is_empty()
            || !self.pending_directory_creates.is_empty()
            || !self.searches.is_empty()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-profile-hydration-after-use",
                "writable profile bytes must hydrate before file use",
            ));
        }
        let previous = self.writable_file(object_id)?.clone();
        if !previous.exists
            || previous.content_loaded
            || previous.generation != generation
            || u64::try_from(bytes.len()) != Ok(previous.byte_length)
            || previous
                .sha256
                .as_deref()
                .is_none_or(|expected| !expected.eq_ignore_ascii_case(&hex_sha256(bytes)))
        {
            return Err(vfs_invalid(
                "writable-profile-hydration-invalid",
                "writable profile hydration bytes do not match unloaded object metadata",
            ));
        }
        let committed_bytes = copy_writable_content(bytes)?;
        let working_bytes = copy_writable_content(bytes)?;
        let mut next = previous.clone();
        next.content_loaded = true;
        next.committed_bytes = committed_bytes;
        next.working_bytes = working_bytes;
        Ok(RuntimeWritableProfileHydrationPlan { previous, next })
    }

    pub(super) fn commit_writable_profile_hydration(
        &mut self,
        plan: RuntimeWritableProfileHydrationPlan,
    ) -> Result<&RuntimeWritableFile, RuntimeStateError> {
        if !self.open_files.is_empty()
            || !self.pending_reads.is_empty()
            || !self.pending_commits.is_empty()
            || !self.pending_deletes.is_empty()
            || !self.pending_directory_creates.is_empty()
            || !self.searches.is_empty()
            || self
                .writable_profile
                .as_ref()
                .and_then(|profile| profile.files.get(&plan.previous.object_id))
                != Some(&plan.previous)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-profile-hydration-plan-stale",
                "writable profile state changed after hydration planning",
            ));
        }
        let object_id = plan.next.object_id;
        self.writable_profile
            .as_mut()
            .expect("validated writable hydration profile")
            .files
            .insert(object_id, plan.next);
        self.writable_file(object_id)
    }

    #[cfg(test)]
    pub(super) fn plan_open(
        &self,
        requested_path: &[u8],
        current_directory: &[u8],
        desired_access: u32,
        share_mode: u32,
        flags_and_attributes: u32,
    ) -> Result<Option<RuntimeSourceFileOpenPlan>, RuntimeStateError> {
        self.plan_open_with_disposition(
            requested_path,
            current_directory,
            desired_access,
            share_mode,
            3,
            flags_and_attributes,
        )
    }

    pub(super) fn plan_open_with_disposition(
        &self,
        requested_path: &[u8],
        current_directory: &[u8],
        desired_access: u32,
        share_mode: u32,
        creation_disposition: u32,
        flags_and_attributes: u32,
    ) -> Result<Option<RuntimeSourceFileOpenPlan>, RuntimeStateError> {
        let normalized = resolve_path(requested_path, current_directory)?;
        let root_identity = self
            .writable_profile
            .as_ref()
            .map(|profile| profile.root_identity.as_slice())
            .or_else(|| {
                self.snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.root_identity.as_slice())
            })
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-vfs",
                    "file-mount-unregistered",
                    "runtime process has no immutable source or writable profile mount",
                )
            })?;
        let relative = relative_to_mount(&normalized.identity, root_identity)?;
        if self.open_files.len() >= RUNTIME_SOURCE_FILE_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-vfs",
                "open-file-limit-exceeded",
                "logical file count exceeds the runtime bound",
            ));
        }
        let writable = self
            .writable_profile
            .as_ref()
            .and_then(|profile| profile.path_index.get(relative).copied());
        let source = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.path_index.get(relative).copied());
        let (backing, existed, previous_writable, next_writable) = if let Some(object_id) = writable
        {
            let profile = self
                .writable_profile
                .as_ref()
                .expect("profile path requires profile");
            let previous = profile.files.get(&object_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-vfs",
                    "writable-path-index-corrupt",
                    "writable path index points to a missing object",
                )
            })?;
            if self
                .pending_commits
                .values()
                .any(|pending| pending.request.object_id == object_id)
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-vfs",
                    "writable-file-commit-pending",
                    "writable file cannot open while its durable commit is pending",
                ));
            }
            if self
                .pending_deletes
                .values()
                .any(|pending| pending.request.object_id == object_id)
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-vfs",
                    "writable-file-delete-pending",
                    "writable file cannot open while its durable deletion is pending",
                ));
            }
            let existed = previous.exists;
            let mut next = previous.clone();
            match creation_disposition {
                2 => stage_writable_replacement(&mut next)?,
                3 if !existed => return Ok(None),
                3 => {}
                4 if !existed => stage_writable_replacement(&mut next)?,
                4 => {}
                _ => {
                    return Err(vfs_invalid(
                        "writable-open-disposition-invalid",
                        "writable profile open requires CREATE_ALWAYS, OPEN_EXISTING, or OPEN_ALWAYS",
                    ));
                }
            }
            (
                RuntimeFileBacking::WritableProfile { object_id },
                existed,
                Some(previous.clone()),
                Some(next),
            )
        } else if let Some(object_id) = source {
            if creation_disposition != 3 || desired_access & 0x4000_0000 != 0 {
                return Err(RuntimeStateError::conflict(
                    "runtime-vfs",
                    "immutable-source-write-denied",
                    "immutable source files cannot be created, truncated, or opened for write",
                ));
            }
            let snapshot = self
                .snapshot
                .as_ref()
                .expect("source path requires snapshot");
            if !snapshot.files.contains_key(&object_id) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-vfs",
                    "source-path-index-corrupt",
                    "source path index points to a missing object descriptor",
                ));
            }
            (
                RuntimeFileBacking::ImmutableSource {
                    source_id: snapshot.source_id,
                    object_id,
                },
                true,
                None,
                None,
            )
        } else {
            return Ok(None);
        };
        for open in self
            .open_files
            .values()
            .filter(|open| open.backing == backing)
        {
            if file_access_conflicts(
                desired_access,
                share_mode,
                open.desired_access,
                open.share_mode,
            ) {
                return Err(RuntimeStateError::conflict(
                    "runtime-vfs",
                    "source-file-sharing-violation",
                    "logical file access conflicts with an existing share mode",
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
        let (expected_catalog_revision, next_catalog_revision) =
            if previous_writable != next_writable {
                self.plan_catalog_change()?
            } else {
                (self.catalog_revision, self.catalog_revision)
            };
        Ok(Some(RuntimeSourceFileOpenPlan {
            expected_next_file_id: self.next_file_id,
            next_file_id,
            file: RuntimeSourceFile {
                id,
                backing,
                normalized_path_a: normalized.bytes_with_nul,
                cursor: 0,
                desired_access,
                share_mode,
                flags_and_attributes,
            },
            existed,
            previous_writable,
            next_writable,
            expected_catalog_revision,
            next_catalog_revision,
        }))
    }

    pub(super) fn preflight_open(
        &self,
        plan: &RuntimeSourceFileOpenPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.next_file_id != plan.expected_next_file_id
            || self.catalog_revision != plan.expected_catalog_revision
            || self.open_files.contains_key(&plan.file.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-file-open-plan-stale",
                "source file state changed after open planning",
            ));
        }
        match (
            &plan.previous_writable,
            &plan.next_writable,
            plan.file.backing,
        ) {
            (Some(previous), Some(_), RuntimeFileBacking::WritableProfile { object_id })
                if self
                    .writable_profile
                    .as_ref()
                    .and_then(|profile| profile.files.get(&object_id))
                    == Some(previous) => {}
            (
                None,
                None,
                RuntimeFileBacking::ImmutableSource {
                    source_id,
                    object_id,
                },
            ) if self.snapshot.as_ref().is_some_and(|snapshot| {
                snapshot.source_id == source_id && snapshot.files.contains_key(&object_id)
            }) => {}
            _ => {
                return Err(RuntimeStateError::conflict(
                    "runtime-vfs",
                    "source-file-open-plan-stale",
                    "file backing changed after open planning",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn commit_open(&mut self, plan: RuntimeSourceFileOpenPlan) -> RuntimeSourceFile {
        debug_assert!(self.preflight_open(&plan).is_ok());
        self.next_file_id = plan.next_file_id;
        self.catalog_revision = plan.next_catalog_revision;
        if let (Some(_), Some(next)) = (plan.previous_writable, plan.next_writable) {
            let profile = self
                .writable_profile
                .as_mut()
                .expect("validated writable open requires a profile");
            profile.files.insert(next.object_id, next);
        }
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
        if self
            .pending_commits
            .values()
            .any(|pending| pending.request.file_id == id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-file-commit-pending",
                "logical writable file cannot close while a commit is pending",
            ));
        }
        if let RuntimeFileBacking::WritableProfile { object_id } = previous.backing
            && self.writable_file(object_id)?.dirty
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-file-dirty",
                "dirty writable file requires a durable commit before close",
            ));
        }
        Ok(RuntimeSourceFileRetirementPlan {
            previous,
            pending_reads: Vec::new(),
            pending_commits: Vec::new(),
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
        let pending_commits = self
            .pending_commits
            .values()
            .filter(|pending| pending.request.file_id == id)
            .cloned()
            .collect::<Vec<_>>();
        if pending_reads
            .iter()
            .any(|pending| !cancelling_request_ids.contains(&pending.request.request_id))
            || pending_commits
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
            pending_commits,
        })
    }

    pub(super) fn plan_retirement_after_commit(
        &self,
        commit: &RuntimeWritableCommitCompletionPlan,
    ) -> Result<RuntimeSourceFileRetirementPlan, RuntimeStateError> {
        if !commit.closes_file() {
            return Err(vfs_invalid(
                "writable-retirement-without-close",
                "writable commit retirement requires a successful close request",
            ));
        }
        let previous = self.open_file(commit.file_id())?.clone();
        let pending_reads = self
            .pending_reads
            .values()
            .filter(|pending| pending.request.file_id == previous.id)
            .cloned()
            .collect::<Vec<_>>();
        let pending_commits = self
            .pending_commits
            .values()
            .filter(|pending| pending.request.file_id == previous.id)
            .cloned()
            .collect::<Vec<_>>();
        if !pending_reads.is_empty() || pending_commits != [commit.pending.clone()] {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-close-operation-conflict",
                "writable close commit is not the file's only pending operation",
            ));
        }
        Ok(RuntimeSourceFileRetirementPlan {
            previous,
            pending_reads,
            pending_commits,
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
        if let RuntimeFileBacking::WritableProfile { object_id } = plan.previous.backing
            && !self
                .open_files
                .values()
                .any(|file| file.backing == plan.previous.backing)
            && self
                .writable_profile
                .as_ref()
                .and_then(|profile| profile.files.get(&object_id))
                .is_some_and(|file| file.dirty)
        {
            let file = self
                .writable_profile
                .as_mut()
                .and_then(|profile| profile.files.get_mut(&object_id))
                .expect("validated writable retirement object");
            discard_writable_staging(file);
        }
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
        let pending_commits = self
            .pending_commits
            .values()
            .filter(|pending| pending.request.file_id == plan.previous.id)
            .collect::<Vec<_>>();
        if self.open_files.get(&plan.previous.id) != Some(&plan.previous)
            || pending_reads.len() != plan.pending_reads.len()
            || pending_reads
                .iter()
                .zip(&plan.pending_reads)
                .any(|(actual, expected)| *actual != expected)
            || pending_commits.len() != plan.pending_commits.len()
            || pending_commits
                .iter()
                .zip(&plan.pending_commits)
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

    pub(super) fn file_byte_length(
        &self,
        id: RuntimeSourceFileId,
    ) -> Result<u64, RuntimeStateError> {
        match self.open_file(id)?.backing {
            RuntimeFileBacking::ImmutableSource {
                source_id,
                object_id,
            } => self
                .snapshot
                .as_ref()
                .filter(|snapshot| snapshot.source_id == source_id)
                .and_then(|snapshot| snapshot.files.get(&object_id))
                .map(|file| file.byte_length)
                .ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-vfs",
                        "source-file-backing-missing",
                        "open source file backing is missing from its snapshot",
                    )
                }),
            RuntimeFileBacking::WritableProfile { object_id } => {
                Ok(self.writable_file(object_id)?.byte_length)
            }
        }
    }

    pub(super) fn file_read_service(
        &self,
        id: RuntimeSourceFileId,
    ) -> Result<&'static str, RuntimeStateError> {
        match self.open_file(id)?.backing {
            RuntimeFileBacking::ImmutableSource { .. } => Ok(RUNTIME_SOURCE_READ_SERVICE),
            RuntimeFileBacking::WritableProfile { object_id } => {
                if self.writable_file(object_id)?.dirty {
                    return Err(RuntimeStateError::conflict(
                        "runtime-vfs",
                        "writable-read-requires-staged-bytes",
                        "dirty writable file must read its Rust-owned staged bytes",
                    ));
                }
                Ok(RUNTIME_WRITABLE_READ_SERVICE)
            }
        }
    }

    pub(super) fn writable_file_needs_commit(
        &self,
        id: RuntimeSourceFileId,
    ) -> Result<bool, RuntimeStateError> {
        let RuntimeFileBacking::WritableProfile { object_id } = self.open_file(id)?.backing else {
            return Ok(false);
        };
        Ok(self.writable_file(object_id)?.dirty)
    }

    pub(super) fn plan_writable_read(
        &self,
        id: RuntimeSourceFileId,
        requested_bytes: u32,
    ) -> Result<RuntimeWritableReadPlan, RuntimeStateError> {
        let previous_file = self.open_file(id)?.clone();
        let RuntimeFileBacking::WritableProfile { object_id } = previous_file.backing else {
            return Err(vfs_invalid(
                "writable-read-backing-invalid",
                "immediate writable read requires a writable profile file",
            ));
        };
        if previous_file.desired_access & 0x8000_0000 == 0
            || requested_bytes == 0
            || requested_bytes > RUNTIME_WRITABLE_FILE_MAX_IO_BYTES
        {
            return Err(vfs_invalid(
                "writable-read-variant-invalid",
                "writable file read requires bounded nonzero bytes and GENERIC_READ access",
            ));
        }
        self.require_no_pending_file_operation(id, object_id)?;
        let writable = self.writable_file(object_id)?;
        if !writable.content_loaded {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-profile-content-not-loaded",
                "staged writable reads require hydrated profile bytes",
            ));
        }
        let start = usize::try_from(previous_file.cursor).map_err(|_| {
            vfs_invalid(
                "writable-read-cursor-invalid",
                "writable file cursor is not representable as a content offset",
            )
        })?;
        let available = writable.working_bytes.len().saturating_sub(start);
        let transferred = available.min(requested_bytes as usize);
        let bytes = writable.working_bytes[start..start + transferred].to_vec();
        let cursor_after = previous_file
            .cursor
            .checked_add(transferred as u64)
            .ok_or_else(|| {
                vfs_invalid(
                    "writable-read-cursor-invalid",
                    "writable read cursor overflowed",
                )
            })?;
        let mut next_file = previous_file.clone();
        next_file.cursor = cursor_after;
        Ok(RuntimeWritableReadPlan {
            previous_file,
            next_file,
            bytes,
            result: RuntimeWritableReadResult {
                file_id: id,
                object_id,
                cursor_before: start as u64,
                cursor_after,
                transferred_bytes: transferred as u32,
            },
        })
    }

    pub(super) fn preflight_writable_read(
        &self,
        plan: &RuntimeWritableReadPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.open_files.get(&plan.previous_file.id) != Some(&plan.previous_file) {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-read-plan-stale",
                "writable file changed after read planning",
            ));
        }
        let RuntimeFileBacking::WritableProfile { object_id } = plan.previous_file.backing else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "writable-read-plan-backing-invalid",
                "writable read plan lost its profile backing",
            ));
        };
        self.require_no_pending_file_operation(plan.previous_file.id, object_id)
    }

    pub(super) fn commit_writable_read(
        &mut self,
        plan: RuntimeWritableReadPlan,
    ) -> RuntimeWritableReadResult {
        debug_assert!(self.preflight_writable_read(&plan).is_ok());
        self.open_files.insert(plan.next_file.id, plan.next_file);
        plan.result
    }

    pub(super) fn plan_writable_write(
        &self,
        id: RuntimeSourceFileId,
        bytes: &[u8],
    ) -> Result<RuntimeWritableWritePlan, RuntimeStateError> {
        let previous_file = self.open_file(id)?.clone();
        let RuntimeFileBacking::WritableProfile { object_id } = previous_file.backing else {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "immutable-source-write-denied",
                "immutable source files cannot be written",
            ));
        };
        if previous_file.desired_access & 0x4000_0000 == 0
            || bytes.is_empty()
            || bytes.len() > RUNTIME_WRITABLE_FILE_MAX_IO_BYTES as usize
        {
            return Err(vfs_invalid(
                "writable-write-variant-invalid",
                "writable file write requires bounded nonzero bytes and GENERIC_WRITE access",
            ));
        }
        self.require_no_pending_file_operation(id, object_id)?;
        let profile = self.writable_profile.as_ref().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "writable-profile-missing",
                "writable file handle has no profile authority",
            )
        })?;
        let previous_writable = self.writable_file(object_id)?.clone();
        if !previous_writable.content_loaded {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-profile-content-not-loaded",
                "writable file mutation requires hydrated or replaced profile bytes",
            ));
        }
        let write_end = previous_file
            .cursor
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-vfs",
                    "writable-file-size-overflow",
                    "writable file write end overflowed",
                )
            })?;
        if write_end > profile.quota_bytes {
            return Err(RuntimeStateError::resource_limit(
                "runtime-vfs",
                "writable-profile-quota-exceeded",
                "writable file write exceeds the profile quota",
            ));
        }
        let other_bytes = profile
            .files
            .values()
            .filter(|file| file.object_id != object_id && file.exists)
            .try_fold(0_u64, |total, file| total.checked_add(file.byte_length))
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-vfs",
                    "writable-profile-quota-overflow",
                    "writable profile byte accounting overflowed",
                )
            })?;
        let next_length = previous_writable.byte_length.max(write_end);
        if other_bytes
            .checked_add(next_length)
            .is_none_or(|total| total > profile.quota_bytes)
        {
            return Err(RuntimeStateError::resource_limit(
                "runtime-vfs",
                "writable-profile-quota-exceeded",
                "writable profile write set exceeds its declared quota",
            ));
        }
        let mut next_writable = previous_writable.clone();
        let start = previous_file.cursor as usize;
        let end = write_end as usize;
        next_writable.working_bytes.resize(end, 0);
        next_writable.working_bytes[start..end].copy_from_slice(bytes);
        mark_writable_dirty(&mut next_writable)?;
        let mut next_file = previous_file.clone();
        next_file.cursor = write_end;
        let (expected_catalog_revision, next_catalog_revision) = self.plan_catalog_change()?;
        Ok(RuntimeWritableWritePlan {
            previous_file,
            next_file,
            previous_writable,
            next_writable: next_writable.clone(),
            result: RuntimeWritableWriteResult {
                file_id: id,
                object_id,
                cursor_before: start as u64,
                cursor_after: write_end,
                transferred_bytes: bytes.len() as u32,
                byte_length: next_writable.byte_length,
                revision: next_writable.revision,
            },
            expected_catalog_revision,
            next_catalog_revision,
        })
    }

    pub(super) fn preflight_writable_write(
        &self,
        plan: &RuntimeWritableWritePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.open_files.get(&plan.previous_file.id) != Some(&plan.previous_file)
            || self.catalog_revision != plan.expected_catalog_revision
            || self
                .writable_profile
                .as_ref()
                .and_then(|profile| profile.files.get(&plan.previous_writable.object_id))
                != Some(&plan.previous_writable)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-write-plan-stale",
                "writable file or profile content changed after write planning",
            ));
        }
        self.require_no_pending_file_operation(
            plan.previous_file.id,
            plan.previous_writable.object_id,
        )
    }

    pub(super) fn commit_writable_write(
        &mut self,
        plan: RuntimeWritableWritePlan,
    ) -> RuntimeWritableWriteResult {
        debug_assert!(self.preflight_writable_write(&plan).is_ok());
        self.catalog_revision = plan.next_catalog_revision;
        self.open_files.insert(plan.next_file.id, plan.next_file);
        self.writable_profile
            .as_mut()
            .expect("validated writable write profile")
            .files
            .insert(plan.next_writable.object_id, plan.next_writable);
        plan.result
    }

    pub(super) fn plan_writable_commit_begin(
        &self,
        id: RuntimeSourceFileId,
        request_id: u64,
        close_after_commit: bool,
    ) -> Result<RuntimeWritableCommitBeginPlan, RuntimeStateError> {
        let file = self.open_file(id)?;
        let RuntimeFileBacking::WritableProfile { object_id } = file.backing else {
            return Err(vfs_invalid(
                "writable-commit-backing-invalid",
                "durable commit requires a writable profile file",
            ));
        };
        self.require_no_pending_file_operation(id, object_id)?;
        let writable = self.writable_file(object_id)?;
        let base_generation = writable.dirty_base_generation.ok_or_else(|| {
            RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-file-clean",
                "clean writable file has no durable commit to perform",
            )
        })?;
        Ok(RuntimeWritableCommitBeginPlan {
            pending: RuntimePendingWritableCommit {
                request: RuntimeWritableCommitRequest {
                    request_id,
                    file_id: id,
                    object_id,
                    base_generation,
                    byte_length: writable.byte_length,
                    content_sha256: writable.sha256.clone().ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-vfs",
                            "writable-file-dirty-hash-missing",
                            "dirty writable file is missing its content hash",
                        )
                    })?,
                    close_after_commit,
                },
                expected_revision: writable.revision,
            },
        })
    }

    pub(super) fn preflight_writable_commit_begin(
        &self,
        plan: &RuntimeWritableCommitBeginPlan,
    ) -> Result<(), RuntimeStateError> {
        let pending = &plan.pending;
        let file = self.open_file(pending.request.file_id)?;
        let writable = self.writable_file(pending.request.object_id)?;
        if file.backing
            != (RuntimeFileBacking::WritableProfile {
                object_id: pending.request.object_id,
            })
            || writable.revision != pending.expected_revision
            || writable.dirty_base_generation != Some(pending.request.base_generation)
            || self
                .pending_commits
                .contains_key(&pending.request.request_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-commit-plan-stale",
                "writable file changed after commit planning",
            ));
        }
        self.require_no_pending_file_operation(pending.request.file_id, pending.request.object_id)
    }

    pub(super) fn commit_writable_commit_begin(
        &mut self,
        plan: RuntimeWritableCommitBeginPlan,
    ) -> RuntimeWritableCommitRequest {
        debug_assert!(self.preflight_writable_commit_begin(&plan).is_ok());
        let request = plan.pending.request.clone();
        let previous = self
            .pending_commits
            .insert(request.request_id, plan.pending);
        debug_assert!(previous.is_none());
        request
    }

    pub(super) fn writable_commit_bytes(
        &self,
        request_id: u64,
    ) -> Result<&[u8], RuntimeStateError> {
        let pending = self.pending_commits.get(&request_id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-vfs",
                "writable-commit-request-unknown",
                "writable commit payload does not identify a pending request",
            )
        })?;
        let writable = self.writable_file(pending.request.object_id)?;
        if writable.revision != pending.expected_revision
            || writable.byte_length != pending.request.byte_length
            || writable.sha256.as_deref() != Some(&pending.request.content_sha256)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-commit-payload-stale",
                "writable commit bytes changed after the host request began",
            ));
        }
        Ok(&writable.working_bytes)
    }

    pub(super) fn writable_commit_request(
        &self,
        request_id: u64,
    ) -> Result<&RuntimeWritableCommitRequest, RuntimeStateError> {
        self.pending_commits
            .get(&request_id)
            .map(|pending| &pending.request)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-vfs",
                    "writable-commit-request-unknown",
                    "writable commit metadata does not identify a pending request",
                )
            })
    }

    pub(super) fn plan_writable_commit_completion(
        &self,
        response: RuntimeWritableCommitHostResponse,
    ) -> Result<RuntimeWritableCommitCompletionPlan, RuntimeStateError> {
        let pending = self
            .pending_commits
            .get(&response.request_id)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-vfs",
                    "writable-commit-request-unknown",
                    "writable commit completion does not identify a pending request",
                )
            })?
            .clone();
        let previous_writable = self.writable_file(pending.request.object_id)?.clone();
        if response.object_id != pending.request.object_id
            || response.base_generation != pending.request.base_generation
            || previous_writable.revision != pending.expected_revision
            || previous_writable.dirty_base_generation != Some(pending.request.base_generation)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-commit-completion-identity-mismatch",
                "writable commit completion identity or generation is stale",
            ));
        }
        let succeeded = response.win32_error == 0;
        let expected_generation = response.base_generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-vfs",
                "writable-generation-exhausted",
                "writable profile generation space is exhausted",
            )
        })?;
        if (succeeded && response.committed_generation != expected_generation)
            || (!succeeded && response.committed_generation != response.base_generation)
        {
            return Err(vfs_invalid(
                "writable-commit-completion-generation-invalid",
                "writable commit response generation does not match success or failure semantics",
            ));
        }
        let mut next_writable = previous_writable.clone();
        if succeeded {
            next_writable.generation = response.committed_generation;
            next_writable.committed_bytes = next_writable.working_bytes.clone();
            next_writable.dirty = false;
            next_writable.dirty_base_generation = None;
        }
        Ok(RuntimeWritableCommitCompletionPlan {
            pending: pending.clone(),
            previous_writable,
            next_writable,
            completion: RuntimeWritableCommitCompletion {
                request: pending.request,
                succeeded,
                committed_generation: response.committed_generation,
                win32_error: response.win32_error,
            },
        })
    }

    pub(super) fn preflight_writable_commit_completion(
        &self,
        plan: &RuntimeWritableCommitCompletionPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.pending_commits.get(&plan.pending.request.request_id) != Some(&plan.pending)
            || self
                .writable_profile
                .as_ref()
                .and_then(|profile| profile.files.get(&plan.previous_writable.object_id))
                != Some(&plan.previous_writable)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-commit-completion-plan-stale",
                "writable commit state changed after completion planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_writable_commit_completion(
        &mut self,
        plan: RuntimeWritableCommitCompletionPlan,
    ) -> RuntimeWritableCommitCompletion {
        debug_assert!(self.preflight_writable_commit_completion(&plan).is_ok());
        self.pending_commits
            .remove(&plan.pending.request.request_id);
        self.writable_profile
            .as_mut()
            .expect("validated writable completion profile")
            .files
            .insert(plan.next_writable.object_id, plan.next_writable);
        plan.completion
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
        let (source_id, object_id) = match file.backing {
            RuntimeFileBacking::ImmutableSource {
                source_id,
                object_id,
            } => (source_id, object_id),
            RuntimeFileBacking::WritableProfile { object_id }
                if !self.writable_file(object_id)?.dirty =>
            {
                (0, object_id)
            }
            RuntimeFileBacking::WritableProfile { .. } => {
                return Err(RuntimeStateError::conflict(
                    "runtime-vfs",
                    "writable-read-requires-staged-bytes",
                    "dirty writable file cannot read stale committed host bytes",
                ));
            }
        };
        if file.desired_access & 0x8000_0000 == 0 {
            return Err(vfs_invalid(
                "source-read-access-denied",
                "file was not opened with GENERIC_READ access",
            ));
        }
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
                    source_id,
                    object_id,
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
        let byte_length = self.file_byte_length(request.file_id)?;
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
                    .is_none_or(|end| end > byte_length)
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
        if succeeded
            && let RuntimeFileBacking::WritableProfile { object_id } = previous_file.backing
        {
            let writable = self.writable_file(object_id)?;
            let start = usize::try_from(response.offset).map_err(|_| {
                vfs_invalid(
                    "writable-read-completion-range-invalid",
                    "writable read completion offset is not representable",
                )
            })?;
            let end = start + response.bytes.len();
            if writable.content_loaded
                && writable.working_bytes.get(start..end) != Some(response.bytes.as_slice())
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-vfs",
                    "writable-read-completion-content-mismatch",
                    "profile read host bytes differ from the registered committed generation",
                ));
            }
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

    pub(super) fn plan_writable_commit_cancellation(
        &self,
        request_ids: &[u64],
    ) -> RuntimeWritableCommitCancellationPlan {
        let selected = request_ids.iter().copied().collect::<BTreeSet<_>>();
        RuntimeWritableCommitCancellationPlan {
            requests: self
                .pending_commits
                .iter()
                .filter_map(|(request_id, pending)| {
                    selected.contains(request_id).then_some(pending.clone())
                })
                .collect(),
        }
    }

    pub(super) fn commit_writable_commit_cancellation(
        &mut self,
        plan: RuntimeWritableCommitCancellationPlan,
    ) {
        for pending in plan.requests {
            let removed = self.pending_commits.remove(&pending.request.request_id);
            debug_assert_eq!(removed, Some(pending));
        }
    }

    pub(super) fn plan_writable_delete_cancellation(
        &self,
        request_ids: &[u64],
    ) -> RuntimeWritableDeleteCancellationPlan {
        let selected = request_ids.iter().copied().collect::<BTreeSet<_>>();
        RuntimeWritableDeleteCancellationPlan {
            requests: self
                .pending_deletes
                .iter()
                .filter_map(|(request_id, pending)| {
                    selected.contains(request_id).then_some(pending.clone())
                })
                .collect(),
        }
    }

    pub(super) fn commit_writable_delete_cancellation(
        &mut self,
        plan: RuntimeWritableDeleteCancellationPlan,
    ) {
        for pending in plan.requests {
            let removed = self.pending_deletes.remove(&pending.request.request_id);
            debug_assert_eq!(removed, Some(pending));
        }
    }

    pub(super) fn plan_writable_directory_create_cancellation(
        &self,
        request_ids: &[u64],
    ) -> RuntimeWritableDirectoryCreateCancellationPlan {
        let selected = request_ids.iter().copied().collect::<BTreeSet<_>>();
        RuntimeWritableDirectoryCreateCancellationPlan {
            requests: self
                .pending_directory_creates
                .iter()
                .filter_map(|(request_id, pending)| {
                    selected.contains(request_id).then_some(pending.clone())
                })
                .collect(),
        }
    }

    pub(super) fn commit_writable_directory_create_cancellation(
        &mut self,
        plan: RuntimeWritableDirectoryCreateCancellationPlan,
    ) {
        for pending in plan.requests {
            let removed = self
                .pending_directory_creates
                .remove(&pending.request.request_id);
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
        let object_id = match file.backing {
            RuntimeFileBacking::ImmutableSource { object_id, .. }
            | RuntimeFileBacking::WritableProfile { object_id } => object_id,
        };
        self.require_no_pending_file_operation(id, object_id)?;
        let byte_length = self.file_byte_length(id)?;
        let base = match origin {
            0 => 0i128,
            1 => i128::from(file.cursor),
            2 => i128::from(byte_length),
            _ => {
                return Err(vfs_invalid(
                    "source-seek-origin-invalid",
                    "source file seek origin is outside FILE_BEGIN/CURRENT/END",
                ));
            }
        };
        let cursor = base + i128::from(distance);
        let maximum = match file.backing {
            RuntimeFileBacking::ImmutableSource { .. } => i128::from(byte_length),
            RuntimeFileBacking::WritableProfile { .. } => i128::from(
                self.writable_profile
                    .as_ref()
                    .expect("writable file requires a profile")
                    .quota_bytes,
            ),
        };
        if cursor < 0 || cursor > maximum {
            return Err(vfs_invalid(
                "source-seek-range-invalid",
                "file seek result is negative or exceeds its backing bound",
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

    fn writable_file(&self, object_id: u32) -> Result<&RuntimeWritableFile, RuntimeStateError> {
        self.writable_profile
            .as_ref()
            .and_then(|profile| profile.files.get(&object_id))
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-vfs",
                    "writable-file-backing-missing",
                    "writable profile object identity is missing",
                )
            })
    }

    fn writable_directory(
        &self,
        directory_id: u32,
    ) -> Result<&RuntimeWritableDirectory, RuntimeStateError> {
        self.writable_profile
            .as_ref()
            .and_then(|profile| profile.directories.get(&directory_id))
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-vfs",
                    "writable-directory-identity-missing",
                    "writable profile directory identity is missing",
                )
            })
    }

    fn require_no_pending_file_operation(
        &self,
        id: RuntimeSourceFileId,
        object_id: u32,
    ) -> Result<(), RuntimeStateError> {
        if self
            .pending_reads
            .values()
            .any(|pending| pending.request.file_id == id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-file-read-pending",
                "logical file already owns a pending read",
            ));
        }
        if matches!(
            self.open_file(id)?.backing,
            RuntimeFileBacking::WritableProfile { .. }
        ) && self
            .pending_commits
            .values()
            .any(|pending| pending.request.object_id == object_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "writable-file-commit-pending",
                "writable file already owns a pending durable commit",
            ));
        }
        Ok(())
    }
}

fn validate_source_profile_separation(
    source: &RuntimeSourceSnapshot,
    profile: Option<&RuntimeWritableProfile>,
) -> Result<(), RuntimeStateError> {
    let Some(profile) = profile else {
        return Ok(());
    };
    validate_source_profile_separation_opt(Some(source), profile)
}

fn validate_source_profile_separation_opt(
    source: Option<&RuntimeSourceSnapshot>,
    profile: &RuntimeWritableProfile,
) -> Result<(), RuntimeStateError> {
    let Some(source) = source else {
        return Ok(());
    };
    if source.title_fingerprint != profile.title_fingerprint
        || source.root_identity != profile.root_identity
    {
        return Err(vfs_invalid(
            "source-profile-identity-mismatch",
            "immutable source and writable profile require the same title and mount root",
        ));
    }
    if source.path_index.keys().any(|path| {
        profile.path_index.contains_key(path)
            || profile.directory_path_index.contains_key(path)
            || profile.directory_path_index.keys().any(|directory| {
                directory.len() > path.len()
                    && directory.starts_with(path)
                    && directory.get(path.len()) == Some(&b'\\')
            })
    }) {
        return Err(vfs_invalid(
            "source-profile-path-overlap",
            "writable profile paths may not shadow or descend from immutable source objects",
        ));
    }
    Ok(())
}

fn file_access_conflicts(
    desired_access: u32,
    share_mode: u32,
    open_access: u32,
    open_share_mode: u32,
) -> bool {
    let requested_read = desired_access & 0x8000_0000 != 0;
    let requested_write = desired_access & 0x4000_0000 != 0;
    let open_read = open_access & 0x8000_0000 != 0;
    let open_write = open_access & 0x4000_0000 != 0;
    (requested_read && open_share_mode & 1 == 0)
        || (requested_write && open_share_mode & 2 == 0)
        || (open_read && share_mode & 1 == 0)
        || (open_write && share_mode & 2 == 0)
}

fn stage_writable_replacement(file: &mut RuntimeWritableFile) -> Result<(), RuntimeStateError> {
    file.content_loaded = true;
    file.working_bytes.clear();
    mark_writable_dirty(file)
}

fn mark_writable_dirty(file: &mut RuntimeWritableFile) -> Result<(), RuntimeStateError> {
    if file.dirty_base_generation.is_none() {
        file.dirty_base_generation = Some(file.generation);
    }
    file.revision = file.revision.checked_add(1).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-vfs",
            "writable-file-revision-exhausted",
            "writable file revision space is exhausted",
        )
    })?;
    file.exists = true;
    file.byte_length = file.working_bytes.len() as u64;
    file.sha256 = Some(hex_sha256(&file.working_bytes));
    file.dirty = true;
    Ok(())
}

fn discard_writable_staging(file: &mut RuntimeWritableFile) {
    file.working_bytes = file.committed_bytes.clone();
    file.exists = file.generation > 0;
    file.byte_length = file.committed_bytes.len() as u64;
    file.sha256 = file.exists.then(|| hex_sha256(&file.committed_bytes));
    file.dirty = false;
    file.dirty_base_generation = None;
    file.revision = file.revision.saturating_add(1);
}

fn copy_writable_content(bytes: &[u8]) -> Result<Vec<u8>, RuntimeStateError> {
    let mut copied = Vec::new();
    copied.try_reserve_exact(bytes.len()).map_err(|_| {
        RuntimeStateError::resource_limit(
            "runtime-vfs",
            "writable-profile-hydration-capacity-exhausted",
            "writable profile hydration could not reserve bounded content storage",
        )
    })?;
    copied.extend_from_slice(bytes);
    Ok(copied)
}

#[derive(Debug)]
struct NormalizedPath {
    bytes_with_nul: Vec<u8>,
    identity: Vec<u8>,
}

#[derive(Debug)]
struct ResolvedFileSearchPattern {
    directory_identity: Vec<u8>,
    file_name_pattern: Vec<u8>,
    normalized_pattern_a: Vec<u8>,
}

fn resolve_file_search_pattern(
    requested: &[u8],
    current_directory: &[u8],
) -> Result<ResolvedFileSearchPattern, RuntimeStateError> {
    let body = nul_terminated_body(requested, "file-search-pattern-shape-invalid")?;
    let separators = windows_932_separator_offsets(body)?;
    let (directory, file_name_pattern) = if let Some(separator) = separators.last().copied() {
        let directory_end = if separator == 2 && is_absolute(body) {
            separator + 1
        } else {
            separator
        };
        if directory_end == 0 || separator + 1 == body.len() {
            return Err(vfs_invalid(
                "file-search-pattern-shape-invalid",
                "file search requires a directory and nonempty final pattern component",
            ));
        }
        let mut directory = body[..directory_end].to_vec();
        directory.push(0);
        (
            resolve_path(&directory, current_directory)?,
            body[separator + 1..].to_vec(),
        )
    } else {
        (normalize_absolute_path(current_directory)?, body.to_vec())
    };
    validate_file_search_component(&file_name_pattern)?;
    if !file_name_pattern
        .iter()
        .any(|byte| matches!(byte, b'*' | b'?'))
    {
        validate_component(&file_name_pattern)?;
    }
    let mut normalized_pattern_a =
        directory.bytes_with_nul[..directory.bytes_with_nul.len() - 1].to_vec();
    if normalized_pattern_a.last() != Some(&b'\\') {
        normalized_pattern_a.push(b'\\');
    }
    normalized_pattern_a.extend_from_slice(&file_name_pattern);
    normalized_pattern_a.push(0);
    Ok(ResolvedFileSearchPattern {
        directory_identity: directory.identity,
        file_name_pattern,
        normalized_pattern_a,
    })
}

fn validate_file_search_component(component: &[u8]) -> Result<(), RuntimeStateError> {
    if component.is_empty() || component.len() > 260 {
        return Err(vfs_invalid(
            "file-search-pattern-component-invalid",
            "file search final component must be nonempty and at most 260 bytes",
        ));
    }
    let mut index = 0;
    while index < component.len() {
        let byte = component[index];
        if is_windows_932_lead_byte(byte) {
            let Some(&trail) = component.get(index + 1) else {
                return Err(vfs_invalid(
                    "file-search-pattern-windows-932-invalid",
                    "file search pattern ends in a Windows-932 lead byte",
                ));
            };
            if !is_windows_932_trail_byte(trail) {
                return Err(vfs_invalid(
                    "file-search-pattern-windows-932-invalid",
                    "file search pattern contains an invalid Windows-932 trail byte",
                ));
            }
            index += 2;
            continue;
        }
        if byte < 0x20
            || byte == 0x7f
            || matches!(byte, b'"' | b'<' | b'>' | b'|' | b':' | b'\\' | b'/')
        {
            return Err(vfs_invalid(
                "file-search-pattern-character-invalid",
                "file search pattern contains a disallowed DOS path character",
            ));
        }
        index += 1;
    }
    Ok(())
}

fn windows_932_separator_offsets(bytes: &[u8]) -> Result<Vec<usize>, RuntimeStateError> {
    let mut separators = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if is_windows_932_lead_byte(byte) {
            let Some(&trail) = bytes.get(index + 1) else {
                return Err(vfs_invalid(
                    "file-search-pattern-windows-932-invalid",
                    "file search path ends in a Windows-932 lead byte",
                ));
            };
            if !is_windows_932_trail_byte(trail) {
                return Err(vfs_invalid(
                    "file-search-pattern-windows-932-invalid",
                    "file search path contains an invalid Windows-932 trail byte",
                ));
            }
            index += 2;
        } else {
            if is_separator(byte) {
                separators.push(index);
            }
            index += 1;
        }
    }
    Ok(separators)
}

fn immediate_search_child(
    relative_path: &[u8],
    directory_relative: &[u8],
    terminal_kind: RuntimeFileSearchEntryKind,
) -> Option<(Vec<u8>, RuntimeFileSearchEntryKind)> {
    let identity = windows_932_ascii_fold(relative_path);
    let remainder = if directory_relative.is_empty() {
        relative_path
    } else {
        if identity.len() <= directory_relative.len()
            || !identity.starts_with(directory_relative)
            || identity.get(directory_relative.len()) != Some(&b'\\')
        {
            return None;
        }
        &relative_path[directory_relative.len() + 1..]
    };
    if remainder.is_empty() {
        return None;
    }
    let separator = windows_932_separator_offsets(remainder)
        .ok()?
        .first()
        .copied();
    match separator {
        Some(index) => Some((
            remainder[..index].to_vec(),
            RuntimeFileSearchEntryKind::Directory,
        )),
        None => Some((remainder.to_vec(), terminal_kind)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Windows932WildcardToken<'a> {
    Literal(&'a [u8]),
    AnyOne,
    AnyMany,
}

fn windows_932_wildcard_matches(
    pattern: &[u8],
    candidate: &[u8],
) -> Result<bool, RuntimeStateError> {
    let pattern_tokens = windows_932_wildcard_tokens(pattern, true)?;
    let candidate_tokens = windows_932_wildcard_tokens(candidate, false)?;
    let mut reachable = vec![false; candidate_tokens.len() + 1];
    reachable[0] = true;
    for token in pattern_tokens {
        let mut next = vec![false; candidate_tokens.len() + 1];
        match token {
            Windows932WildcardToken::AnyMany => {
                let mut seen = false;
                for index in 0..=candidate_tokens.len() {
                    seen |= reachable[index];
                    next[index] = seen;
                }
            }
            Windows932WildcardToken::AnyOne => {
                for index in 0..candidate_tokens.len() {
                    next[index + 1] |= reachable[index];
                }
            }
            Windows932WildcardToken::Literal(expected) => {
                for (index, actual) in candidate_tokens.iter().enumerate() {
                    let Windows932WildcardToken::Literal(actual) = actual else {
                        unreachable!("candidate wildcard tokenizer emits literals only")
                    };
                    next[index + 1] |= reachable[index]
                        && windows_932_token_eq_ignore_ascii_case(expected, actual);
                }
            }
        }
        reachable = next;
    }
    Ok(reachable[candidate_tokens.len()])
}

fn windows_932_wildcard_tokens(
    bytes: &[u8],
    allow_wildcards: bool,
) -> Result<Vec<Windows932WildcardToken<'_>>, RuntimeStateError> {
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if is_windows_932_lead_byte(byte) {
            let Some(&trail) = bytes.get(index + 1) else {
                return Err(vfs_invalid(
                    "file-search-windows-932-invalid",
                    "file search text ends in a Windows-932 lead byte",
                ));
            };
            if !is_windows_932_trail_byte(trail) {
                return Err(vfs_invalid(
                    "file-search-windows-932-invalid",
                    "file search text contains an invalid Windows-932 trail byte",
                ));
            }
            tokens.push(Windows932WildcardToken::Literal(&bytes[index..index + 2]));
            index += 2;
        } else {
            tokens.push(if allow_wildcards && byte == b'*' {
                Windows932WildcardToken::AnyMany
            } else if allow_wildcards && byte == b'?' {
                Windows932WildcardToken::AnyOne
            } else {
                Windows932WildcardToken::Literal(&bytes[index..index + 1])
            });
            index += 1;
        }
    }
    Ok(tokens)
}

fn windows_932_token_eq_ignore_ascii_case(left: &[u8], right: &[u8]) -> bool {
    match (left, right) {
        ([left], [right]) => left.eq_ignore_ascii_case(right),
        _ => left == right,
    }
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

    fn source_read_response_control(
        requested_bytes: u32,
        transferred_bytes: u32,
        win32_error: u32,
    ) -> [u8; RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES] {
        let words = [
            RUNTIME_SOURCE_READ_TRANSPORT_VERSION,
            7,
            11,
            3,
            0,
            requested_bytes,
            transferred_bytes,
            win32_error,
        ];
        let mut control = [0; RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES];
        for (word, bytes) in words.into_iter().zip(control.chunks_exact_mut(4)) {
            bytes.copy_from_slice(&word.to_le_bytes());
        }
        control
    }

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

    fn writable_manifest(exists: bool, bytes: &[u8]) -> RuntimeWritableProfileManifest {
        RuntimeWritableProfileManifest {
            schema: RUNTIME_WRITABLE_PROFILE_SCHEMA.to_owned(),
            schema_version: RUNTIME_WRITABLE_PROFILE_VERSION,
            title_fingerprint: "sha256:test-title".to_owned(),
            profile_id: "default".to_owned(),
            root_path_a: b"C:\\Vine\0".to_vec(),
            quota_bytes: 1024,
            files: vec![RuntimeWritableFileManifest {
                object_id: 21,
                relative_path_a: b"BGI.gdb".to_vec(),
                exists,
                generation: u64::from(exists),
                byte_length: if exists { bytes.len() as u64 } else { 0 },
                sha256: exists.then(|| hex_sha256(bytes)),
            }],
            directories: Vec::new(),
        }
    }

    fn writable_directory_manifest(
        relative_path_a: &[u8],
        exists: bool,
    ) -> RuntimeWritableProfileManifest {
        let mut manifest = writable_manifest(false, &[]);
        manifest.directories.push(RuntimeWritableDirectoryManifest {
            directory_id: 31,
            relative_path_a: relative_path_a.to_vec(),
            exists,
            generation: u64::from(exists),
        });
        manifest
    }

    #[test]
    fn writable_directory_create_is_failure_atomic_and_generation_checked() {
        let mut files = RuntimeFiles::default();
        let registration = files
            .plan_writable_profile_registration(writable_directory_manifest(b"UserData", false))
            .unwrap();
        files
            .commit_writable_profile_registration(registration)
            .unwrap();
        assert_eq!(
            files
                .resolve_path_kind(b"C:\\Vine\\UserData\0", b"C:\\Vine\0")
                .unwrap(),
            RuntimePathResolution::MissingFile
        );

        let failed_begin = files
            .plan_writable_directory_create(b"UserData\0", b"C:\\Vine\0", 7)
            .unwrap();
        files
            .preflight_writable_directory_create(&failed_begin)
            .unwrap();
        let failed_request = files.commit_writable_directory_create_begin(failed_begin);
        assert_eq!(failed_request.directory_id, 31);
        assert_eq!(failed_request.relative_path_a, b"UserData");
        assert_eq!(failed_request.base_generation, 0);
        let failed = files
            .plan_writable_directory_create_completion(RuntimeWritableDirectoryCreateHostResponse {
                request_id: 7,
                directory_id: 31,
                base_generation: 0,
                committed_generation: 0,
                win32_error: 29,
            })
            .unwrap();
        let failed = files.commit_writable_directory_create_completion(failed);
        assert!(!failed.succeeded);
        assert_eq!(failed.win32_error, 29);
        let directory = &files.writable_profile().unwrap().directories[&31];
        assert!(!directory.exists);
        assert_eq!(directory.generation, 0);
        assert_eq!(directory.revision, 0);

        let begin = files
            .plan_writable_directory_create(b"c:\\vine\\userdata\0", b"C:\\Vine\0", 8)
            .unwrap();
        let request = files.commit_writable_directory_create_begin(begin);
        assert_eq!(request.base_generation, 0);
        let invalid_before = files.clone();
        let invalid = files
            .plan_writable_directory_create_completion(RuntimeWritableDirectoryCreateHostResponse {
                request_id: 8,
                directory_id: 31,
                base_generation: 0,
                committed_generation: 2,
                win32_error: 0,
            })
            .unwrap_err();
        assert_eq!(
            invalid.code,
            "writable-directory-create-completion-generation-invalid"
        );
        assert_eq!(files, invalid_before);

        let completion = files
            .plan_writable_directory_create_completion(RuntimeWritableDirectoryCreateHostResponse {
                request_id: 8,
                directory_id: 31,
                base_generation: 0,
                committed_generation: 1,
                win32_error: 0,
            })
            .unwrap();
        let stale_completion = completion.clone();
        let completion = files.commit_writable_directory_create_completion(completion);
        assert!(completion.succeeded);
        assert_eq!(completion.committed_generation, 1);
        assert_eq!(
            files
                .preflight_writable_directory_create_completion(&stale_completion)
                .unwrap_err()
                .code,
            "writable-directory-create-completion-plan-stale"
        );
        assert_eq!(
            files
                .resolve_path_kind(b"C:\\Vine\\UserData\0", b"C:\\Vine\0")
                .unwrap(),
            RuntimePathResolution::Directory
        );
        assert_eq!(
            files
                .plan_writable_directory_create(b"UserData\0", b"C:\\Vine\0", 9)
                .unwrap_err()
                .code,
            "writable-directory-already-exists"
        );
    }

    #[test]
    fn writable_directory_create_rejects_unregistered_parent_and_cancels_pending_request() {
        let mut files = RuntimeFiles::default();
        let registration = files
            .plan_writable_profile_registration(writable_directory_manifest(
                b"UserData\\Save",
                false,
            ))
            .unwrap();
        files
            .commit_writable_profile_registration(registration)
            .unwrap();
        assert_eq!(
            files
                .plan_writable_directory_create(b"UserData\\Save\0", b"C:\\Vine\0", 10,)
                .unwrap_err()
                .code,
            "writable-directory-parent-missing"
        );
        assert_eq!(
            files
                .plan_writable_directory_create(b"Other\0", b"C:\\Vine\0", 11)
                .unwrap_err()
                .code,
            "writable-directory-unregistered"
        );
        assert_eq!(
            files
                .plan_writable_directory_create(b"BGI.gdb\0", b"C:\\Vine\0", 12)
                .unwrap_err()
                .code,
            "writable-directory-file-conflict"
        );

        let mut cancellable = RuntimeFiles::default();
        let registration = cancellable
            .plan_writable_profile_registration(writable_directory_manifest(b"UserData", false))
            .unwrap();
        cancellable
            .commit_writable_profile_registration(registration)
            .unwrap();
        let begin = cancellable
            .plan_writable_directory_create(b"UserData\0", b"C:\\Vine\0", 13)
            .unwrap();
        cancellable.commit_writable_directory_create_begin(begin);
        let cancellation = cancellable.plan_writable_directory_create_cancellation(&[13]);
        cancellable.commit_writable_directory_create_cancellation(cancellation);
        assert_eq!(
            cancellable
                .writable_directory_create_request(13)
                .unwrap_err()
                .code,
            "writable-directory-create-request-unknown"
        );
        assert!(!cancellable.writable_profile().unwrap().directories[&31].exists);
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
        assert_eq!(
            opened.file().backing,
            RuntimeFileBacking::ImmutableSource {
                source_id: 7,
                object_id: 11,
            }
        );
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
    fn path_resolution_derives_registered_directories_without_mutation() {
        let mut files = RuntimeFiles::default();
        let source_registration = files.plan_snapshot_registration(manifest()).unwrap();
        files
            .commit_snapshot_registration(source_registration)
            .unwrap();
        let writable_registration = files
            .plan_writable_profile_registration(writable_manifest(false, &[]))
            .unwrap();
        files
            .commit_writable_profile_registration(writable_registration)
            .unwrap();
        let before = files.clone();

        for (path, expected) in [
            (b"C:\\Vine\0".as_slice(), RuntimePathResolution::Directory),
            (b"data\0".as_slice(), RuntimePathResolution::Directory),
            (
                b"DATA\\bgi.GDB\0".as_slice(),
                RuntimePathResolution::RegularFile,
            ),
            (
                b"Data\\missing.bin\0".as_slice(),
                RuntimePathResolution::MissingFile,
            ),
            (
                b"Missing\\file.bin\0".as_slice(),
                RuntimePathResolution::MissingPath,
            ),
            (b"BGI.gdb\0".as_slice(), RuntimePathResolution::MissingFile),
            (
                b"D:\\Vine\\Data\\BGI.gdb\0".as_slice(),
                RuntimePathResolution::MissingPath,
            ),
        ] {
            assert_eq!(
                files.resolve_path_kind(path, b"C:\\Vine\0").unwrap(),
                expected,
                "path {}",
                String::from_utf8_lossy(path)
            );
        }
        assert_eq!(files, before);

        let create = files
            .plan_open_with_disposition(b"BGI.gdb\0", b"C:\\Vine\0", 0x4000_0000, 0, 4, 0x0800_0020)
            .unwrap()
            .unwrap();
        files.commit_open(create);
        assert_eq!(
            files
                .resolve_path_kind(b"bgi.GDB\0", b"C:\\Vine\0")
                .unwrap(),
            RuntimePathResolution::RegularFile
        );
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

    #[test]
    fn source_read_response_decoder_bounds_payload_before_copying() {
        let success_control = source_read_response_control(4, 3, 0);
        let success =
            RuntimeSourceReadHostResponse::decode_binary(9, &success_control, &[1, 2, 3]).unwrap();
        assert_eq!(success.request_id, 9);
        assert_eq!(success.offset, 3);
        assert_eq!(success.bytes, [1, 2, 3]);

        let failure_control = source_read_response_control(4, 0, 5);
        let failure =
            RuntimeSourceReadHostResponse::decode_binary(10, &failure_control, &[]).unwrap();
        assert_eq!(failure.win32_error, 5);
        assert!(failure.bytes.is_empty());

        for (control, payload, expected_code) in [
            (
                source_read_response_control(0, 0, 0),
                Vec::new(),
                "source-read-response-request-size-invalid",
            ),
            (
                source_read_response_control(RUNTIME_SOURCE_READ_MAX_BYTES + 1, 0, 0),
                Vec::new(),
                "source-read-response-request-size-invalid",
            ),
            (
                source_read_response_control(4, 5, 0),
                vec![0; 5],
                "source-read-response-payload-invalid",
            ),
            (
                source_read_response_control(4, 3, 0),
                vec![0; 4],
                "source-read-response-payload-invalid",
            ),
            (
                source_read_response_control(
                    RUNTIME_SOURCE_READ_MAX_BYTES,
                    RUNTIME_SOURCE_READ_MAX_BYTES,
                    0,
                ),
                vec![0; RUNTIME_SOURCE_READ_MAX_BYTES as usize + 1],
                "source-read-response-payload-invalid",
            ),
            (
                source_read_response_control(4, 1, 5),
                vec![0],
                "source-read-response-payload-invalid",
            ),
        ] {
            let error =
                RuntimeSourceReadHostResponse::decode_binary(11, &control, &payload).unwrap_err();
            assert_eq!(error.code, expected_code);
        }
    }

    #[test]
    fn writable_profile_missing_create_commit_and_reopen_is_generation_atomic() {
        let mut files = RuntimeFiles::default();
        let registration = files
            .plan_writable_profile_registration(writable_manifest(false, &[]))
            .unwrap();
        files
            .commit_writable_profile_registration(registration)
            .unwrap();
        assert!(
            files
                .plan_open_with_disposition(
                    b"C:\\Vine\\BGI.gdb\0",
                    b"C:\\Vine\0",
                    0x8000_0000,
                    1,
                    3,
                    0x0800_0020,
                )
                .unwrap()
                .is_none()
        );

        let open = files
            .plan_open_with_disposition(
                b"C:\\Vine\\BGI.gdb\0",
                b"C:\\Vine\0",
                0x4000_0000,
                0,
                4,
                0x0800_0020,
            )
            .unwrap()
            .unwrap();
        assert!(!open.existed());
        let id = open.file_id();
        files.commit_open(open);
        let write = files.plan_writable_write(id, b"SDC FORMAT 1.00").unwrap();
        let stale_write = write.clone();
        let result = files.commit_writable_write(write);
        assert_eq!(result.transferred_bytes, 15);
        assert_eq!(
            files
                .preflight_writable_write(&stale_write)
                .unwrap_err()
                .code,
            "writable-write-plan-stale"
        );

        let begin = files.plan_writable_commit_begin(id, 9, true).unwrap();
        let request = files.commit_writable_commit_begin(begin);
        assert_eq!(request.base_generation, 0);
        assert_eq!(files.writable_commit_bytes(9).unwrap(), b"SDC FORMAT 1.00");
        let before_wrong = files.clone();
        let wrong = files
            .plan_writable_commit_completion(RuntimeWritableCommitHostResponse {
                request_id: 9,
                object_id: 21,
                base_generation: 0,
                committed_generation: 2,
                win32_error: 0,
            })
            .unwrap_err();
        assert_eq!(wrong.code, "writable-commit-completion-generation-invalid");
        assert_eq!(files, before_wrong);

        let completion = files
            .plan_writable_commit_completion(RuntimeWritableCommitHostResponse {
                request_id: 9,
                object_id: 21,
                base_generation: 0,
                committed_generation: 1,
                win32_error: 0,
            })
            .unwrap();
        let retirement = files.plan_retirement_after_commit(&completion).unwrap();
        let completion = files.commit_writable_commit_completion(completion);
        assert!(completion.succeeded);
        assert_eq!(completion.committed_generation, 1);
        files.commit_retirement(retirement);
        assert!(files.open_file(id).is_err());
        let committed = &files.writable_profile().unwrap().files[&21];
        assert_eq!(committed.generation, 1);
        assert_eq!(
            committed.sha256.as_deref().unwrap(),
            hex_sha256(b"SDC FORMAT 1.00")
        );
        assert!(!committed.dirty);

        let reopened = files
            .plan_open_with_disposition(b"bgi.GDB\0", b"C:\\Vine\0", 0x8000_0000, 1, 3, 0x0800_0020)
            .unwrap()
            .unwrap();
        assert!(reopened.existed());
        let reopened_id = reopened.file_id();
        files.commit_open(reopened);
        let read = files.plan_writable_read(reopened_id, 32).unwrap();
        assert_eq!(read.bytes(), b"SDC FORMAT 1.00");
        assert_eq!(read.result().transferred_bytes, 15);
    }

    #[test]
    fn writable_profile_metadata_bootstrap_hydrates_large_content_atomically() {
        let content = vec![0xff; 4 * 1024 * 1024];
        let manifest = RuntimeWritableProfileManifest {
            schema: RUNTIME_WRITABLE_PROFILE_SCHEMA.to_owned(),
            schema_version: RUNTIME_WRITABLE_PROFILE_VERSION,
            title_fingerprint: "sha256:test-title".to_owned(),
            profile_id: "default".to_owned(),
            root_path_a: b"C:\\Vine\0".to_vec(),
            quota_bytes: RUNTIME_WRITABLE_PROFILE_MAX_BYTES,
            files: vec![RuntimeWritableFileManifest {
                object_id: 21,
                relative_path_a: b"BGI.gdb".to_vec(),
                exists: true,
                generation: 7,
                byte_length: content.len() as u64,
                sha256: Some(hex_sha256(&content)),
            }],
            directories: Vec::new(),
        };
        let bootstrap = serde_json::to_vec(&manifest).unwrap();
        assert!(bootstrap.len() < 4096);
        assert!(
            !bootstrap
                .windows(b"\"bytes\"".len())
                .any(|part| part == b"\"bytes\"")
        );

        let mut files = RuntimeFiles::default();
        let registration = files.plan_writable_profile_registration(manifest).unwrap();
        files
            .commit_writable_profile_registration(registration)
            .unwrap();
        assert!(!files.writable_profile().unwrap().files[&21].content_loaded);

        let before_invalid = files.clone();
        let invalid = files
            .plan_writable_profile_hydration(21, 7, b"wrong")
            .unwrap_err();
        assert_eq!(invalid.code, "writable-profile-hydration-invalid");
        assert_eq!(files, before_invalid);

        let hydration = files
            .plan_writable_profile_hydration(21, 7, &content)
            .unwrap();
        files.commit_writable_profile_hydration(hydration).unwrap();
        let hydrated = &files.writable_profile().unwrap().files[&21];
        assert!(hydrated.content_loaded);
        assert_eq!(hydrated.committed_bytes, content);
        assert_eq!(hydrated.working_bytes, content);
    }

    #[test]
    fn writable_profile_rejects_shadow_quota_and_discards_uncommitted_exit_bytes() {
        let mut files = RuntimeFiles::default();
        let registration = files
            .plan_writable_profile_registration(writable_manifest(true, b"old"))
            .unwrap();
        files
            .commit_writable_profile_registration(registration)
            .unwrap();
        let hydration = files
            .plan_writable_profile_hydration(21, 1, b"old")
            .unwrap();
        files.commit_writable_profile_hydration(hydration).unwrap();
        let overlapping = files.plan_snapshot_registration(RuntimeSourceSnapshotManifest {
            files: vec![RuntimeSourceFileManifest {
                object_id: 11,
                relative_path_a: b"BGI.gdb".to_vec(),
                byte_length: 3,
                sha256: hex_sha256(b"old"),
            }],
            ..manifest()
        });
        assert_eq!(overlapping.unwrap_err().code, "source-profile-path-overlap");

        let open = files
            .plan_open_with_disposition(
                b"C:\\Vine\\BGI.gdb\0",
                b"C:\\Vine\0",
                0x4000_0000,
                0,
                4,
                0x0800_0020,
            )
            .unwrap()
            .unwrap();
        let id = open.file_id();
        files.commit_open(open);
        let before_quota = files.clone();
        let quota = files.plan_writable_write(id, &vec![0; 1025]).unwrap_err();
        assert_eq!(quota.code, "writable-profile-quota-exceeded");
        assert_eq!(files, before_quota);

        let write = files.plan_writable_write(id, b"new").unwrap();
        files.commit_writable_write(write);
        let retirement = files.plan_process_retirement(id, &BTreeSet::new()).unwrap();
        files.commit_retirement(retirement);
        let restored = &files.writable_profile().unwrap().files[&21];
        assert_eq!(restored.working_bytes, b"old");
        assert_eq!(restored.generation, 1);
        assert!(!restored.dirty);
    }
}
